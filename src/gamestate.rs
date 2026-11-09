//! The game's own running state, from the snapshots (a seek point every 20 s):
//! each player's lives and whether they're in the game, and the engine's
//! phase.
//!
//! The statborg is the game's scoreboard: one entity per player and per team,
//! 28 rows of two values each, 56 slots.

use crate::bits::bits_at;
use crate::components::Params;
use crate::container::Film;
use crate::holders::Holders;
use crate::snapshot::{self, ENTITY_TABLE};
use std::collections::BTreeMap;

/// The registry block of a player's engine state.
pub const ENGINE_PLAYER: u32 = 5;
pub const STATBORG: u32 = 6;
pub const ENGINE: u32 = 2;
/// Statborg row of the entry's index and type.
pub const ENTRY_ROW: u32 = 57;
pub const ROWS: u32 = 28;
/// Engine player rows.
const LIVES_ROW: u32 = 14;
const ACTIVE_ROW: u32 = 18;
/// Engine rows.
const ENGINE_STATE_ROW: u32 = 2;

#[derive(Debug, Default)]
pub struct GameState {
    /// Each snapshot's frame.
    pub frames: Vec<i64>,
    /// The engine's phase per snapshot: 1 in play, 2 over.
    pub phase: Vec<u32>,
    /// Player index to lives left per snapshot (-1: unlimited; absent: not in
    /// the game then).
    pub lives: BTreeMap<u32, Vec<Option<i32>>>,
}

/// The statistic at `p`: `[size:2]`, then `8 << size` bits,
/// signed. Returns it and the bit after it.
pub fn stat_value(c: &[u8], p: i64) -> (i32, i64) {
    let n = 8u32 << bits_at(c, p, 2);
    let v = bits_at(c, p + 2, n.min(32));
    let v = if n < 32 && v >> (n - 1) & 1 == 1 { v as i64 - (1i64 << n) } else { v as i32 as i64 };
    (v as i32, p + 2 + n as i64)
}

pub fn read(film: &Film, widths: [u32; 3], box_bits: u32, holders: &Holders) -> GameState {
    let ctx = snapshot::Context::new(&film.chunks[0], Params::new(widths), box_bits);
    let mut g = GameState::default();
    let mut frame = 0i64;
    for (ci, chunk) in film.chunks.iter().enumerate().skip(1) {
        let Some(ps) = film.packets(ci) else { continue };
        for pk in ps {
            if pk.kind == 0 {
                frame += 1;
            }
            if pk.kind != ENTITY_TABLE {
                continue;
            }
            let w = snapshot::walk(chunk, pk.start, pk.end, &ctx);
            let at = g.frames.len();
            g.frames.push(frame);
            g.phase.push(0);
            for (e, rows) in w.entities.iter().zip(&w.rows) {
                let block = |r: u32| rows.iter().find(|x| x.0 == r).map(|x| x.1);
                match e.entity_type {
                    ENGINE => {
                        if let Some(p) = block(ENGINE_STATE_ROW) {
                            g.phase[at] = bits_at(chunk, p, 3);
                        }
                    }
                    ENGINE_PLAYER => {
                        let Some(active) = block(ACTIVE_ROW) else { continue };
                        if bits_at(chunk, active, 1) == 0 {
                            continue;
                        }
                        let lead = if bits_at(chunk, e.creation(), 1) == 1 { 9 } else { 1 };
                        let index = bits_at(chunk, e.creation() + lead, 6);
                        let Some(l) = block(LIVES_ROW) else { continue };
                        let series = g.lives.entry(holders.api(index, frame)).or_default();
                        series.resize(at + 1, None);
                        series[at] = Some(bits_at(chunk, l, 7) as i32 - 1);
                    }
                    _ => {}
                }
            }
        }
    }
    let n = g.frames.len();
    for s in g.lives.values_mut() {
        s.resize(n, None);
    }
    g
}
