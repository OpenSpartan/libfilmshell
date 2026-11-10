use super::{PlayerId, TeamId, Tick};

/// Whose row of the scoreboard a value is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum StatOwner {
    /// A player's row.
    Player(PlayerId),
    /// A team's row.
    Team(TeamId),
}

/// The scoreboard's statistics with a fixed meaning, by their statborg
/// slot. The variant's [`stat_labels`](crate::GameVariant::stat_labels) name
/// every slot, these included.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Stat {
    /// The mode's score.
    Score,
    /// Personal score.
    PersonalScore,
    /// Kills.
    Kills,
    /// Deaths.
    Deaths,
    /// Assists.
    Assists,
    /// Suicides.
    Suicides,
    /// The longest kill streak.
    MaxSpree,
    /// Headshot kills.
    Headshots,
    /// Shots fired.
    ShotsFired,
    /// Melee kills.
    MeleeKills,
    /// Grenade kills.
    GrenadeKills,
    /// Power weapon kills.
    PowerWeaponKills,
    /// Damage taken.
    DamageTaken,
    /// Spawns.
    Spawns,
}

impl Stat {
    /// Every named statistic.
    pub const ALL: &'static [Stat] = &[
        Stat::Score,
        Stat::PersonalScore,
        Stat::Kills,
        Stat::Deaths,
        Stat::Assists,
        Stat::Suicides,
        Stat::MaxSpree,
        Stat::Headshots,
        Stat::ShotsFired,
        Stat::MeleeKills,
        Stat::GrenadeKills,
        Stat::PowerWeaponKills,
        Stat::DamageTaken,
        Stat::Spawns,
    ];

    /// The statistic's statborg slot.
    pub fn slot(self) -> u32 {
        match self {
            Stat::Score => 0,
            Stat::PersonalScore => 3,
            Stat::Kills => 4,
            Stat::Deaths => 5,
            Stat::Assists => 6,
            Stat::Suicides => 8,
            Stat::MaxSpree => 11,
            Stat::Headshots => 12,
            Stat::ShotsFired => 13,
            Stat::MeleeKills => 16,
            Stat::GrenadeKills => 17,
            Stat::PowerWeaponKills => 18,
            Stat::DamageTaken => 20,
            Stat::Spawns => 21,
        }
    }
}

/// One scoreboard value changing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct StatChange {
    /// When.
    pub tick: Tick,
    /// Whose row.
    pub owner: StatOwner,
    /// The statborg slot, 0 to 55.
    pub slot: u32,
    /// The new value.
    pub value: i32,
    /// The round the value is for, 0 the first. The game sends it with each
    /// row, so a value restated for a new round is a change too.
    #[cfg_attr(feature = "serde", serde(default))]
    pub round: u32,
}

/// The match clock when its limit or state changes.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MatchClock {
    /// When.
    pub tick: Tick,
    /// The round's limit, seconds.
    pub limit: f64,
    /// Seconds left.
    pub left: f64,
    /// The timer's 5-bit state as the game sends it. Seen on the test
    /// films: 1 while stopped; 6 while running in timed modes, where a mode
    /// like Oddball also stops it while no one scores; 19 while running in
    /// the rounds of Attrition and Elimination, which have no time limit.
    /// What the bits mean isn't known.
    pub state: u32,
}

/// The engine's state at a seek snapshot, every 20 s.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct EngineState {
    /// The snapshot's tick.
    pub tick: Tick,
    /// The match phase: 1 in play, 2 over.
    pub phase: u32,
    /// The lives each player in the game has left, None for unlimited.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub lives_left: Vec<(PlayerId, Option<u32>)>,
    /// The scoreboard's rows as the snapshot holds them: each player's and
    /// team's values of the statborg's 56 slots, by slot. The values the
    /// snapshot misses are carried from the one before.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub rows: Vec<(StatOwner, Vec<i32>)>,
}

/// The game's scoreboard over the match: every value the statborg (the
/// game's replicated scoreboard: a row of 56 slots per player and per team)
/// sends as it changes, and the match clock.
///
/// A value belongs to one round ([`StatChange::round`]), and modes that play
/// rounds restart their rows each round: [`Scoreboard::final_stat`] is the
/// last round's value, [`Scoreboard::total_stat`] the match's.
///
/// ```no_run
/// # fn run(film: &filmshell::Film, id: filmshell::PlayerId) {
/// use filmshell::{Stat, StatOwner};
///
/// let kills = film.scoreboard().total_stat(StatOwner::Player(id), Stat::Kills);
/// # }
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Scoreboard {
    /// Every change, in tick order.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub changes: Vec<StatChange>,
    /// The match clock at each change of its limit or state.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub match_clock: Vec<MatchClock>,
    /// The engine's state at each seek snapshot.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub engine: Vec<EngineState>,
}

