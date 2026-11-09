//! Object updates in multiplayer-composite frames.
//!
//! Each frame bit-packs every replicated object's update at arbitrary bit
//! alignment, MSB-first:
//!
//! ```text
//! [3 class][8 index][2 generation][5 count][count x 6-bit component][5 selector][component data ...]
//!   class       010 biped (players, bots, PvE AI), 011 vehicle
//!   generation  counts index wraps (lives 0-255 are generation 01)
//!   component   strictly increasing indices into the archetype's block of
//!               the chunk-0 registry (`registry.rs`); the data that follows
//!               is one block per listed component, in list order
//! slot_id = (generation - 1) * 256 + index
//! ```
//!
//! Class and index are the low 11 bits of the entity view's 13-bit entity
//! index (bipeds 512-767, vehicles 768-1023), and the `100` before an update
//! is the entry's op bit and the index's top bits. `payload.rs` reads the
//! payload by that grammar; this module scans for updates.
//!
//! The index + generation name one object (one biped life) for the whole
//! match. An update sends only the components that changed:
//!
//! * a **position update** lists position first and the command tick, with
//!   selector 0; position (c1, c2, Z, quantized over the level's box) is the
//!   first data block;
//! * a **stub** lists only aim and command tick: alive, position unchanged.
//!
//! Every live biped update lists the command tick. Lists without it are
//! dead bodies (`[0, 1, 2, 3]`, the ragdoll), or noise: a scan at every bit
//! offset passes short headers by chance.

use crate::bits::Shifted;
use crate::container::Film;
use crate::registry::{Archetype, Schema};
use crate::types::RawSample;
use indexmap::IndexMap;
use std::collections::HashMap;

/// Longest component list. The count is 3 bits: the field's top two bits
/// are 0 on every valid list of a live handle, and a longer count is a
/// chance header.
const MAX_COMPONENTS: u32 = 7;
/// Record gap (frames) that splits a slot's records into segments.
const SEGMENT_GAP: i64 = 600;
/// Median c1 step (15-bit window units per frame) above which a segment is
/// not a track; a sprint is about 3.
const MAX_SEGMENT_STEP: f64 = 64.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Biped,
    Vehicle,
    Stub,
}

/// `(generation - 1) * 256 + index` from the object handle.
fn object_id(g: &Shifted) -> i32 {
    ((((g.get(1) >> 3) & 3) as i32) - 1) * 256 + ((((g.get(0) & 0x1f) << 3) | (g.get(1) >> 5)) as i32)
}

/// The component list of the update `g` dispatched with `off`.
pub fn components<'g>(g: &'g Shifted, off: i32) -> impl Iterator<Item = u32> + 'g {
    let count = (off + 24) / 6;
    (0..count).map(move |m| g.read(6 * m - 22, 6))
}

/// The component list of the update at `g`, if its indices are strictly
/// increasing rows of `arch`: (count, the list as a bitmask).
fn component_list(g: &Shifted, count: u32, arch: &Archetype) -> Option<u64> {
    let mut mask = 0u64;
    let mut prev: i64 = -1;
    for m in 0..count as i32 {
        let x = g.read(6 * m - 22, 6);
        if x as i64 <= prev || x >= arch.rows {
            return None;
        }
        prev = x as i64;
        mask |= 1 << x;
    }
    Some(mask)
}

/// An object update the scan found: its frame, where it starts, and what
/// the scan read of it.
#[derive(Clone, Copy, Debug)]
pub struct Found {
    frame: u32,
    q: u32,
    sh: u8,
    kind: Kind,
    id: i32,
    off: i32,
}

