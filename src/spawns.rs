//! Spawn factors: for each respawn, the pad it was on and what the game's
//! default spawn weights (`ssdf:08dccda1`) give that pad at that moment,
//! from where the other players were.
//!
//! The game scores every spawn point by its influencers: regions around each
//! ally, enemy, recent death, weapon, vehicle and projectile, each adding its
//! weight when the point lies inside, scaled by a falloff function of the
//! distance where the region has one. Points are compared by a key whose last
//! part is `floor(score / quantization step)`, so pads one step apart are
//! equal, and every point that ties for the best key is kept as a
//! candidate. Which candidate wins isn't read.
//!
//! What's modelled is the unit and dead-unit terms from positions. Not: line
//! of sight (the unseen-spawning cones need the collision mesh), the
//! in-combat ally term, spawning allies, weapons, vehicles, projectiles, the
//! key's parts before the score, mode filters (labels, zones). So the model
//! can show a pad was ruled out but not why one pad won over its equals.

use crate::types::{PlayerPath, WorldPosition};

/// A spawn pad (`Spawn Point [Initial]` or `[Respawn]`), in world units, and
/// its index in the caller's list.
#[derive(Clone, Debug)]
pub struct Pad {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub z: Option<f64>,
    pub respawn: bool,
    /// The team the pad is reserved for (MVAR `m_teamDesignator`), when
    /// known; a reserved pad is only used by its own team.
    pub team: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    Ally,
    Enemy,
    /// An ally that died within the model's `dead_seconds`.
    DeadAlly,
}

/// One influencer region: a cylinder, `height` its whole height.
#[derive(Debug)]
pub struct Term {
    pub who: Who,
    pub radius: f64,
    pub height: f64,
    pub weight: f64,
    /// False: only the strongest source counts ("Does not stack").
    pub stacks: bool,
    /// Piecewise linear in distance / radius: (up to x, slope, intercept).
    pub falloff: Vec<[f64; 3]>,
}

#[derive(Debug)]
pub struct Model {
    pub step: f64,
    pub dead_seconds: f64,
    pub terms: Vec<Term>,
}

/// World units per engine unit.
const WORLD_PER_ENGINE: f64 = 10.0;
/// A life start within this of a pad (wu) was on it.
const PAD_WU: f64 = 2.0;
/// Where the others are: their latest sample this many ticks before the
/// spawn, no older than `MAX_AGE_TICKS`.
const BEFORE_TICKS: u32 = 2;
const MAX_AGE_TICKS: u32 = 90;
const TICKS_PER_SECOND: f64 = 60.0;

/// The game's default spawn settings (`ssdf:08dccda1`, from the multiplayer
/// constants), in world units.
pub fn model() -> Model {
    let w = WORLD_PER_ENGINE;
    let term = |who, radius: f64, height: f64, weight, stacks, falloff: &[[f64; 3]]| Term {
        who,
        radius: radius * w,
        height: height * w,
        weight,
        stacks,
        falloff: falloff.to_vec(),
    };
    Model {
        step: 4.0,
        dead_seconds: 12.0,
        terms: vec![
            term(Who::Ally, 18.0, 18.0, 7.0, false, &[[0.1, 10.0, 0.0], [0.7, 0.0, 1.0], [f32::MAX as f64, -3.0, 3.1]]),
            term(Who::Enemy, 18.0, 18.0, -15.0, true, &[[0.5, 0.0, 1.0], [f32::MAX as f64, -1.8, 1.9]]),
            term(Who::Enemy, 10.0, 3.75, -700.0, true, &[]),
            term(Who::Enemy, 10.0, 10.0, -300.0, true, &[]),
            term(Who::DeadAlly, 10.0, 8.0, -17.0, true, &[]),
        ],
    }
}

