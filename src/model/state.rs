use super::Tick;
use crate::TagId;

/// A player's movement input from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Input {
    /// From when.
    pub tick: Tick,
    /// Forward throttle, -31 (back) to 31 (forward).
    pub forward: i8,
    /// Sideways throttle, -31 (right) to 31 (left).
    pub left: i8,
    /// The control record's 5-bit stance field, when sent.
    pub stance: Option<u8>,
}

/// A weapon slot's rounds from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Ammo {
    /// From when.
    pub tick: Tick,
    /// The weapon slot, 0 to 3.
    pub slot: u32,
    /// Rounds in the magazine.
    pub loaded: i32,
    /// Rounds in reserve.
    pub reserve: i32,
}

/// An energy weapon's charge from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Heat {
    /// From when.
    pub tick: Tick,
    /// The weapon slot.
    pub slot: u32,
    /// The heat, percent.
    pub percent: i32,
    /// The heat field's flags (overheated, venting).
    pub flags: u32,
}

/// The grenades a player carries from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct GrenadeCount {
    /// From when.
    pub tick: Tick,
    /// Frag, plasma, dynamo and spike grenades.
    pub counts: [u32; 4],
}

/// A player's equipment charge from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct WeaponSlot {
    /// From when.
    pub tick: Tick,
    /// The weapon slot, 0 to 3.
    pub slot: u32,
    /// The weapon; None when the slot is empty.
    pub weapon: Option<TagId>,
}

/// The weapon slot a player holds from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

/// What a player carried and pressed over the match, each series at its
/// changes.
///
/// The series run across lives and say nothing about whether the player is
/// alive: [`Film::weapon_at`](crate::Film::weapon_at) combines them with the
/// player's lives.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct PlayerState {
    /// Movement input.
    pub inputs: Vec<Input>,
    /// Weapon magazines and reserves.
    pub ammo: Vec<Ammo>,
    /// Energy weapons' charge.
    pub energy: Vec<Energy>,
    /// Weapon heat.
    pub heat: Vec<Heat>,
    /// Grenades carried.
    pub grenades: Vec<GrenadeCount>,
    /// Equipment charge.
    pub equipment: Vec<EquipmentCharge>,
    /// The weapons in the weapon slots.
    pub weapons: Vec<WeaponSlot>,
    /// The weapon slot held.
    pub held: Vec<HeldSlot>,
}

impl PlayerState {
    /// The weapon in `slot` at `tick`.
    pub fn weapon_in(&self, slot: u32, tick: Tick) -> Option<TagId> {
        self.weapons.iter().take_while(|w| w.tick <= tick).filter(|w| w.slot == slot).last()?.weapon
    }

    /// The weapon slot held at `tick`.
    pub fn held_slot(&self, tick: Tick) -> Option<u32> {
        latest(&self.held, tick, |h| h.tick).map(|h| h.slot)
    }

    /// The weapon in the held slot at `tick`.
    pub fn weapon_at(&self, tick: Tick) -> Option<TagId> {
        self.weapon_in(self.held_slot(tick)?, tick)
    }

    /// The magazine and reserve of `slot` at `tick`.
    pub fn ammo_in(&self, slot: u32, tick: Tick) -> Option<&Ammo> {
        self.ammo.iter().take_while(|a| a.tick <= tick).filter(|a| a.slot == slot).last()
    }

    /// The grenades carried at `tick`.
    pub fn grenades_at(&self, tick: Tick) -> Option<&GrenadeCount> {
        latest(&self.grenades, tick, |g| g.tick)
    }

    /// The movement input at `tick`.
    pub fn input_at(&self, tick: Tick) -> Option<&Input> {
        latest(&self.inputs, tick, |i| i.tick)
    }
}
