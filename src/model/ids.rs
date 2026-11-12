use crate::text::{ParseIdError, serde_as_text};
use std::fmt;
use std::str::FromStr;

/// A player's place in the film's roster.
///
/// The roster is everyone present at the start in its order, then joiners and
/// bots as they appear. Every player the model names is named by it. Ids can
/// skip a number (a roster entry with neither an account nor a bot record is
/// left out), so look players up with [`Film::player`](crate::Film::player)
/// rather than by indexing [`Film::players`](crate::Film::players).
///
/// The film itself names players by engine slot, which a joiner can take
/// over from someone who left; the library maps those to the player who
/// held the slot at the time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerId(pub u8);

impl PlayerId {
    /// The index as a `usize`.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A biped's or a vehicle's id: `(generation - 1) * 256 + index`.
///
/// The index is counted within the object's kind (a raw entity index less 512
/// for a biped, less 768 for a vehicle, as [`format`](crate::format) gives
/// them), so bipeds and vehicles count ids separately: a biped and a vehicle
/// can share a number. The game reuses an id once its object is gone, so an id
/// is unique only among the objects that exist at the same time: a biped's id
/// names one life, a vehicle's one [`VehicleTrack`](crate::VehicleTrack).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityId(pub u16);

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An Xbox user id.
///
/// It serializes as its decimal text: XUIDs exceed 2^53, past the integers
/// JavaScript holds exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Xuid(pub u64);

serde_as_text!(Xuid);

impl fmt::Display for Xuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Xuid {
    type Err = ParseIdError;

    /// The decimal number: `2533274857096890`.
    fn from_str(s: &str) -> Result<Xuid, ParseIdError> {
        s.parse().map(Xuid).map_err(|_| ParseIdError { kind: "XUID" })
    }
}

/// A team id: 0 is the first team, 1 the second, up to 7 in modes with
/// more teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TeamId(pub u8);

impl fmt::Display for TeamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A set of players as the game packs it: bit `31 - id` for each
/// [`PlayerId`], so ids 0 to 31 only. Voice lines, game events and zones name
/// their players this way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlayerMask(pub u32);

impl PlayerMask {
    /// Whether `player` is in the set.
    pub fn contains(self, player: PlayerId) -> bool {
        player.0 < 32 && self.0 & (0x8000_0000 >> player.0) != 0
    }

    /// The players in the set, in id order.
    pub fn iter(self) -> impl Iterator<Item = PlayerId> {
        (0..32u8).filter(move |&p| self.0 & (0x8000_0000 >> p) != 0).map(PlayerId)
    }

    /// How many players are in the set.
    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Whether the set is empty.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl FromIterator<PlayerId> for PlayerMask {
    /// The set of the players given; ids from 32 on are left out.
    fn from_iter<I: IntoIterator<Item = PlayerId>>(players: I) -> PlayerMask {
        PlayerMask(players.into_iter().filter(|p| p.0 < 32).fold(0, |m, p| m | 0x8000_0000 >> p.0))
    }
}

/// `get` and `From` for the id newtypes, as [`TagId`](crate::TagId) has.
macro_rules! raw_id {
    ($($t:ident($raw:ty)),*) => {$(
        impl $t {
            /// The raw value.
            pub const fn get(self) -> $raw {
                self.0
            }
        }

        impl From<$raw> for $t {
            fn from(v: $raw) -> $t {
                $t(v)
            }
        }
    )*};
}

raw_id!(PlayerId(u8), TeamId(u8), EntityId(u16), Xuid(u64), PlayerMask(u32));
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

serde_as_text!(Guid);

impl FromStr for Guid {
    type Err = ParseIdError;

    /// The form [`Display`](fmt::Display) prints:
    /// `33075df7-01c8-40e1-8b3e-1baee0054c76`.
    fn from_str(s: &str) -> Result<Guid, ParseIdError> {
        let err = ParseIdError { kind: "GUID" };
        let parts: Vec<&str> = s.split('-').collect();
        if parts.iter().map(|p| p.len()).ne([8, 4, 4, 4, 12]) {
            return Err(err);
        }
        let hex: String = parts.concat();
        let mut v = [0u8; 16];
        for (i, b) in v.iter_mut().enumerate() {
            *b = u8::from_str_radix(hex.get(2 * i..2 * i + 2).ok_or(err.clone())?, 16).map_err(|_| err.clone())?;
        }
        // The first three fields are stored little-endian.
        let mut b = v;
        b[..4].reverse();
        b[4..6].reverse();
        b[6..8].reverse();
        Ok(Guid(b))
    }
}

/// A position in world units.
///
/// A world unit is a foot; the game's files use engine units of ten world
/// units. `x` and `y` are horizontal; `z` is up, known only when the film's
/// positions convert with the level's box
/// ([`CoordinateSource::LevelBox`](crate::CoordinateSource::LevelBox)): on life
/// samples on foot, and on the kill, flag and power seed positions taken from
/// them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    /// The first horizontal axis: minus ten times engine y.
    pub x: f64,
    /// The second horizontal axis: ten times engine x.
    pub y: f64,
    /// Height, when known.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
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
        assert_eq!("33075df7-01c8-40e1-8b3e-1baee0054c76".parse(), Ok(Guid(b)));
        assert!("33075df7-01c8-40e1-8b3e".parse::<Guid>().is_err());
    }

    #[test]
    fn ids_parse_back() {
        assert_eq!("2b1824d5".parse(), Ok(crate::TagId(0x2b1824d5)));
        assert_eq!("0x7ca9".parse(), Ok(crate::TagId(0x7ca9)));
        assert!("2b1824d5f".parse::<crate::TagId>().is_err());
        assert_eq!("2533274857096890".parse(), Ok(Xuid(2533274857096890)));
    }

    #[test]
    fn masks() {
        let m = PlayerMask(0x8000_0000 | 0x2000_0000);
        assert!(m.contains(PlayerId(0)) && m.contains(PlayerId(2)) && !m.contains(PlayerId(1)));
        assert_eq!(m.iter().collect::<Vec<_>>(), vec![PlayerId(0), PlayerId(2)]);
        assert_eq!(m.len(), 2);
        assert_eq!(m.iter().collect::<PlayerMask>(), m);
    }
}