impl Scoreboard {
    /// The value of `slot` in `owner`'s row at `tick` (0 before any change).
    /// Searches back from `tick` through the changes.
    pub fn value_at(&self, owner: StatOwner, slot: u32, tick: Tick) -> i32 {
        let upto = self.changes.partition_point(|c| c.tick <= tick);
        self.changes[..upto].iter().rfind(|c| c.owner == owner && c.slot == slot).map_or(0, |c| c.value)
    }

    /// The value of a named statistic at `tick`: see [`Scoreboard::value_at`].
    pub fn value_at_stat(&self, owner: StatOwner, stat: Stat, tick: Tick) -> i32 {
        self.value_at(owner, stat.slot(), tick)
    }

    /// How many rounds the match had, by the rounds the scoreboard's values
    /// are for: 1 for a match of one round, or a film without a scoreboard
    /// (whose [`Scoreboard::round_starts`] can still find rounds by the match
    /// clock).
    pub fn rounds(&self) -> u32 {
        self.changes.iter().map(|c| c.round).max().map_or(1, |r| r + 1)
    }

    /// The value of `slot` in `owner`'s row at the end of `round` (0 the
    /// first), 0 when the row has none in it. A player who leaves has their
    /// row cleared; their value is the one before the clear.
    pub fn round_value(&self, owner: StatOwner, slot: u32, round: u32) -> i32 {
        let mine: Vec<&StatChange> =
            self.changes.iter().filter(|c| c.owner == owner && c.slot == slot && c.round == round).collect();
        match mine.split_last() {
            Some((last, before)) if last.value == 0 && self.row_cleared(owner, last.tick) => {
                before.last().map_or(0, |c| c.value)
            }
            Some((last, _)) => last.value,
            None => 0,
        }
    }

    /// The value of `slot` in `owner`'s row at the end of the match, in its
    /// last round: see [`Scoreboard::round_value`].
    pub fn final_value(&self, owner: StatOwner, slot: u32) -> i32 {
        self.round_value(owner, slot, self.rounds() - 1)
    }

    /// The final value of a named statistic: see [`Scoreboard::final_value`].
    pub fn final_stat(&self, owner: StatOwner, stat: Stat) -> i32 {
        self.final_value(owner, stat.slot())
    }

    /// The match's total of `slot` in `owner`'s row: each round's value,
    /// added up. The same as [`Scoreboard::final_value`] in a match of one
    /// round. Meaningful for counts (kills, deaths, points), not for highs
    /// such as the longest spree.
    pub fn total(&self, owner: StatOwner, slot: u32) -> i32 {
        (0..self.rounds()).map(|r| self.round_value(owner, slot, r)).sum()
    }

    /// The match's total of a named statistic: see [`Scoreboard::total`].
    pub fn total_stat(&self, owner: StatOwner, stat: Stat) -> i32 {
        self.total(owner, stat.slot())
    }

    /// Whether `owner`'s whole row went to 0 at `tick`: three slots or more.
    fn row_cleared(&self, owner: StatOwner, tick: Tick) -> bool {
        let at = self.changes.partition_point(|c| c.tick < tick);
        self.changes[at..].iter().take_while(|c| c.tick == tick).filter(|c| c.owner == owner && c.value == 0).count()
            >= 3
    }
    /// The teams with a row, in id order.
    pub fn teams(&self) -> Vec<TeamId> {
        let mut t: Vec<TeamId> = self
            .changes
            .iter()
            .filter_map(|c| match c.owner {
                StatOwner::Team(t) => Some(t),
                StatOwner::Player(_) => None,
            })
            .collect();
        t.sort_unstable();
        t.dedup();
        t
    }

    /// The ticks rounds start at after the first: each round's first
    /// scoreboard value. A film without a scoreboard falls back on the match
    /// clock, where it is back at its full limit after running down.
    pub fn round_starts(&self) -> Vec<Tick> {
        if !self.changes.is_empty() {
            return (1..self.rounds())
                .filter_map(|r| self.changes.iter().filter(|c| c.round == r).map(|c| c.tick).min())
                .collect();
        }
        let mut out = Vec::new();
        let mut ran_down = false;
        for c in &self.match_clock {
            if c.limit > 0.0 && c.left >= c.limit - 0.5 {
                if ran_down {
                    out.push(c.tick);
                }
                ran_down = false;
            } else if c.left < c.limit {
                ran_down = true;
            }
        }
        out
    }
}
