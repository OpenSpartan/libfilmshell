use super::{EntityId, PlayerId, PlayerMask, Position, TeamId, Tick};
use crate::TagId;
use crate::game::{DamageCause, Grenade, Medal};

/// One event and when it happened.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TimedEvent {
    /// When.
    pub tick: Tick,
    /// What.
    pub event: Event,
}

/// Everything that happens in a film, in one stream.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum Event {
    /// A player was killed.
    Kill(Kill),
    /// A player earned a medal (from the HighlightEvents chunk, when the
    /// film fixes its clock; `Film::medals` lists every medal).
    Medal(MedalAward),
    /// A weapon fired a round.
    Shot(Shot),
    /// Damage landed on a player.
    Hit(Hit),
    /// A player picked something up.
    Pickup(Pickup),
    /// A player threw a grenade or an object.
    Throw(Throw),
    /// A projectile went off.
    Detonation(Detonation),
    /// A player did something: reloaded, swung, zoomed, jumped.
    Action(Action),
    /// A player went through a teleporter.
    Teleport(Teleport),
    /// A player got out of a vehicle.
    VehicleExit(VehicleExit),
    /// A player used equipment that put an object down.
    EquipmentUse(EquipmentUse),
    /// The announcer or a mode script played a voice line.
    VoiceLine(VoiceLine),
    /// A unit spoke a line.
    Chatter(Chatter),
    /// A message appeared on players' HUDs.
    HudMessage(HudMessage),
    /// A mode script's game event (PlayerGameEventSmall).
    GameEvent(GameEvent),
    /// Something happened to a flag.
    Flag(FlagEvent),
    /// Something happened at a zone or a hill, or to the skull.
    Zone(ZoneEvent),
    /// A player's respawn countdown started.
    RespawnCountdown(RespawnCountdown),
    /// The game counted a headshot kill for the player: the mode script's
    /// headshot event, which the scoreboard's headshot kills count. Sent on
    /// the tick of the kill, which [`Kill::headshot`] marks; where the film
    /// doesn't carry that kill's event, this is all the film has of it. PvP
    /// modes only.
    Headshot(PlayerId),
}

/// The kind of an [`Event`], without its data: for filtering and counting.
///
/// ```no_run
/// # fn run(film: &filmshell::Film) {
/// use filmshell::EventKind;
///
/// let shots = film.events().iter().filter(|e| e.event.kind() == EventKind::Shot).count();
/// # }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum EventKind {
    /// [`Event::Kill`].
    Kill,
    /// [`Event::Medal`].
    Medal,
    /// [`Event::Shot`].
    Shot,
    /// [`Event::Hit`].
    Hit,
    /// [`Event::Pickup`].
    Pickup,
    /// [`Event::Throw`].
    Throw,
    /// [`Event::Detonation`].
    Detonation,
    /// [`Event::Action`].
    Action,
    /// [`Event::Teleport`].
    Teleport,
    /// [`Event::VehicleExit`].
    VehicleExit,
    /// [`Event::EquipmentUse`].
    EquipmentUse,
    /// [`Event::VoiceLine`].
    VoiceLine,
    /// [`Event::Chatter`].
    Chatter,
    /// [`Event::HudMessage`].
    HudMessage,
    /// [`Event::GameEvent`].
    GameEvent,
    /// [`Event::Flag`].
    Flag,
    /// [`Event::Zone`].
    Zone,
    /// [`Event::RespawnCountdown`].
    RespawnCountdown,
    /// [`Event::Headshot`].
    Headshot,
}

