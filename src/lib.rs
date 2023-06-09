#![feature(
    new_uninit,
    allocator_api,
    iter_array_chunks,
    dropck_eyepatch,
    slice_ptr_get,
    portable_simd,
    core_intrinsics
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
        x => x.next_power_of_two(),
    }
}

#[inline(always)]
fn fast_rem(n: usize, modulus_power_of_two: usize) -> usize {
    n & modulus_power_of_two.saturating_sub(1)
}

pub use fifth::Map as CbHashMap;

pub fn get<'a>(map: &'a CbHashMap<usize, usize>, k: &'a usize) -> Option<&'a usize> {
    map.get(k)
}

pub fn insert<'a>(map: &'a mut CbHashMap<usize, usize>, k: usize, v: usize) -> Option<usize> {
    map.insert(k, v)
}

#[cfg(test)]
#[macro_export]
macro_rules! generate_tests {
    ($map:ident, $should_resize:expr) => {
        #[test]
        fn drop_empty_map() {
            let _ = $map::<String, String>::new();
        }

        #[test]
        fn get_from_empty() {
            let m = $map::<String, String>::new();
            assert_eq!(m.get(&String::from("hi")), None);
        }

        #[test]
        fn remove_from_empty() {
            let mut m = $map::<String, String>::new();
            assert_eq!(m.remove(&String::from("hi")), None);
        }

        #[test]
        fn clone() {
            let mut map = $map::new();

            for i in 0..1000 {
                map.insert(i, i);
            }

            assert_eq!(map.len(), 1000);

            let another_map = map.clone();
            assert_eq!(another_map.len(), 1000);

            for i in 0..1000 {
                assert_eq!(map.get(&i), another_map.get(&i));
            }
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
        fn insert_nontrivial_drop() {
            let mut map = $map::new();
            let items = (0..1000).map(|i| (i.to_string(), i.to_string()));

            for (k, v) in items {
                map.insert(k, v);
            }
            assert_eq!(map.len(), 1000);
        }

        #[test]
        fn insert_borrowed_data() {
            let mut map = $map::new();

            let items = (0..1000)
                .map(|i| (i.to_string(), i.to_string()))
                .collect::<Vec<_>>();

            for (k, v) in &items {
                map.insert(k, v);
            }
            assert_eq!(map.len(), 1000);
        }

        #[test]
        fn insert_owned_and_borrowed_data() {
            let mut map_1: $map<String, &str> = $map::new();
            let mut map_2: $map<&str, String> = $map::new();

            let items = (0..1000)
                .map(|i| (i.to_string(), i.to_string()))
                .collect::<Vec<_>>();

            for (k, v) in &items {
                map_1.insert(k.clone(), v);
                map_2.insert(k, v.clone());
            }
            assert_eq!(map_1.len(), 1000);
            assert_eq!(map_2.len(), 1000);
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
        fn clone_then_insert() {
            let mut map = $map::new();

            for i in 0..1000 {
                map.insert(i, i);
            }

            assert_eq!(map.len(), 1000);

            let mut new_map = map.clone();

            for i in 1000..2000 {
                new_map.insert(i, i);
            }
            assert_eq!(new_map.len(), 2000);
            for i in 0..2000 {
                assert_eq!(new_map.get(&i), Some(&i));
            }
        }

        #[test]
        #[should_panic]
        fn clone_bomb() {
            #[derive(PartialEq, Eq, Hash)]
            struct Bomb(usize);

            impl Clone for Bomb {
                fn clone(&self) -> Self {
                    panic!("bomb!!!")
                }
            }

            let mut map: $map<Bomb, String> = $map::new();

            for i in 0..1000 {
                map.insert(Bomb(i), i.to_string());
            }

            // This line panics, but shouldn't cause UB!
            let _ = map.clone();
        }
    };
}

#[cfg(test)]
#[macro_export]
macro_rules! generate_non_alloc_tests {
    ($map:ident) => {
        #[test]
        fn empty_map_doesnt_allocate() {
            let map = $map::<usize, usize>::new();
            assert_eq!(0, std::mem::size_of_val(&*map.storage));
        }
    };
}
