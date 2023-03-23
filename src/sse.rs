//! Defines the group for SSE probing.

use std::simd::{self, SimdPartialEq, ToBitMask};

use crate::metadata;

pub const GROUP_SIZE: usize = 16;
pub type SimdType = simd::u8x16;
pub type MaskType = simd::mask8x16;

/// Find the first set bit in the mask.
#[inline]
pub fn find_first(mask: MaskType) -> Option<usize> {
    let bits = mask.to_bitmask();
    match bits.trailing_zeros() as usize {
        GROUP_SIZE => None,
        i => Some(i),
    }
}

/// Find the last set bit in the mask.
#[inline]
pub fn find_last(mask: MaskType) -> Option<usize> {
    let bits = mask.to_bitmask();
    match bits.leading_zeros() as usize {
        GROUP_SIZE => None,
        i => Some(15 - i),
    }
}

#[inline]
pub fn get_empty(group: SimdType) -> MaskType {
    let empty = SimdType::splat(metadata::empty());
    empty.simd_eq(group)
}

#[inline]
pub fn get_candidates(group: SimdType, h2: u8) -> MaskType {
    let h2 = SimdType::splat(h2);
    h2.simd_eq(group)
}
