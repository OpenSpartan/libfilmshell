//! Reads Halo Infinite theater films.
//!
//! A film is a set of chunks: a header chunk, replay chunks that record the
//! match tick by tick, and a highlights chunk (the game's HighlightEvents)
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
//! // Leaves out the deaths the game deals everyone alive when a match ends.
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
//! their hands and the vehicle they ride, and [`Film::events_in`] what
//! happened in a stretch of time. [`Film::life_at`], [`Film::weapon_at`] and
//! [`Film::vehicle_at`] look up one thing. [`Clock`] maps the film's frames
//! to ticks.
//!
//! # Layout
//!
//! - The crate root holds [`Film`], the input types ([`Chunk`],
//!   [`Hints`]) and the model: [`Player`], [`Life`], [`Event`],
//!   [`Scoreboard`] and the rest.
//! - [`game`] is what the library knows about the game itself: weapons,
//!   vehicles, medals, damage effects and levels, as Rust tables.
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
//! Extraction, Escalation, Grifball) they check that the level, every
//! player's kills and deaths over the match, and each team's score agree
//! with the match stats. Every player's kills and deaths agree, bots
//! included (647 and 668 players), though a bot's lives, deaths and kills
//! come from the film alone. For one player the comparison stops partway:
//! the game cleared their scoreboard row mid-match (a reconnect), and the
//! match stats count only what came before; the film goes on to the end.
//!
//! # Features
//!
//! - `serde`: derives `Serialize` and `Deserialize` on the model's types
//!   (not [`Film`], [`Clock`], [`Chunk`], [`Hints`] or [`Error`]; [`Rule`]
//!   and [`analysis::AccuracyTicks`] only serialize), the [`analysis`]
//!   results and the `game` enums. An
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
