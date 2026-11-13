//! Life assignment for multiplayer-composite films: which roster player owns
//! each per-life biped slot, and which lives ended in a death.
//!
//! The HighlightEvents chunk times every human death per XUID. A life's last
//! record comes about at its owner's death, its first about the respawn
//! delay R after the previous death, on the game-tick clock
//! (`tick = time_ms * TICKS_PER_MS + tick_offset`). Slots are matched to
//! (player, life k) positions by min-cost bipartite matching, solved exactly
//! per connected component (Hungarian). Bots have no XUID and are chained by
//! time instead.

use indexmap::IndexMap;
use std::collections::HashMap;

/// Game ticks per millisecond of HighlightEvents time: 59.94 a second, not
/// 60.
pub const TICKS_PER_MS: f64 = 0.05994;
/// A jump of the tick counter this far over its usual step is a recorder
/// hitch: HighlightEvents time doesn't advance across it.
const HITCH_TICKS: u32 = 6;

/// The film's hitches (the tick on the highlight clock, ticks it skipped),
/// oldest first.
pub fn hitches(game_ticks: &[u32]) -> Vec<(u32, u32)> {
    let mut counts = std::collections::BTreeMap::new();
    for w in game_ticks.windows(2) {
        *counts.entry(w[1] - w[0]).or_insert(0u32) += 1;
    }
    let Some(usual) = counts.iter().max_by_key(|(d, c)| (**c, std::cmp::Reverse(**d))).map(|x| *x.0) else {
        return Vec::new();
    };
    let mut skipped = 0;
    let mut out = Vec::new();
    for w in game_ticks.windows(2) {
        let extra = (w[1] - w[0]).saturating_sub(usual);
        if extra >= HITCH_TICKS {
            out.push((w[0] - skipped, extra));
            skipped += extra;
        }
    }
    out
}

/// The clock HighlightEvents time runs on, in ticks for each frame.
///
/// Its times are the server's milliseconds, which the packets' timestamps keep
/// (a death's record falls within a tick of its kill's packet on every
/// reference film), so with `times_us`, each frame's timestamp, it is the first
/// frame's tick plus the time since, in ticks. A server that falls behind runs
/// fewer ticks than time: a 34-minute minigame ran 29 minutes of ticks. Without
/// timestamps it is `game_ticks` without the ticks recorder hitches skipped.
pub fn highlight_ticks(game_ticks: &[u32], times_us: Option<&[u64]>) -> Vec<u32> {
    match times_us {
        Some(us) if us.len() == game_ticks.len() && !us.is_empty() => {
            let (t0, us0) = (game_ticks[0] as f64, us[0]);
            let mut last = 0u32;
            us.iter()
                .map(|&u| {
                    let t = (t0 + u.saturating_sub(us0) as f64 / 1000.0 * TICKS_PER_MS).round() as u32;
                    last = last.max(t);
                    last
                })
                .collect()
        }
        _ => tick_clock(game_ticks),
    }
}

