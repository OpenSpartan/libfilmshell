//! Each player's weapons' ammunition and heat, grenades and equipment energy
//! over time, from the payload walker's biped and equipment updates.
//!
//! A biped carries four weapon slots' state, as its type info carries the
//! weapons (`components::Fields::loaded`, `reserve`, `heat`): an update lists
//! only the slots that changed, so each value holds until the next. The
//! slots line up with the weapons by the readers' slot index. The grenade
//! counts are in the throws' type order: count k is grenade type k + 1.
//! Equipment is an object of its own; its energy and recharge go to the
//! player whose biped it is attached to.

use crate::components::{Fields, Parent};
use crate::payload::{OpKind, Payload, BIPED};
use crate::types::PlayerPath;
use std::collections::HashMap;

/// The equipment entity type.
const EQUIPMENT: u32 = 37;

/// One player's state changes, frames on the positions' clock; -1 for a
/// value not yet known.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerAmmo {
    /// (frame, slot, rounds loaded, rounds in reserve).
    pub ammo: Vec<(i64, u32, i32, i32)>,
    /// (frame, slot, the weapon's 0..1 fraction as a percent): energy
    /// weapons' (the Plasma Pistol, Pulse Carbine, Ravager, Stalker Rifle and
    /// Gravity Hammer send it, never a magazine).
    pub energy: Vec<(i64, u32, i32)>,
    /// (frame, slot, heat percent, flags): flags as `Fields::heat`.
    pub heat: Vec<(i64, u32, i32, u32)>,
    /// (frame, frag, plasma, dynamo, spike).
    pub grenades: Vec<(i64, [u32; 4])>,
    /// (frame, equipment energy percent, ticks before it recharges).
    pub equipment: Vec<(i64, i32, i32)>,
    /// (frame, slot, the weapon's tag; None for an empty slot).
    pub weapons: Vec<(i64, u32, Option<u32>)>,
    /// (frame, the weapon slot the biped holds).
    pub held: Vec<(i64, u32)>,
}

/// A biped's weapon state as of its last update.
#[derive(Clone, Copy, Default)]
struct Slots {
    ammo: [(i32, i32); 4],
    energy: [i32; 4],
    heat: [(i32, u32); 4],
}

/// The unarmed weapon a biped holds before it spawns.
const UNARMED: u32 = 0x0000_7ca9;

fn percent(v: f64) -> i32 {
    f64::round(v * 100.0) as i32
}

/// Push `v` when it differs from the last value pushed for the same key.
fn push_change<K: Eq + std::hash::Hash, V: PartialEq + Copy, T>(
    last: &mut HashMap<K, V>,
    key: K,
    v: V,
    out: &mut Vec<T>,
    item: T,
) {
    if last.get(&key) != Some(&v) {
        last.insert(key, v);
        out.push(item);
    }
}

