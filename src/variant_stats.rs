//! The game variant's stat definitions and engine settings, from the same
//! Bond data in chunk 0 as `variant.rs`. `BaseVariantData` is read to its
//! end: 1 options, 2 intro and UI, 3 data dependencies, 4 gameplay
//! miscellany, 5 engine miscellany; the field ids are the game's
//! (`i343.NetProtocol.GameOptions`, `.GameStats`).

use crate::bond::{self, Struct, Value};
use crate::variant;

/// Bytes of chunk 0 searched, from where the variant's structures start:
/// through the roster, past the end of a variant with a long script.
const REGION_LEN: usize = 0x110000;

/// One statborg slot's definition, `EngineStatDefinition`. A field at its
/// default isn't written, so these default to 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatDefinition {
    /// m_statIdentifierRawString, "AccuracyPercentage".
    pub name: String,
    /// m_statIdentifier: murmur3 of `name`.
    pub id: u32,
    /// m_baseEntryType (2: a best-of, 3: computed).
    pub base: i64,
    /// m_matchValueAggregationType, how a match's value comes from its rounds.
    pub aggregation: i64,
    /// m_sortDirection: -1 lower is better.
    pub sort: i64,
    /// m_displayType: 0 a number, 1 a time, 2 a percentage.
    pub display: i64,
    /// m_storedValueDecimalPrecision: the stored value is 10^n times the shown.
    pub precision: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Definitions {
    /// By statborg slot.
    pub stats: Vec<StatDefinition>,
    /// The variant's scoreboard columns (m_scoreboardStatEntries): slots.
    pub scoreboard: Vec<usize>,
    /// m_limitServerFramerateTo30FPS: the film runs at 30 Hz.
    pub server_30fps: bool,
    /// m_tickRate, when the variant writes it.
    pub tick_rate: Option<i64>,
    /// PlaybackSettings, each when written: killcamEnabled,
    /// timeWindowForKillCamSeconds, playOfTheGameEnabled, machinimaControlsEnabled.
    pub kill_cam: Option<bool>,
    pub kill_cam_seconds: Option<i64>,
    pub play_of_the_game: Option<bool>,
    pub machinima: Option<bool>,
    /// RespawnOptions.m_overrideSpawnSettingsTag: an `ssdf` tag id, when the
    /// variant overrides the global default.
    pub spawn_settings: Option<u32>,
}

/// A number field: an integer, a flag, or a float cut to an integer.
fn num(s: &Struct<'_>, id: u16) -> Option<i64> {
    match s.get(id)? {
        Value::Int(v) => Some(*v),
        Value::Bool(b) => Some(i64::from(*b)),
        Value::Float(v) => Some(*v as i64),
        _ => None,
    }
}

pub fn read(chunk0: &[u8]) -> Option<Definitions> {
    let end = chunk0.len().min(variant::START + REGION_LEN);
    let (b, _) = variant::aligned(chunk0.get(variant::START..end)?)?;
    // `BaseVariantData` after its metadata, read to its end.
    let v = bond::fields(&b[variant::settings_start(&b)?..]).ok()?;
    let empty = Struct::default();
    let defs = v.strukt(3).and_then(|s| s.strukt(21)).filter(|s| s.get(0).is_some())?;
    let stats: Vec<StatDefinition> = defs
        .list(0)
        .iter()
        .map(|s| {
            let s = match s {
                Value::Struct(s) => s,
                _ => &empty,
            };
            StatDefinition {
                name: match s.get(15) {
                    Some(Value::Str(t)) => String::from_utf8_lossy(t).into_owned(),
                    _ => String::new(),
                },
                id: num(s, 0).unwrap_or(0) as u32,
                base: num(s, 1).unwrap_or(0),
                aggregation: num(s, 3).unwrap_or(0),
                sort: num(s, 12).unwrap_or(0),
                display: num(s, 13).unwrap_or(0),
                precision: num(s, 17).unwrap_or(0),
            }
        })
        .collect();
    let options = v.strukt(1);
    let scoreboard = v
        .strukt(2)
        .map_or(&[][..], |s| s.list(30))
        .iter()
        .filter_map(|e| {
            let id = match e {
                Value::Struct(e) => num(e, 0),
                _ => None,
            };
            stats.iter().position(|s| s.id == id.unwrap_or(0) as u32)
        })
        .collect();
    let engine = v.strukt(5);
    let playback = options.and_then(|o| o.strukt(0)).and_then(|o| o.strukt(5));
    let flag = |id| playback.and_then(|p| num(p, id)).map(|x| x != 0);
    Some(Definitions {
        stats,
        scoreboard,
        server_30fps: engine.and_then(|e| num(e, 0)).is_some_and(|x| x != 0),
        tick_rate: engine.and_then(|e| num(e, 7)),
        kill_cam: flag(0),
        kill_cam_seconds: playback.and_then(|p| num(p, 1)),
        play_of_the_game: flag(2),
        machinima: flag(3),
        spawn_settings: options
            .and_then(|o| o.strukt(2))
            .and_then(|r| r.strukt(18))
            .and_then(|s| num(s, 0))
            .map(|t| t as u32),
    })
}