impl Event {
    /// The event's kind.
    pub fn kind(&self) -> EventKind {
        match self {
            Event::Kill(_) => EventKind::Kill,
            Event::Medal(_) => EventKind::Medal,
            Event::Shot(_) => EventKind::Shot,
            Event::Hit(_) => EventKind::Hit,
            Event::Pickup(_) => EventKind::Pickup,
            Event::Throw(_) => EventKind::Throw,
            Event::Detonation(_) => EventKind::Detonation,
            Event::Action(_) => EventKind::Action,
            Event::Teleport(_) => EventKind::Teleport,
            Event::VehicleExit(_) => EventKind::VehicleExit,
            Event::EquipmentUse(_) => EventKind::EquipmentUse,
            Event::VoiceLine(_) => EventKind::VoiceLine,
            Event::Chatter(_) => EventKind::Chatter,
            Event::HudMessage(_) => EventKind::HudMessage,
            Event::GameEvent(_) => EventKind::GameEvent,
            Event::Flag(_) => EventKind::Flag,
            Event::Zone(_) => EventKind::Zone,
            Event::RespawnCountdown(_) => EventKind::RespawnCountdown,
            Event::Headshot(_) => EventKind::Headshot,
        }
    }

    /// The players the event is about: a kill's victim, killer and
    /// assistant, a hit's attacker and victim, a game event's players, the
    /// player who acted for the rest. A voice line's or a HUD message's
    /// audience isn't counted.
    pub fn players(&self) -> Vec<PlayerId> {
        let one = |p: PlayerId| vec![p];
        match self {
            Event::Kill(k) => [Some(k.victim), k.killer, k.assistant].into_iter().flatten().collect(),
            Event::Medal(m) => one(m.player),
            Event::Shot(s) => one(s.player),
            Event::Hit(h) => [h.attacker, Some(h.victim)].into_iter().flatten().collect(),
            Event::Pickup(p) => one(p.player),
            Event::Throw(t) => one(t.player),
            Event::Action(a) => one(a.player),
            Event::Teleport(t) => one(t.player),
            Event::VehicleExit(x) => one(x.player),
            Event::EquipmentUse(u) => one(u.player),
            Event::Chatter(c) => c.speaker.into_iter().collect(),
            Event::GameEvent(g) => g.players.iter().collect(),
            Event::Flag(f) => one(f.player),
            Event::Zone(z) => z.player.into_iter().collect(),
            Event::RespawnCountdown(r) => one(r.player),
            Event::Headshot(p) => one(*p),
            Event::Detonation(_) | Event::VoiceLine(_) | Event::HudMessage(_) => Vec::new(),
        }
    }

    /// Whether the event is about `player`, as [`Event::players`] counts.
    pub fn involves(&self, player: PlayerId) -> bool {
        match self {
            Event::GameEvent(g) => g.players.contains(player),
            _ => self.players().contains(&player),
        }
    }
}

/// A kill: the game's PlayerKilledEvent, with what dealt the fatal hit.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Kill {
    /// Who died.
    pub victim: PlayerId,
    /// Who killed them.
    pub killer: Option<PlayerId>,
    /// The killer's share of the victim's damage, whole percent.
    pub killer_damage_percent: u32,
    /// Who assisted.
    pub assistant: Option<PlayerId>,
    /// The assistant's share, whole percent.
    pub assistant_damage_percent: u32,
    /// The death was the victim's own doing.
    pub suicide: bool,
    /// Whether the game counted the kill as a headshot: a headshot event for
    /// the killer on the kill's tick or the two around it. None when the
    /// film has no headshot events at all.
    pub headshot: Option<bool>,
    /// Whether the scoreboard counted the death: false for the deaths the
    /// game deals everyone still alive when a match ends. None when the
    /// film's scoreboard isn't read.
    pub scored: Option<bool>,
    /// What the last hit on the victim in the two seconds before was.
    pub cause: Option<DamageCause>,
    /// The weapon: the one the fatal damage belongs to, else the killer's
    /// last shot in the two seconds before.
    pub weapon: Option<TagId>,
    /// Where the victim was.
    pub position: Option<Position>,
}

/// A medal award.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct MedalAward {
    /// Who earned it.
    pub player: PlayerId,
    /// The game's medal index.
    pub index: u32,
}

impl MedalAward {
    /// The medal.
    pub fn medal(&self) -> Option<Medal> {
        Medal::from_index(self.index)
    }
}

/// A round fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Shot {
    /// The shooter.
    pub player: PlayerId,
    /// The weapon.
    pub weapon: TagId,
}

