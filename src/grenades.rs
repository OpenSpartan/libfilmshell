//! Thrown grenades as objects: their flight and where they went off.
//!
//! A throw is a `biped_throw_initiate` event (39): the thrower's life (first
//! handle) and the grenade type. The grenade is then a projectile object
//! (registry block 41) created within a few frames, whose updates list its
//! position nearly every frame until it is deleted when it detonates. This
//! module collects every projectile created within `WINDOW` frames after a
//! throw, with its
//! positions; `assemble` pairs each throw with the one whose first position
//! is nearest the thrower, since bullets and other projectiles are created
//! in the same window.

use crate::events::Body;
use crate::payload::{OpKind, Payload};
use std::collections::HashMap;

/// Registry block of projectile objects.
const PROJECTILE: u32 = 41;
/// Frames after a throw in which its grenade is created.
pub const WINDOW: i64 = 20;

/// One throw: its frame, the thrower's life slot id and the grenade type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Throw {
    pub frame: i64,
    pub slot_id: Option<i32>,
    pub grenade: Option<u32>,
}

/// A projectile created within `WINDOW` frames of a throw: its tag, its
/// positions (frame, level codes) and the frame it was deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projectile {
    /// The object's entity index and generation (a projectile_detonate's
    /// first handle names them: `actions.rs`).
    pub entity: u32,
    pub generation: u32,
    pub tag: u32,
    pub from: i64,
    /// (frame, box index, position codes in that box).
    pub path: Vec<(i64, u32, [u32; 3])>,
    pub to: Option<i64>,
}

#[derive(Debug, Default)]
pub struct Flights {
    pub throws: Vec<Throw>,
    pub projectiles: Vec<Projectile>,
}

/// The throws of `film` and the projectiles created just after them.
/// Packet framing only.
pub fn reader(out: &mut Flights) -> impl FnMut(usize, &[u8], &Payload) + '_ {
    let mut live: HashMap<u32, usize> = HashMap::new();
    move |frame, _, p| {
        let frame = frame as i64;
        for e in &p.events {
            if let Body::Throw { grenade, .. } = e.body {
                let slot_id =
                    e.handles[0].filter(|h| h.generation > 0).map(|h| (h.generation as i32 - 1) * 256 + h.index as i32);
                out.throws.push(Throw { frame, slot_id, grenade });
            }
        }
        for op in p.ops.iter().filter(|op| op.entity_type == Some(PROJECTILE)) {
            match &op.kind {
                OpKind::Create { tag } => {
                    live.remove(&op.index);
                    let after =
                        out.throws.iter().rev().take_while(|t| frame - t.frame <= WINDOW).any(|t| frame >= t.frame);
                    if after {
                        live.insert(op.index, out.projectiles.len());
                        out.projectiles.push(Projectile {
                            entity: op.index,
                            generation: op.generation,
                            tag: tag.unwrap_or(0),
                            from: frame,
                            path: Vec::new(),
                            to: None,
                        });
                    }
                }
                OpKind::Update { fields, .. } => {
                    if let (Some(&k), Some((bx, q))) = (live.get(&op.index), fields.position) {
                        out.projectiles[k].path.push((frame, bx, q));
                    }
                }
                OpKind::Delete => {
                    if let Some(k) = live.remove(&op.index) {
                        out.projectiles[k].to = Some(frame);
                    }
                }
            }
        }
    }
}
