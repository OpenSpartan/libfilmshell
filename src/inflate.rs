//! zlib decompression (RFC 1950 and 1951), for chunks as downloaded.
//! Canonical Huffman codes decoded a bit at a time.

const MAX_BITS: usize = 15;
const LENGTH_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

struct Bits<'a> {
    b: &'a [u8],
    p: usize,
    hold: u32,
    n: u32,
}

impl Bits<'_> {
    fn take(&mut self, want: u32) -> Option<u32> {
        while self.n < want {
            self.hold |= (*self.b.get(self.p)? as u32) << self.n;
            self.p += 1;
            self.n += 8;
        }
        let v = self.hold & ((1u32 << want) - 1);
        self.hold >>= want;
        self.n -= want;
        Some(v)
    }

    /// Drop the bits left in the byte being read.
    fn align(&mut self) {
        self.hold = 0;
        self.n = 0;
    }
}

/// A canonical code: how many codes of each length, the symbols in code order.
struct Code {
    count: [u16; MAX_BITS + 1],
    symbol: Vec<u16>,
}

impl Code {
    fn new(lengths: &[u8]) -> Code {
        let mut count = [0u16; MAX_BITS + 1];
        for &l in lengths {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut offs = [0usize; MAX_BITS + 2];
        for l in 1..=MAX_BITS {
            offs[l + 1] = offs[l] + count[l] as usize;
        }
        let mut symbol = vec![0u16; offs[MAX_BITS + 1]];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize]] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Code { count, symbol }
    }

    fn decode(&self, r: &mut Bits<'_>) -> Option<u16> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for l in 1..=MAX_BITS {
            code |= r.take(1)? as i32;
            let count = self.count[l] as i32;
            if code - count < first {
                return self.symbol.get((index + code - first) as usize).copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

fn fixed() -> (Code, Code) {
    let mut lit = [8u8; 288];
    lit[144..256].fill(9);
    lit[256..280].fill(7);
    (Code::new(&lit), Code::new(&[5u8; 30]))
}

fn dynamic(r: &mut Bits<'_>) -> Option<(Code, Code)> {
    let nlen = r.take(5)? as usize + 257;
    let ndist = r.take(5)? as usize + 1;
    let ncode = r.take(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return None;
    }
    let mut lengths = [0u8; 320];
    for &o in &ORDER[..ncode] {
        lengths[o] = r.take(3)? as u8;
    }
    let code = Code::new(&lengths[..19]);
    let mut lengths = [0u8; 320];
    let mut i = 0;
    while i < nlen + ndist {
        let sym = code.decode(r)?;
        let (value, run) = match sym {
            0..=15 => {
                lengths[i] = sym as u8;
                i += 1;
                continue;
            }
            16 if i > 0 => (lengths[i - 1], 3 + r.take(2)? as usize),
            17 => (0, 3 + r.take(3)? as usize),
            18 => (0, 11 + r.take(7)? as usize),
            _ => return None,
        };
        if i + run > nlen + ndist {
            return None;
        }
        lengths[i..i + run].fill(value);
        i += run;
    }
    Some((Code::new(&lengths[..nlen]), Code::new(&lengths[nlen..nlen + ndist])))
}

fn codes(r: &mut Bits<'_>, out: &mut Vec<u8>, lit: &Code, dist: &Code, limit: usize, over: &mut bool) -> Option<()> {
    loop {
        let sym = lit.decode(r)? as usize;
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return Some(()),
            _ => {
                let i = sym - 257;
                let len = *LENGTH_BASE.get(i)? as usize + r.take(*LENGTH_EXTRA.get(i)? as u32)? as usize;
                let d = dist.decode(r)? as usize;
                let back = *DIST_BASE.get(d)? as usize + r.take(*DIST_EXTRA.get(d)? as u32)? as usize;
                if back > out.len() {
                    return None;
                }
                for _ in 0..len {
                    out.push(out[out.len() - back]);
                }
            }
        }
        if out.len() > limit {
            *over = true;
            return None;
        }
    }
}

fn adler32(b: &[u8]) -> u32 {
    let (mut s1, mut s2) = (1u32, 0u32);
    for chunk in b.chunks(5552) {
        for &x in chunk {
            s1 += x as u32;
            s2 += s1;
        }
        s1 %= 65521;
        s2 %= 65521;
    }
    (s2 << 16) | s1
}

/// Why a zlib stream didn't inflate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Malformed, truncated, or failing its checksum.
    Malformed,
    /// Longer than the limit.
    TooLong,
}

/// A zlib stream's contents, at most `limit` bytes.
pub fn zlib(data: &[u8], limit: usize) -> Result<Vec<u8>, Fault> {
    let mut over = false;
    inflate(data, limit, &mut over).ok_or(if over { Fault::TooLong } else { Fault::Malformed })
}

fn inflate(data: &[u8], limit: usize, over: &mut bool) -> Option<Vec<u8>> {
    if data.len() < 6 || data[0] & 0x0f != 8 || (data[0] as u32 * 256 + data[1] as u32) % 31 != 0 || data[1] & 0x20 != 0
    {
        return None;
    }
    let mut r = Bits { b: data, p: 2, hold: 0, n: 0 };
    let mut out = Vec::new();
    loop {
        let last = r.take(1)?;
        match r.take(2)? {
            0 => {
                // take() buffers whole bytes only as needed, so after align()
                // the next unread byte is at r.p.
                r.align();
                let head = data.get(r.p..r.p + 4)?;
                let len = u16::from_le_bytes([head[0], head[1]]) as usize;
                if len as u16 != !u16::from_le_bytes([head[2], head[3]]) {
                    return None;
                }
                if out.len() + len > limit {
                    *over = true;
                    return None;
                }
                out.extend_from_slice(data.get(r.p + 4..r.p + 4 + len)?);
                r.p += 4 + len;
            }
            1 => {
                let (lit, dist) = fixed();
                codes(&mut r, &mut out, &lit, &dist, limit, over)?;
            }
            2 => {
                let (lit, dist) = dynamic(&mut r)?;
                codes(&mut r, &mut out, &lit, &dist, limit, over)?;
            }
            _ => return None,
        }
        if last == 1 {
            break;
        }
    }
    r.align();
    let sum = data.get(r.p..r.p + 4)?;
    (u32::from_be_bytes(sum.try_into().unwrap()) == adler32(&out)).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::{zlib, Fault};

    #[test]
    fn stored_and_compressed() {
        // zlib.compress(b"hello hello hello hello", 9)
        let z = [0x78, 0xda, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0x01, 0x68, 0x03, 0x08, 0xb1];
        assert_eq!(zlib(&z, 100).as_deref(), Ok(&b"hello hello hello hello"[..]));
        assert_eq!(zlib(&z, 5), Err(Fault::TooLong));
        let mut bad = z;
        bad[15] ^= 1;
        assert_eq!(zlib(&bad, 100), Err(Fault::Malformed));
        assert_eq!(zlib(&z[..10], 100), Err(Fault::Malformed));
        // zlib.compress(b"abc", 0)
        let s = [0x78, 0x01, 0x01, 0x03, 0x00, 0xfc, 0xff, 0x61, 0x62, 0x63, 0x02, 0x4d, 0x01, 0x27];
        assert_eq!(zlib(&s, 100).as_deref(), Ok(&b"abc"[..]));
        assert_eq!(zlib(&s, 2), Err(Fault::TooLong));
    }
}
