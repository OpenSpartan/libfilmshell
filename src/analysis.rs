//! Estimates derived from the model: things the film doesn't state but its
//! data supports, each with what it is computed from.

use crate::model::{Event, PlayerId, Tick};
use crate::{Film, TagId};

/// A player's shots and hits up to a tick: an estimate.
///
/// The scoreboard's accuracy slots are zero in films, so accuracy is
/// counted: shots are [`Event::Shot`]s plus the non-grenade projectiles the
/// player created (the two don't overlap), hits are damage events whose
/// effect belongs to a weapon, once per attacker, victim and frame of the
/// film (one or two ticks: [`Clock`](crate::Clock)). Measured
/// against the match stats, shots and hits are a median 1.00 of the game's
/// counts, and accuracy is off by a median of 4.4 points (90th percentile 38).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Accuracy {
    /// Rounds fired.
    pub shots: u32,
    /// Frames of the film in which the player's weapon damage landed on
    /// another player, counted once per victim: a shotgun blast counts once,
    /// a blast that hits two players twice.
    pub hits: u32,
}

impl Accuracy {
    /// Hits over shots, None without shots.
    pub fn ratio(&self) -> Option<f64> {
        (self.shots > 0).then(|| self.hits as f64 / self.shots as f64)
    }
}

/// A weapon's numbers in this film, from the projectile and damage effect
/// its hits used, so a variant that swaps them shows its own.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct WeaponProfile {
    /// The weapon.
    pub weapon: TagId,
    /// Rounds a second.
    pub rounds_per_second: f64,
    /// Shots per trigger pull; 0 for an automatic.
    pub burst: i32,
    /// Rounds loaded, carried at the start, carried at most.
    pub magazine: Option<[i32; 3]>,
    /// The projectile's initial velocity, world units a second.
    pub velocity: Option<f64>,
    /// The projectile's range, world units.
    pub range: Option<f64>,
    /// A hit's damage on a shield and on a body, when it is one amount.
    pub damage: Option<[f64; 2]>,
    /// Body hits that kill a Spartan at full shield and health, without
    /// headshots.
    pub hits_to_kill: Option<u32>,
}

/// How a respawn's pad scored under the game's default spawn weights, from
/// where everyone else was 2 ticks before. Needs the map's spawn points in
/// [`Hints::spawn_pads`](crate::Hints::spawn_pads).
///
/// The game scores every spawn point by regions around allies, enemies and
/// recent deaths, each adding its weight inside, scaled by a falloff, and
/// compares points by `floor(score / step)` ([`SpawnModel::step`]). Only
/// the player influencers are modelled, and there is no line of sight, so
/// a factor can rule pads out but doesn't name the one the game chose.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct SpawnFactor {
    /// When the player spawned.
    pub tick: Tick,
    /// Who.
    pub player: PlayerId,
    /// The pad, an index into the spawn pads given to the hints.
    pub pad: usize,
    /// The pad's score.
    pub score: f64,
    /// Respawn pads open to the player that scored a better step.
    pub better: u32,
    /// Respawn pads open to the player that scored the same step.
    pub same: u32,
    /// Respawn pads open to the player that scored a worse step.
    pub worse: u32,
    /// Per region of [`spawn_model`], how many sources were inside.
    pub sources: Vec<u32>,
}

/// Whose presence a spawn influencer region is around.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum Influencer {
    /// A living teammate.
    Ally,
    /// A living opponent.
    Enemy,
    /// A teammate who died within [`SpawnModel::dead_seconds`].
    DeadAlly,
}

/// One influencer region of the spawn model: a cylinder around each source,
/// which adds its weight to a spawn point inside it.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct SpawnRegion {
    /// Around whom.
    pub around: Influencer,
    /// The cylinder's radius, world units.
    pub radius: f64,
    /// Its whole height, world units.
    pub height: f64,
    /// What a point inside gets.
    pub weight: f64,
    /// Every source inside counts; else only the strongest.
    pub stacks: bool,
    /// The weight's falloff with `d`, the distance over the radius,
    /// piecewise linear: each piece `[up_to, slope, intercept]` is
    /// `slope * d + intercept` for `d` up to `up_to`, clamped to 0..1, the
    /// last piece continuing past its end. Empty when the weight is flat.
    pub falloff: Vec<[f64; 3]>,
}

