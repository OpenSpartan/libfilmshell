# Changelog

The crate follows [semantic versioning](https://semver.org). Before 1.0, a minor release may break
the API; the raw format layer (`filmshell::format`) may change in any minor release.

## 0.1.0 (unreleased)

First release.

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
- `game::Level` carries the level's English name where the game's map info gives one, and the
  services' level id (`MatchInfo.LevelId`); `game::level_by_api_id` finds a level by it.
- Features: `serde` derives `Serialize` and `Deserialize` (events tagged by `type`, ids as text,
  empty fields left out); `research` makes the internal readers public, outside the stability
  guarantees.
- A film whose chunks inflate past 1 GiB, or a chunk past 256 MiB, is refused. `Hints` ignore
  spawn points with coordinates that aren't finite or lie more than a million world units out,
  and level boxes that aren't finite or whose low corner isn't below their high one.
- Films older game builds recorded (`FilmMajorVersion` before 41) are refused with
  `Error::UnknownHeader`.
- `GameMode` tells Attrition and Elimination, Extraction and Escalation apart; `game::modes` lists
  their options, and Infection's and VIP's.
- Without HighlightEvents to time them (a human against bots, a human who never dies), lives go to
  the player the film names as controlling them and end in a death where that player let go,
  which agrees with the match stats' deaths; calibration falls back on bots' paths.
- A death the game sends no kill event for (a bot's, among others) is still a `Kill`, marked
  `inferred`: its killer is the player whose kills the scoreboard counts up then, else the attacker
  of the last hit. Against bots, every player's kills now equal the match stats'.
- `StatChange::round` is the round a scoreboard value is for, as the game sends it, and
  `Scoreboard::rounds`, `round_value` and `round_starts` work from it; a match's totals
  (`Scoreboard::total`) agree with the match stats in Attrition, Elimination and Grifball too.
- The `Hints` builder methods and `Tick::saturating_sub` are `#[must_use]`.
