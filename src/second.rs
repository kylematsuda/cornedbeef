//! A naive map with open addressing and quadratic probing.

use core::hash::{BuildHasher, Hash};

use crate::{fix_capacity, make_hash, DefaultHashBuilder};

pub enum Bucket<K, V> {
    Empty,
    Tombstone,
    Full(K, V),
}

impl<K, V> Bucket<K, V> {
    pub fn into_inner(self) -> Option<(K, V)> {
        if let Self::Full(k, v) = self {
            Some((k, v))
        } else {
            None
        }
    }

    pub fn as_inner(&self) -> Option<(&K, &V)> {
        if let Self::Full(k, v) = self {
            Some((k, v))
        } else {
            None
        }
    }

    pub fn as_mut(&mut self) -> Option<(&mut K, &mut V)> {
        if let Self::Full(k, v) = self {
            Some((k, v))
        } else {
            None
        }
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
    storage: Box<[Bucket<K, V>]>,
}

impl<K, V> Map<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = fix_capacity(capacity);

        let storage = (0..capacity).map(|_| Bucket::Empty).collect();

        Self {
            hasher: DefaultHashBuilder::default(),
            n_items: 0,
            n_occupied: 0,
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
    fn probe_find(&self, k: &K) -> ProbeResult {
        let mut current = self.bucket_index(k);
        let initial_index = current;
        let mut step = 1;

        loop {
            match &self.storage[current] {
                Bucket::Empty => return ProbeResult::Empty(current),
                Bucket::Full(kk, _) if kk == k => {
                    return ProbeResult::Full(current);
                }
                // Keep probing.
                Bucket::Tombstone | Bucket::Full(..) => {}
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
            ProbeResult::Full(index) => self.storage[index].as_inner().map(|(_, v)| v),
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
                self.storage[index] = Bucket::Full(k, v);
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
                let old_bucket = std::mem::replace(&mut self.storage[index], Bucket::Tombstone);
                // Important to decrement only `n_items` and not `n_occupied` here,
                // since we're leaving a tombstone.
                self.n_items -= 1;
                old_bucket.into_inner().map(|(_, v)| v)
            }
        }
    }

    fn bucket_index(&self, k: &K) -> usize {
        let hash = make_hash(&self.hasher, k);
        usize::rem_euclid(hash as usize, self.n_buckets())
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
        let new_storage = (0..capacity).map(|_| Bucket::Empty).collect();
        let old_storage = std::mem::replace(&mut self.storage, new_storage);

        self.n_items = 0;
        self.n_occupied = 0;

        // Move nodes from `old_storage` to `self.storage`.
        for bucket in Vec::from(old_storage).into_iter() {
            if let Some((k, v)) = bucket.into_inner() {
                self._insert(k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::second::Map;
    crate::generate_tests!(Map, true);
}
