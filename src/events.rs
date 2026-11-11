//! The event view of a type-0 payload: the game events of one tick, each a
//! 7-bit type, three optional object handles and a body the type's own
//! reader lays out.
//!
//! ```text
//! { [1][type:7] 3 of ([present:1] handle) body(type) }*  [0]
//! ```
//!
//! The handle widths depend on the handle's kind, which the event type fixes
//! per slot ([`handle_kinds`]). The bodies `body` knows are read the way the
//! game's readers read them; a type whose body isn't known ends the view's
//! walk ([`Stop::EventBody`]). The fields of the types [`Body`] names are
//! kept; every other known body is read past.
//!
//! The widths here are the game's writers' constants: the melee kind is 3
//! bits, a direction 19 (as in ai_jump), the object-impact floats 7.

use crate::bits::bits_at;
use crate::payload::Stop;

/// Bits of an entity index; an object handle of a kind that names any
/// entity has this many.
pub const INDEX_BITS: u32 = 13;
/// Longest event view walked: a longer run is a misread. Warzone's ticks hold
/// up to 160.
const MAX_EVENTS: usize = 256;
/// Width of the melee-initiate kind field.
const MELEE_KIND_BITS: u32 = 3;
/// Direction width of the AI-jump vector.
const AI_JUMP_DIRECTION_BITS: u32 = 19;
/// Float widths of the projectile-object-impact body: two 7-bit floats and
/// two 19-bit directions.
const OBJECT_IMPACT_FLOATS: [u32; 4] = [7, 7, 19, 19];

/// Event types whose fields are kept.
pub const WEAPON_FIRE: u32 = 36;
pub const PLAYER_KILLED: u32 = 85;

/// How the level lays out a position: its widths, and on a level of more
/// than two structure BSPs (`components::Params::set_level`) the box index's
/// width, the play space's box and the other boxes' widths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub widths: [u32; 3],
    pub box_bits: u32,
    pub play_box: u32,
    pub box_widths: [[u32; 3]; 4],
}

impl Layout {
    /// The widths of box `b`: its own when the level lists them, else the
    /// play space's.
    fn widths_of(&self, b: u32) -> [u32; 3] {
        match self.box_widths.get(b as usize) {
            Some(&x) if x[0] > 0 => x,
            _ => self.widths,
        }
    }
}

impl From<[u32; 3]> for Layout {
    fn from(widths: [u32; 3]) -> Layout {
        Layout { widths, box_bits: 1, play_box: 0, box_widths: [[0; 3]; 4] }
    }
}

impl From<&crate::components::Params> for Layout {
    fn from(p: &crate::components::Params) -> Layout {
        Layout { widths: p.widths, box_bits: p.box_bits, play_box: p.play_box, box_widths: p.box_widths }
    }
}

/// An object handle: an entity index and its 2-bit generation. A unit
/// handle (kind 1 with its unit bit set, kinds 4 and 6) indexes the unit
/// range, entities 512-1023 (bipeds, then vehicles). A kind-0 handle keeps
/// the entity itself (its index read from `OBJECT_BASE`); kinds 2, 3 and
/// 5 keep their 8-bit index within bipeds, vehicles and class 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    pub index: u32,
    pub generation: u32,
    pub unit: bool,
}

/// First entity index of the unit range (bipeds 512-767, vehicles 768-1023).
pub const UNIT_BASE: u32 = 512;
/// First entity index of the object range, which kind-0 handles count from.
pub const OBJECT_BASE: u32 = 512;

impl Handle {
    /// The entity index the handle names.
    pub fn entity(&self) -> u32 {
        if self.unit {
            UNIT_BASE + self.index
        } else {
            self.index
        }
    }

    /// The biped slot id (`(generation - 1) * 256 + index`, the per-life id
    /// of a biped) when the handle names a biped.
    pub fn biped_slot(&self) -> Option<i32> {
        let e = self.entity();
        ((UNIT_BASE..UNIT_BASE + 256).contains(&e) && self.generation > 0)
            .then(|| (self.generation as i32 - 1) * 256 + (e - UNIT_BASE) as i32)
    }

