//! Lists a film's kills with the weapon or cause of each.
//!
//! ```text
//! cargo run --release --example kills -- path/to/film
//! ```

mod common;

use filmshell::{Film, Kill, PlayerId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: kills <film folder>")?;
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;
    let name = |id: PlayerId| film.player(id).and_then(|p| p.name()).unwrap_or("someone");

    // The deaths everyone still alive takes when the match ends are kills
    // the scoreboard doesn't count.
    for (tick, kill) in film.kills().filter(|(_, k)| k.scored != Some(false)) {
        let victim = name(kill.victim);
        let line = match kill.killer {
            Some(killer) if killer != kill.victim && !kill.suicide => {
                format!("{} killed {victim} with {}", name(killer), how(kill))
            }
            _ => format!("{victim} died to {}", how(kill)),
        };
        let headshot = if kill.headshot == Some(true) { ", headshot" } else { "" };
        println!("{tick}  {line}{headshot}");
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