impl Term {
    /// The weight at horizontal distance `r` and height difference `dz` (None:
    /// unknown, taken as level), None outside.
    fn at(&self, r: f64, dz: Option<f64>) -> Option<f64> {
        if r > self.radius || dz.is_some_and(|d| d.abs() > self.height / 2.0) {
            return None;
        }
        if self.falloff.is_empty() {
            return Some(self.weight);
        }
        let x = r / self.radius;
        let seg = self.falloff.iter().find(|s| x <= s[0]).or(self.falloff.last())?;
        Some(self.weight * (seg[1] * x + seg[2]).clamp(0.0, 1.0))
    }
}

/// One respawn: the pad, how many sources fell inside each term (table order),
/// the pad's score, and how many of the comparable pads scored a better,
/// the same or a worse step.
#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
    pub frame: i64,
    pub player: i32,
    /// Index into the pads given to [`explain`].
    pub pad: usize,
    pub counts: Vec<u32>,
    pub score: f64,
    pub better: u32,
    pub same: u32,
    pub worse: u32,
}

/// Someone on the map at a moment.
struct Unit {
    x: f64,
    y: f64,
    z: Option<f64>,
}

fn tick(ticks: &[u32], frame: i64) -> u32 {
    usize::try_from(frame).ok().and_then(|f| ticks.get(f).copied()).unwrap_or(frame.max(0) as u32)
}

/// A pad's score; `alive` is everyone else on the map, true for an ally.
fn score(m: &Model, pad: &Pad, alive: &[(bool, Unit)], dead: &[Unit], counts: &mut [u32]) -> f64 {
    let mut total = 0.0;
    for (k, t) in m.terms.iter().enumerate() {
        let sources: Vec<&Unit> = match t.who {
            Who::Ally => alive.iter().filter(|a| a.0).map(|a| &a.1).collect(),
            Who::Enemy => alive.iter().filter(|a| !a.0).map(|a| &a.1).collect(),
            Who::DeadAlly => dead.iter().collect(),
        };
        let mut sum = 0.0;
        let mut strongest: f64 = 0.0;
        counts[k] = 0;
        for u in sources {
            let r = ((u.x - pad.x).powi(2) + (u.y - pad.y).powi(2)).sqrt();
            let dz = pad.z.zip(u.z).map(|(a, b)| a - b);
            if let Some(w) = t.at(r, dz) {
                counts[k] += 1;
                sum += w;
                if w.abs() > strongest.abs() {
                    strongest = w;
                }
            }
        }
        total += if t.stacks { sum } else { strongest };
    }
    total
}

/// A path's positions in world units, with each one's height when known.
#[derive(Debug)]
pub struct WorldTrack<'a> {
    pub positions: &'a [WorldPosition],
    pub z: Vec<Option<f64>>,
}

