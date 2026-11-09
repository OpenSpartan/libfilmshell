use super::{PlayerId, TeamId, Tick, Xuid};

/// Whether a player is a person or a bot, and what identifies them.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum PlayerKind {
    /// A person signed in with an Xbox account.
    Human {
        /// The account.
        xuid: Xuid,
        /// The gamertag, from chunk 0's roster or the player's
        /// HighlightEvents records.
        gamertag: Option<String>,
    },
    /// A bot.
    Bot {
        /// The bot's id in the match.
        bot_id: u32,
        /// The bot's name ("343 Cliffton").
        name: String,
    },
}

/// One player of the match.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Player {
    /// The player's index.
    pub id: PlayerId,
    /// Person or bot.
    pub kind: PlayerKind,
    /// The player's team. Known for people who appear in the HighlightEvents
    /// chunk; bots' teams aren't read.
    pub team: Option<TeamId>,
    /// In the match from its start.
    pub present_at_start: bool,
    /// When a player who joined in progress first appears.
    pub joined: Option<Tick>,
    /// When a player who left before the end was last listed.
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
}
