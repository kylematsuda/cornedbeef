//! A Swiss Tables-inspired map with metadata.
//! Uses SSE instructions on the metadata.
//!
//! Warning: This does not work well yet.

use core::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use std::mem::MaybeUninit;

use crate::metadata::{self, Metadata};
use crate::sse::{self, GROUP_SIZE};
use crate::{fast_rem, fix_capacity, make_hash, DefaultHashBuilder};

pub enum ProbeResult {
    Empty(usize, u8),
    Full(usize),
    End,
}

pub struct Map<K, V, S: BuildHasher = DefaultHashBuilder> {
    hasher: S,
    n_items: usize,    // Number of live items
    n_occupied: usize, // Number of occupied buckets
    /// Safety: we maintain the following invariant:
    /// `self.storage[i]` is initialized whenever `self.metadata[i].is_value()`.
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
    /// No idea if this is right, but need to be able to clone to do benchmarks.
    fn clone(&self) -> Self {
        let mut other = Self {
            hasher: DefaultHashBuilder::default(),
            n_items: self.n_items,
            n_occupied: self.n_occupied,
            storage: Box::new_uninit_slice(self.n_buckets()),
            metadata: self.metadata.clone(),
            _ph: PhantomData,
        };

        for (i, m) in self.metadata.iter().enumerate().take(self.storage.len()) {
            if metadata::is_full(*m) {
                let (k, v) = unsafe { self.storage[i].assume_init_ref() };
                other.storage[i].write((k.clone(), v.clone()));
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
            let group = sse::SimdType::from_slice(&self.metadata[current..]);

            // First, check full buckets.
            let mut candidates = sse::get_candidates(group, h2);
            while let Some(i) = sse::find_first(candidates) {
                let index = fast_rem(current + i, self.n_buckets());
                // SAFETY: we checked the invariant that `meta.is_value()`.
                let (kk, _) = unsafe { self.storage.get_unchecked(index).assume_init_ref() };
                if kk == k {
                    return ProbeResult::Full(index);
                }
                candidates.set(i, false);
            }

            // If we've made it to here, our key isn't in this group.
            // Look for the first empty bucket.
            let empty = sse::find_first(sse::get_empty(group));
            if let Some(i) = empty {
                let index = fast_rem(current + i, self.n_buckets());
                return ProbeResult::Empty(index, h2);
            }
        }
        ProbeResult::End
    }

    #[inline]
    fn set_metadata(&mut self, index: usize, value: Metadata) {
        self.metadata[index] = value;
        if index < GROUP_SIZE {
            self.metadata[index + self.n_buckets()] = value;
        }
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        match self.probe_find(k) {
            ProbeResult::Empty(..) | ProbeResult::End => None,
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, v) = unsafe { self.storage[index].assume_init_ref() };
                Some(v)
            }
        }
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        match self.probe_find(k) {
            ProbeResult::Empty(..) | ProbeResult::End => None,
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, v) = unsafe { self.storage[index].assume_init_mut() };
                Some(v)
            }
        }
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        if self.needs_resize() {
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
            ProbeResult::End => {
                panic!("backing storage is full, we didn't resize correctly")
            }
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        match self.probe_find(k) {
            ProbeResult::Empty(..) | ProbeResult::End => None,
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

    /// We can set back to empty unless we're in the middle of a bunch of tombstone or full.
    fn decide_tombstone_or_empty(&self, index: usize) -> Metadata {
        // Pathological case where n_buckets is GROUP_SIZE
        if self.n_buckets() == GROUP_SIZE {
            return metadata::empty();
        }

        let probe_current = sse::SimdType::from_slice(&self.metadata[index..]);
        let next_empty = sse::find_first(sse::get_empty(probe_current));

        let previous = fast_rem(index + self.n_buckets() - GROUP_SIZE, self.n_buckets());
        let probe_previous = sse::SimdType::from_slice(&self.metadata[previous..]);
        let last_empty = sse::find_last(sse::get_empty(probe_previous));

        match (last_empty, next_empty) {
            (Some(i), Some(j)) if j + GROUP_SIZE - fast_rem(i, self.n_buckets()) < GROUP_SIZE => {
                metadata::empty()
            }
            _ => metadata::tombstone(),
        }
    }

    fn bucket_index_and_h2(&self, k: &K) -> (usize, u8) {
        let hash = make_hash(&self.hasher, k);
        let (h1, h2) = (hash >> 7, (hash & 0x7F) as u8);
        let index = fast_rem(h1 as usize, self.n_buckets());
        (index, h2)
    }

    fn needs_resize(&self) -> bool {
        // Using a load factor of 7/8.
        // NOTE: we need to use n_occupied instead of n_items here!
        self.n_buckets() == 0 || ((self.n_occupied * 8) / self.n_buckets()) > 7
    }

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

        // Move nodes from `old_storage` to `self.storage`.
        for (&metadata, bucket) in old_metadata.iter().zip(Vec::from(old_storage).into_iter()) {
            if metadata::is_full(metadata) {
                // SAFETY: we just checked the invariant above.
                let (k, v) = unsafe { bucket.assume_init() };
                self._insert(k, v);
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
