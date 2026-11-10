//! How weapons fire, what projectiles do, and how big vehicles are.

use super::sandbox_data::{FIRE, PROJECTILES, VEHICLE_BOUNDS};
use crate::TagId;

/// A weapon's first barrel and magazine.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct FireStats {
    /// The weapon's tag.
    pub tag: TagId,
    /// Rounds a second at the barrel's top rate.
    pub rounds_per_second: f64,
    /// Shots per trigger pull; 0 for an automatic.
    pub burst: i32,
    /// Seconds between bursts.
    pub recovery: f64,
    /// Rounds loaded, carried at the start and carried at most; None for a
    /// weapon without a magazine.
    pub magazine: Option<[i32; 3]>,
    /// The projectile it fires.
    pub projectile: Option<TagId>,
}

/// A projectile's flight and its impact damage effect.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ProjectileStats {
    /// The projectile's tag.
    pub tag: TagId,
    /// Initial velocity, engine units a second.
    pub velocity: f64,
    /// Maximum range, engine units.
    pub range: f64,
    /// The damage effect of a direct hit.
    pub impact: Option<TagId>,
}

pub(super) type FireRow = FireStats;
pub(super) type ProjectileRow = ProjectileStats;

/// How weapon `tag` fires, when the catalogue has it.
pub fn fire_stats(tag: TagId) -> Option<&'static FireStats> {
    FIRE.binary_search_by_key(&tag, |r| r.tag).ok().map(|i| &FIRE[i])
}

/// Projectile `tag`'s numbers, when the catalogue has it.
pub fn projectile_stats(tag: TagId) -> Option<&'static ProjectileStats> {
    PROJECTILES.binary_search_by_key(&tag, |r| r.tag).ok().map(|i| &PROJECTILES[i])
}

/// A box in engine units, its axes `[x, y, z]` with `z` up.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Bounds {
    /// The low corner.
    pub min: [f64; 3],
    /// The high corner.
    pub max: [f64; 3],
}

/// A vehicle's render-model bounds, in the vehicle's own frame (x forward).
pub fn vehicle_bounds(tag: TagId) -> Option<Bounds> {
    let b = VEHICLE_BOUNDS.binary_search_by_key(&tag, |r| r.0).ok().map(|i| VEHICLE_BOUNDS[i].1)?;
    Some(Bounds { min: [b[0], b[2], b[4]], max: [b[1], b[3], b[5]] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_rows() {
        let br = fire_stats(TagId(0x2b1824d5)).unwrap();
        assert_eq!((br.rounds_per_second, br.burst, br.magazine), (15.0, 3, Some([36, 144, 252])));
        let p = projectile_stats(br.projectile.unwrap()).unwrap();
        assert_eq!((p.velocity, p.range, p.impact), (480.0, 250.0, Some(TagId(0x0000b29c))));
        let b = vehicle_bounds(TagId(0x00002705)).unwrap();
        assert_eq!(b.min[1], -b.max[1]);
    }
}
