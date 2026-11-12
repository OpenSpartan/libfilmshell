//! Reads Halo Infinite theater films.
//!
//! A film is a set of chunks: a header chunk, replay chunks that record the
//! match tick by tick, and a highlights chunk (the game's `HighlightEvents`)
//! with the theater timeline. [`Film::from_chunks`] reads them into a data
//! model: who played, every life with its positions and vitals, vehicles and
//! what stood on the map, one stream of events (kills, shots, hits, pickups,
//! medals, voice lines, objectives), the scoreboard, and each player's inputs
//! and ammunition.
//!
//! The library does no I/O. Hand it the chunks' bytes, compressed as
//! downloaded or already inflated, from wherever they come from: with the
//! manifest's chunk types ([`ChunkKind::from_manifest`]), or, for chunks
//! saved as `filmChunk{N}` files, with [`Chunk::from_file`], which takes the
//! index from the file name and the kind from the bytes.
//!
//! # Quick start
//!
//! ```no_run
//! use filmshell::{Chunk, ChunkKind, Film};
//!
//! # struct Entry { index: u32, chunk_type: u32, bytes: Vec<u8> }
//! # let manifest: Vec<Entry> = Vec::new();
//! // One `Chunk` per manifest entry: its index, its type and its bytes.
//! let chunks = manifest.iter().map(|e| {
//!     let kind = ChunkKind::from_manifest(e.chunk_type).expect("a known chunk type");
//!     Chunk::new(e.index, kind, e.bytes.as_slice())
//! });
//! let film = Film::from_chunks(chunks)?;
//!
//! let name = |id| film.player_name(id).unwrap_or("someone");
//! // Leaves out the deaths the scoreboard doesn't count: those after a round
//! // is decided, among them the deaths the game deals everyone alive when
//! // the match ends.
//! for (tick, kill) in film.kills().filter(|(_, k)| k.is_scored()) {
//!     match kill.credited_killer() {
//!         Some(killer) => println!("{tick} {} killed {}", name(killer), name(kill.victim)),
//!         None => println!("{tick} {} died", name(kill.victim)),
//!     }
//! }
//! # Ok::<(), filmshell::Error>(())
//! ```
//!
//! # Playback
//!
//! Everything is read up front, so playback can seek to any tick: at any
//! [`Tick`], [`Film::players_at`] gives every living player with their
//! sample (where they were, their aim, health and shield), the weapon in
//! their hands, the objective they carry and the vehicle they ride, and
//! [`Film::events_in`] what happened in a stretch of time. [`Film::life_at`],
//! [`Film::weapon_at`] and [`Film::vehicle_at`] look up one thing. The rest
//! of the map at a tick is [`Film::vehicles_at`], [`Film::grenades_at`],
//! [`Film::projectiles_at`], [`Film::ground_weapons_at`] and
//! [`Film::world_objects_at`], with [`Zone::state_at`] for a zone mode.
//! [`Clock`] maps the film's frames to ticks.
//!
//! A viewer's loop, one tick at a time:
//!
//! ```no_run
//! # fn run(film: &filmshell::Film) {
//! use filmshell::Tick;
//!
//! let mut from = Tick(0);
//! let mut now = Tick(0);
//! while now <= film.clock().end() {
//!     for p in film.players_at(now) {
//!         // Between samples, where the player was this tick.
//!         let _at = p.life.position_at(now);
//!     }
//!     // What happened since the last frame drawn, this tick included.
//!     for _event in film.events_in(from..=now) {}
//!     from = now + 1;
//!     now += 1;
//! }
//! # }
//! ```
//!
//! # Layout
//!
//! - The crate root holds [`Film`], the input types ([`Chunk`],
//!   [`Hints`]) and the model: [`Player`], [`Life`], [`Event`],
//!   [`Scoreboard`] and the rest.
//! - [`game`] is what the library knows about the game itself: weapons,
//!   vehicles and other objects, damage effects, sandbox numbers, medals,
//!   levels, mode options, the modes' event trees and the English text of
//!   the game's text keys, as Rust tables.
//! - [`format`](mod@format) exposes the raw layers: chunks, packets, frames,
//!   the event and entity views of a tick packet, the seek snapshots, a bit
//!   reader.
//! - [`analysis`] derives estimates the film doesn't state directly:
//!   accuracy, weapon numbers, spawn factors.
//!
//! # Time and space
//!
//! Times are [`Tick`]s, 1/60 s of game time from the film's first frame.
//! A span that lasts through its ticks (a life, a ride, a vehicle's track)
//! gives its first and last, both inclusive; one that ends when something is
//! gone (an object off the map, a weapon picked up) gives that tick, and says
//! so.
//! Positions are in world units ([`Position`]): a world unit is a foot, and
//! the game's files use engine units of ten world units. The model and
//! [`analysis`] use world units; the [`game`] tables keep engine units. A film
//! doesn't carry the box its positions are quantized over: the library knows
//! it for the levels in its catalogue or from [`Hints::level_bounds`];
//! otherwise it fits one (to [`Hints::spawn_pads`] where given) or leaves
//! positions unconverted ([`Sample::raw`]).
//!
//! # Checked against the match stats
//!
//! A film doesn't carry the stats the Halo services keep for its match (the
//! match stats), and the library doesn't read them; its tests do. On 95
//! films (film version 41: Slayer, Juggernaut, Headhunter, CTF, Big Team
//! Battle, King of the Hill, Strongholds, Land Grab, Oddball, Stockpile,
//! Infection, Firefight, the minigames, Attrition and Elimination,
//! Extraction, Escalation, Grifball) they check the level, every player's
//! kills of enemy players (765 players) and deaths (786), and every team's
//! score (136 teams) against the match stats: all agree, bots included,
//! though the game sends no kill event or highlights record for a bot's
//! death: those deaths are inferred from the rest of the film
//! ([`Kill::inferred`]). For one player the comparison stops partway:
//! the game cleared their scoreboard row mid-match (a reconnect), and the
//! match stats count only what came before; the film goes on to the end.
//!
//! They also check, per player:
//!
//! - Capture the Flag's grabs, steals, captures, returns, carriers killed
//!   and capture assists ([`FlagEvent`]), and Big Team Stockpile's power
//!   seeds ([`SeedEvent`]), all equal.
//! - Hijacks ([`Hijack`]) equal.
//! - Time carrying the flag, the skull or a power seed
//!   ([`PlayerAt::carrying`]), counted while a round is in play, within a
//!   second for 152 of 155 players.
//! - The personal score of the mode scripts' events
//!   ([`Film::game_response`]) equal on every Capture the Flag, Oddball, King
//!   of the Hill, Stockpile, Attrition and Extraction film, and for 93% of
//!   arena Slayer players.
//!
//! Vehicle destroys ([`VehicleDestroyed`]) are the mode scripts' scored ones,
//! which the match stats don't count the same way.
//!
//! # Features
//!
//! - `serde`: derives `Serialize` and `Deserialize` on the model's data
//!   (the players, lives, events, scoreboard and the rest of what the
//!   accessors return, except [`PlayerAt`], [`Clock`], [`Hitch`] and the
//!   [`format`](mod@format) types), the
//!   [`analysis`] results ([`Rule`] and [`analysis::AccuracyTicks`] only
//!   serialize), and the `game` enums and value types such as
//!   [`game::Bounds`]. [`Film`], the inputs ([`Chunk`], [`ChunkKind`],
//!   [`Hints`] and its parts), [`Error`] and the `game` catalogues' entries
//!   ([`game::Level`], [`game::DamageEffect`] and the like) have neither. An
//!   [`Event`] is an object with a `type` (`{"type": "kill", "victim": 3,
//!   ...}`), enum variants are snake_case, [`TagId`]s, [`StringId`]s,
//!   [`Xuid`]s and [`Guid`]s are strings, and empty `Option` and `Vec` fields
//!   are left out. With `serde_json`, turn on its `float_roundtrip` feature
//!   for floats to read back exactly.
//! - `research`: makes the internal readers public under a hidden
//!   `research` module, for studying the format; they are outside the API's
//!   stability guarantees.

