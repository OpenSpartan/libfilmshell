//! Damage effects: what each one belongs to and how much it deals.

use super::damage_data::{DAMAGE, SPARTAN};
use crate::TagId;

/// What dealt a hit, from where its damage effect sits in the game's tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum DamageCause {
    /// A weapon's projectile, or a projectile with no weapon on record.
    Weapon,
    /// A melee strike, a sword or hammer swing included.
    Melee,
    /// A grenade.
    Grenade,
    /// Equipment.
    Equipment,
    /// A vehicle.
    Vehicle,
    /// A physics model: something ran into something.
    Collision,
    /// The game's globals: falling, kill volumes and the like.
    World,
    /// A model blowing up.
    Explosion,
    /// Not in the catalogue.
    Unknown,
}

impl std::fmt::Display for DamageCause {
    /// A short lowercase description: "a grenade", "the environment".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DamageCause::Weapon => "a weapon",
            DamageCause::Melee => "melee",
            DamageCause::Grenade => "a grenade",
            DamageCause::Equipment => "equipment",
            DamageCause::Vehicle => "a vehicle",
            DamageCause::Collision => "a collision",
            DamageCause::World => "the environment",
            DamageCause::Explosion => "an explosion",
            DamageCause::Unknown => "an unknown cause",
        })
    }
}

/// A damage effect's numbers, from its tag and from what the multiplayer
/// Spartan's shield and body armor multiply it by.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct DamageAmount {
    /// The lower bound.
    pub lower: f64,
    /// The upper bound's minimum and maximum.
    pub upper: [f64; 2],
    /// The Spartan's shield multiplier.
    pub shield_scale: f64,
    /// The Spartan's body multiplier.
    pub body_scale: f64,
    /// The effect's `DamageDefinitionFlags`.
    pub flags: u32,
    /// The area's outer radius in engine units; 0 for a direct hit.
    pub radius: f64,
}

impl DamageAmount {
    /// The flag set on an effect that can headshot.
    pub const CAN_HEADSHOT: u32 = 1 << 1;
    /// The flag set on an effect that goes straight to the body.
    pub const SKIPS_SHIELDS: u32 = 1 << 9;
    /// The flag set on an effect whose damage beyond the shield is lost.
    pub const NO_SPILL: u32 = 1 << 23;

    /// One amount whatever the distance or the roll: a direct hit with
    /// equal bounds.
    pub fn is_fixed(&self) -> bool {
        self.radius == 0.0 && self.lower == self.upper[0] && self.lower == self.upper[1]
    }

    /// Whether `flag` is set.
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}

/// One catalogued damage effect.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct DamageEffect {
    /// The effect's tag.
    pub tag: TagId,
    /// What it belongs to.
    pub cause: DamageCause,
    /// The weapon (or grenade) that deals it, when exactly one does.
    pub weapon: Option<TagId>,
    /// Its numbers; None for an effect that deals no damage.
    pub amount: Option<DamageAmount>,
    /// A blast that grenades spawn among other projectiles, so that the
    /// effect alone doesn't say what set it off.
    pub grenade_blast: bool,
    projectiles: &'static [u32],
}

impl DamageEffect {
    /// The projectiles that deal it.
    pub fn projectiles(&self) -> impl Iterator<Item = TagId> + '_ {
        self.projectiles.iter().map(|&p| TagId(p))
    }
}

pub(super) type DamageRow = DamageEffect;

pub(super) const fn amount(
    lower: f64,
    upper: [f64; 2],
    shield_scale: f64,
    body_scale: f64,
    flags: u32,
    radius: f64,
) -> DamageAmount {
    DamageAmount { lower, upper, shield_scale, body_scale, flags, radius }
}

pub(super) const fn row(
    tag: u32,
    cause: DamageCause,
    weapon: u32,
    amount: Option<DamageAmount>,
    grenade_blast: bool,
    projectiles: &'static [u32],
) -> DamageEffect {
    DamageEffect {
        tag: TagId(tag),
        cause,
        weapon: if weapon == 0 { None } else { Some(TagId(weapon)) },
        amount,
        grenade_blast,
        projectiles,
    }
}

/// The damage effect `tag`, when the catalogue lists it.
pub fn damage_effect(tag: TagId) -> Option<&'static DamageEffect> {
    DAMAGE.binary_search_by_key(&tag, |d| d.tag).ok().map(|i| &DAMAGE[i])
}

/// The multiplayer Spartan's body and shield vitality, from its model's
/// damage info.
pub fn spartan_vitality() -> (f64, f64) {
    SPARTAN
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Weapon;

    #[test]
    fn known_effects() {
        assert!(DAMAGE.windows(2).all(|w| w[0].tag < w[1].tag));
        let br = damage_effect(TagId(0x0000b29c)).unwrap();
        assert_eq!(br.cause, DamageCause::Weapon);
        assert_eq!(br.weapon.and_then(Weapon::from_tag), Some(Weapon::Br75BattleRifle));
        let a = br.amount.unwrap();
        assert!(a.is_fixed() && a.lower == 12.73 && a.shield_scale == 1.0 && a.has(DamageAmount::NO_SPILL));
        assert_eq!(damage_effect(TagId(0xd9dbf765)).map(|d| (d.cause, d.amount)), Some((DamageCause::Collision, None)));
        assert_eq!(damage_effect(TagId(0x00403594)).map(|d| d.cause), Some(DamageCause::World));
        let hammer = damage_effect(TagId(0x877da252)).unwrap();
        assert_eq!((hammer.cause, hammer.weapon), (DamageCause::Melee, Some(TagId(0x00007ee6))));
        assert_eq!(damage_effect(TagId(0xdaa03c35)).map(|d| (d.cause, d.weapon)), Some((DamageCause::Melee, None)));
        assert!(damage_effect(TagId(0xd21ac495)).unwrap().grenade_blast);
        assert_eq!(damage_effect(TagId(0xda3b5ba4)).map(|d| d.cause), Some(DamageCause::Grenade));
        assert_eq!(spartan_vitality(), (90.0, 140.0));
    }
}
