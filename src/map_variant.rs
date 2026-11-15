//! Map variants: the objects a map places, and the game modes' objectives
//! among them.
//!
//! A map variant is the file a map's objects come from (`map.mvar` among a
//! map's files on the Halo services): every object the map places, with its
//! position, axes, team, Forge labels, volume and properties, in Bond
//! compact binary. A film doesn't carry it. [`MapVariant::from_bytes`] reads
//! one; [`MapVariant::spawn_pads`] gives its spawn points for
//! [`Hints::spawn_pads`](crate::Hints::spawn_pads), and
//! [`MapVariant::objectives`] the flags, plates, zones, hills, ball stands
//! and bomb sites each game mode uses.
//!
//! The fields are read from the game's code: its executable's Bond metadata
//! names every struct and field of a map variant. Positions are world units
//! in the model's axes ([`Position`]), with the height known.
//!
//! ```no_run
//! use filmshell::map_variant::MapVariant;
//!
//! # let bytes: Vec<u8> = Vec::new();
//! let map = MapVariant::from_bytes(&bytes)?;
//! for o in &map.objects {
//!     println!("{} at {:.1}, {:.1}", o.name().unwrap_or("?"), o.position.x, o.position.y);
//! }
//! for o in map.objectives() {
//!     println!("{:?} {:?} at {:.1}, {:.1}", o.modes, o.kind, o.position.x, o.position.y);
//! }
//! # Ok::<(), filmshell::Error>(())
//! ```

use crate::bond::{self, Struct, Value};
use crate::calibrate::{WORLD_PER_ENGINE, engine_to_world, world_axes};
use crate::{Error, Position, SpawnKind, SpawnPad, StringId, TagId, TeamId, ZoneArea, ZoneShape, game};

// The fields, by the names the game's Bond metadata gives them.
/// `Definition.m_properties`; its field 0 is `m_scenarioIdentifier`, the level.
const DEF_PROPERTIES: u16 = 1;
/// `Definition.m_entries`: the objects.
const DEF_ENTRIES: u16 = 3;
/// `Definition.m_objectScriptProperties`: an object's index in
/// `m_entries` to its `ForgeScriptProperties.ValueCollection`.
const DEF_SCRIPT_PROPERTIES: u16 = 11;
/// `DefinitionEntry`'s `m_tag`, `m_position`, `m_up`, `m_forward`,
/// `m_type`, and `m_properties`, a `PropertyStorage`: one list per kind of
/// properties, of one struct when the object has them.
const ENT_TAG: u16 = 2;
const ENT_POSITION: u16 = 3;
const ENT_UP: u16 = 4;
const ENT_FORWARD: u16 = 5;
const ENT_TYPE: u16 = 7;
const ENT_PROPERTIES: u16 = 8;
/// `PropertyStorage`'s `BaseProperties`, `DefaultProperties` and
/// `SpawnPointProperties`.
const SLOT_BASE: u16 = 0;
const SLOT_DEFAULT: u16 = 1;
const SLOT_SPAWN_POINT: u16 = 7;
/// `BaseProperties`: `m_boundary`, `m_teamDesignator`, the Forge labels (a
/// list of string ids) and the spawn order.
const BASE_BOUNDARY: u16 = 0;
const BASE_TEAM: u16 = 8;
const BASE_LABELS: u16 = 9;
const BASE_SPAWN_ORDER: u16 = 11;
/// `DefaultProperties`: the variant (a list of one string id) and
/// `m_respawnTime`, seconds.
const DEFAULT_VARIANT: u16 = 0;
const DEFAULT_RESPAWN_TIME: u16 = 4;
/// `SpawnPointProperties.m_initialSpawnPoint`, true when left out.
const SPAWN_INITIAL: u16 = 0;
/// `Boundary`: the shape (2 a cylinder, 3 a box), then its whole length
/// along the forward axis or its radius, its whole width, its height above
/// its centre and its depth below, each 1/65536 engine units.
const BOUNDARY_SHAPE: u16 = 0;
const BOUNDARY_EXTENTS: [u16; 4] = [5, 6, 7, 8];
/// `ValueCollection`'s bool, float, string id and tag properties, each a map
/// from the property name's string id.
const SCRIPT_MAPS: [u16; 4] = [0, 1, 2, 3];

/// A map variant: the level a map is built on and the objects it places.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MapVariant {
    /// The `levl` tag the map is built on: [`game::level`] knows the
    /// catalogue's, and [`Hints::level`](crate::Hints::level) takes it.
    pub level: TagId,
    /// Every object it places, in the file's order.
    pub objects: Vec<MapObject>,
}

