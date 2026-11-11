//! Zone, hill and skull events from PlayerGameEventSmall (82), for
//! Strongholds, King of the Hill, Firefight King of the Hill and Oddball.
//!
//! The ids at 0x2a0 and up are the mode's own script events: the same id
//! means one thing in Strongholds and another in Oddball, so each mode has
//! its own table, picked by the mode's scripts. An event goes once to each
//! player it's sent to (one bit of the last 32 bits each, bit 31 minus the
//! player index): to one player when it's about them, to a team when it's
//! about the team, or to everyone. A zone is named by a type-6 value under
//! ZONE_KEY, the zone object's entity index or'd with 0x40000000; the object
//! is a crate in the snapshots, which says where it stands.
//!
//! Strongholds: `31e` goes to each capturing player, `322` to everyone with
//! the zone, `335` to the player who secures and `33d` to the scoring team
//! on each scoring tick.
//!
//! Firefight King of the Hill: each of the five sets is `33c` (set start;
//! `33e` the last), `312` (the hill appears, with its position), `313` (it
//! goes live, with its zone, after a 600-frame countdown), `316` (hill
//! controlled, with the zone), `329` (boss wave) and `333` (set complete).
//!
//! Oddball: `339` goes to the player who grabs the skull, `320` to the
//! scoring team on each tick (the carrier's team), and `344`
//! (ballSpawnedResponse in the mode's event scripts) marks the skull's
//! spawn.

use crate::container::Film;
use crate::events::{Body, Layout, Value};
use crate::payload::for_each_event_view;
use std::collections::BTreeMap;

/// Event kinds, as serialized.
pub const CAPTURE: u8 = 0;
pub const SECURE: u8 = 1;
pub const SCORE: u8 = 2;
pub const HILL_APPEAR: u8 = 3;
pub const HILL_LIVE: u8 = 4;
pub const HILL_CONTROLLED: u8 = 5;
pub const BOSS_WAVE: u8 = 6;
pub const SET_START: u8 = 7;
pub const SET_COMPLETE: u8 = 8;
pub const SKULL_GRAB: u8 = 9;
pub const SKULL_SPAWN: u8 = 10;
// King of the Hill.
pub const HILL_WON: u8 = 11;
pub const CONTEST_START: u8 = 12;
pub const CONTEST_END: u8 = 13;
pub const CONTROL_LOST: u8 = 14;
pub const NEUTRAL: u8 = 15;
pub const NEARLY_WON: u8 = 16;

/// The value key a zone's entity is under.
const ZONE_KEY: u32 = 0x436d_4d78;
/// A zone value: the entity index or'd with this.
const ZONE_BIT: u32 = 0x4000_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Strongholds,
    FirefightKoth,
    Oddball,
    /// PvP King of the Hill: its hills are crates of HILL_TAG.
    Koth,
}

/// Who an event is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum About {
    /// Each recipient: one event per player.
    Player,
    /// The recipients' team.
    Team,
    /// Nobody in particular: one event.
    Match,
}

/// Each mode's ids: id, kind, who it's about.
fn table(m: Mode) -> &'static [(u32, u8, About)] {
    match m {
        Mode::Strongholds => {
            &[(0x31e, CAPTURE, About::Player), (0x335, SECURE, About::Player), (0x33d, SCORE, About::Team)]
        }
        Mode::FirefightKoth => &[
            (0x33c, SET_START, About::Match),
            (0x33e, SET_START, About::Match),
            (0x312, HILL_APPEAR, About::Match),
            (0x313, HILL_LIVE, About::Match),
            (0x316, HILL_CONTROLLED, About::Match),
            (0x329, BOSS_WAVE, About::Match),
            (0x333, SET_COMPLETE, About::Match),
        ],
        Mode::Oddball => &[
            (0x339, SKULL_GRAB, About::Player),
            (0x320, SCORE, About::Team),
            (0x344, SKULL_SPAWN, About::Match), // ballSpawnedResponse; 33d is a pickup's sound
        ],
        // Arena and Ranked share the ids; Ranked has no capture phase and
        // never sends 320 or 323. Each team id has a pair for the other
        // team, which says the same.
        Mode::Koth => &[
            (0x31c, HILL_APPEAR, About::Match),
            (0x31d, HILL_LIVE, About::Match),
            (0x320, CAPTURE, About::Team),
            (0x323, SECURE, About::Team),
            (0x325, CONTROL_LOST, About::Team), // the controlling team left
            (0x322, NEUTRAL, About::Match),     // back to neutral
            (0x327, CONTEST_START, About::Match),
            (0x328, CONTEST_END, About::Team), // to the team remaining
            (0x331, NEARLY_WON, About::Team),  // a team's hill points reach 70%
            (0x334, HILL_WON, About::Team),
        ],
    }
}

