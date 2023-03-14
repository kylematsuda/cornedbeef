//! Defines the group for SSE probing.

use std::simd;

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
    array: &'a [Metadata],
}

impl<'a> Group<'a> {
    pub fn new(array: &'a [Metadata]) -> Self {
        Self { array }
    }

    /// TODO: implement with SIMD instructions.
    pub fn get_empty(&self) -> simd::Mask<i8, GROUP_SIZE> {
        let mut empties = [false; GROUP_SIZE];
        for i in 0..GROUP_SIZE {
            // Eventually, change this to get_unchecked to elide bounds check.
            let entry = self.array[i];
            if metadata::is_empty(entry) {
                empties[i] = true;
            }
        }
        simd::Mask::from_array(empties)
    }

    /// TODO: implement with SIMD instructions.
    pub fn get_candidates(&self, h2: u8) -> simd::Mask<i8, GROUP_SIZE> {
        let mut candidates = [false; GROUP_SIZE];
        for i in 0..GROUP_SIZE {
            let entry = self.array[i];
            if metadata::is_value(entry) && metadata::h2(entry) == h2 {
                candidates[i] = true;
            }
        }
        simd::Mask::from_array(candidates)
    }
}
