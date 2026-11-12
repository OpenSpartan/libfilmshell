//! What the examples share: running one, and filling in a game text's markup.
//!
//! The examples read a film saved as a folder of chunk files with
//! `Film::from_folder`. A downloader that has the film's manifest instead
//! maps each of its entries (an `Index`, a `ChunkType`: 1 header, 2 replay,
//! 3 highlights) to
//! `Chunk::new(entry.Index, ChunkKind::from_manifest(entry.ChunkType)?, bytes)`.

use filmshell::game::{self, TextPart};
use filmshell::{Film, GameEventArgument, StringId};
use std::process::ExitCode;

/// Runs an example, printing its error as text and exiting with status 1.
pub fn main(run: fn() -> Result<(), Box<dyn std::error::Error>>) -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A text with its markup filled in from the event's arguments, in order:
/// players by name, strings by their text, numbers, and icons as `[name]`.
/// Colour and style are left out.
#[allow(dead_code)]
pub fn fill(text: &str, arguments: &[GameEventArgument], film: &Film) -> String {
    let mut args = arguments.iter();
    let mut out = String::new();
    for part in game::text_parts(text) {
        match part {
            TextPart::Words(w) => out.push_str(w),
            TextPart::Player(_) => out.push_str(match args.next() {
                Some(GameEventArgument::Player(Some(p))) => film.player_name(*p).unwrap_or("someone"),
                _ => "someone",
            }),
            TextPart::String => match args.next() {
                Some(&GameEventArgument::Word { value, .. }) => {
                    out.push_str(game::text(StringId(value)).unwrap_or("?"))
                }
                _ => out.push('?'),
            },
            TextPart::Number => match args.next() {
                Some(&GameEventArgument::Word { value, .. }) => out.push_str(&value.to_string()),
                _ => out.push('?'),
            },
            TextPart::Icon(name) => out.push_str(&format!("[{}]", name.trim_start_matches("killfeed_"))),
            _ => {}
        }
    }
    out
}
