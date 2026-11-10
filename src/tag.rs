use std::fmt;

/// A global tag id: the 32-bit name the game gives an asset (a weapon, a
/// vehicle, a damage effect, a sound, a level). Films refer to everything in
/// the game's content by these ids. They print as eight hex digits.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TagId(pub u32);

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
