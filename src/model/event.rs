use super::{EntityId, PlayerId, PlayerMask, Position, RideIndex, TeamId, Tick, VehicleIndex, ZoneIndex};
use crate::game::{DamageCause, Grenade, Medal};
use crate::{StringId, TagId};

/// One event and when it happened.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimedEvent {
    /// The tick it happened at.
    pub tick: Tick,
    /// What happened.
    pub event: Event,
}

/// Everything that happens in a film, in one stream.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", tag = "type"))]
pub enum Event {
    /// A player was killed.
    Kill(Kill),
    /// A player earned a medal, from the highlights chunk, when its clock is
    /// known ([`Clock::highlight_zero`](crate::Clock::highlight_zero));
    /// [`Film::medals`](crate::Film::medals) lists every medal.
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
    /// A player destroyed a vehicle of another team.
    VehicleDestroyed(VehicleDestroyed),
    /// A player hijacked a vehicle.
    Hijack(Hijack),
    /// A player used equipment that put an object down.
    EquipmentUse(EquipmentUse),
    /// The announcer or a mode script played a voice line.
    VoiceLine(VoiceLine),
    /// A unit spoke a line.
    Chatter(Chatter),
    /// A message appeared on players' HUDs.
    HudMessage(HudMessage),
    /// A mode script's game event (`PlayerGameEventSmall`).
    GameEvent(GameEvent),
    /// Something happened to a flag.
    Flag(FlagEvent),
    /// Something happened to a power seed in Big Team Stockpile.
    Seed(SeedEvent),
    /// Something happened at a zone or a hill, or to the skull.
    Zone(ZoneEvent),
    /// A player's respawn countdown started.
    RespawnCountdown(RespawnCountdown),
    /// The game counted a headshot kill for a player.
    Headshot(Headshot),
}

/// A headshot kill the game counted: the mode script's headshot event.
///
/// The scoreboard's headshot kills count these. The game sends one within 2
/// ticks of the kill, which [`Kill::headshot`] marks, inferred kills included.
/// PvP modes only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Headshot {
    /// Who.
    pub player: PlayerId,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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
    /// [`Event::VehicleDestroyed`].
    VehicleDestroyed,
    /// [`Event::Hijack`].
    Hijack,
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
    /// [`Event::Seed`].
    Seed,
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
            Event::VehicleDestroyed(_) => EventKind::VehicleDestroyed,
            Event::Hijack(_) => EventKind::Hijack,
            Event::EquipmentUse(_) => EventKind::EquipmentUse,
            Event::VoiceLine(_) => EventKind::VoiceLine,
            Event::Chatter(_) => EventKind::Chatter,
            Event::HudMessage(_) => EventKind::HudMessage,
            Event::GameEvent(_) => EventKind::GameEvent,
            Event::Flag(_) => EventKind::Flag,
            Event::Seed(_) => EventKind::Seed,
            Event::Zone(_) => EventKind::Zone,
            Event::RespawnCountdown(_) => EventKind::RespawnCountdown,
            Event::Headshot(_) => EventKind::Headshot,
        }
    }

    /// The players the event is about: a kill's victim, killer and assistant, a
    /// hit's attacker and victim, a destroyed vehicle's destroyer and the
    /// riders killed with it, a hijack's hijacker and the player thrown out,
    /// the players a game event was sent to and, for every other event, the
    /// player who acted. A voice line's or a HUD message's audience isn't
    /// counted.
    pub fn players(&self) -> impl Iterator<Item = PlayerId> + use<> {
        let one = |p: PlayerId| [Some(p), None, None];
        let few = match self {
            Event::Kill(k) => [Some(k.victim), k.killer, k.assistant],
            Event::Medal(m) => one(m.player),
            Event::Shot(s) => one(s.player),
            Event::Hit(h) => [h.attacker, Some(h.victim), None],
            Event::Pickup(p) => one(p.player),
            Event::Throw(t) => one(t.player),
            Event::Action(a) => one(a.player),
            Event::Teleport(t) => one(t.player),
            Event::VehicleExit(x) => one(x.player),
            Event::VehicleDestroyed(d) => one(d.player),
            Event::Hijack(h) => [Some(h.player), h.from, None],
            Event::EquipmentUse(u) => one(u.player),
            Event::Chatter(c) => [c.speaker, None, None],
            Event::Flag(f) => one(f.player),
            Event::Seed(s) => one(s.player),
            Event::Zone(z) => [z.player, None, None],
            Event::RespawnCountdown(r) => one(r.player),
            Event::Headshot(h) => one(h.player),
            Event::GameEvent(_) | Event::Detonation(_) | Event::VoiceLine(_) | Event::HudMessage(_) => [None; 3],
        };
        let mask = match self {
            Event::GameEvent(g) => g.players,
            Event::VehicleDestroyed(d) => d.killed,
            _ => PlayerMask::default(),
        };
        few.into_iter().flatten().chain(mask.iter())
    }

    /// Whether the event is about `player`, as [`Event::players`] counts.
    pub fn involves(&self, player: PlayerId) -> bool {
        self.players().any(|p| p == player)
    }
}

