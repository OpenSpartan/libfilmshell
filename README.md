<p align="center">
  <img src="https://raw.githubusercontent.com/OpenSpartan/libfilmshell/main/assets/icon.png" alt="" width="200">
</p>

<h1 align="center">libfilmshell</h1>

<p align="center">The library for parsing Halo Infinite film files.</p>

<p align="center">
  <a href="https://github.com/OpenSpartan/libfilmshell/actions/workflows/ci.yml"><img src="https://github.com/OpenSpartan/libfilmshell/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT license"></a>
  <img src="https://img.shields.io/badge/rust-1.85%2B-orange.svg" alt="Rust 1.85 or newer">
</p>

`libfilmshell` reads Halo Infinite film files into typed data: who played, where everyone went, every
kill, shot and medal, and the scoreboard as it changed. I first wrote about parsing them in
[Extracting Match Stats From Halo Infinite Film
Files](https://den.dev/blog/extracting-stats-film-files-halo-infinite/), and this library collects
those investigations in one package.

This is the library that powers [OpenSpartan Films](https://films.openspartan.com).

## Install

The crate is named `filmshell`:

```toml
[dependencies]
filmshell = { git = "https://github.com/OpenSpartan/libfilmshell" }

# Reading a film unoptimized takes about ten times as long, so optimize this crate in debug
# builds too.
[profile.dev.package.filmshell]
opt-level = 3
```

## Film chunks

Halo Infinite records every match for theater replay in **chunks** that are listed through the API.
A film has three kinds:

| Type | `ChunkKind`  | Where  | What's in it                                                         |
| ---- | ------------ | ------ | -------------------------------------------------------------------- |
| 1    | `Header`     | First  | Who played, details about the map, the game mode, and the game build |
| 2    | `Replay`     | Middle | The match itself                                                     |
| 3    | `Highlights` | Last   | Summary of kills, deaths, and medals on the theater timeline         |

Reading a film takes the header and every replay chunk. The highlights chunk is optional, but
without it a film has no medals, and a life whose owner the film doesn't name can't be matched to a
player by death times. You can download the chunks yourself with a script or a third-party tool,
using your own [Spartan token](https://den.dev/blog/halo-api-authentication/), including for
matches you didn't play in.

## Quick start

To extract film data, wrap each chunk in a `Chunk` and pass them all to `Film::from_chunks`:

```rust
use filmshell::{Chunk, Film};

/// `chunks`: (index, chunk type, bytes) for every chunk of one film, as its manifest lists them.
fn print_match(chunks: &[(u32, u32, Vec<u8>)]) -> Result<(), filmshell::Error> {
    let chunks = chunks
        .iter()
        .map(|(index, chunk_type, bytes)| Chunk::from_manifest(*index, *chunk_type, bytes.as_slice()))
        .collect::<Result<Vec<_>, _>>()?;
    let film = Film::from_chunks(chunks)?;

    let name = |id| film.player_name(id).unwrap_or("unknown");
    for player in film.players() {
        println!("{}", name(player.id));
    }
    // Leaves out the deaths the scoreboard doesn't count: those after a round is decided,
    // among them the deaths the game deals everyone alive when the match ends.
    for (tick, kill) in film.kills().filter(|(_, k)| k.is_scored()) {
        match kill.credited_killer() {
            Some(killer) => println!("{tick}: {} killed {}", name(killer), name(kill.victim)),
            None => println!("{tick}: {} died", name(kill.victim)),
        }
    }
    Ok(())
}
```

Chunks saved as files named `filmChunk{N}` (as downloaded) or `filmChunk{N}_dec` (inflated) need no
manifest: `Chunk::from_file(name, bytes)` takes the index from the file name and the kind from the
bytes, and `Film::from_folder(dir)` reads a folder of them. The [`examples` folder](examples) has
eight programs that read a film saved that way:

```sh
# The match, its players and the score
cargo run --release --example players -- path/to/film
# Every kill with its weapon
cargo run --release --example kills -- path/to/film
# One player's match, event by event
cargo run --release --example timeline -- path/to/film 3
# The kill feed one player saw, in the game's words
cargo run --release --example feed -- path/to/film 3
# The map 90 seconds in: every player, vehicle, grenade and weapon on the ground
cargo run --release --example playback -- path/to/film 90
# Every path and scored kill, drawn as a map
cargo run --release --example trails -- path/to/film > map.svg
# The packets and events underneath
cargo run --release --example raw -- path/to/film
# The model as JSON
cargo run --release --features serde --example json -- path/to/film > film.json
```

>[!NOTE]
>Positions are in world units, a foot each; the game's engine unit is ten world units. On the test films every level in
>the library's catalogue converts exactly; a film whose fields don't fit the catalogue's box (another game build) gets
>a fitted conversion, which `Film::coordinates` names. For any other level, give the level's bounds or the map's spawn
>points with `Hints` and `Film::from_chunks_with`, or read the unconverted coordinates from
>`Sample::raw`. A `Sample` records where a player was at a given tick, along with their aim, speed,
>health, and shield.

If you would like to read the raw contents of the film, `Film::raw()` exposes the layers the data
model is built from: inflated chunks, packets, frames, each frame's events and entity updates, and
the seek snapshots. It is mostly useful for working on the format itself; most applications you
will build with `libfilmshell` won't need it, and `filmshell::format` may change in any minor
release.

## License

MIT. See [LICENSE](LICENSE).

## Maintainers

- [Den Delimarsky](https://den.dev), with love for Halo and the Halo community