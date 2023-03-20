//! A naive map with separate chaining.

use core::hash::{BuildHasher, Hash};
use std::collections::LinkedList;

use crate::{fast_rem, fix_capacity, make_hash, DefaultHashBuilder};

#[derive(Debug, Clone)]
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
            .collect();

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

impl<K, V> Map<K, V> {
    pub fn len(&self) -> usize {
        self.n_items
    }

    pub fn is_empty(&self) -> bool {
        self.n_items == 0
    }

    /// Used for testing
    #[inline]
    fn n_buckets(&self) -> usize {
        self.storage.len()
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

    fn bucket_index(&self, k: &K) -> usize {
        let hash = make_hash(&self.hasher, k);
        fast_rem(hash as usize, self.n_buckets())
        // usize::rem_euclid(hash as usize, self.n_buckets())
    }

    fn needs_resize(&self) -> bool {
        // Using a load factor of 7/8.
        self.n_buckets() == 0 || ((self.n_items * 8) / self.n_buckets()) > 7
    }

    fn resize(&mut self) {
        // Calculate the new capacity.
        let capacity = match self.n_buckets() {
            0 => 16,
            x => x * 2,
        };

        // Set `self.storage` to a new array.
        let new_storage = (0..capacity)
            .map(|_| LinkedList::new())
            .collect();
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
    crate::generate_tests!(Map, false);
    crate::generate_non_alloc_tests!(Map);
}