/// A kill: the game's `PlayerKilledEvent`, with what dealt the fatal hit. The
/// game sends no such event for some deaths, a bot's among them; those are
/// kills too, marked [`inferred`](Kill::inferred).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Kill {
    /// Who died.
    pub victim: PlayerId,
    /// Who killed them: None when no player is credited (a fall, a kill
    /// volume, an AI). On a suicide it can be the victim; check `suicide`.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub killer: Option<PlayerId>,
    /// The number the kill event carries for the killer's damage, in whole
    /// percent, as the game computes it from its damage records. Values above
    /// 100 occur (up to 239 on the test films). None without a killer: the
    /// game then sends whatever the field held.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub killer_damage_percent: Option<u32>,
    /// Who assisted.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub assistant: Option<PlayerId>,
    /// The assistant's damage percentage, as `killer_damage_percent`. None
    /// without an assistant.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub assistant_damage_percent: Option<u32>,
    /// Whether the death was the victim's own doing.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
    pub suicide: bool,
    /// Whether the game counted the kill as a headshot: a headshot event for
    /// the killer within 2 ticks of the kill. None when the film has no
    /// headshot events at all.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub headshot: Option<bool>,
    /// Whether the scoreboard counted the death: the film's scoreboard records
    /// it, or the highlights chunk does (the game records there every death its
    /// scoreboard counts, and a film can miss the scoreboard's update). It
    /// counts only while a round is under way: the deaths after a round is
    /// decided, until the next starts, are false, among them those the game
    /// deals everyone still alive when the match ends. In a small lobby, whose
    /// scoreboard isn't read, a death with cause World 359 to 365 ticks
    /// before the film's end is false, and every other death is None.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub scored: Option<bool>,
    /// What the last hit on the victim in the 2 s before was.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub cause: Option<DamageCause>,
    /// The weapon: the one the fatal damage belongs to, else the killer's
    /// last shot in the 2 s before.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub weapon: Option<TagId>,
    /// Where the victim was.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    /// Whether the film has no kill event for this death: the game sends none
    /// for a bot's death. The tick is when the victim let go of the life, else
    /// its last sample. `killer` is the player whose kill the scoreboard
    /// counted then, else the victim when it counted a suicide, else whom the
    /// game's rule credits from the film's hits: the fatal hit's attacker when
    /// an enemy, else the most recent enemy to hit the victim in the 6 s
    /// before, else the most recent player to. `assistant` is the enemy, other
    /// than the killer, whose hits on the life in those 6 s took the most of
    /// its shield and health, at least a fifth, with that share as
    /// `assistant_damage_percent`; `killer_damage_percent` is None.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
    pub inferred: bool,
    /// Whether the killer and the victim were on one team the tick before
    /// (Infection moves a victim at the kill): a betrayal. False for a
    /// suicide.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
    pub betrayal: bool,
}

impl Kill {
    /// The player credited with the kill: the killer, unless the death was
    /// the victim's own.
    pub fn credited_killer(&self) -> Option<PlayerId> {
        self.killer.filter(|&k| k != self.victim && !self.suicide)
    }

    /// Whether the scoreboard counted the death: false when [`Kill::scored`]
    /// is, for the deaths after a round is decided, until the next starts,
    /// among them those the game deals everyone still alive when the match
    /// ends. A death whose count isn't known counts.
    pub fn is_scored(&self) -> bool {
        self.scored != Some(false)
    }

    /// Whether the game counted the kill as a headshot.
    pub fn is_headshot(&self) -> bool {
        self.headshot == Some(true)
    }
}

/// A medal award.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct MedalAward {
    /// Who earned it.
    pub player: PlayerId,
    /// The game's medal index.
    pub index: u32,
}

