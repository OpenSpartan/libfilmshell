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

/// A vehicle's track: one vehicle object, its positions from its first
/// record to its last, and when it left the map. A vehicle sends its position
/// when it moves, so one parked stays where its last record put it, often
/// for minutes, until the game deletes it.
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
    /// When it left the map, so not inclusive: the game deleted it (a wreck
    /// is deleted some seconds after it was destroyed), or its id was given
    /// to another vehicle. None if it was there to the end, or the film's
    /// delete wasn't read.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub removed: Option<Tick>,
}

impl VehicleTrack {
    /// What vehicle it is.
    pub fn vehicle(&self) -> Option<game::Vehicle> {
        game::Vehicle::from_tag(self.tag)
    }

    /// The ticks of the track's first and last records, both inclusive.
    pub fn span(&self) -> Option<(Tick, Tick)> {
        Some((self.track.first()?.tick, self.track.last()?.tick))
    }

    /// Whether the vehicle was on the map at `tick`: from its first record
    /// until it was [removed](VehicleTrack::removed).
    pub fn on_map_at(&self, tick: Tick) -> bool {
        self.track.first().is_some_and(|p| p.tick <= tick) && self.removed.is_none_or(|r| tick < r)
    }

    /// Where the vehicle was at `tick`: its last position at or before it,
    /// while it was [on the map](VehicleTrack::on_map_at).
    pub fn position_at(&self, tick: Tick) -> Option<Position> {
        if !self.on_map_at(tick) {
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

    /// Whether the snapshots had it on the map at `tick`: from
    /// [`from`](WorldObject::from) until [`to`](WorldObject::to).
    pub fn on_map_at(&self, tick: Tick) -> bool {
        self.from <= tick && self.to.is_none_or(|t| tick < t)
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

impl GroundWeapon {
    /// Whether it lay on the ground at `tick`: from
    /// [`from`](GroundWeapon::from) until [`to`](GroundWeapon::to).
    pub fn on_map_at(&self, tick: Tick) -> bool {
        self.from <= tick && self.to.is_none_or(|t| tick < t)
    }
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

impl GrenadeFlight {
    /// Where the grenade was at `tick`, from its path's first point to
    /// [`end`](GrenadeFlight::end): between the two points around `tick`,
    /// in proportion to the time between them, then at the last point. The
    /// path is thinned, so this is the flight's line, not each tick's
    /// recorded position. None outside the flight.
    pub fn position_at(&self, tick: Tick) -> Option<Position> {
        along(&self.path, self.end, tick)
    }
}

/// A projectile's flight: a rocket, a plasma bolt, a spike, anything a
/// weapon fires as an object that flies rather than hitting on the spot.
/// Thrown grenades are [`GrenadeFlight`]s.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ProjectileFlight {
    /// Who fired it: the shooter its creation names, or the rider of the
    /// vehicle or turret that fired it. None when the creation names no one.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub player: Option<PlayerId>,
    /// The projectile's tag.
    pub projectile: TagId,
    /// Its path from where it was created, thinned as a grenade's is: a
    /// straight shot is its two ends.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub path: Vec<PathPoint>,
    /// When it went off or was gone: its deletion, else its last position.
    pub end: Tick,
    /// Whether `end` is the projectile's deletion.
    pub deleted: bool,
}

impl ProjectileFlight {
    /// Where the projectile was at `tick`, as
    /// [`GrenadeFlight::position_at`] places a grenade.
    pub fn position_at(&self, tick: Tick) -> Option<Position> {
        along(&self.path, self.end, tick)
    }

    /// The projectile's numbers in the game's sandbox: its velocity, range
    /// and the damage effect of a direct hit.
    pub fn stats(&self) -> Option<&'static game::ProjectileStats> {
        game::projectile_stats(self.projectile)
    }
}

/// A position along a thinned path at `tick`, from its first point to `end`.
fn along(path: &[PathPoint], end: Tick, tick: Tick) -> Option<Position> {
    let first = path.first()?;
    if tick < first.tick || tick > end {
        return None;
    }
    let i = path.partition_point(|p| p.tick <= tick);
    let a = path[i - 1];
    let Some(b) = path.get(i) else { return Some(a.position) };
    let f = (tick.0 - a.tick.0) as f64 / (b.tick.0 - a.tick.0) as f64;
    let mid = |a: f64, b: f64| a + (b - a) * f;
    let (p, q) = (a.position, b.position);
    Some(Position { x: mid(p.x, q.x), y: mid(p.y, q.y), z: p.z.zip(q.z).map(|(a, b)| mid(a, b)).or(p.z) })
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

impl Zone {
    /// Its state at `tick`: the last to start at or before it. None before
    /// the first.
    pub fn state_at(&self, tick: Tick) -> Option<&ZoneState> {
        self.states[..self.states.partition_point(|s| s.tick <= tick)].last()
    }
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

impl HillScore {
    /// `team`'s points on the current hill at `tick`: its last change at or
    /// before it, 0 before the first.
    pub fn points_at(&self, team: TeamId, tick: Tick) -> u32 {
        self.points.iter().take_while(|p| p.tick <= tick).filter(|p| p.team == team).last().map_or(0, |p| p.points)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t: u32, x: f64) -> PathPoint {
        PathPoint::new(Tick(t), Position::new(x, 0.0))
    }

    #[test]
    fn a_parked_vehicle_stays_until_removed() {
        let v = VehicleTrack {
            entity: EntityId(1),
            tag: TagId(0),
            track: vec![at(10, 1.0), at(20, 2.0)],
            removed: Some(Tick(100)),
        };
        assert_eq!(v.position_at(Tick(9)), None);
        assert_eq!(v.position_at(Tick(15)).map(|p| p.x), Some(1.0));
        assert_eq!(v.position_at(Tick(99)).map(|p| p.x), Some(2.0));
        assert_eq!(v.position_at(Tick(100)), None);
        let kept = VehicleTrack { removed: None, ..v };
        assert!(kept.on_map_at(Tick(1_000_000)));
    }

    #[test]
    fn a_grenade_moves_along_its_path() {
        let g = GrenadeFlight {
            player: PlayerId(0),
            grenade: None,
            path: vec![at(10, 0.0), at(20, 10.0)],
            end: Tick(30),
            deleted: true,
        };
        assert_eq!(g.position_at(Tick(9)), None);
        assert_eq!(g.position_at(Tick(10)).map(|p| p.x), Some(0.0));
        assert_eq!(g.position_at(Tick(15)).map(|p| p.x), Some(5.0));
        assert_eq!(g.position_at(Tick(25)).map(|p| p.x), Some(10.0));
        assert_eq!(g.position_at(Tick(31)), None);
    }

    #[test]
    fn states_and_points_hold_until_they_change() {
        let state = |t: u32, progress: u32| ZoneState {
            tick: Tick(t),
            owner: None,
            capturing: None,
            progress,
            contested: false,
            inside: PlayerMask(0),
        };
        let zone = Zone {
            entity: 0,
            position: Position::new(0.0, 0.0),
            letter: Some('A'),
            shape: None,
            states: vec![state(10, 0), state(20, 50)],
        };
        assert_eq!(zone.state_at(Tick(9)), None);
        assert_eq!(zone.state_at(Tick(19)).map(|s| s.progress), Some(0));
        assert_eq!(zone.state_at(Tick(20)).map(|s| s.progress), Some(50));

        let points = |t: u32, team: u8, points: u32| HillPoints { tick: Tick(t), team: TeamId(team), points };
        let hill = HillScore {
            points_to_win: 100,
            hills_to_win: 3,
            points: vec![points(10, 0, 5), points(12, 1, 3), points(20, 0, 0)],
        };
        assert_eq!(hill.points_at(TeamId(0), Tick(9)), 0);
        assert_eq!(hill.points_at(TeamId(0), Tick(15)), 5);
        assert_eq!(hill.points_at(TeamId(1), Tick(15)), 3);
        assert_eq!(hill.points_at(TeamId(0), Tick(20)), 0);
    }
}
