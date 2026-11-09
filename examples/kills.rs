//! Lists a film's kills with the weapon or cause of each.
//!
//! ```text
//! cargo run --release --example kills -- path/to/film
//! ```

mod common;

use filmshell::game::{self, DamageCause};
use filmshell::{Film, PlayerId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: kills <film folder>")?;
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;
    let name = |id: PlayerId| film.player(id).and_then(|p| p.name()).unwrap_or("?").to_string();

    // The deaths everyone still alive takes when the match ends are kills
    // the scoreboard doesn't count.
    for (tick, kill) in film.kills().filter(|(_, k)| k.scored != Some(false)) {
        let killer = kill.killer.map_or("the world".to_string(), name);
        let how = match (kill.weapon.and_then(game::item), kill.cause) {
            (Some(item), _) => item.name().to_string(),
            (None, Some(DamageCause::Weapon) | None) => "a weapon".to_string(),
            (None, Some(cause)) => format!("{cause:?}").to_lowercase(),
        };
        println!("{tick}  {killer} killed {}, {how}", name(kill.victim));
    }
    Ok(())
}