/// Damage landing on a player.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Hit {
    /// Who dealt it; None for damage with no player behind it (a fall).
    pub attacker: Option<PlayerId>,
    /// Who took it.
    pub victim: PlayerId,
    /// The damage effect.
    pub damage: Option<TagId>,
    /// The weapon that dealt it, when the effect or the attacker's shots
    /// say.
    pub weapon: Option<TagId>,
    /// Points off the shield (a Spartan's full shield is 140).
    pub shield: f64,
    /// Points off the body (a Spartan's full health is 90).
    pub body: f64,
    /// `shield` and `body` come from the victim's vitals around the hit
    /// rather than the effect's numbers.
    pub from_vitals: bool,
}

/// A pickup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Pickup {
    /// Who picked it up.
    pub player: PlayerId,
    /// The item's tag.
    pub item: TagId,
    /// The inventory slot it went into.
    pub slot: u32,
}

/// A throw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Throw {
    /// The thrower.
    pub player: PlayerId,
    /// The grenade, or None for a thrown object.
    pub grenade: Option<Grenade>,
}

/// A projectile going off.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Detonation {
    /// Where.
    pub position: Position,
    /// The grenade, when the projectile is a tracked grenade's.
    pub grenade: Option<Grenade>,
}

/// What a player did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Action {
    /// Who.
    pub player: PlayerId,
    /// What.
    pub kind: ActionKind,
}

/// A kind of action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum ActionKind {
    /// A reload.
    Reload,
    /// A melee swing, with the player it lunged at.
    Melee {
        /// The lunge's target.
        target: Option<PlayerId>,
    },
    /// A zoom level change (0 unzoomed).
    Zoom {
        /// The new level.
        level: u32,
    },
    /// The trigger pulled on an empty weapon.
    EmptyClick,
    /// Active Camo applied.
    Camo,
    /// A thruster dash.
    Thruster,
    /// A grapple.
    Grapple,
    /// The start of a sprint.
    Sprint,
    /// A jump's take-off.
    Jump,
    /// A switch to another weapon slot.
    WeaponSwitch {
        /// The slot now held.
        slot: u32,
    },
}

/// A trip through a teleporter.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Teleport {
    /// Who.
    pub player: PlayerId,
    /// Where they went in.
    pub from: Position,
    /// Where they came out.
    pub to: Position,
}

/// A player leaving a vehicle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct VehicleExit {
    /// Who.
    pub player: PlayerId,
    /// The vehicle: [`Film::vehicle_at`](crate::Film::vehicle_at) finds its
    /// track.
    pub vehicle: EntityId,
    /// The seat, counted from the driver's.
    pub seat: u32,
}

/// Equipment that put an object down, attributed to the player nearest it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct EquipmentUse {
    /// The user.
    pub player: PlayerId,
    /// The equipment.
    pub equipment: TagId,
    /// The object it put down.
    pub object: TagId,
    /// Where the object came down.
    pub position: Position,
}

/// A voice line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct VoiceLine {
    /// The sound tag.
    pub sound: TagId,
    /// Who hears it.
    pub recipients: PlayerMask,
}

/// A line a unit speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Chatter {
    /// The speaker, when the unit is a player's.
    pub speaker: Option<PlayerId>,
    /// The sound tag.
    pub sound: TagId,
    /// The line's priority class.
    pub priority: u32,
    /// The vocalization's index in the game's dialogue tables.
    pub line: Option<u32>,
}

/// A HUD message's argument.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum HudArgument {
    /// A number.
    Number(f64),
    /// A string, as its localization key.
    Text(u32),
    /// A player.
    Player(PlayerId),
}

/// A message on players' HUDs.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct HudMessage {
    /// The message's localization key.
    pub key: u32,
    /// Its arguments, in order; None where an argument's kind isn't read.
    pub arguments: Vec<Option<HudArgument>>,
    /// How many players' HUDs showed it.
    pub viewers: u32,
}

