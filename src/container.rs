//! The film container: decompressed chunks, the frames inside them and the
//! game-tick clock.
//!
//! Chunk 0 is the session header (roster table, `identity.rs`). Chunks 1..n
//! are a packet stream, walked exactly from byte 0 to a packet boundary at
//! the chunk's end:
//!
//! ```text
//! <u16LE type> <u8> <u8> <u32LE size> <u64LE timestamp_us> <size bytes>
//!   type 0   one simulation tick's update packet (60 per second; 30 on BTB)
//!   type 10  per-tick companion, 1:1 with type 0
//!   type 1/2 replication start / world-state snapshots at chunk start
//!   type 6, 7, 8, 12   chunk bookkeeping, player and bot metadata
//!   type 9   HighlightEvents (last chunk)
//! ```
//!
//! A type-0 payload is one bitstream: a flag bit, the event view, the entity
//! view (`payload.rs`). One entity-view entry per packet is the tick
//! message, entity 123's update: `100000 0111101101000010 00000` then an
//! 8-bit tick counter, at any bit offset after the events; `A0 7B 42` is its
//! byte-aligned case. Object updates follow it in the same view.
//!
//! Two framings are available:
//!
//! * [`Framing::Packets`]: one frame per type-0 packet, scanned over its own
//!   payload; the clock comes from the tick counters, unwrapped per packet.
//! * [`Framing::Markers`]: one frame per `A0 7B 42`, running to the next
//!   marker, which folds the marker-less tick packets in between into the
//!   previous frame. A film falls back to it when a chunk's packets don't
//!   walk to its end.

use crate::bits::byte;
use crate::types::EncodingMode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Framing {
    Markers,
    Packets,
}

/// One packet of chunks 1..n.
#[derive(Clone, Copy, Debug)]
pub struct Packet {
    pub kind: u16,
    pub ts_us: u64,
    /// Payload `chunk[start..end]`.
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub struct Film {
    pub chunks: Vec<Vec<u8>>,
    pub framing: Framing,
    /// Marker offsets per chunk (empty for chunk 0, which is never scanned).
    markers: Vec<Vec<usize>>,
    /// Packets per chunk; None where the walk does not end exactly on the
    /// chunk's last byte (chunk 0 always).
    packets: Vec<Option<Vec<Packet>>>,
    frames: Vec<FrameSpan>,
    ticks: Vec<u32>,
    /// The object updates the record scan finds, kept from its first run
    /// (`replication::for_each_object_update`).
    pub(crate) object_updates: std::sync::OnceLock<Vec<crate::replication::Found>>,
}

#[derive(Clone, Copy, Debug)]
struct FrameSpan {
    ci: usize,
    /// Start of the frame (marker, or packet payload).
    start: usize,
    /// First byte the record scanners look at.
    body: usize,
    end: usize,
    gp: i64,
    ts_us: Option<u64>,
}

/// One frame: records in `chunk[body..end]` share its index and its global
/// byte position (offset over chunks 1.., the sort key of every sample).
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    pub chunk: &'a [u8],
    pub start: usize,
    pub body: usize,
    pub end: usize,
    pub index: usize,
    pub gp: i64,
    pub ts_us: Option<u64>,
}

impl Film {
    pub fn new(chunks: Vec<Vec<u8>>, framing: Framing) -> Film {
        let markers: Vec<Vec<usize>> =
            chunks.iter().enumerate().map(|(ci, c)| if ci == 0 { Vec::new() } else { find_markers(c) }).collect();
        let packets: Vec<Option<Vec<Packet>>> =
            chunks.iter().enumerate().map(|(ci, c)| if ci == 0 { None } else { walk_packets(c) }).collect();
        let mut film = Film {
            chunks,
            framing,
            markers,
            packets,
            frames: Vec::new(),
            ticks: Vec::new(),
            object_updates: std::sync::OnceLock::new(),
        };
        // A chunk whose walk fails falls back to markers for the whole film,
        // so a film never mixes two frame definitions.
        if framing == Framing::Packets && film.packets.iter().skip(1).any(|p| p.is_none()) {
            film.framing = Framing::Markers;
        }
        film.frames = match film.framing {
            Framing::Markers => film.marker_frames(),
            Framing::Packets => film.packet_frames(),
        };
        film.ticks = match film.framing {
            Framing::Markers => marker_ticks(&film),
            Framing::Packets => packet_ticks(&film),
        };
        film
    }

