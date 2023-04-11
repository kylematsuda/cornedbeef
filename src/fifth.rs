//! A Swiss Tables-inspired map with metadata.
//! Uses SSE instructions on the metadata.

use core::hash::{BuildHasher, Hash};
use std::intrinsics::{likely, unlikely};
use std::marker::PhantomData;
use std::mem::MaybeUninit;

use crate::metadata::{self, Metadata};
use crate::sse::{self, GROUP_SIZE};
use crate::{fast_rem, fix_capacity, make_hash, DefaultHashBuilder};

pub enum ProbeResult {
    Empty(usize, u8),
    Full(usize),
}

pub struct Map<K, V, S: BuildHasher = DefaultHashBuilder> {
    hasher: S,
    n_items: usize,    // Number of live items
    n_occupied: usize, // Number of occupied buckets
    /// Safety: we maintain the following invariant:
    /// `self.storage[i]` is initialized whenever `metadata::is_full(self.metadata[i])`.
    storage: Box<[MaybeUninit<(K, V)>]>,
    /// Contains an extra `GROUP_SIZE` elements to avoid wrapping SIMD access
    metadata: Box<[Metadata]>,
    _ph: PhantomData<(K, V)>,
}

impl<K, V> Map<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = fix_capacity(capacity);

        let storage = Box::new_uninit_slice(capacity);

        let metadata = if capacity == 0 {
            Box::new([])
        } else {
            (0..(capacity + GROUP_SIZE))
                .map(|_| metadata::empty())
                .collect::<Vec<_>>()
                .into_boxed_slice()
        };

        Self {
            hasher: DefaultHashBuilder::default(),
            n_items: 0,
            n_occupied: 0,
            storage,
            metadata,
            _ph: PhantomData,
        }
    }
}

impl<K, V> Default for Map<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<#[may_dangle] K, #[may_dangle] V, S> Drop for Map<K, V, S>
where
    S: BuildHasher,
{
    fn drop(&mut self) {
        if std::mem::needs_drop::<(K, V)>() {
            for (i, &m) in self.metadata.iter().take(self.n_buckets()).enumerate() {
                if metadata::is_full(m) {
                    unsafe { self.storage[i].assume_init_drop() };
                }
            }
        }
    }
}

impl<K, V> Clone for Map<K, V>
where
    K: Clone + PartialEq + Eq + Hash,
    V: Clone,
{
    fn clone(&self) -> Self {
        let mut other = Self::with_capacity(self.n_buckets());
        assert_eq!(self.n_buckets(), other.n_buckets());

        for (i, m) in self.metadata.iter().enumerate().take(self.n_buckets()) {
            if metadata::is_full(*m) {
                let (k, v) = unsafe { self.storage[i].assume_init_ref() };
                other.storage[i].write((k.clone(), v.clone()));

                // Important: Only update the metadata after we successfully clone!
                // If cloning panics, then updating the metadata before cloning
                // leads to a read of uninitialized memory when `other` is dropped.
                other.set_metadata(i, *m);
                other.n_items += 1;
                other.n_occupied += 1;
            }
        }
        other
    }
}

impl<K, V, S: BuildHasher> Map<K, V, S> {
    pub fn len(&self) -> usize {
        self.n_items
    }

    pub fn is_empty(&self) -> bool {
        self.n_items == 0
    }

    /// Used for tests
    #[inline]
    fn n_buckets(&self) -> usize {
        self.storage.len()
    }
}

