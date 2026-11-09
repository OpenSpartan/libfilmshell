//! What a running accuracy is estimated from. The scoreboard's own Shots Hit
//! and Accuracy slots hold nothing in the film, so each player's shots and
//! hits are counted from what the film does hold.
//!
//! A weapon-fire event (36, `fire.rs`) covers only some shots. The others are
//! projectile objects (block 41), whose creation data names the shooter, and
//! the two don't overlap.
//!
//! A hit is a `damage_aftermath` whose damage belongs to a weapon, once per
//! attacker, victim and frame.

use crate::components::Params;
use crate::container::Film;
use crate::damage::{self, Cause};
use crate::events::Body;
use crate::payload::{for_each_payload, OpKind, Walker};
use crate::types::PlayerPath;
use crate::weapons;
use std::collections::{HashMap, HashSet};

/// Registry block of projectile objects.
const PROJECTILE: u32 = 41;

/// Per path (parallel to the paths given to [`read`]), in frame order.
#[derive(Debug, Default)]
pub struct Aim {
    /// The frames of the player's projectiles, grenades left out.
    pub projectiles: Vec<Vec<i64>>,
    /// The frames of their hits.
    pub hits: Vec<Vec<i64>>,
}

pub fn read(film: &Film, params: Params, paths: &[PlayerPath]) -> Aim {
    let mut slot_owner: HashMap<i32, usize> = HashMap::new();
    let mut by_index: HashMap<i32, usize> = HashMap::new();
    for (k, p) in paths.iter().enumerate() {
        by_index.insert(p.player_index, k);
        for &s in p.life_slots.iter().flatten() {
            slot_owner.insert(s, k);
        }
    }
    let mut aim = Aim { projectiles: vec![Vec::new(); paths.len()], hits: vec![Vec::new(); paths.len()] };
    let mut grenade: HashMap<u32, bool> = HashMap::new();
    let mut seen = HashSet::new();
    let mut walker = Walker::new(&film.chunks[0], params);
    for_each_payload(film, &mut walker, |frame, p| {
        let frame = frame as i64;
        for op in p.ops.iter().filter(|op| op.entity_type == Some(PROJECTILE)) {
            let OpKind::Create { tag } = op.kind else { continue };
            let is_grenade = tag.is_some_and(|t| {
                *grenade.entry(t).or_insert_with(|| weapons::weapon(t).name.is_some_and(|n| n.contains("Grenade")))
            });
            if is_grenade {
                continue;
            }
            let (player, unit) = op.shooter;
            let owner = unit
                .and_then(|h| h.biped_slot())
                .and_then(|s| slot_owner.get(&s))
                .or_else(|| player.and_then(|i| by_index.get(&(i as i32))));
            if let Some(&k) = owner {
                aim.projectiles[k].push(frame);
            }
        }
        for e in &p.events {
            let Body::Damage(d) = &e.body else { continue };
            if !d.tag.is_some_and(|t| damage::cause(t).0 == Cause::Weapon) {
                continue;
            }
            let owner = e.handles[1]
                .and_then(|h| h.biped_slot())
                .and_then(|s| slot_owner.get(&s))
                .or_else(|| d.player.and_then(|i| by_index.get(&(i as i32))));
            let Some(&k) = owner else { continue };
            if seen.insert((k, e.handles[0].map(|h| h.index), frame)) {
                aim.hits[k].push(frame);
            }
        }
    });
    aim
}