    /// The vehicle id (`(generation - 1) * 256 + index` in the vehicles' own
    /// space) of a handle in the vehicle range.
    pub fn vehicle_slot(&self) -> Option<i32> {
        let e = self.entity();
        ((UNIT_BASE + 256..UNIT_BASE + 512).contains(&e) && self.generation > 0)
            .then(|| (self.generation as i32 - 1) * 256 + (e - UNIT_BASE - 256) as i32)
    }
}

/// Weapon fire (`action_weapon_fire`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fire {
    /// The 5-bit player index in the body, when present.
    pub player: Option<u32>,
    /// The weapon's tag id, when present.
    pub weapon: Option<u32>,
    /// The weapon variant's string id (murmur3 of its name).
    pub variant: u32,
}

/// damage_aftermath fields as read: the damage tag, the attacker's player
/// index, a 19-bit direction, an optional 31-bit value and three small fields
/// (5, 5 and 6 bits), the first a scale that is 15 on a full direct hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Damage {
    pub tag: Option<u32>,
    pub player: Option<u32>,
    pub direction: u32,
    pub second: Option<u32>,
    pub a: [u32; 3],
}

/// `PlayerKilledEvent`, in stream order: `a` the victim, `b` the killer, `x`
/// the killer's percent of the damage, `flag` the death was the victim's own,
/// `c` the assistant, `y` the assistant's percent (filler when there's no
/// assistant).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Killed {
    pub a: Option<u32>,
    pub b: Option<u32>,
    pub x: u32,
    pub flag: bool,
    pub c: Option<u32>,
    pub y: u32,
}

/// A value of PlayerGameEventSmall, by its 3-bit type: none (0), 32 bits
/// with their type (1, 2, 3, 6), a bit (4), a string of up to 16 bytes (5),
/// a position (7; level codes, None outside the box).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    None,
    Word(u32, u32),
    Bit(bool),
    Str([u8; 16]),
    Pos(Option<[u32; 3]>),
}

/// A parameter of PlayerGameEventSmall, by its 3-bit type: none (0), a
/// player index (1), 24 or 32 bits (2), 32 bits (3-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    None,
    Player(Option<u32>),
    Word(u32, u32),
}

/// PlayerGameEventSmall (82): a 32-bit id and an 8-bit sequence number, up
/// to 7 keyed values, an optional 32-bit id with up to 7 parameters, and a
/// last 32 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameEvent {
    pub id: u32,
    pub small: u32,
    pub values: [Option<(u32, Value)>; 7],
    pub extra: Option<(u32, [Option<Param>; 7])>,
    pub last: u32,
}

/// Game events the game sent again. Each one carries an 8-bit sequence
/// number, one more than the event before; a film can record an event a
/// second time 10 to 25 ticks later, sequence number and all. A repeat of
/// one of the last 128 events within `RESEND_FRAMES` is that.
#[derive(Debug, Default)]
pub struct Resends {
    recent: std::collections::VecDeque<(usize, GameEvent)>,
}

impl Resends {
    const RESEND_FRAMES: usize = 60;