/// Every object update in the frames of chunks 1.., dispatched as
/// `f(g, gp, frame, kind, id, off)`. `off` is the bit shift of the data
/// blocks against a 4-component list: position's c1 is at `read(7 + off)`
/// (0 for stubs). Returning false stops. The scan runs once per film; later
/// calls read what it found.
pub fn for_each_object_update<'a, F>(film: &'a Film, mut f: F)
where
    F: FnMut(&Shifted<'a>, i64, i64, Kind, i32, i32) -> bool,
{
    for u in film.object_updates.get_or_init(|| scan_object_updates(film)) {
        let fr = film.frame(u.frame as usize);
        let g = Shifted { c: fr.chunk, q: u.q as usize, sh: u.sh as u32 };
        if !f(&g, fr.gp, u.frame as i64, u.kind, u.id, u.off) {
            return;
        }
    }
}

/// Scan every frame of chunks 1.. for object updates.
fn scan_object_updates(film: &Film) -> Vec<Found> {
    let mut found = Vec::new();
    let schema = Schema::read(&film.chunks[0]);
    let stub_list = (1u64 << schema.biped.aim) | (1u64 << schema.biped.command_tick);
    let mut biped_max = [-1i32; 4];
    let mut started = false;
    for fr in film.frames() {
        let tick = fr.index as i64;
        let c = fr.chunk;
        let end = fr.end as isize;
        let mut q = fr.body;
        while (q as isize) < end - 12 {
            let mut hit = false;
            for sh in 0..8u32 {
                let g = Shifted { c, q, sh };
                let g0 = g.get(0);
                if !(0x40..=0x7f).contains(&g0) {
                    continue;
                }
                let g1 = g.get(1);
                let gen = ((g1 >> 3) & 3) as usize;
                if gen == 0 {
                    continue;
                }
                let is_biped = g0 < 0x60;
                let index = (((g0 & 0x1f) << 3) | (g1 >> 5)) as i32;
                // Generation g starts only after g - 1 has used its indexes up.
                if is_biped && gen > 1 && biped_max[gen - 1] < 248 {
                    continue;
                }
                let count = ((g1 & 7) << 2) | (g.get(2) >> 6);
                if count == 0 || count > MAX_COMPONENTS {
                    continue;
                }
                let arch = if is_biped { &schema.biped } else { &schema.vehicle };
                let Some(list) = component_list(&g, count, arch) else { continue };
                if list & (1 << arch.command_tick) == 0 {
                    continue;
                }
                let kind;
                let mut off = 0i32;
                if list & 1 << arch.position != 0 && list & ((1 << arch.position) - 1) == 0 {
                    // Selector, then position as the first data block.
                    if g.read(6 * count as i32 - 22, 5) != 0 {
                        continue;
                    }
                    // The match starts with the first biped update that lists
                    // aim; before it the players stand on their spawn pads
                    // choosing loadouts (lists carry 48 and 56, never 21), and
                    // only some of them send records.
                    if is_biped && !started {
                        if list & 1 << arch.aim == 0 {
                            continue;
                        }
                        started = true;
                    }
                    off = 6 * count as i32 - 24;
                    kind = if is_biped { Kind::Biped } else { Kind::Vehicle };
                    if is_biped && index > biped_max[gen] {
                        biped_max[gen] = index;
                    }
                } else if is_biped && list == stub_list {
                    kind = Kind::Stub;
                } else {
                    continue;
                }
                found.push(Found { frame: tick as u32, q: q as u32, sh: sh as u8, kind, id: object_id(&g), off });
                hit = true;
                break;
            }
            // Step past the validated header only: records are 51-190 bits.
            q += if hit { 5 } else { 1 };
        }
    }
    found
}

/// Smoothness of a 15-bit window at each candidate payload bit: the median
/// over slots of the 95th percentile step between a slot's consecutive
/// biped updates, over the first 2,000. A window that starts on a field's MSB
/// scores in the tens; one that starts inside a field wraps and scores in the
/// thousands. None where no slot has 8 updates.
fn seam_scores(film: &Film, cands: &[i32]) -> Vec<Option<f64>> {
    let mut by_slot: Vec<HashMap<i32, (u32, Vec<f64>)>> = vec![HashMap::new(); cands.len()];
    let mut n = 0usize;
    for_each_object_update(film, |g, _gp, _t, kind, slot, off| {
        if kind != Kind::Biped {
            return true;
        }
        for (ci, &cand) in cands.iter().enumerate() {
            let v = g.read(cand + off, 15);
            match by_slot[ci].get_mut(&slot) {
                None => {
                    by_slot[ci].insert(slot, (v, Vec::new()));
                }
                Some(s) => {
                    s.1.push((v as f64 - s.0 as f64).abs());
                    s.0 = v;
                }
            }
        }
        n += 1;
        n < 2000
    });
    by_slot
        .iter_mut()
        .map(|slots| {
            let mut p95s: Vec<f64> = Vec::new();
            for s in slots.values_mut() {
                if s.1.len() < 8 {
                    continue;
                }
                s.1.sort_by(f64::total_cmp);
                p95s.push(s.1[(s.1.len() as f64 * 0.95).floor() as usize]);
            }
            p95s.sort_by(f64::total_cmp);
            (!p95s.is_empty()).then(|| p95s[p95s.len() / 2])
        })
        .collect()
}

/// c2's start bit (7 plus c1's width) by smoothness: the candidate whose
/// window scores lowest (`seam_scores`).
pub fn detect_coord_layout(film: &Film) -> i32 {
    const CANDS: [i32; 8] = [19, 20, 21, 22, 23, 24, 25, 26];
    let mut best = 22;
    let mut best_score = f64::INFINITY;
    for (&cand, score) in CANDS.iter().zip(seam_scores(film, &CANDS)) {
        if let Some(s) = score {
            if s < best_score {
                best_score = s;
                best = cand;
            }
        }
    }
    best
}

/// A 15-bit window reads wrapping noise on this scale; a window on a field's
/// MSB scores in the tens (`seam_scores`).
const ROUGH_SEAM: f64 = 1000.0;

/// c2's width. Z follows c2, so c2 ends where a window first turns smooth
/// again past c2's start; c2 is within a bit of c1's width. Falls back to
/// c1's width.
pub fn detect_c2_width(film: &Film, c2_start: i32) -> u32 {
    let n1 = c2_start - 7;
    let cands: Vec<i32> = (n1 - 2..=n1 + 2).map(|w| c2_start + w).collect();
    let scores = seam_scores(film, &cands);
    for i in 1..cands.len() {
        if let (Some(prev), Some(s)) = (scores[i - 1], scores[i]) {
            if prev >= ROUGH_SEAM && s < ROUGH_SEAM && (n1 - 1..=n1 + 1).contains(&(cands[i] - c2_start)) {
                return (cands[i] - c2_start) as u32;
            }
        }
    }
    n1 as u32
}

/// A coordinate field of `width` bits at payload bit `at`, in 15-bit window
/// units (field value times 2^(15 - width)): a field wider than 15 bits keeps its
/// low bits as a fraction, a narrower one reads no bits of the next field.
#[inline]
pub fn read_coord(g: &Shifted, at: i32, width: u32) -> f64 {
    g.read(at, width) as f64 * (2.0f64).powi(15 - width as i32)
}

pub struct CompositeExtraction {
    /// Biped slot_id to samples, in first-seen order.
    pub slots: IndexMap<i32, Vec<RawSample>>,
    /// Biped slot_id to the frames that carried a stub for it.
    pub stubs: IndexMap<i32, Vec<i64>>,
    /// Vehicle id to samples, not attributed to anyone yet.
    pub vehicles: IndexMap<i32, Vec<RawSample>>,
    /// Frame index to game tick.
    pub game_ticks: Vec<u32>,
    /// Frame index to the frame's global byte position, for frames that
    /// carried an update.
    pub tick_gp: Vec<Option<i64>>,
}

/// A slot's records are one life. Split at gaps over SEGMENT_GAP frames, a
/// segment that is a single record, or whose c1 moves more than
/// MAX_SEGMENT_STEP window units per frame (median), is chance headers: a
/// one-off list, or a structure elsewhere in the stream that parses as a
/// biped header over and over at random positions. Kept, it would stretch
/// the life by minutes. A real segment moves 3 units a frame at most.
fn drop_chance_segments(v: &mut Vec<RawSample>) {
    let mut keep = vec![true; v.len()];
    let mut start = 0;
    for i in 1..=v.len() {
        if i < v.len() && v[i].tick - v[i - 1].tick <= SEGMENT_GAP {
            continue;
        }
        let seg = &v[start..i];
        let mut steps: Vec<f64> =
            seg.windows(2).map(|w| (w[1].c1 - w[0].c1).abs() / (w[1].tick - w[0].tick).max(1) as f64).collect();
        steps.sort_by(f64::total_cmp);
        if seg.len() < 2 || steps[steps.len() / 2] > MAX_SEGMENT_STEP {
            keep[start..i].iter_mut().for_each(|k| *k = false);
        }
        start = i;
    }
    let mut k = keep.into_iter();
    v.retain(|_| k.next().unwrap());
}

/// `c2_start` from `detect_coord_layout`, `c2_width` from `detect_c2_width`;
/// `c1_at`: where c1 starts (`levels::c1_at`; 7 unless the level's box index
/// is wider than a bit).
pub fn extract_multiplayer_composite(film: &Film, c1_at: i32, c2_start: i32, c2_width: u32) -> CompositeExtraction {
    let c1_width = (c2_start - c1_at) as u32;
    let actor_control = Schema::read(&film.chunks[0]).biped.actor_control;
    let mut slots: IndexMap<i32, Vec<RawSample>> = IndexMap::new();
    let mut stubs: IndexMap<i32, Vec<i64>> = IndexMap::new();
    let mut vehicles: IndexMap<i32, Vec<RawSample>> = IndexMap::new();
    let game_ticks = film.frame_ticks().to_vec();
    let mut tick_gp: Vec<Option<i64>> = vec![None; game_ticks.len()];
    for_each_object_update(film, |g, gp, tick, kind, id, off| {
        if (tick as usize) < tick_gp.len() {
            tick_gp[tick as usize] = Some(gp);
        }
        if kind == Kind::Stub {
            stubs.entry(id).or_default().push(tick);
            return true;
        }
        let target = if kind == Kind::Vehicle { &mut vehicles } else { &mut slots };
        let mut s =
            RawSample::new(gp, tick, read_coord(g, c1_at + off, c1_width), read_coord(g, c2_start + off, c2_width));
        // Bots and Firefight AI list actor control.
        s.ai = kind == Kind::Biped && components(g, off).any(|x| x == actor_control);
        target.entry(id).or_default().push(s);
        true
    });
    for v in slots.values_mut() {
        drop_chance_segments(v);
    }
    slots.retain(|_, v| !v.is_empty());
    CompositeExtraction { slots, stubs, vehicles, game_ticks, tick_gp }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scan_runs_once_and_stops_where_asked() {
        let Some(chunks) = crate::testing::chunks("f9129808") else { return };
        let film = Film::new(chunks, crate::container::Framing::Packets);
        let mut all = Vec::new();
        for_each_object_update(&film, |g, gp, frame, kind, id, off| {
            all.push((g.q, g.sh, gp, frame, kind, id, off));
            true
        });
        assert!(all.len() > 10_000);
        assert!(film.object_updates.get().is_some());
        let mut first = Vec::new();
        for_each_object_update(&film, |g, gp, frame, kind, id, off| {
            first.push((g.q, g.sh, gp, frame, kind, id, off));
            first.len() < 100
        });
        assert_eq!(first, all[..100]);
    }
}
