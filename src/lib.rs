use core::hash::{BuildHasher, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::collections::LinkedList;

// Use std's default hasher.
pub type DefaultHashBuilder = core::hash::BuildHasherDefault<DefaultHasher>;

fn make_hash<S: BuildHasher, K: core::hash::Hash>(build_hasher: &S, key: &K) -> u64 {
    let mut hasher = build_hasher.build_hasher();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Naive first Map using separate chaining for now.
pub struct CbHashMap<K, V, S: BuildHasher = DefaultHashBuilder> {
    hasher: S,
    n_buckets: usize,
    n_items: usize,
    storage: Box<[LinkedList<(K, V)>]>,
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
            .map(|_| LinkedList::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            hasher: DefaultHashBuilder::default(),
            n_buckets: capacity,
            n_items: 0,
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
        let index = self.key_to_index(&k);
        match self.get_mut_inner(&k, index) {
            Some(vv) => {
                let mut v = v;
                std::mem::swap(&mut v, vv);
                Some(v)
            }
            None => {
                self.insert_at(index, k, v);
                None
            }
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        let index = self.key_to_index(k);

        // Find the index of `k`
        let split_index = self.storage[index]
            .iter()
            .enumerate()
            .find(|(_, (kk, _))| kk == k)
            .map(|(j, _)| j);

        if let Some(j) = split_index {
            // Take the item we want to remove
            let mut tail = self.storage[index].split_off(j);
            let item = tail.pop_front();
            self.n_items -= 1;

            // Connect the remaining elements to the original list
            self.storage[index].append(&mut tail);

            item.map(|(_, v)| v)
        } else {
            None
        }
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        let index = self.key_to_index(k);
        self.get_inner(k, index)
    }

    fn get_inner(&self, k: &K, index: usize) -> Option<&V> {
        self.storage[index]
            .iter()
            .find(|(kk, _)| k == kk)
            .map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        let index = self.key_to_index(k);
        self.get_mut_inner(k, index)
    }

    fn get_mut_inner(&mut self, k: &K, index: usize) -> Option<&mut V> {
        self.storage[index]
            .iter_mut()
            .find(|(kk, _)| k == kk)
            .map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.n_items
    }

    fn insert_at(&mut self, index: usize, k: K, v: V) {
        self.storage[index].push_front((k, v));
        self.n_items += 1;
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
            .map(|_| LinkedList::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let old_storage = std::mem::replace(&mut self.storage, new_storage);
        self.n_buckets = capacity;

        for chain in Vec::from(old_storage).into_iter() {
            for (k, v) in chain.into_iter() {
                let index = self.key_to_index(&k);
                self.storage[index].push_front((k, v));
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
            assert!(map.get(&i).is_some());
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
}
