//! Metadata for Swiss tables

const EMPTY: u8 = 0x80;
const TOMBSTONE: u8 = 0xFE;
const MASK: u8 = 0x7F;

pub type Metadata = u8;

#[inline]
pub const fn from_h2(h2: u8) -> Metadata {
    h2 & MASK
}

#[inline]
pub const fn empty() -> Metadata {
    EMPTY
}

#[inline]
pub const fn tombstone() -> Metadata {
    TOMBSTONE
}

#[inline]
pub const fn is_empty(m: Metadata) -> bool {
    m == EMPTY
}

#[inline]
pub const fn is_full(m: Metadata) -> bool {
    (m & 0x80) == 0
}

#[inline]
pub const fn h2(m: Metadata) -> u8 {
    m & MASK
}
