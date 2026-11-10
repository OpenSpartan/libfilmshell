//! Types shared by the extraction stages.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EncodingMode {
    PerEntity,
    BotComposite,
    MultiplayerComposite,
}

impl EncodingMode {}

/// One decoded position sample of one life (raw window units).
#[derive(Clone, Copy, Debug)]
pub struct RawSample {
    /// Global byte position of the sample's frame (sort key).
    pub gp: i64,
    /// Frame index, not game time (`Film::frame_ticks` maps it to ticks).
    pub tick: i64,
    pub c1: f64,
    pub c2: f64,
    /// Spliced in from a vehicle (`splice_vehicle_rides`).
    pub in_vehicle: bool,
    /// The vehicle it was spliced from, its id `(generation - 1) * 256 + index`
    /// (a part's rides use its vehicle's); -1 when none.
    pub vehicle: i32,
    /// A full biped update whose item list marks AI control.
    pub ai: bool,
    /// A stationary tick held through a stub run (`hold_through_stubs`).
    pub held: bool,
}

impl RawSample {
    pub fn new(gp: i64, tick: i64, c1: f64, c2: f64) -> RawSample {
        RawSample { gp, tick, c1, c2, in_vehicle: false, vehicle: -1, ai: false, held: false }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MotionPoint {
    pub frame: i64,
    pub raw1: f64,
    pub raw2: f64,
    pub gp: i64,
    pub in_vehicle: bool,
    /// RawSample::vehicle.
    pub vehicle: i32,
}

#[derive(Clone, Debug)]
pub struct PlayerPath {
    pub player_index: i32,
    pub is_bot: bool,
    pub team_id: Option<f64>,
    pub positions: Vec<MotionPoint>,
    /// Slot ids of the lives concatenated into `positions`.
    pub life_slots: Option<Vec<i32>>,
    /// Index into `positions` of each life's first sample.
    pub life_starts: Option<Vec<usize>>,
}

#[derive(Clone, Debug)]
pub struct DeathPosition {
    pub slot_id: i32,
    pub gp: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibSource {
    LevelTable,
    ForgeCanvas,
    AssetTable,
    SpawnFit,
    ExtentFit,
    TwoSpawn,
    SingleSpawn,
}

impl CalibSource {}

/// `world y = scale_y * raw1 + offset_y`, `world x = -scale_x * raw2 + offset_x`.
#[derive(Clone, Copy, Debug)]
pub struct Calibration {
    pub scale_y: f64,
    pub scale_x: f64,
    pub offset_y: f64,
    pub offset_x: f64,
    pub source: CalibSource,
    pub spawn_inliers: Option<usize>,
    pub spawn_samples: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct WorldPosition {
    pub x: f64,
    pub y: f64,
    pub frame: i64,
    pub in_vehicle: bool,
    /// RawSample::vehicle.
    pub vehicle: i32,
}

/// What life assignment needs to know of a player.
#[derive(Clone, Debug, Default)]
pub struct RosterEntry {
    pub bot: bool,
    pub last_team_id: Option<f64>,
    /// The XUID, decimal.
    pub xuid: Option<String>,
    pub present_at_beginning: Option<bool>,
    pub left_in_progress: Option<bool>,
    pub deaths: Option<f64>,
    pub spawns: Option<f64>,
}

impl RosterEntry {
    pub fn is_bot(&self) -> bool {
        self.bot
    }
}

/// Player identity from chunk 0's roster table.
#[derive(Clone, Debug)]
pub struct RosterIdentity {
    pub xuid: String,
    pub gamertag: String,
}

#[derive(Clone, Copy, Debug)]
pub struct MapBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub width: f64,
}
