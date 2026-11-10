//! The game variant as the film carries it: chunk 0 holds it as Bond Compact
//! Binary at a bit offset. Its strings
//! are one blob, NUL-separated, 18 languages per string with English first:
//! the name, the description, score text, then the scoreboard's labels in
//! statborg slot order.

/// Where the variant's structures start in chunk 0.
const START: usize = 0xce680;
/// Entries per string: one per language.
const LANGUAGES: usize = 18;
/// The slot after the fixed labels, where a mode's own start. One entry of
/// another kind ("Alpha") sits before them.
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
/// A field at its default isn't written. Floats are in thousandths (`value`).
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
/// Flag sets, each element a bit index, named by the mask of the game's
/// exported `ManagedGameVariant_Set*` setter for it. A flag not listed is off.
/// The variant's option table: each entry the murmur3 of an option's lowercased
/// name, and 8 bytes of value (`Variant::options`).
const OPTION_HASH: &[u32] = &[3, 9, 0, 0, 0];
const OPTION_VALUE: &[u32] = &[3, 9, 0, 1, 0];
/// Byte 4 of an option's value: its type.
const OPTION_FLOAT: u8 = 3;
const OPTION_BOOL: u8 = 2;
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
    /// Seconds.
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
}

pub use crate::game::OptionValue;

/// An option's 8 bytes (packed little-endian by `value`): the value in the
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

fn varint(b: &[u8], p: &mut usize) -> Option<u64> {
    let (mut v, mut s) = (0u64, 0);
    loop {
        let x = *b.get(*p)?;
        *p += 1;
        v |= ((x & 127) as u64) << s;
        s += 7;
        if x < 128 {
            return Some(v);
        }
        if s > 63 {
            return None;
        }
    }
}

/// Read a Bond Compact Binary v2 value of type `t` at `p`, calling `f` on each
/// integer with the path of field ids to it.
fn value(b: &[u8], p: &mut usize, t: u8, path: &mut Vec<u32>, f: &mut impl FnMut(&[u32], i64)) -> Option<()> {
    let zig = |v: u64| (v >> 1) as i64 ^ -((v & 1) as i64);
    match t {
        // bool, uint8, int8
        2 | 3 | 14 => {
            let v = *b.get(*p)? as i8 as i64;
            *p += 1;
            f(path, v);
        }
        4..=6 => {
            let v = varint(b, p)?;
            f(path, v as i64);
        }
        15..=17 => {
            let v = varint(b, p)?;
            f(path, zig(v));
        }
        7 => {
            let v = f32::from_le_bytes(b.get(*p..*p + 4)?.try_into().ok()?);
            *p += 4;
            f(path, (v as f64 * 1000.0).round() as i64);
        }
        8 => *p += 8,
        9 => {
            let n = varint(b, p)? as usize;
            *p += n;
        }
        18 => {
            let n = varint(b, p)? as usize;
            *p += 2 * n;
        }
        10 => {
            let n = varint(b, p)? as usize;
            let end = p.checked_add(n)?;
            fields(b, p, path, f)?;
            *p = end;
        }
        11 | 12 => {
            let x = *b.get(*p)?;
            *p += 1;
            let n = if x >> 5 == 0 { varint(b, p)? } else { (x >> 5) as u64 - 1 };
            if x & 31 == 3 {
                // A byte list: the string blob, the script list, or an
                // option's 8 bytes, which are passed on as one integer.
                if n == 8 {
                    f(path, i64::from_le_bytes(b.get(*p..*p + 8)?.try_into().ok()?));
                }
                *p += n as usize;
                return Some(());
            }
            for _ in 0..n {
                value(b, p, x & 31, path, f)?;
            }
        }
        13 => {
            let (kt, vt) = (*b.get(*p)?, *b.get(*p + 1)?);
            *p += 2;
            for _ in 0..varint(b, p)? {
                value(b, p, kt, path, f)?;
                value(b, p, vt, path, f)?;
            }
        }
        _ => return None,
    }
    Some(())
}

/// A struct's fields, up to its stop.
fn fields(b: &[u8], p: &mut usize, path: &mut Vec<u32>, f: &mut impl FnMut(&[u32], i64)) -> Option<()> {
    loop {
        let h = *b.get(*p)?;
        *p += 1;
        let (t, mut id) = (h & 31, (h >> 5) as u32);
        if id == 6 {
            id = *b.get(*p)? as u32;
            *p += 1;
        } else if id == 7 {
            id = *b.get(*p)? as u32 | (*b.get(*p + 1)? as u32) << 8;
            *p += 2;
        }
        match t {
            0 => return Some(()),
            1 => continue,
            _ => {
                path.push(id);
                let r = value(b, p, t, path, f);
                path.pop();
                r?;
            }
        }
    }
}

fn shifted(r: &[u8], s: u32) -> Vec<u8> {
    let at = |i: usize| r.get(i).copied().unwrap_or(0) as u32;
    (0..r.len()).map(|i| (((at(i) << 8 | at(i + 1)) << s) >> 8) as u8).collect()
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
    let r = chunk0.get(START..)?;
    let (b, at) = (0..8).find_map(|s| {
        let b = shifted(r, s);
        let at = b.windows(ANCHOR.len()).position(|w| w == ANCHOR)?;
        Some((b, at))
    })?;
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
    if let Some(at) = b.windows(MODE_PATH.len()).position(|w| w == MODE_PATH) {
        let mut p = at + b[at..].iter().position(|&c| c == 0)? + 1;
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
        // The fields past the settings aren't all read: a stop there changes nothing.
        let _ = fields(&b, &mut p, &mut Vec::new(), &mut f);
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

/// Every integer of the settings struct, with its path of field ids, for
/// surveys.
#[cfg(feature = "research")]
pub fn settings(chunk0: &[u8]) -> Vec<(Vec<u32>, i64)> {
    let mut out = Vec::new();
    let Some(r) = chunk0.get(START..) else { return out };
    let Some(b) = (0..8).map(|s| shifted(r, s)).find(|b| b.windows(ANCHOR.len()).any(|w| w == ANCHOR)) else {
        return out;
    };
    let Some(at) = b.windows(MODE_PATH.len()).position(|w| w == MODE_PATH) else { return out };
    let Some(end) = b[at..].iter().position(|&c| c == 0) else { return out };
    let mut p = at + end + 1;
    let _ = fields(&b, &mut p, &mut Vec::new(), &mut |path: &[u32], x: i64| out.push((path.to_vec(), x)));
    out
}