/// The kinds whose ZONE_KEY value is a zone.
const ZONED: [u8; 6] = [CAPTURE, SECURE, HILL_APPEAR, HILL_LIVE, HILL_CONTROLLED, HILL_WON];

/// A PvP King of the Hill hill's crate ("KOTH Capture Zone"); Strongholds'
/// zones are `f95a2c76`.
pub const HILL_TAG: u32 = 0x01a4_6de6;

/// Each zone-like crate in the film's first snapshot: entity index and tag.
pub fn crates(film: &Film) -> Vec<(u32, u32)> {
    use crate::snapshot::{self, ENTITY_TABLE};
    for (ci, chunk) in film.chunks.iter().enumerate().skip(1) {
        let Some(ps) = film.packets(ci) else { continue };
        let Some(pk) = ps.iter().find(|p| p.kind == ENTITY_TABLE) else { continue };
        return snapshot::entity_table(chunk, pk.start, pk.end)
            .into_iter()
            .filter(|e| e.entity_type == crate::payload::CRATE)
            .filter_map(|e| Some((e.index, snapshot::creation(chunk, &e)?.0.tag)))
            .collect();
    }
    Vec::new()
}

/// Strongholds' capture to everyone, which names the zone captured.
const SH_CAPTURED: u32 = 0x322;

/// The zone mode a film's scripts run, if any: King of the Hill's and
/// Firefight's hill scripts are the bastion scripts, Strongholds' its own.
pub fn mode(scripts: &std::collections::BTreeSet<String>) -> Option<Mode> {
    let has = |s: &str| scripts.contains(s);
    if has("firefightevents.lua") {
        return has("fbastionevents.lua").then_some(Mode::FirefightKoth);
    }
    if has("bastionevents.lua") {
        return Some(Mode::Koth);
    }
    if has("parcel_stronghold.lua") {
        return Some(Mode::Strongholds);
    }
    if has("oddballevents.lua") {
        return Some(Mode::Oddball);
    }
    None
}

/// One event: who it's about (a player, else -1), their team or the
/// recipients' (else -1), the zone's entity index, a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZoneEvent {
    pub frame: i64,
    pub kind: u8,
    pub player: i32,
    pub team: i32,
    pub zone: Option<u32>,
    pub at: Option<[u32; 3]>,
}

/// The film's events in frame order. `layout` is how the level lays out a
/// position, `player_of` maps an engine slot at a frame to the player's
/// index (the events' masks name engine slots), and `team_of` gives a
/// player's team.
pub fn read(
    film: &Film,
    layout: Layout,
    mode: Mode,
    player_of: impl Fn(i32, i64) -> i32,
    team_of: impl Fn(i32) -> Option<i32>,
) -> Vec<ZoneEvent> {
    let ids = table(mode);
    let mut out = Vec::new();
    for_each_event_view(film, layout, |frame, evs| {
        // Per id this frame: the recipients, the zone and the position.
        let mut seen: BTreeMap<u32, (Vec<i32>, Option<u32>, Option<[u32; 3]>)> = BTreeMap::new();
        for e in evs {
            let Body::Game(g) = e.body else { continue };
            if g.id != SH_CAPTURED && !ids.iter().any(|t| t.0 == g.id) {
                continue;
            }
            let s = seen.entry(g.id).or_default();
            s.0.extend((0..32).filter(|p| g.last & (0x8000_0000u32 >> p) != 0).map(|p| player_of(p, frame as i64)));
            for (k, v) in g.values.iter().flatten() {
                match *v {
                    Value::Word(6, w) if *k == ZONE_KEY && w & ZONE_BIT != 0 => s.1 = Some(w & !ZONE_BIT),
                    Value::Pos(Some(p)) if s.2.is_none() => s.2 = Some(p),
                    _ => {}
                }
            }
        }
        let captured = seen.get(&SH_CAPTURED).and_then(|s| s.1);
        for &(id, kind, about) in ids {
            let Some((to, zone, at)) = seen.get(&id) else { continue };
            // Only these kinds are about a zone: on Oddball the same key
            // names the skull's object.
            let zone =
                if ZONED.contains(&kind) { zone.or(if kind == CAPTURE { captured } else { None }) } else { None };
            let ev = |player: i32, team: i32| ZoneEvent { frame: frame as i64, kind, player, team, zone, at: *at };
            match about {
                About::Player => {
                    let mut ps = to.clone();
                    ps.sort();
                    ps.dedup();
                    out.extend(ps.into_iter().map(|p| ev(p, team_of(p).unwrap_or(-1))));
                }
                About::Team => {
                    let mut ts: Vec<i32> = to.iter().filter_map(|&p| team_of(p)).collect();
                    ts.sort();
                    ts.dedup();
                    // To one team only; to both it says nothing of whose.
                    if let [t] = ts[..] {
                        out.push(ev(-1, t));
                    }
                }
                About::Match => out.push(ev(-1, -1)),
            }
        }
    });
    // A hill's appearance names no zone: it's the zone of the next HILL_LIVE.
    for i in 0..out.len() {
        if out[i].kind == HILL_APPEAR && out[i].zone.is_none() {
            out[i].zone = out[i..].iter().find(|e| e.kind == HILL_LIVE).and_then(|e| e.zone);
        }
    }
    out
}

