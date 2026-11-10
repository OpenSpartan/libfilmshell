//! Per-player motion from the decoded object updates: vehicle splicing,
//! stationary holds, lives to players, deaths, and the small-lobby decoder.

use crate::bits::bits_at;
use crate::container::{detect_encoding_mode, Film, MIN_PLAYER_FRAMES};
use crate::events::Body;
use crate::highlights::{EventType, HighlightEvent};
use crate::lives::{assign_lives, LifeAssignment, LifePlayer, LifeSlot, LIFE_TUNING, TICKS_PER_MS};
use crate::payload::for_each_event_view;
use crate::replication::{detect_c2_width, detect_coord_layout, extract_multiplayer_composite};
use crate::types::*;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

type Slots = IndexMap<i32, Vec<RawSample>>;

/// Sort samples by global position and relabel them as motion points
/// (`frame` = the frame index every record of a frame shares).
fn sort_to_motion_points(mut frames: Vec<RawSample>) -> Vec<MotionPoint> {
    frames.sort_by_key(|f| f.gp);
    frames
        .iter()
        .map(|f| MotionPoint {
            frame: f.tick,
            raw1: f.c1,
            raw2: f.c2,
            gp: f.gp,
            in_vehicle: f.in_vehicle,
            vehicle: f.vehicle,
        })
        .collect()
}

/// Index in `positions` (gp order) of each life's first sample.
fn life_start_indexes(positions: &[MotionPoint], lives: &[&Vec<RawSample>]) -> Vec<usize> {
    lives
        .iter()
        .map(|frames| {
            let g = frames.iter().map(|f| f.gp).min().map(|g| g as f64).unwrap_or(f64::INFINITY);
            let (mut lo, mut hi) = (0usize, positions.len());
            while lo < hi {
                let m = (lo + hi) >> 1;
                if (positions[m].gp as f64) < g {
                    lo = m + 1;
                } else {
                    hi = m;
                }
            }
            lo
        })
        .collect()
}

/// The roster's spawns summed, from the film's scoreboard, or None when any
/// entry lacks them.
fn roster_spawns(roster: Option<&[RosterEntry]>) -> Option<f64> {
    let roster = roster?;
    if roster.is_empty() {
        return None;
    }
    let mut total = 0.0;
    for r in roster {
        total += r.spawns?;
    }
    Some(total)
}

/// More life-slots than the roster spawned: non-roster bipeds (PvE AI).
fn has_non_roster_bipeds(n_slots: usize, roster: Option<&[RosterEntry]>) -> bool {
    match roster_spawns(roster) {
        Some(expected) => n_slots as f64 > expected + f64::max(8.0, expected * 0.1),
        None => false,
    }
}

/// A slot whose own biped updates mostly list actor control (bots, PvE AI).
fn is_ai_slot(frames: &[RawSample]) -> bool {
    let (mut n, mut ai) = (0usize, 0usize);
    for f in frames {
        if !f.in_vehicle && !f.held {
            n += 1;
            if f.ai {
                ai += 1;
            }
        }
    }
    ai * 2 > n
}

fn max_respawns(r: &RosterEntry) -> f64 {
    if let Some(s) = r.spawns {
        if s >= 1.0 {
            return s - 1.0;
        }
    }
    r.deaths.unwrap_or(f64::INFINITY)
}

/// Time-blind death rule (no usable HighlightEvents): every slot but the
/// latest-ending `n_players` is a death, at its last on-foot sample.
fn slot_end_deaths(slots: &Slots, n_players: usize, roster: Option<&[RosterEntry]>) -> Vec<DeathPosition> {
    if has_non_roster_bipeds(slots.len(), roster) {
        return Vec::new();
    }
    struct S {
        slot_id: i32,
        last: RawSample,
    }
    let slot_list: Vec<S> = slots
        .iter()
        .filter_map(|(&slot_id, f)| {
            let mut foot: Vec<RawSample> = f.iter().copied().filter(|x| !x.in_vehicle).collect();
            if foot.is_empty() {
                return None;
            }
            foot.sort_by_key(|x| x.gp);
            Some(S { slot_id, last: *foot.last().unwrap() })
        })
        .collect();
    if slot_list.len() <= n_players {
        return Vec::new();
    }
    let mut by_last: Vec<&S> = slot_list.iter().collect();
    by_last.sort_by_key(|s| std::cmp::Reverse(s.last.gp));
    let survivors: HashSet<i32> = by_last.iter().take(n_players).map(|s| s.slot_id).collect();
    let mut deaths: Vec<DeathPosition> = slot_list
        .iter()
        .filter(|s| !survivors.contains(&s.slot_id))
        .map(|s| DeathPosition { slot_id: s.slot_id, gp: s.last.gp })
        .collect();
    deaths.sort_by_key(|d| d.gp);
    deaths
}