    pub fn markers(&self, ci: usize) -> &[usize] {
        &self.markers[ci]
    }

    pub fn packets(&self, ci: usize) -> Option<&[Packet]> {
        self.packets[ci].as_deref()
    }

    /// Every frame of chunks 1 to n-1 in order.
    pub fn frames(&self) -> impl Iterator<Item = Frame<'_>> + '_ {
        self.frames.iter().enumerate().map(|(index, f)| Frame {
            chunk: self.chunks[f.ci].as_slice(),
            start: f.start,
            body: f.body,
            end: f.end,
            index,
            gp: f.gp,
            ts_us: f.ts_us,
        })
    }

    /// Frame `index`.
    pub fn frame(&self, index: usize) -> Frame<'_> {
        let f = &self.frames[index];
        Frame {
            chunk: self.chunks[f.ci].as_slice(),
            start: f.start,
            body: f.body,
            end: f.end,
            index,
            gp: f.gp,
            ts_us: f.ts_us,
        }
    }

    /// 60 Hz game tick of every frame index.
    pub fn frame_ticks(&self) -> &[u32] {
        &self.ticks
    }

    /// Each frame's packet timestamp, microseconds; None unless every frame
    /// has one (packet framing).
    pub fn frame_times_us(&self) -> Option<Vec<u64>> {
        self.frames.iter().map(|f| f.ts_us).collect()
    }

    /// Each frame's tick on the HighlightEvents clock
    /// (`lives::highlight_ticks`).
    pub fn highlight_ticks(&self) -> Vec<u32> {
        crate::lives::highlight_ticks(&self.ticks, self.frame_times_us().as_deref())
    }

    fn marker_frames(&self) -> Vec<FrameSpan> {
        let mut out = Vec::new();
        let mut global = 0i64;
        for ci in 1..self.chunks.len() {
            let c = &self.chunks[ci];
            let m = &self.markers[ci];
            for (mi, &p) in m.iter().enumerate() {
                let end = if mi + 1 < m.len() { m[mi + 1] } else { c.len() };
                out.push(FrameSpan { ci, start: p, body: p + 5, end, gp: global + p as i64, ts_us: None });
            }
            global += c.len() as i64;
        }
        out
    }

    fn packet_frames(&self) -> Vec<FrameSpan> {
        let mut out = Vec::new();
        let mut global = 0i64;
        for ci in 1..self.chunks.len() {
            let c = &self.chunks[ci];
            for p in self.packets[ci].as_deref().unwrap_or(&[]) {
                if p.kind != 0 {
                    continue;
                }
                // The A0 xx 42 message header is 5 bytes; records follow it.
                let pl = &c[p.start..p.end];
                let body = if pl.len() >= 5 && pl[0] == 0xa0 && pl[2] == 0x42 { p.start + 5 } else { p.start };
                out.push(FrameSpan {
                    ci,
                    start: p.start,
                    body,
                    end: p.end,
                    gp: global + p.start as i64,
                    ts_us: Some(p.ts_us),
                });
            }
            global += c.len() as i64;
        }
        out
    }
}

/// The most packets a chunk may hold. A 20 s chunk of a 60 Hz film holds
/// about 2,400; a run of empty packet headers would otherwise turn a chunk
/// of zeros into millions of frames.
const MAX_PACKETS: usize = 1 << 16;

