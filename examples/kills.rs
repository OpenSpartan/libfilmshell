//! Lists a film's kills with the weapon or cause of each.
//!
//! ```text
//! cargo run --release --example kills -- path/to/film
//! ```

mod common;

use filmshell::{Film, Kill, PlayerId};

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: kills <film folder>")?;
    let film = Film::from_folder(&dir)?;
    let name = |id: PlayerId| film.player_name(id).unwrap_or("someone");

    // Leaves out the deaths the scoreboard doesn't count: those after a round
    // is decided, among them the deaths the game deals everyone alive when the
    // match ends.
    let mut any = false;
    for (tick, kill) in film.kills().filter(|(_, k)| k.is_scored()) {
        any = true;
        let victim = name(kill.victim);
        let line = match kill.credited_killer() {
            Some(killer) => format!("{} killed {victim} with {}", name(killer), how(kill)),
            None => format!("{victim} died to {}", how(kill)),
        };
        let headshot = if kill.is_headshot() { ", headshot" } else { "" };
        println!("{tick}  {line}{headshot}");
    }
    if !any {
        // Firefight's AI aren't players: killing them is no kill here.
        println!("No player died.");
    }
    Ok(())
}

/// The weapon's name, else what dealt the last hit.
fn how(kill: &Kill) -> String {
    match (kill.weapon.and_then(|w| w.name()), kill.cause) {
        (Some(weapon), _) => weapon.to_string(),
        (None, Some(cause)) => cause.to_string(),
        (None, None) => "an unknown cause".to_string(),
    }
}