/// Fallback stitcher for films without usable HighlightEvents: initial lives
/// by slot id equal to roster index, respawns FIFO by spawn side, capped at
/// spawns minus 1. Returns the paths and the slots it could not link. Sorts
/// each kept slot's samples by gp in place.
fn stitch_slots_to_players(
    slots: &mut Slots,
    roster: Option<&[RosterEntry]>,
) -> (Vec<PlayerPath>, Vec<Vec<RawSample>>) {
    let n_players = roster.map(|r| r.len()).unwrap_or(0);
    struct S {
        slot_id: i32,
        frames: Vec<RawSample>,
        first_gp: i64,
        last_gp: i64,
        first_c1: f64,
        ai: bool,
    }
    let mut slot_list: Vec<S> = Vec::new();
    for (&slot_id, frames) in slots.iter_mut() {
        if frames.len() < 3 {
            continue;
        }
        frames.sort_by_key(|f| f.gp);
        slot_list.push(S {
            slot_id,
            frames: frames.clone(),
            first_gp: frames[0].gp,
            last_gp: frames[frames.len() - 1].gp,
            first_c1: frames[0].c1,
            ai: is_ai_slot(frames),
        });
    }
    slot_list.sort_by_key(|s| s.first_gp);
    if slot_list.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let roster = match roster {
        Some(r) if !r.is_empty() => r,
        _ => {
            let paths = slot_list
                .into_iter()
                .enumerate()
                .map(|(i, s)| PlayerPath {
                    player_index: i as i32,
                    is_bot: false,
                    team_id: None,
                    positions: sort_to_motion_points(s.frames),
                    life_slots: None,
                    life_starts: None,
                })
                .collect();
            return (paths, Vec::new());
        }
    };

    struct P {
        frames: Vec<RawSample>,
        last_gp: i64,
        side: i64,
        respawns: f64,
        life_slots: Vec<i32>,
        lives: Vec<usize>, // indexes into slot_list
    }
    let by_slot: HashMap<i32, usize> = slot_list.iter().enumerate().map(|(i, s)| (s.slot_id, i)).collect();
    let is_bot_at = |ri: usize| roster.get(ri).is_some_and(|r| r.is_bot());
    let mut seeded_ids: HashSet<i32> = HashSet::new();
    let mut players: Vec<P> = Vec::new();
    for (ri, entry) in roster.iter().enumerate().take(n_players) {
        let found = by_slot.get(&(ri as i32)).copied();
        let s = found.filter(|&i| slot_list[i].ai == is_bot_at(ri));
        if s.is_some() {
            seeded_ids.insert(ri as i32);
        }
        let team = entry.last_team_id.unwrap_or(ri as f64);
        players.push(P {
            frames: s.map(|i| slot_list[i].frames.clone()).unwrap_or_default(),
            last_gp: s.map(|i| slot_list[i].last_gp).unwrap_or(-1),
            side: (team % 2.0) as i64,
            respawns: 0.0,
            life_slots: s.map(|i| vec![slot_list[i].slot_id]).unwrap_or_default(),
            lives: s.map(|i| vec![i]).unwrap_or_default(),
        });
    }

    // Spawn sides of the seeded initial lives: the c1 or c2 split with the
    // widest gap, oriented by majority vote of their teams.
    let seeded: Vec<&S> = slot_list.iter().filter(|s| seeded_ids.contains(&s.slot_id)).collect();
    let gap_axis = |vals: Vec<f64>| -> (f64, Option<f64>) {
        let mut s = vals;
        s.sort_by(f64::total_cmp);
        let (mut g, mut m) = (0.0f64, s.first().copied());
        for i in 1..s.len() {
            if s[i] - s[i - 1] > g {
                g = s[i] - s[i - 1];
                m = Some((s[i] + s[i - 1]) / 2.0);
            }
        }
        (g, m)
    };
    let a1 = gap_axis(seeded.iter().map(|s| s.first_c1).collect());
    let a2 = gap_axis(seeded.iter().map(|s| s.frames[0].c2).collect());
    let use_c1 = a1.0 >= a2.0;
    let lt = |v: f64, mid: Option<f64>| mid.is_some_and(|m| v < m);
    let coord_side_of = |s: &S| -> i64 {
        if if use_c1 { lt(s.first_c1, a1.1) } else { lt(s.frames[0].c2, a2.1) } {
            0
        } else {
            1
        }
    };
    let agree = seeded.iter().filter(|s| coord_side_of(s) == players[s.slot_id as usize].side).count();
    let flip = agree < seeded.len() - agree;
    let side_of = |s: &S| if flip { 1 - coord_side_of(s) } else { coord_side_of(s) };

    let mut unlinked: Vec<Vec<RawSample>> = Vec::new();
    for (idx, s) in slot_list.iter().enumerate() {
        if seeded_ids.contains(&s.slot_id) {
            continue;
        }
        let side = side_of(s);
        let mut pick: Option<usize> = None;
        let mut pick_gp = f64::INFINITY;
        for (ri, st) in players.iter().enumerate() {
            if is_bot_at(ri) != s.ai || st.side != side || st.last_gp >= s.first_gp {
                continue;
            }
            if st.respawns >= max_respawns(&roster[ri]) {
                continue;
            }
            if (st.last_gp as f64) < pick_gp {
                pick = Some(ri);
                pick_gp = st.last_gp as f64;
            }
        }
        if pick.is_none() {
            for (ri, st) in players.iter().enumerate() {
                if is_bot_at(ri) != s.ai || st.last_gp >= s.first_gp {
                    continue;
                }
                if st.respawns >= max_respawns(&roster[ri]) {
                    continue;
                }
                if (st.last_gp as f64) < pick_gp {
                    pick = Some(ri);
                    pick_gp = st.last_gp as f64;
                }
            }
        }
        match pick {
            Some(p) => {
                let st = &mut players[p];
                st.frames.extend_from_slice(&s.frames);
                st.last_gp = s.last_gp;
                st.respawns += 1.0;
                st.life_slots.push(s.slot_id);
                st.lives.push(idx);
            }
            None => unlinked.push(s.frames.clone()),
        }
    }
    let paths = players
        .into_iter()
        .enumerate()
        .map(|(ri, st)| {
            let positions = sort_to_motion_points(st.frames);
            let lives: Vec<&Vec<RawSample>> = st.lives.iter().map(|&i| &slot_list[i].frames).collect();
            let life_starts = life_start_indexes(&positions, &lives);
            PlayerPath {
                player_index: ri as i32,
                is_bot: is_bot_at(ri),
                team_id: roster[ri].last_team_id,
                positions,
                life_slots: Some(st.life_slots),
                life_starts: Some(life_starts),
            }
        })
        .collect();
    (paths, unlinked)
}

/// The film's median on-foot speed in raw units per frame ("a walk"). Slots
/// must be sorted by frame. None when nothing moves.
fn median_walk(slots: &Slots) -> Option<f64> {
    let mut speeds: Vec<f64> = Vec::new();
    for s in slots.values() {
        for i in 1..s.len() {
            if s[i].in_vehicle || s[i - 1].in_vehicle {
                continue;
            }
            let dt = s[i].tick - s[i - 1].tick;
            if !(1..=3).contains(&dt) {
                continue;
            }
            let v = f64::hypot(s[i].c1 - s[i - 1].c1, s[i].c2 - s[i - 1].c2) / dt as f64;
            if v >= 0.5 {
                speeds.push(v);
            }
        }
    }
    if speeds.is_empty() {
        return None;
    }
    speeds.sort_by(f64::total_cmp);
    Some(f64::max(1.0, speeds[speeds.len() >> 1]))
}

fn sorted_unique(ticks: &[i64]) -> Vec<i64> {
    let mut seen = HashSet::new();
    let mut v: Vec<i64> = ticks.iter().copied().filter(|t| seen.insert(*t)).collect();
    v.sort();
    v
}

