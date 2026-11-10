use super::Tick;

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

/// What a player carried and pressed over the match, each series at its
/// changes.
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
}
