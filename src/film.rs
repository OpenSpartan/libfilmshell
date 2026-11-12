use crate::analysis::AnalysisData;
use crate::chunk::is_zlib;
use crate::game::Level;
use crate::model::*;
use crate::{Chunk, ChunkKind, Error, Hints, TagId};

/// A theater film, read whole.
///
/// Built from the film's chunks with [`Film::from_chunks`]; everything the
/// library reads is computed then, so the accessors are cheap.
pub struct Film {
    pub(crate) raw: crate::container::Film,
    pub(crate) params: Option<crate::components::Params>,
    pub(crate) has_highlights: bool,
    pub(crate) encoding: crate::format::Encoding,
    pub(crate) header: Header,
    pub(crate) clock: Clock,
    pub(crate) players: Vec<Player>,
    pub(crate) variant: Option<GameVariant>,
    pub(crate) level: Option<&'static Level>,
    pub(crate) level_id: Option<TagId>,
    pub(crate) coordinates: Option<Coordinates>,
    pub(crate) lives: Vec<Life>,
    pub(crate) vehicles: Vec<VehicleTrack>,
    pub(crate) rides: Vec<Ride>,
    pub(crate) world: Vec<WorldObject>,
    pub(crate) ground: Vec<GroundWeapon>,
    pub(crate) grenades: Vec<GrenadeFlight>,
    pub(crate) projectiles: Vec<ProjectileFlight>,
    pub(crate) event_group: Option<&'static crate::game::events::EventGroup>,
    pub(crate) zones: Vec<Zone>,
    pub(crate) hill_score: Option<HillScore>,
    pub(crate) events: Vec<TimedEvent>,
    pub(crate) highlights: Vec<Highlight>,
    pub(crate) scoreboard: Scoreboard,
    pub(crate) states: Vec<(PlayerId, PlayerState)>,
    pub(crate) analysis: AnalysisData,
}

/// The most missing chunk indices [`Error::MissingChunks`] lists.
const MISSING_LISTED: usize = 16;

/// The film's model, each part under its accessor's name: `header`,
/// `variant`, `level_id`, `coordinates`, `match_start`, `frame_ticks` (the
/// clock: [`Clock::frame_ticks`]), `players`, `lives`, `rides`, `vehicles`,
/// `world_objects`, `ground_weapons`, `grenades`, `projectiles`, `zones`,
/// `hill_score`, `events`, `highlights`, `scoreboard` and `player_states`
/// (by player id). Serialize only: a film is read from its chunks.
#[cfg(feature = "serde")]
impl serde::Serialize for Film {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        struct States<'a>(&'a [(PlayerId, PlayerState)]);
        impl serde::Serialize for States<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_map(self.0.iter().map(|(p, st)| (p.0, st)))
            }
        }
        let mut f = s.serialize_struct("Film", 20)?;
        f.serialize_field("header", &self.header)?;
        f.serialize_field("variant", &self.variant)?;
        f.serialize_field("level_id", &self.level_id)?;
        f.serialize_field("coordinates", &self.coordinates)?;
        f.serialize_field("match_start", &self.match_start())?;
        f.serialize_field("frame_ticks", self.clock.frame_ticks())?;
        f.serialize_field("players", &self.players)?;
        f.serialize_field("lives", &self.lives)?;
        f.serialize_field("rides", &self.rides)?;
        f.serialize_field("vehicles", &self.vehicles)?;
        f.serialize_field("world_objects", &self.world)?;
        f.serialize_field("ground_weapons", &self.ground)?;
        f.serialize_field("grenades", &self.grenades)?;
        f.serialize_field("projectiles", &self.projectiles)?;
        f.serialize_field("zones", &self.zones)?;
        f.serialize_field("hill_score", &self.hill_score)?;
        f.serialize_field("events", &self.events)?;
        f.serialize_field("highlights", &self.highlights)?;
        f.serialize_field("scoreboard", &self.scoreboard)?;
        f.serialize_field("player_states", &States(&self.states))?;
        f.end()
    }
}