    /// Whether `g`, read in `frame`, is new rather than sent again.
    pub fn fresh(&mut self, frame: usize, g: &GameEvent) -> bool {
        while self.recent.front().is_some_and(|x| x.0 + Self::RESEND_FRAMES < frame) {
            self.recent.pop_front();
        }
        if self.recent.iter().any(|x| x.1 == *g) {
            return false;
        }
        if self.recent.len() == 128 {
            self.recent.pop_front();
        }
        self.recent.push_back((frame, *g));
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    Fire(Fire),
    Killed(Killed),
    /// biped_pickup (9): the slot, 3 bits, and the item's tag id.
    Pickup {
        slot: u32,
        tag: Option<u32>,
    },
    /// damage_aftermath (0): the damage tag, a player index, a 19-bit
    /// direction, and the three small fields after it (5, 5, 6 bits).
    Damage(Damage),
    /// projectile_detonate (5): the projectile's tag and where it went off
    /// (level position codes; None outside the box).
    Detonate {
        tag: Option<u32>,
        at: Option<[u32; 3]>,
    },
    /// biped_throw_initiate (39): a grenade type (3 bits) or a thrown
    /// object's tag, and the thrower's player index.
    Throw {
        grenade: Option<u32>,
        tag: Option<u32>,
        player: Option<u32>,
    },
    /// PlayerGameEventSmall (82).
    Game(GameEvent),
    /// weapon_reload (38): the reloader's player index.
    Reload {
        player: Option<u32>,
    },
    /// biped_melee_initiate (40): the attacker's player index; the second
    /// handle is the lunge target.
    Melee {
        player: Option<u32>,
    },
    /// unit_zoom (21): the zoom level, 0 (none) to 2.
    Zoom {
        level: u32,
    },
    /// weapon_empty_click (11): the player's index.
    EmptyClick {
        player: Option<u32>,
    },
    /// PowerUpApplied (100): the power-up's tag (Active Camo).
    PowerUp {
        tag: Option<u32>,
    },
    /// teleport_effects (116) with its two places: where the unit left and
    /// where it arrived (level codes; None outside the box).
    Teleport {
        from: Option<[u32; 3]>,
        to: Option<[u32; 3]>,
    },
    /// unit_exit_vehicle (22): the seat left; the handles are the unit and
    /// the vehicle.
    Exit {
        seat: u32,
    },
    /// Dialogue2D (76): a voice line's tag, an optional 32-bit value, a
    /// player index and 32 bits.
    Dialogue {
        tag: Option<u32>,
        a: Option<u32>,
        player: Option<u32>,
        b: u32,
    },
    /// AIDialog (75): the kind, `DialoguePriority` (5 bits: 0 none, 1
    /// autonomic, 2 recall, 3 idle, 4 comment, 5 idle_response, 6 postcombat,
    /// 7 combat, 8 status, 9 respond, 10 warn, 11 act, 12 react, 13
    /// involuntary, 14 scream, 15 scripted, 16 death), an optional 32-bit value
    /// (the sound), an optional 12-bit value (the line: an index into the
    /// seven `dsdt` tags' vocalizations), a player index, then 1 or 2 bits.
    AiDialog {
        kind: u32,
        value: Option<u32>,
        line: Option<u32>,
        player: Option<u32>,
    },
    /// unit_teleported (32): two places (level codes; None outside the box)
    /// and two tags, in the writer's order; the first handle is the unit.
    Teleported {
        from: Option<[u32; 3]>,
        to: Option<[u32; 3]>,
        tags: [Option<u32>; 2],
    },
    /// player_set_respawn_target_transform (64): the player's slot, the
    /// place and three floats (their bits; f32::from_bits).
    RespawnTarget {
        player: u32,
        at: Option<[u32; 3]>,
        floats: [u32; 3],
    },
    /// A known body read past.
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    /// Absolute bit of the event's type field.
    pub at: i64,
    pub kind: u32,
    pub handles: [Option<Handle>; 3],
    pub body: Body,
}

/// MSB-first reader over one payload.
pub(crate) struct Bits<'a> {
    pub c: &'a [u8],
    pub at: i64,
}

