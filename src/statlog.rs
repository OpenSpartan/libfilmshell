//! The scoreboard move by move: statborg updates in the tick packets, where
//! `gamestate` has only its value every 20 s.

use crate::bits::bits_at;
use crate::components::Params;
use crate::container::Film;
use crate::gamestate::{stat_value, ENGINE, ENGINE_PLAYER, ENTRY_ROW, ROWS, STATBORG};
use crate::holders::Holders;
use crate::payload::{OpKind, Payload};
use crate::snapshot::{self, ENTITY_TABLE};
use std::collections::{BTreeMap, HashMap};

/// One slot taking a new value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub frame: i64,
    /// The statborg's entry: 0 a player, 1 a team.
    pub kind: u32,
    /// Player index or team id.
    pub id: u32,
    pub slot: u32,
    pub value: i32,
    /// The round the value is for, 0 the first: the game sends it with each
    /// row, twice (the two counts agree on every reference film).
    pub round: u32,
}

/// A respawn countdown (`player-respawn-timer-component`: `[flag:1]`, seconds
/// left, seconds in all).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Respawn {
    /// The frame it started.
    pub frame: i64,
    pub player: u32,
    pub seconds: u32,
}

/// The match clock at a change of its limit or state
/// (`game-engine-round-timer-component`: limit and time left, floats over 0
/// to 36000 s in 16 bits, then 5 bits of state).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub frame: i64,
    pub limit: f64,
    pub left: f64,
    pub state: u32,
}

#[derive(Debug, Default)]
pub struct Log {
    pub changes: Vec<Change>,
    pub respawns: Vec<Respawn>,
    pub clock: Vec<Clock>,
}

/// The statborg slot of deaths, and how far apart a death and its countdown
/// can be reported.
const DEATHS_SLOT: u32 = 5;
const DEATH_FRAMES: i64 = 3;
/// `entries`' kind for an engine player.
const PLAYER_ROWS: u32 = 2;
const RESPAWN_ROW: u32 = 1;
const ROUND_TIMER_ROW: u32 = 5;
/// Seconds of a 16-bit timer step.
const TIMER_STEP: f64 = 36000.0 / 65535.0;

/// Per chunk, entity index -> entry (kind, id) of statborgs and engine players,
/// from its snapshots: a player who leaves frees their entity for the next
/// holder. An entity created inside a chunk is in the next chunk's map.
fn entries(film: &Film, params: Params) -> Vec<HashMap<u32, (u32, u32)>> {
    let ctx = snapshot::Context::new(&film.chunks[0], params, params.box_bits);
    let mut maps = vec![HashMap::new(); film.chunks.len()];
    for (ci, chunk) in film.chunks.iter().enumerate().skip(1) {
        let Some(ps) = film.packets(ci) else { continue };
        let out = &mut maps[ci];
        for pk in ps.iter().filter(|p| p.kind == ENTITY_TABLE) {
            let w = snapshot::walk(chunk, pk.start, pk.end, &ctx);
            for (e, rows) in w.entities.iter().zip(&w.rows) {
                if e.entity_type == ENGINE_PLAYER {
                    let lead = if bits_at(chunk, e.creation(), 1) == 1 { 9 } else { 1 };
                    out.insert(e.index, (PLAYER_ROWS, bits_at(chunk, e.creation() + lead, 6)));
                }
                if e.entity_type != STATBORG {
                    continue;
                }
                let Some(at) = rows.iter().find(|x| x.0 == ENTRY_ROW).map(|x| x.1) else { continue };
                let (id, kind) = (bits_at(chunk, at, 32), bits_at(chunk, at + 32, 8));
                match kind {
                    0 => out.insert(e.index, (0, id >> 16)),
                    1 => out.insert(e.index, (1, id & 0xffff)),
                    _ => None,
                };
            }
        }
    }
    maps
}

