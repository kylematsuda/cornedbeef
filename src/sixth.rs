//! A Swiss Tables-inspired map with metadata.
//! This uses a ton of unsafe to put the metadata and the storage array in the same allocation.

use core::hash::{BuildHasher, Hash};
use std::alloc::{Allocator, Global, Layout};
use std::mem::MaybeUninit;
use std::ptr::NonNull;

use crate::{fix_capacity, make_hash, DefaultHashBuilder};

const EMPTY: u8 = 0x80;
const TOMBSTONE: u8 = 0xFE;
const MASK: u8 = 0x7F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata(u8);

impl Metadata {
    #[inline]
    pub fn from_h2(h2: u8) -> Self {
        Self(h2 & MASK)
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
        (self.0 >> 7) == 0x0
    }

    #[inline]
    pub fn h2(&self) -> u8 {
        self.0 & MASK
    }
}

pub enum ProbeResult {
    Empty(usize, u8),
    Full(usize),
    End,
}

/// Returns a pair `(layout, offset)`, where `offset` is the offset in bytes from the beginning of
/// the layout to the start of the `storage`.
fn layout_for_capacity<K, V>(capacity: usize) -> (Layout, usize) {
    Layout::array::<Metadata>(capacity)
        .unwrap()
        .extend(Layout::array::<(K, V)>(capacity).unwrap())
        .unwrap()
}

/// Allocate backing storage with `capacity`.
///
/// Pretty sure `capacity` needs to be nonzero for this to be sound.
unsafe fn allocate_for_capacity<A: Allocator, K, V>(
    allocator: &A,
    capacity: usize,
) -> (NonNull<Metadata>, NonNull<MaybeUninit<(K, V)>>) {
    let (layout, start_of_storage) = layout_for_capacity::<K, V>(capacity);

    let allocation = if let Ok(ptr) = allocator.allocate(layout) {
        ptr
    } else {
        // Abort the program.
        std::alloc::handle_alloc_error(layout)
    };

    let metadata = allocation.as_mut_ptr().cast::<Metadata>();
    let metadata = NonNull::new(metadata).unwrap();
    let storage = allocation
        .as_mut_ptr()
        .offset(start_of_storage as isize)
        .cast::<MaybeUninit<(K, V)>>();
    let storage = NonNull::new(storage).unwrap();

    // Initialize metadata.
    // We'll leave storage uninitialized.
    std::ptr::write_bytes(metadata.as_ptr(), EMPTY, capacity);

    (metadata, storage)
}

pub struct Map<K, V, S: BuildHasher = DefaultHashBuilder, A: Allocator + Clone = Global> {
    hasher: S,
    allocator: A,
    n_items: usize,    // Number of live items
    n_occupied: usize, // Number of occupied buckets
    n_buckets: usize,  // Number of total buckets
    /// SAFETY:
    /// Two invariants:
    /// - `metadata` and `storage` are non-null iff `n_buckets > 0`.
    /// - `storage[i]` is initialized if `metadata[i].is_value()`.
    metadata: NonNull<Metadata>,
    storage: NonNull<MaybeUninit<(K, V)>>,
    _ph: std::marker::PhantomData<(K, V)>,
}

impl<K, V, S, A> Drop for Map<K, V, S, A>
where
    S: BuildHasher,
    A: Allocator + Clone,
{
    fn drop(&mut self) {
        if std::mem::needs_drop::<(K, V)>() {
            for offset in 0..(self.n_buckets as isize) {
                unsafe {
                    let metadata = self.metadata.as_ptr().offset(offset);
                    let storage = self.storage.as_ptr().offset(offset);

                    if (*metadata).is_value() {
                        let (_k, _v) = std::ptr::read(storage).assume_init();
                    }
                }
            }
        }

        if self.n_buckets > 0 {
            let (layout, _) = layout_for_capacity::<K, V>(self.n_buckets);
            unsafe {
                self.allocator.deallocate(self.metadata.cast(), layout);
            }
        }
    }
}

