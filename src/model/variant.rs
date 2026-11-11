use crate::game::{self, OptionValue};
use crate::{StringId, TagId};

/// The mode a game variant runs, from the mode scripts chunk 0 names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum GameMode {
    /// Slayer and its variants (Big Team Battle Slayer, Fiesta), and the modes
    /// that run Slayer's scripts: Grifball, Juggernaut, Headhunter.
    Slayer,
    /// Capture the Flag, Big Team Battle's too, and the bomb modes that run
    /// its scripts.
    CaptureTheFlag,
    /// Oddball.
    Oddball,
    /// Strongholds, and Land Grab, which runs its scripts.
    Strongholds,
    /// King of the Hill.
    KingOfTheHill,
    /// Firefight.
    Firefight,
    /// Firefight with King of the Hill's hills.
    FirefightKingOfTheHill,
    /// Attrition and Elimination: limited lives, revives, a last player
    /// standing.
    Attrition,
    /// Extraction: arming and extracting at zones.
    Extraction,
    /// Escalation Slayer: a weapon ladder.
    Escalation,
    /// Infection: rounds of survivors against the infected, whose team a
    /// survivor joins when killed, so a player's
    /// [`Player::team`](crate::Player::team) is the one they ended on.
    Infection,
    /// VIP. Recognised by `vipevents.lua`, which films of older builds
    /// carry; no VIP film of the current format has been read yet.
    Vip,
    /// Stockpile: teams carry power seeds to their stockpiles. On a Big Team
    /// Stockpile film the events include each seed's [`SeedEvent`](crate::SeedEvent)s.
    Stockpile,
    /// Scripts the library doesn't tell apart.
    Other,
}

impl GameMode {
    pub(crate) fn from_scripts(scripts: &std::collections::BTreeSet<String>) -> GameMode {
        let has = |s: &str| scripts.contains(s);
        if has("firefightevents.lua") {
            return if has("fbastionevents.lua") { GameMode::FirefightKingOfTheHill } else { GameMode::Firefight };
        }
        if has("arenaflagevents.lua") || has("btbflagevents.lua") {
            GameMode::CaptureTheFlag
        } else if has("oddballevents.lua") {
            GameMode::Oddball
        } else if has("bastionevents.lua") {
            GameMode::KingOfTheHill
        } else if has("parcel_stronghold.lua") {
            GameMode::Strongholds
        } else if has("attritionevents.lua") {
            GameMode::Attrition
        } else if has("eevents.lua") {
            GameMode::Extraction
        } else if has("gungameevents.lua") {
            GameMode::Escalation
        } else if has("infectionevents.lua") {
            GameMode::Infection
        } else if has("vipevents.lua") {
            GameMode::Vip
        } else if has("stockpile.lua") || has("btbstockpileevents.lua") {
            GameMode::Stockpile
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

    /// The mode's English name.
    pub fn name(self) -> &'static str {
        match self {
            GameMode::Slayer => "Slayer",
            GameMode::CaptureTheFlag => "Capture the Flag",
            GameMode::Oddball => "Oddball",
            GameMode::Strongholds => "Strongholds",
            GameMode::KingOfTheHill => "King of the Hill",
            GameMode::Firefight => "Firefight",
            GameMode::FirefightKingOfTheHill => "Firefight King of the Hill",
            GameMode::Attrition => "Attrition",
            GameMode::Extraction => "Extraction",
            GameMode::Escalation => "Escalation Slayer",
            GameMode::Infection => "Infection",
            GameMode::Vip => "VIP",
            GameMode::Stockpile => "Stockpile",
            GameMode::Other => "Other",
        }
    }
}

impl std::fmt::Display for GameMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The starting loadout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Loadout {
    /// The starting weapons.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub weapons: Vec<TagId>,
    /// Starting weapons are random.
    pub random: bool,
    /// The starting equipment, a soft asset's tag.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub equipment: Option<TagId>,
}

/// An option the variant stores: its name's hash and its value.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct VariantOption {
    /// The option's lower-cased name, as the game hashes it.
    pub hash: StringId,
    /// The option's name, when the game's option scripts name it.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub name: Option<String>,
    /// The stored value.
    pub value: OptionValue,
}

/// One option of the mode's Options scripts, with its default and the
/// variant's value: a view [`GameVariant::rules`] builds, so it serializes
/// but doesn't read back.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Rule {
    /// The option's name.
    pub name: &'static str,
    /// The default the mode's scripts give it, when that is a literal.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub default: Option<OptionValue>,
    /// The variant's value, when it stores one.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct StatDefinition {
    /// The statistic's identifier ("AccuracyPercentage").
    pub name: String,
    /// `name`, as the game hashes it.
    pub id: StringId,
    /// The base entry type (2 a best-of, 3 computed).
    pub base: i64,
    /// How a match's value comes from its rounds, as the game numbers it;
    /// what each number means isn't known.
    pub aggregation: i64,
    /// Whether a lower value ranks higher.
    pub lower_is_better: bool,
    /// How the scoreboard shows the value.
    pub display: StatDisplay,
    /// The stored value is 10^precision times the shown one.
    pub precision: i64,
}