/// Every respawn of a timed life that started on a pad. `paths` and
/// `world_paths` are parallel; `ticks` maps the film's frames to game ticks.
pub fn explain(pads: &[Pad], paths: &[PlayerPath], world_paths: &[WorldTrack<'_>], ticks: &[u32]) -> Vec<Spawn> {
    let m = &model();
    if pads.is_empty() || m.terms.is_empty() || paths.len() != world_paths.len() {
        return Vec::new();
    }
    let dead_ticks = (m.dead_seconds * TICKS_PER_SECOND) as u32;
    // Each player's lives as index ranges into its positions.
    let lives: Vec<Vec<(usize, usize)>> = paths
        .iter()
        .zip(world_paths)
        .map(|(p, w)| {
            let Some(starts) = p.life_starts.as_ref().filter(|_| p.positions.len() == w.positions.len()) else {
                return Vec::new();
            };
            starts
                .iter()
                .enumerate()
                .map(|(i, &s)| (s, starts.get(i + 1).copied().unwrap_or(w.positions.len())))
                .filter(|r| r.0 < r.1)
                .collect()
        })
        .collect();
    let z_of = |w: &WorldTrack<'_>, i: usize| w.z.get(i).copied().flatten();
    let mut out = Vec::new();
    for (k, (p, w)) in paths.iter().zip(world_paths).enumerate() {
        for &(start, _) in lives[k].iter().skip(1) {
            let s = &w.positions[start];
            let Some(pad) = pads
                .iter()
                .map(|q| (((q.x - s.x).powi(2) + (q.y - s.y).powi(2)).sqrt(), q))
                .filter(|q| q.0 <= PAD_WU)
                .min_by(|a, b| f64::total_cmp(&a.0, &b.0))
                .map(|q| q.1)
            else {
                continue;
            };
            let now = tick(ticks, s.frame).saturating_sub(BEFORE_TICKS);
            let ally = |q: &PlayerPath| p.team_id.is_some() && q.team_id == p.team_id;
            // The others alive at that moment, and the allies dead within the window.
            let mut alive: Vec<(bool, Unit)> = Vec::new();
            let mut dead: Vec<Unit> = Vec::new();
            for (j, (q, wq)) in paths.iter().zip(world_paths).enumerate() {
                for (n, &(a, b)) in lives[j].iter().enumerate() {
                    let last = &wq.positions[b - 1];
                    let ended = n + 1 < lives[j].len();
                    if ally(q) && ended && tick(ticks, last.frame) <= now && now - tick(ticks, last.frame) <= dead_ticks
                    {
                        dead.push(Unit { x: last.x, y: last.y, z: z_of(wq, b - 1) });
                    }
                    if j == k {
                        continue;
                    }
                    let i = a + wq.positions[a..b].partition_point(|x| tick(ticks, x.frame) <= now);
                    if i == a {
                        continue;
                    }
                    let at = &wq.positions[i - 1];
                    if now - tick(ticks, at.frame) > MAX_AGE_TICKS || (i == b && ended && tick(ticks, at.frame) < now) {
                        continue;
                    }
                    alive.push((ally(q), Unit { x: at.x, y: at.y, z: z_of(wq, i - 1) }));
                }
            }
            let mut counts = vec![0; m.terms.len()];
            let chosen = score(m, pad, &alive, &dead, &mut counts);
            // The pads it competed with: respawn pads open to the player's team.
            let open = |q: &&Pad| q.respawn && (q.team.is_none() || p.team_id.map(|t| t as i64) == q.team);
            let step = |s: f64| if m.step > 0.0 { (s / m.step).floor() } else { s };
            let (mut better, mut same, mut worse) = (0, 0, 0);
            let mut scratch = vec![0; m.terms.len()];
            for q in pads.iter().filter(open).filter(|q| q.index != pad.index) {
                let o = step(score(m, q, &alive, &dead, &mut scratch));
                match o.partial_cmp(&step(chosen)) {
                    Some(std::cmp::Ordering::Greater) => better += 1,
                    Some(std::cmp::Ordering::Less) => worse += 1,
                    _ => same += 1,
                }
            }
            out.push(Spawn {
                frame: s.frame,
                player: p.player_index,
                pad: pad.index,
                counts,
                score: chosen,
                better,
                same,
                worse,
            });
        }
    }
    out.sort_by_key(|s| (s.frame, s.player));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_loads_and_falloffs_are_continuous() {
        let m = &model();
        assert_eq!(m.step, 4.0);
        assert_eq!(m.terms.len(), 5);
        for t in m.terms.iter().filter(|t| !t.falloff.is_empty()) {
            // Each segment meets the next at its end.
            for w in t.falloff.windows(2) {
                let x = w[0][0];
                assert!((w[0][1] * x + w[0][2] - (w[1][1] * x + w[1][2])).abs() < 1e-5);
            }
            assert_eq!(t.at(t.radius * 0.3, Some(0.0)), Some(t.weight));
            assert_eq!(t.at(t.radius * 1.01, Some(0.0)), None);
            assert_eq!(t.at(0.0, Some(t.height * 0.51)), None);
        }
    }
}