/// `game_ticks` without the ticks hitches skipped.
fn tick_clock(game_ticks: &[u32]) -> Vec<u32> {
    let steps = hitches(game_ticks);
    let mut at = 0;
    let mut skipped = 0;
    game_ticks
        .iter()
        .map(|&t| {
            while at < steps.len() && t - skipped > steps[at].0 {
                skipped += steps[at].1;
                at += 1;
            }
            t - skipped
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct LifeSlot {
    pub slot_id: i32,
    pub is_bot: bool,
    /// The roster index of the player the film names as the life's owner,
    /// when it does: the life can only be theirs.
    pub player: Option<usize>,
    pub start: f64,
    pub end: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// The tick the player controlling the life let go of it, which a death
    /// does (not the release of every life at the film's end); None when the
    /// film doesn't say.
    pub released: Option<f64>,
}

/// Ticks before a player's first listing that their first life may start.
/// A human is listed when they join and spawns later; a bot can be listed
/// after it spawns, by up to 463 ticks on the test films.
const LISTED_EARLY_TICKS: f64 = 120.0;
const BOT_LISTED_EARLY_TICKS: f64 = 600.0;

#[derive(Clone, Debug)]
pub struct LifePlayer {
    pub index: usize,
    pub is_bot: bool,
    pub team: Option<f64>,
    pub spawns: f64,
    pub deaths: f64,
    pub present_at_start: bool,
    pub left: bool,
    pub death_ms: Option<Vec<f64>>,
    /// An uncounted bot's listing span (`RosterEntry::listed`).
    pub listed: Option<(f64, f64)>,
    /// When the player's lives can start (`RosterEntry::in_match`).
    pub in_match: Option<(f64, f64)>,
    /// The deaths the scoreboard counts, each as the earliest tick it can be
    /// at and the tick it is counted at (`RosterEntry::death_frames`); None
    /// without a scoreboard.
    pub death_ticks: Option<Vec<(f64, f64)>>,
}

impl LifePlayer {
    /// Whether a life starting at `start` can be the player's: not before
    /// they joined, nor after they left.
    fn can_start(&self, start: f64) -> bool {
        let early = if self.is_bot { BOT_LISTED_EARLY_TICKS } else { LISTED_EARLY_TICKS };
        self.in_match.is_none_or(|(a, b)| start >= a - early && start <= b)
    }

    /// Whether letting go of a life at `released` was a death: the
    /// scoreboard counts one then, when there is a scoreboard. A bot removed
    /// to make room for a joiner lets go of its life with no death counted,
    /// as does everyone the end of the match kills.
    fn died_at(&self, released: Option<f64>) -> bool {
        released.is_some_and(|r| {
            let m = COUNTED_DEATH_TICKS;
            self.death_ticks.as_ref().is_none_or(|d| d.iter().any(|&(a, b)| r >= a - m && r <= b + m))
        })
    }
}

/// How far apart a release and the death the scoreboard counts for it can be.
const COUNTED_DEATH_TICKS: f64 = 30.0;

#[derive(Clone, Copy, Debug)]
pub struct LifeClock {
    pub tick_offset: f64,
    pub respawn_ticks: f64,
}

#[derive(Debug)]
pub struct LifeAssignment {
    pub clock: LifeClock,
    /// Slot id to (roster index, life index).
    pub owner: HashMap<i32, (usize, usize)>,
    /// Owned slots whose life ended in a death, to its HighlightEvents time, or
    /// None for a time-chained player.
    pub deaths: HashMap<i32, Option<f64>>,
}

/// Matching costs, in game ticks of disagreement.
#[derive(Debug)]
pub struct LifeTuning {
    pub start_early_max: f64,
    pub start_soft: f64,
    pub end_late_max: f64,
    pub end_soft: f64,
    pub late_cap: f64,
    pub early_cap: f64,
    pub id_prior: f64,
    pub initial_window: f64,
    pub join_cost: f64,
    pub survivor_window: f64,
    pub side_cost: f64,
    pub unassigned: f64,
}

pub const LIFE_TUNING: LifeTuning = LifeTuning {
    start_early_max: 150.0,
    start_soft: 100.0,
    end_late_max: 60.0,
    end_soft: 60.0,
    late_cap: 150.0,
    early_cap: 150.0,
    id_prior: 120.0,
    initial_window: 300.0,
    join_cost: 60.0,
    survivor_window: 600.0,
    side_cost: 150.0,
    unassigned: 250.0,
};

const FORBIDDEN: f64 = 1e9;

/// Median of the values inside the densest window of half-width `win` of `xs`, and the
/// window's count.
fn vote(xs: &[f64], win: f64) -> (f64, usize) {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    let (mut best_c, mut best_v) = (0usize, 0.0f64);
    let mut j = 0usize;
    for i in 0..s.len() {
        while s[i] - s[j] > 2.0 * win {
            j += 1;
        }
        if i - j + 1 > best_c {
            best_c = i - j + 1;
            best_v = (s[i] + s[j]) / 2.0;
        }
    }
    let in_win: Vec<f64> = s.iter().copied().filter(|x| (x - best_v).abs() <= win).collect();
    let v = if in_win.is_empty() { best_v } else { in_win[in_win.len() / 2] };
    (v, best_c)
}

/// The game tick of HighlightEvents time 0 (the densest cluster of slot end
/// minus death tick) and the respawn delay R (respawn starts cluster at the
/// offset plus R).
pub fn estimate_life_clock(slots: &[LifeSlot], death_ticks: &[f64]) -> Option<LifeClock> {
    if death_ticks.len() < 3 {
        return None;
    }
    let mut ends = Vec::new();
    let mut starts = Vec::new();
    for s in slots {
        if s.is_bot {
            continue;
        }
        for &d in death_ticks {
            let e = s.end - d;
            if e > -4000.0 && e < 2000.0 {
                ends.push(e);
            }
            starts.push(s.start - d);
        }
    }
    let (off_v, off_c) = vote(&ends, 30.0);
    if (off_c as f64) < f64::max(3.0, death_ticks.len() as f64 * 0.3) {
        return None;
    }
    let cand: Vec<f64> = starts.into_iter().filter(|&x| x > off_v + 300.0 && x < off_v + 1200.0).collect();
    let (k_v, k_c) = vote(&cand, 40.0);
    let mut respawn = k_v - off_v;
    // Else 8.7 s, the usual multiplayer respawn delay.
    if !(respawn > 360.0 && respawn < 1100.0) || k_c < 3 {
        respawn = 520.0;
    }
    Some(LifeClock { tick_offset: off_v, respawn_ticks: respawn })
}

/// Rectangular min-cost assignment (no more rows than columns): the column of every row.
fn hungarian(cost: &[Vec<f64>]) -> Vec<i64> {
    let n = cost.len();
    if n == 0 {
        return Vec::new();
    }
    let m = cost[0].len();
    const INF: f64 = 1e18;
    let mut u = vec![0.0f64; n + 1];
    let mut v = vec![0.0f64; m + 1];
    let mut p = vec![0usize; m + 1];
    let mut way = vec![0usize; m + 1];
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0usize;
        let mut minv = vec![INF; m + 1];
        let mut used = vec![false; m + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = INF;
            let mut j1 = 0usize;
            for j in 1..=m {
                if used[j] {
                    continue;
                }
                let cur = cost[i0 - 1][j - 1] - u[i0] - v[j];
                if cur < minv[j] {
                    minv[j] = cur;
                    way[j] = j0;
                }
                if minv[j] < delta {
                    delta = minv[j];
                    j1 = j;
                }
            }
            for j in 0..=m {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    let mut res = vec![-1i64; n];
    for j in 1..=m {
        if p[j] != 0 {
            res[p[j] - 1] = (j - 1) as i64;
        }
    }
    res
}

/// Which half of the map a life's first record is in, as a team index.
#[derive(Clone, Copy)]
struct SideOf {
    use_x: bool,
    mid: f64,
    flip: bool,
}

impl SideOf {
    fn raw(&self, s: &LifeSlot) -> i64 {
        let v = (if self.use_x { s.x } else { s.y }).unwrap_or(self.mid);
        if v < self.mid {
            0
        } else {
            1
        }
    }
    fn of(&self, s: &LifeSlot) -> i64 {
        if self.flip {
            1 - self.raw(s)
        } else {
            self.raw(s)
        }
    }
}

fn team_bit(team: f64) -> i64 {
    (team as i64) & 1
}

/// Spawn-side model from the initial lives of a first pass; switched on only
/// when 90% or more of that pass's on-time respawns agree with it.
fn learn_side(initial: &[(&LifeSlot, f64)], respawns: &[(&LifeSlot, f64)]) -> Option<SideOf> {
    let usable: Vec<&(&LifeSlot, f64)> = initial.iter().filter(|x| x.0.x.is_some() && x.0.y.is_some()).collect();
    if usable.len() < 4 {
        return None;
    }
    let gap = |vals: Vec<f64>| -> (f64, f64) {
        let mut v = vals;
        v.sort_by(f64::total_cmp);
        let (mut g, mut mid) = (0.0f64, v[0]);
        for i in 1..v.len() {
            if v[i] - v[i - 1] > g {
                g = v[i] - v[i - 1];
                mid = (v[i] + v[i - 1]) / 2.0;
            }
        }
        (g, mid)
    };
    let gx = gap(usable.iter().map(|u| u.0.x.unwrap()).collect());
    let gy = gap(usable.iter().map(|u| u.0.y.unwrap()).collect());
    let use_x = gx.0 >= gy.0;
    let mid = if use_x { gx.1 } else { gy.1 };
    let probe = SideOf { use_x, mid, flip: false };
    let agree = usable.iter().filter(|u| probe.raw(u.0) == team_bit(u.1)).count();
    let flip = agree * 2 < usable.len();
    let side = SideOf { use_x, mid, flip };
    let checked: Vec<&(&LifeSlot, f64)> = respawns.iter().filter(|x| x.0.x.is_some()).collect();
    let ok = checked.iter().filter(|x| side.of(x.0) == team_bit(x.1)).count();
    if checked.len() >= 10 && ok as f64 >= 0.9 * checked.len() as f64 {
        Some(side)
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PosKind {
    Initial,
    Join,
    Respawn,
}

struct Pos {
    player: usize,
    life: usize,
    s_exp: Option<f64>,
    e_exp: Option<f64>,
    kind: PosKind,
    left: bool,
}

/// Assign slots to (player, life). None when no player has usable
/// HighlightEvents deaths or the clock can't be estimated.
pub fn assign_lives(slots: &[LifeSlot], players: &[LifePlayer]) -> Option<LifeAssignment> {
    let t = &LIFE_TUNING;
    let by_index: HashMap<usize, &LifePlayer> = players.iter().map(|p| (p.index, p)).collect();
    let timed: Vec<&LifePlayer> = players.iter().filter(|p| p.death_ms.is_some() && p.spawns > 0.0).collect();
    let death_ticks: Vec<f64> =
        timed.iter().flat_map(|p| p.death_ms.as_ref().unwrap().iter().map(|ms| ms * TICKS_PER_MS)).collect();
    let clock = estimate_life_clock(slots, &death_ticks)?;
    let (off, r) = (clock.tick_offset, clock.respawn_ticks);
    let tick_of = |ms: f64| ms * TICKS_PER_MS + off;

    let human_slots: Vec<&LifeSlot> = slots.iter().filter(|s| !s.is_bot).collect();
    if human_slots.is_empty() {
        return None;
    }
    let t0 = human_slots.iter().map(|s| s.start).fold(f64::INFINITY, f64::min);
    let match_end = slots.iter().map(|s| s.end).fold(f64::NEG_INFINITY, f64::max);
    let team_of = |player: usize| -> Option<f64> { by_index.get(&player).and_then(|p| p.team) };

    // Stage 1: humans with HighlightEvents, by min-cost matching.
    let mut pos: Vec<Pos> = Vec::new();
    for p in &timed {
        let d = p.death_ms.as_ref().unwrap();
        // More lives than the film has slots and deaths can't be matched; a
        // damaged scoreboard can claim billions.
        let lives = p.spawns.min((slots.len() + d.len() + 1) as f64);
        let mut k = 0usize;
        while (k as f64) < lives {
            pos.push(Pos {
                player: p.index,
                life: k,
                left: p.left,
                kind: if k > 0 {
                    PosKind::Respawn
                } else if p.present_at_start {
                    PosKind::Initial
                } else {
                    PosKind::Join
                },
                s_exp: if k > 0 && k - 1 < d.len() { Some(tick_of(d[k - 1]) + r) } else { None },
                e_exp: if k < d.len() { Some(tick_of(d[k])) } else { None },
            });
            k += 1;
        }
    }
    let cost = |s: &LifeSlot, q: &Pos, side: Option<SideOf>| -> f64 {
        if s.is_bot != by_index.get(&q.player).map(|p| p.is_bot).unwrap_or(false) {
            return FORBIDDEN;
        }
        if s.player.is_some_and(|p| p != q.player) {
            return FORBIDDEN;
        }
        if by_index.get(&q.player).is_some_and(|p| !p.can_start(s.start)) {
            return FORBIDDEN;
        }
        // A slot whose player the film names needs no prior on who it is:
        // only which of their lives.
        let named = s.player.is_some();
        let team = team_of(q.player);
        let wrong_side = match (side, team) {
            (Some(sd), Some(tm)) => !named && sd.of(s) != team_bit(tm),
            _ => false,
        };
        let mut c = 0.0f64;
        match q.kind {
            PosKind::Initial => {
                if let Some(e) = q.e_exp {
                    if s.start > e {
                        return FORBIDDEN;
                    }
                }
                if !named {
                    if s.slot_id as i64 != q.player as i64 {
                        c += t.id_prior;
                    }
                    c += f64::min(t.late_cap, 0.5 * f64::max(0.0, s.start - t0 - t.initial_window));
                }
                if wrong_side && s.start <= t0 + t.initial_window {
                    c += t.side_cost;
                }
            }
            PosKind::Join => {
                if let Some(e) = q.e_exp {
                    if s.start > e {
                        return FORBIDDEN;
                    }
                }
                c += t.join_cost;
            }
            PosKind::Respawn => {
                let Some(se) = q.s_exp else { return FORBIDDEN };
                let ds = s.start - se;
                if ds < -t.start_early_max {
                    return FORBIDDEN;
                }
                c += if ds < 0.0 {
                    -ds
                } else {
                    f64::min(t.late_cap, if ds <= t.start_soft { ds } else { t.start_soft + 0.5 * (ds - t.start_soft) })
                };
                if wrong_side && ds <= t.start_soft {
                    c += t.side_cost;
                }
            }
        }
        if let Some(ee) = q.e_exp {
            // Where the player let go of the life, when the film says: a body
            // in a vehicle can send records after its death.
            let e = s.released.unwrap_or(s.end) - ee;
            if e > t.end_late_max {
                // A slot the film names its player for can outlast the death:
                // a body left seated in a moving vehicle sends the vehicle's
                // records until it is deleted.
                if !named || s.start > ee {
                    return FORBIDDEN;
                }
                c += f64::min(t.early_cap, 0.5 * t.end_late_max);
            } else {
                c += f64::min(
                t.early_cap,
                    if e >= -t.end_soft { 0.5 * e.abs() } else { 0.5 * t.end_soft + 0.25 * (-e - t.end_soft) },
                );
            }
        } else if !q.left {
            c += f64::min(t.early_cap, 0.25 * f64::max(0.0, match_end - t.survivor_window - s.end));
        }
        c
    };

    let solve = |side: Option<SideOf>| -> IndexMap<usize, usize> {
        let mut edges: Vec<(usize, usize, f64)> = Vec::new();
        for (si, s) in human_slots.iter().enumerate() {
            for (qi, q) in pos.iter().enumerate() {
                let c = cost(s, q, side);
                if c < t.unassigned {
                    edges.push((si, qi, c));
                }
            }
        }
        let hn = human_slots.len();
        let mut parent: Vec<usize> = (0..hn + pos.len()).collect();
        fn find(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        for &(si, qi, _) in &edges {
            let a = find(&mut parent, si);
            let b = find(&mut parent, hn + qi);
            if a != b {
                parent[a] = b;
            }
        }
        let mut comps: IndexMap<usize, Vec<(usize, usize, f64)>> = IndexMap::new();
        for &e in &edges {
            let root = find(&mut parent, e.0);
            comps.entry(root).or_default().push(e);
        }
        let mut out: IndexMap<usize, usize> = IndexMap::new();
        for list in comps.values() {
            let mut rows: IndexMap<usize, usize> = IndexMap::new();
            let mut cols: IndexMap<usize, usize> = IndexMap::new();
            for e in list {
                let nr = rows.len();
                rows.entry(e.0).or_insert(nr);
                let nc = cols.len();
                cols.entry(e.1).or_insert(nc);
            }
            let (nr, nc) = (rows.len(), cols.len());
            let mut m: Vec<Vec<f64>> = (0..nr)
                .map(|i| {
                    let mut row = vec![FORBIDDEN; nc + nr];
                    row[nc + i] = t.unassigned;
                    row
                })
                .collect();
            for &(si, qi, c) in list {
                m[rows[&si]][cols[&qi]] = c;
            }
            let row_keys: Vec<usize> = rows.keys().copied().collect();
            let col_keys: Vec<usize> = cols.keys().copied().collect();
            for (i, col) in hungarian(&m).into_iter().enumerate() {
                if col >= 0 && (col as usize) < nc {
                    out.insert(row_keys[i], col_keys[col as usize]);
                }
            }
        }
        out
    };

    let mut matched = solve(None);
    let sample = |matched: &IndexMap<usize, usize>, kind: PosKind| -> Vec<(&LifeSlot, f64)> {
        matched
            .iter()
            .filter(|&(&si, &qi)| {
                pos[qi].kind == kind
                    && (kind != PosKind::Respawn
                        || (human_slots[si].start - pos[qi].s_exp.unwrap_or(f64::NAN)).abs() <= t.start_soft)
            })
            .filter_map(|(&si, &qi)| team_of(pos[qi].player).map(|tm| (human_slots[si], tm)))
            .collect()
    };
    let side = learn_side(&sample(&matched, PosKind::Initial), &sample(&matched, PosKind::Respawn));
    if side.is_some() {
        matched = solve(side);
    }

    let mut owner: HashMap<i32, (usize, usize)> = HashMap::new();
    let mut deaths: HashMap<i32, Option<f64>> = HashMap::new();
    for (&si, &qi) in &matched {
        let (s, q) = (human_slots[si], &pos[qi]);
        owner.insert(s.slot_id, (q.player, q.life));
        let d = by_index[&q.player].death_ms.as_ref().unwrap();
        if q.life < d.len() {
            deaths.insert(s.slot_id, Some(d[q.life]));
        }
    }

    // Stage 2: players without HighlightEvents (bots), chained by time.
    let untimed: Vec<&LifePlayer> = players.iter().filter(|p| p.death_ms.is_none() && p.spawns > 0.0).collect();
    let mut chains: HashMap<usize, Vec<&LifeSlot>> = untimed.iter().map(|p| (p.index, Vec::new())).collect();
    let mut rest: Vec<&LifeSlot> = slots.iter().filter(|s| !owner.contains_key(&s.slot_id)).collect();
    rest.sort_by(|a, b| f64::total_cmp(&a.start, &b.start));
    for s in rest {
        let mut best: Option<usize> = None;
        let mut best_c = f64::INFINITY;
        for p in &untimed {
            if p.is_bot != s.is_bot || s.player.is_some_and(|o| o != p.index) {
                continue;
            }
            let chain = &chains[&p.index];
            if chain.len() as f64 >= p.spawns {
                continue;
            }
            // An uncounted bot's lives start while it is listed.
            if p.listed.is_some_and(|(a, b)| s.start < a - BOT_LISTED_EARLY_TICKS || s.start > b) || !p.can_start(s.start) {
                continue;
            }
            let mut c;
            if let Some(last) = chain.last() {
                let gap = s.start - last.end;
                if gap < r - t.start_early_max {
                    continue;
                }
                c = f64::min(t.late_cap, (gap - r).abs());
            } else if p.present_at_start {
                c = (if s.slot_id as i64 == p.index as i64 { 0.0 } else { t.id_prior })
                    + f64::min(t.late_cap, 0.5 * f64::max(0.0, s.start - t0 - t.initial_window));
            } else {
                c = t.late_cap;
            }
            if let (Some(sd), Some(tm)) = (side, p.team) {
                if sd.of(s) != team_bit(tm) {
                    c += t.side_cost;
                }
            }
            if c < best_c {
                best_c = c;
                best = Some(p.index);
            }
        }
        if let Some(b) = best {
            chains.get_mut(&b).unwrap().push(s);
        }
    }
    // Where the film says when a player let go of a life, that is a death
    // when the scoreboard counts one then; else every life but the last
    // died, as many as the scoreboard counts.
    let said = slots.iter().any(|s| s.released.is_some());
    for p in &untimed {
        let chain = &chains[&p.index];
        for (k, s) in chain.iter().enumerate() {
            owner.insert(s.slot_id, (p.index, k));
            let last = k == chain.len() - 1;
            let died = if said {
                p.died_at(s.released)
            } else {
                (!last
                    || p.deaths >= p.spawns
                    || ((chain.len() as f64) < p.spawns && !p.left && s.end < match_end - t.survivor_window))
                    && (k as f64) < p.deaths
            };
            if died {
                deaths.insert(s.slot_id, None);
            }
        }
    }

    // A life the film names that the matching left out is its owner's: a
    // mode whose rounds respawn everyone (Infection) gives a player more
    // lives than their deaths and spawns predict. It ends in one of the
    // owner's deaths when one falls at its end.
    for s in slots {
        if owner.contains_key(&s.slot_id) {
            continue;
        }
        let Some(p) = s.player.and_then(|p| by_index.get(&p)) else { continue };
        if p.is_bot != s.is_bot || !p.can_start(s.start) {
            continue;
        }
        owner.insert(s.slot_id, (p.index, usize::MAX));
        let Some(times) = &p.death_ms else {
            if p.died_at(s.released) {
                deaths.insert(s.slot_id, None);
            }
            continue;
        };
        let taken: Vec<f64> = owner
            .iter()
            .filter(|(_, o)| o.0 == p.index)
            .filter_map(|(slot, _)| deaths.get(slot).copied().flatten())
            .collect();
        let at_end = times.iter().copied().filter(|ms| !taken.contains(ms)).find(|&ms| {
            let d = tick_of(ms);
            let end = s.released.unwrap_or(s.end);
            end <= d + t.end_late_max && end >= d - NAMED_DEATH_EARLY_TICKS
        });
        if let Some(ms) = at_end {
            deaths.insert(s.slot_id, Some(ms));
        }
    }
    // Each player's lives numbered in the order they start.
    let start_of: HashMap<i32, f64> = slots.iter().map(|s| (s.slot_id, s.start)).collect();
    let mut by_player: HashMap<usize, Vec<i32>> = HashMap::new();
    for (&slot, &(p, _)) in &owner {
        by_player.entry(p).or_default().push(slot);
    }
    for (p, mut mine) in by_player {
        mine.sort_by(|a, b| f64::total_cmp(&start_of[a], &start_of[b]).then(a.cmp(b)));
        for (k, slot) in mine.into_iter().enumerate() {
            owner.insert(slot, (p, k));
        }
    }

    Some(LifeAssignment { clock, owner, deaths })
}

/// How long before a death a named life's last record can be and still end
/// in it: a player standing still sends none.
const NAMED_DEATH_EARLY_TICKS: f64 = 600.0;
