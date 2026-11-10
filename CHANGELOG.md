# Changelog

The crate follows [semantic versioning](https://semver.org). Before 1.0, a minor release may break
the API; the raw format layer (`filmshell::format`) may change in any minor release.

## Unreleased

The first release, 0.1.0.

- `Film::from_chunks` reads a film from its chunks, compressed as downloaded or inflated, into a
  typed model: who played, bots included, and on which team; every life with its positions, aim,
  speed, health and shield; vehicles and who rode them; what stood on the map; one stream of
  events (kills, shots, hits, pickups, medals, voice lines, objectives); the scoreboard; and each
  player's inputs, weapons and ammunition. Films recorded before film version 41 are refused
  (`Error::UnknownHeader`).
- Every death is a `Kill` with its killer, cause and weapon, including the deaths the game sends no
  kill event for, a bot's among them (`Kill::inferred`). On the test films each player's kills of
  other players equal the match stats' for 97% of players, and for all of them in matches against
  bots.
- A life belongs to the player the film names as controlling it, else to the player whose death
  times in the highlights chunk it matches; none starts before its player joined or after they
  left.
- The scoreboard has every value the game sends, with the round it is for (`StatChange::round`);
  `Scoreboard::total_stat` adds the rounds up, as the match stats do.
- Playback: `Film::life_at`, `sample_at`, `samples_at`, `weapon_at`, `ride_at`, `vehicle_at`.
- `GameMode` names the mode a film's scripts run, Attrition and Elimination, Extraction and
  Escalation among them; `game::modes` lists each mode's options and their defaults.
- `filmshell::game` holds the game's own tables: weapons, vehicles, medals, damage effects, and
  levels with their English names, the services' level ids and their quantization boxes.
- `filmshell::format` exposes the raw layers, with the game's names for every event and entity
  type; `filmshell::analysis` estimates what films don't state: accuracy, weapon numbers, spawn
  factors.
- A film whose chunks inflate past 1 GiB, or a chunk past 256 MiB, is refused. `Hints` ignore
  spawn points with coordinates that aren't finite or lie more than a million world units out, and
  level boxes that aren't finite, whose low corner isn't below their high one, or that span a
  million engine units or more.
- Features: `serde` derives `Serialize` and `Deserialize` (events tagged by `type`, ids as text,
  empty fields left out); `research` makes the internal readers public, outside the stability
  guarantees.