impl std::fmt::Debug for Film {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Film")
            .field("header", &self.header)
            .field("players", &self.players.len())
            .field("lives", &self.lives.len())
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl Film {
    /// Reads a film from its chunks, in any order, each compressed as
    /// downloaded or already inflated.
    ///
    /// The chunks must be numbered 0 to n without gaps: chunk 0 the header,
    /// then the replay chunks, then optionally the highlights chunk.
    /// Everything is read up front: in a release build about 2 s for a
    /// 9-minute Big Team Battle match of 27 players, 5 s for an 18-minute one
    /// of 35. Unoptimized, as in a debug build of a crate that depends on
    /// this one, it is about ten times slower; `[profile.dev.package.filmshell]
    /// opt-level = 3` in that crate's `Cargo.toml` avoids it. The crate's
    /// [Quick start](crate#quick-start) shows a film read from a manifest.
    ///
    /// # Errors
    ///
    /// When the chunks don't make up a film:
    ///
    /// - [`Error::NoChunks`]: none given.
    /// - [`Error::DuplicateChunk`], [`Error::MissingChunks`]: a repeat or a
    ///   gap in their indices.
    /// - [`Error::UnexpectedKind`]: a chunk 0 that isn't the header, a
    ///   header elsewhere, or highlights that aren't the last chunk.
    /// - [`Error::NoReplay`]: no replay chunk.
    /// - [`Error::Inflate`], [`Error::TooLarge`]: a chunk that doesn't
    ///   inflate, or one larger than [`MAX_CHUNK_BYTES`](crate::MAX_CHUNK_BYTES),
    ///   or chunks together larger than [`MAX_FILM_BYTES`](crate::MAX_FILM_BYTES).
    /// - [`Error::UnknownHeader`]: chunk 0 isn't a film header this library
    ///   reads, among them those of films before version 41.
    /// - [`Error::NoTicks`]: replay chunks that record no tick.
    pub fn from_chunks<'a>(chunks: impl IntoIterator<Item = Chunk<'a>>) -> Result<Film, Error> {
        Film::from_chunks_with(chunks, &Hints::default())
    }

    /// Reads the film saved as chunk files in folder `dir`
    /// ([`Chunk::read_folder`]).
    ///
    /// ```no_run
    /// let film = filmshell::Film::from_folder("path/to/film")?;
    /// println!("{} players", film.players().len());
    /// # Ok::<(), filmshell::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// As [`Chunk::read_folder`] and [`Film::from_chunks`].
    pub fn from_folder(dir: impl AsRef<std::path::Path>) -> Result<Film, Error> {
        Film::from_chunks(Chunk::read_folder(dir)?)
    }