/// An object a map variant places.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MapObject {
    /// The Forge object it is (`m_tag`), by the id the game's Forge manifest
    /// gives it: [`game::forge_object_name`] names it.
    pub tag: TagId,
    /// What the game takes it for (`m_type`).
    pub object_type: ObjectType,
    /// Where it stands.
    pub position: Position,
    /// Its forward axis, a unit vector in world axes: `[x, y, z]`.
    pub forward: [f64; 3],
    /// Its up axis.
    pub up: [f64; 3],
    /// The team the map gives it (`m_teamDesignator`), in the team ids films
    /// use (measured on films: the respawns on a team's spawn points are
    /// that team's).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub team: Option<TeamId>,
    /// Its Forge labels, by the string ids of their names: the modes find
    /// their objects by them (`flag_spawn`, `bastion_zone`).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub labels: Vec<StringId>,
    /// Its volume (`m_boundary`), when it is a cylinder or a box: what a
    /// zone or a hill covers, a kill volume's or a spawn zone's extent. Its
    /// angle is the object's [`heading`](MapObject::heading), its team the
    /// object's.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub shape: Option<ZoneShape>,
    /// Its spawn order: King of the Hill's plates give their hills' places
    /// in the rotation by it.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub spawn_order: Option<u32>,
    /// The object's variant, by the string id of its name (`default`,
    /// `damaged`).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub variant: Option<StringId>,
    /// Seconds before it places its item again (`m_respawnTime`), when the
    /// variant sets them here. Forge objects keep theirs as the script
    /// property `spawntime`.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub respawn_time: Option<u32>,
    /// For a spawn point, the spawns it serves: initial spawns, or respawns
    /// (backup spawn points among them).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub spawn: Option<SpawnKind>,
    /// Its Forge script properties, by the string ids of their names
    /// (`spawntime`, `vehicle`, `externalreceivingchannel`):
    /// [`script_property`](MapObject::script_property) finds one.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub script: Vec<(StringId, ScriptValue)>,
}

impl MapObject {
    /// The Forge object's English name ([`game::forge_object_name`]).
    pub fn name(&self) -> Option<&'static str> {
        game::forge_object_name(self.tag)
    }

    /// The direction it faces: its forward axis on the horizontal plane, in
    /// radians from +x toward +y.
    pub fn heading(&self) -> f64 {
        self.forward[1].atan2(self.forward[0])
    }

    /// The script property `name`: the string id of the property's name.
    ///
    /// ```no_run
    /// # fn f(o: &filmshell::map_variant::MapObject) {
    /// use filmshell::{StringId, map_variant::ScriptValue};
    ///
    /// if let Some(ScriptValue::Number(s)) = o.script_property(StringId::of("spawntime")) {
    ///     println!("respawns every {s} s");
    /// }
    /// # }
    /// ```
    pub fn script_property(&self, name: StringId) -> Option<ScriptValue> {
        self.script.iter().find(|p| p.0 == name).map(|p| p.1)
    }
}

/// What the game takes a map object for: its `m_type`, which picks the
/// properties it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ObjectType {
    /// Type 1, every other placed object: props, weapon and vehicle pads,
    /// flag stands, zones, hills. Its Forge object tells them apart.
    Default,
    /// Type 3, a teleporter.
    Teleporter,
    /// Type 4, a named location: a volume the HUD names a place by.
    NamedLocation,
    /// Type 7, a spawn point.
    SpawnPoint,
    /// Type 8, a spawn zone.
    SpawnZone,
    /// Type 10, a launcher (the game's impulse cannon).
    Launcher,
    /// Type 14, a safe volume.
    SafeVolume,
    /// Type 15, a kill volume.
    KillVolume,
    /// Type 21, a Forge primitive: a block, a wall, a floor.
    ForgePrimitive,
    /// Another type.
    Other(u8),
}

impl ObjectType {
    /// The type `m_type` is.
    pub fn from_raw(t: u8) -> ObjectType {
        match t {
            1 => ObjectType::Default,
            3 => ObjectType::Teleporter,
            4 => ObjectType::NamedLocation,
            7 => ObjectType::SpawnPoint,
            8 => ObjectType::SpawnZone,
            10 => ObjectType::Launcher,
            14 => ObjectType::SafeVolume,
            15 => ObjectType::KillVolume,
            21 => ObjectType::ForgePrimitive,
            t => ObjectType::Other(t),
        }
    }

    /// Its `m_type`.
    pub fn raw(self) -> u8 {
        match self {
            ObjectType::Default => 1,
            ObjectType::Teleporter => 3,
            ObjectType::NamedLocation => 4,
            ObjectType::SpawnPoint => 7,
            ObjectType::SpawnZone => 8,
            ObjectType::Launcher => 10,
            ObjectType::SafeVolume => 14,
            ObjectType::KillVolume => 15,
            ObjectType::ForgePrimitive => 21,
            ObjectType::Other(t) => t,
        }
    }
}

