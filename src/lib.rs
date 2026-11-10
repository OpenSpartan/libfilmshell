//! Reads Halo Infinite theater films.
//!
//! A film is a set of chunks: a header chunk, replay chunks that record the
//! match tick by tick, and a HighlightEvents chunk with the theater
//! timeline. [`Film::from_chunks`] reads them into a data model: who played,
//! every life with its positions and vitals, vehicles and what stood on the
//! map, one stream of events (kills, shots, hits, pickups, medals, voice
//! lines, objectives), the scoreboard, and each player's inputs and
//! ammunition.
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
//! use filmshell::{Chunk, ChunkKind, Event, Film};
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
//! for p in film.players() {
//!     println!("{} {}", p.id, p.name().unwrap_or("?"));
//! }
//! for (tick, kill) in film.kills() {
//!     println!("{tick} {:?} killed {}", kill.killer, kill.victim);
//! }
//! # Ok::<(), filmshell::Error>(())
//! ```
//!
//! # Layout
//!
//! - The crate root holds [`Film`], the input types ([`Chunk`],
//!   [`Hints`]) and the model: [`Player`], [`Life`], [`Event`],
//!   [`Scoreboard`] and the rest.
//! - [`game`] is what the library knows about the game itself: weapons,
//!   vehicles, medals, damage effects and levels, as Rust tables.
//! - [`format`](mod@format) exposes the raw layers: chunks, packets, frames, the event
//!   and entity views of a tick packet, the seek snapshots, a bit reader.
//! - [`analysis`] derives estimates the film doesn't state directly:
//!   accuracy, weapon numbers, spawn factors.
//!
//! # Time and space
//!
//! Times are [`Tick`]s, 1/60 s of game time from the film's first frame.
//! Positions are world units ([`Position`]). A film doesn't carry the box
//! its positions are quantized over; the library knows it for the levels in
//! its catalogue, and otherwise fits it to the spawn pads given in the
//! [`Hints`], or leaves positions unconverted ([`Sample::raw`]).
//!
//! # Features
//!
//! - `serde`: derives `serde::Serialize` on the model types.
//! - `research`: makes the internal readers public under a hidden
//!   `research` module, for studying the format; they are outside the API's
//!   stability guarantees.

#![deny(missing_docs)]

pub mod analysis;
mod chunk;
mod error;
mod film;
pub mod format;
pub mod game;
mod hints;
mod model;
mod tag;

pub use chunk::{Chunk, ChunkKind, MAX_CHUNK_BYTES};
pub use error::Error;
pub use film::Film;
pub use hints::{Hints, LevelBounds, SpawnPad};
pub use model::*;
pub use tag::TagId;

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
