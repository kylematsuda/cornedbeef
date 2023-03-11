#![feature(new_uninit)]

use core::hash::{BuildHasher, Hasher};
use std::collections::hash_map::DefaultHasher;

pub mod first;
pub mod fourth;
pub mod second;
pub mod third;

// Use std's default hasher.
pub type DefaultHashBuilder = core::hash::BuildHasherDefault<DefaultHasher>;

pub(crate) fn make_hash<S, K>(build_hasher: &S, key: &K) -> u64
where
    S: BuildHasher,
    K: core::hash::Hash,
{
    let mut hasher = build_hasher.build_hasher();
    key.hash(&mut hasher);
    hasher.finish()
}

pub use fourth::Map as CbHashMap;