/// A Forge script property's value.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ScriptValue {
    /// A flag.
    Bool(bool),
    /// A number: `spawntime`, `maxcount`.
    Number(f64),
    /// A string id: the item `vehicle` and `grenade` name
    /// (`vehicle_warthog`), a teleporter's channel (`destination_alpha`).
    String(StringId),
    /// A tag.
    Tag(TagId),
}

impl MapVariant {
    /// Reads a map variant, `map.mvar`.
    ///
    /// # Errors
    ///
    /// [`Error::NotAMapVariant`] when the bytes aren't Bond compact binary
    /// or aren't a map variant's: no level, or objects that aren't structs.
    pub fn from_bytes(bytes: &[u8]) -> Result<MapVariant, Error> {
        let doc = bond::read(bytes).map_err(|_| Error::NotAMapVariant)?;
        let level = doc.strukt(DEF_PROPERTIES).and_then(|p| p.int(0)).ok_or(Error::NotAMapVariant)?;
        let mut script: Vec<(usize, &Struct<'_>)> = doc
            .map(DEF_SCRIPT_PROPERTIES)
            .iter()
            .filter_map(|(k, v)| match (k, v) {
                (Value::Int(i), Value::Struct(s)) => Some((usize::try_from(*i).ok()?, s)),
                _ => None,
            })
            .collect();
        script.sort_by_key(|s| s.0);
        let objects = doc
            .list(DEF_ENTRIES)
            .iter()
            .enumerate()
            .map(|(i, e)| match e {
                Value::Struct(e) => {
                    let props = script.binary_search_by_key(&i, |s| s.0).ok().map(|j| script[j].1);
                    Ok(object(e, props))
                }
                _ => Err(Error::NotAMapVariant),
            })
            .collect::<Result<_, _>>()?;
        Ok(MapVariant { level: TagId(level as u32), objects })
    }

    /// Its spawn points, as the hints that fit a film's coordinates on a
    /// level the library has no box for take them:
    /// `Hints::new().spawn_pads(map.spawn_pads())`.
    pub fn spawn_pads(&self) -> impl Iterator<Item = SpawnPad> + '_ {
        self.objects.iter().filter_map(|o| {
            let pad = SpawnPad::new(o.position, o.spawn?);
            Some(match o.team {
                Some(t) => pad.for_team(t),
                None => pad,
            })
        })
    }
}

/// One `DefinitionEntry`, with its script properties.
fn object(e: &Struct<'_>, script: Option<&Struct<'_>>) -> MapObject {
    let v3 = |id| {
        let s = e.strukt(id);
        [0, 1, 2].map(|i| s.and_then(|s| s.float(i)).unwrap_or(0.0))
    };
    let [x, y, z] = engine_to_world(v3(ENT_POSITION));
    let object_type = ObjectType::from_raw(e.int(ENT_TYPE).and_then(|t| u8::try_from(t).ok()).unwrap_or(0));
    let empty = Struct::default();
    let props = e.strukt(ENT_PROPERTIES).unwrap_or(&empty);
    let base = props.first(SLOT_BASE).unwrap_or(&empty);
    let default = props.first(SLOT_DEFAULT).unwrap_or(&empty);
    let forward = world_axes(v3(ENT_FORWARD));
    MapObject {
        tag: TagId(e.int(ENT_TAG).unwrap_or(0) as u32),
        object_type,
        position: Position { x, y, z: Some(z) },
        forward,
        up: world_axes(v3(ENT_UP)),
        team: team(base),
        labels: base
            .list(BASE_LABELS)
            .iter()
            .filter_map(|l| match l {
                Value::Struct(l) => Some(StringId(l.int(0).unwrap_or(0) as u32)),
                _ => None,
            })
            .collect(),
        shape: volume(props, forward[1].atan2(forward[0])),
        spawn_order: base.int(BASE_SPAWN_ORDER).and_then(|v| u32::try_from(v).ok()),
        variant: default.first(DEFAULT_VARIANT).and_then(|v| v.int(0)).map(|v| StringId(v as u32)).filter(|v| v.0 != 0),
        respawn_time: default.int(DEFAULT_RESPAWN_TIME).and_then(|v| u32::try_from(v).ok()),
        spawn: (object_type == ObjectType::SpawnPoint).then(|| {
            match props.first(SLOT_SPAWN_POINT).and_then(|s| s.bool(SPAWN_INITIAL)) {
                Some(false) => SpawnKind::Respawn,
                _ => SpawnKind::Initial,
            }
        }),
        script: script.map(script_values).unwrap_or_default(),
    }
}