/// A value of a game event.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum GameEventValue {
    /// A word, with its 3-bit type.
    Word {
        /// The type.
        kind: u32,
        /// The value.
        value: u32,
    },
    /// A flag.
    Bit(bool),
    /// 16 bytes of text.
    Text([u8; 16]),
    /// A position, in world units when the film's coordinates are known.
    Position(Option<Position>),
    /// No value.
    Empty,
}

/// An argument of a game event's text, by its 3-bit type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum GameEventArgument {
    /// No value (type 0).
    Empty,
    /// A player (type 1); None when the index names no one.
    Player(Option<PlayerId>),
    /// A value with its type: 3 a string id (`%s`), 4 an integer (`%i`),
    /// the others not told apart.
    Word {
        /// The type, 2 to 7.
        kind: u32,
        /// The value.
        value: u32,
    },
}

/// A mode script's game event: an id whose meaning depends on the mode, up
/// to seven keyed values, and the players it is about.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct GameEvent {
    /// The event's id: its response's position in the mode's event tree.
    pub id: u32,
    /// The 8 bits after the id.
    pub small: u32,
    /// Its values, each under a key (the murmur3 of the property's name).
    pub values: Vec<(u32, GameEventValue)>,
    /// The string id of the event's own text, when it has one.
    pub text: Option<u32>,
    /// The text's arguments, in order.
    pub arguments: Vec<GameEventArgument>,
    /// The players it is about.
    pub players: PlayerMask,
}

/// A flag event.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct FlagEvent {
    /// What happened.
    pub kind: FlagEventKind,
    /// Who did it.
    pub player: PlayerId,
    /// Where: the event's own position, else the player's.
    pub position: Option<Position>,
}

/// What happened to a flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum FlagEventKind {
    /// Picked up from its stand.
    Grab,
    /// Picked up from the ground.
    Steal,
    /// Captured.
    Capture,
    /// Returned.
    Return,
    /// Its carrier killed.
    CarrierKilled,
    /// An assist on a capture.
    CaptureAssist,
}

/// A zone, hill or skull event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct ZoneEvent {
    /// What happened.
    pub kind: ZoneEventKind,
    /// The player it is about.
    pub player: Option<PlayerId>,
    /// The team it is about.
    pub team: Option<TeamId>,
    /// The zone, an index into `Film::zones`.
    pub zone: Option<usize>,
}

/// What happened at a zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum ZoneEventKind {
    /// A zone captured.
    Capture,
    /// A zone secured.
    Secure,
    /// A scoring tick.
    Score,
    /// A hill appeared.
    HillAppear,
    /// A hill went live.
    HillLive,
    /// A hill taken under control.
    HillControlled,
    /// A boss wave began.
    BossWave,
    /// A set began.
    SetStart,
    /// A set was completed.
    SetComplete,
    /// The skull picked up.
    SkullGrab,
    /// The skull spawned.
    SkullSpawn,
    /// A hill won.
    HillWon,
    /// A contest began.
    ContestStart,
    /// A contest ended.
    ContestEnd,
    /// The controlling team left.
    ControlLost,
    /// A zone went neutral.
    Neutral,
    /// A team's hill points reached 70%.
    NearlyWon,
}

/// A respawn countdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct RespawnCountdown {
    /// Who is waiting.
    pub player: PlayerId,
    /// The countdown's length, seconds.
    pub seconds: u32,
}

/// A record of the HighlightEvents chunk: the theater timeline's markers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Highlight {
    /// The player, when the film's roster has their XUID.
    pub player: Option<PlayerId>,
    /// The XUID the record names.
    pub xuid: super::Xuid,
    /// The gamertag the record carries.
    pub gamertag: String,
    /// What it marks.
    pub kind: HighlightKind,
    /// Milliseconds since the match started, on the highlight clock.
    pub time_ms: u32,
    /// The record's tick, when the film's kills or medals fix the highlight
    /// clock.
    pub tick: Option<Tick>,
}

/// What a highlight marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum HighlightKind {
    /// The player killed someone.
    Kill,
    /// The player died.
    Death,
    /// The player earned the medal with this index.
    Medal(u32),
    /// A mode event (an objective tick).
    Mode,
}
