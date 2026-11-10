//! Weapon names for the readers: a tag id resolved through the catalogue.

use crate::game::{self, ObjectKind};
use crate::TagId;

/// A weapon as the readers list it: its tag and what the catalogue says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weapon {
    pub id: u32,
    pub name: Option<&'static str>,
    pub kind: Option<ObjectKind>,
}

/// The catalogue entry of tag `id`.
pub fn tag(id: u32) -> Option<&'static game::TagInfo> {
    game::tag_info(TagId(id))
}

/// The weapon with tag id `id`; unknown tags keep only their id.
pub fn weapon(id: u32) -> Weapon {
    let t = tag(id);
    Weapon { id, name: t.and_then(|t| t.name), kind: t.map(|t| t.kind) }
}
