//! The per-chunk seek snapshot.
//!
//! A chunk opens with packets 1 and 2: the state a player seeking to the
//! chunk restores. Playback skips both. Type 2 is the replication entity
//! table, one bitstream of a `[1]` flag and, for every entity slot in index
//! order:
//!
//! ```text
//! [id:32]   generation in the top two bits, the entity index below
//! [type:32] the entity type (registry block); all ones for an empty slot
//! [c:32]    objects: the object's handle
//! [flags:4][8]
//! type set: [n:32] creation data  [m:32] state: each component of the
//!           block in registry order (no lengths, no framing). n and m are
//!           the game's structure sizes, not lengths in the stream.
//! ```
//!
//! An empty slot is 108 bits, all ones then twelve zeros. There are no
//! lengths, and the non-object creation readers and many components aren't
//! written, so a live record is still found by its header: the index climbs,
//! and the fixed bits (17 + 26 zeros, the flags, a zero byte) rule out chance.

use crate::components::{self, Params, Table};
use crate::creation::{self, Creation, Form};
use crate::registry;

pub const ENTITY_TABLE: u16 = 2;

/// Bits of a type-2 record's header.
const HEADER_BITS: i64 = 108;

/// One live entity of a type-2 snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entity {
    /// Absolute bit of the record.
    pub at: i64,
    pub index: u32,
    pub generation: u32,
    /// Registry block.
    pub entity_type: u32,
    /// The third header field: an object's handle.
    pub handle: u32,
    /// The slot's 4-bit flags: 3 on nearly every entity, also 0 and 1. What
    /// they mean isn't traced.
    pub flags: u32,
    /// The `n` field: the size of the type's creation structure (0 for none).
    pub n: u32,
}

impl Entity {
    /// The first bit of the creation data, after `n`.
    pub fn creation(&self) -> i64 {
        self.at + HEADER_BITS + 32
    }
}

/// The 32 bits at `at`; `at` must leave 8 bytes in `c`.
fn u32_at(c: &[u8], at: i64) -> u32 {
    let i = (at >> 3) as usize;
    let w = u64::from_be_bytes(c[i..i + 8].try_into().unwrap());
    ((w << (at & 7)) >> 32) as u32
}

/// The size of a type's creation structure, slot 4 of its definition.
fn creation_size(entity_type: u32) -> u32 {
    match entity_type {
        3 | 9 | 22 => 12,
        5 => 2,
        6 | 14 | 17 | 21 | 24 | 28 | 47 | 48 => 4,
        7 | 10 => 8,
        11 | 12 | 20 | 23 | 29 | 46 | 49 => 1,
        13 => 136,
        35 => 152,
        36 | 43 => 96,
        37..=39 => 104,
        40 => 176,
        41 | 44 => 212,
        42 => 168,
        _ => 0,
    }
}

/// The record of a live slot at `at` when its fixed bits say so: a type,
/// the flags and a zero byte, and the sizes that go with them.
fn live_header(c: &[u8], at: i64) -> Option<Entity> {
    let id = u32_at(c, at);
    let entity_type = u32_at(c, at + 32);
    let flags = u32_at(c, at + 96) >> 20;
    if entity_type >= 64 || flags & 0xe0 != 0 {
        return None;
    }
    let n = u32_at(c, at + 108);
    // An unreferenced slot has no handle and an empty `n` and `m`; a live
    // one has its type's creation size.
    let plausible = match flags >> 8 {
        0 => u32_at(c, at + 64) == u32::MAX && n == 0 && u32_at(c, at + 140) == 0,
        1 | 3 => n == creation_size(entity_type),
        _ => false,
    };
    plausible.then(|| Entity {
        at,
        index: id & 0x3fff_ffff,
        generation: id >> 30,
        entity_type,
        handle: u32_at(c, at + 64),
        flags: flags >> 8,
        n,
    })
}