impl MedalAward {
    /// The medal, or None for an index the library's medal table lacks.
    pub fn medal(&self) -> Option<Medal> {
        Medal::from_index(self.index)
    }
}

/// A round fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Shot {
    /// The shooter.
    pub player: PlayerId,
    /// The weapon.
    pub weapon: TagId,
}

/// Damage landing on a player: every damage effect the game applies to a
/// player's biped, those that take nothing included, such as a collision
/// with no attacker.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Hit {
    /// Who dealt it; None for damage with no player behind it (a fall).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub attacker: Option<PlayerId>,
    /// Who took it.
    pub victim: PlayerId,
    /// The damage effect.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub damage: Option<TagId>,
    /// The weapon that dealt it, when the effect or the attacker's shots
    /// say.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub weapon: Option<TagId>,
    /// Points off the shield (a Spartan's full shield is 140).
    pub shield: f64,
    /// Points off the body (a Spartan's full health is 90).
    pub body: f64,
    /// Whether `shield` and `body` come from the victim's vitals around the
    /// hit rather than the effect's numbers: 8-bit readings, so within about
    /// 2 points.
    pub from_vitals: bool,
}

/// A pickup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Pickup {
    /// Who picked it up.
    pub player: PlayerId,
    /// The item's tag.
    pub item: TagId,
    /// The inventory slot it went into, as the pickup event names it (3
    /// bits).
    pub slot: u32,
}

/// A throw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Throw {
    /// The thrower.
    pub player: PlayerId,
    /// The grenade, or None for a thrown object.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub grenade: Option<Grenade>,
}

/// A projectile going off.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Detonation {
    /// Where.
    pub position: Position,
    /// The grenade, when the projectile is a tracked grenade's.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub grenade: Option<Grenade>,
}

/// What a player did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Action {
    /// Who.
    pub player: PlayerId,
    /// What.
    pub kind: ActionKind,
}

/// A kind of action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ActionKind {
    /// A reload.
    Reload,
    /// A melee swing, with the player it lunged at.
    #[non_exhaustive]
    Melee {
        /// The lunge's target.
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
        target: Option<PlayerId>,
    },
    /// A zoom level change (0 unzoomed).
    #[non_exhaustive]
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
    #[non_exhaustive]
    WeaponSwitch {
        /// The slot now held.
        slot: u32,
    },
}

/// A trip through a teleporter.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct VehicleExit {
    /// Who.
    pub player: PlayerId,
    /// The vehicle: [`Film::vehicle_at`](crate::Film::vehicle_at) finds its
    /// track.
    pub vehicle: EntityId,
    /// The seat, 0 the driver's.
    pub seat: u32,
}

/// A vehicle destroyed by a player of another team, as the mode scripts
/// score it.
///
/// When damage puts a vehicle in its doom state, the game passes the mode
/// scripts the player it credits; they give that player a personal score for
/// the vehicle's type when the vehicle belongs to a team and the player is
/// hostile to it. A vehicle with no team, or one a friend destroyed, has no
/// event. The match stats count vehicle destroys by another rule, and the
/// two don't agree player by player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct VehicleDestroyed {
    /// The player credited.
    pub player: PlayerId,
    /// The vehicle's type, as the scripts score it: a Gungoose is a
    /// Mongoose, a Rocket Warthog and a Razorback are Warthogs.
    pub vehicle: crate::game::Vehicle,
    /// The vehicle's track, an index into
    /// [`Film::vehicles`](crate::Film::vehicles): the vehicle of that type
    /// hit last in the 2 s before, preferring the credited player's hits. None
    /// when no such hit is recorded.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub track: Option<VehicleIndex>,
    /// The damage effect of that hit: often the wreck's own blast.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub damage: Option<TagId>,
    /// What destroyed it, as for a kill ([`Kill::cause`]): the cause of the
    /// hit or kill that `weapon` was taken from, else of the credited
    /// player's last hit on the vehicle.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub cause: Option<DamageCause>,
    /// The weapon that destroyed it, or the vehicle whose weapon did: the
    /// one the credited player's last hit on it belongs to (the wreck's own
    /// blast belongs to none), else the weapon a rider they killed with it
    /// died to, else their last shot in the 2 s before.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub weapon: Option<TagId>,
    /// The riders who died with it: killed within three ticks of it while
    /// seated in it.
    pub killed: PlayerMask,
}

