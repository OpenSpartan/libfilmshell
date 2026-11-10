//! HighlightEvents: the last film chunk (ChunkType 3), the theater timeline of
//! kills, deaths, medals and mode events per XUID with millisecond times.
//! The record layout follows Andy Curtis's SPNKr `film/highlight_events.py`
//! (<https://github.com/acurtis166/SPNKr>). A medal is carried as its
//! medal index.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventType {
    Kill,
    Death,
    Medal,
    Mode,
}

impl EventType {}

#[derive(Clone, Debug)]
pub struct HighlightEvent {
    pub xuid: String,
    pub gamertag: String,
    pub event_type: EventType,
    /// ms since match start (big-endian u32 in the payload).
    pub time_ms: f64,
    pub medal_value: u32,
    /// The player's team, payload byte 38.
    pub team: u32,
}

const MIN_XUID: u64 = 2_000_000_000_000_000;
const MAX_XUID: u64 = 3_000_000_000_000_000;
/// type_hint values on medal events: the medal SortingWeight set.
const MEDAL_WEIGHTS: [u32; 16] = [50, 51, 52, 100, 101, 150, 200, 205, 210, 220, 225, 230, 235, 240, 245, 250];
/// From a record's XUID to its `00002ee0` end marker: 15406 bits (chunk 0's
/// roster records have a 15395-bit stride).
const GAP_BITS: i64 = 15406;
const PAYLOAD_BITS: i64 = 60 * 8;

#[inline]
fn u8_at(b: &[u8], bp: i64) -> u32 {
    let i = (bp >> 3) as isize;
    let sh = (bp & 7) as u32;
    let hi = crate::bits::byte(b, i);
    let lo = crate::bits::byte(b, i + 1);
    ((hi << sh) | if sh == 0 { 0 } else { lo >> (8 - sh) }) & 0xff
}

fn u32be(b: &[u8], bp: i64) -> u32 {
    (u8_at(b, bp) << 24) | (u8_at(b, bp + 8) << 16) | (u8_at(b, bp + 16) << 8) | u8_at(b, bp + 24)
}

fn u64le(b: &[u8], bp: i64) -> u64 {
    (0..8).fold(0u64, |v, i| v | (u8_at(b, bp + i * 8) as u64) << (i * 8))
}

fn utf16le(b: &[u8], bp: i64, n: i64) -> String {
    let mut units = Vec::new();
    let mut i = 0;
    while i < n {
        let cu = u8_at(b, bp + i * 8) | (u8_at(b, bp + (i + 1) * 8) << 8);
        if cu == 0 {
            break;
        }
        units.push(cu as u16);
        i += 2;
    }
    String::from_utf16_lossy(&units)
}

/// The gamertag at the payload's start, or 12 bytes in (film versions 39
/// and 40 pad it).
fn gamertag(chunk: &[u8], p: i64) -> String {
    let at0 = utf16le(chunk, p, 32);
    if at0.chars().next().is_some_and(|c| !c.is_control()) {
        return at0;
    }
    utf16le(chunk, p + 12 * 8, 32)
}

fn infer_type(hint: u32, is_medal: bool) -> EventType {
    if is_medal && MEDAL_WEIGHTS.contains(&hint) {
        EventType::Medal
    } else if hint == 10 {
        EventType::Mode
    } else if hint == 20 {
        EventType::Death
    } else {
        // 50 = kill; unknown hints count as kills so totals are preserved.
        EventType::Kill
    }
}

/// Parse the inflated HighlightEvents chunk. Records sit at arbitrary bit
/// shifts, so every bit position is tried for `c0` preceded by `2d|25`
/// preceded by a valid little-endian XUID.
pub fn extract_highlight_events(chunk: &[u8]) -> Vec<HighlightEvent> {
    let mut out = Vec::new();
    let total_bits = chunk.len() as i64 * 8;
    let mut bp: i64 = 72;
    while bp + GAP_BITS + 32 < total_bits {
        if u8_at(chunk, bp) != 0xc0 {
            bp += 1;
            continue;
        }
        let disc = u8_at(chunk, bp - 8);
        if disc != 0x2d && disc != 0x25 {
            bp += 1;
            continue;
        }
        let xuid_start = bp - 72;
        let xuid = u64le(chunk, xuid_start);
        if xuid <= MIN_XUID || xuid >= MAX_XUID {
            bp += 1;
            continue;
        }
        let end_bp = xuid_start + GAP_BITS;
        if u32be(chunk, end_bp) != 0x0000_2ee0 {
            bp += 1;
            continue;
        }
        let p = end_bp - PAYLOAD_BITS;
        let type_hint = u8_at(chunk, p + 47 * 8);
        let is_medal = u8_at(chunk, p + 55 * 8) == 1;
        let medal_value = u8_at(chunk, p + 59 * 8);
        let team = u8_at(chunk, p + 38 * 8);
        out.push(HighlightEvent {
            xuid: xuid.to_string(),
            gamertag: gamertag(chunk, p),
            event_type: infer_type(type_hint, is_medal),
            time_ms: u32be(chunk, p + 48 * 8) as f64,
            medal_value,
            team,
        });
        // Events don't overlap: continue past this one's end marker.
        bp = end_bp + 32;
    }
    out
}
