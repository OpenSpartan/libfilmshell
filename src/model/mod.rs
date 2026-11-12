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
pub use ids::{EntityId, Guid, PlayerId, PlayerMask, Position, TeamId, Xuid};
pub use life::{CoordinateSource, Coordinates, Encoding, Life, Sample, SpawnFit};
pub use objects::{
    GrenadeFlight, GroundWeapon, Heading, HillPoints, HillScore, PathPoint, ProjectileFlight, Ride, RideIndex,
    VehicleIndex, VehicleTrack, WorldObject, Zone, ZoneArea, ZoneIndex, ZoneShape, ZoneState,
};
pub use player::{Player, PlayerKind, TeamChange};
pub use scoreboard::{EngineState, MatchClock, Scoreboard, Stat, StatChange, StatOwner};
pub use state::{
    Ammo, Carried, Carry, Energy, EquipmentCharge, GrenadeCount, Heat, HeldSlot, Input, PlayerState, WeaponSlot,
};
pub use time::{Clock, Hitch, Tick};
pub use variant::{GameMode, GameVariant, Loadout, Rule, StatDefinition, StatDisplay, StatSettings, VariantOption};