/// The team an object's `BaseProperties` give it.
fn team(base: &Struct<'_>) -> Option<TeamId> {
    base.int(BASE_TEAM).and_then(|t| u8::try_from(t).ok()).map(TeamId)
}

/// The volume an object's properties (its `PropertyStorage`) give it, in
/// world units, its forward axis at `angle`, with the object's team: what it
/// covers, its height above its centre and its depth below. None for a shape
/// other than a cylinder or a box.
///
/// A film carries the same struct for a zone it places, so its zones read
/// their shapes with this too.
pub(crate) fn volume(properties: &Struct<'_>, angle: f64) -> Option<ZoneShape> {
    let base = properties.first(SLOT_BASE)?;
    let b = base.first(BASE_BOUNDARY)?;
    let [a, w, top, bottom] =
        BOUNDARY_EXTENTS.map(|id| b.int(id).map_or(0.0, |v| v as f64 / 65536.0 * WORLD_PER_ENGINE));
    let area = match b.int(BOUNDARY_SHAPE)? {
        2 => ZoneArea::Cylinder { radius: a },
        3 => ZoneArea::Box { length: a, width: w },
        _ => return None,
    };
    Some(ZoneShape { area, top, bottom, angle, team: team(base) })
}

/// A `ValueCollection`'s properties: its flags, numbers, string ids and
/// tags, each in the file's order.
fn script_values(s: &Struct<'_>) -> Vec<(StringId, ScriptValue)> {
    let mut out = Vec::new();
    for (kind, id) in SCRIPT_MAPS.into_iter().enumerate() {
        for (k, v) in s.map(id) {
            let Value::Int(k) = k else { continue };
            let v = match (kind, v) {
                (0, Value::Bool(b)) => ScriptValue::Bool(*b),
                (1, Value::Float(f)) => ScriptValue::Number(*f),
                (2, Value::Int(i)) => ScriptValue::String(StringId(*i as u32)),
                (3, Value::Struct(t)) => ScriptValue::Tag(TagId(t.int(0).unwrap_or(0) as u32)),
                _ => continue,
            };
            out.push((StringId(*k as u32), v));
        }
    }
    out
}

/// A game mode's objective on a map: a flag stand, a capture plate, a zone,
/// a hill, a ball stand, a bomb or a bomb site.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Objective {
    /// The modes it serves, in [`ObjectiveMode`]'s order: one object can be
    /// a King of the Hill hill and a Strongholds zone.
    pub modes: Vec<ObjectiveMode>,
    /// What it is.
    pub kind: ObjectiveKind,
    /// Where it stands.
    pub position: Position,
    /// The team it belongs to.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub team: Option<TeamId>,
    /// Its volume: every zone has one, and others may.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub shape: Option<ZoneShape>,
    /// A King of the Hill hill's place in the rotation.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub order: Option<u32>,
    /// Its object, an index into [`MapVariant::objects`].
    pub object: usize,
}

/// A game mode an [`Objective`] serves, by its labels' names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ObjectiveMode {
    /// Capture the Flag: `flag_spawn`, `flag_delivery`.
    CaptureTheFlag,
    /// King of the Hill, which the game calls Bastion: `bastion_zone`,
    /// `bastion_plate`.
    KingOfTheHill,
    /// Strongholds: `strongholds_zone`.
    Strongholds,
    /// Land Grab: `landgrab_zone`.
    LandGrab,
    /// Total Control: `totalcontrol_zone`.
    TotalControl,
    /// Extraction: `extraction_zone`.
    Extraction,
    /// Oddball: `oddball_spawn`.
    Oddball,
    /// Assault: `assault_bomb_spawn`, `assault_site`.
    Assault,
}

/// What an [`Objective`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ObjectiveKind {
    /// Where a team's flag stands.
    FlagSpawn,
    /// Where a team delivers the flag.
    FlagPlate,
    /// A zone or a hill.
    Zone,
    /// Where the ball spawns.
    BallStand,
    /// Where the bomb spawns.
    BombSpawn,
    /// A site to plant the bomb at.
    BombSite,
}

/// What a label makes an object: an objective of a kind, a hill, or a King
/// of the Hill plate (which only orders the hills).
#[derive(Clone, Copy, PartialEq)]
enum Role {
    Is(ObjectiveKind),
    Hill,
    Plate,
}