impl<K, V> Map<K, V>
where
    K: PartialEq + Eq + Hash,
{
    fn probe_find(&self, k: &K) -> ProbeResult {
        let (mut current, h2) = self.bucket_index_and_h2(k);

        for step in 0..self.n_buckets() {
            current = fast_rem(current + step * GROUP_SIZE, self.n_buckets());
            let group = sse::Group::from_slice(&self.metadata[current..]);

            // First, check full buckets.
            let candidates = sse::MaskIter::forward(group.to_candidates(h2));
            for i in candidates {
                let index = fast_rem(current + i, self.n_buckets());
                // SAFETY: we checked the invariant that `meta.is_value()`.
                let (kk, _) = unsafe { self.storage.get_unchecked(index).assume_init_ref() };
                if kk == k {
                    return ProbeResult::Full(index);
                }
            }

            // If we've made it to here, our key isn't in this group.
            // Look for the first empty bucket.
            let empty = sse::find_first(group.to_empties());
            if let Some(i) = empty {
                let index = fast_rem(current + i, self.n_buckets());
                return ProbeResult::Empty(index, h2);
            }
        }
        unreachable!("backing storage is full, we didn't resize correctly")
    }

    fn set_metadata(&mut self, index: usize, value: Metadata) {
        let index = fast_rem(index, self.n_buckets());
        let index2 = fast_rem(index.wrapping_sub(GROUP_SIZE), self.n_buckets()) + GROUP_SIZE;
        self.metadata[index] = value;
        self.metadata[index2] = value;
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        match self.probe_find(k) {
            ProbeResult::Empty(..) => None,
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, v) = unsafe { self.storage[index].assume_init_ref() };
                Some(v)
            }
        }
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        match self.probe_find(k) {
            ProbeResult::Empty(..) => None,
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, v) = unsafe { self.storage[index].assume_init_mut() };
                Some(v)
            }
        }
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        if unlikely(self.needs_resize()) {
            self.resize();
        }
        self._insert(k, v)
    }

    fn _insert(&mut self, k: K, v: V) -> Option<V> {
        match self.probe_find(&k) {
            ProbeResult::Empty(index, h2) => {
                self.set_metadata(index, metadata::from_h2(h2));
                self.storage[index].write((k, v));
                self.n_items += 1;
                self.n_occupied += 1;
                None
            }
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, vv) = unsafe { self.storage[index].assume_init_mut() };
                Some(std::mem::replace(vv, v))
            }
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        match self.probe_find(k) {
            ProbeResult::Empty(..) => None,
            ProbeResult::Full(index) => {
                let old_bucket = std::mem::replace(&mut self.storage[index], MaybeUninit::uninit());
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, vv) = unsafe { old_bucket.assume_init() };

                let metadata_value = self.decide_tombstone_or_empty(index);
                self.set_metadata(index, metadata_value);

                self.n_items -= 1;
                if metadata::is_empty(metadata_value) {
                    self.n_occupied -= 1;
                }

                Some(vv)
            }
        }
    }

    /// We can set back to empty unless we're inside a run of `GROUP_SIZE`
    /// non-empty buckets.
    fn decide_tombstone_or_empty(&self, index: usize) -> Metadata {
        // Degenerate case where n_buckets is GROUP_SIZE
        if self.n_buckets() == GROUP_SIZE {
            return metadata::empty();
        }

        let probe_current = sse::Group::from_slice(&self.metadata[index..]);
        let next_empty = sse::find_first(probe_current.to_empties()).unwrap_or(GROUP_SIZE);

        let previous = fast_rem(index.wrapping_sub(GROUP_SIZE), self.n_buckets());
        let probe_previous = sse::Group::from_slice(&self.metadata[previous..]);
        let last_empty = sse::find_last(probe_previous.to_empties()).unwrap_or(0);

        // Find the distance between nearest two empty buckets.
        // If it's less than GROUP_SIZE, then all groups containing `index` have
        // at least one empty bucket.
        if likely((next_empty + GROUP_SIZE).saturating_sub(last_empty) < GROUP_SIZE) {
            metadata::empty()
        } else {
            metadata::tombstone()
        }
    }

    fn bucket_index_and_h2(&self, k: &K) -> (usize, u8) {
        let hash = make_hash(&self.hasher, k);
        let (h1, h2) = (hash >> 7, (hash & 0x7F) as u8);
        let index = fast_rem(h1 as usize, self.n_buckets());
        (index, h2)
    }

    #[inline]
    fn needs_resize(&self) -> bool {
        // Using a load factor of 7/8.
        // NOTE: we need to use n_occupied instead of n_items here!
        self.n_buckets() == 0 || ((self.n_occupied * 8) / self.n_buckets()) > 7
    }

    #[cold]
    #[inline(never)]
    fn resize(&mut self) {
        // Calculate the new capacity.
        let capacity = match self.n_buckets() {
            0 => 16,
            x => x * 2,
        };

        // Set `self.storage` to a new array.
        let new_storage = Box::new_uninit_slice(capacity);
        let old_storage = std::mem::replace(&mut self.storage, new_storage);

        let new_metadata = (0..(capacity + GROUP_SIZE))
            .map(|_| metadata::empty())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        // Here, we need to keep the old metadata, as it's unsafe to blindly access the old storage
        // array.
        let old_metadata = std::mem::replace(&mut self.metadata, new_metadata);

        self.n_items = 0;
        self.n_occupied = 0;

        // Chunk metadata and storage into `GROUP_SIZE` chunks
        let metadata_chunks = Vec::from(old_metadata)
            .into_iter()
            .array_chunks::<GROUP_SIZE>();
        let storage_chunks = Vec::from(old_storage)
            .into_iter()
            .array_chunks::<GROUP_SIZE>();

        // Zipping `metadata_chunks` and `storage_chunks` ensures that we correctly ignore the
        // replicated metadata group.
        for (m_chunk, s_chunk) in metadata_chunks.zip(storage_chunks) {
            // Get a mask showing the indices with full buckets.
            let full_mask = sse::Group::from_array(m_chunk).to_fulls();
            // Re-insert each full bucket.
            for (is_full, bucket) in full_mask.to_array().into_iter().zip(s_chunk) {
                if is_full {
                    // Safety: if `is_full`, then we can assume the `bucket` 
                    // is initialized according to our safety invariant.
                    let (k, v) = unsafe { bucket.assume_init() };
                    self._insert(k, v);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fifth::Map;
    crate::generate_tests!(Map, true);
    crate::generate_non_alloc_tests!(Map);
}