/// The game's default spawn weights as [`spawn_factors`] models them.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct SpawnModel {
    /// Scores are compared in steps of this.
    pub step: f64,
    /// A death counts this many seconds.
    pub dead_seconds: f64,
    /// The modelled regions, as [`SpawnFactor::sources`] counts them.
    pub regions: Vec<SpawnRegion>,
}

/// One player's projectiles and hits, as their accuracy counts them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct AccuracyTicks<'a> {
    /// When the player created a non-grenade projectile.
    pub projectiles: &'a [Tick],
    /// When one of their weapon hits landed on a player.
    pub hits: &'a [Tick],
}

/// One player's projectile and hit ticks.
#[derive(Clone, Debug, Default)]
pub(crate) struct AccuracyData {
    pub player: PlayerId,
    pub projectiles: Vec<Tick>,
    pub landed: Vec<Tick>,
}

/// What the estimates are computed from, kept from the read.
#[derive(Clone, Debug, Default)]
pub(crate) struct AnalysisData {
    pub accuracy: Vec<AccuracyData>,
    pub weapons: Vec<WeaponProfile>,
    pub spawn_factors: Vec<SpawnFactor>,
}

/// `player`'s accuracy over the whole film.
pub fn accuracy(film: &Film, player: PlayerId) -> Accuracy {
    accuracy_at(film, player, Tick(u32::MAX))
}

/// `player`'s accuracy up to and including `tick`.
pub fn accuracy_at(film: &Film, player: PlayerId, tick: Tick) -> Accuracy {
    let fired = film
        .events()
        .iter()
        .take_while(|e| e.tick <= tick)
        .filter(|e| matches!(&e.event, Event::Shot(s) if s.player == player))
        .count();
    let Some(d) = film.analysis.accuracy.iter().find(|a| a.player == player) else {
        return Accuracy { shots: fired as u32, hits: 0 };
    };
    let upto = |v: &[Tick]| v.partition_point(|&t| t <= tick) as u32;
    Accuracy { shots: fired as u32 + upto(&d.projectiles), hits: upto(&d.landed) }
}

/// The ticks [`accuracy_at`] counts for `player` besides their
/// [`Event::Shot`]s: the projectiles they created and the hits they landed.
pub fn accuracy_ticks(film: &Film, player: PlayerId) -> AccuracyTicks<'_> {
    film.analysis
        .accuracy
        .iter()
        .find(|a| a.player == player)
        .map_or(AccuracyTicks::default(), |d| AccuracyTicks { projectiles: &d.projectiles, hits: &d.landed })
}

/// The numbers of every weapon fired in the film that the catalogue has.
pub fn weapon_profiles(film: &Film) -> &[WeaponProfile] {
    &film.analysis.weapons
}

/// Every respawn that started on a hinted spawn pad, of a player whose
/// lives the HighlightEvents death times placed. Empty unless the film has
/// five or more players, its positions convert to world units, and the
/// hints carry spawn pads.
pub fn spawn_factors(film: &Film) -> &[SpawnFactor] {
    &film.analysis.spawn_factors
}

/// The spawn weights [`spawn_factors`] scores pads with: the game's default
/// spawn settings (`ssdf:08dccda1`), its unit and dead-unit regions.
pub fn spawn_model() -> SpawnModel {
    let m = crate::spawns::model();
    SpawnModel {
        step: m.step,
        dead_seconds: m.dead_seconds,
        regions: m
            .terms
            .into_iter()
            .map(|t| SpawnRegion {
                around: match t.who {
                    crate::spawns::Who::Ally => Influencer::Ally,
                    crate::spawns::Who::Enemy => Influencer::Enemy,
                    crate::spawns::Who::DeadAlly => Influencer::DeadAlly,
                },
                radius: t.radius,
                height: t.height,
                weight: t.weight,
                stacks: t.stacks,
                falloff: t.falloff,
            })
            .collect(),
    }
}