/// A stationary stretch: the samples either side are within this many walks.
const HOLD_MAX_WALKS: f64 = 3.0;

/// Fill a life's stationary stretches: between two on-foot samples 3 walks or less
/// apart, every stub frame gets a copy of the earlier position (`held`).
fn hold_through_stubs(slots: &mut Slots, stubs: &IndexMap<i32, Vec<i64>>, tick_gp: &[Option<i64>]) {
    for s in slots.values_mut() {
        s.sort_by_key(|x| x.tick);
    }
    let Some(walk) = median_walk(slots) else { return };
    let max_step = HOLD_MAX_WALKS * walk;
    for (slot_id, stub_ticks) in stubs {
        let Some(s) = slots.get_mut(slot_id) else { continue };
        if s.len() < 2 {
            continue;
        }
        let st = sorted_unique(stub_ticks);
        let mut held: Vec<RawSample> = Vec::new();
        let mut k = 0usize;
        for i in 1..s.len() {
            let (a, b) = (s[i - 1], s[i]);
            if a.in_vehicle || b.in_vehicle || b.tick - a.tick < 2 {
                continue;
            }
            while k < st.len() && st[k] <= a.tick {
                k += 1;
            }
            if k >= st.len() || st[k] >= b.tick {
                continue;
            }
            if f64::hypot(b.c1 - a.c1, b.c2 - a.c2) > max_step {
                continue;
            }
            let mut j = k;
            while j < st.len() && st[j] < b.tick {
                if let Some(gp) = tick_gp.get(st[j] as usize).copied().flatten() {
                    let mut h = RawSample::new(gp, st[j], a.c1, a.c2);
                    h.held = true;
                    held.push(h);
                }
                j += 1;
            }
        }
        if !held.is_empty() {
            s.extend(held);
            s.sort_by_key(|x| x.tick);
        }
    }
}

const RIDE_MIN_STUBS: usize = 3;
/// Stubs run on through the dismount after an exit.
const EXIT_STUB_FRAMES: i64 = 60;
const RIDE_BOARD_TOL: f64 = 40.0; // walks
const RIDE_DISMOUNT_TOL: f64 = 40.0; // walks
const RIDE_BOARD_MAX_SPEED: f64 = 1.5; // walks per frame, rides without a dismount
const RIDE_BOARD_LOOKAHEAD: i64 = 30; // frames after the first stub

/// Index of the last sample with tick at most t, or -1.
fn last_at_or_before(a: &[RawSample], t: i64) -> i64 {
    let (mut lo, mut hi, mut ans) = (0i64, a.len() as i64 - 1, -1i64);
    while lo <= hi {
        let m = (lo + hi) >> 1;
        if a[m as usize].tick <= t {
            ans = m;
            lo = m + 1;
        } else {
            hi = m - 1;
        }
    }
    ans
}

/// Index of the first sample with tick at least t, or -1.
fn first_at_or_after(a: &[RawSample], t: i64) -> i64 {
    let (mut lo, mut hi, mut ans) = (0i64, a.len() as i64 - 1, -1i64);
    while lo <= hi {
        let m = (lo + hi) >> 1;
        if a[m as usize].tick >= t {
            ans = m;
            hi = m - 1;
        } else {
            lo = m + 1;
        }
    }
    ans
}

fn dist(a: &RawSample, b: &RawSample) -> f64 {
    f64::hypot(a.c1 - b.c1, a.c2 - b.c2)
}

/// A life leaving a vehicle: unit_exit_vehicle names both, and the frame.
struct Exit {
    life: i32,
    vehicle: i32,
    tick: i64,
}

fn read_exits(film: &Film, widths: [u32; 3]) -> Vec<Exit> {
    let mut out = Vec::new();
    for_each_event_view(film, widths, |frame, evs| {
        for e in evs {
            if let (Body::Exit { .. }, Some(life), Some(vehicle)) =
                (&e.body, e.handles[0].and_then(|h| h.biped_slot()), e.handles[1].and_then(|h| h.vehicle_slot()))
            {
                out.push(Exit { life, vehicle, tick: frame as i64 });
            }
        }
    });
    out
}

/// Splice in every ride the parent state records (`attach.rs`): while a
/// life is attached to a vehicle it is in that vehicle's seat, so its samples
/// are the vehicle's positions (the vehicle that carries it, for a part),
/// from the frame it was attached to the frame it left. On-foot samples
/// inside a ride are dropped (an attached biped sends its position relative
/// to the vehicle, which the scanner doesn't take, so there are almost none).
/// A life that only ever rode (it spawned and boarded before sending a
/// position) gets a slot from its rides alone, unless the biped is AI: a
/// dropship's pilot is an AI biped in a seat that no player owns.
fn splice_attachments(slots: &mut Slots, vehicles: &IndexMap<i32, Vec<RawSample>>, at: &crate::attach::Attachments) {
    let sorted: IndexMap<i32, Vec<RawSample>> = vehicles
        .iter()
        .map(|(&id, v)| {
            let mut v = v.clone();
            v.sort_by_key(|s| s.tick);
            (id, v)
        })
        .collect();
    for s in at.bipeds.iter() {
        if !slots.contains_key(&s.id) && s.ai {
            continue;
        }
        let mut ride: Vec<RawSample> = Vec::new();
        // The carrying vehicle can change within a span only when a part or
        // a carried vehicle is set down; take it per sample.
        let mut ids: Vec<i32> = vec![s.vehicle];
        for f in [s.from, s.to] {
            let r = at.root(s.vehicle, f);
            if !ids.contains(&r) {
                ids.push(r);
            }
        }
        for id in ids {
            let Some(v) = sorted.get(&id) else { continue };
            let lo = v.partition_point(|x| x.tick < s.from);
            for x in v[lo..].iter().take_while(|x| x.tick <= s.to) {
                if at.root(s.vehicle, x.tick) != id {
                    continue;
                }
                let mut r = RawSample::new(x.gp, x.tick, x.c1, x.c2);
                r.in_vehicle = true;
                r.vehicle = id;
                ride.push(r);
            }
        }
        if ride.is_empty() {
            continue;
        }
        let foot = slots.entry(s.id).or_default();
        foot.retain(|x| x.in_vehicle || x.tick < s.from || x.tick > s.to);
        foot.extend(ride);
        foot.sort_by_key(|x| x.tick);
    }
}