impl Bits<'_> {
    pub fn bits(&mut self, n: u32) -> u32 {
        let v = if n == 0 { 0 } else { bits_at(self.c, self.at, n.min(32)) };
        self.at += n as i64;
        v
    }
    pub fn flag(&mut self) -> bool {
        self.bits(1) == 1
    }
    /// `[set:1]`, then `n` bits when set.
    fn opt(&mut self, n: u32) -> Option<u32> {
        self.flag().then(|| self.bits(n))
    }
    /// `[none:1]`, then `n` bits when not none.
    fn unless(&mut self, n: u32) -> Option<u32> {
        (!self.flag()).then(|| self.bits(n))
    }
    /// A tag id: `[present:1]`, then 32 bits.
    fn tag(&mut self) -> Option<u32> {
        self.opt(32)
    }
    /// A player index: `[none:1]`, else 5 bits.
    fn player(&mut self) -> Option<u32> {
        self.unless(5)
    }
    /// A handle of `kind` (the game's handle table): kinds 7 and 8 name any
    /// entity (13 bits), kind 0 any object (13 bits); kind 1 reads a unit bit
    /// first (1: a 9-bit unit index); kinds 2, 3 and 5 have 8 bits, 4 and 6
    /// 9 bits (units). Then a 2-bit generation.
    ///
    /// A kind-0 handle's index counts from entity 512 (kinds 0 and 1 share
    /// the object range's base), so it is kept as `512 + index`.
    pub fn handle(&mut self, kind: u32) -> Handle {
        if kind == 0 {
            let index = OBJECT_BASE + self.bits(INDEX_BITS);
            return Handle { index, generation: self.bits(2), unit: false };
        }
        let (n, unit) = match kind {
            1 => {
                if self.flag() {
                    (9, true)
                } else {
                    (INDEX_BITS, false)
                }
            }
            2 | 3 | 5 => (8, false),
            4 | 6 => (9, true),
            _ => (INDEX_BITS, false),
        };
        let index = self.bits(n);
        Handle { index, generation: self.bits(2), unit }
    }
    /// `[present:1]`, then a handle of `kind`.
    pub(crate) fn opt_handle(&mut self, kind: u32) -> Option<Handle> {
        self.flag().then(|| self.handle(kind))
    }
    /// A position at `precision` bits (16 = the level's widths): `[raw:1]`;
    /// 0: a box index and three fields at that box's widths less the
    /// precision drop; 1: three fields of 22 bits less the drop.
    /// A position (`pos`) kept: the codes in the play space's box at full
    /// width (the low bits dropped by the precision as 0), None in another box
    /// the level lists.
    fn pos_codes(&mut self, layout: Layout, precision: u32) -> Option<[u32; 3]> {
        let d = 16 - precision;
        if !self.flag() {
            let b = self.bits(layout.box_bits);
            let codes = layout.widths_of(b).map(|w| self.bits(w.saturating_sub(d)) << d.min(w));
            // A box the level doesn't list reads as the play space (as `components::Reader::point`).
            (b == layout.play_box || layout.box_widths.get(b as usize).is_none_or(|x| x[0] == 0)).then_some(codes)
        } else {
            for _ in 0..3 {
                self.bits(22 - d);
            }
            None
        }
    }
    pub(crate) fn pos(&mut self, layout: Layout, precision: u32) {
        self.pos_codes(layout, precision);
    }
    /// `[zero:1]`, else a direction and a magnitude.
    fn vec(&mut self, dir: u32, mag: u32) {
        if !self.flag() {
            self.bits(dir + mag);
        }
    }
}

/// The handle kinds of an event type's three handle slots; None for a type
/// whose handle table isn't known.
pub fn handle_kinds(t: u32) -> Option<[u32; 3]> {
    Some(match t {
        0 | 20 | 22 | 53 | 54 | 98 => [1, 1, 7],
        1 | 2 | 7 | 31 | 32 | 35 | 36 | 37 | 58 | 75 | 76 | 86 | 100 | 110 | 118 => [1, 8, 7],
        3 | 4 | 19 | 23 | 28 | 59 | 60 | 61 | 69 | 81 | 82 | 83 | 84 | 107 => [0, 8, 7],
        5 | 6 => [5, 8, 7],
        9 | 11 | 38 | 39 | 42 | 45 | 51 | 63 | 70..=74 | 102 => [2, 8, 7],
        10 | 12 | 15 | 16 | 34 | 80 | 85 | 99 | 120 | 121 | 122 => [8, 8, 7],
        13 | 21 | 30 | 46 | 47 | 48 | 78 => [4, 8, 7],
        18 | 29 | 55 | 62 | 64 | 66 | 67 | 68 | 77 | 87..=92 | 94..=97 | 101 | 108 | 111..=114 => [6, 8, 7],
        27 | 103..=106 | 109 => [0, 0, 7],
        40 | 43 | 44 | 52 | 57 | 93 | 117 | 119 => [2, 0, 7],
        41 | 65 => [3, 8, 7],
        8 => [2, 3, 7],
        24..=26 => [0, 2, 7],
        33 => [3, 4, 7],
        49 => [3, 2, 7],
        56 => [5, 1, 7],
        79 => [4, 0, 7],
        50 => [4, 4, 7],
        14 => [2, 2, 7],
        17 => [6, 6, 7],
        115 | 116 => [1, 0, 7],
        _ => return None,
    })
}

