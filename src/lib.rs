use core::hash::{BuildHasher, Hasher};
use std::collections::hash_map::DefaultHasher;

// Use std's default hasher.
pub type DefaultHashBuilder = core::hash::BuildHasherDefault<DefaultHasher>;

fn make_hash<S: BuildHasher, K: core::hash::Hash>(build_hasher: &S, key: &K) -> u64 {
    let mut hasher = build_hasher.build_hasher();
    key.hash(&mut hasher);
    hasher.finish()
}

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

/// Open addressing, quadratic probing
pub struct CbHashMap<K, V, S: BuildHasher = DefaultHashBuilder> {
    hasher: S,
    n_buckets: usize,
    // This will increment when adding an item,
    // but DOES NOT DECREMENT when removing (since we'll still have a tombstone).
    n_occupied: usize,
    n_items: usize,
    metadata: Box<[Metadata]>,
    storage: Box<[Option<(K, V)>]>,
}

impl<K, V> CbHashMap<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = match capacity {
            0 => 0,
            x if x < 16 => 16,
            x => 1 << (x.ilog2() + 1),
        };
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
            n_buckets: capacity,
            n_items: 0,
            n_occupied: 0,
            storage,
            metadata,
        }
    }
}

impl<K, V> Default for CbHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> CbHashMap<K, V>
where
    K: PartialEq + Eq + core::hash::Hash,
{
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        if self.needs_grow() {
            self.grow_table();
        }
        self.insert_unchecked(k, v)
    }

    /// Don't grow the table
    fn insert_unchecked(&mut self, k: K, v: V) -> Option<V> {
        let (mut current, h2) = self.key_to_index(&k);
        let mut step = 1;
        loop {
            let meta = self.metadata[current];
            if meta.is_empty() || meta.is_tombstone() {
                self.n_occupied += meta.is_empty() as usize;
                self.n_items += 1;
                self.storage[current] = Some((k, v));
                self.metadata[current] = Metadata::from_h2(h2);
                return None;
            } else if meta.is_value() && meta.h2() == h2 {
                let (kk, vv) = self.storage.get_mut(current).unwrap().as_mut().unwrap();
                if kk == &k {
                    return Some(std::mem::replace(vv, v));
                }
            }
            current = usize::rem_euclid(current + step, self.n_buckets);
            step += 1;
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        let (mut current, h2) = self.key_to_index(k);
        let mut step = 1;
        loop {
            let meta = self.metadata[current];
            if meta.is_empty() {
                return None;
            } else if meta.is_value() && h2 == meta.h2() {
                let (kk, _) = self.storage[current].as_ref().unwrap();
                if kk == k {
                    self.n_items -= 1;
                    self.metadata[current] = Metadata::tombstone();
                    let (_, vv) = self.storage[current].take().unwrap();
                    return Some(vv);
                }
            }
            current = usize::rem_euclid(current + step, self.n_buckets);
            step += 1;
        }
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        let (mut current, h2) = self.key_to_index(k);
        let mut step = 1;
        loop {
            let meta = self.metadata[current];
            if meta.is_empty() {
                return None;
            } else if meta.is_value() && h2 == meta.h2() {
                let (kk, vv) = self.storage[current].as_ref().unwrap();
                if kk == k {
                    return Some(vv);
                }
            }
            current = usize::rem_euclid(current + step, self.n_buckets);
            step += 1;
        }
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        let (mut current, h2) = self.key_to_index(k);
        let mut step = 1;
        let index = loop {
            let meta = self.metadata[current];
            if meta.is_empty() {
                break None;
            } else if meta.is_value() && h2 == meta.h2() {
                let (kk, _) = self.storage[current].as_ref().unwrap();
                if kk == k {
                    break Some(current);
                }
            }
            current = usize::rem_euclid(current + step, self.n_buckets);
            step += 1;
        }?;
        self.storage[index].as_mut().map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.n_items
    }

    pub fn is_empty(&self) -> bool {
        self.n_items == 0
    }

    fn hashes(&self, k: &K) -> (u64, u8) {
        let hash = make_hash(&self.hasher, &k);
        let h2 = (hash & 0x7F) as u8;
        let h1 = hash >> 7;
        (h1, h2)
    }

    fn key_to_index(&self, k: &K) -> (usize, u8) {
        let (h1, h2) = self.hashes(k);
        (usize::rem_euclid(h1 as usize, self.n_buckets), h2)
    }

    fn needs_grow(&self) -> bool {
        // Load factor set at 7/8 for now
        self.n_buckets == 0 || ((self.n_items * 8).saturating_div(self.n_buckets) > 7)
    }

    fn grow_table(&mut self) {
        let capacity = if self.n_buckets == 0 {
            16
        } else {
            self.n_buckets * 2
        };
        let new_storage = (0..capacity)
            .map(|_| None)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let old_storage = std::mem::replace(&mut self.storage, new_storage);
        let new_metadata = (0..capacity)
            .map(|_| Metadata::empty())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let old_metadata = std::mem::replace(&mut self.metadata, new_metadata);

        self.n_buckets = capacity;
        self.n_items = 0;
        self.n_occupied = 0;

        for (&meta, slot) in old_metadata.iter().zip(Vec::from(old_storage).into_iter()) {
            if meta.is_value() {
                let (k, v) = slot.unwrap();
                self.insert_unchecked(k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::CbHashMap;

    #[test]
    fn insert() {
        let mut map = CbHashMap::new();

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
        let mut map = CbHashMap::new();

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
        let mut map = CbHashMap::new();

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
        let mut map = CbHashMap::new();
        let range = 0..1000;

        for i in range.clone() {
            map.insert(i, i);
        }
        assert_eq!(map.len(), 1000);

        let buckets = map.n_buckets;
        for i in range.clone() {
            assert_eq!(map.remove(&i), Some(i));
        }
        assert_eq!(map.len(), 0);
        assert_eq!(buckets, map.n_buckets);

        for i in range {
            map.insert(i, i);
        }
        assert_eq!(map.len(), 1000);
        assert_eq!(buckets, map.n_buckets);
    }
}
