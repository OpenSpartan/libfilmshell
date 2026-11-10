//! One film, start to finish: the container's frames and clock, who played,
//! their lives, the coordinates, and every reader's output, gathered into
//! the model.

use crate::actions;
use crate::analysis::{AccuracyData, AnalysisData, SpawnFactor, WeaponProfile};
use crate::calibrate::{apply_calibration, level_matches, scale_paths_to_world, Pad, WORLD_PER_ENGINE};
use crate::components::{self, Params, SlotStates};
use crate::container::{Film as Container, Framing};
use crate::damage::{self, Cause};
use crate::events::{Body, Param, Value};
use crate::fire::{self, FilmFire};
use crate::game::{self, Grenade, Level};
use crate::grenades;
use crate::highlights::{extract_highlight_events, EventType, HighlightEvent};
use crate::hints::Hints;
use crate::holders::Holders;
use crate::inventory::{self};
use crate::levels::{self, LevelBox};
use crate::model::*;
use crate::motion::{detect_layout, extract_all_player_positions, small_lobby_deaths, ExtractionResult};
use crate::objectives;
use crate::payload::{for_each_event_view, walk_shared, Reader};
use crate::roster::{self, Roster};
use crate::types::*;
use crate::vitals::Vitals;
use crate::zones;
use crate::TagId;
use std::collections::{BTreeMap, HashMap};

/// A player's latest sample may be this old and still place them at a deploy.
const DEPLOY_SAMPLE_FRAMES: i64 = 30;
/// The object comes down this close to its user, world units.
const DEPLOY_NEAR_WU: f64 = 30.0;
/// And the next nearest player this many times farther.
const DEPLOY_APART: f64 = 2.0;
/// Most world units between a thrower, at the frame their grenade appears,
/// and its first position (the grenade appears about 18 frames after the
/// throw event, already in flight).
const GRENADE_MATCH_WU: f64 = 15.0;
/// Flights are thinned to within this many world units of the full path.
const GRENADE_TOLERANCE: f64 = 0.25;
/// A flight ends at a gap of more frames than this between its updates, or
/// a step of more world units a frame than `GRENADE_STEP_WU`.
const GRENADE_GAP_FRAMES: i64 = 60;
const GRENADE_STEP_WU: f64 = 15.0;
/// Frames after a grenade flight's end in which a detonation naming its
/// object is still that grenade's.
const BLAST_AFTER_END_FRAMES: i64 = 60;
/// Ticks before an event in which the player's latest sample places an
/// event that carries no position.
const SAMPLE_TICKS: u32 = 120;
/// Without the parent state, a ride's exit comes this many frames after its
/// last sample at most.
const EXIT_SLACK: i64 = 90;
/// Statborg slots of deaths and spawns.
const DEATHS_SLOT: u32 = 5;
const SPAWNS_SLOT: u32 = 21;
/// Frames between a kill and the scoreboard counting the death.
const DEATH_FRAMES: i64 = 6;

/// A path's samples in world units, with each sample's state.
struct WorldPath {
    player: i32,
    positions: Vec<WorldPosition>,
    states: Vec<SampleState>,
}

/// A player's state at one sample, each value as of its last update.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SampleState {
    yaw: Option<f64>,
    z: Option<f64>,
    speed: Option<f64>,
    health: Option<f64>,
    shield: Option<f64>,
}

/// A position code in box `bx` in world units: the play space's through the
/// calibration, another box's through the level's boxes. None for a box the
/// level doesn't list.
fn place(bx: u32, q: [u32; 3], w: [u32; 3], c: &Calibration, level: Option<&LevelBox>) -> Option<(f64, f64)> {
    if bx == level.map_or(0, |l| l.box_index) {
        return Some(window_to_world(q, w, c));
    }
    let v = level?.in_play_window(bx, q)?;
    let raw1 = v[0] * 2f64.powi(15 - w[0] as i32);
    let raw2 = v[1] * 2f64.powi(15 - w[1] as i32);
    Some((-c.scale_x * raw2 + c.offset_x, c.scale_y * raw1 + c.offset_y))
}

/// A mask of engine player ids (bit 31 - id) as a mask of the player indices
/// that held them at `frame`.
fn players_mask(holders: &Holders, mask: u32, frame: i64) -> u32 {
    (0..32u32)
        .filter(|p| mask & (0x8000_0000 >> p) != 0)
        .map(|p| holders.api(p, frame))
        .filter(|&i| i < 32)
        .fold(0, |m, i| m | 0x8000_0000 >> i)
}

fn window_to_world(q: [u32; 3], w: [u32; 3], c: &Calibration) -> (f64, f64) {
    let raw1 = q[0] as f64 * 2f64.powi(15 - w[0] as i32);
    let raw2 = q[1] as f64 * 2f64.powi(15 - w[1] as i32);
    (-c.scale_x * raw2 + c.offset_x, c.scale_y * raw1 + c.offset_y)
}

/// The widths of the position fields (c1, c2, Z): the level's when the
/// catalogue has it (and, in a composite film, its widths match the detected
/// ones), else in a composite film c1 and c2 as detected and Z found by
/// `components::detect_z_width`.
pub(crate) fn position_widths(
    film: &Container,
    composite: bool,
    c2_start: i64,
    c2_width: Option<u32>,
    level: Option<&LevelBox>,
) -> Option<[u32; 3]> {
    if !composite {
        return level.map(|b| b.widths);
    }
    let c2w = c2_width?;
    if let Some(b) = level.filter(|b| level_matches(b, c2_start, c2_width)) {
        return Some(b.widths);
    }
    let c1 = (c2_start - levels::c1_at(level) as i64) as u32;
    components::detect_z_width(film, c1, c2w).map(|z| [c1, c2w, z])
}

/// The box the film's positions are quantized over: the hints' bounds, else
/// the catalogue's level the hints or the header name.
fn level_box(header: &crate::header::Header, hints: &Hints) -> (Option<LevelBox>, Option<Level>) {
    let id = hints.level.map(|t| t.0).or(header.level);
    if let Some(b) = hints.level_bounds {
        let widths: [u32; 3] =
            std::array::from_fn(|k| (60.0 * (b.max[k] as f64 - b.min[k] as f64)).max(1.0).ceil().log2().ceil() as u32);
        let lb = LevelBox {
            min: b.min.map(|v| v as f64),
            max: b.max.map(|v| v as f64),
            widths,
            box_index: 0,
            box_bits: 1,
            others: Vec::new(),
        };
        let level = Level {
            id: TagId(id.unwrap_or(0)),
            name: "",
            structure: TagId(0),
            play: game::LevelBox { index: 0, min: b.min, max: b.max, widths },
            index_bits: 1,
            others: &[],
        };
        return (Some(lb), Some(level));
    }
    let Some(id) = id else { return (None, None) };
    (levels::level(id).cloned(), game::level(TagId(id)).copied())
}

/// Each path's sample states and jumps, from the component data of the
/// updates of the life each sample belongs to.
fn sample_states(
    film: &Container,
    paths: &[PlayerPath],
    params: &Params,
    level: Option<&LevelBox>,
    calib: &Calibration,
    vitals: &Vitals,
) -> (Vec<Vec<SampleState>>, Vec<Vec<i64>>) {
    let slot_states = SlotStates::read(film, params);
    let jumps = paths
        .iter()
        .map(|p| {
            let (Some(slots), Some(starts)) = (&p.life_slots, &p.life_starts) else { return Vec::new() };
            let mut out = Vec::new();
            for (k, &slot) in slots.iter().enumerate() {
                let (Some(&a), b) = (starts.get(k), starts.get(k + 1)) else { continue };
                let from = p.positions.get(a).map_or(i64::MIN, |m| m.frame);
                let to = b.and_then(|&b| p.positions.get(b)).map_or(i64::MAX, |m| m.frame);
                out.extend(slot_states.jumps(slot).iter().copied().filter(|&f| f >= from && f < to));
            }
            out
        })
        .collect();
    let states = paths
        .iter()
        .map(|p| {
            let (Some(slots), Some(starts)) = (&p.life_slots, &p.life_starts) else { return Vec::new() };
            let mut life = 0;
            p.positions
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    while life + 1 < starts.len() && starts[life + 1] <= i {
                        life += 1;
                    }
                    let s = slots.get(life).and_then(|&slot| slot_states.at(slot, m.frame)).unwrap_or_default();
                    let (health, shield) = slots.get(life).map_or((None, None), |&slot| {
                        let (h, s) = vitals.at(slot, m.frame);
                        (Some(h), s)
                    });
                    SampleState {
                        // Engine (cos, sin) to world (-sx sin, sy cos).
                        yaw: s.aim.map(|(y, _)| (calib.scale_y * y.cos()).atan2(-calib.scale_x * y.sin())),
                        speed: s.speed.filter(|_| !m.in_vehicle).map(|v| v * WORLD_PER_ENGINE),
                        z: level
                            .filter(|_| calib.source == CalibSource::LevelTable)
                            .and_then(|b| s.position.and_then(|(bx, q)| b.world_z(bx, q[2]))),
                        health,
                        shield,
                    }
                })
                .collect()
        })
        .collect();
    (states, jumps)
}

