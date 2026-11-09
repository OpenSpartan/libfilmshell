//! What the library knows about the game itself, read from the game's files:
//! weapons, vehicles, grenades, equipment and other objects by tag id,
//! damage effects, medals, levels and their quantization boxes, and the
//! names and defaults of game variant options.
//!
//! Everything here is a static table compiled into the library. Lookups are
//! binary searches over tag ids.
//!
//! ```
//! use filmshell::game::{Weapon, damage_effect, DamageCause};
//! use filmshell::TagId;
//!
//! assert_eq!(Weapon::from_tag(TagId(0x2b1824d5)), Some(Weapon::Br75BattleRifle));
//! let br_round = damage_effect(TagId(0x0000b29c)).unwrap();
//! assert_eq!(br_round.cause, DamageCause::Weapon);
//! ```

mod damage;
#[rustfmt::skip]
mod damage_data;
mod items;
#[rustfmt::skip]
mod items_data;
mod levels;
#[rustfmt::skip]
mod levels_data;
mod medals;
#[rustfmt::skip]
mod medals_data;
mod options;
#[rustfmt::skip]
mod options_data;
mod sandbox;
#[rustfmt::skip]
mod sandbox_data;

pub use damage::{damage_effect, spartan_vitality, DamageAmount, DamageCause, DamageEffect};
pub use items::{
    item, tag_info, tags, weapon_variant_name, Equipment, Grenade, Item, ObjectKind, TagInfo, Vehicle, Weapon,
};
pub use levels::{catalogue_build, level, levels, Level, LevelBox};
pub use medals::{Medal, MedalDifficulty, MedalKind};
pub use options::{modes, murmur3, option_name, ModeOption, ModeOptions, OptionValue};
pub use sandbox::{fire_stats, projectile_stats, vehicle_bounds, FireStats, ProjectileStats};