/// The labels that make objectives, by the names whose string ids map
/// variants carry.
const ROLES: [(&str, ObjectiveMode, Role); 11] = [
    ("flag_spawn", ObjectiveMode::CaptureTheFlag, Role::Is(ObjectiveKind::FlagSpawn)),
    ("flag_delivery", ObjectiveMode::CaptureTheFlag, Role::Is(ObjectiveKind::FlagPlate)),
    ("bastion_zone", ObjectiveMode::KingOfTheHill, Role::Hill),
    ("bastion_plate", ObjectiveMode::KingOfTheHill, Role::Plate),
    ("strongholds_zone", ObjectiveMode::Strongholds, Role::Is(ObjectiveKind::Zone)),
    ("landgrab_zone", ObjectiveMode::LandGrab, Role::Is(ObjectiveKind::Zone)),
    ("totalcontrol_zone", ObjectiveMode::TotalControl, Role::Is(ObjectiveKind::Zone)),
    ("extraction_zone", ObjectiveMode::Extraction, Role::Is(ObjectiveKind::Zone)),
    ("oddball_spawn", ObjectiveMode::Oddball, Role::Is(ObjectiveKind::BallStand)),
    ("assault_bomb_spawn", ObjectiveMode::Assault, Role::Is(ObjectiveKind::BombSpawn)),
    ("assault_site", ObjectiveMode::Assault, Role::Is(ObjectiveKind::BombSite)),
];

/// The custom games' labels whose names are known: an object with one
/// belongs to a minigame, not to a mode.
const MINIGAME_LABELS: [&str; 4] =
    ["minigame_megalo_object_1", "minigame_megalo_object_2", "minigame_megalo_object_3", "minigame_megalo_object_5"];

/// The Forge object King of the Hill's hills are; other objects labelled
/// `bastion_zone` belong to custom games.
const KOTH_HILL: &str = "KOTH Capture Zone";

/// How far from its hill a King of the Hill plate may stand, world units.
const PLATE_REACH: f64 = 40.0;