/// Where each of `zones` (entity indices) stands, in engine units: its
/// first position in the type-2 snapshots (`world.rs`). Packet framing only.
pub fn snapshot_places(film: &Film, params: crate::components::Params, zones: &[u32]) -> BTreeMap<u32, [f32; 3]> {
    use crate::snapshot::{self, ENTITY_TABLE};
    let _ = params;
    let mut out = BTreeMap::new();
    for (ci, chunk) in film.chunks.iter().enumerate().skip(1) {
        let Some(ps) = film.packets(ci) else { continue };
        for pk in ps.iter().filter(|p| p.kind == ENTITY_TABLE) {
            // The table scan, not the walk: a walk can stop before the
            // zone crates.
            for e in snapshot::entity_table(chunk, pk.start, pk.end) {
                if !zones.contains(&e.index) || out.contains_key(&e.index) {
                    continue;
                }
                let Some((_, state)) = snapshot::creation(chunk, &e) else { continue };
                if let Some(at) = snapshot::raw_position(chunk, &e, state).filter(|a| *a != [0.0, 0.0, 0.0]) {
                    out.insert(e.index, at);
                }
            }
        }
        if out.len() == zones.len() {
            break;
        }
    }
    out
}

/// The zones the events name, by entity index, each with the first
/// position an event gives for it (a hill's appearance names no zone; the
/// next HILL_LIVE's zone is that hill).
pub fn event_places(events: &[ZoneEvent]) -> BTreeMap<u32, Option<[u32; 3]>> {
    let mut zones: BTreeMap<u32, Option<[u32; 3]>> = BTreeMap::new();
    let mut pending: Option<[u32; 3]> = None;
    for e in events {
        if e.kind == HILL_APPEAR {
            pending = e.at;
        }
        if let Some(z) = e.zone {
            let p = zones.entry(z).or_insert(None);
            if p.is_none() {
                *p = e.at.or(if e.kind == HILL_LIVE { pending } else { None });
            }
        }
    }
    zones
}

// Zone objects: letter, shape and live state.
//
// Each zone is a crate (entity type 38) with a managed object (type 10)
// whose creation data names it: `[version][present:1][index:13][gen:2]`, the
// zone at 512 + index. The manager's component rows 2-17 each hold
// 0x40000000 | a navpoint (type 12) entity index, and the navpoint carries
// what the game draws on the zone:
//
// - row 6, docking filter: a kind-2 clause is the mask of players inside
//   (bit 31 minus the player index).
// - row 14, radial progress (8 bits): 127 is none, 128-253 a capture rising.
// - row 20, visual state group 0: its first team clause is the owner (team
//   + 1, 0 none).
// - rows 21 and 22, read by their group's name since they swap on some
//   navpoints: `has_progress` gives the capturing team and, as its state,
//   `contested` while the zone is contested; `has_text` is below.
// - row 23, group 3: its state is the zone's letter, the string id
//   (murmur3) of 'a', 'b', 'c' and so on.
//
// The zone's own multiplayer properties (crate row 9) are the map variant
// item's Bond struct: its shape (3 box, 2 cylinder), the box's full extents
// along its forward and right axes (a cylinder's radius first) and the
// extents above and below, in 1/65536 engine units; its orientation (crate
// row 2) a turn about the up axis.

