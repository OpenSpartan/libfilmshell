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
    pub const ALL: [Stat; 14] = [
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
    /// The timer's state.
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
    /// The lives each player in the game has left, -1 for unlimited.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub lives_left: Vec<(PlayerId, i32)>,
    /// The scoreboard's rows as the snapshot holds them: each player's and
    /// team's values of the statborg's 56 slots, by slot. The values the
    /// snapshot misses are carried from the one before.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub rows: Vec<(StatOwner, Vec<i32>)>,
}

/// The game's scoreboard over the match: every value the statborg
/// replicates as it changes, and the match clock.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Scoreboard {
    /// Every change, in tick order.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub changes: Vec<StatChange>,
    /// The match clock at each change of its limit or state.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub clock: Vec<MatchClock>,
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

    /// The value of `slot` in `owner`'s row at the end of the match, in its
    /// last round. A player who leaves has their row cleared; their value is
    /// the one before the clear.
    pub fn final_value(&self, owner: StatOwner, slot: u32) -> i32 {
        let from = self.round_starts().last().copied().unwrap_or_default();
        self.round_value(owner, slot, from, None)
    }

    /// The final value of a named statistic: see [`Scoreboard::final_value`].
    pub fn final_stat(&self, owner: StatOwner, stat: Stat) -> i32 {
        self.final_value(owner, stat.slot())
    }

    /// The match's total of `slot` in `owner`'s row: each round's final
    /// value, added up. The same as [`Scoreboard::final_value`] in a match of
    /// one round. Meaningful for counts (kills, deaths, points), not for
    /// highs such as the longest spree.
    pub fn total(&self, owner: StatOwner, slot: u32) -> i32 {
        let starts: Vec<Tick> = std::iter::once(Tick(0)).chain(self.round_starts()).collect();
        (0..starts.len()).map(|k| self.round_value(owner, slot, starts[k], starts.get(k + 1).copied())).sum()
    }

    /// The match's total of a named statistic: see [`Scoreboard::total`].
    pub fn total_stat(&self, owner: StatOwner, stat: Stat) -> i32 {
        self.total(owner, stat.slot())
    }

    /// The last value of `slot` in `owner`'s row from `from` up to `to`, the
    /// value before a clear of the whole row (a player leaving, or the next
    /// round starting) when the range ends with one.
    fn round_value(&self, owner: StatOwner, slot: u32, from: Tick, to: Option<Tick>) -> i32 {
        let mine: Vec<&StatChange> = self
            .changes
            .iter()
            .filter(|c| c.owner == owner && c.slot == slot && c.tick >= from && to.is_none_or(|t| c.tick < t))
            .collect();
        match mine.split_last() {
            Some((last, before)) if last.value == 0 && self.row_cleared(owner, last.tick) => {
                before.last().map_or(0, |c| c.value)
            }
            Some((last, _)) => last.value,
            None => 0,
        }
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

    /// The ticks rounds start at after the first: where a new round clears
    /// every player's row, the spawns of two rows or more, half or more of
    /// the rows that had spawned, dropping to 0 at once (a player who leaves
    /// has only their own row cleared). Without spawn counts, where the match
    /// clock is back at its full limit after running down, up to 3 s later.
    pub fn round_starts(&self) -> Vec<Tick> {
        let spawns = Stat::Spawns.slot();
        let mut had: std::collections::BTreeMap<PlayerId, i32> = std::collections::BTreeMap::new();
        let mut clears = Vec::new();
        let mut seen_spawns = false;
        let rows: Vec<&StatChange> =
            self.changes.iter().filter(|c| c.slot == spawns && matches!(c.owner, StatOwner::Player(_))).collect();
        for same in rows.chunk_by(|a, b| a.tick == b.tick) {
            seen_spawns = true;
            let live = had.values().filter(|&&v| v > 0).count();
            let cleared = same
                .iter()
                .filter(
                    |c| matches!(c.owner, StatOwner::Player(p) if c.value == 0 && had.get(&p).is_some_and(|&v| v > 0)),
                )
                .count();
            if cleared >= 2 && 2 * cleared >= live {
                clears.push(same[0].tick);
            }
            for c in same {
                if let StatOwner::Player(p) = c.owner {
                    had.insert(p, c.value);
                }
            }
        }
        if seen_spawns {
            return clears;
        }
        let mut out = Vec::new();
        let mut ran_down = false;
        for c in &self.clock {
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
