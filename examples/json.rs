//! Writes a film's model as JSON, for a viewer in another language: the
//! header, the players, every life with its samples, the vehicles, the
//! events, the scoreboard and the rest, each under its accessor's name. The
//! samples make it large: about 100 MB for a 9-minute Big Team Battle match.
//!
//! ```text
//! cargo run --release --features serde --example json -- path/to/film > film.json
//! ```

mod common;

use filmshell::Film;

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: json <film folder>")?;
    let film = Film::from_folder(&dir)?;
    serde_json::to_writer(std::io::stdout().lock(), &film)?;
    Ok(())
}