/// The engine player ids in the first replay chunk's snapshot: everyone in
/// the match when the film starts.
pub fn players_at_start(film: &Film, params: Params) -> Vec<u32> {
    let ctx = snapshot::Context::new(&film.chunks[0], params, params.box_bits);
    let Some(chunk) = film.chunks.get(1) else { return Vec::new() };
    let Some(pk) = film.packets(1).and_then(|ps| ps.iter().find(|p| p.kind == ENTITY_TABLE)) else { return Vec::new() };
    let w = snapshot::walk(chunk, pk.start, pk.end, &ctx);
    let mut ids: Vec<u32> = w
        .entities
        .iter()
        .filter(|e| e.entity_type == ENGINE_PLAYER)
        .map(|e| {
            let lead = if bits_at(chunk, e.creation(), 1) == 1 { 9 } else { 1 };
            bits_at(chunk, e.creation() + lead, 6)
        })
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub fn reader<'a>(
    film: &'a Film,
    params: Params,
    holders: &'a Holders,
    log: &'a mut Log,
) -> impl FnMut(usize, &[u8], &Payload) + 'a {
    let entries = entries(film, params);
    let frames: Vec<_> = film.frames().collect();
    let at: BTreeMap<usize, usize> = frames.iter().enumerate().map(|(k, f)| (f.index, k)).collect();
    let chunk_of: HashMap<*const u8, usize> = film.chunks.iter().enumerate().map(|(i, c)| (c.as_ptr(), i)).collect();
    let mut last: HashMap<(u32, u32, u32), (i32, u32)> = HashMap::new();
    let mut left: HashMap<u32, u32> = HashMap::new();
    move |i, _, p| {
        let Some(f) = at.get(&i).map(|&k| &frames[k]) else { return };
        for op in &p.ops {
            let OpKind::Update { fields, .. } = &op.kind else { continue };
            let ci = chunk_of[&f.chunk.as_ptr()];
            if op.entity_type == Some(ENGINE) {
                for &(_, b) in fields.blocks.iter().filter(|x| x.0 == ROUND_TIMER_ROW) {
                    let (limit, time) = (bits_at(f.chunk, b, 16) as f64, bits_at(f.chunk, b + 16, 16) as f64);
                    let state = bits_at(f.chunk, b + 32, 5);
                    if log.clock.last().is_none_or(|c: &Clock| c.state != state || c.limit != limit * TIMER_STEP) {
                        log.clock.push(Clock {
                            frame: i as i64,
                            limit: limit * TIMER_STEP,
                            left: time * TIMER_STEP,
                            state,
                        });
                    }
                }
            }
            let found = entries[ci].get(&op.index).or_else(|| entries[ci + 1..].iter().find_map(|m| m.get(&op.index)));
            let Some(&(kind, id)) = found else { continue };
            if kind == PLAYER_ROWS {
                for &(_, b) in fields.blocks.iter().filter(|x| x.0 == RESPAWN_ROW) {
                    let (now, total) = (bits_at(f.chunk, b + 1, 10), bits_at(f.chunk, b + 11, 10));
                    // A countdown starts when the value goes up.
                    if total > 0 && now > left.get(&id).copied().unwrap_or(0) {
                        log.respawns.push(Respawn {
                            frame: i as i64,
                            player: holders.api(id, i as i64),
                            seconds: total,
                        });
                    }
                    left.insert(id, now);
                }
                continue;
            }
            for &(row, b) in fields.blocks.iter().filter(|x| x.0 < ROWS) {
                // The round, twice in 5 bits, then the row's two values.
                let round = bits_at(f.chunk, b, 5);
                let (x, q) = stat_value(f.chunk, b + 10);
                let (y, _) = stat_value(f.chunk, q);
                for (slot, value) in [(2 * row, x), (2 * row + 1, y)] {
                    // A value is new when it changes, or when a new round restates it.
                    if last.insert((kind, id, slot), (value, round)).unwrap_or((0, 0)) != (value, round) {
                        let id = if kind == 0 { holders.api(id, i as i64) } else { id };
                        log.changes.push(Change { frame: i as i64, kind, id, slot, value, round });
                    }
                }
            }
        }
    }
}

/// What [`reader`] leaves to do once the walk is over.
pub fn finish(log: &mut Log) {
    fix_respawn_owners(log);
}

/// A death and its countdown share a frame. An engine player slot is reused by
/// a joiner, also without a bot between, so the slot's first holder can be the
/// wrong owner: when the holder didn't die on that frame and exactly one other
/// player did, without a countdown of their own, the countdown is theirs.
fn fix_respawn_owners(log: &mut Log) {
    let mut deaths: HashMap<i64, Vec<u32>> = HashMap::new();
    for c in log.changes.iter().filter(|c| c.kind == 0 && c.slot == DEATHS_SLOT) {
        for f in c.frame - DEATH_FRAMES..=c.frame + DEATH_FRAMES {
            deaths.entry(f).or_default().push(c.id);
        }
    }
    let starts: HashMap<i64, Vec<u32>> = log.respawns.iter().fold(HashMap::new(), |mut m, r| {
        m.entry(r.frame).or_default().push(r.player);
        m
    });
    for r in &mut log.respawns {
        let d = deaths.get(&r.frame).map_or(&[][..], |v| &v[..]);
        if d.contains(&r.player) {
            continue;
        }
        let mine = &starts[&r.frame];
        let free: Vec<u32> = d
            .iter()
            .copied()
            .filter(|p| !mine.contains(p))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if free.len() == 1 {
            r.player = free[0];
        }
    }
}
