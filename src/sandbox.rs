//! Firing and projectile numbers by raw tag id, for the readers.

use crate::game;
use crate::TagId;

pub use crate::game::{FireStats as WeaponRow, ProjectileStats as Projectile};

pub fn weapon(id: u32) -> Option<WeaponRow> {
    game::fire_stats(TagId(id)).copied()
}

pub fn projectile(id: u32) -> Option<Projectile> {
    game::projectile_stats(TagId(id)).copied()
}