    /// [`Film::from_chunks`] with facts the film lacks ([`Hints`]): a level
    /// box or spawn points for a level the catalogue lacks
    /// ([`Film::coordinates`] is None, or not [`CoordinateSource::LevelBox`]),
    /// a level id in place of the film's, and spawn points for
    /// [`analysis::spawn_factors`](crate::analysis::spawn_factors).
    ///
    /// # Errors
    ///
    /// As [`Film::from_chunks`].
    pub fn from_chunks_with<'a>(chunks: impl IntoIterator<Item = Chunk<'a>>, hints: &Hints) -> Result<Film, Error> {
        let mut chunks: Vec<Chunk<'a>> = chunks.into_iter().collect();
        if chunks.is_empty() {
            return Err(Error::NoChunks);
        }
        chunks.sort_by_key(|c| c.index);
        if let Some(w) = chunks.windows(2).find(|w| w[0].index == w[1].index) {
            return Err(Error::DuplicateChunk(w[0].index));
        }
        let last = chunks[chunks.len() - 1].index;
        if last as usize + 1 != chunks.len() {
            // The first gaps only: an index near u32::MAX would list billions.
            let mut missing = Vec::new();
            let mut next = 0;
            'gaps: for c in &chunks {
                while next < c.index {
                    if missing.len() == MISSING_LISTED {
                        break 'gaps;
                    }
                    missing.push(next);
                    next += 1;
                }
                next = c.index + 1;
            }
            return Err(Error::MissingChunks(missing));
        }
        for c in &chunks {
            let fits = match c.kind {
                ChunkKind::Header => c.index == 0,
                ChunkKind::Highlights => c.index == last && c.index > 0,
                ChunkKind::Replay => c.index > 0,
            };
            if !fits {
                return Err(Error::UnexpectedKind { index: c.index, kind: c.kind });
            }
        }
        if chunks.len() < 2 || chunks[1].kind != ChunkKind::Replay {
            return Err(Error::NoReplay);
        }
        let has_highlights = chunks[chunks.len() - 1].kind == ChunkKind::Highlights;
        let mut data = Vec::with_capacity(chunks.len());
        let mut total = 0usize;
        for c in chunks {
            let limit = crate::MAX_CHUNK_BYTES.min(crate::MAX_FILM_BYTES - total);
            let bytes = if is_zlib(&c.data) {
                crate::inflate::zlib(&c.data, limit).map_err(|f| match f {
                    crate::inflate::Fault::Malformed => Error::Inflate { index: c.index },
                    crate::inflate::Fault::TooLong => Error::TooLarge { index: c.index },
                })?
            } else if c.data.len() > limit {
                return Err(Error::TooLarge { index: c.index });
            } else {
                c.data.into_owned()
            };
            total += bytes.len();
            data.push(bytes);
        }
        if crate::header::read(&data[0]).build.is_none() {
            return Err(Error::UnknownHeader);
        }
        crate::assemble::assemble(data, has_highlights, hints)
    }

    /// What chunk 0's header says: the build, the start time, the map.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The film's clock: which tick each frame is, and where `HighlightEvents`
    /// times fall.
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// The tick the match started: when its clock first ran.
    ///
    /// Ticks count from the film's first frame. On the test films that frame
    /// comes 16 s to 2 minutes before the match starts, while players load
    /// in, and on most of them the first lives start within a few ticks of
    /// the match start. Time into the match is
    /// `tick - start` ticks, and the clock the game shows is
    /// [`Scoreboard::clock_at`]. None on a film without a match clock (on the
    /// test films, the small lobbies and a few others).
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film) {
    /// use filmshell::Tick;
    ///
    /// if let Some(start) = film.match_start() {
    ///     let two_minutes_in = start + 120 * Tick::PER_SECOND;
    ///     for p in film.players_at(two_minutes_in) {
    ///         println!("{}: {:?}", p.player.id, p.sample.position);
    ///     }
    /// }
    /// # }
    /// ```
    pub fn match_start(&self) -> Option<Tick> {
        self.scoreboard.match_clock.iter().find(|c| c.rate() != 0.0).map(|c| c.tick)
    }

    /// Everyone who played, in id order.
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// The player with id `id`, or None when no player has it.
    pub fn player(&self, id: PlayerId) -> Option<&Player> {
        match self.players.get(id.index()) {
            Some(p) if p.id == id => Some(p),
            _ => self.players.iter().find(|p| p.id == id),
        }
    }

    /// The gamertag or bot name of the player `id`: None when no player has
    /// the id, or the film carries no name for them.
    pub fn player_name(&self, id: PlayerId) -> Option<&str> {
        self.player(id)?.name()
    }

    /// The game variant; None when chunk 0's variant data doesn't parse.
    pub fn variant(&self) -> Option<&GameVariant> {
        self.variant.as_ref()
    }

    /// The mode the variant's scripts run.
    pub fn mode(&self) -> GameMode {
        self.variant.as_ref().map_or(GameMode::Other, GameVariant::mode)
    }

    /// The level the film is on, when the library's catalogue has it
    /// ([`game::levels`](crate::game::levels)). Positions convert with its
    /// box unless [`Hints::level_bounds`] gives another.
    pub fn level(&self) -> Option<&'static Level> {
        self.level
    }

    /// The `levl` tag of the level the film is on: the one
    /// [`Hints::level`] names, else the film's.
    pub fn level_id(&self) -> Option<TagId> {
        self.level_id
    }

    /// How raw positions map to world units, when the library could tell.
    pub fn coordinates(&self) -> Option<&Coordinates> {
        self.coordinates.as_ref()
    }

    /// Every biped life, players' and unowned, in the order of their first
    /// sample.
    pub fn lives(&self) -> &[Life] {
        &self.lives
    }

    /// One player's lives, in order.
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film, player: filmshell::PlayerId) {
    /// // How long the player was alive, in ticks: a life lasts to its
    /// // `until`, else to the film's end.
    /// let end = film.clock().end();
    /// let alive: i64 = film
    ///     .lives_of(player)
    ///     .filter_map(|l| Some(l.until.unwrap_or(end) - l.start()? + 1))
    ///     .sum();
    /// # }
    /// ```
    pub fn lives_of(&self, player: PlayerId) -> impl Iterator<Item = &Life> + '_ {
        self.lives.iter().filter(move |l| l.player == Some(player))
    }

    /// The life `player` is in at `tick`: the last to start at or before it,
    /// while it lasts ([`Life::until`]). None while the player isn't alive:
    /// before their first life, after a death until the next spawn, after
    /// they left, and past the film's end.
    ///
    /// Looks through the film's lives, so it costs O(lives) a call.
    pub fn life_at(&self, player: PlayerId, tick: Tick) -> Option<&Life> {
        if tick > self.clock.end() {
            return None;
        }
        self.lives_of(player)
            .take_while(|l| l.start().is_some_and(|s| s <= tick))
            .last()
            .filter(|l| !l.is_over_at(tick))
    }

    /// Where `player` was at `tick`: their last sample at or before it, in
    /// the life they are in then ([`Film::life_at`]).
    pub fn sample_at(&self, player: PlayerId, tick: Tick) -> Option<&Sample> {
        self.life_at(player, tick)?.sample_at(tick)
    }

    /// Whether `player` is alive at `tick`, by the rules of
    /// [`Film::sample_at`].
    pub fn alive_at(&self, player: PlayerId, tick: Tick) -> bool {
        self.sample_at(player, tick).is_some()
    }

    /// Every player alive at `tick` with their sample then, in id order:
    /// one frame of playback. O(players × lives).
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film) {
    /// use filmshell::Tick;
    ///
    /// let at = Tick::from_seconds(90.0);
    /// for (player, sample) in film.samples_at(at) {
    ///     if let Some(p) = sample.position {
    ///         println!("{player} at {:.1}, {:.1}", p.x, p.y);
    ///     }
    /// }
    /// # }
    /// ```
    pub fn samples_at(&self, tick: Tick) -> impl Iterator<Item = (PlayerId, &Sample)> + '_ {
        self.players.iter().filter_map(move |p| Some((p.id, self.sample_at(p.id, tick)?)))
    }

    /// Everything playback draws for `player` at `tick`: their life and
    /// sample then, the weapon in their hands with its rounds, charge or
    /// heat, their grenades, the objective they carry, and the ride and
    /// vehicle they're in. None while they aren't alive
    /// ([`Film::life_at`]). The sample is the last at or before `tick`;
    /// [`Life::position_at`] places the player between samples.
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film, id: filmshell::PlayerId) {
    /// use filmshell::Tick;
    ///
    /// match film.player_at(id, Tick::from_seconds(90.0)) {
    ///     Some(now) => match now.sample.position {
    ///         Some(p) => println!("at ({:.1}, {:.1})", p.x, p.y),
    ///         None => println!("alive, at a place the film can't convert"),
    ///     },
    ///     None => println!("not on the map"),
    /// }
    /// # }
    /// ```
    pub fn player_at(&self, player: PlayerId, tick: Tick) -> Option<PlayerAt<'_>> {
        let life = self.life_at(player, tick)?;
        let sample = life.sample_at(tick)?;
        let ride = self.ride_at(player, tick);
        let vehicle =
            ride.and_then(|r| self.ride_vehicle(r)).or_else(|| sample.vehicle.and_then(|v| self.vehicle_at(v, tick)));
        let state = self.player_state(player);
        let on_foot = state.filter(|_| !sample.in_vehicle);
        let weapon = on_foot.and_then(|s| s.weapon_at(tick));
        // Readings from before this life's spawn are an earlier life's.
        let start = life.start().unwrap_or(tick);
        let ammo = on_foot.and_then(|s| s.ammo_in(s.held_slot(tick)?, tick)).filter(|a| a.tick >= start);
        let energy = on_foot.and_then(|s| s.energy_in(s.held_slot(tick)?, tick)).filter(|e| e.tick >= start);
        let heat = on_foot.and_then(|s| s.heat_in(s.held_slot(tick)?, tick)).filter(|h| h.tick >= start);
        let grenades = state.and_then(|s| s.grenades_at(tick)).filter(|g| g.tick >= start);
        let carrying = state.and_then(|s| s.carrying(tick));
        let player = self.player(player)?;
        Some(PlayerAt { player, life, sample, weapon, ammo, energy, heat, grenades, carrying, ride, vehicle })
    }

    /// [`Film::player_at`] for every player alive at `tick`, in id order:
    /// one frame of playback. O(players × lives).
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film) {
    /// use filmshell::Tick;
    ///
    /// for p in film.players_at(Tick::from_seconds(90.0)) {
    ///     let holding = p.weapon.and_then(|w| w.name()).unwrap_or("nothing");
    ///     println!("{} holds {holding}", p.player.name().unwrap_or("someone"));
    /// }
    /// # }
    /// ```
    pub fn players_at(&self, tick: Tick) -> impl Iterator<Item = PlayerAt<'_>> + '_ {
        self.players.iter().filter_map(move |p| self.player_at(p.id, tick))
    }

    /// The players of `team`, in id order.
    pub fn players_on(&self, team: TeamId) -> impl Iterator<Item = &Player> + '_ {
        self.players.iter().filter(move |p| p.team == Some(team))
    }

    /// Every vehicle track, in the order of their first record.
    pub fn vehicles(&self) -> &[VehicleTrack] {
        &self.vehicles
    }

    /// The track of the vehicle `entity` names at `tick`, while that vehicle
    /// is on the map ([`VehicleTrack::on_map_at`]). Ids are reused, so the
    /// tick picks the vehicle.
    pub fn vehicle_at(&self, entity: EntityId, tick: Tick) -> Option<&VehicleTrack> {
        self.vehicles.iter().filter(|v| v.entity == entity && v.on_map_at(tick)).max_by_key(|v| v.span().map(|s| s.0))
    }

    /// Every vehicle on the map at `tick`, with where it was, in the order
    /// of [`Film::vehicles`]: driven, ridden or standing empty.
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film, now: filmshell::Tick) {
    /// for (track, at) in film.vehicles_at(now) {
    ///     let name = track.vehicle().map_or("a vehicle", |v| v.name());
    ///     println!("{name} at ({:.0}, {:.0})", at.x, at.y);
    /// }
    /// # }
    /// ```
    pub fn vehicles_at(&self, tick: Tick) -> impl Iterator<Item = (&VehicleTrack, Position)> + '_ {
        self.vehicles.iter().filter_map(move |v| Some((v, v.position_at(tick)?)))
    }

    /// Every ride, in tick order.
    pub fn rides(&self) -> &[Ride] {
        &self.rides
    }

    /// The ride `player` is on at `tick`, when they are in a vehicle.
    pub fn ride_at(&self, player: PlayerId, tick: Tick) -> Option<&Ride> {
        self.rides.iter().rfind(|r| r.player == player && r.from <= tick && tick <= r.to)
    }

    /// The track of the vehicle a ride is in, when it has one.
    pub fn ride_vehicle(&self, ride: &Ride) -> Option<&VehicleTrack> {
        self.vehicles.get(ride.vehicle?.0)
    }

    /// The rides of one life, in tick order: its player's rides that start
    /// within it.
    pub fn life_rides<'a>(&'a self, life: &'a Life) -> impl Iterator<Item = &'a Ride> + 'a {
        let span = life.start().zip(life.end());
        self.rides
            .iter()
            .filter(move |r| Some(r.player) == life.player && span.is_some_and(|(a, b)| a <= r.from && r.from <= b))
    }

    /// What stood on the map, from the seek snapshots. Empty unless the
    /// film's positions convert exactly ([`CoordinateSource::LevelBox`]).
    pub fn world_objects(&self) -> &[WorldObject] {
        &self.world
    }

    /// What the snapshots had on the map at `tick`
    /// ([`WorldObject::on_map_at`]).
    pub fn world_objects_at(&self, tick: Tick) -> impl Iterator<Item = &WorldObject> + '_ {
        self.world.iter().filter(move |o| o.on_map_at(tick))
    }

    /// Weapons that lay on the ground.
    pub fn ground_weapons(&self) -> &[GroundWeapon] {
        &self.ground
    }

    /// The weapons lying on the ground at `tick`
    /// ([`GroundWeapon::on_map_at`]).
    pub fn ground_weapons_at(&self, tick: Tick) -> impl Iterator<Item = &GroundWeapon> + '_ {
        self.ground.iter().filter(move |g| g.on_map_at(tick))
    }

    /// Thrown grenades' flights, in throw order.
    pub fn grenades(&self) -> &[GrenadeFlight] {
        &self.grenades
    }

    /// The grenades in the air at `tick`, with where each was
    /// ([`GrenadeFlight::position_at`]), in throw order.
    pub fn grenades_at(&self, tick: Tick) -> impl Iterator<Item = (&GrenadeFlight, Position)> + '_ {
        self.grenades.iter().filter_map(move |g| Some((g, g.position_at(tick)?)))
    }

    /// Every projectile's flight but thrown grenades' (those are
    /// [`Film::grenades`]), in the order they were fired.
    pub fn projectiles(&self) -> &[ProjectileFlight] {
        &self.projectiles
    }

    /// The projectiles in flight at `tick`, with where each was, in the order
    /// they were fired.
    pub fn projectiles_at(&self, tick: Tick) -> impl Iterator<Item = (&ProjectileFlight, Position)> + '_ {
        self.projectiles.iter().filter_map(move |p| Some((p, p.position_at(tick)?)))
    }

    /// The zones or hills of a zone mode.
    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }

    /// King of the Hill's points on the current hill. None outside King of
    /// the Hill, and on a King of the Hill film whose positions don't convert
    /// with the level's box or whose hill points aren't read.
    pub fn hill_score(&self) -> Option<&HillScore> {
        self.hill_score.as_ref()
    }

    /// What a mode script's game event is in this film's mode: its
    /// response in the mode's event tree, with the medal, personal score,
    /// announcer words and kill feed line it stands for. None when the
    /// library has no event tree for the film's scripts, the tree has no
    /// response for the id, the event's own text names another response, or
    /// the id is one the tree can't place for certain (inside a script's
    /// loop, or in Firefight's own files) and the event's text doesn't
    /// confirm it.
    /// The match stats also count
    /// personal scores the game gives no name, which have no response here:
    /// on the test films' Big Team Battle matches they are the whole
    /// difference between a player's named points and their stats' score
    /// for 91 of 95 players.
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film) {
    /// use filmshell::Event;
    ///
    /// for e in film.events() {
    ///     let Event::GameEvent(g) = &e.event else { continue };
    ///     if let Some((points, text)) = film.game_response(g).and_then(|r| r.score) {
    ///         println!("{} +{points} {text}", e.tick);
    ///     }
    /// }
    /// # }
    /// ```
    pub fn game_response(&self, event: &GameEvent) -> Option<&'static crate::game::GameResponse> {
        self.event_group?.response(event.id, event.text.map(|t| t.0))
    }

    /// Every event, in tick order.
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film) {
    /// use filmshell::{Event, EventKind};
    ///
    /// let shots = film.events().iter().filter(|e| e.event.kind() == EventKind::Shot).count();
    /// for e in film.events() {
    ///     if let Event::Hijack(h) = &e.event {
    ///         println!("{} hijacked a {}", e.tick, h.vehicle);
    ///     }
    /// }
    /// # }
    /// ```
    pub fn events(&self) -> &[TimedEvent] {
        &self.events
    }

    /// The events in a range of ticks, in tick order: `film.events_in(a..b)`
    /// leaves out tick `b`, `film.events_in(a..=b)` keeps it.
    ///
    /// Playback that steps from tick `last` to `now` shows what happened after
    /// `last`, `now` included:
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film, last: filmshell::Tick, now: filmshell::Tick) {
    /// use filmshell::Event;
    ///
    /// for e in film.events_in(last + 1..=now) {
    ///     if let Event::Kill(k) = &e.event {
    ///         println!("{} {} died", e.tick, film.player_name(k.victim).unwrap_or("someone"));
    ///     }
    /// }
    /// # }
    /// ```
    pub fn events_in(&self, range: impl std::ops::RangeBounds<Tick>) -> &[TimedEvent] {
        use std::ops::Bound;
        let a = match range.start_bound() {
            Bound::Included(&t) => self.events.partition_point(|e| e.tick < t),
            Bound::Excluded(&t) => self.events.partition_point(|e| e.tick <= t),
            Bound::Unbounded => 0,
        };
        let b = match range.end_bound() {
            Bound::Included(&t) => self.events.partition_point(|e| e.tick <= t),
            Bound::Excluded(&t) => self.events.partition_point(|e| e.tick < t),
            Bound::Unbounded => self.events.len(),
        };
        &self.events[a..b.max(a)]
    }

    /// Every kill, in tick order: the deaths the film records a kill event
    /// for and those it doesn't ([`Kill::inferred`]), the scoreboard's
    /// uncounted ones included ([`Kill::is_scored`] is false for those: the
    /// deaths after a round is decided, among them those the game deals
    /// everyone still alive when the match ends).
    ///
    /// Each weapon's scored kills:
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film) {
    /// use std::collections::BTreeMap;
    ///
    /// let mut by_weapon: BTreeMap<&str, u32> = BTreeMap::new();
    /// let scored = film.kills().filter(|(_, k)| k.is_scored() && k.credited_killer().is_some());
    /// for (_, kill) in scored {
    ///     let weapon = kill.weapon.and_then(|w| w.name()).unwrap_or("unknown");
    ///     *by_weapon.entry(weapon).or_default() += 1;
    /// }
    /// # }
    /// ```
    pub fn kills(&self) -> impl Iterator<Item = (Tick, &Kill)> + '_ {
        self.events.iter().filter_map(|e| match &e.event {
            Event::Kill(k) => Some((e.tick, k)),
            _ => None,
        })
    }

    /// Every medal the highlights chunk records for a player of the
    /// film, in its order, with the medal's tick. The tick is None when the
    /// highlight clock isn't known ([`Clock::highlight_zero`]); only medals
    /// with a tick are also in the event stream as [`Event::Medal`].
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film) {
    /// for (tick, award) in film.medals() {
    ///     let medal = award.medal().map_or("an unknown medal".to_string(), |m| m.to_string());
    ///     let name = film.player_name(award.player).unwrap_or("someone");
    ///     println!("{tick:?} {name} earned {medal}");
    /// }
    /// # }
    /// ```
    pub fn medals(&self) -> impl Iterator<Item = (Option<Tick>, MedalAward)> + '_ {
        self.highlights.iter().filter_map(|h| match h.kind {
            HighlightKind::Medal(index) => Some((h.tick, MedalAward { player: h.player?, index })),
            _ => None,
        })
    }

    /// The highlights chunk's records, in its order.
    pub fn highlights(&self) -> &[Highlight] {
        &self.highlights
    }

    /// The scoreboard over the match.
    pub fn scoreboard(&self) -> &Scoreboard {
        &self.scoreboard
    }

    /// What `player` pressed and carried over the match. None for a player
    /// whose biped updates the library doesn't read: in small lobbies (four
    /// players or fewer), and for a player who never spawned.
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film, player: filmshell::PlayerId, now: filmshell::Tick) {
    /// if let Some(state) = film.player_state(player) {
    ///     let input = state.input_at(now);
    ///     let grenades = state.grenades_at(now);
    ///     println!("{input:?} {grenades:?}");
    /// }
    /// # }
    /// ```
    pub fn player_state(&self, player: PlayerId) -> Option<&PlayerState> {
        self.states.iter().find(|s| s.0 == player).map(|s| &s.1)
    }

    /// The weapon in `player`'s hands at `tick`: the weapon in the slot they
    /// hold. None while they aren't alive ([`Film::sample_at`]), in a
    /// vehicle, where they fire the vehicle's weapons, and in small lobbies,
    /// whose weapon state isn't read. Measured on the test films, 99.6% of
    /// shots on foot come from this weapon.
    pub fn weapon_at(&self, player: PlayerId, tick: Tick) -> Option<TagId> {
        if self.sample_at(player, tick)?.in_vehicle {
            return None;
        }
        self.player_state(player)?.weapon_at(tick)
    }

    /// The film's raw layers: chunks, packets, frames.
    pub fn raw(&self) -> crate::format::Raw<'_> {
        crate::format::Raw::new(&self.raw, self.has_highlights, self.params, self.encoding)
    }
}

