#![feature(new_uninit)]

use core::hash::{BuildHasher, Hasher};
use std::collections::hash_map::DefaultHasher;

pub mod first;
pub mod fourth;
pub mod second;
pub mod third;

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
fn fix_capacity(capacity: usize) -> usize
{
    match capacity {
        0 => 0,
        x if x < 16 => 16,
        x => 1 << (x.ilog2() + 1),
    }
}

pub use fourth::Map as CbHashMap;