/// The navpoint state groups read here, by string id (the `nvpt` tag's names).
const HAS_PROGRESS: u32 = 0x0b55_6074;
const HAS_TEXT: u32 = 0x7244_9896;
const CONTROL_POINT: u32 = 0x2946_0f42;
/// `has_progress`'s state while its zone is contested.
const CONTESTED: u32 = 0x63e8_e53f;
/// `has_text`'s state while its hill counts down to going live ("incoming"):
/// its radial progress is the countdown, not a capture.
const INCOMING: u32 = 0x897e_705d;
/// Managed object, navpoint.
const MANAGER: u32 = 10;
const NAVPOINT: u32 = 12;

/// The letter whose string id is `id`, 'A' to 'Z'.
fn letter_of(id: u32) -> Option<char> {
    (b'a'..=b'z').find(|&c| crate::game::murmur3(&[c]) == id).map(|c| (c - 32) as char)
}

struct Bits<'a> {
    c: &'a [u8],
    at: i64,
}

impl Bits<'_> {
    fn bits(&mut self, n: u32) -> u32 {
        let v = crate::bits::bits_at(self.c, self.at, n);
        self.at += n as i64;
        v
    }
    fn flag(&mut self) -> bool {
        self.bits(1) == 1
    }
}

/// A filter's clauses (kind, negated, value), read as the game writes
/// them: a 4-bit mask of clauses, one bit (`narrow`) or 32, then per clause
/// a 4-bit kind, a negation bit and the kind's value. None at a kind whose
/// width isn't known.
fn filter(r: &mut Bits<'_>, narrow: bool) -> Option<Vec<(u32, bool, u32)>> {
    let mask = r.bits(4);
    r.bits(if narrow { 1 } else { 32 });
    let mut out = Vec::new();
    for _ in 0..mask.count_ones() {
        let k = r.bits(4);
        if k == 0 {
            out.push((0, false, 0));
            continue;
        }
        let neg = r.flag();
        let w = match k {
            1 => 1,
            2 | 9 => 32,
            3 | 7 => 4,
            4 => 9,
            8 => 8,
            _ => return None,
        };
        out.push((k, neg, r.bits(w)));
    }
    Some(out)
}

/// A visual state group: its filter's clauses and its state, or None when
/// the group isn't set.
fn state_group(r: &mut Bits<'_>) -> Option<(u32, Vec<(u32, bool, u32)>, u32)> {
    if !r.flag() {
        return None;
    }
    let group = r.bits(32);
    let clauses = filter(r, true)?;
    let state = r.bits(32);
    Some((group, clauses, state))
}

/// The first team clause's value as a team id, None for none.
fn team_of_clauses(clauses: &[(u32, bool, u32)]) -> Option<i32> {
    clauses.iter().find(|c| c.0 == 3 && !c.1).and_then(|c| (c.2 > 0).then(|| c.2 as i32 - 1))
}

/// A zone's shape in world units: a box (`a` its full extent along its
/// forward axis, `b` along its right) or a cylinder (`a` its radius),
/// `angle` the world direction of its forward axis (radians from +X), the
/// extents above and below its centre, and the team the map gives it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ZoneShape {
    pub cylinder: bool,
    pub a: f64,
    pub b: f64,
    pub top: f64,
    pub bottom: f64,
    pub angle: f64,
    pub team: Option<i32>,
}