/// Walk `buf` as packets; None unless the walk ends exactly at its end, or
/// with more than `MAX_PACKETS`.
pub fn walk_packets(buf: &[u8]) -> Option<Vec<Packet>> {
    let mut out = Vec::new();
    let mut p = 0usize;
    while p + 16 <= buf.len() {
        if out.len() == MAX_PACKETS {
            return None;
        }
        let kind = u16::from_le_bytes([buf[p], buf[p + 1]]);
        let size = u32::from_le_bytes(buf[p + 4..p + 8].try_into().ok()?) as usize;
        let ts_us = u64::from_le_bytes(buf[p + 8..p + 16].try_into().ok()?);
        let start = p + 16;
        let end = start.checked_add(size)?;
        if end > buf.len() {
            return None;
        }
        out.push(Packet { kind, ts_us, start, end });
        p = end;
    }
    (p == buf.len()).then_some(out)
}

/// All `A0 7B 42` offsets. A marker in the last three bytes of a chunk is
/// not reported.
pub fn find_markers(buf: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    if buf.len() < 4 {
        return out;
    }
    for i in 0..buf.len() - 3 {
        if buf[i] == 0xa0 && buf[i + 1] == 0x7b && buf[i + 2] == 0x42 {
            out.push(i);
        }
    }
    out
}

/// The 8-bit tick counter of an `A0 7B 42` message at `p`.
#[inline]
fn counter_at(c: &[u8], p: usize) -> (u32, bool) {
    let b3 = byte(c, p as isize + 3);
    let b4 = byte(c, p as isize + 4);
    ((((b3 << 8) | b4) >> 3) & 0xff, b3 < 8)
}

/// Marker framing's clock: bytes 3-4 of each marker carry an 8-bit tick
/// counter, +1 per game tick, unwrapped. A chance `A0 7B 42` inside a payload
/// (b3 at least 8, or a jump of 128 ticks or more that the next marker
/// undoes) keeps the previous tick.
fn marker_ticks(film: &Film) -> Vec<u32> {
    let mut t8: Vec<i32> = Vec::new();
    let mut ok: Vec<bool> = Vec::new();
    for ci in 1..film.chunks.len() {
        let c = &film.chunks[ci];
        for &p in film.markers(ci) {
            let (t, real) = counter_at(c, p);
            t8.push(t as i32);
            ok.push(real);
        }
    }
    let mut out = vec![0u32; t8.len()];
    let mut prev = -1i32;
    let mut total = 0u32;
    for i in 0..t8.len() {
        if ok[i] && prev >= 0 {
            let step = (t8[i] - prev + 256) & 0xff;
            if step >= 128 {
                let mut j = i + 1;
                while j < t8.len() && !ok[j] {
                    j += 1;
                }
                if j < t8.len() && ((t8[j] - prev + 256) & 0xff) <= 16 {
                    ok[i] = false;
                }
            }
            if ok[i] {
                total += step as u32;
            }
        }
        if ok[i] {
            prev = t8[i];
        }
        out[i] = total;
    }
    out
}

/// The 27 fixed bits of the tick message, `100000 0111101101000010 00000`;
/// its 8-bit tick counter follows them.
const TICK_MESSAGE: u64 = 0b100000_0111101101000010_00000;

/// The tick counter of the first tick message in `payload`, at any bit
/// offset. A non-empty type-0 payload carries exactly one; `A0 7B 42` is the
/// case where it starts 2 bits into a byte.
pub fn tick_message_counter(payload: &[u8]) -> Option<u32> {
    let n = payload.len();
    for i in 0..n {
        let mut w = 0u64;
        for k in 0..8 {
            w = (w << 8) | payload.get(i + k).copied().unwrap_or(0) as u64;
        }
        for sh in 0..8u32 {
            // 35 bits from bit 8i + sh: the message and its counter.
            if (i * 8 + sh as usize + 35) > n * 8 {
                return None;
            }
            let v = (w << sh) >> 29;
            if v >> 8 == TICK_MESSAGE {
                return Some((v & 0xff) as u32);
            }
        }
    }
    None
}

