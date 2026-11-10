//! The match at one moment: where everyone was, their shield and health,
//! and what they held or rode.
//!
//! ```text
//! cargo run --release --example playback -- path/to/film 90
//! ```

mod common;

use filmshell::{Film, Tick};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or("usage: playback <film folder> <seconds>")?;
    let at = Tick::from_seconds(args.next().ok_or("give a time in seconds")?.parse()?);
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
        let percent = |v: Option<f64>| v.map_or("   ".to_string(), |v| format!("{:>3.0}", v * 100.0));
        let doing = match (now.vehicle, now.weapon) {
            (Some(v), _) => format!("in {}", v.vehicle().map_or("a vehicle", |v| v.name())),
            (None, Some(w)) => w.name().unwrap_or("").to_string(),
            (None, None) => String::new(),
        };
        println!("{name:<16} {place}  shield {}  health {}  {doing}", percent(s.shield), percent(s.health));
    }
    Ok(())
}
