//! Player identity from chunk 0's bit-packed roster table: the XUID and
//! gamertag of every player present at the start.
//!
//! ```text
//! [29 0d ee 6e 89 2c 80 00 00 10 00 00 00 00]  anchor, byte-aligned
//! per record, at any bit shift:
//! [10-byte prefix] [48-bit session hash] [XUID big-endian 64]
//!   ... [gamertag UTF-16LE] [21 zero bytes] [XUID little-endian 64] ...
//!   15395 bits from the little-endian XUID to the next record's big-endian one
//! ```

use crate::bits::byte;
use crate::types::RosterIdentity;

const ROSTER_MARKER: [u8; 14] = [0x29, 0x0d, 0xee, 0x6e, 0x89, 0x2c, 0x80, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00];
/// Every XUID's high 16 bits: the loop sentinel.
const XUID_HI: u64 = 0x0009;
/// Bits from a record's LE XUID to the next record's BE XUID.
const LE_TO_NEXT_BE: i64 = 15395;
/// Max gap between a BE XUID and its LE copy.
const LE_SCAN_BITS: i64 = 30000;

fn bit_read_be(buf: &[u8], bp: i64, n: u32) -> u64 {
    let mut v = 0u64;
    for i in 0..n as i64 {
        let b = bp + i;
        v = (v << 1) | ((byte(buf, (b >> 3) as isize) >> (7 - (b & 7))) & 1) as u64;
    }
    v
}

fn index_of_bytes(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// The LE copy of `xuid` within (bp, bp + LE_SCAN_BITS) at any bit alignment:
/// the earliest start bit after `bp`, or None.
fn bit_find_le(buf: &[u8], xuid: u64, bp: i64) -> Option<i64> {
    let le = xuid.to_le_bytes();
    let lo = (bp >> 3) as usize;
    let hi = buf.len().min((((bp + LE_SCAN_BITS) >> 3) + 1) as usize);
    if lo >= hi {
        return None;
    }
    let win = &buf[lo..hi];
    let mut best: Option<i64> = None;
    for s in 0..8u32 {
        // At shift s the 7 fully-determined bytes of the 8-byte value.
        let pat: Vec<u8> = if s == 0 {
            le.to_vec()
        } else {
            (0..7).map(|k| (((le[k] as u32) << s | (le[k + 1] as u32) >> (8 - s)) & 0xff) as u8).collect()
        };
        let mut i = 0usize;
        while let Some(at) = index_of_bytes(win, &pat, i) {
            let cand = if s == 0 { (lo + at) as i64 * 8 } else { (lo + at) as i64 * 8 - 8 + (8 - s as i64) };
            if cand > bp && best.is_none_or(|b| cand < b) {
                best = Some(cand);
            }
            i = at + 1;
        }
    }
    best
}

/// Gamertag: UTF-16LE units walking back from the 21 zero bytes before the
/// LE XUID, until a non-printable.
fn read_gamertag(buf: &[u8], le_pos: i64) -> String {
    let mut p = le_pos - 21 * 8;
    let mut units: Vec<u16> = Vec::new();
    while p >= 16 {
        let u = bit_read_be(buf, p - 16, 8) as u32 | ((bit_read_be(buf, p - 8, 8) as u32) << 8);
        if !(0x20..=0xfffd).contains(&u) {
            break;
        }
        units.push(u as u16);
        p -= 16;
    }
    units.reverse();
    String::from_utf16_lossy(&units)
}

/// XUID and gamertag per roster position. Players who joined later are
/// absent: chunk 0 is written when the match starts.
pub fn extract_roster(chunk0: &[u8]) -> Vec<RosterIdentity> {
    let Some(m) = index_of_bytes(chunk0, &ROSTER_MARKER, 0) else { return Vec::new() };
    // Anchor plus 4 bytes: record 0's prefix; then 10 bytes of prefix and 6
    // of session hash before the big-endian XUID.
    let mut bp = (m as i64 + 20) * 8;
    let mut out = Vec::new();
    while bp + 64 <= chunk0.len() as i64 * 8 {
        let v = bit_read_be(chunk0, bp, 64);
        if v >> 48 != XUID_HI {
            break;
        }
        let Some(le_pos) = bit_find_le(chunk0, v, bp + 64) else { break };
        out.push(RosterIdentity { xuid: v.to_string(), gamertag: read_gamertag(chunk0, le_pos) });
        bp = le_pos + LE_TO_NEXT_BE;
    }
    out
}
