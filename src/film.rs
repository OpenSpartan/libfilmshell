use crate::analysis::AnalysisData;
use crate::chunk::is_zlib;
use crate::game::Level;
use crate::model::*;
use crate::{Chunk, ChunkKind, Error, Hints};

/// A theater film, read whole.
///
/// Built from the film's chunks with [`Film::from_chunks`]; everything the
/// library reads is computed then, so the accessors are cheap.
pub struct Film {
    pub(crate) raw: crate::container::Film,
    pub(crate) params: Option<crate::components::Params>,
    pub(crate) has_highlights: bool,
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
    /// the rest replay chunks, the last of them optionally the highlights.
    pub fn from_chunks<'a>(chunks: impl IntoIterator<Item = Chunk<'a>>) -> Result<Film, Error> {
        Film::from_chunks_with(chunks, &Hints::default())
    }

    /// [`Film::from_chunks`] with what the caller knows beyond the film.
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
        let missing: Vec<u32> = (0..=last).filter(|i| chunks.binary_search_by_key(i, |c| c.index).is_err()).collect();
        if !missing.is_empty() {
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
        for c in chunks {
            data.push(if is_zlib(&c.data) {
                crate::inflate::zlib(&c.data, usize::MAX).ok_or(Error::Inflate { index: c.index })?
            } else {
                c.data.into_owned()
            });
        }
        if crate::header::read(&data[0]).build.is_none() {
            return Err(Error::UnknownHeader);
        }
        Ok(crate::assemble::assemble(data, has_highlights, hints))
    }

    /// What chunk 0's header says: the build, the start time, the map.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Frames and ticks.
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// Everyone who played, in index order.
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// The player with index `id`.
    pub fn player(&self, id: PlayerId) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    /// The game variant, when chunk 0's variant data reads.
    pub fn variant(&self) -> Option<&GameVariant> {
        self.variant.as_ref()
    }

    /// The mode the variant's scripts run.
    pub fn mode(&self) -> GameMode {
        self.variant.as_ref().map_or(GameMode::Other, GameVariant::mode)
    }

    /// The level, when the catalogue or the hints know it.
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

    /// Where `player` was at `tick`: their last sample at or before it, in
    /// the life under way then. None while the player isn't alive: before
    /// their first life, between a death and the next spawn, after the last
    /// sample of a life no kill ended once another has begun, and after a
    /// player who left was last listed.
    pub fn sample_at(&self, player: PlayerId, tick: Tick) -> Option<&Sample> {
        let mut lives = self.lives_of(player).peekable();
        let mut current = None;
        while let Some(l) = lives.next_if(|l| l.start().is_some_and(|s| s <= tick)) {
            current = Some(l);
        }
        let life = current?;
        let last = lives.peek().is_none();
        if !last && life.death.is_none() && life.end().is_some_and(|e| tick > e) {
            return None;
        }
        if last && self.player(player).and_then(|p| p.left).is_some_and(|left| tick > left) {
            return None;
        }
        life.at(tick)
    }

    /// Whether `player` is alive at `tick`, by the rules of
    /// [`Film::sample_at`].
    pub fn alive_at(&self, player: PlayerId, tick: Tick) -> bool {
        self.sample_at(player, tick).is_some()
    }

    /// Every vehicle track, in the order of their first record.
    pub fn vehicles(&self) -> &[Vehicle] {
        &self.vehicles
    }

    /// Every ride, in tick order.
    pub fn rides(&self) -> &[Ride] {
        &self.rides
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

    /// Every kill, in tick order.
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

    /// What `player` pressed and carried over the match.
    pub fn state(&self, player: PlayerId) -> Option<&PlayerState> {
        self.states.iter().find(|s| s.0 == player).map(|s| &s.1)
    }

    /// The film's raw layers: chunks, packets, frames.
    pub fn raw(&self) -> crate::format::Raw<'_> {
        crate::format::Raw::new(&self.raw, self.has_highlights, self.params)
    }
}