/// The live entities of the type-2 payload `c[start..end]`, by index.
pub fn entity_table(c: &[u8], start: usize, end: usize) -> Vec<Entity> {
    let (first, last) = (8 * start as i64 + 1, 8 * end as i64 - HEADER_BITS - 128);
    let mut out = Vec::new();
    let mut next_index = 0u32;
    let mut at = first;
    while at < last {
        // [gen:2][17 zeros][index:13]: the second bit onward must open with
        // zeros; test the cheap byte first.
        let id = u32_at(c, at);
        let (generation, index) = (id >> 30, id & 0x3fff_ffff);
        // A wrapped generation 0 is left to `walk`: here a run of zeros would
        // pass for one and end the scan early.
        if generation == 0 || index >= 8192 || index < next_index {
            at += 1;
            continue;
        }
        let Some(e) = live_header(c, at) else {
            at += 1;
            continue;
        };
        out.push(e);
        next_index = index + 1;
        at += HEADER_BITS;
    }
    out
}

/// The first bit of an object's state and what its creation data names:
/// past the creation data and `[m:32]`, the state's structure size.
pub fn creation(c: &[u8], e: &Entity) -> Option<(Creation, i64)> {
    if e.n == 0 {
        return None;
    }
    let made = creation::read(c, e.creation(), e.entity_type, Form::Snapshot)?;
    Some((made, made.end + 32))
}

/// The first bit of any entity's state, past its creation data and `m`.
pub fn state_start(c: &[u8], e: &Entity) -> Option<i64> {
    if e.n == 0 {
        return Some(e.creation() + 32);
    }
    if (35..=43).contains(&e.entity_type) {
        return creation(c, e).map(|x| x.1);
    }
    Some(creation::manager_end(c, e.creation(), e.entity_type)? + 32)
}

/// What a film's snapshots are read with: its registry and the level's
/// widths.
pub struct Context {
    tables: Vec<Table>,
    lists: Vec<Vec<u32>>,
    versions: Vec<Vec<u32>>,
    params: Params,
}

impl Context {
    pub fn new(chunk0: &[u8], mut params: Params, box_bits: u32) -> Context {
        params.snapshot = true;
        params.box_bits = box_bits;
        let blocks = registry::blocks(chunk0);
        Context {
            tables: blocks.iter().enumerate().map(|(k, r)| Table::from_rows(r, k as u32)).collect(),
            lists: blocks
                .iter()
                .map(|b| (0..b.len() as u32).filter(|&k| !b[k as usize].is_empty()).collect())
                .collect(),
            versions: registry::versions(chunk0),
            params,
        }
    }
}

/// A snapshot read in sequence.
pub struct Walk {
    pub entities: Vec<Entity>,
    /// Per entity, where each of its components' blocks starts.
    pub rows: Vec<Vec<(u32, i64)>>,
    /// Slots read, empty ones too.
    pub slots: u64,
    /// The bit after the last slot read.
    pub end: i64,
    /// Where the read stopped: the entity type and component (64: its
    /// creation data), when it did.
    pub stopped: Option<(u32, u32)>,
}

/// The vehicle entity type.
const VEHICLE: u32 = 40;

/// Whether the next slot's record plausibly starts at `at`, after the entity
/// of index `index`: an empty slot (all ones, then twelve zeros), a live
/// entity of a later index, or the payload's end.
fn slot_follows(c: &[u8], at: i64, index: u32, last: i64) -> bool {
    if at > last {
        return at <= last + HEADER_BITS;
    }
    if (0..3).all(|k| u32_at(c, at + 32 * k) == u32::MAX) {
        return u32_at(c, at + 96) >> 20 == 0;
    }
    live_header(c, at).is_some_and(|e| (index + 1..8192).contains(&e.index))
}

