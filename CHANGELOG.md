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
  kill event for, a bot's (`Kill::inferred`). Those are credited as the game credits them: the kill
  its scoreboard counted, else its rule (the fatal hit's attacker when an enemy, else the last enemy
  to hit the victim in the 6 s before), with the assistant by its rule (a fifth of the victim's
  shield and health). On the test films each player's kills of other players equal the match
  stats' for 646 of 647 players, and for all of them in matches against bots.
- A life belongs to the player the film names as controlling it, else to the player whose death
  times in the highlights chunk it matches; none starts before its player joined or after they
  left. A player letting go of a life is a death only when the scoreboard counts one then, so a
  bot removed to make room for a joiner doesn't die. Each player's deaths, the bots' included,
  equal the match stats' for 667 of 668 players on the test films.
- `Player::teams` and `Player::team_at` give a player's team over the match, as the game replicates
  it: Infection moves a survivor to the infected when killed, and some minigames change teams
  between rounds.
- The scoreboard has every value the game sends, with the round it is for (`StatChange::round`);
  `Scoreboard::total_stat` adds the rounds up, as the match stats do. The match clock's state is
  the index of its rate in the game's table: `MatchClock::rate`, `is_stopped`, `is_counting_down`
  and `is_counting_up`.
- Playback: `Film::players_at` gives every living player at a tick with their life, sample,
  weapon and its rounds, grenades, ride and vehicle; `life_at`, `sample_at`, `weapon_at`,
  `ride_at`, `vehicle_at` and `events_in` look up one thing, and `Life::position_at` places a
  player between samples.
- `GameMode` names the mode a film's scripts run, Attrition and Elimination, Extraction,
  Escalation, Infection and Stockpile among them; `game::modes` lists each mode's options and their
  defaults.
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