/// Splice in the rides the game recorded. The exit gives the life, the vehicle
/// and the end; nothing records boarding, so a ride starts where that vehicle
/// comes closest to the life's last sample before the exit. Returns the stubs
/// no such ride covers, for `splice_vehicle_rides`, which finds the rides with
/// no exit: those that end in death.
fn splice_exits(
    slots: &mut Slots,
    stubs: &IndexMap<i32, Vec<i64>>,
    vehicles: &IndexMap<i32, Vec<RawSample>>,
    ident: &crate::vehicles::Vehicles,
    exits: &[Exit],
) -> IndexMap<i32, Vec<i64>> {
    let mut open = stubs.clone();
    for s in slots.values_mut() {
        s.sort_by_key(|x| x.tick);
    }
    for x in exits {
        let Some(foot) = slots.get_mut(&x.life) else { continue };
        // An exit from a part (a gunner seat) is a ride in its vehicle.
        let Some(v) = vehicles.get(&ident.resolve(x.vehicle)) else {
            continue;
        };
        let mut v = v.clone();
        v.sort_by_key(|s| s.tick);
        let ia = last_at_or_before(foot, x.tick - 1);
        let (from, anchor) = if ia >= 0 {
            (foot[ia as usize].tick + 1, Some(foot[ia as usize]))
        } else {
            (stubs.get(&x.life).and_then(|s| s.iter().min().copied()).unwrap_or(x.tick), None)
        };
        let near = |s: &RawSample| anchor.map_or(0.0, |a| dist(&a, s));
        let inside: Vec<&RawSample> = v.iter().filter(|s| s.tick >= from && s.tick <= x.tick).collect();
        let Some(board) = inside.iter().copied().min_by(|a, b| f64::total_cmp(&near(a), &near(b))) else { continue };
        let ride: Vec<RawSample> = inside
            .iter()
            .filter(|s| s.tick >= board.tick)
            .map(|s| {
                let mut r = RawSample::new(s.gp, s.tick, s.c1, s.c2);
                r.in_vehicle = true;
                r.vehicle = s.vehicle;
                r
            })
            .collect();
        foot.extend(ride);
        foot.sort_by_key(|s| s.tick);
        if let Some(o) = open.get_mut(&x.life) {
            o.retain(|&t| t < from || t > x.tick + EXIT_STUB_FRAMES);
        }
    }
    open
}

/// Splice vehicle positions into the lives that rode in them, per life-slot,
/// before lives are given to players. A ride starts at the first stub after
/// an on-foot sample (the anchor) and lasts until the slot's next on-foot
/// sample, or its last stub. The closest vehicle must board within 40 walks,
/// move 40 walks or more, and either dismount within 40 walks of the next on-foot
/// sample with the player ending up elsewhere, or (no next sample) have been
/// slower than 1.5 walks/frame when boarded.
fn splice_vehicle_rides(slots: &mut Slots, stubs: &IndexMap<i32, Vec<i64>>, vehicles: &IndexMap<i32, Vec<RawSample>>) {
    if vehicles.is_empty() || stubs.is_empty() {
        return;
    }
    for s in slots.values_mut() {
        s.sort_by_key(|x| x.tick);
    }
    let veh_list: Vec<Vec<RawSample>> = vehicles
        .values()
        .map(|v| {
            let mut v = v.clone();
            v.sort_by_key(|x| x.tick);
            v
        })
        .collect();
    let Some(walk) = median_walk(slots) else { return };
    let board_tol = RIDE_BOARD_TOL * walk;
    let dismount_tol = RIDE_DISMOUNT_TOL * walk;

    for (slot_id, stub_ticks) in stubs {
        let Some(foot) = slots.get_mut(slot_id) else { continue };
        if foot.is_empty() {
            continue;
        }
        let st = sorted_unique(stub_ticks);
        let mut added: Vec<RawSample> = Vec::new();
        let mut i = 0usize;
        while i < st.len() {
            let t0 = st[i];
            let ia = last_at_or_before(foot, t0);
            let inext = first_at_or_after(foot, t0 + 1);
            let next = if inext >= 0 { Some(foot[inext as usize]) } else { None };
            let mut j = i;
            while j + 1 < st.len() && next.is_none_or(|n| st[j + 1] < n.tick) {
                j += 1;
            }
            let t1 = st[j];
            let n_stubs = j - i + 1;
            i = j + 1;
            if ia < 0 || n_stubs < RIDE_MIN_STUBS {
                continue;
            }
            let anchor = foot[ia as usize];
            let t_lim = match next {
                Some(n) => n.tick - 1,
                None => t1,
            };

            struct Best<'v> {
                v: &'v Vec<RawSample>,
                k_board: i64,
                k_in: i64,
                d: f64,
            }
            let mut best: Option<Best> = None;
            for v in &veh_list {
                let k_in = first_at_or_after(v, t0);
                if k_in < 0 || v[k_in as usize].tick > t_lim {
                    continue;
                }
                let k_held = last_at_or_before(v, anchor.tick);
                let mut k_board = k_held;
                let mut d = if k_held >= 0 { dist(&anchor, &v[k_held as usize]) } else { f64::INFINITY };
                let t_board_max = (t0 + RIDE_BOARD_LOOKAHEAD).min(t_lim);
                let mut m = k_held + 1;
                while (m as usize) < v.len() && v[m as usize].tick <= t_board_max {
                    let dm = dist(&anchor, &v[m as usize]);
                    if dm < d {
                        d = dm;
                        k_board = m;
                    }
                    m += 1;
                }
                if k_board >= 0 && best.as_ref().is_none_or(|b| d < b.d) {
                    best = Some(Best { v, k_board, k_in, d });
                }
            }
            let Some(best) = best else { continue };
            if best.d > board_tol {
                continue;
            }
            let v = best.v;
            let board = v[best.k_board as usize];

            let mut k_exit = best.k_in.max(last_at_or_before(v, t1));
            if let Some(next) = next {
                let mut d_exit = f64::INFINITY;
                let mut m = k_exit;
                while (m as usize) < v.len() && v[m as usize].tick < next.tick {
                    let dm = dist(&next, &v[m as usize]);
                    if dm <= d_exit {
                        d_exit = dm;
                        k_exit = m;
                    }
                    m += 1;
                }
                if d_exit > dismount_tol || dist(&anchor, &next) < board_tol / 2.0 {
                    continue;
                }
            } else {
                let k0 = last_at_or_before(v, board.tick - 10);
                if k0 >= 0
                    && board.tick > v[k0 as usize].tick
                    && dist(&board, &v[k0 as usize]) / (board.tick - v[k0 as usize].tick) as f64
                        > RIDE_BOARD_MAX_SPEED * walk
                {
                    continue;
                }
            }

            let t_board = (anchor.tick + 1).max(board.tick);
            let t_exit = v[k_exit as usize].tick;
            let mut ride: Vec<RawSample> = Vec::new();
            let mut moved = 0.0f64;
            let mut m = first_at_or_after(v, t_board).max(0);
            while (m as usize) < v.len() && v[m as usize].tick <= t_exit {
                let vm = v[m as usize];
                m += 1;
                if vm.tick < t_board {
                    continue;
                }
                let mut r = RawSample::new(vm.gp, vm.tick, vm.c1, vm.c2);
                r.in_vehicle = true;
                r.vehicle = vm.vehicle;
                ride.push(r);
                moved = f64::max(moved, dist(&board, &vm));
            }
            if ride.is_empty() || moved < board_tol {
                continue;
            }
            added.extend(ride);
        }
        if !added.is_empty() {
            foot.extend(added);
            foot.sort_by_key(|x| x.tick);
        }
    }
}

