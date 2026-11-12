//! Loading a film from a folder of chunk files.
//!
//! The Halo Infinite film manifest lists each chunk with an `Index`, a
//! `ChunkType` (1 header, 2 replay, 3 highlights) and a `FileRelativePath`
//! such as `/filmChunk0`. A downloader maps each entry to
//! `Chunk::new(entry.Index, ChunkKind::from_manifest(entry.ChunkType)?, bytes)`.
//! Without the manifest, `Chunk::from_file` reads the index from a
//! `filmChunk{N}` (as downloaded) or `filmChunk{N}_dec` (inflated) file name
//! and the kind from the bytes.

use filmshell::game::{self, TextPart};
use filmshell::{Chunk, Film, GameEventArgument, StringId};
use std::path::Path;
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

/// The chunks of the film in `dir`, the inflated copy of a chunk where both
/// are there. Other files are skipped.
pub fn load_folder(dir: &Path) -> Result<Vec<Chunk<'static>>, Box<dyn std::error::Error>> {
    let mut chunks = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if Chunk::file_index(name).is_none() || dir.join(format!("{name}_dec")).exists() {
            continue;
        }
        chunks.push(Chunk::from_file(name, std::fs::read(&path)?)?);
    }
    Ok(chunks)
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
