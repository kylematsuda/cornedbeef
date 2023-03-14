//! Defines the group for SSE probing.

use std::simd::{self, SimdPartialEq};

use crate::metadata::{self, Metadata};

pub const GROUP_SIZE: usize = 16;

/// Find the first set bit in the mask.
pub fn find_first(mask: &simd::Mask<i8, GROUP_SIZE>) -> Option<usize> {
    for i in 0..GROUP_SIZE {
        if mask.test(i) {
            return Some(i);
        }
    }
    None
}

/// Find the last set bit in the mask.
pub fn find_last(mask: &simd::Mask<i8, GROUP_SIZE>) -> Option<usize> {
    for i in (0..GROUP_SIZE).rev() {
        if mask.test(i) {
            return Some(i);
        }
    }
    None
}

pub struct Group<'a> {
    array: &'a [Metadata; GROUP_SIZE],
}

impl<'a> Group<'a> {
    pub fn new(slice: &'a [Metadata], index: usize) -> Self {
        let array =
            <&[Metadata; GROUP_SIZE]>::try_from(&slice[index..(index + GROUP_SIZE)]).unwrap();
        Self { array }
    }

    /// TODO: implement with SIMD instructions.
    #[inline]
    pub fn get_empty(&self) -> simd::Mask<i8, GROUP_SIZE> {
        let empty = simd::Simd::<u8, GROUP_SIZE>::splat(metadata::empty());
        let metadata = simd::Simd::<u8, GROUP_SIZE>::from_array(*self.array);
        empty.simd_eq(metadata)
    }

    /// TODO: implement with SIMD instructions.
    #[inline]
    pub fn get_candidates(&self, h2: u8) -> simd::Mask<i8, GROUP_SIZE> {
        let h2 = simd::Simd::<u8, GROUP_SIZE>::splat(h2);
        let metadata = simd::Simd::<u8, GROUP_SIZE>::from_array(*self.array);
        h2.simd_eq(metadata)
    }
}