impl<K, V> Map<K, V> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = fix_capacity(capacity);
        let allocator = Global::default();

        let (metadata, storage) = if capacity > 0 {
            unsafe { allocate_for_capacity(&allocator, capacity) }
        } else {
            (NonNull::dangling(), NonNull::dangling())
        };

        Self {
            hasher: DefaultHashBuilder::default(),
            allocator,
            n_items: 0,
            n_occupied: 0,
            n_buckets: capacity,
            storage,
            metadata,
            _ph: std::marker::PhantomData,
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
    /// SAFETY: `self.metadata` and `self.storage` can't be null!
    ///
    /// Only call this if `self.n_buckets > 0`.
    unsafe fn probe_find(&self, k: &K) -> ProbeResult {
        let (mut current, h2) = self.bucket_index_and_h2(k);
        let initial_index = current;
        let mut step = 1;

        loop {
            let meta = unsafe { *self.metadata.as_ptr().offset(current as isize) };

            if meta.is_empty() {
                return ProbeResult::Empty(current, h2);
            } else if meta.is_value() && meta.h2() == h2 {
                // SAFETY: we checked the invariant that `meta.is_value()`.
                let (kk, _) =
                    unsafe { (*self.storage.as_ptr().offset(current as isize)).assume_init_ref() };
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
        if self.n_buckets == 0 {
            return None;
        }
        match unsafe { self.probe_find(k) } {
            ProbeResult::Empty(..) | ProbeResult::End => None,
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, v) =
                    unsafe { (*self.storage.as_ptr().offset(index as isize)).assume_init_ref() };
                Some(v)
            }
        }
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        if self.n_buckets == 0 {
            return None;
        }
        match unsafe { self.probe_find(k) } {
            ProbeResult::Empty(..) | ProbeResult::End => None,
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, v) =
                    unsafe { (*self.storage.as_ptr().offset(index as isize)).assume_init_mut() };
                Some(v)
            }
        }
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        if self.needs_resize() {
            self.resize();
        }
        unsafe { self._insert(k, v) }
    }

    /// SAFETY: `self.n_buckets > 0`, and `self.n_buckets` is big enough to hold the new item.
    unsafe fn _insert(&mut self, k: K, v: V) -> Option<V> {
        match self.probe_find(&k) {
            ProbeResult::Empty(index, h2) => {
                let index = index as isize;
                std::ptr::write(self.metadata.as_ptr().offset(index), Metadata::from_h2(h2));
                (*self.storage.as_ptr().offset(index)).write((k, v));
                self.n_items += 1;
                self.n_occupied += 1;
                None
            }
            ProbeResult::Full(index) => {
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, vv) = (*self.storage.as_ptr().offset(index as isize)).assume_init_mut();
                Some(std::mem::replace(vv, v))
            }
            ProbeResult::End => {
                panic!("backing storage is full, we didn't resize correctly")
            }
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        if self.n_buckets == 0 {
            return None;
        }
        match unsafe { self.probe_find(k) } {
            ProbeResult::Empty(..) | ProbeResult::End => None,
            ProbeResult::Full(index) => {
                let old_bucket = unsafe {
                    std::ptr::replace(
                        self.storage.as_ptr().offset(index as isize),
                        MaybeUninit::uninit(),
                    )
                };
                // SAFETY: `ProbeResult::Full` implies that `self.storage[index]` is initialized.
                let (_, vv) = unsafe { old_bucket.assume_init() };
                unsafe {
                    std::ptr::write(
                        self.metadata.as_ptr().offset(index as isize),
                        Metadata::tombstone(),
                    );
                }
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

    /// Used for tests
    #[inline]
    fn n_buckets(&self) -> usize {
        self.n_buckets
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
        let old_capacity = self.n_buckets();

        // Calculate the new capacity.
        let capacity = match old_capacity {
            0 => 16,
            x => x * 2,
        };

        let (new_metadata, new_storage) =
            unsafe { allocate_for_capacity(&self.allocator, capacity) };

        self.n_buckets = capacity;
        self.n_items = 0;
        self.n_occupied = 0;

        // Make sure to early return if our old capacity was zero!
        if old_capacity == 0 {
            self.metadata = new_metadata;
            self.storage = new_storage;
            return;
        }

        // Set `self.storage` to a new array.
        //
        // Trying to use `std::ptr::swap` here is UB :(
        let old_metadata = self.metadata;
        let old_storage = self.storage;
        self.metadata = new_metadata;
        self.storage = new_storage;

        // Move nodes from `old_storage` to `self.storage`.
        for offset in 0..(old_capacity as isize) {
            unsafe {
                let metadata = old_metadata.as_ptr().offset(offset);
                let storage = old_storage.as_ptr().offset(offset);

                if (*metadata).is_value() {
                    // SAFETY: we just checked the invariant above.
                    let (k, v) = std::ptr::read(storage).assume_init();
                    self._insert(k, v);
                }
            }
        }

        let (old_layout, _) = layout_for_capacity::<K, V>(old_capacity);
        unsafe {
            self.allocator.deallocate(old_metadata.cast(), old_layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sixth::Map;

    #[test]
    fn drop_empty_map() {
        let _ = Map::<String, String>::new();
    }

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

        let buckets = map.n_buckets();
        for i in range.clone() {
            assert_eq!(map.remove(&i), Some(i));
        }
        assert_eq!(map.len(), 0);
        assert_eq!(buckets, map.n_buckets());

        for i in range {
            map.insert(i, i);
        }
        assert_eq!(map.len(), 1000);
        assert_eq!(buckets * 2, map.n_buckets());
    }

    #[test]
    fn insert_nontrivial_drop() {
        let mut map = Map::new();
        let items = (0..1000).map(|i| (i.to_string(), i.to_string()));

        for (k, v) in items {
            map.insert(k, v);
        }
        assert_eq!(map.len(), 1000);
    }

    #[test]
    fn insert_borrowed_data() {
        let items = (0..1000)
            .map(|i| (i.to_string(), i.to_string()))
            .collect::<Vec<_>>();
        let mut map = Map::new();

        for (k, v) in &items {
            map.insert(k, v);
        }
        assert_eq!(map.len(), 1000);
    }
}
