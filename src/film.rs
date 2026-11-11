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
    /// - [`Error::UnexpectedKind`]: a header that isn't chunk 0, highlights
    ///   that aren't the last chunk.
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

    /// The film's clock: which tick each frame is, and where HighlightEvents
    /// times fall.
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// Everyone who played, in id order.
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// The player with id `id`.
    pub fn player(&self, id: PlayerId) -> Option<&Player> {
        match self.players.get(id.index()) {
            Some(p) if p.id == id => Some(p),
            _ => self.players.iter().find(|p| p.id == id),
        }
    }

    /// The gamertag or bot name of the player `id`.
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

    /// Every player alive at `tick` with their sample then, in player order:
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
    /// sample then, the weapon in their hands, and the ride and vehicle
    /// they're in. None while they aren't alive ([`Film::life_at`]). The
    /// sample is the last at or before `tick`; [`Life::position_at`] places
    /// the player between samples.
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
        let ammo = on_foot.and_then(|s| s.ammo_in(s.held_slot(tick)?, tick));
        // Counts from before this life's spawn are an earlier life's.
        let start = life.start().unwrap_or(tick);
        let grenades = state.and_then(|s| s.grenades_at(tick)).filter(|g| g.tick >= start);
        Some(PlayerAt { player: self.player(player)?, life, sample, weapon, ammo, grenades, ride, vehicle })
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
        self.vehicles.get(ride.vehicle?)
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
    pub fn grenade_flights(&self) -> &[GrenadeFlight] {
        &self.grenades
    }

    /// The grenades in the air at `tick`, with where each was
    /// ([`GrenadeFlight::position_at`]), in throw order.
    pub fn grenades_at(&self, tick: Tick) -> impl Iterator<Item = (&GrenadeFlight, Position)> + '_ {
        self.grenades.iter().filter_map(move |g| Some((g, g.position_at(tick)?)))
    }

    /// The zones or hills of a zone mode.
    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }

    /// King of the Hill's points on the current hill.
    pub fn hill_score(&self) -> Option<&HillScore> {
        self.hill_score.as_ref()
    }

    /// Every event, in tick order.
    pub fn events(&self) -> &[TimedEvent] {
        &self.events
    }

    /// The events in a range of ticks, in tick order: `film.events_in(a..b)`
    /// leaves out tick `b`, `film.events_in(a..=b)` keeps it.
    ///
    /// Playback that steps from tick `last` to `now` shows what happened in
    /// between:
    ///
    /// ```no_run
    /// # fn run(film: &filmshell::Film, last: filmshell::Tick, now: filmshell::Tick) {
    /// use filmshell::Event;
    ///
    /// for e in film.events_in(last..now) {
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
    /// for and those it doesn't ([`Kill::inferred`]), and the deaths the game
    /// deals everyone still alive when a match ends ([`Kill::is_scored`] is
    /// false for those).
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
    /// The rounds in that weapon and in reserve, as last sent.
    pub ammo: Option<&'a Ammo>,
    /// The grenades they carry, as sent since they spawned.
    pub grenades: Option<&'a GrenadeCount>,
    /// The ride they are on.
    pub ride: Option<&'a Ride>,
    /// The track of the vehicle they are in.
    pub vehicle: Option<&'a VehicleTrack>,
}
