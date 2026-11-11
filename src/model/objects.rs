use super::{EntityId, PlayerId, PlayerMask, Position, TeamId, Tick};
use crate::TagId;
use crate::game::{self, Grenade, Item};

/// Where something was at a tick: a point of a vehicle's track or a
/// grenade's path.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct PathPoint {
    /// When.
    pub tick: Tick,
    /// Where.
    pub position: Position,
}

impl PathPoint {
    /// A point at `tick`.
    pub fn new(tick: Tick, position: Position) -> PathPoint {
        PathPoint { tick, position }
    }
}

/// A vehicle's track: one vehicle object from its creation, or the film's
/// start, to its last position record.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct VehicleTrack {
    /// The vehicle's id. Later vehicles reuse it; each creation starts a new
    /// track.
    pub entity: EntityId,
    /// The vehicle's tag.
    pub tag: TagId,
    /// Where it was, in tick order.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub track: Vec<PathPoint>,
}

impl VehicleTrack {
    /// What vehicle it is.
    pub fn vehicle(&self) -> Option<game::Vehicle> {
        game::Vehicle::from_tag(self.tag)
    }

    /// The first and last ticks of the track, both inclusive.
    pub fn span(&self) -> Option<(Tick, Tick)> {
        Some((self.track.first()?.tick, self.track.last()?.tick))
    }

    /// Where the vehicle was at `tick`: its last position at or before it.
    /// None before the track starts and after it ends.
    pub fn position_at(&self, tick: Tick) -> Option<Position> {
        let (_, end) = self.span()?;
        if tick > end {
            return None;
        }
        let i = self.track.partition_point(|p| p.tick <= tick).checked_sub(1)?;
        Some(self.track[i].position)
    }
}

/// A stretch of a player's life spent in one seat of a vehicle.
///
/// A biped's parent-state component names the vehicle it is attached to and
/// its seat, so a ride runs from the update that seats the biped to the one
/// that unseats it, moves it to another seat or deletes it; the rider's
/// samples over it are the vehicle's positions. On a film whose biped
/// updates can't be read, rides are inferred from the exits and from where
/// the vehicles went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Ride {
    /// The rider.
    pub player: PlayerId,
    /// The vehicle, an index into [`Film::vehicles`](crate::Film::vehicles),
    /// when it has a track. A rider in a part (a Falcon's side gun, a
    /// Scorpion's turret) rides the vehicle that carries the part.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub vehicle: Option<usize>,
    /// The seat: the parent state's, else on a film without it that of the
    /// exit that ends the ride. None when neither names one. It counts the
    /// seats of [`part`](Ride::part) when that is set, else the vehicle's,
    /// whose seat 0 is the driver's.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub seat: Option<u32>,
    /// The part the rider sits in, when the seat is a part's: a turret or a
    /// side gun the vehicle carries, by its tag.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub part: Option<TagId>,
    /// The first tick in the vehicle.
    pub from: Tick,
    /// The last tick in the vehicle (inclusive).
    pub to: Tick,
}

impl Ride {
    /// Whether the rider drives: seat 0 of the vehicle itself.
    pub fn is_driver(&self) -> bool {
        self.part.is_none() && self.seat == Some(0)
    }

    /// How long the ride lasted, its last tick included.
    pub fn duration(&self) -> std::time::Duration {
        Tick((self.to - self.from + 1).max(0) as u32).duration()
    }
}

/// An object standing on the map, from the seek snapshots: vehicles, weapons
/// and equipment on their spawners, devices.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct WorldObject {
    /// The object's tag.
    pub tag: TagId,
    /// The object's entity type: its block in chunk 0's component registry.
    pub entity_type: u32,
    /// Where it stood.
    pub position: Position,
    /// The first snapshot that had it.
    pub from: Tick,
    /// The first snapshot without it, so not inclusive: snapshots are 20 s
    /// apart, and it went somewhere in between. None if it was there to the
    /// end.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub to: Option<Tick>,
}

impl WorldObject {
    /// The item the object is.
    pub fn item(&self) -> Option<Item> {
        game::item(self.tag)
    }
}

/// A weapon lying on the ground after it was dropped or spawned, where it
/// came to rest.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct GroundWeapon {
    /// The weapon's tag.
    pub tag: TagId,
    /// Where it lay.
    pub position: Position,
    /// When it came to rest.
    pub from: Tick,
    /// When it was removed, so not inclusive; None if it was still there at
    /// the end.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub to: Option<Tick>,
    /// Removed by a player picking it up.
    pub taken: bool,
}

/// A thrown grenade's flight.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct GrenadeFlight {
    /// The thrower.
    pub player: PlayerId,
    /// The grenade.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub grenade: Option<Grenade>,
    /// Its path, thinned to within a quarter of a world unit of the full one.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub path: Vec<PathPoint>,
    /// When it went off: its deletion, else its last position.
    pub end: Tick,
    /// Whether `end` is the grenade's deletion.
    pub deleted: bool,
}

/// The ground a zone covers, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ZoneArea {
    /// A circle.
    #[non_exhaustive]
    Cylinder {
        /// Its radius.
        radius: f64,
    },
    /// A rectangle, turned by the shape's `angle`.
    #[non_exhaustive]
    Box {
        /// Its whole length along its forward axis.
        length: f64,
        /// Its whole width along its right axis.
        width: f64,
    },
}

/// A zone's or a hill's shape, from its object's properties.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ZoneShape {
    /// The ground it covers.
    pub area: ZoneArea,
    /// How far it reaches above its centre, world units.
    pub top: f64,
    /// How far below.
    pub bottom: f64,
    /// The direction of its forward axis, radians from +x toward +y.
    pub angle: f64,
    /// The team the map gives it.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub team: Option<TeamId>,
}

/// A zone's state from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ZoneState {
    /// From when.
    pub tick: Tick,
    /// The team that holds it.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub owner: Option<TeamId>,
    /// The team capturing it.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub capturing: Option<TeamId>,
    /// The capture's progress, 0 to 100.
    pub progress: u32,
    /// Players of more than one team are inside.
    pub contested: bool,
    /// The players inside.
    pub inside: PlayerMask,
}

/// A Strongholds zone, a hill, or a Firefight hill.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Zone {
    /// The zone object's entity index.
    pub entity: u32,
    /// Where it stands.
    pub position: Position,
    /// Its letter.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub letter: Option<char>,
    /// Its shape.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub shape: Option<ZoneShape>,
    /// Its state over time.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub states: Vec<ZoneState>,
}

/// King of the Hill's points on the current hill.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct HillScore {
    /// Points that win a hill.
    pub points_to_win: u32,
    /// Hills that win the match.
    pub hills_to_win: u32,
    /// Each team's points on the current hill, at each change.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub points: Vec<HillPoints>,
}

/// A team's points on the current King of the Hill hill, from `tick` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct HillPoints {
    /// When the points changed.
    pub tick: Tick,
    /// Whose.
    pub team: TeamId,
    /// The points.
    pub points: u32,
}
