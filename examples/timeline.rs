//! Follows one player through a film: what they did, then where they were
//! once a second of each life.
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
    let name = |id: PlayerId| film.player(id).and_then(|p| p.name()).unwrap_or("someone");
    println!("{}", name(me));

    for e in film.events().iter().filter(|e| e.event.involves(me)) {
        let line = match &e.event {
            Event::Kill(k) if k.victim == me => match k.killer {
                Some(killer) if killer != me => format!("killed by {}", name(killer)),
                _ => "died".to_string(),
            },
            Event::Kill(k) if k.killer == Some(me) => format!("killed {}", name(k.victim)),
            Event::Kill(k) => format!("assisted on {}", name(k.victim)),
            Event::Pickup(p) => format!("picked up {}", p.item.name().unwrap_or("an item")),
            Event::Action(a) if a.kind == ActionKind::Reload => "reloaded".to_string(),
            Event::Medal(m) => format!("earned {}", m.medal().map_or("a medal", |m| m.name())),
            Event::VehicleExit(x) => {
                let vehicle = film.vehicle_at(x.vehicle, e.tick).and_then(|v| v.kind());
                format!("got out of {}", vehicle.map_or("a vehicle", |v| v.name()))
            }
            _ => continue,
        };
        println!("{}  {line}", e.tick);
    }

    for life in film.lives_of(me) {
        let (Some(start), Some(end)) = (life.start(), life.end()) else { continue };
        println!("life from {start} to {end}{}", if life.died { ", died" } else { "" });
        let mut t = start;
        while t <= end {
            let sample = life.at(t);
            t += Tick::PER_SECOND;
            let Some((s, p)) = sample.and_then(|s| Some((s, s.position?))) else { continue };
            let health = s.health.map_or(String::new(), |h| format!("  health {:.0}%", h * 100.0));
            println!("  {}  ({:.1}, {:.1}){health}", s.tick, p.x, p.y);
        }
    }
    Ok(())
}
