//! Weapon fire per player, the weapon behind each death, and every hit, from
//! the event views of the tick packets (`events.rs`).
//!
//! A weapon-fire event (36) names the shooter twice: its first handle is
//! the shooter's unit, and its body carries a 5-bit player index. The handle
//! as a biped slot gives the player life assignment gave that life; the
//! body's index covers the rest (small lobbies, which have no life slots,
//! and shots fired from a vehicle seat). The body also names the weapon by
//! its tag id.
//!
//! A PlayerKilledEvent (85) names the victim, killer and assistant by player
//! index but not the weapon: its two 32-bit fields are small numbers, not
//! tag ids. Without a damage effect that names one, the weapon of a death is
//! the killer's last shot at most 2 s before it (a melee, grenade or vehicle
//! kill has none).
//!
//! A damage_aftermath event (0) is one hit: its first handle is the
//! victim's unit, its second the attacker's, and its body names the
//! attacker's player index too.

use crate::container::Film;
use crate::events::{Body, Layout, PLAYER_KILLED, WEAPON_FIRE};
use crate::lives::TICKS_PER_MS;
use crate::payload::for_each_event_view;
use crate::types::PlayerPath;
use crate::weapons::{self, Weapon};
use std::collections::{BTreeSet, HashMap};

/// Ticks before a death in which the killer's last shot names the weapon.
const KILL_SHOT_TICKS: f64 = 120.0;
/// A shot this many ticks after the death time still counts (the kill is
/// logged a tick or two before the shot's packet).
const KILL_SHOT_LATE_TICKS: f64 = 3.0;
/// How far a kill event can be from the estimated clock.
const ZERO_SEARCH_TICKS: f64 = 90.0;
/// Ticks before a death in which the last damage on the victim is its
/// cause.
const KILL_CAUSE_TICKS: u32 = 120;
/// How long a grenade can be the cause of a blast it didn't name, in ticks:
/// its fuse and flight, generously.
const THROW_TICKS: u32 = 360;

#[cfg_attr(test, derive(Default))]
#[derive(Debug)]
pub struct FilmFire {
    /// The weapons fired and the `extra` ones given to [`read`], by tag id;
    /// a shot's weapon is an index into it.
    pub weapons: Vec<Weapon>,
    /// Per path (parallel to the paths given to [`read`]): each shot's frame
    /// and weapon index, in frame order.
    pub shots: Vec<Vec<(i64, usize)>>,
    /// PlayerKilledEvents: frame, victim, killer (player indices).
    pub kills: Vec<(i64, Option<u32>, Option<u32>)>,
    /// What else each of `kills` says.
    pub kill_details: Vec<KillDetail>,
    /// Damage events: frame, victim's player index in `paths`, damage tag.
    pub damage: Vec<(i64, usize, u32)>,
    /// damage_aftermath events: frame, the attacker's and the
    /// victim's path (index into the paths given to [`read`]), in frame
    /// order. The victim is the first handle's biped life; the attacker the
    /// second handle's, else the body's player index; None for damage with
    /// no player behind it (falls, the map).
    pub hits: Vec<(i64, Option<usize>, usize)>,
    /// Parallel to `hits`: the victim's life slot, the damage tag and the
    /// body's first 5-bit scale (`hit_damage.rs`).
    pub hit_details: Vec<(i32, Option<u32>, u32)>,
    /// Per path: each grenade or object thrown (biped_throw_initiate), its
    /// frame and the 3-bit grenade type (None: a thrown object named by tag).
    /// The thrower is the first handle (an 8-bit biped index and generation,
    /// the life's slot id).
    pub throws: Vec<Vec<(i64, Option<u32>)>>,
    /// Per path: each exit from a vehicle: frame, the vehicle's id, the seat.
    pub exits: Vec<Vec<(i64, i32, u32)>>,
    /// Every voice line (Dialogue2D): frame, sound tag id, the players who
    /// hear it (bit 31 - player index).
    pub voice: Vec<(i64, u32, u32)>,
    /// Lines a unit speaks (AIDialog): frame, the speaker's index in `paths`
    /// (-1 when the unit isn't a player's life), the sound, the line's kind
    /// (DialoguePriority), and its 12-bit line (a vocalization's index).
    pub chatter: Vec<(i64, i32, u32, u32, Option<u32>)>,
}