/// Bond Compact Binary v2, as much as the multiplayer properties need.
mod bond {
    #[derive(Debug, Clone)]
    pub enum V {
        Int(i64),
        Struct(Vec<(u16, V)>),
        List(Vec<V>),
        Other,
    }
    pub struct R<'a> {
        pub b: &'a [u8],
        pub p: usize,
    }
    impl R<'_> {
        fn u8(&mut self) -> Option<u8> {
            let v = *self.b.get(self.p)?;
            self.p += 1;
            Some(v)
        }
        fn uleb(&mut self) -> Option<u64> {
            let (mut v, mut sh) = (0u64, 0);
            loop {
                let b = self.u8()?;
                v |= u64::from(b & 0x7f) << sh;
                sh += 7;
                if b & 0x80 == 0 || sh > 63 {
                    return Some(v);
                }
            }
        }
        fn val(&mut self, ty: u8) -> Option<V> {
            Some(match ty {
                2 | 3 => V::Int(i64::from(self.u8()?)),
                14 => V::Int(i64::from(self.u8()? as i8)),
                4..=6 => V::Int(self.uleb()? as i64),
                15..=17 => {
                    let u = self.uleb()?;
                    V::Int((u >> 1) as i64 ^ -((u & 1) as i64))
                }
                7 => {
                    self.p += 4;
                    V::Other
                }
                8 => {
                    self.p += 8;
                    V::Other
                }
                9 => {
                    let n = self.uleb()? as usize;
                    self.p += n;
                    V::Other
                }
                18 => {
                    let n = self.uleb()? as usize;
                    self.p += 2 * n;
                    V::Other
                }
                10 => {
                    let n = self.uleb()? as usize;
                    let end = self.p + n;
                    let s = self.fields(end)?;
                    self.p = end;
                    V::Struct(s)
                }
                11 | 12 => {
                    let tc = self.u8()?;
                    let (e, c) = (tc & 0x1f, tc >> 5);
                    let n = if c == 0 { self.uleb()? as usize } else { c as usize - 1 };
                    let mut l = Vec::with_capacity(n.min(256));
                    for _ in 0..n {
                        l.push(self.val(e)?);
                    }
                    V::List(l)
                }
                13 => {
                    let k = self.u8()? & 0x1f;
                    let v = self.u8()? & 0x1f;
                    let n = self.uleb()?;
                    for _ in 0..n {
                        self.val(k)?;
                        self.val(v)?;
                    }
                    V::Other
                }
                _ => return None,
            })
        }
        pub fn fields(&mut self, end: usize) -> Option<Vec<(u16, V)>> {
            let mut out = Vec::new();
            while self.p < end {
                let b = self.u8()?;
                let (ty, mut id) = (b & 0x1f, u16::from(b >> 5));
                if ty == 0 {
                    break;
                }
                if ty == 1 {
                    continue;
                }
                if id == 6 {
                    id = u16::from(self.u8()?);
                } else if id == 7 {
                    id = u16::from(self.u8()?) | (u16::from(self.u8()?) << 8);
                }
                out.push((id, self.val(ty)?));
            }
            Some(out)
        }
    }
    pub fn get(s: &[(u16, V)], id: u16) -> Option<&V> {
        s.iter().find(|f| f.0 == id).map(|f| &f.1)
    }
    /// A list's first struct, or the struct itself.
    pub fn first_struct(v: Option<&V>) -> Option<&[(u16, V)]> {
        match v? {
            V::List(l) => match l.first()? {
                V::Struct(s) => Some(s),
                _ => None,
            },
            V::Struct(s) => Some(s),
            _ => None,
        }
    }
    /// An int field, or a struct field holding `{0: int}`.
    pub fn int(s: &[(u16, V)], id: u16) -> Option<i64> {
        match get(s, id)? {
            V::Int(v) => Some(*v),
            V::Struct(f) => match get(f, 0)? {
                V::Int(v) => Some(*v),
                _ => None,
            },
            _ => None,
        }
    }
}

/// A crate's multiplayer properties at bit `at`: `[0][variant:5]`, a ULEB
/// length, then that many bytes of Bond, bit-aligned.
fn shape_at(c: &[u8], at: i64) -> Option<ZoneShape> {
    let mut r = Bits { c, at };
    if r.flag() {
        return None;
    }
    r.bits(5);
    let (mut len, mut sh) = (0usize, 0);
    loop {
        let b = r.bits(8);
        len |= ((b & 0x7f) as usize) << sh;
        sh += 7;
        if b & 0x80 == 0 || sh > 28 {
            break;
        }
    }
    if len == 0 || len > 4096 {
        return None;
    }
    let bytes: Vec<u8> = (0..len).map(|_| r.bits(8) as u8).collect();
    let mut br = bond::R { b: &bytes, p: 0 };
    let root = br.fields(bytes.len())?;
    let item = bond::first_struct(bond::get(&root, 0))?;
    let geo = bond::first_struct(bond::get(item, 0))?;
    let unit = |id| bond::int(geo, id).map_or(0.0, |v| v as f64 / 65536.0 * crate::calibrate::WORLD_PER_ENGINE);
    let shape = bond::int(geo, 0)?;
    if shape != 2 && shape != 3 {
        return None;
    }
    Some(ZoneShape {
        cylinder: shape == 2,
        a: unit(5),
        b: if shape == 2 { 0.0 } else { unit(6) },
        top: unit(7),
        bottom: unit(8),
        // No orientation component: forward is engine +x, world +y.
        angle: std::f64::consts::FRAC_PI_2,
        team: bond::int(item, 8).map(|t| t as i32),
    })
}