/// Walk an event view from `r.at` (its first continuation bit). Ok with the
/// bit after the terminating 0 (the entity view's start), or the stop; the
/// events read before a stop are kept in `out`.
pub(crate) fn walk(r: &mut Bits<'_>, widths: Layout, out: &mut Vec<Event>) -> Result<i64, Stop> {
    let mut n = 0;
    while r.flag() {
        let at = r.at;
        let kind = r.bits(7);
        let Some(kinds) = handle_kinds(kind) else { return Err(Stop::EventBody(kind)) };
        let handles = kinds.map(|k| r.opt_handle(k));
        let body = body(r, kind, widths).ok_or(Stop::EventBody(kind))?;
        out.push(Event { at, kind, handles, body });
        n += 1;
        if n > MAX_EVENTS {
            return Err(Stop::EventCount);
        }
    }
    Ok(r.at)
}

/// One event body; None when the type's reader isn't known. The comments
/// name the game's event type.
fn body(r: &mut Bits<'_>, t: u32, widths: Layout) -> Option<Body> {
    match t {
        // damage_aftermath
        0 => {
            let tag = r.tag();
            let player = r.player();
            let direction = r.bits(19);
            let second = r.opt(19 + 12);
            let a = [r.bits(5), r.bits(5), r.bits(6)];
            let d = Damage { tag, player, direction, second, a };
            r.opt(5);
            let mut last = false;
            for _ in 0..15 {
                last = r.flag();
            }
            if last {
                r.bits(32);
            }
            r.bits(1 + 3 + 5 + 5 + 1 + 4);
            r.unless(10);
            let k = r.bits(4);
            r.opt(32);
            if k == 1 {
                r.bits(8);
            }
            r.bits(4);
            if r.flag() {
                r.handle(0);
            }
            return Some(Body::Damage(d));
        }
        // damage_section_response
        1 => {
            r.bits(5);
            r.unless(4);
            r.bits(3);
            r.opt(19);
        }
        // projectile_detonate
        5 => {
            r.bits(6);
            if !r.flag() {
                r.tag();
                r.bits(32);
            }
            let tag = r.tag();
            let at = r.pos_codes(widths, 15);
            r.bits(19 + 5 + 1 + 9);
            if r.flag() {
                r.bits(10 + 8);
            }
            r.bits(1 + 8 + 2);
            return Some(Body::Detonate { tag, at });
        }
        // projectile_impact_effect
        6 => {
            if !r.flag() {
                r.tag();
                r.bits(32);
            }
            r.bits(7 + 7 + 19);
            r.pos(widths, 12);
            r.bits(19 + 9 + 1);
        }
        // projectile_object_impact_effect
        7 => {
            if !r.flag() {
                r.tag();
                r.bits(32);
            }
            let w = OBJECT_IMPACT_FLOATS;
            r.bits(w[0] + w[1] + w[2] + 2 + 36 + w[3] + 9 + 16 + 1 + 1);
        }
        // biped_pickup
        9 => {
            let slot = r.bits(3);
            return Some(Body::Pickup { slot, tag: r.tag() });
        }
        // weapon_empty_click
        11 => {
            r.unless(2);
            r.tag();
            r.bits(32 + 1);
            return Some(Body::EmptyClick { player: r.player() });
        }
        // Script (the optional fields in their usual state)
        15 => {
            r.bits(15 + 13);
            let n = r.bits(10);
            r.bits(n);
        }
        // unit_zoom
        21 => return Some(Body::Zoom { level: r.bits(2) }),
        // controlToggleResponse
        27 => {
            r.bits(2);
        }
        // unit_exit_vehicle
        22 => {
            let seat = r.bits(6);
            r.bits(1 + 3);
            return Some(Body::Exit { seat });
        }
        WEAPON_FIRE => return Some(Body::Fire(fire(r, widths))),
        // weapon_reload
        38 => {
            r.bits(4);
            return Some(Body::Reload { player: r.player() });
        }
        // biped_throw_initiate
        39 => {
            let (grenade, tag) = if r.bits(1) == 0 {
                (Some(r.bits(3)), None)
            } else {
                let t = r.tag();
                r.bits(4);
                (None, t)
            };
            return Some(Body::Throw { grenade, tag, player: r.player() });
        }
        // biped_melee_initiate
        40 => {
            if r.bits(MELEE_KIND_BITS) == 1 {
                r.bits(2);
            }
            r.bits(2);
            r.tag();
            r.bits(32);
            return Some(Body::Melee { player: r.player() });
        }
        // vehicle_trick
        41 => {
            r.bits(3);
            r.player();
        }
        // weapon_drop, weapon_throw
        46 | 47 => {
            r.bits(1);
            r.unless(2);
            r.tag();
            r.bits(32);
            if t == 47 {
                r.player();
            } else {
                r.bits(1);
            }
        }
        // projectile_supercombine_request, repair_complete
        58 | 118 => {
            r.tag();
        }
        // AIDialog
        75 => {
            let kind = r.bits(5);
            let value = r.opt(32);
            let line = r.unless(12);
            let player = r.player();
            let n = r.bits(1) + 1;
            r.bits(n);
            return Some(Body::AiDialog { kind, value, line, player });
        }
        // Dialogue2D
        76 => {
            let (tag, a, player) = (r.tag(), r.opt(32), r.player());
            return Some(Body::Dialogue { tag, a, player, b: r.bits(32) });
        }
        // ai_jump
        78 => {
            r.pos(widths, 16);
            r.vec(AI_JUMP_DIRECTION_BITS, 10);
            r.bits(2);
        }
        // PlayerGameEventSmall
        82 => {
            let id = r.bits(32);
            let small = r.bits(8);
            let mut values = [None; 7];
            for v in values.iter_mut().take(r.bits(3) as usize) {
                let key = r.bits(32);
                *v = Some((key, typed_value(r, widths)));
            }
            let extra = r.flag().then(|| {
                let id = r.bits(32);
                let mut params = [None; 7];
                for p in params.iter_mut().take(r.bits(3) as usize) {
                    *p = Some(param_value(r));
                }
                (id, params)
            });
            let last = r.bits(32);
            return Some(Body::Game(GameEvent { id, small, values, extra, last }));
        }
        PLAYER_KILLED => {
            return Some(Body::Killed(Killed {
                a: r.player(),
                b: r.player(),
                x: r.bits(32),
                flag: r.flag(),
                c: r.player(),
                y: r.bits(32),
            }))
        }
        // PowerUpApplied
        100 => {
            let tag = r.tag();
            r.bits(32);
            r.tag();
            return Some(Body::PowerUp { tag });
        }
        // unit_teleported (`Pos Pos T T`, the writer's order)
        32 => {
            let from = r.pos_codes(widths, 16);
            let to = r.pos_codes(widths, 16);
            let (a, b) = (r.tag(), r.tag());
            return Some(Body::Teleported { from, to, tags: [a, b] });
        }
        // player_set_respawn_target_transform: the player's 6-bit slot, the
        // place, three raw floats and a flag.
        64 => {
            let player = r.bits(6);
            let at = r.pos_codes(widths, 16);
            let floats = [0; 3].map(|_| r.bits(32).swap_bytes());
            r.bits(1);
            return Some(Body::RespawnTarget { player, at, floats });
        }
        // NetworkedActionRequest: a 5-bit action variant and its body, the
        // bodies of the biped's action list (the writers' layouts of
        // variants 0-5, 7 and 11 are theirs field for field).
        102 => {
            let variant = r.bits(5);
            r.at = crate::components::action_end(r.c, r.at, variant, &crate::components::Params::new(widths.widths))?;
        }
        // PersonalAILifecycleEffect
        109 => {
            r.bits(2);
            r.opt(32);
            r.opt(32);
            r.bits(32);
        }
        // teleport_effects
        116 => {
            if r.flag() {
                r.unless(19);
                r.bits(8);
            }
            r.bits(1);
            if r.tag().is_some() {
                let from = r.pos_codes(widths, 16);
                let to = r.pos_codes(widths, 16);
                return Some(Body::Teleport { from, to });
            }
        }
        // restore_damage_section (`142ef90a4`): `[wide:1]`, then 5 bits
        // (a value under 32), else 2 (under 3).
        2 => {
            let wide = r.flag();
            r.bits(if wide { 5 } else { 2 });
        }
        // biped_board_vehicle (`142f168c0`): the 6-bit seat.
        8 => {
            r.bits(6);
        }
        // biped_melee_clang (`142ef8e08`): two players, an optional tag,
        // a point at precision 12 (`14076e494`) and a 19-bit direction
        // (`14076dc04`).
        12 => {
            r.player();
            r.player();
            r.tag();
            r.pos(widths, 12);
            r.bits(19);
        }
        // motor_system_interruption (`142ef8f44` -> `142ef47c8`): `[wide:1]`,
        // then 7 bits (under 100), else 2 (under 4); then a flag.
        13 => {
            let wide = r.flag();
            r.bits(if wide { 7 } else { 2 });
            r.bits(1);
        }
        // PlayEffectOnObject (`142eebf08`): 32 bits.
        14 => {
            r.bits(32);
        }
        // networked_ai_effect (`142ef8f74`): an optional tag, then
        // `142ef15e0`: a 2-bit form, 0 one point, 1 three points
        // (`142eefa5c`), at precision 16.
        80 => {
            r.tag();
            match r.bits(2) {
                0 => r.pos(widths, 16),
                1 => {
                    for _ in 0..3 {
                        r.pos(widths, 16);
                    }
                }
                _ => {}
            }
        }
        // EquipmentKnockbackPlayer (`14116c344` -> `14076d528`): a vector,
        // a 19-bit direction and a 10-bit magnitude, as ai_jump's.
        104 => r.vec(19, 10),
        // EquipmentObjectKnockedBack (`141118a00`): an optional tag.
        105 => {
            r.tag();
        }
        // ObjectCollisionDamage (`14112134c`): a point at precision 12, a
        // 10-bit direction and a 9-bit index (stored less 1).
        106 => {
            r.pos(widths, 12);
            r.bits(10 + 9);
        }
        // EquipmentTranslocatorTeleportEffects (`140f04fb8`): an optional
        // tag, then two points at precision 16.
        117 => {
            r.tag();
            r.pos(widths, 16);
            r.pos(widths, 16);
        }
        // Types whose body is empty.
        3 | 4 | 23..=26 | 33 | 49 | 54 | 57 | 59 | 92 | 103 => {}
        _ => return None,
    }
    Some(Body::Other)
}

