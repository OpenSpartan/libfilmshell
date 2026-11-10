//! MSB-first bit reads over byte buffers. Reads past the end of a buffer give
//! zero bits; the scanners rely on that at chunk ends.

/// Byte `i` of `c`, or 0 outside it.
#[inline]
pub fn byte(c: &[u8], i: isize) -> u32 {
    if i >= 0 && (i as usize) < c.len() {
        c[i as usize] as u32
    } else {
        0
    }
}

/// `width` (at most 32) bits at absolute bit `bp` of `c`.
#[inline]
pub fn bits_at(c: &[u8], bp: i64, width: u32) -> u32 {
    let mut v = 0u32;
    for b in bp..bp + width as i64 {
        v = (v << 1) | ((byte(c, (b >> 3) as isize) >> (7 - (b & 7))) & 1);
    }
    v
}

/// The byte stream seen from byte `q` at bit shift `sh`: `get(k)` is the
/// byte that starts `sh` bits into byte `q + k`. A record found at bit
/// `8q + sh` reads its header as `get(0)`, `get(1)` and on.
#[derive(Clone, Copy)]
pub struct Shifted<'a> {
    pub c: &'a [u8],
    pub q: usize,
    pub sh: u32,
}

impl Shifted<'_> {
    #[inline]
    pub fn get(&self, k: i32) -> u32 {
        let i = self.q as isize + k as isize;
        if self.sh == 0 {
            byte(self.c, i)
        } else {
            ((byte(self.c, i) << self.sh) | (byte(self.c, i + 1) >> (8 - self.sh))) & 0xff
        }
    }

    /// `width` bits starting `start` bits past the MSB of `get(5)` (the first
    /// payload byte of a composite record); `start` may be negative to reach
    /// back into the header and item list.
    #[inline]
    pub fn read(&self, start: i32, width: u32) -> u32 {
        let mut v = 0u32;
        for b in start..start + width as i32 {
            v = (v << 1) | ((self.get(5 + (b >> 3)) >> (7 - (b & 7))) & 1);
        }
        v
    }
}
