//! A naive map with separate chaining.

use core::hash::{BuildHasher, Hash};
use std::collections::LinkedList;

use crate::{fix_capacity, make_hash, DefaultHashBuilder};

pub struct Map<K, V, S: BuildHasher = DefaultHashBuilder> {
    hasher: S,
    n_items: usize,
    storage: Box<[LinkedList<(K, V)>]>,
}

impl<K, V> Map<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = fix_capacity(capacity);

        let storage = (0..capacity)
            .map(|_| LinkedList::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            hasher: DefaultHashBuilder::default(),
            n_items: 0,
            storage,
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
    pub fn get(&self, k: &K) -> Option<&V> {
        let index = self.bucket_index(k);
        for (kk, vv) in self.storage[index].iter() {
            if kk == k {
                return Some(vv);
            }
        }
        None
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        let index = self.bucket_index(k);
        for (kk, vv) in self.storage[index].iter_mut() {
            if kk == k {
                return Some(vv);
            }
        }
        None
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        if self.needs_resize() {
            self.resize();
        }
        self._insert(k, v)
    }

    fn _insert(&mut self, k: K, v: V) -> Option<V> {
        let index = self.bucket_index(&k);
        for (kk, vv) in self.storage[index].iter_mut() {
            if kk == &k {
                return Some(std::mem::replace(vv, v));
            }
        }

        // If we reached here, we need to add a new node for this item.
        self.storage[index].push_front((k, v));
        self.n_items += 1;
        None
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        let index = self.bucket_index(k);
        let mut list_index = None;

        for (i, (kk, _)) in self.storage[index].iter().enumerate() {
            if kk == k {
                list_index = Some(i);
                break;
            }
        }

        let list_index = list_index?;
        let mut tail = self.storage[index].split_off(list_index);
        let (_k, v) = tail.pop_front().unwrap();
        self.storage[index].append(&mut tail);
        self.n_items -= 1;

        Some(v)
    }

    pub fn len(&self) -> usize {
        self.n_items
    }

    pub fn is_empty(&self) -> bool {
        self.n_items == 0
    }

    fn bucket_index(&self, k: &K) -> usize {
        let hash = make_hash(&self.hasher, k);
        usize::rem_euclid(hash as usize, self.storage.len())
    }

    fn needs_resize(&self) -> bool {
        // Using a load factor of 7/8.
        self.storage.len() == 0 || ((self.n_items * 8) / self.storage.len()) > 7
    }

    fn resize(&mut self) {
        // Calculate the new capacity.
        let capacity = match self.storage.len() {
            0 => 16,
            x => x * 2,
        };

        // Set `self.storage` to a new array.
        let new_storage = (0..capacity)
            .map(|_| LinkedList::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let old_storage = std::mem::replace(&mut self.storage, new_storage);

        self.n_items = 0;

        // Move nodes from `old_storage` to `self.storage`.
        for mut bucket in Vec::from(old_storage).into_iter() {
            while !bucket.is_empty() {
                // We want to reuse the nodes, so we can't pop them.
                let tail = bucket.split_off(1);
                let mut head = bucket;
                bucket = tail;

                let (k, _) = head.front().unwrap();
                let index = self.bucket_index(k);
                self.storage[index].append(&mut head);
                self.n_items += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::first::Map;

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
        assert_eq!(buckets, map.storage.len());
    }
}