/// A typed value of PlayerGameEventSmall: a 3-bit type, then its value.
fn typed_value(r: &mut Bits<'_>, widths: Layout) -> Value {
    match r.bits(3) {
        0 => Value::None,
        t @ (1 | 2 | 3 | 6) => Value::Word(t, r.bits(32)),
        4 => Value::Bit(r.flag()),
        // A string of up to 16 bytes, NUL-terminated.
        5 => {
            let mut s = [0; 16];
            for c in s.iter_mut() {
                *c = r.bits(8) as u8;
                if *c == 0 {
                    break;
                }
            }
            Value::Str(s)
        }
        _ => Value::Pos(r.pos_codes(widths, 16)),
    }
}

/// A parameter of PlayerGameEventSmall: a 3-bit type, then its value.
fn param_value(r: &mut Bits<'_>) -> Param {
    match r.bits(3) {
        0 => Param::None,
        1 => Param::Player(r.player()),
        2 => {
            if r.flag() {
                Param::Word(2, r.bits(24))
            } else {
                Param::Word(2, r.bits(32))
            }
        }
        t => Param::Word(t, r.bits(32)),
    }
}

/// A target of the fire body: a 2-bit kind (1: a kind-1 handle and an
/// optional 6-bit marker; 2: a kind-2 handle).
fn fire_target(r: &mut Bits<'_>) {
    match r.bits(2) {
        1 => {
            r.handle(1);
            r.opt(6);
        }
        2 => {
            r.handle(2);
        }
        _ => {}
    }
}

