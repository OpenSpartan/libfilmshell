//! The film's data model. Every type here is re-exported at the crate root.

mod event;
mod header;
mod ids;
mod life;
mod objects;
mod player;
mod scoreboard;
mod state;
mod time;
mod variant;

pub use event::*;
pub use header::Header;
pub use ids::{Guid, PlayerId, PlayerMask, Position, TeamId, Xuid};
pub use life::{CoordinateSource, Coordinates, Life, Sample};
pub use objects::{GrenadeFlight, GroundWeapon, HillScore, Ride, Vehicle, WorldObject, Zone, ZoneShape, ZoneState};
pub use player::{Player, PlayerKind};
pub use scoreboard::{EngineState, MatchClock, Scoreboard, Stat, StatChange, StatOwner};
pub use state::{Ammo, Energy, EquipmentCharge, GrenadeCount, Heat, Input, PlayerState};
pub use time::{Clock, Tick};
pub use variant::{GameMode, GameVariant, Loadout, Rule, StatDefinition, StatSettings, VariantOption};