/// A crate's orientation at bit `at` (forward-and-up, dynamic precision):
/// the world angle of its forward axis when it is three floats or a turn
/// about the default up axis; None for the forms not read.
fn angle_at(c: &[u8], at: i64) -> Option<f64> {
    let mut r = Bits { c, at };
    // Engine (x, y) is world (-10 y, 10 x): a forward at engine angle t
    // points along world angle t + pi/2.
    let world = |t: f64| t + std::f64::consts::FRAC_PI_2;
    if r.flag() {
        let (x, y) = (f64::from(f32::from_bits(r.bits(32))), f64::from(f32::from_bits(r.bits(32))));
        return Some(world(y.atan2(x)));
    }
    // Baseline forms, and a direction other than the default: not read.
    if r.flag() || !r.flag() || !r.flag() {
        return None;
    }
    let turn = f64::from(r.bits(30)) / f64::from(1u32 << 30);
    Some(world(turn * std::f64::consts::TAU))
}

/// One zone's state from a frame on: owner and capturing team (team id, -1
/// none), capture progress 0-126, contested, the players inside (bit 31
/// minus the player index).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ZoneState {
    pub frame: i64,
    pub owner: i32,
    pub capturing: i32,
    pub progress: u32,
    pub contested: bool,
    pub inside: u32,
}

/// What the film says of a zone: its letter, shape and state changes in
/// frame order.
#[derive(Clone, Debug, Default)]
pub struct ZoneObject {
    pub letter: Option<char>,
    pub shape: Option<ZoneShape>,
    pub states: Vec<ZoneState>,
}

/// What [`objects_reader`] gathers: each zone's object, and the crates'
/// angles.
#[derive(Default, Debug)]
pub struct ObjectsSeen {
    out: BTreeMap<u32, ZoneObject>,
    angles: std::collections::HashMap<u32, f64>,
}