pub fn reader<'a>(paths: &'a [PlayerPath], out: &'a mut Vec<PlayerAmmo>) -> impl FnMut(usize, &[u8], &Payload) + 'a {
    *out = vec![PlayerAmmo::default(); paths.len()];
    let owner: HashMap<i32, usize> =
        paths.iter().enumerate().flat_map(|(k, p)| p.life_slots.iter().flatten().map(move |&s| (s, k))).collect();
    let mut bipeds: HashMap<i32, Slots> = HashMap::new();
    // The slot of each biped index's current life, for equipment's parents (a
    // unit handle names the index only).
    let mut current: HashMap<u32, i32> = HashMap::new();
    // Equipment: its holder's slot, energy and recharge.
    let mut equipment: HashMap<u32, (Option<i32>, i32, i32)> = HashMap::new();
    let mut last_ammo = HashMap::new();
    let mut last_energy = HashMap::new();
    let mut last_heat = HashMap::new();
    let mut last_grenades = HashMap::new();
    let mut last_equipment = HashMap::new();
    let mut last_weapon = HashMap::new();
    let mut last_held = HashMap::new();
    move |frame, _, p| {
        let frame = frame as i64;
        for op in &p.ops {
            let fields: Option<&Fields> = match &op.kind {
                OpKind::Update { fields, .. } => Some(fields),
                OpKind::Create { .. } => op.init.as_deref(),
                OpKind::Delete => {
                    equipment.remove(&op.index);
                    None
                }
            };
            let Some(f) = fields else { continue };
            match op.entity_type {
                Some(BIPED) if (512..768).contains(&op.index) => {
                    let Some(slot) = op.biped_id() else { continue };
                    current.insert(op.index, slot);
                    let s = bipeds.entry(slot).or_insert(Slots {
                        ammo: [(-1, -1); 4],
                        energy: [-1; 4],
                        heat: [(-1, 0); 4],
                    });
                    for k in 0..4 {
                        if let Some((rounds, fraction)) = f.loaded[k] {
                            s.ammo[k].0 = rounds as i32;
                            s.energy[k] = fraction.map_or(-1, percent);
                        }
                        if let Some(r) = f.reserve[k] {
                            s.ammo[k].1 = r as i32;
                        }
                        if let Some((h, flags)) = f.heat[k] {
                            s.heat[k] = (percent(h), flags);
                        }
                    }
                    let Some(&who) = owner.get(&slot) else { continue };
                    let o = &mut out[who];
                    for k in 0..4 {
                        let key = (who, k);
                        let (a, e, h) = (s.ammo[k], s.energy[k], s.heat[k]);
                        if f.loaded[k].is_some() || f.reserve[k].is_some() {
                            push_change(&mut last_ammo, key, a, &mut o.ammo, (frame, k as u32, a.0, a.1));
                            if e >= 0 {
                                push_change(&mut last_energy, key, e, &mut o.energy, (frame, k as u32, e));
                            }
                        }
                        if f.heat[k].is_some() {
                            push_change(&mut last_heat, key, h, &mut o.heat, (frame, k as u32, h.0, h.1));
                        }
                    }
                    if let Some(g) = f.grenades {
                        push_change(&mut last_grenades, who, g, &mut o.grenades, (frame, g));
                    }
                    for (k, w) in f.weapons.iter().enumerate() {
                        let Some(w) = w else { continue };
                        let tag = w.map(|(tag, _)| tag).filter(|&t| t != UNARMED);
                        push_change(&mut last_weapon, (who, k), tag, &mut o.weapons, (frame, k as u32, tag));
                    }
                    // The desired weapon set's first slot is the slot held.
                    if let Some((Some(h), _)) = f.weapon_slots {
                        push_change(&mut last_held, who, h, &mut o.held, (frame, h));
                    }
                }
                Some(EQUIPMENT) => {
                    let e = equipment.entry(op.index).or_insert((None, -1, -1));
                    match f.parent {
                        Some(Some(Parent::Unit(u))) if u < 256 => {
                            e.0 = current.get(&(512 + u)).copied();
                        }
                        Some(_) => e.0 = None,
                        None => {}
                    }
                    if let Some(v) = f.energy {
                        e.1 = percent(v);
                    }
                    if let Some(t) = f.recharge {
                        e.2 = t as i32;
                    }
                    if f.energy.is_none() && f.recharge.is_none() {
                        continue;
                    }
                    let Some(&who) = e.0.and_then(|s| owner.get(&s)) else { continue };
                    if e.1 < 0 {
                        continue;
                    }
                    // The recharge counts down a tick at a time: its start is
                    // kept, and the steps that follow it are left out.
                    let counting = last_equipment.get(&who).is_some_and(|&(f, energy, ticks): &(i64, i32, i32)| {
                        let (spent, frames) = (i64::from(ticks - e.2), frame - f);
                        energy == e.1 && e.2 > 0 && (frames..=2 * frames + 2).contains(&spent)
                    });
                    if !counting && last_equipment.get(&who).map(|x| (x.1, x.2)) != Some((e.1, e.2)) {
                        last_equipment.insert(who, (frame, e.1, e.2));
                        out[who].equipment.push((frame, e.1, e.2));
                    }
                }
                _ => {}
            }
        }
    }
}
