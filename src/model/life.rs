use super::{EntityId, PlayerId, Position, Tick};

/// How the film's quantized positions became world units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CoordinateSource {
    /// The level's own quantization box, from the level catalogue or the
    /// hints: exact.
    LevelBox,
    /// The box every Forge canvas shares, its offset fitted to spawn pads.
    ForgeCanvas,
    /// A box the library fitted for this map variant beforehand, its offset refitted to the film.
    KnownLayout,
    /// Scale and offset fitted to the hinted spawn pads.
    SpawnFit,
    /// The positions' extent stretched over the pads' extent: an estimate.
    ExtentFit,
    /// Anchored on two spawn pads (small lobbies on an unknown level).
    TwoSpawn,
    /// Anchored on one spawn pad.
    SingleSpawn,
}

/// The affine map from raw positions to world units:
/// `x = -scale_x * raw[1] + offset_x`, `y = scale_y * raw[0] + offset_y`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Coordinates {
    /// Where the map came from.
    pub source: CoordinateSource,
    /// World units per raw unit along x.
    pub scale_x: f64,
    /// World units per raw unit along y.
    pub scale_y: f64,
    /// World x at raw 0.
    pub offset_x: f64,
    /// World y at raw 0.
    pub offset_y: f64,
    /// How well the life starts fit the spawn pads, when the box was fitted
    /// to them.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub spawn_fit: Option<SpawnFit>,
}

/// How well a fitted box puts the life starts on the spawn pads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct SpawnFit {
    /// The life starts checked against the pads.
    pub checked: usize,
    /// How many landed within 5 world units of one.
    pub on_pads: usize,
}
impl Coordinates {
    /// A raw position in world units.
    pub fn to_world(&self, raw: [f64; 2]) -> Position {
        Position::new(-self.scale_x * raw[1] + self.offset_x, self.scale_y * raw[0] + self.offset_y)
    }
}

/// One sample of a life: where the biped was and its state as of its last
/// update.
///
/// In a small lobby (four players or fewer:
/// [`Encoding::PerEntity`](crate::format::Encoding::PerEntity) and
/// [`Encoding::BotComposite`](crate::format::Encoding::BotComposite)) the
/// biped updates aren't read field by field, so `yaw`, `speed`, `health` and
/// `shield` are None.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Sample {
    /// When.
    pub tick: Tick,
    /// The two horizontal position fields as the film stores them, scaled to
    /// a 15-bit window (`field * 2^(15 - width)`; in a small lobby `raw[0]`
    /// is 64 and `raw[1]` 8 times smaller): `raw[0]` becomes world y,
    /// `raw[1]` world x.
    /// [`Coordinates::to_world`] converts them.
    pub raw: [f64; 2],
    /// The position in world units, when the film's coordinates are known.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub position: Option<Position>,
    /// Aim yaw in radians, -π to π, in world axes (from +x toward +y).
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub yaw: Option<f64>,
    /// On-foot speed, world units a second; None in a vehicle.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub speed: Option<f64>,
    /// Health as a fraction of the maximum.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub health: Option<f64>,
    /// Shield as a fraction of the maximum, above 1 with an overshield; None
    /// in a mode without shields.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub shield: Option<f64>,
    /// In a vehicle: the sample is the vehicle's position.
    pub in_vehicle: bool,
    /// The vehicle, when known: [`Film::vehicle_at`](crate::Film::vehicle_at)
    /// finds its track.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub vehicle: Option<EntityId>,
}

/// One life of a biped: from its spawn to its death or the film's end.
///
/// A life is one biped entity. Its owner is the player controlling the unit,
/// as the biped's own records say; where they don't, the library matches
/// the life's first and last records against the players' death times in
/// the highlights chunk. A life neither places has no player.
///
/// [`Film::life_at`](crate::Film::life_at) finds the life a player is in at a
/// tick, and [`Film::sample_at`](crate::Film::sample_at) their sample then.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Life {
    /// The player whose life it is.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub player: Option<PlayerId>,
    /// The biped's id, unique among the lives under way at the same time;
    /// None for a life no player owns.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub entity: Option<EntityId>,
    /// Whether the life ended in a death: by the HighlightEvents death times,
    /// else where the player controlling it let go of it at a time the
    /// scoreboard counts a death (a bot removed to make room for a joiner
    /// lets go without dying); in a small lobby, by a respawn following it.
    pub died: bool,
    /// The tick of the [`Kill`](crate::Kill) that ended the life, recorded or
    /// [`inferred`](crate::Kill::inferred). None when `died` is true but no
    /// kill was placed.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub death: Option<Tick>,
    /// The life's last tick, when it ends before the film does. For a death
    /// with a kill, the kill's tick; for a death without one, or a life the
    /// player's next life follows, the last sample; for a player who left,
    /// the tick they left. None when the life lasts to the film's end.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub until: Option<Tick>,
    /// Its samples, in tick order.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub samples: Vec<Sample>,
}

impl Life {
    /// The first sample's tick.
    pub fn start(&self) -> Option<Tick> {
        self.samples.first().map(|s| s.tick)
    }

    /// The last sample's tick. The life can last past it, to
    /// [`Life::until`] or the film's end: [`Life::is_over_at`] says when.
    pub fn end(&self) -> Option<Tick> {
        self.samples.last().map(|s| s.tick)
    }

    /// Whether the life is over at `tick`: after [`Life::until`].
    pub fn is_over_at(&self, tick: Tick) -> bool {
        self.samples.is_empty() || self.until.is_some_and(|u| tick > u)
    }

    /// The last sample at or before `tick`, while the life lasts: None
    /// before its first sample and once it is over ([`Life::is_over_at`]).
    pub fn sample_at(&self, tick: Tick) -> Option<&Sample> {
        if self.is_over_at(tick) {
            return None;
        }
        self.samples.partition_point(|s| s.tick <= tick).checked_sub(1).map(|i| &self.samples[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn life(ticks: &[u32], died: bool, death: Option<u32>) -> Life {
        let sample = |t: u32| Sample {
            tick: Tick(t),
            raw: [0.0; 2],
            position: None,
            yaw: None,
            speed: None,
            health: None,
            shield: None,
            in_vehicle: false,
            vehicle: None,
        };
        // As the film sets it for a player's only life.
        let until = death.or(ticks.last().copied().filter(|_| died)).map(Tick);
        Life {
            player: None,
            entity: None,
            died,
            death: death.map(Tick),
            until,
            samples: ticks.iter().map(|&t| sample(t)).collect(),
        }
    }

    #[test]
    fn a_life_ends_when_it_is_over() {
        let l = life(&[10, 20, 30], true, Some(32));
        assert!(l.sample_at(Tick(9)).is_none());
        assert_eq!(l.sample_at(Tick(25)).map(|s| s.tick), Some(Tick(20)));
        assert_eq!(l.sample_at(Tick(32)).map(|s| s.tick), Some(Tick(30)));
        assert!(l.sample_at(Tick(33)).is_none());
        // A death no kill was placed for ends at the last sample.
        let l = life(&[10, 20, 30], true, None);
        assert!(l.sample_at(Tick(30)).is_some() && l.sample_at(Tick(31)).is_none());
        // Samples past the kill (a vehicle's) don't extend the life.
        let l = life(&[10, 20, 30, 90], true, Some(30));
        assert!(l.sample_at(Tick(31)).is_none());
        // A life that didn't end in a death lasts past its last sample.
        let l = life(&[10, 20], false, None);
        assert_eq!(l.sample_at(Tick(1000)).map(|s| s.tick), Some(Tick(20)));
    }
}
