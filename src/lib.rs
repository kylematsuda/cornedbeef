use core::hash::{BuildHasher, Hasher};
use std::collections::hash_map::DefaultHasher;

// Use std's default hasher.
pub type DefaultHashBuilder = core::hash::BuildHasherDefault<DefaultHasher>;

fn make_hash<S: BuildHasher, K: core::hash::Hash>(build_hasher: &S, key: &K) -> u64 {
    let mut hasher = build_hasher.build_hasher();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Represent a slot in the array.
/// Open addressing implies we need a tombstone (I think).
pub enum Bucket<T> {
    Empty,
    Tombstone,
    Value(T),
}

impl<T> Bucket<T> {
    /// Insert a new value into the bucket, returning the old value if present.
    pub fn insert(&mut self, val: T) -> Option<T> {
        match self {
            Self::Empty | Self::Tombstone => {
                *self = Self::Value(val);
                None
            }
            Self::Value(old) => Some(std::mem::replace(old, val)),
        }
    }

    /// Remove the value from the bucket, returning the old value if it exists.
    pub fn remove(&mut self) -> Option<T> {
        match self {
            Self::Empty | Self::Tombstone => None,
            Self::Value(_) => {
                if let Self::Value(old) = std::mem::replace(self, Self::Tombstone) {
                    Some(old)
                } else {
                    unreachable!()
                }
            }
        }
    }

    pub fn as_inner(&self) -> Option<&T> {
        if let Self::Value(t) = self {
            Some(t)
        } else {
            None
        }
    }

    pub fn as_inner_mut(&mut self) -> Option<&mut T> {
        if let Self::Value(t) = self {
            Some(t)
        } else {
            None
        }
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
    storage: Box<[Bucket<(K, V)>]>,
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
            .map(|_| Bucket::Empty)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            hasher: DefaultHashBuilder::default(),
            n_buckets: capacity,
            n_items: 0,
            n_occupied: 0,
            storage,
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
        let mut current = self.key_to_index(&k);
        let mut step = 1;
        loop {
            match &mut self.storage[current] {
                Bucket::Value(_) => {}
                x => {
                    self.n_items += 1;
                    self.n_occupied += 1;
                    return x.insert((k, v)).map(|(_, v)| v);
                }
            }
            current = usize::rem_euclid(current + step, self.n_buckets);
            step += 1;
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        let mut current = self.key_to_index(&k);
        let mut step = 1;
        loop {
            match &mut self.storage[current] {
                Bucket::Tombstone => {}
                Bucket::Empty => return None,
                b @ Bucket::Value(_) => {
                    let kk = b.as_inner().map(|(kk, _)| kk).unwrap();
                    if kk == k {
                        self.n_items -= 1;
                        return b.remove().map(|(_, v)| v);
                    }
                }
            }
            current = usize::rem_euclid(current + step, self.n_buckets);
            step += 1;
        }
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        let mut current = self.key_to_index(&k);
        let mut step = 1;
        loop {
            match &self.storage[current] {
                Bucket::Tombstone => {}
                Bucket::Empty => return None,
                Bucket::Value((kk, vv)) => {
                    if kk == k {
                        return Some(vv);
                    }
                }
            }
            current = usize::rem_euclid(current + step, self.n_buckets);
            step += 1;
        }
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        let mut current = self.key_to_index(&k);
        let mut step = 1;

        // Ugh, it's annoying that it won't let me just return from the loop.
        let index = loop {
            match self.storage.get(current).unwrap() {
                Bucket::Tombstone => {}
                Bucket::Empty => break None,
                Bucket::Value((kk, _)) => {
                    if kk == k {
                        break Some(current);
                    }
                }
            }
            current = usize::rem_euclid(current + step, self.n_buckets);
            step += 1;
        };

        let index = if let Some(i) = index {
            i
        } else {
            return None;
        };
        self.storage[index].as_inner_mut().map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.n_items
    }

    fn key_to_index(&self, k: &K) -> usize {
        usize::rem_euclid(make_hash(&self.hasher, &k) as usize, self.n_buckets)
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
            .map(|_| Bucket::Empty)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let old_storage = std::mem::replace(&mut self.storage, new_storage);
        self.n_buckets = capacity;

        self.n_items = 0;
        self.n_occupied = 0;
        for bucket in Vec::from(old_storage).into_iter() {
            if let Bucket::Value((k, v)) = bucket {
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
}