impl MapVariant {
    /// The objectives the map places for each game mode, in its objects'
    /// order.
    ///
    /// An object's Forge labels make it an objective: `flag_spawn` makes a
    /// Capture the Flag flag stand, `strongholds_zone` a Strongholds zone, and so on
    /// ([`ObjectiveMode`] lists them). One object can serve several modes. An
    /// object with a minigame's label is left out, as is a zone without a
    /// volume. A King of the Hill hill is a `KOTH Capture Zone` object
    /// labelled `bastion_zone`; its place in the rotation is the spawn order
    /// of the plate (`bastion_plate`) nearest it within 40 world units, and
    /// the plates themselves are left out.
    pub fn objectives(&self) -> Vec<Objective> {
        let roles = ROLES.map(|(name, mode, role)| (StringId::of(name), mode, role));
        let minigame = MINIGAME_LABELS.map(StringId::of);
        let mut out: Vec<Objective> = Vec::new();
        let mut plates = Vec::new();
        for (i, o) in self.objects.iter().enumerate() {
            if o.labels.iter().any(|l| minigame.contains(l)) {
                continue;
            }
            let is_hill = o.name() == Some(KOTH_HILL);
            let mine: Vec<(ObjectiveMode, Role)> = o
                .labels
                .iter()
                .filter_map(|l| roles.iter().find(|r| r.0 == *l))
                .filter(|r| r.2 != Role::Hill || is_hill)
                .map(|r| (r.1, r.2))
                .collect();
            let Some(&(_, first)) = mine.first() else { continue };
            let mut modes: Vec<ObjectiveMode> = mine.iter().map(|r| r.0).collect();
            modes.sort();
            modes.dedup();
            let has = |f: fn(Role) -> bool| mine.iter().any(|r| f(r.1));
            let kind = if has(|r| matches!(r, Role::Hill | Role::Is(ObjectiveKind::Zone))) {
                ObjectiveKind::Zone
            } else if has(|r| r == Role::Plate) {
                if modes.contains(&ObjectiveMode::KingOfTheHill) {
                    plates.push((o.position, o.spawn_order));
                }
                continue;
            } else {
                match first {
                    Role::Is(k) => k,
                    Role::Hill | Role::Plate => continue,
                }
            };
            let team = o.team.filter(|t| t.0 < 8);
            let shape = o.shape.map(|b| ZoneShape { team, ..b });
            if kind == ObjectiveKind::Zone && shape.is_none() {
                continue;
            }
            out.push(Objective { modes, kind, position: o.position, team, shape, order: None, object: i });
        }
        for (at, order) in plates {
            let Some(order) = order else { continue };
            let near = out
                .iter_mut()
                .filter(|h| h.kind == ObjectiveKind::Zone && h.modes.contains(&ObjectiveMode::KingOfTheHill))
                .map(|h| {
                    let (dx, dy) = (h.position.x - at.x, h.position.y - at.y);
                    (dx * dx + dy * dy, h)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, hill)) = near.filter(|n| n.0 < PLATE_REACH * PLATE_REACH) {
                hill.order = Some(order);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Bond value to write.
    enum B {
        Bool(bool),
        U8(u8),
        U16(u16),
        U32(u32),
        I32(i32),
        F(f32),
        S(Vec<(u16, B)>),
        /// A list: the elements' type, the elements.
        L(u8, Vec<B>),
        /// A map: the keys' and the values' types, the pairs.
        M(u8, u8, Vec<(B, B)>),
    }

    fn uleb(mut n: u64, out: &mut Vec<u8>) {
        while n >= 0x80 {
            out.push(n as u8 | 0x80);
            n >>= 7;
        }
        out.push(n as u8);
    }

    fn ty(v: &B) -> u8 {
        match v {
            B::Bool(_) => 2,
            B::U8(_) => 3,
            B::U16(_) => 4,
            B::U32(_) => 5,
            B::F(_) => 7,
            B::S(_) => 10,
            B::L(..) => 11,
            B::M(..) => 13,
            B::I32(_) => 16,
        }
    }

    fn write(v: &B, out: &mut Vec<u8>) {
        match v {
            B::Bool(b) => out.push(u8::from(*b)),
            B::U8(b) => out.push(*b),
            B::U16(n) => uleb(u64::from(*n), out),
            B::U32(n) => uleb(u64::from(*n), out),
            B::I32(n) => uleb(u64::from(((n << 1) ^ (n >> 31)) as u32), out),
            B::F(f) => out.extend(f.to_le_bytes()),
            B::S(fields) => {
                let mut body = Vec::new();
                for (id, v) in fields {
                    match u8::try_from(*id) {
                        Ok(i) if i < 6 => body.push(i << 5 | ty(v)),
                        Ok(i) => body.extend([6 << 5 | ty(v), i]),
                        Err(_) => body.extend([7 << 5 | ty(v), id.to_le_bytes()[0], id.to_le_bytes()[1]]),
                    }
                    write(v, &mut body);
                }
                body.push(0);
                uleb(body.len() as u64, out);
                out.extend(body);
            }
            B::L(e, items) => {
                if items.len() < 6 {
                    out.push((items.len() as u8 + 1) << 5 | e);
                } else {
                    out.push(*e);
                    uleb(items.len() as u64, out);
                }
                items.iter().for_each(|i| write(i, out));
            }
            B::M(k, v, pairs) => {
                out.extend([*k, *v]);
                uleb(pairs.len() as u64, out);
                for (a, b) in pairs {
                    write(a, out);
                    write(b, out);
                }
            }
        }
    }

    /// A struct `{0: id}` of the string id of `name`, as labels and variants
    /// are.
    fn named(name: &str) -> B {
        B::S(vec![(0, B::I32(StringId::of(name).0 as i32))])
    }

    fn v3(v: [f32; 3]) -> B {
        B::S(vec![(0, B::F(v[0])), (1, B::F(v[1])), (2, B::F(v[2]))])
    }

    /// A `DefinitionEntry`: its Forge object, place, forward axis, type and
    /// properties by slot.
    fn entry(tag: u32, at: [f32; 3], forward: [f32; 3], ty: u8, slots: Vec<(u16, Vec<(u16, B)>)>) -> B {
        let slots = slots.into_iter().map(|(slot, s)| (slot, B::L(10, vec![B::S(s)]))).collect();
        B::S(vec![
            (ENT_TAG, B::S(vec![(0, B::I32(tag as i32))])),
            (ENT_POSITION, v3(at)),
            (ENT_UP, v3([0.0, 0.0, 1.0])),
            (ENT_FORWARD, v3(forward)),
            (ENT_TYPE, B::U8(ty)),
            (ENT_PROPERTIES, B::S(slots)),
        ])
    }

    /// Engine units as a boundary's 1/65536 fixed point.
    fn fixed(v: f64) -> B {
        B::S(vec![(0, B::I32((v * 65536.0) as i32))])
    }

    #[test]
    fn reads_a_map_variant() {
        let respawn = entry(
            0xa495fe83,
            [1.0, 2.0, 3.0],
            [0.0, 1.0, 0.0],
            7,
            vec![(SLOT_BASE, vec![(BASE_TEAM, B::I32(1))]), (SLOT_SPAWN_POINT, vec![(SPAWN_INITIAL, B::Bool(false))])],
        );
        let initial = entry(0x1, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 7, vec![]);
        let volume = B::S(vec![
            (BOUNDARY_SHAPE, B::I32(3)),
            (5, fixed(2.5)),
            (6, fixed(1.0)),
            (7, fixed(1.5)),
            (8, fixed(0.25)),
        ]);
        let hill = entry(
            0xa7ff0c39,
            [-1.0, 0.5, 60.0],
            [1.0, 0.0, 0.0],
            1,
            vec![
                (
                    SLOT_BASE,
                    vec![(BASE_BOUNDARY, B::L(10, vec![volume])), (BASE_LABELS, B::L(10, vec![named("bastion_zone")]))],
                ),
                (
                    SLOT_DEFAULT,
                    vec![(DEFAULT_VARIANT, B::L(10, vec![named("default")])), (DEFAULT_RESPAWN_TIME, B::U16(45))],
                ),
            ],
        );
        let plate = entry(
            0x2,
            [-1.0, 0.6, 60.0],
            [1.0, 0.0, 0.0],
            1,
            vec![(
                SLOT_BASE,
                vec![(BASE_LABELS, B::L(10, vec![named("bastion_plate")])), (BASE_SPAWN_ORDER, B::U8(4))],
            )],
        );
        let id = |name: &str| B::U32(StringId::of(name).0);
        let script = B::S(vec![
            (1, B::M(5, 7, vec![(id("spawntime"), B::F(30.0))])),
            (2, B::M(5, 5, vec![(id("vehicle"), id("vehicle_warthog"))])),
            (3, B::M(5, 10, vec![(id("item"), B::S(vec![(0, B::I32(0x2b1824d5))]))])),
        ]);
        let doc = B::S(vec![
            (DEF_PROPERTIES, B::S(vec![(0, B::S(vec![(0, B::I32(0x054c5f41))]))])),
            (DEF_ENTRIES, B::L(10, vec![respawn, initial, hill, plate])),
            (DEF_SCRIPT_PROPERTIES, B::M(4, 10, vec![(B::U16(2), script)])),
        ]);
        let mut bytes = Vec::new();
        write(&doc, &mut bytes);
        let map = MapVariant::from_bytes(&bytes).unwrap();
        assert_eq!(map.level, TagId(0x054c5f41));
        let [respawn, initial, hill, _] = &map.objects[..] else { panic!("{:?}", map.objects) };

        assert_eq!(respawn.object_type, ObjectType::SpawnPoint);
        assert_eq!(respawn.name(), Some("Spawn Point [Respawn]"));
        // Engine (1, 2, 3) is world (-20, 10, 30); engine forward +y is
        // world -x.
        assert_eq!(respawn.position, Position { x: -20.0, y: 10.0, z: Some(30.0) });
        assert_eq!(respawn.forward, [-1.0, 0.0, 0.0]);
        assert_eq!(respawn.heading(), std::f64::consts::PI);
        assert_eq!((respawn.team, respawn.spawn), (Some(TeamId(1)), Some(SpawnKind::Respawn)));
        assert_eq!((initial.spawn, initial.team), (Some(SpawnKind::Initial), None));
        let pads: Vec<SpawnPad> = map.spawn_pads().collect();
        assert_eq!(pads.len(), 2);
        assert_eq!((pads[0].kind, pads[0].team), (SpawnKind::Respawn, Some(TeamId(1))));

        assert_eq!(hill.name(), Some("KOTH Capture Zone"));
        assert_eq!(hill.labels, [StringId::of("bastion_zone")]);
        assert_eq!(hill.variant, Some(StringId::of("default")));
        assert_eq!(hill.respawn_time, Some(45));
        assert_eq!(hill.spawn, None);
        let shape = hill.shape.unwrap();
        assert_eq!(shape.area, ZoneArea::Box { length: 25.0, width: 10.0 });
        assert_eq!((shape.top, shape.bottom, shape.angle), (15.0, 2.5, std::f64::consts::FRAC_PI_2));
        assert_eq!(hill.script_property(StringId::of("spawntime")), Some(ScriptValue::Number(30.0)));
        let vehicle = hill.script_property(StringId::of("vehicle"));
        assert_eq!(vehicle, Some(ScriptValue::String(StringId::of("vehicle_warthog"))));
        assert_eq!(hill.script_property(StringId::of("item")), Some(ScriptValue::Tag(TagId(0x2b1824d5))));
        assert!(respawn.script.is_empty());

        // The plate gives the hill its place in the rotation.
        let objectives = map.objectives();
        assert_eq!(objectives.len(), 1);
        let o = &objectives[0];
        assert_eq!(
            (o.object, o.kind, &o.modes[..], o.order),
            (2, ObjectiveKind::Zone, &[ObjectiveMode::KingOfTheHill][..], Some(4))
        );
    }

    #[test]
    fn other_bytes_are_refused() {
        assert_eq!(MapVariant::from_bytes(&[]), Err(Error::NotAMapVariant));
        assert_eq!(MapVariant::from_bytes(&[0x00]), Err(Error::NotAMapVariant));
        assert_eq!(MapVariant::from_bytes(b"not a map variant"), Err(Error::NotAMapVariant));
        // Objects that aren't structs.
        let doc = B::S(vec![(DEF_PROPERTIES, B::S(vec![(0, B::I32(1))])), (DEF_ENTRIES, B::L(16, vec![B::I32(1)]))]);
        let mut bytes = Vec::new();
        write(&doc, &mut bytes);
        assert_eq!(MapVariant::from_bytes(&bytes), Err(Error::NotAMapVariant));
    }

    /// An object at world (x, y) with labels, as the objectives see it.
    fn placed(tag: u32, x: f64, y: f64, labels: &[&str]) -> MapObject {
        MapObject {
            tag: TagId(tag),
            object_type: ObjectType::Default,
            position: Position { x, y, z: Some(0.0) },
            forward: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            team: None,
            labels: labels.iter().map(|l| StringId::of(l)).collect(),
            shape: None,
            spawn_order: None,
            variant: None,
            respawn_time: None,
            spawn: None,
            script: Vec::new(),
        }
    }

    fn volume(cylinder: bool) -> Option<ZoneShape> {
        let area =
            if cylinder { ZoneArea::Cylinder { radius: 10.0 } } else { ZoneArea::Box { length: 20.0, width: 8.0 } };
        Some(ZoneShape { area, top: 5.0, bottom: 1.0, angle: 0.0, team: None })
    }

    #[test]
    fn objectives_by_label() {
        const KOTH: u32 = 0xa7ff0c39;
        let flag = MapObject { team: Some(TeamId(0)), ..placed(1, 0.0, -160.0, &["flag_spawn"]) };
        // A team past the eighth is none.
        let plate = MapObject { team: Some(TeamId(9)), ..placed(2, 0.0, 160.0, &["flag_delivery"]) };
        let no_volume = placed(3, 50.0, 50.0, &["strongholds_zone"]);
        let hill_a =
            MapObject { shape: volume(false), ..placed(KOTH, 0.0, 0.0, &["bastion_zone", "strongholds_zone"]) };
        let hill_b = MapObject { shape: volume(true), ..placed(KOTH, 100.0, 0.0, &["bastion_zone"]) };
        // Labelled a hill, but not a hill's object: a custom game's.
        let banner = MapObject { shape: volume(true), ..placed(4, 200.0, 0.0, &["bastion_zone"]) };
        let plate_a = MapObject { spawn_order: Some(2), ..placed(5, 5.0, 0.0, &["bastion_plate"]) };
        let too_far = MapObject { spawn_order: Some(3), ..placed(5, 100.0, 45.0, &["bastion_plate"]) };
        let plate_b = MapObject { spawn_order: Some(5), ..placed(5, 98.0, 1.0, &["bastion_plate"]) };
        let minigame = placed(6, 0.0, 10.0, &["oddball_spawn", "minigame_megalo_object_1"]);
        // The first label's kind, every label's modes.
        let site = placed(7, -30.0, 0.0, &["assault_site", "flag_spawn"]);
        let unknown = placed(8, 0.0, 0.0, &["not_a_mode_label"]);
        let objects =
            vec![flag, plate, no_volume, hill_a, hill_b, banner, plate_a, too_far, plate_b, minigame, site, unknown];
        let map = MapVariant { level: TagId(1), objects };
        let got: Vec<_> = map.objectives().into_iter().map(|o| (o.object, o.kind, o.modes, o.team, o.order)).collect();
        use ObjectiveKind::*;
        use ObjectiveMode::*;
        assert_eq!(
            got,
            [
                (0, FlagSpawn, vec![CaptureTheFlag], Some(TeamId(0)), None),
                (1, FlagPlate, vec![CaptureTheFlag], None, None),
                (3, Zone, vec![KingOfTheHill, Strongholds], None, Some(2)),
                (4, Zone, vec![KingOfTheHill], None, Some(5)),
                (10, BombSite, vec![CaptureTheFlag, Assault], None, None),
            ]
        );
        assert_eq!(map.objectives()[2].shape, volume(false));
    }

    #[test]
    fn label_names_hash_to_the_ids_map_variants_carry() {
        assert_eq!(StringId::of("flag_spawn"), StringId(0x590852e1));
        assert_eq!(StringId::of("bastion_zone"), StringId(0xd239d61f));
        assert_eq!(StringId::of("bastion_plate"), StringId(0xa7a5de0f));
        assert_eq!(StringId::of("assault_bomb_spawn"), StringId(0x4ebbcf02));
        assert_eq!(StringId::of("minigame_megalo_object_5"), StringId(0x3f8076ca));
    }
}
