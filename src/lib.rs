#![feature(
    new_uninit,
    allocator_api,
    slice_ptr_get,
    nonnull_slice_from_raw_parts,
    portable_simd
)]

use core::hash::{BuildHasher, Hasher};
use std::collections::hash_map::DefaultHasher;

#[rustfmt::skip]
pub mod first;
pub mod fifth;
pub mod fourth;
pub mod second;
pub mod sixth;
pub mod third;

mod metadata;
mod sse;

/// Hash builder for std's default hasher.
pub type DefaultHashBuilder = core::hash::BuildHasherDefault<DefaultHasher>;

/// Convenience function for hashing a key.
fn make_hash<S, K>(build_hasher: &S, key: &K) -> u64
where
    S: BuildHasher,
    K: core::hash::Hash,
{
    let mut hasher = build_hasher.build_hasher();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Choose an actual capacity from the requested one.
fn fix_capacity(capacity: usize) -> usize {
    match capacity {
        0 => 0,
        x if x < 16 => 16,
        x => 1 << (x.ilog2() + 1),
    }
}

pub use first::Map as CbHashMap;

#[cfg(test)]
#[macro_export]
macro_rules! generate_tests {
    ($map:ident, $should_resize:expr) => {
        #[test]
        fn empty_map_doesnt_allocate() {
            let map = $map::<usize, usize>::new();
            assert_eq!(0, std::mem::size_of_val(&*map.storage));
        }

        #[test]
        fn insert() {
            let mut map = $map::new();

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
            let mut map = $map::new();

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
            let mut map = $map::new();

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
            let mut map = $map::new();
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

            let buckets = if $should_resize { buckets * 2 } else { buckets };
            assert_eq!(buckets, map.n_buckets());
        }

        #[test]
        fn insert_nontrivial_drop() {
            let mut map = $map::new();
            let items = (0..1000).map(|i| (i.to_string(), i.to_string()));

            for (k, v) in items {
                map.insert(k, v);
            }
            assert_eq!(map.len(), 1000);
        }
    };
}