impl StatDefinition {
    /// A stored value as shown, its precision taken out.
    pub fn shown(&self, stored: i32) -> f64 {
        stored as f64 / 10f64.powi(self.precision.clamp(0, 18) as i32)
    }
}

/// How the scoreboard shows a statistic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum StatDisplay {
    /// A number.
    Number,
    /// A time.
    Time,
    /// A percentage.
    Percentage,
    /// A display type the library doesn't know, as the game numbers it.
    Other(i64),
}

impl StatDisplay {
    pub(crate) fn from_game(v: i64) -> StatDisplay {
        match v {
            0 => StatDisplay::Number,
            1 => StatDisplay::Time,
            2 => StatDisplay::Percentage,
            v => StatDisplay::Other(v),
        }
    }
}

/// The variant's stat definitions and engine settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct StatSettings {
    /// The statistics, by scoreboard slot.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub stats: Vec<StatDefinition>,
    /// The scoreboard's columns, as slots.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub scoreboard: Vec<usize>,
    /// The server runs at 30 Hz.
    pub server_30fps: bool,
    /// The variant's tick rate setting as stored, when it sets one; its
    /// unit isn't known.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub tick_rate: Option<i64>,
    /// Kill cams, when set.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub kill_cam: Option<bool>,
    /// The kill cam's window in seconds, when set.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub kill_cam_seconds: Option<i64>,
    /// Play of the game, when set.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub play_of_the_game: Option<bool>,
    /// Machinima controls, when set.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub machinima: Option<bool>,
    /// An `ssdf` spawn settings tag overriding the default, when set.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub spawn_settings: Option<TagId>,
}

/// The game variant: its name and rules, as chunk 0 holds it.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct GameVariant {
    /// The variant's English name ("Super Husky Raid").
    pub name: String,
    /// Its English description.
    pub description: String,
    /// The scoreboard's English labels, by slot.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub stat_labels: Vec<String>,
    /// Seconds from death to respawn.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub respawn_seconds: Option<i64>,
    /// The score that wins a round.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub score_to_win: Option<i64>,
    /// The round's time limit, seconds.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub time_limit_seconds: Option<i64>,
    /// Seconds added to the respawn after a suicide.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub suicide_penalty_seconds: Option<i64>,
    /// Seconds added to the respawn after a betrayal.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub betrayal_penalty_seconds: Option<i64>,
    /// Seconds of overtime.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub overtime_seconds: Option<i64>,
    /// Seconds of last chance, when overtime has it.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub last_chance_seconds: Option<i64>,
    /// The starting loadout.
    pub loadout: Loadout,
    /// Further settings the variant writes, by the game's field names.
    /// Floats are in thousandths.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub settings: Vec<(String, i64)>,
    /// Flags that are on, by the game's names ("friendlyFire").
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub flags: Vec<String>,
    /// The mode options the variant stores.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub options: Vec<VariantOption>,
    /// The script files chunk 0 names, lower-cased.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub scripts: Vec<String>,
    /// The stat definitions and engine settings.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub stat_settings: Option<StatSettings>,
}

impl GameVariant {
    /// The mode, from the scripts.
    pub fn mode(&self) -> GameMode {
        GameMode::from_scripts(&self.scripts.iter().cloned().collect())
    }

    /// The stored value of option `name`.
    pub fn option(&self, name: &str) -> Option<OptionValue> {
        let h = StringId::of(&name.to_ascii_lowercase());
        self.options.iter().find(|o| o.hash == h).map(|o| o.value)
    }

    /// Every option of the mode's Options scripts with its default and the
    /// variant's value, in the scripts' order. A mode counts when chunk 0
    /// names its Options script or the variant stores an option with its
    /// prefix.
    pub fn rules(&self) -> Vec<Rule> {
        let prefixes: Vec<&str> =
            self.options.iter().filter_map(|o| o.name.as_deref()?.split_once('_').map(|p| p.0)).collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(names: &[&str]) -> GameMode {
        GameMode::from_scripts(&names.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn modes_come_from_their_event_scripts() {
        // The mode-specific scripts films carry, beside the shared ones.
        assert_eq!(mode(&["infectionevents.lua", "pvpevents.lua"]), GameMode::Infection);
        assert_eq!(mode(&["vipevents.lua", "vipoptions.lua", "pvpevents.lua"]), GameMode::Vip);
        assert_eq!(mode(&["btbflagevents.lua", "flageventstubs.lua", "ctfbtboptions.lua"]), GameMode::CaptureTheFlag);
        assert_eq!(mode(&["attritionevents.lua", "eliminationconfig.lua"]), GameMode::Attrition);
        assert_eq!(mode(&["btbstockpileevents.lua", "stockpile.lua", "bigteambattle.lua"]), GameMode::Stockpile);
        assert_eq!(mode(&["slayerevents.lua"]), GameMode::Slayer);
        assert_eq!(mode(&["pvpevents.lua"]), GameMode::Other);
    }
}