/// The letters, shapes and states of `zones` (entity indices).
pub fn objects_reader<'a>(
    zones: &[u32],
    seen: &'a mut ObjectsSeen,
) -> impl FnMut(usize, &[u8], &crate::payload::Payload) + use<'a> {
    use crate::payload::OpKind;
    use std::collections::HashMap;
    seen.out = zones.iter().map(|&z| (z, ZoneObject::default())).collect();
    let ObjectsSeen { out, angles } = seen;
    // Manager -> zone, navpoint -> zone. A navpoint's state is kept from its
    // create on: it is created a frame before its manager names it.
    let mut manager: HashMap<u32, u32> = HashMap::new();
    let mut nav: HashMap<u32, u32> = HashMap::new();
    // Each manager row's navpoint: a row set to none unlinks it.
    let mut rows: HashMap<(u32, u32), u32> = HashMap::new();
    let mut countdown: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut nav_state: HashMap<u32, (ZoneState, Option<char>)> = HashMap::new();
    let mut last: HashMap<u32, ZoneState> = HashMap::new();
    let fresh = ZoneState { owner: -1, capturing: -1, ..ZoneState::default() };
    // A zone takes its navpoint's state when it changes.
    let mut emit =
        move |out: &mut BTreeMap<u32, ZoneObject>, z: u32, mut s: ZoneState, letter: Option<char>, frame: i64| {
            let Some(o) = out.get_mut(&z) else { return };
            if let Some(l) = letter {
                o.letter.get_or_insert(l);
            }
            s.frame = frame;
            let key = |x: &ZoneState| (x.owner, x.capturing, x.progress, x.contested, x.inside);
            if last.get(&z).is_none_or(|p| key(p) != key(&s)) {
                o.states.push(s);
                last.insert(z, s);
            }
        };
    move |frame, c, p| {
        for op in &p.ops {
            let blocks: &[(u32, i64)] = match &op.kind {
                OpKind::Update { fields, .. } => &fields.blocks,
                OpKind::Create { .. } => op.init.as_ref().map_or(&[][..], |f| &f.blocks),
                OpKind::Delete => {
                    manager.remove(&op.index);
                    nav.remove(&op.index);
                    nav_state.remove(&op.index);
                    countdown.remove(&op.index);
                    continue;
                }
            };
            match op.entity_type {
                Some(MANAGER) => {
                    if let OpKind::Create { .. } = op.kind {
                        // [index:13][gen:2][type:6], then the creation data.
                        let mut r = Bits { c, at: op.at + 21 };
                        if r.flag() {
                            r.bits(8);
                        }
                        if r.flag() {
                            let z = 512 + r.bits(13);
                            if out.contains_key(&z) {
                                manager.insert(op.index, z);
                            }
                        }
                    }
                    let Some(&z) = manager.get(&op.index) else { continue };
                    for &(row, at) in blocks {
                        if (2..=17).contains(&row) {
                            let v = crate::bits::bits_at(c, at, 32);
                            if v != u32::MAX && v & ZONE_BIT != 0 {
                                let n = v & 0xffff;
                                nav.insert(n, z);
                                rows.insert((op.index, row), n);
                                if let Some(&(s, l)) = nav_state.get(&n) {
                                    emit(out, z, s, l, frame as i64);
                                }
                            } else if v == u32::MAX {
                                // The hill's navpoint dropped: nobody counts
                                // as inside any more (its filter keeps
                                // updating), and nothing is in progress.
                                if let Some(n) = rows.remove(&(op.index, row)) {
                                    nav.remove(&n);
                                    if let Some(&(s, l)) = nav_state.get(&n) {
                                        let idle =
                                            ZoneState { inside: 0, progress: 0, capturing: -1, contested: false, ..s };
                                        emit(out, z, idle, l, frame as i64);
                                    }
                                }
                            }
                        }
                    }
                }
                Some(NAVPOINT) => {
                    let (mut s, mut letter) = nav_state.get(&op.index).copied().unwrap_or((fresh, None));
                    for &(row, at) in blocks {
                        let mut r = Bits { c, at };
                        match row {
                            6 => {
                                if let Some(cl) = filter(&mut r, true) {
                                    s.inside = cl.iter().find(|c| c.0 == 2 && !c.1).map_or(0, |c| {
                                        if c.2 == u32::MAX {
                                            0
                                        } else {
                                            c.2
                                        }
                                    });
                                }
                            }
                            14 => {
                                let v = r.bits(8).saturating_sub(127).min(126);
                                s.progress = if countdown.contains(&op.index) { 0 } else { v };
                            }
                            20 => {
                                if let Some((_, cl, _)) = state_group(&mut r) {
                                    s.owner = team_of_clauses(&cl).unwrap_or(-1);
                                }
                            }
                            21..=23 => match state_group(&mut r) {
                                Some((HAS_PROGRESS, cl, st)) => {
                                    s.capturing = team_of_clauses(&cl).unwrap_or(-1);
                                    s.contested = st == CONTESTED;
                                }
                                // "incoming" marks a countdown; the second
                                // player mask is who contests (Ranked King of
                                // the Hill sets no contested state).
                                Some((HAS_TEXT, cl, st)) => {
                                    if st == INCOMING {
                                        countdown.insert(op.index);
                                        s.progress = 0;
                                    }
                                    let masks: Vec<u32> = cl.iter().filter(|c| c.0 == 2).map(|c| c.2).collect();
                                    if masks.get(1).is_some_and(|&m| m != 0 && m != u32::MAX) {
                                        s.contested = true;
                                    }
                                }
                                Some((CONTROL_POINT, _, st)) => {
                                    if let Some(l) = letter_of(st) {
                                        letter = Some(l);
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                    nav_state.insert(op.index, (s, letter));
                    if let Some(&z) = nav.get(&op.index) {
                        emit(out, z, s, letter, frame as i64);
                    }
                }
                Some(crate::payload::CRATE) if out.contains_key(&op.index) => {
                    for &(row, at) in blocks {
                        match row {
                            9 => {
                                if let (Some(sh), Some(o)) = (shape_at(c, at), out.get_mut(&op.index)) {
                                    o.shape.get_or_insert(sh);
                                }
                            }
                            2 => {
                                if let Some(a) = angle_at(c, at) {
                                    angles.entry(op.index).or_insert(a);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// The zones' objects, each shape turned by its crate's angle.
pub fn finish_objects(seen: ObjectsSeen) -> BTreeMap<u32, ZoneObject> {
    let ObjectsSeen { mut out, angles } = seen;
    for (z, o) in out.iter_mut() {
        if let (Some(sh), Some(&a)) = (o.shape.as_mut(), angles.get(z)) {
            sh.angle = a;
        }
    }
    out
}

// King of the Hill's hill points.
//
// The HUD's networked properties (entity type 13, a 32-bit name in the
// creation data) carry each team's points on the current hill:
// `centerscoreboard_leftpointbanknormalized` (7b7315d7) holds the viewing
// player's team's, `...rightpointbanknormalized` (0a1ce07d) the other
// team's, row 2 + player index, kind 7 (24 bits): (q - 0x7fffff) /
// 0x7fffff * 100 is the bank, 1.0 the hill won. `localteammaxscore`
// (29453215) is the points that win a hill, c728669c the hills that win the
// match. The bank times the points is the team's scoring ticks since its
// last hill; both banks reset when a hill is won.

const PROP_LEFT: u32 = 0x7b73_15d7;
const PROP_RIGHT: u32 = 0x0a1c_e07d;
const PROP_TO_WIN: u32 = 0x2945_3215;
const PROP_HILLS: u32 = 0xc728_669c;
const PROPERTY: u32 = 13;

/// Each team's points on the current hill over time, and what wins.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HillScore {
    /// Points that win a hill, hills that win the match.
    pub to_win: u32,
    pub hills: u32,
    /// (frame, team, points), on change.
    pub runs: Vec<(i64, i32, u32)>,
}

/// The hill points, from the properties above; None without them.
pub fn hill_score_reader<'a>(
    team_of: impl Fn(i32) -> Option<i32> + 'a,
    out: &'a mut HillScore,
) -> impl FnMut(usize, &[u8], &crate::payload::Payload) + 'a {
    use crate::payload::OpKind;
    use std::collections::HashMap;
    let mut names: HashMap<u32, u32> = HashMap::new();
    let mut last: HashMap<i32, u32> = HashMap::new();
    let mut banks: std::collections::BTreeMap<i32, f64> = std::collections::BTreeMap::new();
    move |frame, c, p| {
        for op in &p.ops {
            if op.entity_type != Some(PROPERTY) {
                continue;
            }
            let blocks: &[(u32, i64)] = match &op.kind {
                OpKind::Create { .. } => {
                    // [index:13][gen:2][type:6], a version, the name.
                    let mut r = Bits { c, at: op.at + 21 };
                    if r.flag() {
                        r.bits(8);
                    }
                    names.insert(op.index, r.bits(32));
                    op.init.as_ref().map_or(&[][..], |f| &f.blocks)
                }
                OpKind::Update { fields, .. } => &fields.blocks,
                OpKind::Delete => {
                    names.remove(&op.index);
                    continue;
                }
            };
            let Some(&name) = names.get(&op.index) else { continue };
            if ![PROP_LEFT, PROP_RIGHT, PROP_TO_WIN, PROP_HILLS].contains(&name) {
                continue;
            }
            for &(row, at) in blocks {
                if row < 2 {
                    continue;
                }
                let mut r = Bits { c, at };
                let kind = r.bits(4);
                let v = r.bits(crate::creation::property_bits(kind));
                match name {
                    PROP_TO_WIN if v > 0 && v < 1000 => out.to_win = v,
                    PROP_HILLS if v > 0 && v < 100 => out.hills = v,
                    PROP_LEFT | PROP_RIGHT if kind == 7 => {
                        let Some(own) = team_of(row as i32 - 2) else { continue };
                        // Two teams: the right bank is the other one's.
                        let team = if name == PROP_LEFT { own } else { 1 - own };
                        if !(0..2).contains(&team) {
                            continue;
                        }
                        banks.insert(team, (f64::from(v) - f64::from(0x7f_ffffu32)) / f64::from(0x7f_ffffu32) * 100.0);
                    }
                    _ => {}
                }
            }
        }
        if out.to_win > 0 {
            for (&team, &bank) in &banks {
                let pts = (bank * f64::from(out.to_win)).round().max(0.0) as u32;
                if last.get(&team) != Some(&pts) {
                    last.insert(team, pts);
                    out.runs.push((frame as i64, team, pts));
                }
            }
        }
    }
}

/// The hill score [`hill_score_reader`] read, when the film has one.
pub fn finish_hill_score(out: HillScore) -> Option<HillScore> {
    (out.to_win > 0 && !out.runs.is_empty()).then_some(out)
}
