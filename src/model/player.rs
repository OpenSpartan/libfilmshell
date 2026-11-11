use super::{PlayerId, TeamId, Tick, Xuid};

/// Whether a player is a person or a bot, and what identifies them.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PlayerKind {
    /// A person signed in with an Xbox account.
    #[non_exhaustive]
    Human {
        /// The account.
        xuid: Xuid,
        /// The gamertag, from chunk 0's roster or the player's
        /// HighlightEvents records.
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
        gamertag: Option<String>,
    },
    /// A bot.
    #[non_exhaustive]
    Bot {
        /// The bot's profile, which picks its appearance: the bot id the
        /// Halo services list it under (`bid(N)`).
        bot_id: u32,
        /// The bot's name ("343 Cliffton").
        name: String,
    },
}

/// One player of the match.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Player {
    /// The player's id.
    pub id: PlayerId,
    /// Person or bot.
    pub kind: PlayerKind,
    /// The player's team: from their HighlightEvents records, else from the
    /// team their engine player id carries while they hold it. None in free
    /// for all. A player can change teams during a match (Infection moves a
    /// survivor to the infected when killed): this is the last, and
    /// [`Player::team_at`] the one at a tick.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub team: Option<TeamId>,
    /// Each team the player was on, from the tick they joined it, in tick
    /// order: the team designator the game replicates for their participant.
    /// Empty in free for all.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub teams: Vec<TeamChange>,
    /// In the match from its start.
    pub present_at_start: bool,
    /// When a player who joined in progress first appears.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub joined: Option<Tick>,
    /// When a player who left before the end was last listed.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub left: Option<Tick>,
}

impl Player {
    /// The gamertag or the bot's name.
    pub fn name(&self) -> Option<&str> {
        match &self.kind {
            PlayerKind::Human { gamertag, .. } => gamertag.as_deref(),
            PlayerKind::Bot { name, .. } => Some(name),
        }
    }

    /// The account of a person.
    pub fn xuid(&self) -> Option<Xuid> {
        match self.kind {
            PlayerKind::Human { xuid, .. } => Some(xuid),
            PlayerKind::Bot { .. } => None,
        }
    }

    /// Whether the player is a bot.
    pub fn is_bot(&self) -> bool {
        matches!(self.kind, PlayerKind::Bot { .. })
    }

    /// The player's team at `tick`: the last of [`Player::teams`] by then,
    /// else the first, else [`Player::team`]. A death that moves a player to
    /// another team (Infection's) moves them at the kill's tick, so the
    /// teams a kill was between are those of the tick before.
    pub fn team_at(&self, tick: Tick) -> Option<TeamId> {
        self.teams.iter().rfind(|c| c.tick <= tick).or(self.teams.first()).map(|c| c.team).or(self.team)
    }
}

/// A player's team from a tick on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct TeamChange {
    /// When the player joined the team.
    pub tick: Tick,
    /// The team.
    pub team: TeamId,
}