#![doc(html_logo_url = "https://raw.githubusercontent.com/OpenSpartan/libfilmshell/main/assets/icon.png")]
#![warn(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

pub mod analysis;
mod chunk;
mod error;
mod film;
pub mod format;
pub mod game;
mod hints;
mod model;
mod tag;
mod text;

pub use chunk::{Chunk, ChunkKind, MAX_CHUNK_BYTES, MAX_FILM_BYTES};
pub use error::Error;
pub use film::{Film, PlayerAt};
pub use hints::{Hints, LevelBounds, SpawnKind, SpawnPad};
pub use model::*;
pub use tag::{StringId, TagId};
pub use text::ParseIdError;

// The README's code compiles as a doc test.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// The internal readers: private, or public and hidden from the docs with the
// `research` feature.
macro_rules! internal {
    ($($m:ident),* $(,)?) => {
        $(
            #[cfg(feature = "research")]
            #[doc(hidden)]
            pub mod $m;
            #[cfg(not(feature = "research"))]
            mod $m;
        )*

        /// The internal readers the model is built from, for studying the
        /// format. Not part of the API: they change without notice.
        #[cfg(feature = "research")]
        #[doc(hidden)]
        pub mod research {
            $(pub use crate::$m;)*
        }
    };
}

internal!(
    accuracy,
    actions,
    ammo,
    assemble,
    attach,
    bits,
    calibrate,
    components,
    container,
    control,
    creation,
    damage,
    deploys,
    events,
    fire,
    gamestate,
    grenades,
    header,
    highlights,
    hit_damage,
    holders,
    identity,
    inflate,
    inventory,
    levels,
    lives,
    motion,
    objectives,
    owners,
    payload,
    registry,
    replication,
    roster,
    sandbox,
    scripts,
    snapshot,
    spawns,
    splash,
    statlog,
    teams,
    types,
    variant,
    variant_stats,
    vehicles,
    vitals,
    weapon_stats,
    weapons,
    world,
    zones,
);
#[cfg(test)]
mod testing;
