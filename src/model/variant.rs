use crate::TagId;
use crate::game::{self, OptionValue};

/// The mode a game variant runs, from the mode scripts chunk 0 names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum GameMode {
    /// Slayer and its variants (Big Team Battle Slayer, Fiesta).
    Slayer,
    /// Capture the Flag.
    CaptureTheFlag,
    /// Oddball.
    Oddball,
    /// Strongholds.
    Strongholds,
    /// King of the Hill.
    KingOfTheHill,
    /// Firefight.
    Firefight,
    /// Firefight with King of the Hill's hills.
    FirefightKingOfTheHill,
    /// Scripts the library doesn't tell apart.
    Other,
}

impl GameMode {
    pub(crate) fn from_scripts(scripts: &std::collections::BTreeSet<String>) -> GameMode {
        let has = |s: &str| scripts.contains(s);
        if has("firefightevents.lua") {
            return if has("fbastionevents.lua") { GameMode::FirefightKingOfTheHill } else { GameMode::Firefight };
        }
        if has("arenaflagevents.lua") {
            GameMode::CaptureTheFlag
        } else if has("oddballevents.lua") {
            GameMode::Oddball
        } else if has("bastionevents.lua") {
            GameMode::KingOfTheHill
        } else if has("parcel_stronghold.lua") {
            GameMode::Strongholds
        } else if has("slayerevents.lua") || has("btbslayerevents.lua") {
            GameMode::Slayer
        } else {
            GameMode::Other
        }
    }

    /// Whether players fight players (every mode but Firefight).
    pub fn is_pvp(self) -> bool {
        !matches!(self, GameMode::Firefight | GameMode::FirefightKingOfTheHill)
    }
}

/// The starting loadout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Loadout {
    /// The starting weapons.
    pub weapons: Vec<TagId>,
    /// Starting weapons are random.
    pub random: bool,
    /// The starting equipment, a soft asset's tag.
    pub equipment: Option<TagId>,
}

/// An option the variant stores: its name's hash and its value.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct VariantOption {
    /// The murmur3 hash of the option's lower-cased name.
    pub hash: u32,
    /// The option's name, when the game's option scripts name it.
    pub name: Option<&'static str>,
    /// The stored value.
    pub value: OptionValue,
}

/// One option of the mode's Options scripts, with its default and the
/// variant's value.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Rule {
    /// The option's name.
    pub name: &'static str,
    /// The default the mode's scripts give it, when that is a literal.
    pub default: Option<OptionValue>,
    /// The variant's value, when it stores one.
    pub value: Option<OptionValue>,
}

impl Rule {
    /// The value in force: the variant's, else the default.
    pub fn effective(&self) -> Option<OptionValue> {
        self.value.or(self.default)
    }
}

/// A scoreboard statistic the variant defines.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct StatDefinition {
    /// The statistic's identifier ("AccuracyPercentage").
    pub name: String,
    /// The murmur3 hash of `name`.
    pub id: u32,
    /// The base entry type (2 a best-of, 3 computed).
    pub base: i64,
    /// How a match's value comes from its rounds.
    pub aggregation: i64,
    /// -1 when lower is better.
    pub sort: i64,
    /// 0 a number, 1 a time, 2 a percentage.
    pub display: i64,
    /// The stored value is 10^precision times the shown one.
    pub precision: i64,
}

/// The variant's stat definitions and engine settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct StatSettings {
    /// The statistics, by statborg slot.
    pub stats: Vec<StatDefinition>,
    /// The scoreboard's columns, as statborg slots.
    pub scoreboard: Vec<usize>,
    /// The server runs at 30 Hz.
    pub server_30fps: bool,
    /// The tick rate, when the variant sets one.
    pub tick_rate: Option<i64>,
    /// Kill cams, when set.
    pub kill_cam: Option<bool>,
    /// The kill cam's window in seconds, when set.
    pub kill_cam_seconds: Option<i64>,
    /// Play of the game, when set.
    pub play_of_the_game: Option<bool>,
    /// Machinima controls, when set.
    pub machinima: Option<bool>,
    /// An `ssdf` spawn settings tag overriding the default, when set.
    pub spawn_settings: Option<TagId>,
}

/// The game variant: its name and rules, as chunk 0 holds it.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct GameVariant {
    /// The variant's English name ("Super Husky Raid").
    pub name: String,
    /// Its English description.
    pub description: String,
    /// The scoreboard's English labels, by statborg slot.
    pub stat_labels: Vec<String>,
    /// Seconds from death to respawn.
    pub respawn_seconds: Option<i64>,
    /// The score that wins a round.
    pub score_to_win: Option<i64>,
    /// The round's time limit, seconds.
    pub time_limit_seconds: Option<i64>,
    /// Seconds added to the respawn after a suicide.
    pub suicide_penalty_seconds: Option<i64>,
    /// Seconds added to the respawn after a betrayal.
    pub betrayal_penalty_seconds: Option<i64>,
    /// Seconds of overtime.
    pub overtime_seconds: Option<i64>,
    /// Seconds of last chance, when overtime has it.
    pub last_chance_seconds: Option<i64>,
    /// The starting loadout.
    pub loadout: Loadout,
    /// Further settings the variant writes, by the game's field names.
    /// Floats are in thousandths.
    pub settings: Vec<(&'static str, i64)>,
    /// Flags that are on, by the game's names ("friendlyFire").
    pub flags: Vec<&'static str>,
    /// The mode options the variant stores.
    pub options: Vec<VariantOption>,
    /// The script files chunk 0 names, lower-cased.
    pub scripts: Vec<String>,
    /// The stat definitions and engine settings.
    pub stat_settings: Option<StatSettings>,
}

impl GameVariant {
    /// The mode, from the scripts.
    pub fn mode(&self) -> GameMode {
        GameMode::from_scripts(&self.scripts.iter().cloned().collect())
    }

    /// The stored value of option `name`.
    pub fn option(&self, name: &str) -> Option<OptionValue> {
        let h = game::murmur3(name.to_ascii_lowercase().as_bytes());
        self.options.iter().find(|o| o.hash == h).map(|o| o.value)
    }

    /// Every option of the mode's Options scripts with its default and the
    /// variant's value, in the scripts' order. A mode counts when chunk 0
    /// names its Options script or the variant stores an option with its
    /// prefix.
    pub fn rules(&self) -> Vec<Rule> {
        let prefixes: Vec<&str> = self.options.iter().filter_map(|o| o.name?.split_once('_').map(|p| p.0)).collect();
        let mut out = Vec::new();
        for m in game::modes() {
            if !self.scripts.iter().any(|s| s == m.script) && !prefixes.contains(&m.prefix) {
                continue;
            }
            for o in m.options {
                let value = o.keys.iter().find_map(|k| self.option(k));
                out.push(Rule { name: o.name, default: o.default, value });
            }
        }
        out
    }
}
