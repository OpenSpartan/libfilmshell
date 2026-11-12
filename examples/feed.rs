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

use filmshell::{Event, Film, GameEventArgument, PlayerId, game};

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or("usage: feed <film folder> <player id>")?;
    let id = args.next().ok_or("give a player id")?;
    let me = PlayerId(id.parse().map_err(|_| format!("{id} is not a player id"))?);
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;
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
        println!("{}  {}", e.tick, fill(text, &g.arguments, &film));
    }
    Ok(())
}

/// A text with its markup filled in from the event's arguments, in order:
/// `%p`, `%pa`, `%pe` and `%ps` a player, `%s` a string, `%i` a number,
/// `&killfeed_<name>` an icon, shown as `[name]`. Colour (`%c` and a letter,
/// closed by `%`) and style (`%zy`) are left out.
fn fill(text: &str, arguments: &[GameEventArgument], film: &Film) -> String {
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
                        out.push_str(game::text(filmshell::StringId(value)).unwrap_or("?"))
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
