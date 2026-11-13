//! The kill feed one player saw, in the game's words: the mode scripts'
//! kill feed lines sent to that player, their players, values and icons
//! filled in.
//!
//! ```text
//! cargo run --release --example feed -- path/to/film 3
//! ```
//!
//! The players example lists the players' ids.

mod common;

use filmshell::{Event, Film, PlayerId, game};

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or("usage: feed <film folder> <player id>")?;
    let id = args.next().ok_or("give a player id")?;
    let me = PlayerId(id.parse().map_err(|_| format!("{id} is not a player id"))?);
    let film = Film::from_folder(&dir)?;
    if film.player(me).is_none() {
        return Err(format!("the film has no player {me}").into());
    }
    println!("{}", film.player_name(me).unwrap_or("unnamed"));
    for e in film.events() {
        let Event::GameEvent(g) = &e.event else { continue };
        if !g.players.contains(me) || !film.game_response(g).is_some_and(|r| r.kill_feed) {
            continue;
        }
        let Some(text) = g.text.and_then(game::text) else { continue };
        println!("{}  {}", e.tick, film.fill_text(text, &g.arguments));
    }
    Ok(())
}
