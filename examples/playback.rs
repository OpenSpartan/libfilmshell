//! The match at one moment: where everyone was, their shield and health,
//! and what they held, with its rounds, or rode.
//!
//! ```text
//! cargo run --release --example playback -- path/to/film 90
//! ```

mod common;

use filmshell::{Film, Tick};

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
            println!("{name:<16} not on the map");
            continue;
        };
        let s = now.sample;
        let place = s.position.map_or("unknown place".to_string(), |w| format!("{:7.1} {:7.1}", w.x, w.y));
        // Small lobbies' films send no shield or health.
        let percent =
            |label: &str, v: Option<f64>| v.map_or(String::new(), |v| format!("  {label} {:>3.0}", v * 100.0));
        let doing = match (now.vehicle, now.weapon) {
            (Some(v), _) => format!("in {}", v.vehicle().map_or("a vehicle", |v| v.name())),
            (None, Some(w)) => {
                let rounds = now.ammo.map_or(String::new(), |a| format!(" {}/{}", a.loaded, a.reserve));
                format!("{}{rounds}", w.name().unwrap_or("a weapon"))
            }
            (None, None) => String::new(),
        };
        let vitals = percent("shield", s.shield) + &percent("health", s.health);
        println!("{name:<16} {place}{vitals}  {doing}");
    }
    Ok(())
}