/// A player alive at one tick, with what playback draws for them:
/// [`Film::player_at`].
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct PlayerAt<'a> {
    /// The player.
    pub player: &'a Player,
    /// The life they are in.
    pub life: &'a Life,
    /// Their last sample at or before the tick.
    pub sample: &'a Sample,
    /// The weapon in their hands: None in a vehicle, where they fire its
    /// weapons, and in small lobbies, whose weapon state isn't read.
    pub weapon: Option<TagId>,
    /// The rounds in that weapon and in reserve, as it last sent them in
    /// this life. A weapon sends them when they change, so this is None from
    /// the spawn or the pickup to its first shot or reload.
    pub ammo: Option<&'a Ammo>,
    /// That weapon's charge, for an energy weapon: None until it sends one,
    /// as for `ammo`.
    pub energy: Option<&'a Energy>,
    /// That weapon's heat, for a weapon that heats up: None until it sends
    /// one, as for `ammo`.
    pub heat: Option<&'a Heat>,
    /// The grenades they carry, as sent since they spawned: None until the
    /// game sends a count in this life.
    pub grenades: Option<&'a GrenadeCount>,
    /// The objective they carry: a flag, the skull, Grifball's ball or a
    /// power seed.
    pub carrying: Option<Carried>,
    /// The ride they are on.
    pub ride: Option<&'a Ride>,
    /// The track of the vehicle they are in.
    pub vehicle: Option<&'a VehicleTrack>,
}
