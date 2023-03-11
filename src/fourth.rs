//! A Swiss Tables-inspired map with metadata.
//! This is similar to the one in `third`, except using MaybeUninit as an optimization.

use core::hash::{BuildHasher, Hash};
use std::mem::MaybeUninit;

use crate::{make_hash, DefaultHashBuilder};

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
    /// Safety: we maintain the following invariant:
    /// `self.storage[i]` is initialized whenever `self.metadata[i].is_value()`.
    storage: Box<[MaybeUninit<(K, V)>]>,
    metadata: Box<[Metadata]>,
}

impl<K, V> Map<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = match capacity {
            0 => 0,
            x if x < 16 => 16,
            y => 1 << (y.ilog2() + 1),
        };

        let storage = Box::new_uninit_slice(capacity);

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
                // SAFETY: we checked the invariant that `meta.is_value()`.
                let (kk, _) = unsafe { self.storage[current].assume_init_ref() };
                if kk == k {
                    return ProbeResult::Full(current);
                }
            }

            current = usize::rem_euclid(current + step, self.storage.len());
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
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, v) = unsafe { self.storage[index].assume_init_ref() };
                Some(v)
            }
        }
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        match self.probe_find(k) {
            ProbeResult::Empty(_) | ProbeResult::End => None,
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
            ProbeResult::Empty(index) => {
                self.metadata[index] = Metadata::from_key(&self.hasher, &k);
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
            ProbeResult::Empty(_) | ProbeResult::End => None,
            ProbeResult::Full(index) => {
                let old_bucket = std::mem::replace(&mut self.storage[index], MaybeUninit::uninit());
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, vv) = unsafe { old_bucket.assume_init() };
                self.metadata[index] = Metadata::tombstone();
                self.n_items -= 1;
                Some(vv)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.n_items
    }

    pub fn is_empty(&self) -> bool {
        self.n_items == 0
    }

    fn bucket_index_and_h2(&self, k: &K) -> (usize, u8) {
        let hash = make_hash(&self.hasher, k);
        let (h1, h2) = (hash >> 7, (hash & 0x7F) as u8);
        let index = usize::rem_euclid(h1 as usize, self.storage.len());
        (index, h2)
    }

    fn needs_resize(&self) -> bool {
        // Using a load factor of 7/8.
        // NOTE: we need to use n_occupied instead of n_items here!
        self.storage.len() == 0 || ((self.n_occupied * 8) / self.storage.len()) > 7
    }

    fn resize(&mut self) {
        // Calculate the new capacity.
        let capacity = match self.storage.len() {
            0 => 16,
            x => x * 2,
        };

        // Set `self.storage` to a new array.
        let new_storage = Box::new_uninit_slice(capacity);
        let old_storage = std::mem::replace(&mut self.storage, new_storage);

        let new_metadata = (0..capacity)
            .map(|_| Metadata::empty())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        // Here, we need to keep the old metadata, as it's unsafe to blindly access the old storage
        // array.
        let old_metadata = std::mem::replace(&mut self.metadata, new_metadata);

        self.n_items = 0;
        self.n_occupied = 0;

        // Move nodes from `old_storage` to `self.storage`.
        for (&metadata, bucket) in old_metadata.iter().zip(Vec::from(old_storage).into_iter()) {
            if metadata.is_value() {
                // SAFETY: we just checked the invariant above.
                let (k, v) = unsafe { bucket.assume_init() };
                self._insert(k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fourth::Map;

    #[test]
    fn insert() {
        let mut map = Map::new();

        for i in 0..1000 {
            map.insert(i, i);
        }

        assert_eq!(map.len(), 1000);

        for i in 0..1000 {
            assert_eq!(map.get(&i), Some(&i));
        }
    }

    #[test]
    fn remove() {
        let mut map = Map::new();

        for i in 0..1000 {
            map.insert(i, i);
        }

        assert_eq!(map.len(), 1000);

        for i in 0..1000 {
            assert_eq!(map.remove(&i), Some(i));
        }

        assert_eq!(map.len(), 0);
    }

    #[test]
    fn miss() {
        let mut map = Map::new();

        for i in 0..1000 {
            map.insert(i, i);
        }

        assert_eq!(map.len(), 1000);

        for i in 1000..2000 {
            assert!(map.get(&i).is_none());
        }

        assert_eq!(map.len(), 1000);
    }

    #[test]
    fn remove_and_reinsert() {
        let mut map = Map::new();
        let range = 0..1000;

        for i in range.clone() {
            map.insert(i, i);
        }
        assert_eq!(map.len(), 1000);

        let buckets = map.storage.len();
        for i in range.clone() {
            assert_eq!(map.remove(&i), Some(i));
        }
        assert_eq!(map.len(), 0);
        assert_eq!(buckets, map.storage.len());

        for i in range {
            map.insert(i, i);
        }
        assert_eq!(map.len(), 1000);
        // Note that the above loop will trigger a resize because we have a ton of tombstones.
        assert_eq!(buckets * 2, map.storage.len());
    }
}
