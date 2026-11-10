use crate::text::{ParseIdError, serde_as_text};
use std::fmt;
use std::str::FromStr;

/// A global tag id: the 32-bit name the game gives an asset (a weapon, a
/// vehicle, a damage effect, a sound, a level). Films refer to everything in
/// the game's content by these ids. They print, serialize and parse as eight
/// hex digits.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TagId(pub u32);

serde_as_text!(TagId);

impl FromStr for TagId {
    type Err = ParseIdError;

    /// Up to eight hex digits, with or without `0x`: `2b1824d5`.
    fn from_str(s: &str) -> Result<TagId, ParseIdError> {
        let hex = s.strip_prefix("0x").unwrap_or(s);
        if hex.is_empty() || hex.len() > 8 {
            return Err(ParseIdError { kind: "tag id" });
        }
        u32::from_str_radix(hex, 16).map(TagId).map_err(|_| ParseIdError { kind: "tag id" })
    }
}

impl TagId {
    /// The raw id.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// What the game's files say about the tag, when the catalogue lists it:
    /// [`game::tag_info`](crate::game::tag_info).
    pub fn info(self) -> Option<&'static crate::game::TagInfo> {
        crate::game::tag_info(self)
    }

    /// The tag's English name, when the catalogue has one ("BR75 Battle
    /// Rifle"). A weapon variant without a name of its own takes its base
    /// weapon's.
    pub fn name(self) -> Option<&'static str> {
        self.info()?.name
    }
}

impl fmt::Debug for TagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TagId({:08x})", self.0)
    }
}

impl fmt::Display for TagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

impl From<u32> for TagId {
    fn from(v: u32) -> TagId {
        TagId(v)
    }
}