// Small lobbies: per-entity and bot-composite.

/// One entity record: `[01][id:9][010][k:4][0000][(k - 1) 6-bit codes]`,
/// then the payload `0100000, c1, c2, ...`, at any bit offset of a frame.
struct EntityRecord<'a> {
    id: i32,
    tick: i64,
    gp: i64,
    bot: bool,
    chunk: &'a [u8],
    /// Absolute bit position of the payload in `chunk`.
    payload: i64,
}

/// Payload bits a record must have inside its frame body.
const RECORD_PAYLOAD_BITS: i64 = 41;

fn for_each_entity_record<'a>(film: &'a Film, f: &mut dyn FnMut(EntityRecord<'a>)) {
    for fr in film.frames() {
        let c = fr.chunk;
        let end = fr.end;
        let mut q = fr.body;
        while q + 3 < end {
            let w = ((c[q] as u32) << 24) | ((c[q + 1] as u32) << 16) | ((c[q + 2] as u32) << 8) | c[q + 3] as u32;
            for sh in 0..8u32 {
                let v = w << sh;
                if (v >> 30) != 1 || ((v >> 18) & 7) != 2 || ((v >> 10) & 15) != 0 {
                    continue;
                }
                let k = ((v >> 14) & 15) as i64;
                if !(2..=7).contains(&k) {
                    continue;
                }
                let b = q as i64 * 8 + sh as i64;
                let h = b + 22 + 6 * (k - 1);
                if h + RECORD_PAYLOAD_BITS > end as i64 * 8 {
                    continue;
                }
                let (mut prev, mut has6, mut extra, mut bot, mut ok) = (-1i64, false, false, false, true);
                let mut i = 0;
                while i < k - 1 && ok {
                    let code = bits_at(c, b + 22 + 6 * i, 6) as i64;
                    let group = code & 15;
                    if (i == 0 && code >= 16) || group < prev {
                        ok = false;
                    }
                    prev = group;
                    if group == 6 {
                        has6 = true;
                    }
                    if group > 6 {
                        extra = true;
                    }
                    if code >= 48 {
                        bot = true;
                    }
                    i += 1;
                }
                if !ok || !has6 {
                    continue;
                }
                // Payload opens 0100000; with a group above 6 in the list its
                // first two bits vary.
                let head = bits_at(c, h, 7);
                if if extra { (head & 0x1f) != 0 } else { head != 0x20 } {
                    continue;
                }
                f(EntityRecord {
                    id: ((v >> 21) & 0x1ff) as i32,
                    tick: fr.index as i64,
                    gp: fr.gp,
                    bot,
                    chunk: c,
                    payload: h,
                });
            }
            q += 1;
        }
    }
}

/// c2 start bit for small-lobby records, by the same smoothness test as
/// `detect_coord_layout`. Falls back to 20 (Aquarius).
fn detect_record_seam(records: &[EntityRecord]) -> i64 {
    const CANDS: [i64; 8] = [19, 20, 21, 22, 23, 24, 25, 26];
    let sample = &records[..records.len().min(2000)];
    let mut best = 20;
    let mut best_score = f64::INFINITY;
    for cand in CANDS {
        let mut by_id: HashMap<i32, (u32, Vec<f64>)> = HashMap::new();
        for r in sample {
            let v = bits_at(r.chunk, r.payload + cand, 15);
            match by_id.get_mut(&r.id) {
                None => {
                    by_id.insert(r.id, (v, Vec::new()));
                }
                Some(s) => {
                    s.1.push((v as f64 - s.0 as f64).abs());
                    s.0 = v;
                }
            }
        }
        let mut p95s: Vec<f64> = Vec::new();
        for s in by_id.values_mut() {
            if s.1.len() < 8 {
                continue;
            }
            s.1.sort_by(f64::total_cmp);
            p95s.push(s.1[(s.1.len() as f64 * 0.95).floor() as usize]);
        }
        if p95s.is_empty() {
            continue;
        }
        p95s.sort_by(f64::total_cmp);
        let median = p95s[p95s.len() >> 1];
        if median < best_score {
            best_score = median;
            best = cand;
        }
    }
    best
}

struct SmallLobbyLife {
    id: i32,
    samples: Vec<RawSample>,
    bot: bool,
    first_tick: i64,
    last_tick: i64,
}

