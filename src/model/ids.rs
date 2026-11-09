use std::fmt;

/// A player's index in the film: the engine's player id for everyone
/// present at the start and for bots. Events, the scoreboard and recipient
/// masks name players by it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PlayerId(pub u8);

impl PlayerId {
    /// The index as a `usize`.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "player {}", self.0)
    }
}

/// An Xbox user id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Xuid(pub u64);

impl fmt::Display for Xuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A team id: 0 is the first team, 1 the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TeamId(pub u8);

impl fmt::Display for TeamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "team {}", self.0)
    }
}

/// A set of players as the game packs it: bit `31 - index` for each player.
/// Voice lines, HUD messages and game events name their recipients this way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PlayerMask(pub u32);

impl PlayerMask {
    /// Whether `player` is in the set.
    pub fn contains(self, player: PlayerId) -> bool {
        player.0 < 32 && self.0 & (0x8000_0000 >> player.0) != 0
    }

    /// The players in the set, in index order.
    pub fn iter(self) -> impl Iterator<Item = PlayerId> {
        (0..32u8).filter(move |&p| self.0 & (0x8000_0000 >> p) != 0).map(PlayerId)
    }

    /// Whether the set is empty.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// A GUID as the Halo services write it, such as a map variant's asset id.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Guid(pub [u8; 16]);

impl fmt::Display for Guid {
    /// The first three fields little-endian, as the services print it:
    /// `33075df7-01c8-40e1-8b3e-1baee0054c76`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = &self.0;
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
        )
    }
}

impl fmt::Debug for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({self})")
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Guid {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// A position in world units (ten to an engine unit, the unit of the game's
/// files). `x` and `y` are horizontal, `z` is up and known only when the
/// film's level is in the catalogue.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Position {
    /// The first horizontal axis: minus ten times engine y.
    pub x: f64,
    /// The second horizontal axis: ten times engine x.
    pub y: f64,
    /// Height, when known.
    pub z: Option<f64>,
}

impl Position {
    /// A position on the horizontal plane.
    pub fn new(x: f64, y: f64) -> Position {
        Position { x, y, z: None }
    }

    /// The horizontal distance to `other`.
    pub fn distance(&self, other: &Position) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guid_prints_as_the_services_write_it() {
        let b = [0xf7, 0x5d, 0x07, 0x33, 0xc8, 0x01, 0xe1, 0x40, 0x8b, 0x3e, 0x1b, 0xae, 0xe0, 0x05, 0x4c, 0x76];
        assert_eq!(Guid(b).to_string(), "33075df7-01c8-40e1-8b3e-1baee0054c76");
    }

    #[test]
    fn masks() {
        let m = PlayerMask(0x8000_0000 | 0x2000_0000);
        assert!(m.contains(PlayerId(0)) && m.contains(PlayerId(2)) && !m.contains(PlayerId(1)));
        assert_eq!(m.iter().collect::<Vec<_>>(), vec![PlayerId(0), PlayerId(2)]);
    }
}
