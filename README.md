# libfilmshell

A Rust library that reads Halo Infinite theater films into a typed data
model: who played, every life with its positions and vitals, vehicles and
what stood on the map, one stream of events, the scoreboard, and each
player's inputs and ammunition.

The library does no I/O and needs no network: give it a film's chunks and it
returns the model. It compiles for `wasm32-unknown-unknown`.

## Install

```toml
[dependencies]
filmshell = { git = "https://github.com/OpenSpartan/libfilmshell" }
```

Enable the `serde` feature to derive `serde::Serialize` on the model types.

## Quick start

A film's manifest lists its chunks, each with an index and a chunk type: 1
for the header, 2 for the replay chunks, 3 for the HighlightEvents chunk.
Each becomes a `Chunk`; the bytes can be compressed as downloaded or already
inflated.

```rust
use filmshell::{Chunk, ChunkKind, Film};

let chunks = manifest.iter().map(|entry| {
    let kind = ChunkKind::from_manifest(entry.chunk_type).expect("a known chunk type");
    Chunk::new(entry.index, kind, download(&entry.path))
});
let film = Film::from_chunks(chunks)?;

for player in film.players() {
    println!("{} {}", player.id, player.name().unwrap_or("?"));
}
for (tick, kill) in film.kills() {
    println!("{tick} {:?} killed {} with {:?}", kill.killer, kill.victim, kill.weapon);
}
```

The examples read a folder of `filmChunk{N}` or `filmChunk{N}_dec` files:

```text
cargo run --release --example players -- path/to/film
cargo run --release --example kills -- path/to/film
cargo run --release --example timeline -- path/to/film 3
```

## What it reads

- **Header**: the game build, when the film starts, the map variant's ids
  and the level.
- **Clock**: frames, game ticks, the tick of every HighlightEvents time.
- **Players**: people (XUID, gamertag, team) and bots (id, name), who joined
  late and who left.
- **Game variant**: name, description, the mode, respawn and scoring
  settings, loadout, flags, stored options with their names and the mode
  scripts' defaults, stat definitions, script names.
- **Lives**: each biped life's owner and samples: raw and world position,
  height, aim yaw, speed, health, shield, the vehicle it rode.
- **Vehicles** and their tracks, **rides**, what stood on the map from the
  seek snapshots, weapons on the ground, grenades in flight, zones and hills.
- **Events**: kills (killer, assistant, damage shares, cause, weapon),
  medals, shots, hits with damage, pickups, throws, detonations, reloads,
  melee, zoom, abilities, jumps, weapon switches, teleports, vehicle exits,
  equipment uses, voice lines, unit chatter, HUD messages, the mode scripts'
  game events, flag and zone events, respawn countdowns, headshots.
- **Scoreboard**: every statborg change, the match clock, the engine's
  phase and lives left at each seek snapshot.
- **Player state**: movement input, ammunition, energy and heat per weapon
  slot, grenades carried, equipment charge.

The `game` module holds what the library knows about the game itself, read
from the game's files: weapons, vehicles, grenades, equipment, medals,
damage effects, levels and their position boxes, variant option names. The
`analysis` module derives estimates: accuracy, weapon numbers, spawn
factors.

A film doesn't carry everything. For a level outside the catalogue,
positions convert only when `Hints` give the level's box or the map's spawn
points; otherwise samples keep their raw values.

## The format layer

`Film::raw()` exposes the layers the model is built from: the inflated
chunks, the packets of the replay chunks, frames (tick packets), each
frame's event view and entity view, the seek snapshots, the component
registry, and a bit reader for the MSB-first bit-packed payloads.

## License

MIT
