//! Estimates derived from the model: things the film doesn't state but its
//! data supports, each with what it is computed from.

use crate::model::{Event, PlayerId, Tick};
use crate::{Film, TagId};

/// A player's shots and hits up to a tick.
///
/// The scoreboard's accuracy slots are empty in films, so accuracy is
/// counted: shots are weapon-fire events plus the non-grenade projectiles
/// the player created (the two don't overlap), hits are damage events whose
/// effect belongs to a weapon, once per attacker, victim and frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Accuracy {
    /// Rounds fired.
    pub shots: u32,
    /// Rounds that hit a player.
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
    /// The projectile's initial velocity, engine units a second.
    pub velocity: Option<f64>,
    /// The projectile's range, engine units.
    pub range: Option<f64>,
    /// A hit's damage on a shield and on a body, when it is one amount.
    pub damage: Option<[f64; 2]>,
    /// Body hits that kill a Spartan at full shield and health, without
    /// headshots.
    pub hits_to_kill: Option<u32>,
}

/// How a respawn's pad scored under the game's default spawn weights, from
/// where everyone else was a moment before. Needs the map's spawn points in
/// the hints.
///
/// The game scores every spawn point by regions around allies, enemies and
/// recent deaths, each adding its weight inside, scaled by a falloff, and
/// compares points by `floor(score / 4)`. Only the player influencers are
/// modelled.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SpawnFactor {
    /// When the player spawned.
    pub tick: Tick,
    /// Who.
    pub player: PlayerId,
    /// The pad, an index into the hinted spawn pads.
    pub pad: usize,
    /// The pad's score.
    pub score: f64,
    /// Respawn pads open to the player that scored better, the same, worse.
    pub better: u32,
    /// See `better`.
    pub same: u32,
    /// See `better`.
    pub worse: u32,
    /// Per influencer region, how many sources were inside.
    pub sources: Vec<u32>,
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

/// The numbers of every weapon fired in the film that the catalogue has.
pub fn weapon_profiles(film: &Film) -> &[WeaponProfile] {
    &film.analysis.weapons
}

/// Every respawn of a timed life that started on a hinted spawn pad.
pub fn spawn_factors(film: &Film) -> &[SpawnFactor] {
    &film.analysis.spawn_factors
}
