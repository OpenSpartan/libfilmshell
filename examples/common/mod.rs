//! Loading a film from a folder of chunk files.
//!
//! The Halo Infinite film manifest lists each chunk with an `Index`, a
//! `ChunkType` (1 header, 2 replay, 3 highlights) and a `FileRelativePath`
//! such as `/filmChunk0`. A downloader maps each entry to
//! `Chunk::new(entry.Index, ChunkKind::from_manifest(entry.ChunkType)?, bytes)`.
//! Without the manifest, `Chunk::from_file` reads the index from a
//! `filmChunk{N}` (as downloaded) or `filmChunk{N}_dec` (inflated) file name
//! and the kind from the bytes.

use filmshell::{Chunk, Film, GameEventArgument, StringId, game};
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
/// `%p`, `%pa`, `%pe` and `%ps` a player, `%s` a string, `%i` a number,
/// `&killfeed_<name>` an icon, shown as `[name]`. Colour (`%c` and a letter,
/// closed by `%`) and style (`%zy`) are left out.
#[allow(dead_code)]
pub fn fill(text: &str, arguments: &[GameEventArgument], film: &Film) -> String {
    let mut args = arguments.iter();
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '%' => match chars.next() {
                Some('p') => {
                    if chars.peek().is_some_and(|c| matches!(c, 'a' | 'e' | 's')) {
                        chars.next();
                    }
                    let player = match args.next() {
                        Some(GameEventArgument::Player(Some(p))) => film.player_name(*p),
                        _ => None,
                    };
                    out.push_str(player.unwrap_or("someone"));
                }
                Some(k @ ('s' | 'i')) => match args.next() {
                    Some(&GameEventArgument::Word { value, .. }) if k == 's' => {
                        out.push_str(game::text(StringId(value)).unwrap_or("?"))
                    }
                    Some(&GameEventArgument::Word { value, .. }) => out.push_str(&value.to_string()),
                    _ => out.push('?'),
                },
                // Colour and style codes take one more letter.
                Some('c' | 'z') => {
                    chars.next();
                }
                // `%` alone closes coloured text.
                _ => {}
            },
            '&' => {
                let mut name = String::new();
                while let Some(&c) = chars.peek().filter(|c| c.is_ascii_alphanumeric() || **c == '_') {
                    name.push(c);
                    chars.next();
                }
                out.push_str(&format!("[{}]", name.trim_start_matches("killfeed_")));
            }
            _ => out.push(c),
        }
    }
    out
}
