# Changelog

The crate follows [semantic versioning](https://semver.org). Before 1.0, a minor release may break
the API; the raw format layer (`filmshell::format`) may change in any minor release.

## Unreleased

The first release, 0.1.0.

- `Film::from_chunks` reads a film from its chunks, compressed as downloaded or inflated, into a
  typed model: who played, bots included, and on which team; every life with its positions, aim,
  speed, health and shield; vehicles and who rode them; what stood on the map; one stream of events
  (kills, shots, hits, pickups, medals, voice lines, objectives); the scoreboard; and each player's
  inputs, weapons and ammunition. Films recorded before film version 41 are refused
  (`Error::UnknownHeader`).
- Every death is a `Kill` with its killer, cause and weapon, including the deaths the game sends no
  kill event for, a bot's (`Kill::inferred`). Those are credited as the game credits them: the kill
  its scoreboard counted, else its rule (the fatal hit's attacker when an enemy, else the last enemy
  to hit the victim in the 6 s before), with the assistant by its rule (a fifth of the victim's
  shield and health).
- A life belongs to the player the film names as controlling it, else to the player whose death
  times in the highlights chunk it matches; none starts before its player joined or after they left.
  A player letting go of a life is a death only when the scoreboard counts one then, so a bot
  removed to make room for a joiner doesn't die. On the test films every player's kills of other
  players and deaths, the bots' included, equal the match stats' (which stop counting a player whose
  scoreboard row a reconnect clears).
- `Player::teams` and `Player::team_at` give a player's team over the match, as the game replicates
  it: Infection moves a survivor to the infected when killed, and some minigames change teams
  between rounds.
- The scoreboard has every value the game sends, with the round it is for (`StatChange::round`);
  `Scoreboard::total_stat` adds the rounds up, as the match stats do. The match clock's state is the
  index of its rate in the game's table: `MatchClock::rate`, `is_stopped`, `is_counting_down` and
  `is_counting_up`.
- Playback: `Film::players_at` gives every living player at a tick with their life, sample, weapon
  and its rounds, charge or heat, grenades, the objective they carry, ride and vehicle; `life_at`,
  `sample_at`, `weapon_at`, `ride_at`, `vehicle_at` and `events_in` look up one thing, and
  `Life::position_at` places a player between samples. The rest of the map at a tick: `vehicles_at`
  (a parked vehicle stays where it stopped until the game deletes it, `VehicleTrack::removed`),
  `grenades_at`, `projectiles_at`, `ground_weapons_at` and `world_objects_at`; `Zone::state_at` and
  `HillScore::points_at`.
- A ride ends when its vehicle is deleted, and with the rider's life: a driver killed with their
  tank stays seated in the wreck, but the ride stops at the death.
- Damaged chunks never panic: the tests read real films with flipped bits, cut-short chunks and
  stretches of noise, and each one reads or is refused.
- `GameMode` names the mode a film's scripts run, Attrition and Elimination, Extraction, Escalation,
  Infection and Stockpile among them; `game::modes` lists each mode's options and their defaults.
  Capture the Flag's flag events and Big Team Stockpile's power seed events (steals, deposits,
  carriers killed) are in the event stream, named by the modes' scripts.
- `PlayerAt::carrying` and `PlayerState::carrying`: the flag, the skull, Grifball's ball or a power
  seed a player carries, from the item the game puts in the carrier's weapon slots. On the test
  films each player's time carrying is the match stats' time as carrier, within a second for 151 of
  155.
- Events name players by engine slot, which joins and leaves can set apart from player ids: the
  mode scripts' events, and the player index in a shot's, a hit's or a throw's body, which is what
  names the player of one fired from a vehicle. Every reader maps them to player ids. On the test
  films each player's flag events equal the match stats' (1,338 counts).
- `Film::projectiles` and `Film::projectiles_at`: every projectile but thrown grenades, from where
  it was created, with the player who fired it, or who rode the vehicle or turret that did.
- `GameEvent::sequence`, the events' sequence number. An event the game sent again, which a film can
  record twice a few ticks apart, is kept once.
- `Event::VehicleDestroyed` and `Event::Hijack`: the personal scores the modes' scripts give for
  destroying a vehicle of another team and for hijacking one. A destroy names the vehicle's track
  (the vehicle of its type hit last), the damage and the riders killed with it; a hijack the
  hijacker's ride and the player thrown out. On the test films each player's hijacks equal the match
  stats'; the match stats count vehicle destroys by another rule.
- `filmshell::game` holds the game's own tables: weapons, vehicles, medals, damage effects, and
  levels with their English names, the services' level ids and their quantization boxes.
- `filmshell::format` exposes the raw layers, with the game's names for every event and entity type;
  `filmshell::analysis` estimates what films don't state: accuracy, weapon numbers, spawn factors.
- A film whose chunks inflate past 1 GiB, or a chunk past 256 MiB, is refused. `Hints` ignore spawn
  points with coordinates that aren't finite or lie more than a million world units out, and level
  boxes that aren't finite, whose low corner isn't below their high one, or that span a million
  engine units or more.
- Features: `serde` derives `Serialize` and `Deserialize` (events tagged by `type`, ids as text,
  empty fields left out); `research` makes the internal readers public, outside the stability
  guarantees.