/// A hijack, as the mode scripts score it: the game passes them the vehicle,
/// the player getting in and the one thrown out, and they give the player
/// getting in a personal score for the vehicle's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Hijack {
    /// The hijacker.
    pub player: PlayerId,
    /// The vehicle's type, as for [`VehicleDestroyed::vehicle`].
    pub vehicle: crate::game::Vehicle,
    /// The hijacker's ride, an index into [`Film::rides`](crate::Film::rides):
    /// their ride in a vehicle of that type that starts nearest the hijack,
    /// within 2 s. The game seats a hijacker first in a seat of their own
    /// beside the rider (on a Ghost, seat 1 or 3), so that is this ride's
    /// seat. Their next ride, in the rider's seat, starts once the rider is
    /// out.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub ride: Option<RideIndex>,
    /// The player thrown out: of the riders already seated when the
    /// hijacker's ride starts (the hijacker boards into a seat of their own
    /// while the rider is still in theirs), the one whose ride ends first,
    /// within 3 s of the hijack. The film's event doesn't name them.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub from: Option<PlayerId>,
}

/// Equipment that put an object down, attributed to the player nearest it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

/// A voice line the announcer or a mode script played, with the players who
/// hear it. The game can send one line to its listeners separately, each its
/// own event at the same tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct VoiceLine {
    /// The sound's tag. The library's catalogue names no sounds.
    pub sound: TagId,
    /// The players who hear it.
    pub recipients: PlayerMask,
}

/// A line a unit speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Chatter {
    /// The speaker, when the unit is a player's.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub speaker: Option<PlayerId>,
    /// The sound tag.
    pub sound: TagId,
    /// The line's priority class: the game's `DialoguePriority`.
    pub priority: u32,
    /// The vocalization's index in the game's dialogue tables.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub line: Option<u32>,
}

/// A HUD message's argument.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum HudArgument {
    /// A number.
    Number(f64),
    /// A string, as its localization key.
    Text(StringId),
    /// A player.
    Player(PlayerId),
}

/// A message on players' HUDs.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct HudMessage {
    /// The message's localization key: [`game::text`](crate::game::text)
    /// gives its English.
    pub key: StringId,
    /// Its arguments, in order; None where an argument's kind isn't read.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub arguments: Vec<Option<HudArgument>>,
    /// How many players' HUDs showed it.
    pub viewers: u32,
}

/// A value of a game event.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum GameEventValue {
    /// A word, with its 3-bit type.
    #[non_exhaustive]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum GameEventArgument {
    /// No value (type 0).
    Empty,
    /// A player (type 1); None when the index names no one.
    Player(Option<PlayerId>),
    /// A value with its type: 3 a string id (`%s`), 4 an integer (`%i`),
    /// the others not told apart.
    #[non_exhaustive]
    Word {
        /// The type, 2 to 7.
        kind: u32,
        /// The value.
        value: u32,
    },
}

/// A mode script's game event: an id whose meaning depends on the mode, up
/// to seven keyed values, and the player it was sent to.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct GameEvent {
    /// The event's id: its response's position in the mode's event tree.
    pub id: u32,
    /// Its 8-bit sequence number, one more than the event sent before it.
    /// An event the game sent again repeats its number; the library keeps
    /// one of the two.
    pub sequence: u32,
    /// Its values, each under its property's name.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub values: Vec<(StringId, GameEventValue)>,
    /// The event's own text, when it has one: [`game::text`](crate::game::text)
    /// gives its English.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub text: Option<StringId>,
    /// The text's arguments, in order.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub arguments: Vec<GameEventArgument>,
    /// The player it was sent to, as a set (one player on every event of
    /// the test films). The game sends a response once to each of its
    /// targets, each its own event: a medal's or a personal score's names the
    /// player who earned it, and a kill feed line comes once for each player
    /// shown it. To show each line once, keep the events sent to one player,
    /// the one the viewer follows (the `feed` example does).
    pub players: PlayerMask,
}

/// A flag event.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct FlagEvent {
    /// What happened.
    pub kind: FlagEventKind,
    /// Who did it.
    pub player: PlayerId,
    /// Where: the event's own position, else the player's.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// What happened to a flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum FlagEventKind {
    /// Picked up where it was dropped: the match stats' `FlagGrabs`.
    Grab,
    /// Taken from its stand: the match stats' `FlagSteals`.
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

