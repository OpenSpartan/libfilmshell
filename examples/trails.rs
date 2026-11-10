//! Draws every life's path and every kill as an SVG map, coloured by team.
//!
//! ```text
//! cargo run --release --example trails -- path/to/film > trails.svg
//! ```

mod common;

use filmshell::{Film, Position, TeamId};
use std::fmt::Write;

const SIZE: f64 = 1000.0;
const TEAMS: [&str; 4] = ["#3b82f6", "#ef4444", "#f59e0b", "#22c55e"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: trails <film folder>")?;
    let film = Film::from_chunks(common::load_folder(dir.as_ref())?)?;
    if film.coordinates().is_none() {
        return Err("the film's positions can't be placed on its map".into());
    }

    // World x and y to the SVG's pixels, north up.
    let all: Vec<Position> = film.lives().iter().flat_map(|l| l.samples.iter().filter_map(|s| s.position)).collect();
    if all.is_empty() {
        return Err("the film has no positions".into());
    }
    let (x0, x1) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (y0, y1) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    let scale = SIZE / (x1 - x0).max(y1 - y0).max(1.0);
    let px = |p: Position| ((p.x - x0) * scale, (y1 - p.y) * scale);

    let colour = |team: Option<TeamId>| team.map_or("#9ca3af", |t| TEAMS[t.0 as usize % TEAMS.len()]);
    let mut svg = String::new();
    let (w, h) = ((x1 - x0) * scale, (y1 - y0) * scale);
    writeln!(svg, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-10 -10 {} {}">"#, w + 20.0, h + 20.0)?;
    writeln!(svg, r##"<rect x="-10" y="-10" width="{}" height="{}" fill="#111827"/>"##, w + 20.0, h + 20.0)?;
    for life in film.lives() {
        let team = life.player.and_then(|p| film.player(p)).and_then(|p| p.team);
        // A point every pixel or so is enough to draw the path.
        let mut points: Vec<String> = Vec::new();
        let mut last: Option<(f64, f64)> = None;
        for p in life.samples.iter().filter_map(|s| s.position) {
            let (x, y) = px(p);
            if last.is_none_or(|(lx, ly)| (x - lx).hypot(y - ly) >= 1.0) {
                points.push(format!("{x:.1},{y:.1}"));
                last = Some((x, y));
            }
        }
        if points.len() > 1 {
            writeln!(
                svg,
                r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="1.2" stroke-opacity="0.55"/>"#,
                points.join(" "),
                colour(team),
            )?;
        }
    }
    for (_, kill) in film.kills() {
        let Some(p) = kill.position else { continue };
        let (x, y) = px(p);
        let team = film.player(kill.victim).and_then(|p| p.team);
        writeln!(
            svg,
            r#"<circle cx="{x:.1}" cy="{y:.1}" r="3" fill="none" stroke="{}" stroke-width="1.5"/>"#,
            colour(team)
        )?;
    }
    svg.push_str("</svg>\n");
    print!("{svg}");
    Ok(())
}