/// Ramer-Douglas-Peucker on (x, y): the samples that keep the polyline
/// within `tol` of the full one, first and last always kept.
fn thin(pts: &[(i64, f64, f64)], tol: f64) -> Vec<(i64, f64, f64)> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (ax, ay, bx, by) = (pts[a].1, pts[a].2, pts[b].1, pts[b].2);
        let (dx, dy) = (bx - ax, by - ay);
        let len = dx.hypot(dy);
        let mut far = (0.0, 0);
        for (i, p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d =
                if len == 0.0 { (p.1 - ax).hypot(p.2 - ay) } else { ((p.1 - ax) * dy - (p.2 - ay) * dx).abs() / len };
            if d > far.0 {
                far = (d, i);
            }
        }
        if far.0 > tol {
            keep[far.1] = true;
            stack.push((a, far.1));
            stack.push((far.1, b));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

/// A paired grenade flight, before conversion.
struct Flight {
    path_index: usize,
    grenade: Option<u32>,
    path: Vec<(i64, f64, f64)>,
    end: i64,
    deleted: bool,
    entity: (u32, u32),
    from: i64,
}

/// Each throw paired with the projectile created in the window after it
/// whose first position is nearest the thrower, each projectile used once.
fn pair_grenades(
    f: &grenades::Flights,
    paths: &[PlayerPath],
    world: &[WorldPath],
    w: [u32; 3],
    c: &Calibration,
    level: Option<&LevelBox>,
    frame_ticks: &[u32],
) -> Vec<Flight> {
    let owner: HashMap<i32, usize> =
        paths.iter().enumerate().flat_map(|(k, p)| p.life_slots.iter().flatten().map(move |&s| (s, k))).collect();
    let mut used = vec![false; f.projectiles.len()];
    let mut out = Vec::new();
    for t in &f.throws {
        let Some(&k) = t.slot_id.and_then(|s| owner.get(&s)) else { continue };
        let wp = &world[k];
        let at = |frame: i64| {
            let i = wp.positions.partition_point(|s| s.frame <= frame);
            i.checked_sub(1)
                .map(|i| &wp.positions[i])
                .filter(|s| fire::within_ticks(frame_ticks, s.frame, frame, SAMPLE_TICKS))
        };
        let best = f
            .projectiles
            .iter()
            .enumerate()
            .filter(|(j, p)| !used[*j] && p.from >= t.frame && p.from - t.frame <= grenades::WINDOW)
            .filter_map(|(j, p)| {
                let me = at(p.from)?;
                let &(_, bx, q) = p.path.first()?;
                let (x, y) = place(bx, q, w, c, level)?;
                Some((j, (x - me.x).hypot(y - me.y)))
            })
            .filter(|&(_, d)| d <= GRENADE_MATCH_WU)
            .min_by(|a, b| f64::total_cmp(&a.1, &b.1));
        let Some((j, _)) = best else { continue };
        used[j] = true;
        let p = &f.projectiles[j];
        let mut full: Vec<(i64, f64, f64)> = p
            .path
            .iter()
            .filter_map(|&(fr, bx, q)| {
                let (x, y) = place(bx, q, w, c, level)?;
                Some((fr, x, y))
            })
            .collect();
        // A missed deletion lets a later object in the same entity slot
        // continue the path: cut it at the first long gap or jump.
        let cut = full.windows(2).position(|s| {
            let gap = s[1].0 - s[0].0;
            gap > GRENADE_GAP_FRAMES || (s[1].1 - s[0].1).hypot(s[1].2 - s[0].2) > GRENADE_STEP_WU * gap as f64
        });
        let whole = cut.is_none();
        if let Some(i) = cut {
            full.truncate(i + 1);
        }
        let last = full.last().map(|s| s.0).unwrap_or(p.from);
        let deleted = whole && p.to.is_some_and(|to| to - last <= GRENADE_GAP_FRAMES);
        let end = if deleted { p.to.unwrap_or(last) } else { last };
        out.push(Flight {
            path_index: k,
            grenade: t.grenade,
            path: thin(&full, GRENADE_TOLERANCE),
            end,
            deleted,
            entity: (p.entity, p.generation),
            from: p.from,
        });
    }
    out
}

/// A bot's spawns when the scoreboard has no row of its: high enough that
/// nothing caps its lives (one fewer deaths, so its last life can survive).
const UNCOUNTED: f64 = 1e9;

/// The frames a new round clears the scoreboard at, in order. A round
/// clears every player's row in one frame, where a player who leaves has
/// only their own cleared: a frame where the spawns of two rows or more
/// drop to 0, half or more of the rows that had spawned. (Measured on the
/// multi-round films, Oddball and the minigames: the clear comes with the
/// round's start, up to 3 s before its clock restarts.)
fn round_resets(log: &crate::statlog::Log) -> Vec<i64> {
    let mut spawns: BTreeMap<u32, i32> = BTreeMap::new();
    let mut out = Vec::new();
    let changes: Vec<_> = log.changes.iter().filter(|c| c.kind == 0 && c.slot == SPAWNS_SLOT).collect();
    for same in changes.chunk_by(|a, b| a.frame == b.frame) {
        let had = spawns.values().filter(|&&v| v > 0).count();
        let cleared = same.iter().filter(|c| c.value == 0 && spawns.get(&c.id).is_some_and(|&v| v > 0)).count();
        if cleared >= 2 && 2 * cleared >= had {
            out.push(same[0].frame);
        }
        for c in same {
            spawns.insert(c.id, c.value);
        }
    }
    out
}

/// What life assignment needs of each player, from the film: the roster, and
/// each player's deaths and spawns on the scoreboard. A player who leaves
/// has their row cleared, so a count is its highest value in each round,
/// summed over the rounds. A bot that took a leaver's place can have no row
/// at all; its lives are chained by time without a count.
fn roster_entries(roster: &Roster, log: &crate::statlog::Log, hl_ticks: &[u32]) -> Vec<RosterEntry> {
    let at = |f: i64| usize::try_from(f).ok().and_then(|f| hl_ticks.get(f)).map_or(0.0, |&t| t as f64);
    let resets = round_resets(log);
    let most = |id: usize, slot: u32| {
        let mut rounds: BTreeMap<usize, i32> = BTreeMap::new();
        for c in log.changes.iter().filter(|c| c.kind == 0 && c.id as usize == id && c.slot == slot) {
            let round = resets.partition_point(|&r| r <= c.frame);
            let m = rounds.entry(round).or_insert(c.value);
            *m = (*m).max(c.value);
        }
        (!rounds.is_empty()).then(|| rounds.values().map(|&v| v as f64).sum())
    };
    let known = !log.changes.is_empty();
    roster
        .players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let has_row = log.changes.iter().any(|c| c.kind == 0 && c.id as usize == i);
            let uncounted = known && p.bot.is_some() && !has_row;
            RosterEntry {
                bot: p.bot.is_some(),
                last_team_id: p.team.map(|t| t as f64),
                xuid: p.xuid.map(|x| x.to_string()),
                present_at_beginning: Some(p.present_at_start),
                left_in_progress: Some(p.left.is_some()),
                deaths: if uncounted { Some(UNCOUNTED - 1.0) } else { most(i, DEATHS_SLOT).or(known.then_some(0.0)) },
                spawns: if uncounted { Some(UNCOUNTED) } else { most(i, SPAWNS_SLOT).or(known.then_some(0.0)) },
                listed: uncounted.then(|| (p.first.map_or(0.0, at), p.left.map_or(f64::INFINITY, at))),
            }
        })
        .collect()
}

