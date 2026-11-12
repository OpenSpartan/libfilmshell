use super::Tick;
use crate::TagId;
use crate::game::{Grenade, Weapon};

/// An objective a player carries. The game puts an item for it in the
/// carrier's weapon slots, where it stays whichever weapon they hold: that
/// item is how a carrier is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Carried {
    /// A Capture the Flag flag. Its item is a pistol of the flag's own
    /// (`magnum_pistol_ctf`), which the game gives the carrier on the grab.
    Flag,
    /// An Oddball skull.
    Skull,
    /// Grifball's ball.
    Ball,
    /// A Stockpile power seed.
    PowerSeed,
}

impl Carried {
    /// The flag carrier's pistol tags.
    const FLAG_PISTOLS: [TagId; 2] = [TagId(0x2a392328), TagId(0x0001_6415)];

    /// The objective a weapon slot's item stands for, if any.
    pub fn from_item(tag: TagId) -> Option<Carried> {
        if Self::FLAG_PISTOLS.contains(&tag) {
            return Some(Carried::Flag);
        }
        match Weapon::from_tag(tag)? {
            Weapon::GenericSkull | Weapon::GenericSkullEscalation => Some(Carried::Skull),
            Weapon::GenericBall => Some(Carried::Ball),
            Weapon::GenericPowerSeed => Some(Carried::PowerSeed),
            _ => None,
        }
    }
}

/// A player's movement input from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Input {
    /// From when.
    pub tick: Tick,
    /// Forward throttle, -31 (back) to 31 (forward).
    pub forward: i8,
    /// Sideways throttle, -31 (right) to 31 (left).
    pub left: i8,
    /// The control record's 5-bit stance field, when sent. Bit 4 (0x10) is
    /// the jump button: on the test films, 89% to 98% of the jumps the
    /// biped's vertical speed shows have it set within 3 frames. The other
    /// bits' meanings aren't known.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub stance: Option<u8>,
}

/// A weapon slot's rounds from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Ammo {
    /// From when.
    pub tick: Tick,
    /// The weapon slot, 0 to 3.
    pub slot: u32,
    /// Rounds in the magazine, 0 for a weapon that has none (an energy
    /// weapon's charge is its [`Energy`]); None until the game sends them.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub loaded: Option<u32>,
    /// Rounds in reserve; None until the game sends them.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub reserve: Option<u32>,
}

/// An energy weapon's charge from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Energy {
    /// From when.
    pub tick: Tick,
    /// The weapon slot.
    pub slot: u32,
    /// The charge, percent.
    pub percent: i32,
}

/// A weapon's heat from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Heat {
    /// From when.
    pub tick: Tick,
    /// The weapon slot.
    pub slot: u32,
    /// The heat, percent.
    pub percent: i32,
    /// The heat field's flags as the game sends them; they include
    /// overheated and venting, but which bit is which isn't known.
    pub flags: u32,
}

/// The grenades a player carries from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct GrenadeCount {
    /// From when.
    pub tick: Tick,
    /// Frag, plasma, dynamo and spike grenades, in that order (measured on
    /// the test films).
    pub counts: [u32; 4],
}

impl GrenadeCount {
    /// How many of `grenade` the player carries.
    pub fn count(&self, grenade: Grenade) -> u32 {
        self.counts.get(grenade.type_index() as usize - 1).copied().unwrap_or(0)
    }
}

/// A player's equipment charge from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct EquipmentCharge {
    /// From when.
    pub tick: Tick,
    /// The charge, percent.
    pub percent: i32,
    /// Ticks before it starts to recharge.
    pub recharge_delay: i32,
}

/// The weapon in one of a player's weapon slots from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct WeaponSlot {
    /// From when.
    pub tick: Tick,
    /// The weapon slot, 0 to 3.
    pub slot: u32,
    /// The weapon; None when the slot is empty.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub weapon: Option<TagId>,
}

