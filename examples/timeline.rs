//! Follows one player through a film: what they did, then where they were
//! once a second of each life.
//!
//! ```text
//! cargo run --release --example timeline -- path/to/film 3
//! ```
//!
//! The players example lists the players' ids.

mod common;

use filmshell::{ActionKind, Event, Film, FlagEventKind, GameEventArgument, PlayerId, SeedEventKind, Tick};

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or("usage: timeline <film folder> <player id>")?;
    let id = args.next().ok_or("give a player id")?;
    let me = PlayerId(id.parse().map_err(|_| format!("{id} is not a player id"))?);
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;
    if film.player(me).is_none() {
        return Err(format!("the film has no player {me}").into());
    }
    let name = |id: PlayerId| film.player_name(id).unwrap_or("someone");
    println!("{}", name(me));

    for e in film.events().iter().filter(|e| e.event.involves(me)) {
        let line = match &e.event {
            // The deaths the game deals everyone alive as a round ends count
            // nothing.
            Event::Kill(k) if k.victim == me && !k.is_scored() => "died, not counted".to_string(),
            Event::Kill(k) if k.victim == me => match k.credited_killer() {
                Some(killer) => format!("killed by {}", name(killer)),
                None => "died".to_string(),
            },
            Event::Kill(k) if k.killer == Some(me) => format!("killed {}", name(k.victim)),
            Event::Kill(k) => format!("assisted on {}", name(k.victim)),
            Event::Pickup(p) => format!("picked up {}", p.item.name().unwrap_or("an item")),
            Event::Action(a) if a.kind == ActionKind::Reload => "reloaded".to_string(),
            Event::Medal(m) => format!("earned {}", m.medal().map_or("a medal", |m| m.name())),
            Event::Flag(f) => match f.kind {
                FlagEventKind::Grab => "picked up the flag",
                FlagEventKind::Steal => "took the flag from its stand",
                FlagEventKind::Capture => "captured the flag",
                FlagEventKind::Return => "returned the flag",
                FlagEventKind::CarrierKilled => "killed the flag carrier",
                FlagEventKind::CaptureAssist => "assisted a capture",
                _ => continue,
            }
            .to_string(),
            Event::Seed(s) => match s.kind {
                SeedEventKind::Steal => "stole a power seed",
                SeedEventKind::Deposit => "deposited a power seed",
                SeedEventKind::CarrierKilled => "killed a power seed carrier",
                _ => continue,
            }
            .to_string(),
            Event::VehicleExit(x) => {
                let vehicle = film.vehicle_at(x.vehicle, e.tick).and_then(|v| v.vehicle());
                format!("got out of {}", vehicle.map_or("a vehicle", |v| v.name()))
            }
            Event::VehicleDestroyed(d) if d.player == me => match d.killed.len() {
                0 => format!("destroyed a {}", d.vehicle),
                n => format!("destroyed a {}, killing {n} riding it", d.vehicle),
            },
            Event::VehicleDestroyed(d) => format!("died in a {} destroyed by {}", d.vehicle, name(d.player)),
            Event::Hijack(h) if h.player == me => match h.from {
                Some(out) => format!("hijacked a {} from {}", h.vehicle, name(out)),
                None => format!("hijacked a {}", h.vehicle),
            },
            Event::Hijack(h) => format!("thrown out of a {} by {}", h.vehicle, name(h.player)),
            // The mode's own scoring, in the game's words, its players filled
            // in from the event's arguments.
            Event::GameEvent(g) => match film.game_response(g).and_then(|r| r.score) {
                Some((points, text)) if !text.is_empty() => {
                    let mut players = g.arguments.iter().filter_map(|a| match a {
                        GameEventArgument::Player(p) => Some(p.map_or("someone", name)),
                        _ => None,
                    });
                    let words: Vec<&str> = text
                        .split(' ')
                        .map(|w| if w.starts_with("%p") { players.next().unwrap_or("someone") } else { w })
                        .collect();
                    format!("{points:+} {}", words.join(" "))
                }
                _ => continue,
            },
            _ => continue,
        };
        println!("{}  {line}", e.tick);
    }

    for life in film.lives_of(me) {
        let (Some(start), Some(end)) = (life.start(), life.end()) else { continue };
        println!("life from {start} to {end}{}", if life.died { ", died" } else { "" });
        let mut t = start;
        while t <= end {
            let sample = life.sample_at(t);
            t += Tick::PER_SECOND;
            let Some((s, p)) = sample.and_then(|s| Some((s, s.position?))) else { continue };
            let health = s.health.map_or(String::new(), |h| format!("  health {:.0}%", h * 100.0));
            println!("  {}  ({:.1}, {:.1}){health}", s.tick, p.x, p.y);
        }
    }
    Ok(())
}
