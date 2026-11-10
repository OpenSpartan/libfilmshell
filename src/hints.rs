use crate::{Position, TagId, TeamId};

/// A spawn point of the map variant, which the film doesn't carry. Spawn
/// points let the library fit coordinates on a level it has no box for, and
/// explain respawns (`analysis::spawn_factors`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct SpawnPad {
    /// Where it stands, in world units.
    pub position: Position,
    /// A respawn point; else an initial spawn point.
    pub respawn: bool,
    /// The team it is reserved for.
    pub team: Option<TeamId>,
}

impl SpawnPad {
    /// A spawn point at `position`.
    pub fn new(position: Position, respawn: bool) -> SpawnPad {
        SpawnPad { position, respawn, team: None }
    }

    /// The same point reserved for `team`.
    pub fn for_team(mut self, team: TeamId) -> SpawnPad {
        self.team = Some(team);
        self
    }
}

/// A level's play-space box in engine units, for a level the catalogue
/// lacks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelBounds {
    /// The low corner.
    pub min: [f32; 3],
    /// The high corner.
    pub max: [f32; 3],
}

/// What the film can't tell the library, when the caller knows it.
///
/// ```
/// use filmshell::{Hints, Position, SpawnPad, TagId};
///
/// let hints = Hints::new()
///     .level(TagId(0xa0bf5c4a))
///     .spawn_pads([SpawnPad::new(Position::new(12.0, -40.5), false)]);
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
    pub fn level(mut self, level: TagId) -> Hints {
        self.level = Some(level);
        self
    }

    /// The level's play-space box, in place of the catalogue's. A box whose
    /// corners aren't finite, or whose low corner isn't below its high one,
    /// is ignored.
    pub fn level_bounds(mut self, bounds: LevelBounds) -> Hints {
        let ok = (0..3).all(|i| {
            let (lo, hi) = (bounds.min[i], bounds.max[i]);
            lo.is_finite() && hi.is_finite() && lo < hi && hi - lo < 1e6
        });
        self.level_bounds = ok.then_some(bounds);
        self
    }

    /// The map variant's spawn points. Points whose coordinates aren't
    /// finite or lie more than a million world units out are left out.
    pub fn spawn_pads(mut self, pads: impl IntoIterator<Item = SpawnPad>) -> Hints {
        let sane = |v: f64| v.is_finite() && v.abs() <= 1e6;
        self.spawn_pads = pads
            .into_iter()
            .filter(|p| sane(p.position.x) && sane(p.position.y) && p.position.z.is_none_or(sane))
            .collect();
        self
    }
}