/// Read the type-2 payload `c[start..end]` as the game does:
/// one slot after another until the slots run out.
pub fn walk(c: &[u8], start: usize, end: usize, ctx: &Context) -> Walk {
    let mut w = Walk { entities: Vec::new(), rows: Vec::new(), slots: 0, end: 8 * start as i64 + 1, stopped: None };
    let last = 8 * end as i64 - HEADER_BITS;
    while w.end <= last {
        let at = w.end;
        let id = u32_at(c, at);
        let entity_type = u32_at(c, at + 32);
        w.slots += 1;
        w.end += HEADER_BITS;
        if entity_type == u32::MAX {
            continue;
        }
        let word = u32_at(c, at + 96);
        let e = Entity {
            at,
            index: id & 0x3fff_ffff,
            generation: id >> 30,
            entity_type,
            handle: u32_at(c, at + 64),
            flags: word >> 28,
            n: u32_at(c, at + 108),
        };
        if entity_type as usize >= ctx.tables.len() {
            w.stopped = Some((entity_type, 65));
            return w;
        }
        let Some(state) = state_start(c, &e) else {
            w.stopped = Some((entity_type, 64));
            return w;
        };
        w.end = state;
        w.entities.push(e);
        w.rows.push(Vec::new());
        if u32_at(c, state - 32) == 0 {
            continue;
        }
        let k = entity_type as usize;
        let read = |params: &Params| {
            components::parse_versions(c, state, &ctx.lists[k], &ctx.tables[k], &ctx.versions[k], params)
        };
        let mut parsed = read(&ctx.params);
        if entity_type == VEHICLE && !parsed.end.is_some_and(|x| slot_follows(c, x, e.index, last)) {
            // Nothing in the snapshot says whether this vehicle has its type
            // state and physics (`Params::vehicle_type`): the reading that
            // lands on the next slot.
            let typed = read(&Params { vehicle_type: true, ..ctx.params });
            if typed.end.is_some_and(|x| slot_follows(c, x, e.index, last)) {
                parsed = typed;
            }
        }
        *w.rows.last_mut().unwrap() = parsed.starts;
        match parsed.end {
            Some(end) => w.end = end,
            None => {
                w.stopped = Some((entity_type, parsed.failed.unwrap_or(66)));
                return w;
            }
        }
    }
    w
}

/// Every entity of a snapshot: read in sequence, and past a stop by the scan.
pub fn entities(c: &[u8], start: usize, end: usize, ctx: &Context) -> Vec<Entity> {
    let w = walk(c, start, end, ctx);
    let mut out = w.entities;
    if w.stopped.is_some() {
        let last = out.last().map_or(0, |e| e.index);
        out.extend(entity_table(c, start, end).into_iter().filter(|e| e.index > last));
    }
    out
}

/// An object's position in a snapshot, in engine units. Unlike an update's
/// quantized codes it is three raw little-endian floats, after `[0][0][0]`
/// on the dynamic-precision types (bipeds, vehicles) and `[0]` on the rest.
/// None on any other branch; an object in a holder reads (0, 0, 0).
pub fn raw_position(c: &[u8], e: &Entity, state: i64) -> Option<[f32; 3]> {
    let lead: u32 = if matches!(e.entity_type, 35 | 40) { 3 } else { 1 };
    if u32_at(c, state) >> (32 - lead) != 0 {
        return None;
    }
    let f = |k: i64| f32::from_bits(u32_at(c, state + lead as i64 + 32 * k).swap_bytes());
    let p = [f(0), f(1), f(2)];
    p.iter().all(|v| v.is_finite()).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_follows_an_entity() {
        // An empty slot: 96 ones, then twelve zeros.
        let mut c = vec![0xff; 12];
        c.extend([0; 16]);
        assert!(slot_follows(&c, 0, 5, 1000));
        // A run of ones alone is no slot.
        assert!(!slot_follows(&[0xff; 28], 0, 5, 1000));
        // Past the last slot: the payload's end.
        assert!(slot_follows(&c, 1001, 5, 1000));
    }
}
