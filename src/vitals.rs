//! Each biped life's health and shield over time. Both are fractions of
//! the biped's maximum (the game's `prop_body_vitality_fraction` and
//! `prop_shield_vitality_fraction`) and no create or update states them at
//! spawn, so a life starts at 1.0. After that the tick packets' updates change them, read with the payload
//! walker because a player standing still and taking damage sends no position,
//! and every chunk's seek snapshot restates them. A mode without shields says
//! so in the biped's maximum vitalities: a shield maximum of zero.

use crate::bits::bits_at;
use crate::components::{dequantize, Params};
use crate::container::Film;
use crate::levels::LevelBox;
use crate::payload::{OpKind, Payload};
use crate::snapshot::{self, ENTITY_TABLE};
use std::collections::HashMap;

/// The biped entity type in a tick packet's creates and in the snapshots.
const BIPED_TYPE: u32 = 35;
/// Components of the biped block.
const HEALTH: u32 = 4;
const SHIELD: u32 = 5;
const MAX_VITALITIES: u32 = 13;

#[derive(Default)]
struct Life {
    /// (frame, health or unchanged, shield or unchanged), then, once read in
    /// order, each mark holds the full state as of it.
    marks: Vec<(i64, Option<f64>, Option<f64>)>,
    shieldless: Option<bool>,
}

/// Each life's marks while a film is being read: [`Vitals::start`], then
/// [`Vitals::reader`] over a walk, then [`Vitals::finish`].
#[derive(Default)]
pub struct Marks(HashMap<i32, Life>);

pub struct Vitals {
    lives: HashMap<i32, Life>,
    /// Most lives' answer, for a life with no statement of its own.
    shieldless: bool,
}

fn slot_of(index: u32, generation: u32) -> i32 {
    (generation as i32 - 1) * 256 + (index as i32 - 512)
}

/// Whether a snapshot's maximum vitalities block sets a shield maximum of 0.
fn shieldless_at(c: &[u8], mut at: i64) -> bool {
    let flags = bits_at(c, at, 5);
    at += 5;
    if flags & 4 != 0 {
        for _ in 0..3 {
            at += if bits_at(c, at, 1) == 1 { 13 } else { 1 };
        }
        at += 1;
    }
    flags & 8 != 0 && bits_at(c, at, 1) == 1 && bits_at(c, at + 1, 12) == 0
}

impl Vitals {
    /// The marks the seek snapshots give.
    pub fn start(film: &Film, params: &Params, level: Option<&LevelBox>) -> Marks {
        let mut lives: HashMap<i32, Life> = HashMap::new();
        // Snapshots first: sorting keeps them before a same-frame update.
        if let Some(level) = level {
            let mut first_frame: HashMap<*const u8, i64> = HashMap::new();
            for f in film.frames() {
                first_frame.entry(f.chunk.as_ptr()).or_insert(f.index as i64);
            }
            let ctx = snapshot::Context::new(&film.chunks[0], *params, level.box_bits);
            for ci in 1..film.chunks.len() {
                let c = &film.chunks[ci];
                let Some(frame) = first_frame.get(&c.as_ptr()).copied() else { continue };
                let Some(packet) = film.packets(ci).and_then(|p| p.iter().find(|x| x.kind == ENTITY_TABLE)) else {
                    continue;
                };
                let walk = snapshot::walk(c, packet.start, packet.end, &ctx);
                for (e, rows) in walk.entities.iter().zip(&walk.rows) {
                    if e.entity_type != BIPED_TYPE {
                        continue;
                    }
                    let at = |r: u32| rows.iter().find(|x| x.0 == r).map(|x| x.1);
                    let (Some(h), Some(s)) = (at(HEALTH), at(SHIELD)) else { continue };
                    let life = lives.entry(slot_of(e.index, e.generation)).or_default();
                    life.marks.push((
                        frame,
                        Some(dequantize(bits_at(c, h, 8), 8, -1.0, 1.0, true, true)),
                        Some(dequantize(bits_at(c, s, 8), 8, 0.0, 4.0, false, true)),
                    ));
                    if let Some(m) = at(MAX_VITALITIES) {
                        life.shieldless.get_or_insert(shieldless_at(c, m));
                    }
                }
            }
        }
        Marks(lives)
    }

    /// The marks the tick packets give, for a shared walk.
    pub fn reader(marks: &mut Marks) -> impl FnMut(usize, &[u8], &Payload) + '_ {
        let lives = &mut marks.0;
        move |frame, _, payload| {
            for op in &payload.ops {
                if !(512..768).contains(&op.index) || op.entity_type != Some(BIPED_TYPE) {
                    continue;
                }
                let life = lives.entry(slot_of(op.index, op.generation)).or_default();
                match &op.kind {
                    OpKind::Create { .. } => {
                        if let Some(s) = op.init.as_ref().and_then(|f| f.shieldless) {
                            life.shieldless = Some(s);
                        }
                    }
                    OpKind::Update { fields, .. } if fields.health.is_some() || fields.shield.is_some() => {
                        life.marks.push((frame as i64, fields.health, fields.shield.map(|s| s.0)));
                    }
                    _ => {}
                }
            }
        }
    }

    /// The vitals from the marks of both.
    pub fn finish(marks: Marks) -> Vitals {
        let mut lives = marks.0;
        let (mut without, mut with) = (0, 0);
        for life in lives.values_mut() {
            life.marks.sort_by_key(|m| m.0);
            let (mut health, mut shield) = (1.0, 1.0);
            for m in &mut life.marks {
                health = m.1.unwrap_or(health);
                shield = m.2.unwrap_or(shield);
                *m = (m.0, Some(health), Some(shield));
            }
            match life.shieldless {
                Some(true) => without += 1,
                Some(false) => with += 1,
                None => {}
            }
        }
        Vitals { lives, shieldless: without > with }
    }

    /// Whether most lives have no shield (Tactical).
    pub fn shieldless(&self) -> bool {
        self.shieldless
    }

    /// `slot`'s health and shield at `frame`; no shield in a mode without them.
    pub fn at(&self, slot: i32, frame: i64) -> (f64, Option<f64>) {
        let life = self.lives.get(&slot);
        let (health, shield) = life.map_or((1.0, 1.0), |l| {
            let i = l.marks.partition_point(|m| m.0 <= frame);
            if i == 0 {
                (1.0, 1.0)
            } else {
                (l.marks[i - 1].1.unwrap_or(1.0), l.marks[i - 1].2.unwrap_or(1.0))
            }
        });
        let shieldless = life.and_then(|l| l.shieldless).unwrap_or(self.shieldless);
        (f64::max(health, 0.0), (!shieldless).then_some(shield))
    }
}
