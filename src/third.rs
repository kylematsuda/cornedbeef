//! A Swiss Tables-inspired map with metadata.

use core::hash::{BuildHasher, Hash};

use crate::{fix_capacity, make_hash, DefaultHashBuilder};

const EMPTY: u8 = 0x80;
const TOMBSTONE: u8 = 0xFE;
const MASK: u8 = 0x7F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata(u8);

impl Metadata {
    #[inline]
    pub fn from_hash(hash: u64) -> Self {
        Self((hash & (MASK as u64)) as u8)
    }

    #[inline]
    pub fn from_h2(h2: u8) -> Self {
        Self(h2 & 0x7F)
    }

    pub fn from_key<S: BuildHasher, K: Hash>(hasher: &S, k: &K) -> Self {
        let hash = make_hash(hasher, k);
        Self::from_hash(hash)
    }

    #[inline]
    pub fn empty() -> Self {
        Self(EMPTY)
    }

    #[inline]
    pub fn tombstone() -> Self {
        Self(TOMBSTONE)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0 == EMPTY
    }

    #[inline]
    pub fn is_tombstone(&self) -> bool {
        self.0 == TOMBSTONE
    }

    #[inline]
    pub fn is_value(&self) -> bool {
        self.control() == 0x0
    }

    #[inline]
    pub fn control(&self) -> u8 {
        self.0 >> 7
    }

    #[inline]
    pub fn h2(&self) -> u8 {
        self.0 & MASK
    }
}

pub enum ProbeResult {
    Empty(usize),
    Full(usize),
    End,
}

pub struct Map<K, V, S: BuildHasher = DefaultHashBuilder> {
    hasher: S,
    n_items: usize,    // Number of live items
    n_occupied: usize, // Number of occupied buckets
    storage: Box<[Option<(K, V)>]>,
    metadata: Box<[Metadata]>,
}

impl<K, V> Map<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = fix_capacity(capacity);

        let storage = (0..capacity)
            .map(|_| None)
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let metadata = (0..capacity)
            .map(|_| Metadata::empty())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            hasher: DefaultHashBuilder::default(),
            n_items: 0,
            n_occupied: 0,
            storage,
            metadata,
        }
    }
}

impl<K, V> Default for Map<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Map<K, V>
where
    K: PartialEq + Eq + Hash,
{
    fn probe_find(&self, k: &K) -> ProbeResult {
        let (mut current, h2) = self.bucket_index_and_h2(k);
        let initial_index = current;
        let mut step = 1;

        loop {
            let meta = &self.metadata[current];

            if meta.is_empty() {
                return ProbeResult::Empty(current);
            } else if meta.is_value() && meta.h2() == h2 {
                let (kk, _) = self.storage[current].as_ref().unwrap();
                if kk == k {
                    return ProbeResult::Full(current);
                }
            }

            current = usize::rem_euclid(current + step, self.n_buckets());
            step += 1;

            // We've seen every element in `storage`!
            if current == initial_index {
                return ProbeResult::End;
            }
        }
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        match self.probe_find(k) {
            ProbeResult::Empty(_) | ProbeResult::End => None,
            ProbeResult::Full(index) => self.storage[index].as_ref().map(|(_, v)| v),
        }
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        match self.probe_find(k) {
            ProbeResult::Empty(_) | ProbeResult::End => None,
            ProbeResult::Full(index) => self.storage[index].as_mut().map(|(_, v)| v),
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
            ProbeResult::Empty(index) => {
                self.metadata[index] = Metadata::from_key(&self.hasher, &k);
                self.storage[index] = Some((k, v));
                self.n_items += 1;
                self.n_occupied += 1;
                None
            }
            ProbeResult::Full(index) => {
                let (_, vv) = self.storage[index].as_mut().unwrap();
                Some(std::mem::replace(vv, v))
            }
            ProbeResult::End => {
                panic!("backing storage is full, we didn't resize correctly")
            }
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        match self.probe_find(k) {
            ProbeResult::Empty(_) | ProbeResult::End => None,
            ProbeResult::Full(index) => {
                let old_bucket = self.storage[index].take();
                self.metadata[index] = Metadata::tombstone();
                self.n_items -= 1;
                old_bucket.map(|(_, v)| v)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.n_items
    }

    pub fn is_empty(&self) -> bool {
        self.n_items == 0
    }

    /// Used for testing
    #[inline]
    pub(crate) fn n_buckets(&self) -> usize {
        self.storage.len()
    }

    fn bucket_index_and_h2(&self, k: &K) -> (usize, u8) {
        let hash = make_hash(&self.hasher, k);
        let (h1, h2) = (hash >> 7, (hash & 0x7F) as u8);
        let index = usize::rem_euclid(h1 as usize, self.n_buckets());
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
        let new_storage = (0..capacity)
            .map(|_| None)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let old_storage = std::mem::replace(&mut self.storage, new_storage);

        // We can throw away the old metadata, we need to recompute it anyway.
        self.metadata = (0..capacity)
            .map(|_| Metadata::empty())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        self.n_items = 0;
        self.n_occupied = 0;

        // Move nodes from `old_storage` to `self.storage`.
        // The crazy iterator flatten was suggested by Clippy...
        for (k, v) in Vec::from(old_storage).into_iter().flatten() {
            self._insert(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::third::Map;
    crate::generate_tests!(Map, true);
}