/// Every small-lobby entity record decoded into per-life samples, in
/// small-lobby units: raw1 = 0x4000 + c1's 15-bit window / 64 (c2's bits
/// below c1 masked off), raw2 = the window at the seam >> 3.
fn extract_small_lobby_lives(film: &Film) -> (Vec<SmallLobbyLife>, i64) {
    let mut records: Vec<EntityRecord> = Vec::new();
    for_each_entity_record(film, &mut |r| records.push(r));
    let c2_start = detect_record_seam(&records);
    let c2_in_c1 = 0.max(15 - (c2_start - 7));
    struct L {
        samples: Vec<RawSample>,
        last_tick: i64,
        records: usize,
        bot: usize,
    }
    let mut by_id: IndexMap<i32, L> = IndexMap::new();
    for r in &records {
        // Ids 256-511 are non-biped objects.
        if r.id >= 256 {
            continue;
        }
        let life = by_id.entry(r.id).or_insert(L { samples: Vec::new(), last_tick: -1, records: 0, bot: 0 });
        life.records += 1;
        if r.bot {
            life.bot += 1;
        }
        if r.tick == life.last_tick {
            continue;
        }
        life.last_tick = r.tick;
        let w1 = bits_at(r.chunk, r.payload + 7, 15) as i64;
        let c1 = 0x4000 as f64 + (((w1 >> c2_in_c1) << c2_in_c1) as f64) / 64.0;
        let c2 = (bits_at(r.chunk, r.payload + c2_start, 15) >> 3) as f64;
        life.samples.push(RawSample::new(r.gp, r.tick, c1, c2));
    }
    let mut lives: Vec<SmallLobbyLife> = by_id
        .into_iter()
        .map(|(id, l)| SmallLobbyLife {
            id,
            bot: l.bot * 2 > l.records,
            first_tick: l.samples[0].tick,
            last_tick: l.samples[l.samples.len() - 1].tick,
            samples: l.samples,
        })
        .collect();
    lives.sort_by_key(|l| l.id);
    (lives, c2_start)
}

/// Small-lobby lives to roster players: each type (human or bot by the code
/// vote) to its own roster entries in slot order; respawns FIFO to the
/// same-type player whose last life ended first, capped at spawns minus 1.
/// Each linked respawn is a death at the previous life's last sample.
fn stitch_small_lobby(
    lives: &[SmallLobbyLife],
    roster: Option<&[RosterEntry]>,
    mut deaths: Option<&mut Vec<DeathPosition>>,
) -> Vec<PlayerPath> {
    let roster = match roster {
        Some(r) if !r.is_empty() => r,
        _ => {
            return lives
                .iter()
                .filter(|l| l.samples.len() >= MIN_PLAYER_FRAMES)
                .enumerate()
                .map(|(i, l)| PlayerPath {
                    player_index: i as i32,
                    is_bot: l.bot,
                    team_id: None,
                    positions: sort_to_motion_points(l.samples.clone()),
                    life_slots: None,
                    life_starts: None,
                })
                .collect();
        }
    };
    let n = roster.len();
    let is_bot_entry = |ri: usize| roster[ri].is_bot();
    let initial: Vec<usize> = (0..lives.len()).filter(|&i| (lives[i].id as usize) < n).collect();
    // owner: (life index, roster index), in insertion order.
    let mut owner: Vec<(usize, usize)> = Vec::new();

    let humans_r: Vec<usize> = (0..n).filter(|&ri| !is_bot_entry(ri)).collect();
    let bots_r: Vec<usize> = (0..n).filter(|&ri| is_bot_entry(ri)).collect();
    let by_slot: HashMap<i32, usize> = initial.iter().map(|&i| (lives[i].id, i)).collect();
    let n_human_l = initial.iter().filter(|&&i| !lives[i].bot).count();
    let n_missing = n as i64 - initial.len() as i64;
    let missing_bot = n_missing > 0 && bots_r.len() as i64 - (initial.len() - n_human_l) as i64 == n_missing;
    let missing_human = n_missing > 0 && humans_r.len() as i64 - n_human_l as i64 == n_missing;
    let (mut hi, mut bi) = (0usize, 0usize);
    for id in 0..n {
        let l = by_slot.get(&(id as i32)).copied();
        let bot = match l {
            Some(li) => Some(lives[li].bot),
            None if missing_bot => Some(true),
            None if missing_human => Some(false),
            None => None,
        };
        let Some(bot) = bot else { continue };
        let ri = if bot {
            bi += 1;
            bots_r.get(bi - 1).copied()
        } else {
            hi += 1;
            humans_r.get(hi - 1).copied()
        };
        if let (Some(li), Some(ri)) = (l, ri) {
            owner.push((li, ri));
        }
    }
    // Initial lives the type split could not place.
    for &li in &initial {
        if owner.iter().any(|&(l, _)| l == li) {
            continue;
        }
        let free: Vec<usize> = (0..n).filter(|ri| !owner.iter().any(|&(_, r)| r == *ri)).collect();
        let Some(&ri) = free.first() else { break };
        owner.push((li, ri));
    }

    struct P {
        lives: Vec<usize>,
        last_tick: i64,
        respawns: f64,
    }
    let mut players: Vec<P> = (0..n).map(|_| P { lives: Vec::new(), last_tick: -1, respawns: 0.0 }).collect();
    for &(li, ri) in &owner {
        players[ri].lives.push(li);
        players[ri].last_tick = lives[li].last_tick;
    }
    let total_spawns = roster_spawns(Some(roster)).unwrap_or(f64::INFINITY);
    let mut respawns: Vec<usize> = (0..lives.len())
        .filter(|&i| {
            let l = &lives[i];
            l.id as usize >= n && (l.id as f64) < total_spawns && l.samples.len() >= MIN_PLAYER_FRAMES
        })
        .collect();
    respawns.sort_by_key(|&i| lives[i].first_tick);
    for li in respawns {
        let l = &lives[li];
        let mut pick: Option<usize> = None;
        for same_type in [true, false] {
            let mut pick_tick = f64::INFINITY;
            for (ri, st) in players.iter().enumerate() {
                if same_type && is_bot_entry(ri) != l.bot {
                    continue;
                }
                if st.last_tick >= l.first_tick || st.respawns >= max_respawns(&roster[ri]) {
                    continue;
                }
                if (st.last_tick as f64) < pick_tick {
                    pick = Some(ri);
                    pick_tick = st.last_tick as f64;
                }
            }
            if pick.is_some() {
                break;
            }
        }
        let Some(p) = pick else {
            continue;
        };
        let st = &mut players[p];
        if let Some(&prev) = st.lives.last() {
            if let (Some(d), Some(end)) = (deaths.as_deref_mut(), lives[prev].samples.last()) {
                d.push(DeathPosition { slot_id: lives[prev].id, gp: end.gp });
            }
        }
        st.lives.push(li);
        st.last_tick = l.last_tick;
        st.respawns += 1.0;
    }
    let mut out = Vec::new();
    for (ri, st) in players.iter().enumerate() {
        let samples: Vec<RawSample> = st.lives.iter().flat_map(|&li| lives[li].samples.iter().copied()).collect();
        if samples.is_empty() {
            continue;
        }
        out.push(PlayerPath {
            player_index: ri as i32,
            is_bot: is_bot_entry(ri),
            team_id: roster[ri].last_team_id,
            positions: sort_to_motion_points(samples),
            life_slots: None,
            life_starts: None,
        });
    }
    out
}

