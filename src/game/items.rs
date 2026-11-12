//! Weapons, vehicles, equipment and the other objects a film names by tag.

pub use super::items_data::{Equipment, Vehicle, Weapon};
use super::items_data::{TAGS, VARIANTS};
use crate::TagId;

/// What kind of object a tag is, from the group the game files it under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ObjectKind {
    /// A weapon, including vehicles' guns and turrets.
    Weapon,
    /// A vehicle or a standalone turret.
    Vehicle,
    /// Equipment, power-ups and grenades as pickups.
    Equipment,
    /// A grenade as the inventory holds it.
    Grenade,
    /// A projectile: a round in flight, a thrown grenade.
    Projectile,
    /// A placed object: props, Forge blocks, zones, spawn points.
    Crate,
    /// A device with moving parts: doors, lifts, terminals.
    Machine,
    /// Static scenery and volumes.
    Scenery,
    /// A control: a switch or a panel.
    Control,
}

/// A grenade. The film's grenade type is one more than its index in the
/// game's grenade list: 1 frag, 2 plasma, 3 dynamo, 4 spike.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Grenade {
    /// M9 Frag Grenade.
    Frag,
    /// Plasma Grenade.
    Plasma,
    /// Dynamo Grenade.
    Dynamo,
    /// Spike Grenade.
    Spike,
}

impl Grenade {
    /// The grenade with the film's one-based grenade type.
    pub fn from_type(t: u32) -> Option<Grenade> {
        Some(match t {
            1 => Grenade::Frag,
            2 => Grenade::Plasma,
            3 => Grenade::Dynamo,
            4 => Grenade::Spike,
            _ => return None,
        })
    }

    /// The film's one-based grenade type.
    pub fn type_index(self) -> u32 {
        self as u32 + 1
    }

    /// The game's English name.
    pub fn name(self) -> &'static str {
        match self {
            Grenade::Frag => "M9 Frag Grenade",
            Grenade::Plasma => "Plasma Grenade",
            Grenade::Dynamo => "Dynamo Grenade",
            Grenade::Spike => "Spike Grenade",
        }
    }
}

/// Something a player can hold, use or ride.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Item {
    /// A weapon the catalogue names.
    Weapon(Weapon),
    /// A vehicle the catalogue names.
    Vehicle(Vehicle),
    /// One of the four grenades.
    Grenade(Grenade),
    /// Equipment or a power-up the catalogue names.
    Equipment(Equipment),
}

impl Item {
    /// The game's English name.
    pub fn name(self) -> &'static str {
        match self {
            Item::Weapon(w) => w.name(),
            Item::Vehicle(v) => v.name(),
            Item::Grenade(g) => g.name(),
            Item::Equipment(e) => e.name(),
        }
    }
}

/// What the game's files say about one tag. With serde it serializes, so a
/// viewer in another language can name the tag ids a film's JSON holds:
/// write out [`tags`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct TagInfo {
    /// The tag's id.
    pub tag: TagId,
    /// What kind of object the tag defines.
    pub kind: ObjectKind,
    /// Its English name, or its nearest ancestor's when it has none of its
    /// own.
    pub name: Option<&'static str>,
    /// The item it is, for weapons, vehicles, grenades and equipment.
    pub item: Option<Item>,
    /// The tag it derives from (a weapon variant's base weapon).
    pub parent: Option<TagId>,
    /// For a vehicle part (a turret, a gunner seat), the vehicle whose model
    /// attaches it.
    pub part_of: Option<TagId>,
    /// Whether the mode scripts count the weapon in the power weapon tier.
    pub power_weapon: bool,
}

pub(super) const fn row(
    tag: u32,
    kind: ObjectKind,
    name: &'static str,
    item: Option<Item>,
    parent: u32,
    part_of: u32,
    power_weapon: bool,
) -> TagInfo {
    TagInfo {
        tag: TagId(tag),
        kind,
        name: if name.is_empty() { None } else { Some(name) },
        item,
        parent: if parent == 0 { None } else { Some(TagId(parent)) },
        part_of: if part_of == 0 { None } else { Some(TagId(part_of)) },
        power_weapon,
    }
}

/// What the game's files say about `tag`, when the catalogue lists it.
pub fn tag_info(tag: TagId) -> Option<&'static TagInfo> {
    TAGS.binary_search_by_key(&tag, |t| t.tag).ok().map(|i| &TAGS[i])
}

/// Every catalogued tag, in tag id order.
pub fn tags() -> &'static [TagInfo] {
    TAGS
}

/// The item a tag is, when it is one.
pub fn item(tag: TagId) -> Option<Item> {
    tag_info(tag)?.item
}

impl Weapon {
    /// The weapon a tag is.
    pub fn from_tag(tag: TagId) -> Option<Weapon> {
        match item(tag)? {
            Item::Weapon(w) => Some(w),
            _ => None,
        }
    }
}

impl Vehicle {
    /// The vehicle a tag is.
    pub fn from_tag(tag: TagId) -> Option<Vehicle> {
        match item(tag)? {
            Item::Vehicle(v) => Some(v),
            _ => None,
        }
    }
}

impl Equipment {
    /// The equipment a tag is.
    pub fn from_tag(tag: TagId) -> Option<Equipment> {
        match item(tag)? {
            Item::Equipment(e) => Some(e),
            _ => None,
        }
    }
}

impl Grenade {
    /// The grenade a tag is, as equipment or as a grenade.
    pub fn from_tag(tag: TagId) -> Option<Grenade> {
        match item(tag)? {
            Item::Grenade(g) => Some(g),
            _ => None,
        }
    }
}

/// The internal name of a weapon variant ("default", "plasma", ...), by the
/// string id weapon events carry.
pub fn weapon_variant_name(id: crate::StringId) -> Option<&'static str> {
    VARIANTS.binary_search_by_key(&id.0, |v| v.0).ok().map(|i| VARIANTS[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookups() {
        assert!(TAGS.windows(2).all(|w| w[0].tag < w[1].tag));
        assert_eq!(Weapon::from_tag(TagId(0x2b1824d5)), Some(Weapon::Br75BattleRifle));
        assert_eq!(Weapon::Br75BattleRifle.name(), "BR75 Battle Rifle");
        assert_eq!(Vehicle::from_tag(TagId(0x0000254b)), Some(Vehicle::Falcon));
        assert_eq!(tag_info(TagId(1)), None);
        assert_eq!(weapon_variant_name(crate::StringId(0x42c9679f)), Some("default"));
        assert_eq!(Grenade::from_type(3).map(Grenade::type_index), Some(3));
    }
}
