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
        let Some(s) = film.sample_at(p.id, at) else {
            println!("{name:<16} not on the map");
            continue;
        };
        let place = s.position.map_or("unknown place".to_string(), |w| format!("{:7.1} {:7.1}", w.x, w.y));
        let percent = |v: Option<f64>| v.map_or("   ".to_string(), |v| format!("{:>3.0}", v * 100.0));
        let doing = if s.in_vehicle {
            let vehicle = s.vehicle.and_then(|v| film.vehicle_at(v, at)).and_then(|v| v.kind());
            format!("in {}", vehicle.map_or("a vehicle", |v| v.name()))
        } else {
            film.weapon_at(p.id, at).and_then(|w| w.name()).unwrap_or("").to_string()
        };
        println!("{name:<16} {place}  shield {}  health {}  {doing}", percent(s.shield), percent(s.health));
    }
    Ok(())
}