/// The film's game events (PlayerGameEventSmall), positions in world units
/// when the calibration is known.
fn game_events(film: &Container, p: &Params, c: Option<&Calibration>, holders: &Holders) -> Vec<(i64, GameEvent)> {
    let w = p.widths;
    let mut out = Vec::new();
    for_each_event_view(film, p, |frame, evs| {
        for e in evs {
            let Body::Game(g) = &e.body else { continue };
            let values = g
                .values
                .iter()
                .flatten()
                .map(|&(k, v)| {
                    let v = match v {
                        Value::Word(kind, value) => GameEventValue::Word { kind, value },
                        Value::Bit(b) => GameEventValue::Bit(b),
                        Value::Str(s) => GameEventValue::Text(s),
                        Value::Pos(p) => GameEventValue::Position(p.zip(c).map(|(q, c)| {
                            let (x, y) = window_to_world(q, w, c);
                            Position::new(x, y)
                        })),
                        Value::None => GameEventValue::Empty,
                    };
                    (k, v)
                })
                .collect();
            let players = players_mask(holders, g.last, frame as i64);
            let arguments = g
                .extra
                .iter()
                .flat_map(|x| x.1.iter().flatten())
                .map(|&a| match a {
                    Param::None => GameEventArgument::Empty,
                    Param::Player(p) => GameEventArgument::Player(
                        p.and_then(|p| u8::try_from(holders.api(p, frame as i64)).ok()).map(PlayerId),
                    ),
                    Param::Word(kind, value) => GameEventArgument::Word { kind, value },
                })
                .collect();
            let event = GameEvent {
                id: g.id,
                small: g.small,
                values,
                text: g.extra.map(|x| x.0),
                arguments,
                players: PlayerMask(players),
            };
            out.push((frame as i64, event));
        }
    });
    out
}

/// The highlight tick of HighlightEvents time 0 from the medals, for films
/// whose kills don't fix it: the game sends a mode script's event to a
/// medal's recipient on the tick it awards the medal, so the offset between
/// a medal's time and the events naming its player peaks at the clock's
/// zero. `medals` are (player index, ms), `events` (frame, player mask);
/// `hl_ticks` is each frame's highlight tick. None unless one offset holds
/// at least 3 medals and half of them.
fn medal_clock(medals: &[(i32, f64)], events: &[(i64, u32)], hl_ticks: &[u32]) -> Option<f64> {
    const NEAR: f64 = 1.5;
    let mut votes: HashMap<i64, usize> = HashMap::new();
    let mut offsets: Vec<Vec<f64>> = Vec::with_capacity(medals.len());
    for &(player, ms) in medals {
        let bit = 0x8000_0000u32.checked_shr(player as u32).unwrap_or(0);
        let mut d: Vec<f64> = events
            .iter()
            .filter(|e| e.1 & bit != 0)
            .filter_map(|e| Some(*hl_ticks.get(usize::try_from(e.0).ok()?)? as f64 - ms * crate::lives::TICKS_PER_MS))
            .collect();
        let mut bins: Vec<i64> = d.iter().map(|x| x.round() as i64).collect();
        bins.sort_unstable();
        bins.dedup();
        for b in bins {
            *votes.entry(b).or_default() += 1;
        }
        d.sort_by(f64::total_cmp);
        offsets.push(d);
    }
    let (&peak, &n) = votes.iter().max_by_key(|&(&b, &n)| (n, std::cmp::Reverse(b)))?;
    if n < 3 || 2 * n < medals.len() {
        return None;
    }
    // Each medal's offset nearest the peak, and their median.
    let mut near: Vec<f64> = offsets
        .iter()
        .filter_map(|d| {
            d.iter()
                .copied()
                .filter(|x| (x - peak as f64).abs() <= NEAR)
                .min_by(|a, b| (a - peak as f64).abs().total_cmp(&(b - peak as f64).abs()))
        })
        .collect();
    near.sort_by(f64::total_cmp);
    near.get(near.len() / 2).copied()
}

/// Ticks between a kill and the headshot event sent with it.
const HEADSHOT_TICKS: u32 = 2;

/// Marks each kill the game counted as a headshot: the headshot event for
/// its killer nearest it within [`HEADSHOT_TICKS`], each event used once.
/// `events` are in tick order.
fn mark_headshots(events: &mut [TimedEvent]) {
    let shots: Vec<(Tick, PlayerId)> = events
        .iter()
        .filter_map(|e| match e.event {
            Event::Headshot(p) => Some((e.tick, p)),
            _ => None,
        })
        .collect();
    if shots.is_empty() {
        return;
    }
    let mut used = vec![false; shots.len()];
    for e in events.iter_mut() {
        let Event::Kill(k) = &mut e.event else { continue };
        let from = shots.partition_point(|s| s.0 .0 + HEADSHOT_TICKS < e.tick.0);
        let best = (from..shots.len())
            .take_while(|&i| shots[i].0 .0 <= e.tick.0 + HEADSHOT_TICKS)
            .filter(|&i| !used[i] && Some(shots[i].1) == k.killer)
            .min_by_key(|&i| shots[i].0 .0.abs_diff(e.tick.0));
        if let Some(i) = best {
            used[i] = true;
        }
        k.headshot = Some(best.is_some());
    }
}

/// The vehicle tracks: each tagged vehicle's position records, split where
/// its id stands for a new vehicle, by first frame then id. Records with no
/// known tag are chance matches of the scanner and are left out.
fn vehicle_tracks(ex: &ExtractionResult, c: &Calibration) -> Vec<(i32, u32, Vec<WorldPosition>)> {
    let mut out = Vec::new();
    for (&id, v) in &ex.vehicles {
        let mut v = v.clone();
        v.sort_by_key(|s| s.gp);
        let starts = ex.vehicle_ident.starts(id);
        let mut cur: Option<(usize, u32, Vec<MotionPoint>)> = None;
        for s in &v {
            let k = starts.iter().rposition(|x| x.0 <= s.tick).unwrap_or(0);
            let Some(tag) = ex.vehicle_ident.tag(id, s.tick) else { continue };
            if let Some((_, t, pts)) = cur.take_if(|c| c.0 != k || c.1 != tag) {
                out.push((id, t, apply_calibration(&pts, c)));
            }
            let p = MotionPoint { frame: s.tick, raw1: s.c1, raw2: s.c2, gp: s.gp, in_vehicle: false, vehicle: id };
            cur.get_or_insert_with(|| (k, tag, Vec::new())).2.push(p);
        }
        if let Some((_, t, pts)) = cur {
            out.push((id, t, apply_calibration(&pts, c)));
        }
    }
    out.sort_by_key(|t| (t.2[0].frame, t.0));
    out
}

