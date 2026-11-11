//! Prints a film's match, its players and the score.
//!
//! ```text
//! cargo run --release --example players -- path/to/film
//! ```

mod common;

use filmshell::{Film, Stat, StatOwner};

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: players <film folder>")?;
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;

    let variant = film.variant().map_or("unknown variant", |v| v.name.as_str());
    let level = film.level().map_or("unknown level", |l| l.title());
    println!("{variant} on {level}, {:.0} s", film.clock().end().seconds());

    // The scoreboard's values added up over every round.
    let board = film.scoreboard();
    for team in board.teams() {
        println!("team {team}: {}", board.total_stat(StatOwner::Team(team), Stat::Score));
    }
    for p in film.players() {
        let row = StatOwner::Player(p.id);
        println!(
            "{:>3}  {:<16} {:<4} {:<7} kills {:>3}  deaths {:>3}",
            p.id.0,
            p.name().unwrap_or("unnamed"),
            if p.is_bot() { "bot" } else { "" },
            p.team.map_or(String::new(), |t| format!("team {t}")),
            board.total_stat(row, Stat::Kills),
            board.total_stat(row, Stat::Deaths),
        );
    }
    Ok(())
}
