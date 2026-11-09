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
//! downloaded or already inflated, from wherever they come from.
//!
//! # Quick start
//!
//! ```no_run
//! use filmshell::{Chunk, ChunkKind, Event, Film};
//!
//! # fn bytes(i: u32) -> Vec<u8> { Vec::new() }
//! // One `Chunk` per manifest entry: its index, its type and its bytes.
//! let chunks = (0..=31).map(|i| Chunk::new(i, ChunkKind::by_position(i, 32), bytes(i)));
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

pub use chunk::{Chunk, ChunkKind};
pub use error::Error;
pub use film::Film;
pub use hints::{Hints, LevelBounds, SpawnPad};
pub use model::*;
pub use tag::TagId;

mod accuracy;
mod actions;
mod ammo;
mod assemble;
mod bits;
mod calibrate;
mod components;
mod container;
mod control;
mod creation;
mod damage;
mod deploys;
mod events;
mod fire;
mod gamestate;
mod grenades;
mod header;
mod highlights;
mod hit_damage;
mod holders;
mod identity;
mod inflate;
mod inventory;
mod levels;
mod lives;
mod motion;
mod objectives;
mod payload;
mod registry;
mod replication;
mod roster;
mod sandbox;
mod scripts;
mod snapshot;
mod spawns;
mod splash;
mod statlog;
#[cfg(test)]
mod testing;
mod types;
mod variant;
mod variant_stats;
mod vehicles;
mod vitals;
mod weapon_stats;
mod weapons;
mod world;
mod zones;