/// The weapon slot a player holds from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct HeldSlot {
    /// From when.
    pub tick: Tick,
    /// The weapon slot, 0 to 3.
    pub slot: u32,
}

/// The last entry of a series at or before `tick`.
fn latest<T>(series: &[T], tick: Tick, at: impl Fn(&T) -> Tick) -> Option<&T> {
    series.partition_point(|x| at(x) <= tick).checked_sub(1).map(|i| &series[i])
}

/// The last entry for `slot` of a series of all slots' changes at or before
/// `tick`.
fn latest_in<T>(series: &[T], tick: Tick, at: impl Fn(&T) -> (Tick, u32), slot: u32) -> Option<&T> {
    series[..series.partition_point(|x| at(x).0 <= tick)].iter().rfind(|x| at(x).1 == slot)
}

/// What a player carried and pressed over the match, each series at its
/// changes.
///
/// The series run across lives and say nothing about whether the player is
/// alive: [`Film::weapon_at`](crate::Film::weapon_at) combines them with the
/// player's lives.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PlayerState {
    /// Movement input.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub inputs: Vec<Input>,
    /// Weapon magazines and reserves.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub ammo: Vec<Ammo>,
    /// Energy weapons' charge.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub energy: Vec<Energy>,
    /// Weapon heat.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub heat: Vec<Heat>,
    /// Grenades carried.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub grenades: Vec<GrenadeCount>,
    /// Equipment charge.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub equipment: Vec<EquipmentCharge>,
    /// The weapons in the weapon slots.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub weapons: Vec<WeaponSlot>,
    /// The weapon slot held.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub held: Vec<HeldSlot>,
}

impl PlayerState {
    /// The weapon in `slot` at `tick`.
    pub fn weapon_in(&self, slot: u32, tick: Tick) -> Option<TagId> {
        latest_in(&self.weapons, tick, |w| (w.tick, w.slot), slot)?.weapon
    }

    /// The weapon slot held at `tick`.
    pub fn held_slot(&self, tick: Tick) -> Option<u32> {
        latest(&self.held, tick, |h| h.tick).map(|h| h.slot)
    }

    /// The weapon in the held slot at `tick`.
    pub fn weapon_at(&self, tick: Tick) -> Option<TagId> {
        self.weapon_in(self.held_slot(tick)?, tick)
    }

    /// The objective carried at `tick`: the one whose item is in a weapon
    /// slot ([`Carried`]).
    pub fn carrying(&self, tick: Tick) -> Option<Carried> {
        (0..4).find_map(|slot| self.weapon_in(slot, tick).and_then(Carried::from_item))
    }

    /// The magazine and reserve of `slot` at `tick`.
    pub fn ammo_in(&self, slot: u32, tick: Tick) -> Option<&Ammo> {
        latest_in(&self.ammo, tick, |a| (a.tick, a.slot), slot)
    }

    /// The grenades carried at `tick`.
    pub fn grenades_at(&self, tick: Tick) -> Option<&GrenadeCount> {
        latest(&self.grenades, tick, |g| g.tick)
    }

    /// The movement input at `tick`.
    pub fn input_at(&self, tick: Tick) -> Option<&Input> {
        latest(&self.inputs, tick, |i| i.tick)
    }

    /// The charge of the energy weapon in `slot` at `tick`.
    pub fn energy_in(&self, slot: u32, tick: Tick) -> Option<&Energy> {
        latest_in(&self.energy, tick, |e| (e.tick, e.slot), slot)
    }

    /// The heat of the weapon in `slot` at `tick`.
    pub fn heat_in(&self, slot: u32, tick: Tick) -> Option<&Heat> {
        latest_in(&self.heat, tick, |h| (h.tick, h.slot), slot)
    }

    /// The equipment's charge at `tick`.
    pub fn equipment_at(&self, tick: Tick) -> Option<&EquipmentCharge> {
        latest(&self.equipment, tick, |e| e.tick)
    }
}
