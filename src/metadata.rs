//! Metadata for Swiss tables

const EMPTY: u8 = 0x80;
const TOMBSTONE: u8 = 0xFE;
const MASK: u8 = 0x7F;

pub type Metadata = u8;

#[inline]
pub fn from_h2(h2: u8) -> Metadata {
    h2 & MASK
}

#[inline]
pub fn empty() -> Metadata {
    EMPTY
}

#[inline]
pub fn tombstone() -> Metadata {
    TOMBSTONE
}

#[inline]
pub fn is_empty(m: Metadata) -> bool {
    m == EMPTY
}

#[inline]
pub fn is_value(m: Metadata) -> bool {
    (m & 0x80) == 0
}

#[inline]
pub fn h2(m: Metadata) -> u8 {
    m & MASK
}