/// The shots and kills of `film`, attributed to `paths`. `layout` is how
/// the level lays out a position, which some event bodies read; `extra` are
/// other weapons' tag ids the weapon list must hold (picked up, on the
/// ground: `inventory.rs`).
pub fn read(film: &Film, layout: Layout, paths: &[PlayerPath], extra: &[u32]) -> FilmFire {
    let mut slot_owner: HashMap<i32, usize> = HashMap::new();
    let mut by_index: HashMap<i32, usize> = HashMap::new();
    for (k, p) in paths.iter().enumerate() {
        by_index.insert(p.player_index, k);
        for &s in p.life_slots.iter().flatten() {
            slot_owner.insert(s, k);
        }
    }
    let mut raw: Vec<(i64, usize, u32)> = Vec::new();
    let mut kills = Vec::new();
    let mut kill_details = Vec::new();
    let mut damage = Vec::new();
    let mut hits = Vec::new();
    let mut hit_details = Vec::new();
    let mut throws = vec![Vec::new(); paths.len()];
    let mut exits = vec![Vec::new(); paths.len()];
    let mut voice = Vec::new();
    let mut chatter = Vec::new();
    for_each_event_view(film, layout, |frame, evs| {
        for e in evs {
            match e.body {
                Body::Fire(f) if e.kind == WEAPON_FIRE => {
                    let slot = e.handles[0].and_then(|h| h.biped_slot());
                    let owner = slot
                        .and_then(|s| slot_owner.get(&s))
                        .or_else(|| f.player.and_then(|p| by_index.get(&(p as i32))));
                    if let (Some(&k), Some(w)) = (owner, f.weapon) {
                        raw.push((frame as i64, k, w));
                    }
                }
                Body::Killed(k) if e.kind == PLAYER_KILLED => {
                    kills.push((frame as i64, k.a, k.b));
                    kill_details.push(KillDetail {
                        killer_percent: k.x,
                        suicide: k.flag,
                        assistant: k.c,
                        assistant_percent: k.y,
                        frame: frame as i64,
                        killer: k.b,
                    });
                }
                Body::Dialogue { tag: Some(t), b, .. } => voice.push((frame as i64, t, b)),
                Body::AiDialog { kind, value: Some(t), line, .. } => {
                    let speaker = e.handles[0]
                        .and_then(|h| h.biped_slot())
                        .and_then(|s| slot_owner.get(&s))
                        .map_or(-1, |&k| k as i32);
                    chatter.push((frame as i64, speaker, t, kind, line));
                }
                Body::Exit { seat } => {
                    let owner = e.handles[0].and_then(|h| h.biped_slot()).and_then(|s| slot_owner.get(&s));
                    if let (Some(&k), Some(v)) = (owner, e.handles[1].and_then(|h| h.vehicle_slot())) {
                        exits[k].push((frame as i64, v, seat));
                    }
                }
                Body::Throw { grenade, player, .. } => {
                    let life = e.handles[0]
                        .filter(|h| h.generation > 0)
                        .map(|h| (h.generation as i32 - 1) * 256 + h.index as i32);
                    let owner = life
                        .and_then(|s| slot_owner.get(&s))
                        .or_else(|| player.and_then(|p| by_index.get(&(p as i32))));
                    if let Some(&k) = owner {
                        throws[k].push((frame as i64, grenade));
                    }
                }
                Body::Damage(d) => {
                    let life =
                        |k: usize| e.handles[k].and_then(|h| h.biped_slot()).and_then(|s| slot_owner.get(&s)).copied();
                    let attacker = life(1).or_else(|| d.player.and_then(|p| by_index.get(&(p as i32)).copied()));
                    if let (Some(victim), Some(slot)) = (life(0), e.handles[0].and_then(|h| h.biped_slot())) {
                        hits.push((frame as i64, attacker, victim));
                        hit_details.push((slot, d.tag, d.a[0]));
                        if let Some(t) = d.tag {
                            damage.push((frame as i64, victim, t));
                        }
                    }
                }
                _ => {}
            }
        }
    });
    let ids: Vec<u32> =
        raw.iter().map(|s| s.2).chain(extra.iter().copied()).collect::<BTreeSet<_>>().into_iter().collect();
    let mut shots = vec![Vec::new(); paths.len()];
    for (frame, k, w) in raw {
        shots[k].push((frame, ids.binary_search(&w).unwrap()));
    }
    FilmFire {
        weapons: ids.into_iter().map(weapons::weapon).collect(),
        shots,
        kills,
        kill_details,
        damage,
        hits,
        hit_details,
        throws,
        exits,
        voice,
        chatter,
    }
}

impl FilmFire {
    /// The index in `weapons` of the weapon named as tag id `tag` is: a tag
    /// behind a damage effect is often a variant or base of the one in the film.
    pub fn named(&self, tag: u32) -> Option<usize> {
        let name = weapons::weapon(tag).name?;
        self.weapons.iter().position(|w| w.name == Some(name))
    }

    /// The index of the weapon with tag id `tag` in `weapons`.
    pub fn index_of(&self, tag: u32) -> Option<usize> {
        self.weapons.binary_search_by_key(&tag, |w| w.id).ok()
    }
}