/// action_weapon_fire. Two mode bits `a` and `b`, the shooter's player and
/// the weapon (tag and variant), then per-mode fire data: barrels and
/// projectile batches, an aim block with targets, and the origin.
fn fire(r: &mut Bits<'_>, widths: Layout) -> Fire {
    let a = r.flag();
    let b = r.flag();
    r.bits(7 + 1);
    let player = r.player();
    r.unless(2);
    let weapon = r.tag();
    let variant = r.bits(32);
    let out = Fire { player, weapon, variant };
    r.bits(2);
    let mut d = false;
    if b {
        r.bits(1);
        d = r.flag();
    }
    if d {
        r.opt(10);
    }
    if a {
        r.bits(10);
        return out;
    }
    // Barrels (n) and their marker handles (m).
    let (n, m) = if r.flag() {
        (0, 0)
    } else {
        let n = if r.flag() { 1 } else { r.bits(4) };
        let m = if r.flag() {
            0
        } else if r.flag() {
            1
        } else {
            r.bits(4)
        };
        (n, m)
    };
    let mut markers = Vec::new();
    for _ in 0..m {
        markers.push(r.bits(2));
        r.bits(1);
        r.handle(1);
    }
    let base = if n == 1 { 12 } else { 4 };
    let mut any = true;
    for _ in 0..n {
        r.bits(4);
        if r.flag() {
            any = r.bits(3) != 0;
            let idx = r.bits(if m < 3 { 1 } else { 4 }) as usize;
            r.bits(16);
            let w = if markers.get(idx) == Some(&1) { base.min(6) } else { base };
            r.bits(3 * w);
        }
    }
    if any {
        // The aim block: a target with its vector, a second target.
        let mut have = false;
        if r.flag() {
            have = true;
            fire_target(r);
            let g = r.flag();
            let mut vector = g;
            if !g {
                r.bits(4 + 4);
                vector = r.flag();
            }
            if vector {
                r.vec(15, 7);
            }
        }
        if r.flag() {
            fire_target(r);
        }
        if !have {
            r.bits(30);
        }
    }
    if !d {
        r.opt(6);
        r.bits(6);
    }
    if b {
        match r.bits(2) {
            0 => r.pos(widths, 16),
            1 => {
                r.bits(19);
            }
            _ => {}
        }
        r.bits(4);
    } else {
        r.bits(2);
        let e = r.flag();
        r.opt(4);
        if e {
            r.opt(12);
            r.opt(12);
        }
    }
    r.bits(6);
    r.opt(7);
    if r.flag() {
        r.pos(widths, 16);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_networked_action_request_is_read() {
        // [1] type 102, no handles, action variant 3 (2 + 15 bits), the end.
        let bits = format!("1 {:07b} 000 00011 {} 0", 102, "1".repeat(17));
        let mut b: Vec<u8> = bits.bytes().filter(|c| *c == b'0' || *c == b'1').map(|c| c - b'0').collect();
        let n = b.len() as i64;
        b.resize(b.len() + 64, 0);
        let c: Vec<u8> = b.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |v, (i, &x)| v | x << (7 - i))).collect();
        let mut out = Vec::new();
        let mut r = Bits { c: &c, at: 0 };
        assert_eq!(walk(&mut r, [15, 15, 15].into(), &mut out), Ok(n));
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn a_position_reads_at_its_boxs_widths() {
        // Two box bits, the play space box 1, box 0 its own widths.
        let layout = Layout {
            widths: [15, 15, 15],
            box_bits: 2,
            play_box: 1,
            box_widths: [[12, 12, 13], [0; 3], [0; 3], [0; 3]],
        };
        let c = [0b0001_0000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut r = Bits { c: &c, at: 0 };
        assert_eq!(r.pos_codes(layout, 16), None);
        assert_eq!(r.at, 3 + 37);
        let c = [0b0010_0000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut r = Bits { c: &c, at: 0 };
        assert_eq!(r.pos_codes(layout, 16), Some([0; 3]));
        assert_eq!(r.at, 3 + 45);
    }
}
