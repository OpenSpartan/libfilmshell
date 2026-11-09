//! What players do besides moving, shooting and throwing, from the tick
//! packets' event views and the bipeds' component data.
//!
//! Events, each naming the actor's biped by its first handle (the life's
//! slot id) and, for most, the actor's player index in the body:
//! - weapon_reload (38);
//! - biped_melee_initiate (40): the second handle is the lunge target;
//! - unit_zoom (21): the zoom level, 0 to 2;
//! - weapon_empty_click (11);
//! - PowerUpApplied (100): Active Camo;
//! - teleport_effects (116): where the unit left and arrived.
//!
//! The biped's own updates:
//! - the spartan ability's state (component 57): 1 a thruster burst (24
//!   frames), 2 a sprint, 3 a grapple;
//! - the desired weapon set (42): its first slot is the slot the biped
//!   holds, so a change is a weapon switch.
//!
//! And two that aren't a player's own:
//! - projectile_detonate (5): where a projectile went off; its first
//!   handle is the projectile (kind 5: entities 1024 to 1279);
//! - PlayerGameEventSmall id 027: a headshot kill, by the player its mask
//!   names. Read in PvP modes only: the ids index a per-mode list.

use crate::events::Body;
use crate::payload::{OpKind, Payload};
use crate::types::PlayerPath;
use std::collections::HashMap;

/// Action kinds.
pub const RELOAD: u8 = 0;
pub const MELEE: u8 = 1;
pub const ZOOM: u8 = 2;
pub const EMPTY_CLICK: u8 = 3;
pub const CAMO: u8 = 4;
pub const THRUST: u8 = 5;
pub const GRAPPLE: u8 = 6;
pub const WEAPON_SET: u8 = 7;
/// The spartan ability entering its sprint state.
pub const SPRINT: u8 = 8;

/// Registry block of bipeds.
const BIPED: u32 = 35;
/// First entity of the bipeds, and of the objects kind-5 handles index.
const BIPED_BASE: u32 = 512;
const PROJECTILE_BASE: u32 = 1024;
/// The headshot-kill id of PlayerGameEventSmall.
const HEADSHOT: u32 = 0x027;
/// Spartan-ability states that are actions.
const ABILITY_THRUST: u32 = 1;
const ABILITY_SPRINT: u32 = 2;
const ABILITY_GRAPPLE: u32 = 3;

#[derive(Debug, Default)]
pub struct FilmActions {
    /// Per path (parallel to the paths given to [`read`]): frame, kind and
    /// argument (-1: none), in frame order.
    pub actions: Vec<Vec<(i64, u8, i32)>>,
    /// Teleports: frame, path, and the level codes left and arrived at.
    pub teleports: Vec<(i64, usize, [u32; 3], [u32; 3])>,
    /// Detonations: frame, the projectile (entity, generation) when the
    /// event names it, and the level codes.
    pub blasts: Vec<(i64, Option<(u32, u32)>, [u32; 3])>,
    /// Headshot kills: frame and player index.
    pub headshots: Vec<(i64, i32)>,
}

/// The actions of `film`, attributed to `paths` the way `fire.rs` does: the
/// life the event's biped handle names, else the body's player index.
/// Headshots only when `pvp`. Packet framing only.
pub fn reader<'a>(
    paths: &'a [PlayerPath],
    pvp: bool,
    out: &'a mut FilmActions,
) -> impl FnMut(usize, &[u8], &Payload) + 'a {
    let mut slot_owner: HashMap<i32, usize> = HashMap::new();
    let mut by_index: HashMap<i32, usize> = HashMap::new();
    for (k, p) in paths.iter().enumerate() {
        by_index.insert(p.player_index, k);
        for &s in p.life_slots.iter().flatten() {
            slot_owner.insert(s, k);
        }
    }
    *out = FilmActions { actions: vec![Vec::new(); paths.len()], ..Default::default() };
    // Per life: the last spartan-ability state and weapon slot seen.
    let mut ability: HashMap<i32, u32> = HashMap::new();
    let mut weapon_slot: HashMap<i32, u32> = HashMap::new();
    move |frame, _, p| {
        let frame = frame as i64;
        for e in &p.events {
            // A kind-2 handle (an 8-bit biped index), else a unit handle.
            let life = e.handles[0].and_then(|h| match e.kind {
                11 | 38 | 40 => (h.generation > 0).then(|| (h.generation as i32 - 1) * 256 + h.index as i32),
                _ => h.biped_slot(),
            });
            let owner = |player: Option<u32>| {
                life.and_then(|s| slot_owner.get(&s))
                    .or_else(|| player.and_then(|p| by_index.get(&(p as i32))))
                    .copied()
            };
            let (k, kind, arg) = match e.body {
                Body::Reload { player } => (owner(player), RELOAD, -1),
                Body::Melee { player } => {
                    let target = e.handles[1].and_then(|h| h.biped_slot()).and_then(|s| slot_owner.get(&s));
                    (owner(player), MELEE, target.map_or(-1, |&t| paths[t].player_index))
                }
                Body::Zoom { level } => (owner(None), ZOOM, level as i32),
                Body::EmptyClick { player } => (owner(player), EMPTY_CLICK, -1),
                Body::PowerUp { .. } => (owner(None), CAMO, -1),
                Body::Teleport { from: Some(a), to: Some(b) } => {
                    if let Some(k) = owner(None) {
                        out.teleports.push((frame, k, a, b));
                    }
                    continue;
                }
                Body::Detonate { at: Some(q), .. } => {
                    let projectile = e.handles[0].map(|h| (PROJECTILE_BASE + h.index, h.generation));
                    out.blasts.push((frame, projectile, q));
                    continue;
                }
                Body::Game(g) if pvp && g.id == HEADSHOT => {
                    for pi in (0..32).filter(|p| g.last & (0x8000_0000u32 >> p) != 0) {
                        if by_index.contains_key(&pi) {
                            out.headshots.push((frame, pi));
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            if let Some(k) = k {
                out.actions[k].push((frame, kind, arg));
            }
        }
        for op in &p.ops {
            let (Some(BIPED), OpKind::Update { fields, .. }) = (op.entity_type, &op.kind) else { continue };
            if op.generation == 0 || !(BIPED_BASE..BIPED_BASE + 256).contains(&op.index) {
                continue;
            }
            let slot = (op.generation as i32 - 1) * 256 + (op.index - BIPED_BASE) as i32;
            let Some(&k) = slot_owner.get(&slot) else { continue };
            if let Some(s) = fields.ability {
                if ability.insert(slot, s) != Some(s) {
                    match s {
                        ABILITY_THRUST => out.actions[k].push((frame, THRUST, -1)),
                        ABILITY_GRAPPLE => out.actions[k].push((frame, GRAPPLE, -1)),
                        ABILITY_SPRINT => out.actions[k].push((frame, SPRINT, -1)),
                        _ => {}
                    }
                }
            }
            // The desired weapon set's first slot: the weapon slot the biped
            // holds.
            if let Some((Some(v), _)) = fields.weapon_slots {
                if weapon_slot.insert(slot, v) != Some(v) {
                    out.actions[k].push((frame, WEAPON_SET, v as i32));
                }
            }
        }
    }
}