// Lives to players from HighlightEvents.

/// The roster and its HighlightEvents deaths as life players. None when any
/// entry lacks its spawns or deaths from the scoreboard.
fn life_players_of(roster: &[RosterEntry], highlights: &[HighlightEvent], xuids: &[String]) -> Option<Vec<LifePlayer>> {
    let mut deaths_by_xuid: HashMap<&str, Vec<f64>> = HashMap::new();
    for e in highlights {
        if e.event_type == EventType::Death {
            deaths_by_xuid.entry(e.xuid.as_str()).or_default().push(e.time_ms);
        }
    }
    let mut out = Vec::new();
    for (i, r) in roster.iter().enumerate() {
        let (spawns, deaths) = (r.spawns?, r.deaths?);
        let is_bot = r.is_bot();
        let xuid = r.xuid.clone().or_else(|| xuids.get(i).cloned());
        let times = match (&xuid, is_bot) {
            (Some(x), false) if !x.is_empty() => {
                let mut t = deaths_by_xuid.get(x.as_str()).cloned().unwrap_or_default();
                t.sort_by(f64::total_cmp);
                Some(t)
            }
            _ => None,
        };
        out.push(LifePlayer {
            index: i,
            is_bot,
            team: r.last_team_id,
            spawns,
            deaths,
            present_at_start: r.present_at_beginning.unwrap_or(true),
            left: r.left_in_progress.unwrap_or(false),
            death_ms: times.filter(|t| t.len() as f64 == deaths),
        });
    }
    Some(out)
}

/// One life slot per biped slot, timed by its on-foot records. With
/// `rides_recorded` (the parent state was read) a life's rides count as its
/// records: it is alive and seated through them, and a life killed in a
/// vehicle ends there. Inferred rides don't.
fn life_slots_of(slots: &Slots, game_ticks: &[u32], rides_recorded: bool) -> Vec<LifeSlot> {
    let mut out = Vec::new();
    for (&slot_id, frames) in slots {
        let foot: Vec<&RawSample> = frames.iter().filter(|f| rides_recorded || !f.in_vehicle).collect();
        if foot.is_empty() {
            continue;
        }
        let mut first = foot[0];
        let mut last = foot[0];
        for f in &foot {
            if f.tick < first.tick {
                first = f;
            }
            if f.tick > last.tick {
                last = f;
            }
        }
        out.push(LifeSlot {
            slot_id,
            is_bot: is_ai_slot(frames),
            start: game_ticks[first.tick as usize] as f64,
            end: game_ticks[last.tick as usize] as f64,
            x: Some(first.c1),
            y: Some(first.c2),
        });
    }
    out
}

/// Paths, deaths and leftovers from a life assignment. A death goes at the
/// life's last sample up to the HighlightEvents death tick plus `end_late_max`; bots'
/// deaths at their last on-foot record. A timed life's path ends there too: a
/// vehicle splice can run on after its rider died (the slot's stubs outlive
/// the death) and would otherwise overlap the owner's next life.
fn paths_from_assignment(
    slots: &Slots,
    roster: &[RosterEntry],
    a: &LifeAssignment,
    game_ticks: &[u32],
) -> (Vec<PlayerPath>, Vec<Vec<RawSample>>, Vec<DeathPosition>) {
    struct L<'s> {
        life: usize,
        slot_id: i32,
        frames: &'s Vec<RawSample>,
        /// Game tick past which the life's samples are dropped.
        limit: f64,
    }
    let mut per_player: Vec<Vec<L>> = roster.iter().map(|_| Vec::new()).collect();
    let mut unlinked: Vec<Vec<RawSample>> = Vec::new();
    let mut deaths: Vec<DeathPosition> = Vec::new();
    for (&slot_id, frames) in slots {
        let Some(&(player, life)) = a.owner.get(&slot_id) else {
            if frames.len() >= 3 {
                unlinked.push(frames.clone());
            }
            continue;
        };
        let time_ms = a.deaths.get(&slot_id).copied();
        let limit = match time_ms {
            Some(Some(ms)) => ms * TICKS_PER_MS + a.clock.tick_offset + LIFE_TUNING.end_late_max,
            _ => f64::INFINITY,
        };
        per_player[player].push(L { life, slot_id, frames, limit });
        let Some(time_ms) = time_ms else { continue };
        let mut last: Option<&RawSample> = None;
        for f in frames {
            if time_ms.is_none() && f.in_vehicle {
                continue;
            }
            if game_ticks[f.tick as usize] as f64 > limit {
                continue;
            }
            if last.is_none_or(|l| f.tick > l.tick || (f.tick == l.tick && f.gp > l.gp)) {
                last = Some(f);
            }
        }
        let last = match last {
            Some(l) => *l,
            None => {
                let mut foot: Vec<&RawSample> = frames.iter().filter(|f| !f.in_vehicle).collect();
                if foot.is_empty() {
                    // A life seen only riding.
                    foot = frames.iter().collect();
                }
                let mut x = foot[0];
                for f in &foot {
                    if f.gp > x.gp {
                        x = f;
                    }
                }
                *x
            }
        };
        deaths.push(DeathPosition { slot_id, gp: last.gp });
    }
    deaths.sort_by_key(|d| d.gp);
    let paths = roster
        .iter()
        .enumerate()
        .map(|(ri, r)| {
            let lives = &mut per_player[ri];
            lives.sort_by_key(|l| l.life);
            let samples: Vec<RawSample> = lives
                .iter()
                .flat_map(|l| l.frames.iter().filter(|f| game_ticks[f.tick as usize] as f64 <= l.limit).copied())
                .collect();
            let positions = sort_to_motion_points(samples);
            let frames: Vec<&Vec<RawSample>> = lives.iter().map(|l| l.frames).collect();
            let life_starts = life_start_indexes(&positions, &frames);
            PlayerPath {
                player_index: ri as i32,
                is_bot: r.is_bot(),
                team_id: r.last_team_id,
                positions,
                life_slots: Some(lives.iter().map(|l| l.slot_id).collect()),
                life_starts: Some(life_starts),
            }
        })
        .collect();
    (paths, unlinked, deaths)
}