impl std::fmt::Display for FlagEventKind {
    /// A short lowercase name: "steal", "carrier killed".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FlagEventKind::Grab => "grab",
            FlagEventKind::Steal => "steal",
            FlagEventKind::Capture => "capture",
            FlagEventKind::Return => "return",
            FlagEventKind::CarrierKilled => "carrier killed",
            FlagEventKind::CaptureAssist => "capture assist",
        })
    }
}

/// A power seed scoring event in Big Team Stockpile.
///
/// The mode's scripts name each one, and on the test films each player's count
/// of each kind equals the match stats'. Picking a seed up is an
/// [`Event::Pickup`] of the seed
/// ([`game::Weapon::GenericPowerSeed`](crate::game::Weapon::GenericPowerSeed)).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct SeedEvent {
    /// What happened.
    pub kind: SeedEventKind,
    /// Who did it.
    pub player: PlayerId,
    /// Where: the event's own position, else the player's.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
}

/// What happened to a power seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum SeedEventKind {
    /// Taken from the other team's stockpile: the match stats'
    /// `PowerSeedsStolen`.
    Steal,
    /// Delivered to the player's team's stockpile: the match stats'
    /// `PowerSeedsDeposited`.
    Deposit,
    /// Its carrier killed: the match stats' `PowerSeedCarriersKilled`.
    CarrierKilled,
}

impl std::fmt::Display for SeedEventKind {
    /// A short lowercase name: "deposit", "carrier killed".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SeedEventKind::Steal => "steal",
            SeedEventKind::Deposit => "deposit",
            SeedEventKind::CarrierKilled => "carrier killed",
        })
    }
}

/// A zone, hill or skull event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct ZoneEvent {
    /// What happened.
    pub kind: ZoneEventKind,
    /// The player it is about, when the event names one: on the test films,
    /// a skull grab and a Strongholds secure.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub player: Option<PlayerId>,
    /// The team it is about, when the event names one.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub team: Option<TeamId>,
    /// The zone, an index into [`Film::zones`](crate::Film::zones), when the
    /// event names one: on the test films, captures, and a King of the Hill
    /// hill appearing, going live and secured. Scoring ticks, contests and
    /// the skull's events name none.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub zone: Option<ZoneIndex>,
}

/// What happened at a zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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

impl std::fmt::Display for ZoneEventKind {
    /// A short lowercase name: "capture", "skull grabbed".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ZoneEventKind::Capture => "capture",
            ZoneEventKind::Secure => "secure",
            ZoneEventKind::Score => "score",
            ZoneEventKind::HillAppear => "hill appeared",
            ZoneEventKind::HillLive => "hill live",
            ZoneEventKind::HillControlled => "hill controlled",
            ZoneEventKind::BossWave => "boss wave",
            ZoneEventKind::SetStart => "set started",
            ZoneEventKind::SetComplete => "set completed",
            ZoneEventKind::SkullGrab => "skull grabbed",
            ZoneEventKind::SkullSpawn => "skull spawned",
            ZoneEventKind::HillWon => "hill won",
            ZoneEventKind::ContestStart => "contest started",
            ZoneEventKind::ContestEnd => "contest ended",
            ZoneEventKind::ControlLost => "control lost",
            ZoneEventKind::Neutral => "neutral",
            ZoneEventKind::NearlyWon => "nearly won",
        })
    }
}

/// A respawn countdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct RespawnCountdown {
    /// Who is waiting.
    pub player: PlayerId,
    /// The countdown's length, seconds.
    pub seconds: u32,
}

/// A record of the highlights chunk: the theater timeline's markers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Highlight {
    /// The player, when the film's roster has their XUID.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub player: Option<PlayerId>,
    /// The XUID the record names.
    pub xuid: super::Xuid,
    /// The gamertag the record carries.
    pub gamertag: String,
    /// What it marks.
    pub kind: HighlightKind,
    /// Milliseconds since the film's first frame, on the highlight clock
    /// ([`Clock::highlight_zero`](crate::Clock::highlight_zero)), not since
    /// the match clock started.
    pub time_ms: u32,
    /// The record's tick, when the highlight clock is known
    /// ([`Clock::highlight_zero`](crate::Clock::highlight_zero)).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub tick: Option<Tick>,
}

/// What a highlight marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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
