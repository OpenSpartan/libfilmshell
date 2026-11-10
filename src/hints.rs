use crate::{Position, TagId, TeamId};

/// A spawn point of the map variant, which the film doesn't carry. Spawn
/// points let the library fit coordinates on a level it has no box for, and
/// explain respawns ([`analysis::spawn_factors`](crate::analysis::spawn_factors)).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct SpawnPad {
    /// Where it stands, in world units.
    pub position: Position,
    /// Which spawns it serves.
    pub kind: SpawnKind,
    /// The team it is reserved for.
    pub team: Option<TeamId>,
}

/// Which spawns a [`SpawnPad`] serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SpawnKind {
    /// The match's first spawns.
    Initial,
    /// Spawns after a death.
    Respawn,
}

impl SpawnPad {
    /// A spawn point of `kind` at `position`.
    pub fn new(position: Position, kind: SpawnKind) -> SpawnPad {
        SpawnPad { position, kind, team: None }
    }

    /// An initial spawn point at `position`.
    pub fn initial(position: Position) -> SpawnPad {
        SpawnPad::new(position, SpawnKind::Initial)
    }

    /// A respawn point at `position`.
    pub fn respawn(position: Position) -> SpawnPad {
        SpawnPad::new(position, SpawnKind::Respawn)
    }

    /// The same point reserved for `team`.
    #[must_use]
    pub fn for_team(mut self, team: TeamId) -> SpawnPad {
        self.team = Some(team);
        self
    }
}

/// A level's play-space box for a level not in
/// [`game::levels`](crate::game::levels), as the game's files hold it: in
/// engine units, `[x, y, z]` with `z` up. World positions are ten times
/// engine units with the horizontal axes turned: world x is `-10 * y`,
/// world y is `10 * x`. The widths of the film's position fields follow
/// from the box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelBounds {
    /// The low corner.
    pub min: [f32; 3],
    /// The high corner.
    pub max: [f32; 3],
}

/// What the film can't tell the library, when the caller knows it.
///
/// A film on a Forge map built on a level the catalogue lacks has no
/// quantization box; the map variant's spawn points let the library fit
/// one, and explain each respawn:
///
/// ```no_run
/// use filmshell::{Chunk, Film, Hints, Position, SpawnPad};
///
/// # let chunks: Vec<Chunk> = Vec::new();
/// // Spawn points from the map variant, in world units.
/// let pads = [
///     SpawnPad::initial(Position::new(12.0, -40.5)),
///     SpawnPad::respawn(Position::new(-30.2, 18.0)),
/// ];
/// let film = Film::from_chunks_with(chunks, &Hints::new().spawn_pads(pads))?;
/// # Ok::<(), filmshell::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hints {
    pub(crate) level: Option<TagId>,
    pub(crate) level_bounds: Option<LevelBounds>,
    pub(crate) spawn_pads: Vec<SpawnPad>,
}

impl Hints {
    /// No hints.
    pub fn new() -> Hints {
        Hints::default()
    }

    /// The `levl` tag the map variant is built on, in place of the one the
    /// film names.
    #[must_use]
    pub fn level(mut self, level: TagId) -> Hints {
        self.level = Some(level);
        self
    }

    /// The level's play-space box, in place of the catalogue's. A box whose
    /// corners aren't finite, whose low corner isn't below its high one, or
    /// that spans a million engine units or more is ignored.
    #[must_use]
    pub fn level_bounds(mut self, bounds: LevelBounds) -> Hints {
        let ok = (0..3).all(|i| {
            let (lo, hi) = (bounds.min[i], bounds.max[i]);
            lo.is_finite() && hi.is_finite() && lo < hi && hi - lo < 1e6
        });
        self.level_bounds = ok.then_some(bounds);
        self
    }

    /// The map variant's spawn points, in any order:
    /// [`SpawnFactor::pad`](crate::analysis::SpawnFactor::pad) indexes them.
    /// Points whose coordinates aren't finite or lie more than a million
    /// world units out are kept in the order but never used.
    #[must_use]
    pub fn spawn_pads(mut self, pads: impl IntoIterator<Item = SpawnPad>) -> Hints {
        self.spawn_pads = pads.into_iter().collect();
        self
    }

    /// The spawn points the library uses, with their indices in the hints.
    pub(crate) fn usable_pads(&self) -> impl Iterator<Item = (usize, &SpawnPad)> + '_ {
        let sane = |v: f64| v.is_finite() && v.abs() <= 1e6;
        self.spawn_pads
            .iter()
            .enumerate()
            .filter(move |(_, p)| sane(p.position.x) && sane(p.position.y) && p.position.z.is_none_or(sane))
    }
}
