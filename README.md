<p align="center">
  <img src="assets/icon.png" alt="" width="200">
</p>

<h1 align="center">libfilmshell</h1>

<p align="center">Reads Halo Infinite theater films into Rust data.</p>

<p align="center">
  <a href="https://github.com/OpenSpartan/libfilmshell/actions/workflows/ci.yml"><img src="https://github.com/OpenSpartan/libfilmshell/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT license"></a>
  <img src="https://img.shields.io/badge/rust-1.85%2B-orange.svg" alt="Rust 1.85 or newer">
</p>

A theater film is a recording of a match: a header chunk, a run of replay chunks holding every tick the server sent, and a chunk of highlight events. `filmshell` takes those chunks and gives you the match as plain Rust types: who played, where everyone went, every kill, medal and shot, the scoreboard as it changed, and what each player pressed.

It does no I/O. You hand it the bytes, however you got them.

## Install

```toml
[dependencies]
filmshell = { git = "https://github.com/OpenSpartan/libfilmshell" }
```

## Quick start

The Halo API lists a film's chunks with an index and a chunk type (1 header, 2 replay, 3 highlights). Pass each one as a `Chunk`, compressed as downloaded or already inflated, in any order:

```rust
use filmshell::{Chunk, ChunkKind, Film};

/// `chunks`: (index, chunk type, bytes) for every chunk of one film.
fn print_match(chunks: &[(u32, u32, Vec<u8>)]) -> Result<(), filmshell::Error> {
    let film = Film::from_chunks(chunks.iter().map(|(index, chunk_type, bytes)| {
        let kind = ChunkKind::from_manifest(*chunk_type).expect("a known chunk type");
        Chunk::new(*index, kind, bytes.as_slice())
    }))?;

    for player in film.players() {
        println!("{}", player.name().unwrap_or("unknown"));
    }
    for (tick, kill) in film.kills() {
        println!("{tick}: {:?} killed {}", kill.killer, kill.victim);
    }
    Ok(())
}
```

The `examples` folder has three small programs that read a film saved as a folder of chunk files:

```sh
cargo run --release --example players -- path/to/film
cargo run --release --example kills -- path/to/film
cargo run --release --example timeline -- path/to/film 3
```

## What you get

- **The match**: the game build, when the film starts, the map variant, the level, the game variant and its settings.
- **Players**: gamertags and XUIDs, bots, teams, who joined late and who left.
- **Lives**: every life with its positions over time, aim, speed, health and shield, and the vehicles it rode.
- **Events**, in tick order: kills with the damage each player dealt, medals, shots, hits, pickups, grenades, melee, abilities, teleports, vehicle exits, voice lines, HUD messages, objectives and the mode's scoring events.
- **Scoreboard**: every stat change as it happened, the match clock and rounds.
- **Player state**: movement input, ammunition, weapon heat, grenades and equipment.
- **The world**: vehicles and their tracks, weapons on the ground, grenades in flight, zones and hills.

The `game` module knows the game's own catalogue: weapons, vehicles, grenades, equipment, medals, damage effects and level bounds, as Rust enums and tables. The `analysis` module derives estimates from the data, such as accuracy.

## Positions

Positions are in world units, as the game uses them. They convert exactly on every level the library knows. For any other level, give the level's bounds or the map's spawn points with `Hints` and `Film::from_chunks_with`, or use the raw quantized values each sample keeps.

## The raw format

`Film::raw()` exposes what the data model is built from: the inflated chunks, packets, frames, each frame's events and entity updates, the seek snapshots and a bit reader. It is for working on the format itself; most programs won't need it.

## License

MIT. See [LICENSE](LICENSE).
