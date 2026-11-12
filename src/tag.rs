use crate::text::{ParseIdError, serde_as_text};
use std::fmt;
use std::str::FromStr;

/// A global tag id: the 32-bit name the game gives an asset.
///
/// Weapons, vehicles, damage effects, sounds and levels are assets, and films
/// refer to all of the game's content by these ids. They print and serialize as
/// eight hex digits, and parse from up to eight, with or without `0x`.
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

impl fmt::LowerHex for TagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for TagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl From<u32> for TagId {
    fn from(v: u32) -> TagId {
        TagId(v)
    }
}
/// A string id: the 32-bit murmur3 hash (x86, seed 0) the game names a string
/// by.
///
/// The strings are such things as localization keys, game events' properties,
/// and options' and statistics' names. Like [`TagId`]s they print and
/// serialize as eight hex digits, and parse from up to eight, with or without
/// `0x`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StringId(pub u32);

serde_as_text!(StringId);

impl StringId {
    /// The raw id.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The English text of the game text key this id names, when the library
    /// has it ([`game::text`](crate::game::text)).
    pub fn text(self) -> Option<&'static str> {
        crate::game::text(self)
    }

    /// The id of `name`, hashed as given: the game lower-cases some names
    /// before hashing them (a variant option's), so pass those lower-cased.
    ///
    /// ```
    /// use filmshell::StringId;
    ///
    /// assert_eq!(StringId::of("default"), StringId(0x42c9679f));
    /// ```
    pub fn of(name: &str) -> StringId {
        StringId(crate::game::murmur3(name.as_bytes()))
    }
}

impl FromStr for StringId {
    type Err = ParseIdError;

    /// Up to eight hex digits, with or without `0x`: `42c9679f`.
    fn from_str(s: &str) -> Result<StringId, ParseIdError> {
        let hex = s.strip_prefix("0x").unwrap_or(s);
        if hex.is_empty() || hex.len() > 8 {
            return Err(ParseIdError { kind: "string id" });
        }
        u32::from_str_radix(hex, 16).map(StringId).map_err(|_| ParseIdError { kind: "string id" })
    }
}

impl fmt::Debug for StringId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StringId({:08x})", self.0)
    }
}

impl fmt::Display for StringId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

impl fmt::LowerHex for StringId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for StringId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl From<u32> for StringId {
    fn from(v: u32) -> StringId {
        StringId(v)
    }
}
