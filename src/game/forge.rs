//! Forge objects, and the items the multiplayer item identifiers name.

use super::ObjectKind;
use super::forge_data::{FORGE_OBJECTS, MP_ITEMS};
use crate::TagId;

/// The English name of the Forge object with id `tag`, the id a map variant
/// places it by ([`MapObject::tag`](crate::map_variant::MapObject::tag)),
/// from the game's Forge manifest.
///
/// The manifest's ids are its own: the object a film carries for a placed
/// object has another tag, which [`tag_info`](super::tag_info) knows.
///
/// ```
/// use filmshell::{TagId, game::forge_object_name};
///
/// assert_eq!(forge_object_name(TagId(0xa7ff0c39)), Some("KOTH Capture Zone"));
/// ```
pub fn forge_object_name(tag: TagId) -> Option<&'static str> {
    FORGE_OBJECTS.binary_search_by_key(&tag.0, |o| o.0).ok().map(|i| FORGE_OBJECTS[i].1)
}

/// A multiplayer item identifier and the item it stands for: the number a
/// stock weapon, equipment or vehicle spawner names its item by.
///
/// Read from the game's code: its scripting enums number the items of each
/// kind (`MP_WEAPON_IDENTIFIER`, `MP_EQUIPMENT_IDENTIFIER`,
/// `MP_VEHICLE_IDENTIFIER`), and its multiplayer scripts' placement tables
/// give each number its item. Some slots named `Unused` were later given
/// one: weapon 10, `UnusedAssault1`, is the Bandit Evo. Grenades are
/// equipment identifiers. Vehicle configurations share their vehicle's tag
/// (the Razorback and the Rockethog are Warthogs, the Gungoose a Mongoose)
/// and differ by `internal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct MpItem {
    /// The identifier's kind: [`ObjectKind::Weapon`],
    /// [`ObjectKind::Equipment`] or [`ObjectKind::Vehicle`].
    pub kind: ObjectKind,
    /// The identifier.
    pub identifier: u32,
    /// The enum's name for it (`"BattleRifle"`, `"UnusedAssault1"`).
    pub name: &'static str,
    /// The item's tag, which [`tag_info`](super::tag_info) names.
    pub tag: TagId,
    /// The placement table's internal name for the item (`"battle_rifle"`,
    /// `"razorbackhog"`).
    pub internal: &'static str,
}

/// The item multiplayer item identifier `identifier` of `kind` stands for,
/// when the game's placement tables give it one.
///
/// ```
/// use filmshell::{TagId, game::{ObjectKind, mp_item}};
///
/// let br = mp_item(ObjectKind::Weapon, 13).map(|m| m.tag);
/// assert_eq!(br, Some(TagId(0x2b1824d5)));
/// assert_eq!(br.and_then(|t| t.name()), Some("BR75 Battle Rifle"));
/// ```
pub fn mp_item(kind: ObjectKind, identifier: u32) -> Option<&'static MpItem> {
    MP_ITEMS.iter().find(|m| m.kind == kind && m.identifier == identifier)
}

/// Every multiplayer item identifier with an item: weapons, then equipment,
/// then vehicles, each by identifier.
pub fn mp_items() -> &'static [MpItem] {
    MP_ITEMS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Equipment, Item, Vehicle, Weapon, item};

    #[test]
    fn forge_objects() {
        assert!(FORGE_OBJECTS.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(forge_object_name(TagId(0xa495fe83)), Some("Spawn Point [Respawn]"));
        assert_eq!(forge_object_name(TagId(1)), None);
    }

    #[test]
    fn identifiers_name_their_items() {
        let kinds = [ObjectKind::Weapon, ObjectKind::Equipment, ObjectKind::Vehicle];
        let order = |m: &MpItem| (kinds.iter().position(|&k| k == m.kind), m.identifier);
        assert!(MP_ITEMS.windows(2).all(|w| order(&w[0]) < order(&w[1])));
        assert!(MP_ITEMS.iter().all(|m| kinds.contains(&m.kind)));
        let tag = |kind, id| mp_item(kind, id).map(|m| m.tag);
        assert_eq!(item(tag(ObjectKind::Weapon, 10).unwrap()), Some(Item::Weapon(Weapon::BanditEvo)));
        assert_eq!(item(tag(ObjectKind::Weapon, 14).unwrap()), Some(Item::Weapon(Weapon::Vk78CommandoRifle)));
        assert_eq!(item(tag(ObjectKind::Equipment, 23).unwrap()), Some(Item::Equipment(Equipment::ThreatSeeker)));
        assert_eq!(item(tag(ObjectKind::Equipment, 25).unwrap()), Some(Item::Equipment(Equipment::RepairField)));
        assert_eq!(item(tag(ObjectKind::Vehicle, 15).unwrap()), Some(Item::Vehicle(Vehicle::Warthog)));
        assert_eq!(mp_item(ObjectKind::Vehicle, 15).map(|m| m.internal), Some("rockethog"));
        assert_eq!(tag(ObjectKind::Vehicle, 11), Some(TagId(0xab7eaefd)));
        assert_eq!(tag(ObjectKind::Weapon, 5), None);
        assert_eq!(tag(ObjectKind::Grenade, 1), None);
        // Every item but the Komodo's is in the catalogue.
        assert!(MP_ITEMS.iter().filter(|m| m.tag != TagId(0xab7eaefd)).all(|m| item(m.tag).is_some()), "{MP_ITEMS:?}");
    }
}
