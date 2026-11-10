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
    pub(crate) level: Option<Level>,
    pub(crate) coordinates: Option<Coordinates>,
    pub(crate) lives: Vec<Life>,
    pub(crate) vehicles: Vec<Vehicle>,
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
    /// Everything is read up front: in a release build about 4 s for a
    /// 9-minute Big Team Battle match of 27 players, 9 s for an 18-minute one
    /// of 35. A debug build is several times slower; a crate that depends on
    /// this one can optimize it alone with `[profile.dev.package.filmshell]
    /// opt-level = 3`.
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

    /// [`Film::from_chunks`] with what the caller knows beyond the film (see
    /// [`Hints`]): for a level the library's catalogue lacks
    /// ([`Film::coordinates`] is None, or not [`CoordinateSource::LevelBox`]),
    /// to name the level in place of the one the film names, or for
    /// [`analysis::spawn_factors`](crate::analysis::spawn_factors), which
    /// needs the map's spawn points.
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

    /// The level, when the catalogue or the hints know it. With
    /// [`Hints::level_bounds`] it is a level made from the box: no name,
    /// `structure` 0, and the hinted or header level id, else 0.
    pub fn level(&self) -> Option<&Level> {
        self.level.as_ref()
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
    /// before their first life, after a death until the next spawn, and
    /// after they left.
    ///
    /// Looks through the film's lives, so it costs O(lives) a call.
    pub fn life_at(&self, player: PlayerId, tick: Tick) -> Option<&Life> {
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

    /// The players of `team`, in id order.
    pub fn team(&self, team: TeamId) -> impl Iterator<Item = &Player> + '_ {
        self.players.iter().filter(move |p| p.team == Some(team))
    }

    /// Every vehicle track, in the order of their first record.
    pub fn vehicles(&self) -> &[Vehicle] {
        &self.vehicles
    }

    /// The track of the vehicle `entity` names at `tick`: of the tracks with
    /// that id, the last to start at or before `tick`. Ids are reused, so
    /// the tick picks the vehicle; a vehicle may stop sending positions while
    /// still in use, so the track can end before `tick`.
    pub fn vehicle_at(&self, entity: EntityId, tick: Tick) -> Option<&Vehicle> {
        self.vehicles
            .iter()
            .filter(|v| v.entity == entity && v.span().is_some_and(|(from, _)| from <= tick))
            .max_by_key(|v| v.span().map(|s| s.0))
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
    pub fn ride_vehicle(&self, ride: &Ride) -> Option<&Vehicle> {
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
    /// film's coordinates are exact.
    pub fn world_objects(&self) -> &[WorldObject] {
        &self.world
    }

    /// Weapons that lay on the ground.
    pub fn ground_weapons(&self) -> &[GroundWeapon] {
        &self.ground
    }

    /// Thrown grenades' flights, in throw order.
    pub fn grenade_flights(&self) -> &[GrenadeFlight] {
        &self.grenades
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

    /// The events from `from` up to and including `to`.
    pub fn events_between(&self, from: Tick, to: Tick) -> &[TimedEvent] {
        let a = self.events.partition_point(|e| e.tick < from);
        let b = self.events.partition_point(|e| e.tick <= to);
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

    /// Every medal the HighlightEvents chunk records for a player of the
    /// film, in its order, with the medal's tick. The tick is None when the
    /// film fixes no highlight clock; only medals with a tick are also in
    /// the event stream as [`Event::Medal`].
    pub fn medals(&self) -> impl Iterator<Item = (Option<Tick>, MedalAward)> + '_ {
        self.highlights.iter().filter_map(|h| match h.kind {
            HighlightKind::Medal(index) => Some((h.tick, MedalAward { player: h.player?, index })),
            _ => None,
        })
    }

    /// The HighlightEvents chunk's records, in its order.
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
    pub fn state(&self, player: PlayerId) -> Option<&PlayerState> {
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
        self.state(player)?.weapon_at(tick)
    }

    /// The film's raw layers: chunks, packets, frames.
    pub fn raw(&self) -> crate::format::Raw<'_> {
        crate::format::Raw::new(&self.raw, self.has_highlights, self.params, self.encoding)
    }
}
