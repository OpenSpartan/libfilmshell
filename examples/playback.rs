//! The match at one moment: where everyone was, their shield and health,
//! what they held, with its rounds, charge or heat, or rode, and what
//! objective they carried; then the vehicles, the grenades and other
//! projectiles in the air and the weapons on the ground.
//!
//! ```text
//! cargo run --release --example playback -- path/to/film 90
//! ```

mod common;

use filmshell::{Carried, Film, Tick};

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or("usage: playback <film folder> <seconds>")?;
    let seconds = args.next().ok_or("give a time in seconds")?;
    let at = Tick::from_seconds(seconds.parse().map_err(|_| format!("{seconds} is not a number of seconds"))?);
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;
    println!("{at} of {}", film.clock().end());

    for p in film.players() {
        let name = p.name().unwrap_or("unnamed");
        let Some(now) = film.player_at(p.id, at) else {
            println!("{name:<20} not on the map");
            continue;
        };
        let s = now.sample;
        let place =
            now.life.position_at(at).map_or("unknown place".to_string(), |w| format!("{:7.1} {:7.1}", w.x, w.y));
        // In small lobbies the library doesn't read shield or health.
        let percent =
            |label: &str, v: Option<f64>| v.map_or(String::new(), |v| format!("  {label} {:>3.0}", v * 100.0));
        let doing = match (now.vehicle, now.weapon) {
            (Some(v), _) => format!("in {}", v.vehicle().map_or("a vehicle", |v| v.name())),
            // An objective's item is shown as what is carried.
            (None, Some(w)) if Carried::from_item(w).is_some() => String::new(),
            (None, Some(w)) => {
                // An energy weapon's charge, else its rounds, else its heat.
                let rounds = match (now.ammo.map(|a| (a.loaded, a.reserve)), now.energy, now.heat) {
                    (_, Some(e), _) => format!(" charge {}%", e.percent),
                    (Some((Some(loaded), Some(reserve))), ..) if loaded + reserve > 0 => format!(" {loaded}/{reserve}"),
                    (_, _, Some(h)) => format!(" heat {}%", h.percent),
                    (Some((Some(loaded), _)), ..) => format!(" {loaded}"),
                    _ => String::new(),
                };
                format!("{}{rounds}", w.name().unwrap_or("a weapon"))
            }
            (None, None) => String::new(),
        };
        let vitals = percent("shield", s.shield) + &percent("health", s.health);
        let carrying = match now.carrying {
            Some(Carried::Flag) => "carrying the flag",
            Some(Carried::Skull) => "carrying the skull",
            Some(Carried::Ball) => "carrying the ball",
            Some(Carried::PowerSeed) => "carrying a power seed",
            _ => "",
        };
        let what = [doing.as_str(), carrying].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
        println!("{name:<20} {place}{vitals}  {what}");
    }

    for (v, at) in film.vehicles_at(at) {
        let name = v.vehicle().map_or("a vehicle", |v| v.name());
        println!("{name:<20} {:7.1} {:7.1}", at.x, at.y);
    }
    for (g, at) in film.grenades_at(at) {
        let name = g.grenade.map_or("a grenade", |g| g.name());
        let thrower = film.player_name(g.player).unwrap_or("someone");
        println!("{name:<20} {:7.1} {:7.1}  thrown by {thrower}", at.x, at.y);
    }
    for (p, at) in film.projectiles_at(at) {
        // A projectile's weapon: the weapon its direct-hit damage effect
        // belongs to, else what its shooter held when it was fired.
        let fired = p.path.first().map(|q| q.tick);
        let weapon = p
            .stats()
            .and_then(|s| filmshell::game::damage_effect(s.impact?)?.weapon)
            .or_else(|| film.weapon_at(p.player?, fired?));
        let name = weapon.and_then(|w| w.name()).unwrap_or("a projectile");
        let shooter = p.player.and_then(|p| film.player_name(p)).unwrap_or("no one");
        println!("{name:<20} {:7.1} {:7.1}  fired by {shooter}", at.x, at.y);
    }
    for w in film.ground_weapons_at(at) {
        let name = w.tag.name().unwrap_or("a weapon");
        println!("{name:<20} {:7.1} {:7.1}  on the ground", w.position.x, w.position.y);
    }
    Ok(())
}
