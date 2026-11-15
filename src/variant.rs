//! The game variant as the film carries it: chunk 0 holds it as Bond Compact
//! Binary at a bit offset, read with the crate's Bond reader once shifted onto
//! byte boundaries (`aligned`). Its strings are one blob, NUL-separated, 18
//! languages per string with English first: the name, the description,
//! score text, then the scoreboard's labels in slot order.

use crate::bond::{self, Struct, Value};

/// Where the variant's structures start in chunk 0.
pub(crate) const START: usize = 0xce680;
/// Entries per string: one per language.
const LANGUAGES: usize = 18;
/// The slot after the fixed labels, where a mode's own labels start. One
/// entry of another kind ("Alpha") sits before them.
const MODE_SLOT: usize = 41;
const ANCHOR: &[u8] = b"Rounds Won\0";
/// The path that closes the header struct; the settings follow it.
const MODE_PATH: &[u8] = b"Multiplayer/";
/// The settings' fields, as paths of Bond field ids, named by the game's own
/// serializers: the options struct (1) holds m_respawn_options (2),
/// m_loadout_options (6) and m_match_flow_options (7), and their fields are
/// listed by id with their names.
/// m_respawn_options: min_respawn_time_seconds, suicide_penalty_seconds,
/// betrayal_penalty_seconds.
const RESPAWN: &[u32] = &[1, 2, 3];
const SUICIDE_PENALTY: &[u32] = &[1, 2, 5];
const BETRAYAL_PENALTY: &[u32] = &[1, 2, 6];
/// m_match_flow_options: scoreToWinRound, roundTimeLimitSeconds,
/// overtimeDurationSeconds, lastChanceEnabledInOvertime,
/// lastChanceDurationSeconds.
const SCORE_TO_WIN: &[u32] = &[1, 7, 0];
const TIME_LIMIT: &[u32] = &[1, 7, 3];
const OVERTIME: &[u32] = &[1, 7, 9];
const LAST_CHANCE_IN_OVERTIME: &[u32] = &[1, 7, 10];
const LAST_CHANCE: &[u32] = &[1, 7, 11];
/// The first loadout's weapons (a list: one path for every element), each a
/// weapon tag id, or a flag for a random weapon; and its equipment, a soft
/// asset's tag id. In the game's names: m_loadout_options (6) / m_loadouts (1),
/// whose entries hold m_identifier 0, m_weapons 1, m_equipment 2,
/// m_grenades 3 and m_localizedNameIndex 4, in their metadata's order.
const LOADOUT_WEAPON: &[u32] = &[1, 6, 1, 1, 0, 0];
const LOADOUT_RANDOM: &[u32] = &[1, 6, 1, 1, 2];
const LOADOUT_EQUIPMENT: &[u32] = &[1, 6, 1, 2, 0, 0];
/// More of the options, each named by the game's Bond writer for its field.
/// A field at its default isn't written. Floats are in thousandths (`walk`).
const RULES: &[(&[u32], &str)] = &[
    (&[1, 0, 1, 6], "droppedWeaponsSeconds"),
    (&[1, 0, 4, 2], "botsTeam1"),
    (&[1, 0, 4, 5], "botsTeam2"),
    (&[1, 2, 22, 0], "spawnProtectionMs"),
    (&[1, 7, 5], "earlyVictoryRounds"),
    (&[1, 7, 17], "roundLimit"),
    (&[1, 7, 18], "swapSides"),
    (&[1, 10, 0, 11, 11], "randomWeapons"),
    (&[1, 10, 1, 8, 12], "randomEquipment"),
    (&[1, 10, 2, 10, 10], "randomVehicles"),
];
/// The variant's option table: each entry the murmur3 of an option's lowercased
/// name, and 8 bytes of value (`Variant::options`).
const OPTION_HASH: &[u32] = &[3, 9, 0, 0, 0];
const OPTION_VALUE: &[u32] = &[3, 9, 0, 1, 0];
/// The header struct's m_iconIndex: the mode's image in the game's sheet of
/// mode icons. Not written when 0.
const ICON_INDEX: &[u32] = &[10];
/// Byte 4 of an option's value: its type.
const OPTION_FLOAT: u8 = 3;
const OPTION_BOOL: u8 = 2;
/// Flag sets, each element a bit index, named by the mask of the game's
/// exported `ManagedGameVariant_Set*` setter for it. A flag not listed is off.
const FLAGS: &[(&[u32], i64, &str)] = &[
    (&[1, 2, 0], 4, "earlyRespawn"),
    (&[1, 3, 0], 3, "friendlyFire"),
    (&[1, 3, 0], 10, "friendlyCollisions"),
    (&[1, 4, 0], 0, "grenadesOnMap"),
    (&[1, 4, 0], 2, "equipmentOnMap"),
    (&[1, 4, 0], 3, "powerupsOnMap"),
    (&[1, 4, 0], 4, "turretsOnMap"),
    (&[1, 4, 0], 5, "weaponsOnMap"),
    (&[1, 4, 0], 6, "vehiclesOnMap"),
    (&[1, 4, 0], 8, "weaponPadsOnMap"),
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Variant {
    pub name: String,
    pub description: String,
    /// The scoreboard's labels, by slot.
    pub stats: Vec<String>,
    /// Seconds from death to respawn (min_respawn_time_seconds).
    pub respawn: Option<i64>,
    pub score_to_win: Option<i64>,
    /// The round's time limit, seconds.
    pub time_limit: Option<i64>,
    /// The starting weapons' tag ids.
    pub loadout: Vec<u32>,
    /// Starting weapons are random.
    pub loadout_random: bool,
    /// Seconds added to the respawn after a suicide, and after a betrayal.
    pub suicide_penalty: Option<i64>,
    pub betrayal_penalty: Option<i64>,
    /// Seconds of overtime.
    pub overtime: Option<i64>,
    /// Seconds of last chance, when overtime has it.
    pub last_chance: Option<i64>,
    /// The starting equipment's soft asset tag id.
    pub equipment: Option<u32>,
    /// `RULES` the variant writes, in that order.
    pub rules: Vec<(&'static str, i64)>,
    /// `FLAGS` that are on, in that order.
    pub flags: Vec<&'static str>,
    /// The options the variant sets: each name's hash, and its value, in
    /// hash order.
    pub options: Vec<(u32, OptionValue)>,
    /// The header's icon index; None when the header wasn't found.
    pub icon: Option<u8>,
}

pub use crate::game::OptionValue;

/// An option's 8 bytes (packed little-endian by `walk`): the value in the
/// first four, the type in the fifth; the last three vary for equal values.
fn option_value(raw: i64) -> Option<OptionValue> {
    let b = raw.to_le_bytes();
    match b[4] {
        OPTION_FLOAT => {
            let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64;
            Some(OptionValue::Number((v * 1000.0).round() / 1000.0))
        }
        OPTION_BOOL => Some(OptionValue::Bool(b[0] != 0)),
        _ => None,
    }
}

/// Calls `f` on every number in `s` with the path of field ids to it:
/// integers and flags as they are, floats in thousandths, and a list of 8
/// bytes (an option's value) as one little-endian integer. A list's elements,
/// and a map's keys and values, take the path of the field that holds them.
fn walk(s: &Struct<'_>, path: &mut Vec<u32>, f: &mut impl FnMut(&[u32], i64)) {
    for (id, v) in &s.fields {
        path.push(u32::from(*id));
        walk_value(v, path, f);
        path.pop();
    }
}

fn walk_value(v: &Value<'_>, path: &mut Vec<u32>, f: &mut impl FnMut(&[u32], i64)) {
    match v {
        Value::Bool(b) => f(path, i64::from(*b)),
        Value::Int(x) => f(path, *x),
        Value::Float(x) => f(path, (x * 1000.0).round() as i64),
        Value::Bytes(b) => {
            if let Ok(b) = <[u8; 8]>::try_from(*b) {
                f(path, i64::from_le_bytes(b));
            }
        }
        Value::Struct(s) => walk(s, path, f),
        Value::List(l) => l.iter().for_each(|v| walk_value(v, path, f)),
        Value::Map(m) => {
            for (k, v) in m {
                walk_value(k, path, f);
                walk_value(v, path, f);
            }
        }
        Value::Str(_) | Value::WStr(_) => {}
    }
}

/// Where the header struct's fields start: after the struct field (id 0) and
/// length that end exactly at `end`, the byte after its closing path's NUL.
fn header_start(b: &[u8], end: usize) -> Option<usize> {
    (end.saturating_sub(600)..end.saturating_sub(8)).rev().find_map(|s| {
        if b[s] != 0x0a {
            return None;
        }
        let (len, n) = bond::uleb(&b[s + 1..])?;
        let q = s + 1 + n;
        (q as u64 + len == end as u64).then_some(q)
    })
}

fn shifted(r: &[u8], s: u32) -> Vec<u8> {
    let at = |i: usize| r.get(i).copied().unwrap_or(0) as u32;
    (0..r.len()).map(|i| (((at(i) << 8 | at(i + 1)) << s) >> 8) as u8).collect()
}

/// `region`, from `START`, shifted by the bits that put the variant's Bond
/// data on byte boundaries (the shift at which `ANCHOR` reads), and where
/// the anchor is.
pub(crate) fn aligned(region: &[u8]) -> Option<(Vec<u8>, usize)> {
    (0..8).find_map(|s| {
        let b = shifted(region, s);
        let at = b.windows(ANCHOR.len()).position(|w| w == ANCHOR)?;
        Some((b, at))
    })
}

/// Where the path that closes the header struct is in aligned bytes.
fn mode_path(b: &[u8]) -> Option<usize> {
    b.windows(MODE_PATH.len()).position(|w| w == MODE_PATH)
}

/// Past the path at `at` and its NUL: where `BaseVariantData`'s fields
/// start.
fn past_path(b: &[u8], at: usize) -> Option<usize> {
    Some(at + b[at..].iter().position(|&c| c == 0)? + 1)
}

/// Where `BaseVariantData`'s fields start in aligned bytes.
pub(crate) fn settings_start(b: &[u8]) -> Option<usize> {
    past_path(b, mode_path(b)?)
}

/// The blob's first entry, after the varint that gives its length: the
/// varint's last byte can read as a letter, so a high byte before a run of
/// text marks one to drop.
fn text(b: &[u8]) -> Option<String> {
    let t = String::from_utf8_lossy(b);
    let at = t.rfind(|c: char| c.is_control() || c == '\u{fffd}');
    let tail = match at {
        Some(i) => {
            let c = t[i..].chars().next()?;
            let rest = &t[i + c.len_utf8()..];
            if c == '\u{fffd}' {
                &rest[rest.chars().next()?.len_utf8()..]
            } else {
                rest
            }
        }
        None => &t,
    };
    (!tail.is_empty()).then(|| tail.to_string())
}

fn clean(b: &[u8]) -> bool {
    !b.is_empty() && std::str::from_utf8(b).is_ok_and(|t| !t.chars().any(char::is_control))
}

pub fn read(chunk0: &[u8]) -> Option<Variant> {
    let (b, at) = aligned(chunk0.get(START..)?)?;
    // Back to the blob's first entry far enough for the name, forward past
    // the labels.
    let span = 0x8000;
    let from = at.saturating_sub(span);
    let parts: Vec<&[u8]> = b[from..(at + span).min(b.len())].split(|&c| c == 0).collect();
    let k = parts.iter().position(|p| *p == &ANCHOR[..ANCHOR.len() - 1])?;
    let points = k.checked_sub(LANGUAGES)?;
    let mut stats = Vec::new();
    let mut j = points;
    while stats.len() < MODE_SLOT && parts.get(j).is_some_and(|p| clean(p)) {
        stats.push(String::from_utf8_lossy(parts[j]).into_owned());
        j += LANGUAGES;
    }
    if stats.len() == MODE_SLOT {
        // `j` is on the single entry's group: past it, then the mode's labels.
        j += 1;
        while parts.get(j).is_some_and(|p| clean(p)) {
            stats.push(String::from_utf8_lossy(parts[j]).into_owned());
            j += LANGUAGES;
        }
    }
    // The blob's first entry has the binary before it: the name.
    let mut first = points;
    while first >= LANGUAGES && clean(parts[first]) {
        first -= LANGUAGES;
    }
    let name = text(parts[first]).unwrap_or_default();
    let description = parts.get(first + LANGUAGES).and_then(|p| text(p)).unwrap_or_default();
    let mut v = Variant { name, description, stats, ..Variant::default() };
    // The settings struct follows the header struct's path and stop.
    if let Some(at) = mode_path(&b) {
        let p = past_path(&b, at)?;
        if let Some(q) = header_start(&b, p) {
            let mut icon = 0;
            if let Ok(header) = bond::fields(&b[q..p]) {
                walk(&header, &mut Vec::new(), &mut |path: &[u32], x: i64| {
                    if path == ICON_INDEX {
                        icon = x;
                    }
                });
            }
            v.icon = u8::try_from(icon).ok();
        }
        let (mut last_chance_on, mut last_chance) = (false, None);
        let mut rules: Vec<(usize, i64)> = Vec::new();
        let mut flags: Vec<usize> = Vec::new();
        let mut hash = None;
        let mut options = Vec::new();
        let mut f = |path: &[u32], x: i64| {
            if path == OPTION_HASH {
                hash = Some(x as u32);
            } else if path == OPTION_VALUE {
                if let (Some(h), Some(value)) = (hash.take(), option_value(x)) {
                    options.push((h, value));
                }
            }
            if let Some(k) = RULES.iter().position(|r| r.0 == path) {
                rules.push((k, x));
            }
            if let Some(k) = FLAGS.iter().position(|r| r.0 == path && r.1 == x) {
                flags.push(k);
            }
            if path == RESPAWN {
                v.respawn = Some(x);
            } else if path == SCORE_TO_WIN {
                v.score_to_win = Some(x);
            } else if path == SUICIDE_PENALTY {
                v.suicide_penalty = Some(x);
            } else if path == BETRAYAL_PENALTY {
                v.betrayal_penalty = Some(x);
            } else if path == OVERTIME {
                v.overtime = Some(x);
            } else if path == LAST_CHANCE_IN_OVERTIME {
                last_chance_on = x == 1;
            } else if path == LAST_CHANCE {
                last_chance = Some(x);
            } else if path == TIME_LIMIT {
                v.time_limit = Some(x);
            } else if path == LOADOUT_WEAPON {
                v.loadout.push(x as u32);
            } else if path == LOADOUT_RANDOM && x == 1 {
                v.loadout_random = true;
            } else if path == LOADOUT_EQUIPMENT {
                v.equipment = Some(x as u32);
            }
        };
        if let Ok(settings) = bond::fields(&b[p..]) {
            walk(&settings, &mut Vec::new(), &mut f);
        }
        options.sort_by_key(|o: &(u32, OptionValue)| o.0);
        options.dedup_by_key(|o| o.0);
        v.options = options;
        v.last_chance = last_chance.filter(|_| last_chance_on);
        rules.sort_unstable();
        flags.sort_unstable();
        flags.dedup();
        v.rules = rules.into_iter().map(|(k, x)| (RULES[k].1, x)).collect();
        v.flags = flags.into_iter().map(|k| FLAGS[k].2).collect();
    }
    Some(v)
}

/// Every number of the settings struct, with its path of field ids, for
/// surveys.
#[cfg(feature = "research")]
pub fn settings(chunk0: &[u8]) -> Vec<(Vec<u32>, i64)> {
    let mut out = Vec::new();
    let Some((b, _)) = chunk0.get(START..).and_then(aligned) else { return out };
    let Some(settings) = settings_start(&b).and_then(|p| bond::fields(&b[p..]).ok()) else { return out };
    walk(&settings, &mut Vec::new(), &mut |path: &[u32], x: i64| out.push((path.to_vec(), x)));
    out
}
