//! Walks the event stream of one player, and samples their path once a
//! second.
//!
//! ```text
//! cargo run --release --example timeline -- path/to/film 3
//! ```

mod common;

use filmshell::{ActionKind, Event, Film, PlayerId, Tick};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or("usage: timeline <film folder> <player index>")?;
    let me = PlayerId(args.next().ok_or("give a player index")?.parse()?);
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;

    for e in film.events() {
        let line = match &e.event {
            Event::Kill(k) if k.victim == me => "died".to_string(),
            Event::Kill(k) if k.killer == Some(me) => format!("killed {}", k.victim),
            Event::Pickup(p) if p.player == me => format!("picked up {}", p.item),
            Event::Action(a) if a.player == me && a.kind == ActionKind::Reload => "reloaded".to_string(),
            Event::Medal(m) if m.player == me => format!("earned {}", m.medal().map_or("a medal", |m| m.name())),
            _ => continue,
        };
        println!("{}  {line}", e.tick);
    }

    for life in film.lives_of(me) {
        let (Some(start), Some(end)) = (life.start(), life.end()) else { continue };
        println!("life from {start} to {end}{}", if life.died { ", died" } else { "" });
        for second in (start.0..=end.0).step_by(Tick::PER_SECOND as usize) {
            let Some(s) = life.at(Tick(second)) else { continue };
            if let Some(p) = s.position {
                println!("  {}  ({:.1}, {:.1})  health {:.2}", s.tick, p.x, p.y, s.health.unwrap_or(f64::NAN));
            }
        }
    }
    Ok(())
}
