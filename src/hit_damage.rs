//! How much each hit took off its victim's shield and body: the damage tag's
//! numbers (`damage::amount`) against the victim's shield and health just
//! before it (`vitals.rs`).
//!
//! A direct hit whose effect has one amount is computed as the game does it:
//! the amount times the shield armor's multiplier comes off the shield; what
//! the shield can't hold spills to the body at the body's multiplier, unless
//! the effect says it doesn't spill; on no shield the body takes it all. A
//! hit can't take more than is left.
//!
//! The body's first 5-bit field scales the hit: 15 on almost every direct
//! hit, lower with distance from a blast, 19 on some hits that take 1.2
//! times the amount off a shield. What it encodes isn't traced, so a hit
//! with any other value, an area or range-falloff effect (whose scale `s`
//! isn't traced either) or an effect the catalogue lacks is measured
//! instead: the victim's shield and health readings across the hit's frame,
//! shared between that frame's hits. Those are 8-bit fractions, so a
//! measured hit is good to about 2 points of shield and 1 of health, and is
//! marked as measured.
//!
//! Headshots aren't in the event. A kill by an effect that can headshot that
//! takes more health than the amount explains took what was left: the
//! game's headshot path finished the victim.

use crate::damage::{self, CAN_HEADSHOT, NO_SPILL, SKIPS_SHIELDS};
use crate::fire::FilmFire;
use crate::vitals::Vitals;

/// The scale field's value on a full direct hit.
const FULL: u32 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitDamage {
    pub shield: f64,
    pub body: f64,
    /// From the vitals readings rather than the tag: approximate.
    pub measured: bool,
}

impl HitDamage {}

/// What `amount` takes off a Spartan with `shield` and `body` points left.
pub fn apply(a: &damage::Amount, d: f64, shield: f64, body: f64) -> (f64, f64) {
    let (s, b) = if shield > 0.0 && a.flags & SKIPS_SHIELDS == 0 {
        let on_shield = d * a.shield_scale;
        if on_shield >= shield {
            let rest = if a.flags & NO_SPILL != 0 { 0.0 } else { d * (1.0 - shield / on_shield) * a.body_scale };
            (shield, rest)
        } else {
            (on_shield, 0.0)
        }
    } else {
        (0.0, d * a.body_scale)
    };
    (s, b.min(body.max(0.0)))
}

/// Each of `ff.hits`' damage, in their order.
pub fn read(ff: &FilmFire, vitals: &Vitals) -> Vec<HitDamage> {
    let (body_max, shield_max) = damage::spartan();
    let mut out = vec![HitDamage::default(); ff.hits.len()];
    // The hits on one life in one frame share its readings.
    let mut order: Vec<usize> = (0..ff.hits.len()).collect();
    order.sort_by_key(|&i| (ff.hits[i].0, ff.hit_details[i].0));
    for group in order.chunk_by(|&a, &b| (ff.hits[a].0, ff.hit_details[a].0) == (ff.hits[b].0, ff.hit_details[b].0)) {
        let (frame, slot) = (ff.hits[group[0]].0, ff.hit_details[group[0]].0);
        let (h0, s0) = vitals.at(slot, frame - 1);
        let (h1, s1) = vitals.at(slot, frame);
        let (mut shield, mut body) = (s0.unwrap_or(0.0) * shield_max, h0 * body_max);
        let mut seen =
            (((s0.unwrap_or(0.0) - s1.unwrap_or(0.0)) * shield_max).max(0.0), ((h0 - h1) * body_max).max(0.0));
        let mut measured = Vec::new();
        for &i in group {
            let (_, tag, scale) = ff.hit_details[i];
            let hit = match tag.and_then(damage::amount) {
                Some(a) if a.is_fixed() && scale == FULL => {
                    let (s, mut b) = apply(&a, a.lower, shield, body);
                    if h1 <= 0.0 && a.flags & CAN_HEADSHOT != 0 && seen.1 > b + 1.0 && shield - s < 1.0 {
                        b = body;
                    }
                    HitDamage { shield: s, body: b, measured: false }
                }
                // A known effect that deals nothing.
                None if tag.is_some_and(damage::known) => HitDamage::default(),
                _ => {
                    measured.push(i);
                    continue;
                }
            };
            shield -= hit.shield;
            body -= hit.body;
            seen = ((seen.0 - hit.shield).max(0.0), (seen.1 - hit.body).max(0.0));
            out[i] = hit;
        }
        // What the readings show beyond the computed hits, shared.
        let n = measured.len() as f64;
        for i in measured {
            out[i] = HitDamage {
                shield: (seen.0 / n).min(shield.max(0.0)),
                body: (seen.1 / n).min(body.max(0.0)),
                measured: true,
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spill() {
        let br = damage::amount(0x0000b29c).unwrap();
        // The BR75 doesn't spill: the shield's last 5 points stop a round.
        assert_eq!(apply(&br, br.lower, 5.0, 90.0), (5.0, 0.0));
        assert_eq!(apply(&br, br.lower, 0.0, 90.0), (0.0, 12.73));
        // The Ravager's blast: 100 on shields, half that on the body.
        let rv = damage::amount(0xba5ac78e).unwrap();
        let (s, b) = apply(&rv, 100.0, 50.0, 90.0);
        assert_eq!(s, 50.0);
        assert!((b - 25.0).abs() < 1e-9);
    }
}
