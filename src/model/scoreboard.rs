use super::{PlayerId, TeamId, Tick};

/// Whose row of the scoreboard a value is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum StatOwner {
    /// A player's row.
    Player(PlayerId),
    /// A team's row.
    Team(TeamId),
}

/// The scoreboard's statistics with a fixed meaning, by their statborg
/// slot. The variant's `stat_labels` name every slot, these included.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct EngineState {
    /// The snapshot's tick.
    pub tick: Tick,
    /// The match phase: 1 in play, 2 over.
    pub phase: u32,
    /// The lives each player in the game has left, -1 for unlimited.
    pub lives_left: Vec<(PlayerId, i32)>,
}

/// The game's scoreboard over the match: every value the statborg
/// replicates as it changes, and the match clock.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Scoreboard {
    /// Every change, in tick order.
    pub changes: Vec<StatChange>,
    /// The match clock at each change of its limit or state.
    pub clock: Vec<MatchClock>,
    /// The engine's state at each seek snapshot.
    pub engine: Vec<EngineState>,
}

impl Scoreboard {
    /// The value of `slot` in `owner`'s row at `tick` (0 before any change).
    pub fn value_at(&self, owner: StatOwner, slot: u32, tick: Tick) -> i32 {
        self.changes
            .iter()
            .take_while(|c| c.tick <= tick)
            .filter(|c| c.owner == owner && c.slot == slot)
            .last()
            .map_or(0, |c| c.value)
    }

    /// The final value of `slot` in `owner`'s row.
    pub fn final_value(&self, owner: StatOwner, slot: u32) -> i32 {
        self.changes.iter().rfind(|c| c.owner == owner && c.slot == slot).map_or(0, |c| c.value)
    }

    /// The final value of a named statistic.
    pub fn final_stat(&self, owner: StatOwner, stat: Stat) -> i32 {
        self.final_value(owner, stat.slot())
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

    /// The ticks rounds start at after the first: where the clock is back at
    /// its full limit after running down.
    pub fn round_starts(&self) -> Vec<Tick> {
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
