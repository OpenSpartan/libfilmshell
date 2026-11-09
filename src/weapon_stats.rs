//! Which weapon dealt each hit, and each weapon's numbers in the film: how
//! it fires and what its hits deal, from the projectile and damage effect
//! its hits actually used, so a variant that swaps them shows its own.

use crate::damage::{self, Cause};
use crate::fire::FilmFire;
use crate::hit_damage::apply;
use crate::sandbox;
use std::collections::HashMap;

/// Frames before a hit in which the attacker's last shot names its weapon
/// when the damage tag doesn't: a projectile's flight.
const SHOT_FRAMES: i64 = 120;
/// A shot this many frames after the hit still counts (the event order in a
/// packet).
const SHOT_LATE_FRAMES: i64 = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponStats {
    pub rounds_per_second: f64,
    pub burst: i32,
    /// Rounds loaded, carried at the start, carried at most.
    pub magazine: Option<[i32; 3]>,
    /// The projectile's initial velocity and maximum range, engine units.
    pub velocity: Option<f64>,
    pub range: Option<f64>,
    /// A hit's damage on a shield and on a body, when it is one amount.
    pub damage: Option<[f64; 2]>,
    /// Body hits that down a Spartan at full shield and health (no shield in
    /// a mode without them), without headshots.
    pub hits_to_kill: Option<u32>,
}

/// The index into `ff.weapons` of the weapon behind each of `ff.hits`: the
/// one the damage tag belongs to, else, for a projectile with no weapon on
/// record, the attacker's last shot in the 2 s before. A melee swing, a
/// grenade the list lacks, a vehicle or the world has none.
pub fn hit_weapons(ff: &FilmFire) -> Vec<Option<usize>> {
    ff.hits
        .iter()
        .zip(&ff.hit_details)
        .map(|(&(frame, attacker, _), &(_, tag, _))| {
            let (cause, owner) = tag.map_or((Cause::Unknown, None), damage::cause);
            if let Some(w) = owner.and_then(|o| ff.index_of(o).or_else(|| ff.named(o))) {
                return Some(w);
            }
            if !matches!(cause, Cause::Weapon | Cause::Unknown) {
                return None;
            }
            ff.shots
                .get(attacker?)?
                .iter()
                .rev()
                .find(|s| s.0 <= frame + SHOT_LATE_FRAMES)
                .filter(|s| s.0 >= frame - SHOT_FRAMES)
                .map(|s| s.1)
        })
        .collect()
}

/// Hits of `a` at `d` that down a Spartan with `shield` and `body`.
fn hits_to_kill(a: &damage::Amount, d: f64, mut shield: f64, mut body: f64) -> Option<u32> {
    for n in 1..=200 {
        let (s, b) = apply(a, d, shield, body);
        shield -= s;
        body -= b;
        if body <= 1e-9 {
            return Some(n);
        }
    }
    None
}

/// Each of `ff.weapons`' numbers (None for one the sandbox catalogue lacks).
pub fn stats(ff: &FilmFire, hit_weapons: &[Option<usize>], shieldless: bool) -> Vec<Option<WeaponStats>> {
    // The damage tags each weapon's hits used, most common first.
    let mut used: HashMap<usize, HashMap<u32, usize>> = HashMap::new();
    for (w, d) in hit_weapons.iter().zip(&ff.hit_details) {
        if let (Some(w), Some(t)) = (w, d.1) {
            if damage::amount(t).is_some() {
                *used.entry(*w).or_default().entry(t).or_default() += 1;
            }
        }
    }
    let (body, shield) = damage::spartan();
    let shield = if shieldless { 0.0 } else { shield };
    ff.weapons
        .iter()
        .enumerate()
        .map(|(k, w)| {
            let row = sandbox::weapon(w.id)?;
            let row_projectile = row.projectile.map(|p| p.0);
            let tag =
                used.get(&k).and_then(|m| m.iter().max_by_key(|(t, n)| (**n, std::cmp::Reverse(**t))).map(|x| *x.0));
            // The film's projectile when its hits name one the catalogue has.
            let proj = tag
                .and_then(|t| damage::projectiles(t).into_iter().find(|&p| sandbox::projectile(p).is_some()))
                .or(row_projectile)
                .and_then(sandbox::projectile);
            let amount =
                tag.or(proj.and_then(|p| p.impact.map(|i| i.0))).and_then(damage::amount).filter(|a| a.fixed());
            Some(WeaponStats {
                rounds_per_second: row.rounds_per_second,
                burst: row.burst,
                magazine: row.magazine,
                velocity: proj.map(|p| p.velocity),
                range: proj.map(|p| p.range),
                damage: amount.map(|a| [a.lower * a.shield_scale, a.lower * a.body_scale]),
                hits_to_kill: amount.and_then(|a| hits_to_kill(&a, a.lower, shield, body)),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_kill() {
        // 11 rounds take the shield (the 11th doesn't spill), 8 the body.
        let br = damage::amount(0x0000b29c).unwrap();
        assert_eq!(hits_to_kill(&br, br.lower, 140.0, 90.0), Some(19));
        assert_eq!(hits_to_kill(&br, br.lower, 0.0, 90.0), Some(8));
    }
}