/// Whether frame `earlier` is at most `ticks` game ticks before frame
/// `frame`, by the film's clock `frame_ticks`.
pub fn within_ticks(frame_ticks: &[u32], earlier: i64, frame: i64, ticks: u32) -> bool {
    let tick = |f: i64| usize::try_from(f).ok().and_then(|f| frame_ticks.get(f)).copied();
    match (tick(earlier), tick(frame)) {
        (Some(a), Some(b)) => earlier <= frame && b - a <= ticks,
        _ => false,
    }
}

/// What killed `victim` (an index into `paths`) at frame `frame`: the last
/// damage tag on them in the 2 s before it.
pub fn kill_cause(ff: &FilmFire, victim: usize, frame_ticks: &[u32], frame: i64) -> Option<u32> {
    ff.damage
        .iter()
        .rev()
        .find(|d| d.1 == victim && within_ticks(frame_ticks, d.0, frame, KILL_CAUSE_TICKS))
        .map(|d| d.2)
}

/// Whether the player at `paths` index `killer` threw a grenade in the
/// `THROW_TICKS` before `frame`.
pub fn threw_before(ff: &FilmFire, killer: usize, frame_ticks: &[u32], frame: i64) -> bool {
    ff.throws.get(killer).is_some_and(|t| t.iter().any(|&(f, _)| within_ticks(frame_ticks, f, frame, THROW_TICKS)))
}

/// The tick of HighlightEvents time 0, from the kill events themselves: the
/// median over `deaths` (victim's player index, ms) of each one's nearest kill
/// event on that victim, within `ZERO_SEARCH_TICKS` of `estimate`. A kill and
/// its highlight are one moment, so this needs no model of the respawn.
pub fn highlight_zero(ff: &FilmFire, frame_ticks: &[u32], deaths: &[(i32, f64)], estimate: f64) -> Option<f64> {
    let mut zeros: Vec<f64> = deaths
        .iter()
        .filter_map(|&(victim, ms)| {
            let at = estimate + ms * TICKS_PER_MS;
            let tick = ff
                .kills
                .iter()
                .filter(|k| k.1 == Some(victim as u32))
                .filter_map(|k| Some(frame_ticks.get(k.0 as usize).copied()? as f64))
                .filter(|t| (t - at).abs() <= ZERO_SEARCH_TICKS)
                .min_by(|a, b| f64::total_cmp(&(a - at).abs(), &(b - at).abs()))?;
            Some(tick - ms * TICKS_PER_MS)
        })
        .collect();
    if zeros.len() < 3 {
        return None;
    }
    zeros.sort_by(f64::total_cmp);
    Some(zeros[zeros.len() / 2])
}

/// A PlayerKilledEvent's other fields: each player's share of
/// the victim's damage, in whole percent, and whether the death was their own.
/// `assistant_percent` means something only with an assistant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KillDetail {
    pub killer_percent: u32,
    pub suicide: bool,
    pub assistant: Option<u32>,
    pub assistant_percent: u32,
    /// The kill event's frame.
    pub frame: i64,
    pub killer: Option<u32>,
}

/// The weapon index of the last shot by the player at `paths` index `killer`
/// in the `KILL_SHOT_TICKS` before frame `frame`, `frame_ticks` the film's
/// clock.
pub fn shot_before(ff: &FilmFire, killer: usize, frame_ticks: &[u32], frame: i64) -> Option<usize> {
    let tick = |f: i64| frame_ticks.get(f as usize).map(|&t| t as f64);
    let death = tick(frame)?;
    ff.shots
        .get(killer)?
        .iter()
        .rev()
        .filter_map(|&(f, w)| Some((tick(f)?, w)))
        .find(|&(t, _)| t <= death + KILL_SHOT_LATE_TICKS)
        .filter(|&(t, _)| t >= death - KILL_SHOT_TICKS)
        .map(|s| s.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_windows_are_game_time() {
        // A 30 Hz film: two ticks a frame, so 2 s is 60 frames.
        let ticks: Vec<u32> = (0..400).map(|f| 2 * f).collect();
        assert!(within_ticks(&ticks, 140, 200, 120));
        assert!(!within_ticks(&ticks, 139, 200, 120));
        assert!(!within_ticks(&ticks, 201, 200, 120));
        let ff = FilmFire {
            damage: vec![(100, 0, 7), (150, 0, 8)],
            throws: vec![vec![(10, Some(1))]],
            ..FilmFire::default()
        };
        assert_eq!(kill_cause(&ff, 0, &ticks, 200), Some(8));
        assert_eq!(kill_cause(&ff, 0, &ticks, 211), None);
        assert!(threw_before(&ff, 0, &ticks, 190));
        assert!(!threw_before(&ff, 0, &ticks, 191));
    }
}