pub(crate) fn assemble(chunks: Vec<Vec<u8>>, has_highlights: bool, hints: &Hints) -> Result<crate::Film, crate::Error> {
    let film = Container::new(chunks, Framing::Packets);
    // Every tick of a film sends the tick message; replay chunks without one
    // aren't a film's.
    let ticks = film
        .frames()
        .any(|f| crate::payload::find_tick_message(f.chunk, 8 * f.start as i64, 8 * f.end as i64).is_some());
    if !ticks {
        return Err(crate::Error::NoTicks);
    }
    let packets = film.framing == Framing::Packets;
    let raw_header = crate::header::read(&film.chunks[0]);
    let highlights: Vec<HighlightEvent> =
        if has_highlights { extract_highlight_events(&film.chunks[film.chunks.len() - 1]) } else { Vec::new() };
    let mut roster = roster::read(&film, &highlights);
    let holders = Holders::new(&roster);
    let (level_owned, level_model) = level_box(&raw_header, hints);
    let level = level_owned.as_ref();
    let layout = detect_layout(&film, Some(roster.players.len()));
    let composite = layout.mode == EncodingMode::MultiplayerComposite;
    let (c2_start, c2w) = layout.composite.map_or((0, None), |(s, w)| (s as i64, Some(w)));
    let widths = position_widths(&film, composite, c2_start, c2w, level);
    let params = widths.map(|w| {
        let mut p = Params::new(w);
        p.set_level(level);
        p
    });
    // The readers that walk the tick packets by their grammar.
    let walk = params.filter(|_| composite && packets);
    if let Some(p) = params.filter(|_| packets) {
        roster.mark_present(&crate::statlog::players_at_start(&film, p));
    }

    // One walk for the readers that need no paths. The scoreboard gives
    // every player's deaths and spawns, which the paths need.
    let mut stat_log = crate::statlog::Log::default();
    let mut vehicle_ident = crate::vehicles::Vehicles::default();
    let mut vitals_marks = crate::vitals::Marks::default();
    let mut inv = inventory::Inventory::default();
    let mut inputs = crate::control::Inputs::new();
    let mut splash_seen = crate::splash::Seen::new();
    let mut thrown = grenades::Flights::default();
    let mut deploys_seen = crate::deploys::Seen::default();
    let mut attach_seen = crate::attach::Seen::default();
    let mut controls = crate::owners::Controls::new();
    let mut designators = crate::teams::Designators::default();
    if let Some(p) = walk {
        let actor_control = crate::registry::Schema::read(&film.chunks[0]).biped.actor_control;
        vitals_marks = Vitals::start(&film, &p, level);
        walk_shared(
            &film,
            p,
            &mut [
                &mut crate::statlog::reader(&film, p, &holders, &mut stat_log),
                &mut vehicle_ident.reader(),
                &mut Vitals::reader(&mut vitals_marks),
                &mut inventory::reader(&mut inv),
                &mut crate::control::reader(&film, p, &mut inputs),
                &mut crate::splash::reader(&film, &mut splash_seen),
                &mut grenades::reader(&mut thrown),
                &mut crate::deploys::reader(&mut deploys_seen),
                &mut crate::attach::reader(actor_control, &mut attach_seen),
                &mut crate::owners::reader(&mut controls),
                &mut crate::teams::reader(&mut designators),
            ],
        );
        crate::statlog::finish(&mut stat_log);
        vehicle_ident.finish(&film, p);
        // Teams for the players HighlightEvents doesn't give one: bots, and
        // people without records.
        crate::teams::assign(&mut roster, &crate::teams::finish(&film, designators));
    }
    let vitals = walk.map(|_| Vitals::finish(vitals_marks));
    let splashes = walk.map(|_| crate::splash::finish(splash_seen)).unwrap_or_default();
    let deploys = walk.map(|_| crate::deploys::finish(deploys_seen)).unwrap_or_default();
    let attachments = walk.map(|_| crate::attach::finish(attach_seen));
    let entries = roster_entries(&roster, &stat_log, &film.highlight_ticks());
    let xuids: Vec<String> = roster.players.iter().map(|p| p.xuid.map(|x| x.to_string()).unwrap_or_default()).collect();
    let owners = crate::owners::owners(&controls, &holders);
    let ex = extract_all_player_positions(
        &film,
        layout,
        Some(&entries),
        &highlights,
        &xuids,
        level,
        walk.map(|_| vehicle_ident),
        attachments,
        &owners,
    );
    let paths = &ex.paths;

    // Spawn pads from the hints anchor fitted coordinates.
    let pads: Vec<Pad> = hints.spawn_pads.iter().map(|p| Pad { x: p.position.x, y: p.position.y }).collect();
    let bounds = (!pads.is_empty()).then(|| {
        let (mut min_x, mut max_x, mut min_y, mut max_y) =
            (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for p in &pads {
            (min_x, max_x, min_y, max_y) = (min_x.min(p.x), max_x.max(p.x), min_y.min(p.y), max_y.max(p.y));
        }
        MapBounds { min_x, max_x, min_y, max_y, width: max_x - min_x }
    });
    let asset = raw_header.map_asset_id.map(|g| Guid(g).to_string());
    let (world_positions, calib) = scale_paths_to_world(
        paths,
        &pads,
        bounds.as_ref(),
        composite,
        asset.as_deref(),
        Some(ex.coord_layout),
        level,
        ex.c2_width,
    );
    let exact = calib.as_ref().is_some_and(|c| c.source == CalibSource::LevelTable);

    let (states, jumps) = match (&walk, &calib, &vitals) {
        (Some(p), Some(c), Some(v)) => sample_states(&film, paths, p, level, c, v),
        _ => (vec![Vec::new(); paths.len()], vec![Vec::new(); paths.len()]),
    };
    let world: Vec<WorldPath> = paths
        .iter()
        .zip(world_positions)
        .zip(states)
        .map(|((p, positions), states)| WorldPath { player: p.player_index, positions, states })
        .collect();
    let extra: Vec<u32> = inv.pickups.iter().map(|p| p.tag).chain(inv.ground.iter().map(|g| g.tag)).collect();
    let fired: Option<FilmFire> = params.map(|p| fire::read(&film, (&p).into(), paths, &extra));
    let scripts = crate::scripts::script_names(&film.chunks[0]);
    let mode = GameMode::from_scripts(&scripts);

    // What zone modes read of their zones' objects, by the zones' entities.
    let team_of = |pi: i32| roster.players.get(pi as usize).and_then(|p| p.team).map(|t| t as i32);
    let zone_input = match (zones::mode(&scripts), walk, &calib) {
        (Some(m), Some(p), Some(_)) => {
            let evs = zones::read(&film, (&p).into(), m, team_of);
            let places = zones::event_places(&evs);
            let crates = zones::crates(&film);
            let mut ids: Vec<u32> = places.keys().copied().collect();
            if m == zones::Mode::Koth {
                ids.extend(crates.iter().filter(|c| c.1 == zones::HILL_TAG).map(|c| c.0));
                ids.sort_unstable();
                ids.dedup();
            }
            Some((m, evs, places, ids))
        }
        _ => None,
    };

    // A second walk for the readers that need the paths.
    let mut acts_out = actions::FilmActions::default();
    let mut aim = crate::accuracy::Aim::default();
    let mut ammo = Vec::new();
    let mut hill_seen = zones::HillScore::default();
    let mut objects_seen = zones::ObjectsSeen::default();
    let koth = zone_input.as_ref().is_some_and(|z| z.0 == zones::Mode::Koth) && exact;
    if let Some(p) = walk {
        let mut a = actions::reader(paths, mode.is_pvp(), &mut acts_out);
        let mut b = crate::accuracy::reader(paths, &mut aim);
        let mut c = crate::ammo::reader(paths, &mut ammo);
        let mut d = koth.then(|| zones::hill_score_reader(team_of, &mut hill_seen));
        let mut e = zone_input.as_ref().filter(|_| exact).map(|z| zones::objects_reader(&z.3, &mut objects_seen));
        let mut readers: Vec<&mut Reader<'_>> = vec![&mut a, &mut b, &mut c];
        if let Some(d) = d.as_mut() {
            readers.push(d);
        }
        if let Some(e) = e.as_mut() {
            readers.push(e);
        }
        walk_shared(&film, p, &mut readers);
    }
    let acts = walk.map(|_| acts_out);

    // The highlight clock: the kill events fix it to the tick.
    let hl_ticks = film.highlight_ticks();
    let pi_of: HashMap<&str, i32> = roster
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.xuid.is_some())
        .map(|(i, _)| (xuids[i].as_str(), i as i32))
        .collect();
    let hl_zero_tick = match (&fired, ex.hl_zero_tick) {
        (Some(ff), Some(z)) => {
            let deaths: Vec<(i32, f64)> = highlights
                .iter()
                .filter(|h| h.event_type == EventType::Death)
                .filter_map(|h| Some((*pi_of.get(h.xuid.as_str())?, h.time_ms)))
                .collect();
            Some(fire::highlight_zero(ff, &hl_ticks, &deaths, z).unwrap_or(z))
        }
        (_, z) => z,
    };
    // The mode scripts' events, which also fix the highlight clock when the
    // kills don't.
    let script_events = match &walk {
        Some(p) => game_events(&film, p, calib.as_ref(), &holders),
        None => Vec::new(),
    };
    let hl_zero_tick = hl_zero_tick.or_else(|| {
        let medals: Vec<(i32, f64)> = highlights
            .iter()
            .filter(|h| h.event_type == EventType::Medal)
            .filter_map(|h| Some((*pi_of.get(h.xuid.as_str())?, h.time_ms)))
            .collect();
        let masks: Vec<(i64, u32)> = script_events.iter().map(|(f, g)| (*f, g.players.0)).collect();
        medal_clock(&medals, &masks, &hl_ticks)
    });
    let clock = Clock { frame_ticks: ex.frame_ticks.clone(), highlight_ticks: hl_ticks, highlight_zero: hl_zero_tick };
    let tick = |frame: i64| Tick(usize::try_from(frame).ok().and_then(|f| ex.frame_ticks.get(f)).copied().unwrap_or(0));
    let pid = |i: i32| u8::try_from(i).ok().map(PlayerId);
    let at_frame = |k: usize, frame: i64| -> Option<Position> {
        let wp = &world[k];
        let i = wp.positions.partition_point(|s| s.frame <= frame).checked_sub(1)?;
        let s = &wp.positions[i];
        (calib.is_some() && fire::within_ticks(&ex.frame_ticks, s.frame, frame, SAMPLE_TICKS)).then(|| Position {
            x: s.x,
            y: s.y,
            z: wp.states.get(i).and_then(|st| st.z),
        })
    };
    let path_of = |player: i32| paths.iter().position(|p| p.player_index == player);

    let mut events: Vec<(i64, Event)> = Vec::new();

    // A death the scoreboard counts moves the victim's deaths within a few
    // frames of the kill.
    let death_frames: Vec<(u32, i64)> =
        stat_log.changes.iter().filter(|c| c.kind == 0 && c.slot == DEATHS_SLOT).map(|c| (c.id, c.frame)).collect();
    let scored = |victim: PlayerId, frame: i64| {
        (!stat_log.changes.is_empty())
            .then(|| death_frames.iter().any(|&(id, f)| id == victim.0 as u32 && (f - frame).abs() <= DEATH_FRAMES))
    };
    if let Some(ff) = &fired {
        // Kills, with the fatal hit's cause and weapon.
        for (&(frame, victim, killer), d) in ff.kills.iter().zip(&ff.kill_details) {
            // The event names engine player ids; a joiner may hold another's.
            let held = |id: Option<u32>| id.map(|i| holders.api(i, frame));
            let (victim, killer, assistant) = (held(victim), held(killer), held(d.assistant));
            let Some(victim) = victim.and_then(|v| pid(v as i32)) else { continue };
            let vk = path_of(victim.0 as i32);
            let kk = killer.and_then(|c| path_of(c as i32));
            let tag = vk.and_then(|k| fire::kill_cause(ff, k, &ex.frame_ticks, frame));
            let (cause, owner) = match tag {
                Some(t)
                    if damage::grenade_blast(t)
                        && kk.is_some_and(|c| fire::threw_before(ff, c, &ex.frame_ticks, frame)) =>
                {
                    (Some(Cause::Grenade), None)
                }
                Some(t) => {
                    let (c, o) = damage::cause(t);
                    (Some(c), o)
                }
                None => (None, None),
            };
            let named = owner.and_then(|o| ff.named(o));
            let weapon = match cause {
                Some(Cause::Weapon | Cause::Unknown) | None if named.is_none() => {
                    kk.and_then(|k| fire::shot_before(ff, k, &ex.frame_ticks, frame))
                }
                _ => named,
            };
            let kill = Kill {
                victim,
                killer: killer.and_then(|k| pid(k as i32)),
                killer_damage_percent: killer.map(|_| d.killer_percent),
                assistant: assistant.and_then(|a| pid(a as i32)),
                assistant_damage_percent: assistant.map(|_| d.assistant_percent),
                suicide: d.suicide,
                headshot: None,
                scored: scored(victim, frame),
                cause,
                weapon: weapon.map(|w| TagId(ff.weapons[w].id)).or(owner.map(TagId)),
                position: vk.and_then(|k| at_frame(k, frame)),
            };
            events.push((frame, Event::Kill(kill)));
        }
        for (k, shots) in ff.shots.iter().enumerate() {
            let Some(player) = pid(paths[k].player_index) else { continue };
            for &(frame, w) in shots {
                events.push((frame, Event::Shot(Shot { player, weapon: TagId(ff.weapons[w].id) })));
            }
        }
        let hit_damage = vitals.as_ref().map(|v| crate::hit_damage::read(ff, v)).unwrap_or_default();
        let hit_weapons = crate::weapon_stats::hit_weapons(ff, &ex.frame_ticks);
        for (i, &(frame, attacker, victim)) in ff.hits.iter().enumerate() {
            let Some(victim) = pid(paths[victim].player_index) else { continue };
            let dmg = hit_damage.get(i).copied().unwrap_or_default();
            let hit = Hit {
                attacker: attacker.and_then(|a| pid(paths[a].player_index)),
                victim,
                damage: ff.hit_details[i].1.map(TagId),
                weapon: hit_weapons.get(i).copied().flatten().map(|w| TagId(ff.weapons[w].id)),
                shield: dmg.shield,
                body: dmg.body,
                from_vitals: dmg.measured,
            };
            events.push((frame, Event::Hit(hit)));
        }
        for (k, throws) in ff.throws.iter().enumerate() {
            let Some(player) = pid(paths[k].player_index) else { continue };
            for &(frame, g) in throws {
                events.push((frame, Event::Throw(Throw { player, grenade: g.and_then(Grenade::from_type) })));
            }
        }
        for (k, exits) in ff.exits.iter().enumerate() {
            let Some(player) = pid(paths[k].player_index) else { continue };
            for &(frame, v, seat) in exits {
                events.push((frame, Event::VehicleExit(VehicleExit { player, vehicle: EntityId(v as u16), seat })));
            }
        }
        for &(frame, sound, mask) in &ff.voice {
            let recipients = PlayerMask(players_mask(&holders, mask, frame));
            events.push((frame, Event::VoiceLine(VoiceLine { sound: TagId(sound), recipients })));
        }
        for &(frame, k, sound, priority, line) in &ff.chatter {
            let speaker = usize::try_from(k).ok().and_then(|k| pid(paths[k].player_index));
            events.push((frame, Event::Chatter(Chatter { speaker, sound: TagId(sound), priority, line })));
        }
    }

    let slot_owner: HashMap<i32, usize> =
        paths.iter().enumerate().flat_map(|(k, p)| p.life_slots.iter().flatten().map(move |&s| (s, k))).collect();
    for p in &inv.pickups {
        let Some(player) = slot_owner.get(&p.slot_id).and_then(|&k| pid(paths[k].player_index)) else { continue };
        events.push((p.frame, Event::Pickup(Pickup { player, item: TagId(p.tag), slot: p.slot })));
    }

    if let Some(a) = &acts {
        for (k, list) in a.actions.iter().enumerate() {
            let Some(player) = pid(paths[k].player_index) else { continue };
            for &(frame, kind, arg) in list {
                let kind = match kind {
                    actions::RELOAD => ActionKind::Reload,
                    actions::MELEE => ActionKind::Melee { target: pid(arg) },
                    actions::ZOOM => ActionKind::Zoom { level: arg.max(0) as u32 },
                    actions::EMPTY_CLICK => ActionKind::EmptyClick,
                    actions::CAMO => ActionKind::Camo,
                    actions::THRUST => ActionKind::Thruster,
                    actions::GRAPPLE => ActionKind::Grapple,
                    actions::SPRINT => ActionKind::Sprint,
                    actions::WEAPON_SET => ActionKind::WeaponSwitch { slot: arg.max(0) as u32 },
                    _ => continue,
                };
                events.push((frame, Event::Action(Action { player, kind })));
            }
        }
        for &(frame, pi) in &a.headshots {
            if let Some(p) = pid(pi) {
                events.push((frame, Event::Headshot(p)));
            }
        }
    }
    for (k, list) in jumps.iter().enumerate() {
        let Some(player) = pid(paths[k].player_index) else { continue };
        for &frame in list {
            events.push((frame, Event::Action(Action { player, kind: ActionKind::Jump })));
        }
    }

    // What needs world units.
    let mut grenade_flights = Vec::new();
    let mut ground = Vec::new();
    let mut zones_out = Vec::new();
    let mut hill_score = None;
    let mut vehicles = Vec::new();
    let mut rides = Vec::new();
    if let (Some(c), Some(w)) = (&calib, widths) {
        let flights =
            walk.map(|_| pair_grenades(&thrown, paths, &world, w, c, level, &ex.frame_ticks)).unwrap_or_default();
        if let Some(a) = &acts {
            for &(frame, k, from, to) in &a.teleports {
                let Some(player) = pid(paths[k].player_index) else { continue };
                let (fx, fy) = window_to_world(from, w, c);
                let (tx, ty) = window_to_world(to, w, c);
                let t = Teleport { player, from: Position::new(fx, fy), to: Position::new(tx, ty) };
                events.push((frame, Event::Teleport(t)));
            }
            for &(frame, projectile, q) in &a.blasts {
                let grenade = projectile
                    .and_then(|e| {
                        flights
                            .iter()
                            .find(|g| g.entity == e && g.from <= frame && frame <= g.end + BLAST_AFTER_END_FRAMES)
                    })
                    .and_then(|g| g.grenade)
                    .and_then(Grenade::from_type);
                let (x, y) = window_to_world(q, w, c);
                events.push((frame, Event::Detonation(Detonation { position: Position::new(x, y), grenade })));
            }
        }
        for g in &inv.ground {
            let Some((x, y)) = place(g.at_box, g.at, w, c, level) else { continue };
            let to = g.to.map(tick);
            ground.push(GroundWeapon {
                tag: TagId(g.tag),
                position: Position::new(x, y),
                from: tick(g.from),
                to,
                taken: g.taken,
            });
        }
        for f in flights {
            let Some(player) = pid(paths[f.path_index].player_index) else { continue };
            grenade_flights.push(GrenadeFlight {
                player,
                grenade: f.grenade.and_then(Grenade::from_type),
                path: f.path.iter().map(|&(fr, x, y)| (tick(fr), Position::new(x, y))).collect(),
                end: tick(f.end),
                deleted: f.deleted,
            });
        }
        // Flags, placed where the event says, else at the player's latest
        // sample in the two seconds before.
        if let Some(p) = &walk {
            for o in objectives::read(&film, p.into(), mode == GameMode::CaptureTheFlag) {
                let position = match o.at {
                    Some(q) => {
                        let (x, y) = window_to_world(q, w, c);
                        Some(Position::new(x, y))
                    }
                    None => path_of(o.player).and_then(|k| at_frame(k, o.frame)),
                };
                let kind = match o.kind {
                    objectives::GRAB => FlagEventKind::Grab,
                    objectives::STEAL => FlagEventKind::Steal,
                    objectives::CAPTURE => FlagEventKind::Capture,
                    objectives::RETURN => FlagEventKind::Return,
                    objectives::CARRIER_KILLED => FlagEventKind::CarrierKilled,
                    _ => FlagEventKind::CaptureAssist,
                };
                if let Some(player) = pid(o.player) {
                    events.push((o.frame, Event::Flag(FlagEvent { kind, player, position })));
                }
            }
        }
        // Equipment uses: the player nearest the object it put down.
        if walk.is_some() {
            for u in deploys {
                let Some((x, y)) = place(u.at_box, u.at, w, c, level) else { continue };
                let mut d: Vec<(f64, i32)> = world
                    .iter()
                    .filter_map(|wp| {
                        let k = wp.positions.partition_point(|s| s.frame <= u.frame);
                        let s = &wp.positions[k.checked_sub(1)?];
                        (u.frame - s.frame <= DEPLOY_SAMPLE_FRAMES).then(|| ((s.x - x).hypot(s.y - y), wp.player))
                    })
                    .collect();
                d.sort_by(|a, b| f64::total_cmp(&a.0, &b.0));
                let Some(&(near, who)) = d.first() else { continue };
                let next = d.get(1).map_or(f64::INFINITY, |n| n.0);
                let Some(player) = pid(who).filter(|_| near <= DEPLOY_NEAR_WU && next >= DEPLOY_APART * near) else {
                    continue;
                };
                let used = EquipmentUse {
                    player,
                    equipment: TagId(u.equipment),
                    object: TagId(u.object),
                    position: Position::new(x, y),
                };
                events.push((u.frame, Event::EquipmentUse(used)));
            }
        }
        // Zones, hills and the skull.
        if let (Some((m, evs, places, ids)), Some(p)) = (zone_input, walk) {
            let snap = if exact { zones::snapshot_places(&film, p, &ids) } else { BTreeMap::new() };
            if m == zones::Mode::Koth && exact {
                hill_score = zones::finish_hill_score(hill_seen).map(|h| HillScore {
                    points_to_win: h.to_win,
                    hills_to_win: h.hills,
                    points: h
                        .runs
                        .iter()
                        .filter_map(|&(f, t, n)| Some((tick(f), TeamId(u8::try_from(t).ok()?), n)))
                        .collect(),
                });
            }
            let mut objects = if exact { zones::finish_objects(objects_seen) } else { BTreeMap::new() };
            // The players inside, limited to the roster: before the match a
            // mask can name every slot.
            let on_roster = paths.iter().fold(0u32, |m, p| m | (0x8000_0000u32 >> (p.player_index as u32 & 31)));
            let team = |t: i32| u8::try_from(t).ok().map(TeamId);
            for &id in &ids {
                let (x, y) = match (snap.get(&id), places.get(&id).copied().flatten()) {
                    (Some(a), _) => (-WORLD_PER_ENGINE * a[1] as f64, WORLD_PER_ENGINE * a[0] as f64),
                    (None, Some(q)) => window_to_world(q, w, c),
                    (None, None) => continue,
                };
                let o = objects.remove(&id).unwrap_or_default();
                let mut states: Vec<ZoneState> = o
                    .states
                    .iter()
                    .map(|s| ZoneState {
                        tick: tick(s.frame),
                        owner: team(s.owner),
                        capturing: team(s.capturing),
                        progress: s.progress,
                        contested: s.contested,
                        inside: PlayerMask(players_mask(&holders, s.inside, s.frame) & on_roster),
                    })
                    .collect();
                states.dedup_by(|b, a| {
                    (a.owner, a.capturing, a.progress, a.contested, a.inside)
                        == (b.owner, b.capturing, b.progress, b.contested, b.inside)
                });
                let shape = o.shape.map(|s| ZoneShape {
                    area: if s.cylinder {
                        ZoneArea::Cylinder { radius: s.a }
                    } else {
                        ZoneArea::Box { length: s.a, width: s.b }
                    },
                    top: s.top,
                    bottom: s.bottom,
                    angle: s.angle,
                    team: s.team.and_then(team),
                });
                zones_out.push(Zone { entity: id, position: Position::new(x, y), letter: o.letter, shape, states });
            }
            for e in &evs {
                let kind = match e.kind {
                    zones::CAPTURE => ZoneEventKind::Capture,
                    zones::SECURE => ZoneEventKind::Secure,
                    zones::SCORE => ZoneEventKind::Score,
                    zones::HILL_APPEAR => ZoneEventKind::HillAppear,
                    zones::HILL_LIVE => ZoneEventKind::HillLive,
                    zones::HILL_CONTROLLED => ZoneEventKind::HillControlled,
                    zones::BOSS_WAVE => ZoneEventKind::BossWave,
                    zones::SET_START => ZoneEventKind::SetStart,
                    zones::SET_COMPLETE => ZoneEventKind::SetComplete,
                    zones::SKULL_GRAB => ZoneEventKind::SkullGrab,
                    zones::SKULL_SPAWN => ZoneEventKind::SkullSpawn,
                    zones::HILL_WON => ZoneEventKind::HillWon,
                    zones::CONTEST_START => ZoneEventKind::ContestStart,
                    zones::CONTEST_END => ZoneEventKind::ContestEnd,
                    zones::CONTROL_LOST => ZoneEventKind::ControlLost,
                    zones::NEUTRAL => ZoneEventKind::Neutral,
                    _ => ZoneEventKind::NearlyWon,
                };
                let zone = e.zone.and_then(|z| zones_out.iter().position(|q| q.entity == z));
                events
                    .push((e.frame, Event::Zone(ZoneEvent { kind, player: pid(e.player), team: team(e.team), zone })));
            }
        }
        // Vehicles and the rides in them: the runs of a path's samples
        // spliced from one vehicle. With the parent state read a run is cut
        // where the life changes seat or moves into a part, its seat is the
        // one the parent state names, and the run that ends its attachment
        // lasts as long as the attachment: a vehicle can stop sending its
        // position with the rider still seated. Otherwise the seat is that of
        // the exit that ends the run.
        let tracks = vehicle_tracks(&ex, c);
        for (k, wp) in world.iter().enumerate() {
            let Some(player) = pid(wp.player) else { continue };
            let mine: std::collections::HashSet<i32> = paths[k].life_slots.iter().flatten().copied().collect();
            let spans: Vec<&crate::attach::Span> =
                ex.attachments.bipeds.iter().filter(|s| mine.contains(&s.id)).collect();
            let span_at = |f: i64| spans.iter().position(|s| s.from <= f && f <= s.to);
            let pos = &wp.positions;
            let mut i = 0;
            while i < pos.len() {
                let v = pos[i].vehicle;
                if !pos[i].in_vehicle || v < 0 {
                    i += 1;
                    continue;
                }
                let at = span_at(pos[i].frame);
                let mut j = i;
                while j + 1 < pos.len()
                    && pos[j + 1].in_vehicle
                    && pos[j + 1].vehicle == v
                    && span_at(pos[j + 1].frame) == at
                {
                    j += 1;
                }
                let last_of_span = at.is_some() && (j + 1 == pos.len() || span_at(pos[j + 1].frame) != at);
                let to = match at {
                    Some(at) if last_of_span => spans[at].to.max(pos[j].frame),
                    _ => pos[j].frame,
                };
                let from = pos[i].frame;
                let track =
                    tracks.iter().position(|t| t.0 == v && t.2[0].frame <= from && from <= t.2[t.2.len() - 1].frame);
                let seat = match at {
                    Some(at) => spans[at].seat,
                    None => fired.as_ref().and_then(|ff| {
                        ff.exits[k]
                            .iter()
                            .rfind(|x| ex.vehicle_ident.resolve(x.1) == v && x.0 >= from && x.0 <= to + EXIT_SLACK)
                            .map(|x| x.2)
                    }),
                };
                rides.push(Ride { player, vehicle: track, seat, from: tick(from), to: tick(to) });
                i = j + 1;
            }
        }
        rides.sort_by_key(|r| (r.from, r.player));
        vehicles = tracks
            .into_iter()
            .map(|(id, tag, pts)| Vehicle {
                entity: EntityId(id as u16),
                tag: TagId(tag),
                track: pts.iter().map(|p| (tick(p.frame), Position::new(p.x, p.y))).collect(),
            })
            .collect();
    }

    for s in &splashes {
        let arguments = s
            .params
            .iter()
            .zip(&s.values)
            .map(|(&p, v)| match (u32::try_from(p), v) {
                (Ok(p), _) => pid(holders.api(p, s.frame) as i32).map(HudArgument::Player),
                (Err(_), Some(crate::splash::Arg::Number(n))) => Some(HudArgument::Number(*n)),
                (Err(_), Some(crate::splash::Arg::Key(k))) => Some(HudArgument::Text(*k)),
                (Err(_), None) => None,
            })
            .collect();
        events.push((s.frame, Event::HudMessage(HudMessage { key: s.key, arguments, viewers: s.seen_by })));
    }
    for (frame, g) in script_events {
        events.push((frame, Event::GameEvent(g)));
    }
    for r in &stat_log.respawns {
        if let Some(player) = pid(r.player as i32) {
            events.push((r.frame, Event::RespawnCountdown(RespawnCountdown { player, seconds: r.seconds })));
        }
    }

    // The HighlightEvents records, and their medals into the stream.
    let mut highlights_out = Vec::new();
    for h in &highlights {
        let player = pi_of.get(h.xuid.as_str()).and_then(|&i| pid(i));
        let t = clock.highlight_tick(h.time_ms as u32);
        let kind = match h.event_type {
            EventType::Kill => HighlightKind::Kill,
            EventType::Death => HighlightKind::Death,
            EventType::Medal => HighlightKind::Medal(h.medal_value),
            EventType::Mode => HighlightKind::Mode,
        };
        if let (HighlightKind::Medal(index), Some(player), Some(t)) = (kind, player, t) {
            let frame = clock.frame_at(t).unwrap_or(0) as i64;
            events.push((frame, Event::Medal(MedalAward { player, index })));
        }
        let xuid = Xuid(h.xuid.parse().unwrap_or(0));
        highlights_out.push(Highlight {
            player,
            xuid,
            gamertag: h.gamertag.clone(),
            kind,
            time_ms: h.time_ms as u32,
            tick: t,
        });
    }

    events.sort_by_key(|e| e.0);
    let mut events: Vec<TimedEvent> =
        events.into_iter().map(|(f, event)| TimedEvent { tick: tick(f), event }).collect();
    mark_headshots(&mut events);

    // Lives: each path split at its life starts, then the unowned ones.
    let coordinates = calib.as_ref().map(|c| Coordinates {
        source: match c.source {
            CalibSource::LevelTable => CoordinateSource::LevelBox,
            CalibSource::ForgeCanvas => CoordinateSource::ForgeCanvas,
            CalibSource::AssetTable => CoordinateSource::KnownLayout,
            CalibSource::SpawnFit => CoordinateSource::SpawnFit,
            CalibSource::ExtentFit => CoordinateSource::ExtentFit,
            CalibSource::TwoSpawn => CoordinateSource::TwoSpawn,
            CalibSource::SingleSpawn => CoordinateSource::SingleSpawn,
        },
        scale_x: c.scale_x,
        scale_y: c.scale_y,
        offset_x: c.offset_x,
        offset_y: c.offset_y,
        spawn_fit: c.spawn_samples.zip(c.spawn_inliers),
    });
    let sample = |p: &MotionPoint, wpos: Option<&WorldPosition>, st: Option<&SampleState>| Sample {
        tick: tick(p.frame),
        raw: [p.raw1, p.raw2],
        position: wpos.map(|w| Position { x: w.x, y: w.y, z: st.and_then(|s| s.z) }),
        yaw: st.and_then(|s| s.yaw),
        speed: st.and_then(|s| s.speed),
        health: st.and_then(|s| s.health),
        shield: st.and_then(|s| s.shield),
        in_vehicle: p.in_vehicle,
        vehicle: u16::try_from(p.vehicle).ok().map(EntityId),
    };
    let deaths = match &ex.deaths {
        Some(d) => d.clone(),
        None => small_lobby_deaths(&film, roster.players.len(), Some(&entries)),
    };
    let mut lives = Vec::new();
    for (p, wp) in paths.iter().zip(&world) {
        let starts = p.life_starts.clone().unwrap_or_else(|| vec![0]);
        for (k, &a) in starts.iter().enumerate() {
            let b = starts.get(k + 1).copied().unwrap_or(p.positions.len()).min(p.positions.len());
            if a >= b {
                continue;
            }
            let samples = (a..b)
                .map(|i| sample(&p.positions[i], calib.as_ref().and(wp.positions.get(i)), wp.states.get(i)))
                .collect();
            let slot = p.life_slots.as_ref().and_then(|s| s.get(k)).copied();
            let entity = slot.and_then(|s| u16::try_from(s).ok()).map(EntityId);
            let died = slot.is_some_and(|s| deaths.iter().any(|d| d.slot_id == s));
            lives.push(Life { player: pid(p.player_index), entity, died, death: None, samples });
        }
    }
    if let Some(c) = &calib {
        for p in &ex.unassigned {
            let w = apply_calibration(&p.positions, c);
            let samples = p.positions.iter().zip(&w).map(|(m, w)| sample(m, Some(w), None)).collect();
            lives.push(Life { player: None, entity: None, died: false, death: None, samples });
        }
    }
    lives.sort_by_key(|l| l.start());
    // Each kill ends the victim's life that started last before it.
    for e in &events {
        let Event::Kill(k) = &e.event else { continue };
        let life =
            lives.iter_mut().filter(|l| l.player == Some(k.victim) && l.start().is_some_and(|s| s <= e.tick)).last();
        if let Some(l) = life.filter(|l| l.death.is_none()) {
            l.death = Some(e.tick);
        }
    }

    let players: Vec<Player> = roster
        .players
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let kind = match (&p.bot, p.xuid) {
                (Some((bot_id, name)), _) => PlayerKind::Bot { bot_id: *bot_id, name: name.clone() },
                (None, Some(xuid)) => PlayerKind::Human { xuid: Xuid(xuid), gamertag: p.gamertag.clone() },
                (None, None) => return None,
            };
            Some(Player {
                id: PlayerId(u8::try_from(i).ok()?),
                kind,
                team: p.team.and_then(|t| u8::try_from(t).ok()).map(TeamId),
                present_at_start: p.present_at_start,
                joined: p.first.filter(|_| !p.present_at_start).map(tick),
                left: p.left.map(tick),
            })
        })
        .collect();

    // What each player pressed and carried.
    let mut states: BTreeMap<PlayerId, PlayerState> = BTreeMap::new();
    for (&pi, series) in &inputs {
        let Some(p) = pid(pi as i32) else { continue };
        states.entry(p).or_default().inputs = series
            .iter()
            .map(|&(f, forward, left, stance)| Input {
                tick: tick(f),
                forward: forward as i8,
                left: left as i8,
                stance: u8::try_from(stance).ok(),
            })
            .collect();
    }
    for (k, a) in ammo.iter().enumerate() {
        let Some(p) = pid(paths[k].player_index) else { continue };
        let s = states.entry(p).or_default();
        s.ammo =
            a.ammo.iter().map(|&(f, slot, loaded, reserve)| Ammo { tick: tick(f), slot, loaded, reserve }).collect();
        s.energy = a.energy.iter().map(|&(f, slot, percent)| Energy { tick: tick(f), slot, percent }).collect();
        s.heat = a.heat.iter().map(|&(f, slot, percent, flags)| Heat { tick: tick(f), slot, percent, flags }).collect();
        s.grenades = a.grenades.iter().map(|&(f, counts)| GrenadeCount { tick: tick(f), counts }).collect();
        s.equipment = a
            .equipment
            .iter()
            .map(|&(f, percent, recharge_delay)| EquipmentCharge { tick: tick(f), percent, recharge_delay })
            .collect();
        s.weapons = a
            .weapons
            .iter()
            .map(|&(f, slot, w)| WeaponSlot { tick: tick(f), slot, weapon: w.map(TagId) })
            .collect();
        s.held = a.held.iter().map(|&(f, slot)| HeldSlot { tick: tick(f), slot }).collect();
    }

    let scoreboard = Scoreboard {
        changes: stat_log
            .changes
            .iter()
            .filter_map(|c| {
                let owner = match c.kind {
                    0 => StatOwner::Player(pid(c.id as i32)?),
                    _ => StatOwner::Team(TeamId(u8::try_from(c.id).ok()?)),
                };
                Some(StatChange { tick: tick(c.frame), owner, slot: c.slot, value: c.value })
            })
            .collect(),
        clock: stat_log
            .clock
            .iter()
            .map(|c| MatchClock { tick: tick(c.frame), limit: c.limit, left: c.left, state: c.state })
            .collect(),
        engine: match &level {
            Some(lv) if exact && packets => {
                let g = crate::gamestate::read(&film, lv.widths, lv.box_bits, &holders);
                g.frames
                    .iter()
                    .enumerate()
                    .map(|(k, &f)| EngineState {
                        tick: tick(f),
                        phase: g.phase[k],
                        lives_left: g
                            .lives
                            .iter()
                            .filter_map(|(&p, series)| Some((pid(p as i32)?, series.get(k).copied().flatten()?)))
                            .collect(),
                        rows: g
                            .players
                            .iter()
                            .filter_map(|(&p, series)| Some((StatOwner::Player(pid(p as i32)?), series.get(k)?.to_vec())))
                            .chain(g.teams.iter().filter_map(|(&t, series)| {
                                Some((StatOwner::Team(TeamId(u8::try_from(t).ok()?)), series.get(k)?.to_vec()))
                            }))
                            .collect(),
                    })
                    .collect()
            }
            _ => Vec::new(),
        },
    };

    let world_objects = match params {
        Some(p) if exact && packets => crate::world::read(&film, p)
            .into_iter()
            .map(|o| WorldObject {
                tag: TagId(o.tag),
                entity_type: o.entity_type,
                position: Position::new(o.x, o.y),
                from: tick(o.from),
                to: o.to.map(tick),
            })
            .collect(),
        _ => Vec::new(),
    };

    let variant = crate::variant::read(&film.chunks[0]).map(|v| GameVariant {
        name: v.name,
        description: v.description,
        stat_labels: v.stats,
        respawn_seconds: v.respawn,
        score_to_win: v.score_to_win,
        time_limit_seconds: v.time_limit,
        suicide_penalty_seconds: v.suicide_penalty,
        betrayal_penalty_seconds: v.betrayal_penalty,
        overtime_seconds: v.overtime,
        last_chance_seconds: v.last_chance,
        loadout: Loadout {
            weapons: v.loadout.iter().map(|&t| TagId(t)).collect(),
            random: v.loadout_random,
            equipment: v.equipment.map(TagId),
        },
        settings: v.rules,
        flags: v.flags,
        options: v
            .options
            .iter()
            .map(|&(hash, value)| VariantOption { hash, name: game::option_name(hash), value })
            .collect(),
        scripts: scripts.iter().cloned().collect(),
        stat_settings: crate::variant_stats::read(&film.chunks[0]).map(|d| StatSettings {
            stats: d
                .stats
                .into_iter()
                .map(|s| StatDefinition {
                    name: s.name,
                    id: s.id,
                    base: s.base,
                    aggregation: s.aggregation,
                    lower_is_better: s.sort == -1,
                    display: StatDisplay::from_game(s.display),
                    precision: s.precision,
                })
                .collect(),
            scoreboard: d.scoreboard,
            server_30fps: d.server_30fps,
            tick_rate: d.tick_rate,
            kill_cam: d.kill_cam,
            kill_cam_seconds: d.kill_cam_seconds,
            play_of_the_game: d.play_of_the_game,
            machinima: d.machinima,
            spawn_settings: d.spawn_settings.map(TagId),
        }),
    });

    // What the analysis module derives from.
    let accuracy = paths
        .iter()
        .enumerate()
        .filter_map(|(k, p)| {
            Some(AccuracyData {
                player: pid(p.player_index)?,
                projectiles: aim.projectiles.get(k).into_iter().flatten().map(|&f| tick(f)).collect(),
                landed: aim.hits.get(k).into_iter().flatten().map(|&f| tick(f)).collect(),
            })
        })
        .collect();
    let weapons = fired.as_ref().map_or(Vec::new(), |ff| {
        let hit_weapons = crate::weapon_stats::hit_weapons(ff, &ex.frame_ticks);
        let shieldless = vitals.as_ref().is_some_and(|v| v.shieldless());
        crate::weapon_stats::stats(ff, &hit_weapons, shieldless)
            .into_iter()
            .zip(&ff.weapons)
            .filter_map(|(s, w)| {
                let s = s?;
                Some(WeaponProfile {
                    weapon: TagId(w.id),
                    rounds_per_second: s.rounds_per_second,
                    burst: s.burst,
                    magazine: s.magazine,
                    // Engine units to world units, as positions are.
                    velocity: s.velocity.map(|v| v * 10.0),
                    range: s.range.map(|r| r * 10.0),
                    damage: s.damage,
                    hits_to_kill: s.hits_to_kill,
                })
            })
            .collect()
    });
    let spawn_factors = if composite && calib.is_some() && !hints.spawn_pads.is_empty() {
        let pads: Vec<crate::spawns::Pad> = hints
            .spawn_pads
            .iter()
            .enumerate()
            .map(|(index, p)| crate::spawns::Pad {
                index,
                x: p.position.x,
                y: p.position.y,
                z: p.position.z,
                respawn: p.respawn,
                team: p.team.map(|t| t.0 as i64),
            })
            .collect();
        let tracks: Vec<crate::spawns::WorldTrack<'_>> = world
            .iter()
            .map(|w| crate::spawns::WorldTrack { positions: &w.positions, z: w.states.iter().map(|s| s.z).collect() })
            .collect();
        crate::spawns::explain(&pads, paths, &tracks, &ex.frame_ticks)
            .into_iter()
            .filter_map(|s| {
                Some(SpawnFactor {
                    tick: tick(s.frame),
                    player: pid(s.player)?,
                    pad: s.pad,
                    score: s.score,
                    better: s.better,
                    same: s.same,
                    worse: s.worse,
                    sources: s.counts,
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    let header = Header {
        build: raw_header.build.clone(),
        branch: raw_header.branch.clone(),
        started_at: raw_header.start,
        map_asset_id: raw_header.map_asset_id.map(Guid),
        map_version_id: raw_header.map_version_id.map(Guid),
        level: hints.level.or(raw_header.level.map(TagId)),
    };

    Ok(crate::Film {
        raw: film,
        params,
        has_highlights,
        encoding: match layout.mode {
            EncodingMode::PerEntity => crate::format::Encoding::PerEntity,
            EncodingMode::BotComposite => crate::format::Encoding::BotComposite,
            EncodingMode::MultiplayerComposite => crate::format::Encoding::Composite,
        },
        header,
        clock,
        players,
        variant,
        level: level_model,
        coordinates,
        lives,
        vehicles,
        rides,
        world: world_objects,
        ground,
        grenades: grenade_flights,
        zones: zones_out,
        hill_score,
        events,
        highlights: highlights_out,
        scoreboard,
        states: states.into_iter().collect(),
        analysis: AnalysisData { accuracy, weapons, spawn_factors },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medals_fix_the_highlight_clock() {
        let hl_ticks: Vec<u32> = (0..2000).collect();
        // Medals at 1, 10 and 20 s for players 0 and 2; each recipient gets an
        // event 13 ticks after the medal's time, among others.
        let medals = [(0, 1000.0), (2, 10_000.0), (0, 20_000.0), (2, 25_000.0)];
        let at = |ms: f64| (ms * crate::lives::TICKS_PER_MS).round() as i64 + 13;
        let mut events: Vec<(i64, u32)> = medals.iter().map(|&(p, ms)| (at(ms), 0x8000_0000u32 >> p)).collect();
        events.extend([(5, 0x8000_0000), (700, 0xa000_0000), (1500, 0x2000_0000)]);
        let zero = medal_clock(&medals, &events, &hl_ticks).unwrap();
        assert!((zero - 13.0).abs() <= 0.5, "{zero}");
        // Too few medals agree: no clock.
        assert_eq!(medal_clock(&medals[..2], &events[..2], &hl_ticks), None);
    }

    #[test]
    fn headshots_join_their_kills() {
        let kill = |killer: u8| {
            Event::Kill(Kill {
                victim: PlayerId(9),
                killer: Some(PlayerId(killer)),
                killer_damage_percent: Some(100),
                assistant: None,
                assistant_damage_percent: None,
                suicide: false,
                headshot: None,
                scored: None,
                cause: None,
                weapon: None,
                position: None,
            })
        };
        let at = |t: u32, event: Event| TimedEvent { tick: Tick(t), event };
        let mut events = vec![
            at(100, kill(1)),
            at(101, Event::Headshot(PlayerId(1))),
            at(200, Event::Headshot(PlayerId(2))),
            at(200, kill(1)),
            at(300, kill(3)),
            at(400, Event::Headshot(PlayerId(4))),
        ];
        mark_headshots(&mut events);
        let marks: Vec<Option<bool>> = events
            .iter()
            .filter_map(|e| match &e.event {
                Event::Kill(k) => Some(k.headshot),
                _ => None,
            })
            .collect();
        assert_eq!(marks, [Some(true), Some(false), Some(false)]);
    }
}
