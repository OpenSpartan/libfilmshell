# Changelog

The crate follows [semantic versioning](https://semver.org). Before 1.0, a minor release may break
the API; the raw format layer (`filmshell::format`) may change in any minor release.

## Unreleased

The first release.

- `Film::from_chunks` reads a film from its chunks, compressed or inflated, into a typed model:
  players and their teams (bots' included), lives with positions and vitals, vehicles and rides,
  what stood on the map, one event stream (kills, shots, hits, pickups, medals, voice lines,
  objectives), the scoreboard, and each player's inputs, weapons and ammunition.
- Playback lookups: `Film::sample_at`, `Film::samples_at`, `Film::weapon_at`, `Film::vehicle_at`.
- `filmshell::game` holds the game's own tables: weapons, vehicles, medals, damage effects, levels
  and their quantization boxes, mode options.
- `filmshell::format` exposes the raw layers, with the game's names for every event and entity
  type.
- `filmshell::analysis` estimates what films don't state: accuracy, weapon numbers, spawn factors.
- A film whose chunks inflate past 1 GiB, or a chunk past 256 MiB, is refused. `Hints` leave out
  spawn points and level boxes whose coordinates aren't finite.