pub struct ExtractionResult {
    pub paths: Vec<PlayerPath>,
    /// The detected c2 start bit (7 plus c1's field width).
    pub coord_layout: i64,
    /// multiplayer-composite: c2's detected width (`detect_c2_width`).
    pub c2_width: Option<u32>,
    /// multiplayer-composite: one death per life that ended in one.
    pub deaths: Option<Vec<DeathPosition>>,
    /// multiplayer-composite: lives no roster player could be given.
    pub unassigned: Vec<PlayerPath>,
    pub frame_ticks: Vec<u32>,
    /// The game tick of HighlightEvents time 0.
    pub hl_zero_tick: Option<f64>,
    /// multiplayer-composite: each vehicle's position records by vehicle id,
    /// each sample's `vehicle` its id.
    pub vehicles: IndexMap<i32, Vec<RawSample>>,
    /// multiplayer-composite: vehicle tags by id, and parts' vehicles.
    pub vehicle_ident: crate::vehicles::Vehicles,
    /// multiplayer-composite: who was attached to which vehicle (`attach.rs`).
    pub attachments: crate::attach::Attachments,
}

/// The film's encoding mode and, in a multiplayer-composite film, its c2
/// start bit and c2 width.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub mode: EncodingMode,
    pub composite: Option<(i32, u32)>,
}

pub fn detect_layout(film: &Film, n_players: Option<usize>) -> Layout {
    let mode = detect_encoding_mode(film, n_players);
    let composite = (mode == EncodingMode::MultiplayerComposite).then(|| {
        let c2_start = detect_coord_layout(film);
        (c2_start, detect_c2_width(film, c2_start))
    });
    Layout { mode, composite }
}

pub fn extract_all_player_positions(
    film: &Film,
    layout: Layout,
    roster: Option<&[RosterEntry]>,
    highlights: &[HighlightEvent],
    xuids: &[String],
    level: Option<&crate::levels::LevelBox>,
    vehicle_ident: Option<crate::vehicles::Vehicles>,
    attachments: Option<crate::attach::Attachments>,
) -> ExtractionResult {
    let c1_at = crate::levels::c1_at(level);
    let n_players = roster.map(|r| r.len());

    if let Some((coord_layout, c2_width)) = layout.composite {
        let ex = extract_multiplayer_composite(film, c1_at, coord_layout, c2_width);
        let mut slots = ex.slots;
        let widths = crate::assemble::position_widths(film, true, coord_layout as i64, Some(c2_width), level);
        let exits = widths.map_or(Vec::new(), |w| read_exits(film, w));
        let mut vehicles = ex.vehicles.clone();
        for (id, v) in vehicles.iter_mut() {
            v.iter_mut().for_each(|s| s.vehicle = *id);
        }
        let ident = vehicle_ident.unwrap_or_else(|| {
            let Some(w) = widths else { return crate::vehicles::Vehicles::default() };
            let mut params = crate::components::Params::new(w);
            params.set_level(level);
            crate::vehicles::Vehicles::read(film, params)
        });
        let attachments = attachments.unwrap_or_default();
        if attachments.read {
            // The parent state names every ride; the exits and the distance
            // rule are for films where it can't be read.
            splice_attachments(&mut slots, &vehicles, &attachments);
        } else {
            let open_stubs = splice_exits(&mut slots, &ex.stubs, &vehicles, &ident, &exits);
            splice_vehicle_rides(&mut slots, &open_stubs, &vehicles);
        }
        hold_through_stubs(&mut slots, &ex.stubs, &ex.tick_gp);

        let life_players = match roster {
            Some(r) if !r.is_empty() && !highlights.is_empty() => life_players_of(r, highlights, xuids),
            _ => None,
        };
        let hl_ticks = crate::lives::highlight_ticks(&ex.game_ticks);
        let assignment =
            life_players.and_then(|lp| assign_lives(&life_slots_of(&slots, &hl_ticks, attachments.read), &lp));

        let (paths, unlinked, deaths);
        match (&assignment, roster) {
            (Some(a), Some(r)) => {
                let (p, u, d) = paths_from_assignment(&slots, r, a, &hl_ticks);
                paths = p;
                unlinked = u;
                deaths = d;
            }
            _ => {
                let (p, u) = stitch_slots_to_players(&mut slots, roster);
                paths = p;
                unlinked = u;
                deaths = match n_players {
                    Some(n) if n > 0 => slot_end_deaths(&slots, n, roster),
                    _ => Vec::new(),
                };
            }
        }
        // Non-roster bipeds (Firefight AI) are nobody's lives.
        let unassigned: Vec<PlayerPath> = if has_non_roster_bipeds(slots.len(), roster) {
            Vec::new()
        } else {
            unlinked
                .into_iter()
                .map(|frames| PlayerPath {
                    player_index: -1,
                    is_bot: is_ai_slot(&frames),
                    team_id: None,
                    positions: sort_to_motion_points(frames),
                    life_slots: None,
                    life_starts: None,
                })
                .collect()
        };
        let hl_zero_tick = assignment.as_ref().map(|a| f64::round(a.clock.tick_offset));
        return ExtractionResult {
            paths,
            coord_layout: coord_layout as i64,
            c2_width: Some(c2_width),
            deaths: Some(deaths),
            unassigned,
            frame_ticks: ex.game_ticks,
            hl_zero_tick,
            vehicles,
            vehicle_ident: ident,
            attachments,
        };
    }

    let (lives, c2_start) = extract_small_lobby_lives(film);
    let paths = stitch_small_lobby(&lives, roster, None);
    ExtractionResult {
        paths,
        coord_layout: c2_start,
        c2_width: None,
        deaths: None,
        unassigned: Vec::new(),
        frame_ticks: film.frame_ticks().to_vec(),
        hl_zero_tick: None,
        vehicles: IndexMap::new(),
        vehicle_ident: Default::default(),
        attachments: Default::default(),
    }
}

/// Small-lobby deaths: each linked respawn ends the previous life.
pub fn small_lobby_deaths(film: &Film, n_players: usize, roster: Option<&[RosterEntry]>) -> Vec<DeathPosition> {
    if n_players < 1 {
        return Vec::new();
    }
    let mut deaths = Vec::new();
    let (lives, _) = extract_small_lobby_lives(film);
    stitch_small_lobby(&lives, roster, Some(&mut deaths));
    deaths.sort_by_key(|d| d.gp);
    deaths
}
