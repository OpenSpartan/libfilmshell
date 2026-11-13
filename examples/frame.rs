//! Draws the map a number of seconds into the match as an SVG, as a viewer
//! would draw one frame of playback: every player with the way they aim,
//! every vehicle as its box turned to its heading, the grenades and other
//! projectiles in the air and the weapons on the ground, over faint trails of
//! the whole match.
//!
//! ```text
//! cargo run --release --example frame -- path/to/film 90 > frame.svg
//! ```

mod common;

use filmshell::{Film, Position, TeamId, Tick, game};
use std::fmt::Write;

const SIZE: f64 = 1000.0;
const TEAMS: [&str; 4] = ["#3b82f6", "#ef4444", "#f59e0b", "#22c55e"];
/// How far an aim line reaches, world units.
const AIM: f64 = 30.0;

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or("usage: frame <film folder> <seconds>")?;
    let seconds = args.next().ok_or("give a time in seconds")?;
    let seconds: f64 = seconds.parse().map_err(|_| format!("{seconds} is not a number of seconds"))?;
    let film = Film::from_folder(&dir)?;
    if film.coordinates().is_none() {
        return Err("the film's positions can't be placed on its map".into());
    }
    // Seconds into the match, from when its clock first ran.
    let at = film.match_start().unwrap_or_default() + Tick::from_seconds(seconds).get();

    // World x and y to the SVG's pixels, +y up, over every position of the match.
    let all: Vec<Position> = film.lives().iter().flat_map(|l| l.samples.iter().filter_map(|s| s.position)).collect();
    if all.is_empty() {
        return Err("the film has no positions".into());
    }
    let (x0, x1) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (y0, y1) = all.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    let scale = SIZE / (x1 - x0).max(y1 - y0).max(1.0);
    let px = |x: f64, y: f64| ((x - x0) * scale, (y1 - y) * scale);
    let colour = |team: Option<TeamId>| team.map_or("#9ca3af", |t| TEAMS[t.0 as usize % TEAMS.len()]);

    let mut svg = String::new();
    let (w, h) = ((x1 - x0) * scale, (y1 - y0) * scale);
    // Room on the right for the names of players at the edge.
    let (vw, vh) = (w + 110.0, h + 40.0);
    writeln!(svg, r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-10 -30 {vw} {vh}" font-family="sans-serif">"#)?;
    writeln!(svg, r##"<rect x="-10" y="-30" width="{vw}" height="{vh}" fill="#111827"/>"##)?;
    writeln!(
        svg,
        r##"<text x="0" y="-12" fill="#e5e7eb" font-size="14">{seconds} s into the match ({at} of the film)</text>"##
    )?;

    // The whole match, faintly: where the map is.
    for life in film.lives() {
        let points: Vec<String> = life
            .samples
            .iter()
            .filter_map(|s| s.position)
            .step_by(8)
            .map(|p| {
                let (x, y) = px(p.x, p.y);
                format!("{x:.0},{y:.0}")
            })
            .collect();
        if points.len() > 1 {
            writeln!(
                svg,
                r##"<polyline points="{}" fill="none" stroke="#374151" stroke-width="1"/>"##,
                points.join(" ")
            )?;
        }
    }

    // Weapons on the ground.
    for g in film.ground_weapons_at(at) {
        let (x, y) = px(g.position.x, g.position.y);
        writeln!(svg, r##"<rect x="{:.1}" y="{:.1}" width="4" height="4" fill="#6b7280"/>"##, x - 2.0, y - 2.0)?;
    }

    // Vehicles: the render model's box, in engine units with x forward and y
    // to the left, turned to the heading. A ridden one takes the colour of
    // its first rider's team.
    let players: Vec<_> = film.players_at(at).collect();
    for (track, pos) in film.vehicles_at(at) {
        let (Some(bounds), Some(heading)) = (game::vehicle_bounds(track.tag), track.heading_at(at)) else { continue };
        let team =
            players.iter().find(|p| p.vehicle.is_some_and(|v| std::ptr::eq(v, track))).and_then(|p| p.player.team);
        let (forward, left) = ((heading.cos(), heading.sin()), (-heading.sin(), heading.cos()));
        let corners: Vec<String> = [(0, 0), (1, 0), (1, 1), (0, 1)]
            .iter()
            .map(|&(i, j)| {
                let (f, l) = (10.0 * [bounds.min[0], bounds.max[0]][i], 10.0 * [bounds.min[1], bounds.max[1]][j]);
                let (x, y) = px(pos.x + f * forward.0 + l * left.0, pos.y + f * forward.1 + l * left.1);
                format!("{x:.1},{y:.1}")
            })
            .collect();
        writeln!(
            svg,
            r##"<polygon points="{}" fill="{}" fill-opacity="0.35" stroke="{}"/>"##,
            corners.join(" "),
            colour(team),
            colour(team)
        )?;
    }

    // Grenades and other projectiles in the air.
    for (_, p) in film.grenades_at(at) {
        let (x, y) = px(p.x, p.y);
        writeln!(svg, r##"<circle cx="{x:.1}" cy="{y:.1}" r="3" fill="#f97316"/>"##)?;
    }
    for (_, p) in film.projectiles_at(at) {
        let (x, y) = px(p.x, p.y);
        writeln!(svg, r##"<circle cx="{x:.1}" cy="{y:.1}" r="1.5" fill="#f9fafb"/>"##)?;
    }

    // Players: a dot where they are and a line the way they aim; a rider's
    // dot sits on their vehicle.
    for p in &players {
        let Some(pos) = p.position else { continue };
        let (x, y) = px(pos.x, pos.y);
        let c = colour(p.player.team);
        if let Some(yaw) = p.yaw.filter(|_| p.vehicle.is_none()) {
            let (ax, ay) = px(pos.x + AIM * yaw.cos(), pos.y + AIM * yaw.sin());
            writeln!(
                svg,
                r#"<line x1="{x:.1}" y1="{y:.1}" x2="{ax:.1}" y2="{ay:.1}" stroke="{c}" stroke-width="1.5"/>"#
            )?;
        }
        let r = if p.vehicle.is_some() { 2.5 } else { 4.5 };
        writeln!(svg, r##"<circle cx="{x:.1}" cy="{y:.1}" r="{r}" fill="{c}" stroke="#f9fafb" stroke-width="1"/>"##)?;
        let name = p.player.name().unwrap_or("unnamed");
        writeln!(
            svg,
            r##"<text x="{:.1}" y="{:.1}" fill="#d1d5db" font-size="10">{}</text>"##,
            x + 6.0,
            y - 6.0,
            escape(name)
        )?;
    }
    svg.push_str("</svg>\n");
    print!("{svg}");
    Ok(())
}

/// A gamertag as SVG text.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
