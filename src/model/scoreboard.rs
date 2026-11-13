use super::{PlayerId, TeamId, Tick};
use std::collections::BTreeMap;

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

/// The scoreboard's statistics with a fixed meaning, by their slot in its
/// rows ([`Scoreboard`]). The variant's
/// [`stat_labels`](crate::GameVariant::stat_labels) name every slot, these
/// included.
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
    /// Kills of teammates.
    Betrayals,
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
        Stat::Betrayals,
        Stat::MaxSpree,
        Stat::Headshots,
        Stat::ShotsFired,
        Stat::MeleeKills,
        Stat::GrenadeKills,
        Stat::PowerWeaponKills,
        Stat::DamageTaken,
        Stat::Spawns,
    ];

    /// The statistic's scoreboard slot.
    pub fn slot(self) -> u32 {
        match self {
            Stat::Score => 0,
            Stat::PersonalScore => 3,
            Stat::Kills => 4,
            Stat::Deaths => 5,
            Stat::Assists => 6,
            Stat::Suicides => 8,
            Stat::Betrayals => 9,
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

/// A named statistic's slot, so the [`Scoreboard`] lookups take a [`Stat`]
/// or a raw slot alike.
impl From<Stat> for u32 {
    fn from(stat: Stat) -> u32 {
        stat.slot()
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
    /// The slot in the row, 0 to 55.
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
    /// When the limit or state changed.
    pub tick: Tick,
    /// The round's limit, seconds. The game sends the timer in 16-bit steps
    /// of 36,000 / 65,535 s (about 0.55 s) and rounds the limit up to a step:
    /// a 300 s round's limit is 300.481 s.
    pub limit: f64,
    /// The timer's seconds, in the same steps as the limit: the time left
    /// while it counts down, the time so far while it counts up.
    pub left: f64,
    /// The timer's 5-bit state as the game sends it: an index into the game's
    /// table of timer rates ([`MatchClock::rate`]). On the test films it is 1
    /// while stopped, 6 counting down at the normal rate in timed modes
    /// (Oddball stops it while someone holds the skull), and 19 counting up at
    /// the normal rate in the rounds of Attrition and Elimination, which have
    /// no time limit.
    pub state: u32,
}

/// The game's timer rates: index 2 to 14 counts down, 15 to 27 counts up.
const CLOCK_RATES: [f64; 13] = [0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0, 5.0, 10.0];

impl MatchClock {
    /// Seconds of clock per second of game time: negative while it counts
    /// down, 0 while it stands still, 1 for a state past the table (the
    /// game's default).
    pub fn rate(&self) -> f64 {
        match self.state {
            0 | 1 => 0.0,
            s @ 2..=14 => -CLOCK_RATES[s as usize - 2],
            s @ 15..=27 => CLOCK_RATES[s as usize - 15],
            _ => 1.0,
        }
    }

    /// Whether the timer is stopped, as the game's scripts ask it
    /// (`IsRoundTimerStopped`): state 1.
    pub fn is_stopped(&self) -> bool {
        self.state == 1
    }

    /// Whether the time left is going down.
    pub fn is_counting_down(&self) -> bool {
        self.rate() < 0.0
    }

    /// Whether the time is going up (a round with no time limit).
    pub fn is_counting_up(&self) -> bool {
        self.rate() > 0.0
    }
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

/// The game's scoreboard over the match.
///
/// It holds every value the statborg (the game's replicated scoreboard: a row
/// of 56 slots per player and per team) sends as it changes, the match clock,
/// and the engine's state at each seek snapshot.
///
/// A value belongs to one round ([`StatChange::round`]), and modes that play
/// rounds restart their rows each round: [`Scoreboard::final_value`] is the
/// last round's value, [`Scoreboard::total`] the match's.
///
/// ```no_run
/// # fn run(film: &filmshell::Film, id: filmshell::PlayerId) {
/// use filmshell::{Stat, StatOwner, Tick};
///
/// let board = film.scoreboard();
/// let me = StatOwner::Player(id);
/// // The scoreboard as playback shows it 5 minutes into the match, and the
/// // match's total.
/// if let Some(five) = film.match_tick(300.0) {
///     let now = board.value_at(me, Stat::Kills, five);
///     let total = board.total(me, Stat::Kills);
///     println!("{now} kills at 5:00, {total} in all");
/// }
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
    /// The slots in a row.
    pub const SLOTS: usize = 56;

    /// Every row at `tick`, in owner order: each player's and team's value
    /// of every slot as of its last change at or before `tick`, 0 before any.
    /// One pass over the changes, for a scoreboard drawn at a tick;
    /// [`Scoreboard::value_at`] reads one value.
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film, now: filmshell::Tick) {
    /// use filmshell::{Stat, StatOwner};
    ///
    /// for (owner, row) in film.scoreboard().rows_at(now) {
    ///     if let StatOwner::Player(p) = owner {
    ///         let (k, d) = (row[Stat::Kills.slot() as usize], row[Stat::Deaths.slot() as usize]);
    ///         println!("{} {k}-{d}", film.player_name(p).unwrap_or("someone"));
    ///     }
    /// }
    /// # }
    /// ```
    pub fn rows_at(&self, tick: Tick) -> BTreeMap<StatOwner, [i32; Scoreboard::SLOTS]> {
        let mut rows = BTreeMap::new();
        for c in &self.changes[..self.changes.partition_point(|c| c.tick <= tick)] {
            if let Some(v) = rows.entry(c.owner).or_insert([0; Scoreboard::SLOTS]).get_mut(c.slot as usize) {
                *v = c.value;
            }
        }
        rows
    }

    /// The value of `slot` (a [`Stat`] or a raw slot) in `owner`'s row at
    /// `tick`, 0 before any change. Searches back from `tick` through the
    /// changes.
    pub fn value_at(&self, owner: StatOwner, slot: impl Into<u32>, tick: Tick) -> i32 {
        let slot = slot.into();
        let upto = self.changes.partition_point(|c| c.tick <= tick);
        self.changes[..upto].iter().rfind(|c| c.owner == owner && c.slot == slot).map_or(0, |c| c.value)
    }

    /// The match clock's seconds at `tick`, as the game shows them: the time
    /// left while it counts down, the time so far while it counts up.
    ///
    /// It runs on from its last change at that change's rate
    /// ([`MatchClock::rate`]), and stops at 0. None before the clock's first
    /// change, and on a film without a match clock.
    pub fn clock_at(&self, tick: Tick) -> Option<f64> {
        let upto = self.match_clock.partition_point(|c| c.tick <= tick);
        let c = self.match_clock[..upto].last()?;
        let ran = (tick - c.tick) as f64 / f64::from(Tick::PER_SECOND);
        Some((c.left + c.rate() * ran).max(0.0))
    }

    /// How many rounds the match had, by the rounds the scoreboard's values
    /// are for: 1 for a match of one round, or a film without a scoreboard
    /// (whose [`Scoreboard::round_starts`] can still find rounds by the match
    /// clock).
    pub fn rounds(&self) -> u32 {
        self.changes.iter().map(|c| c.round).max().map_or(1, |r| r + 1)
    }

    /// The value of `slot` (a [`Stat`] or a raw slot) in `owner`'s row at
    /// the end of `round` (0 the first), 0 when the row has none in it. A
    /// player who leaves has their row cleared; their value is the one before
    /// the clear.
    pub fn round_value(&self, owner: StatOwner, slot: impl Into<u32>, round: u32) -> i32 {
        let slot = slot.into();
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

    /// The value of `slot` (a [`Stat`] or a raw slot) in `owner`'s row at
    /// the end of the match, in its last round: see
    /// [`Scoreboard::round_value`].
    pub fn final_value(&self, owner: StatOwner, slot: impl Into<u32>) -> i32 {
        self.round_value(owner, slot, self.rounds() - 1)
    }

    /// The match's total of `slot` (a [`Stat`] or a raw slot) in `owner`'s
    /// row: each round's value, added up. The same as
    /// [`Scoreboard::final_value`] in a match of one round. Meaningful for
    /// counts (kills, deaths, points), not for highs such as the longest
    /// spree.
    pub fn total(&self, owner: StatOwner, slot: impl Into<u32>) -> i32 {
        let slot = slot.into();
        (0..self.rounds()).map(|r| self.round_value(owner, slot, r)).sum()
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
