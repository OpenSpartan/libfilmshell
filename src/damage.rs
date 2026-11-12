//! Damage effects by raw tag id, for the readers.

use crate::game::{self, DamageCause};
use crate::TagId;

pub use crate::game::DamageAmount as Amount;
pub use crate::game::DamageCause as Cause;

/// The damage numbers of a damage tag; None for an effect that deals none or
/// that isn't known.
pub fn amount(tag: u32) -> Option<Amount> {
    game::damage_effect(TagId(tag)).and_then(|d| d.amount)
}

/// Whether the catalogue has a damage tag.
pub fn known(tag: u32) -> bool {
    game::damage_effect(TagId(tag)).is_some()
}

/// The projectiles that deal a damage tag.
pub fn projectiles(tag: u32) -> Vec<u32> {
    game::damage_effect(TagId(tag)).map_or(Vec::new(), |d| d.projectiles().map(|p| p.0).collect())
}

/// The multiplayer Spartan's body and shield vitality.
pub fn spartan() -> (f64, f64) {
    let v = game::spartan_vitality();
    (v.body, v.shield)
}

/// Whether a damage tag is a blast that grenades spawn among other
/// projectiles.
pub fn grenade_blast(tag: u32) -> bool {
    game::damage_effect(TagId(tag)).is_some_and(|d| d.grenade_blast)
}

/// What a damage tag belongs to, and the weapon that owns it when it's one.
pub fn cause(tag: u32) -> (Cause, Option<u32>) {
    game::damage_effect(TagId(tag)).map_or((DamageCause::Unknown, None), |d| (d.cause, d.weapon.map(|w| w.0)))
}
