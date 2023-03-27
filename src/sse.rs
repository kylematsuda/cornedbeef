//! Defines the group for SSE probing.
use std::simd::{self, SimdPartialEq, ToBitMask};

use crate::metadata;

pub const GROUP_SIZE: usize = 16;
pub type SimdType = simd::u8x16;
pub type MaskType = simd::mask8x16;

#[derive(Clone, Copy)]
pub struct Group(SimdType);

impl Group {
    #[inline]
    pub fn from_slice(s: &[u8]) -> Self {
        Self(SimdType::from_slice(s))
    }

    #[inline]
    pub fn to_empties(self) -> MaskType {
        let empty = SimdType::splat(metadata::empty());
        empty.simd_eq(self.0)
    }

    #[inline]
    pub fn to_candidates(self, h2: u8) -> MaskType {
        let h2 = SimdType::splat(h2);
        h2.simd_eq(self.0)
    }
}

pub struct MaskIter<D> {
    inner: u16,
    _direction: D,
}

pub struct Forward;
pub struct Reverse;

impl MaskIter<Forward> {
    #[inline]
    pub fn forward(mask: MaskType) -> Self {
        Self {
            inner: mask.to_bitmask(),
            _direction: Forward,
        }
    }
}

impl MaskIter<Reverse> {
    #[inline]
    pub fn reverse(mask: MaskType) -> Self {
        Self {
            inner: mask.to_bitmask(),
            _direction: Reverse,
        }
    }
}

impl Iterator for MaskIter<Forward> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.trailing_zeros() as usize {
            GROUP_SIZE => None,
            i => {
                self.inner ^= 1 << i;
                Some(i)
            }
        }
    }
}

impl Iterator for MaskIter<Reverse> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.leading_zeros() as usize {
            GROUP_SIZE => None,
            i => {
                let i = 15 - i;
                self.inner ^= 1 << i;
                Some(i)
            }
        }
    }
}

/// Find the first set bit in the mask.
#[inline]
pub fn find_first(mask: MaskType) -> Option<usize> {
    MaskIter::forward(mask).next()
}

/// Find the last set bit in the mask.
#[inline]
pub fn find_last(mask: MaskType) -> Option<usize> {
    MaskIter::reverse(mask).next()
}
