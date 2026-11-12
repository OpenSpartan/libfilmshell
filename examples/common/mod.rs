//! What the examples share: running one.
//!
//! The examples read a film saved as a folder of chunk files with
//! `Film::from_folder`. A downloader that has the film's manifest instead
//! maps each of its entries (an `Index`, a `ChunkType`: 1 header, 2 replay,
//! 3 highlights) to
//! `Chunk::from_manifest(entry.Index, entry.ChunkType, bytes)?`.

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