/// Packet framing's clock. Each type-0 packet is one simulation tick
/// (two on 30 Hz films), and its tick message says which: the counter steps
/// by the ticks per packet, and by 3 or 4 across a recorder hitch. Between
/// two counter-bearing packets
/// the counter delta is spread evenly; a delta smaller than the packet count
/// or more than 64 ticks past it is not trusted, and the packet steps by the
/// film's ticks per packet.
fn packet_ticks(film: &Film) -> Vec<u32> {
    let frames = &film.frames;
    let counters: Vec<Option<u32>> =
        frames.iter().map(|f| tick_message_counter(&film.chunks[f.ci][f.start..f.end])).collect();
    // Ticks per packet: the most common counter step between adjacent
    // counter-bearing packets.
    let mut votes = [0usize; 5];
    for w in counters.windows(2) {
        if let (Some(a), Some(b)) = (w[0], w[1]) {
            let d = ((b + 256 - a) & 0xff) as usize;
            if (1..=4).contains(&d) {
                votes[d] += 1;
            }
        }
    }
    let tpp = (1..=4).max_by_key(|&d| (votes[d], std::cmp::Reverse(d))).unwrap_or(1) as u32;

    let n = frames.len();
    let mut out = vec![0u32; n];
    let mut last: Option<(usize, u32)> = None; // (frame index, counter)
    for i in 0..n {
        let expected = if i == 0 { 0 } else { out[i - 1] + tpp };
        out[i] = expected;
        let Some(c) = counters[i] else { continue };
        if let Some((j, cj)) = last {
            let k = (i - j) as u32;
            let mut d = (c + 256 - cj) & 0xff;
            // Unwrap: the counter is 8 bits, the true delta is at least k.
            while d < k {
                d += 256;
            }
            if d <= k * tpp + 64 {
                let base = out[j];
                for m in 1..=k {
                    out[j + m as usize] = base + f64::round(m as f64 * d as f64 / k as f64) as u32;
                }
            }
        }
        last = Some((i, c));
    }
    out
}

/// Minimum frame count for the b9=0x35 vote.
pub const MIN_PLAYER_FRAMES: usize = 10;

/// Pick the extraction path from chunk 1-3 marker structure (the chunk0
/// manifest is map-schema-specific and not used):
///
/// * b9=0x56 frames with a body over 150 bytes (50 or more of them), or a
///   roster of 5 or more: multiplayer-composite. Checked first: a match of 5
///   or more players with a bot has both kinds of frame.
/// * b9=0x35 frames opened by a bot record (10 or more): bot-composite.
/// * else per-entity.
pub fn detect_encoding_mode(film: &Film, n_players: Option<usize>) -> EncodingMode {
    let mut n35 = 0usize;
    let mut n56big = 0usize;
    for ci in 1..film.chunks.len().min(4) {
        let c = &film.chunks[ci];
        let markers = film.markers(ci);
        for (mi, &pos) in markers.iter().enumerate() {
            if pos + 14 >= c.len() {
                continue;
            }
            let b5 = c[pos + 5];
            if b5 < 0x40 || (c[pos + 6] & 0x1f) != 0x09 {
                continue;
            }
            if c[pos + 7] == 0x40 && c[pos + 9] == 0x35 {
                n35 += 1;
            }
            if c[pos + 9] == 0x56 {
                let next = if mi + 1 < markers.len() { markers[mi + 1] } else { c.len() };
                if next - pos > 150 {
                    n56big += 1;
                }
            }
        }
    }
    if n56big >= 50 || n_players.is_some_and(|n| n >= 5) {
        return EncodingMode::MultiplayerComposite;
    }
    if n35 >= MIN_PLAYER_FRAMES {
        return EncodingMode::BotComposite;
    }
    EncodingMode::PerEntity
}
