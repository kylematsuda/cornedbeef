//! Defines the group for SSE probing.

use std::simd::{self, SimdPartialEq, ToBitMask};

use crate::metadata::{self, Metadata};

pub const GROUP_SIZE: usize = 16;
pub type SimdType = simd::u8x16;
pub type MaskType = simd::mask8x16;

/// Find the first set bit in the mask.
pub fn find_first(mask: &MaskType) -> Option<usize> {
    let bits = mask.to_bitmask();
    match bits.trailing_zeros() as usize {
        GROUP_SIZE => None,
        i => Some(i),
    }
}

/// Find the last set bit in the mask.
pub fn find_last(mask: &MaskType) -> Option<usize> {
    let bits = mask.to_bitmask();
    match bits.leading_zeros() as usize {
        GROUP_SIZE => None,
        i => Some(15 - i),
    }
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
    pub fn get_empty(&self) -> simd::Mask<i8, GROUP_SIZE> {
        let empty = simd::Simd::<u8, GROUP_SIZE>::splat(metadata::empty());
        let metadata = simd::Simd::<u8, GROUP_SIZE>::from_array(*self.array);
        empty.simd_eq(metadata)
    }

    /// TODO: implement with SIMD instructions.
    pub fn get_candidates(&self, h2: u8) -> simd::Mask<i8, GROUP_SIZE> {
        let h2 = simd::Simd::<u8, GROUP_SIZE>::splat(h2);
        let metadata = simd::Simd::<u8, GROUP_SIZE>::from_array(*self.array);
        h2.simd_eq(metadata)
    }
}
