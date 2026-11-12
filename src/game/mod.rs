//! What the library knows about the game itself, read from the game's files:
//! weapons, vehicles, grenades, equipment and other objects by tag id,
//! damage effects, medals, levels and their quantization boxes, and the
//! names and defaults of game variant options.
//!
//! Everything here is a static table compiled into the library, in the game
//! files' engine units. Lookups by tag id are binary searches.
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
pub(crate) mod events;
#[rustfmt::skip]
mod events_data;
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

pub use damage::{DamageAmount, DamageCause, DamageEffect, Vitality, damage_effect, spartan_vitality};
pub use events::GameResponse;
pub use items::{
    Equipment, Grenade, Item, ObjectKind, TagInfo, Vehicle, Weapon, item, tag_info, tags, weapon_variant_name,
};
pub use levels::{Level, LevelBox, catalogue_build, level, level_by_api_id, levels};
pub use medals::{Medal, MedalDifficulty, MedalKind};
pub use options::{ModeOption, ModeOptions, OptionValue, modes, murmur3, option_name};
pub use sandbox::{Bounds, FireStats, ProjectileStats, fire_stats, projectile_stats, vehicle_bounds};

/// `Display` as the game's English name.
macro_rules! display_name {
    ($($t:ty),*) => {
        $(impl std::fmt::Display for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.name())
            }
        })*
    };
}

display_name!(Weapon, Vehicle, Equipment, Grenade, Item, Medal);
