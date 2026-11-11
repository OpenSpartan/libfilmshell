//! Writes a film's model as JSON: the players, every life with its samples,
//! the events and the scoreboard, for a viewer in another language. The
//! samples make it large: 64 MB for a 9-minute Big Team Battle match.
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
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;
    let out = serde_json::json!({
        "header": film.header(),
        "players": film.players(),
        "lives": film.lives(),
        "events": film.events(),
        "scoreboard": film.scoreboard(),
    });
    serde_json::to_writer(std::io::stdout().lock(), &out)?;
    Ok(())
}
