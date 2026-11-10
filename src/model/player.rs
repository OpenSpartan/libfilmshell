use super::{PlayerId, TeamId, Tick, Xuid};

/// Whether a player is a person or a bot, and what identifies them.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PlayerKind {
    /// A person signed in with an Xbox account.
    Human {
        /// The account.
        xuid: Xuid,
        /// The gamertag, from chunk 0's roster or the player's
        /// HighlightEvents records.
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
        gamertag: Option<String>,
    },
    /// A bot.
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
    /// The player's index.
    pub id: PlayerId,
    /// Person or bot.
    pub kind: PlayerKind,
    /// The player's team: from their HighlightEvents records, else from the
    /// team their engine player id carries while they hold it, which a
    /// free-for-all game leaves out.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub team: Option<TeamId>,
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
}
