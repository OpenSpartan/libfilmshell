//! Medals, by the game's medal index.

pub use super::medals_data::Medal;

/// A medal's category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum MedalKind {
    /// Kills without dying.
    Spree,
    /// A game mode's own.
    Mode,
    /// Kills in quick succession.
    Multikill,
    /// Kills with one kind of weapon or vehicle.
    Proficiency,
    /// A particular kind of kill.
    Skill,
    /// A kill with flair.
    Style,
}

/// How hard a medal is to earn, as the game ranks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum MedalDifficulty {
    /// Normal.
    Normal,
    /// Heroic.
    Heroic,
    /// Legendary.
    Legendary,
    /// Mythic.
    Mythic,
}

impl Medal {
    /// The game's medal index.
    pub fn index(self) -> u32 {
        self as u32
    }

    /// The medal's category.
    pub fn kind(self) -> MedalKind {
        self.row().0
    }

    /// How hard it is to earn.
    pub fn difficulty(self) -> MedalDifficulty {
        self.row().1
    }

    /// The personal score it is worth.
    pub fn personal_score(self) -> u32 {
        self.row().2
    }

    /// The game's sorting weight. HighlightEvents records carry it.
    pub fn sorting_weight(self) -> u32 {
        self.row().3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medals() {
        assert_eq!(Medal::from_index(0), Some(Medal::DoubleKill));
        assert_eq!(Medal::DoubleKill.name(), "Double Kill");
        assert_eq!(Medal::DoubleKill.kind(), MedalKind::Multikill);
        assert_eq!(Medal::DoubleKill.personal_score(), 50);
        assert_eq!(Medal::from_index(14), None);
        assert_eq!(Medal::from_index(121).map(Medal::index), Some(121));
    }
}
