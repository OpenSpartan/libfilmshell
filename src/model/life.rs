use super::{EntityId, PlayerId, Position, Tick};

/// How the film's quantized positions became world units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub enum CoordinateSource {
    /// The level's own quantization box, from the level catalogue or the
    /// hints: exact.
    LevelBox,
    /// The box every Forge canvas shares, its offset fitted to spawn pads.
    ForgeCanvas,
    /// A box fitted earlier for this map variant, its offset refitted.
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    /// For a fit to spawn pads: the life starts checked against the pads,
    /// and how many landed within 5 world units of one.
    pub spawn_fit: Option<(usize, usize)>,
}

impl Coordinates {
    /// A raw position in world units.
    pub fn to_world(&self, raw: [f64; 2]) -> Position {
        Position::new(-self.scale_x * raw[1] + self.offset_x, self.scale_y * raw[0] + self.offset_y)
    }
}

/// One sample of a life: where the biped was and its state as of its last
/// update.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Sample {
    /// When.
    pub tick: Tick,
    /// The two horizontal position fields as the film stores them, scaled to
    /// a 15-bit window (`field * 2^(15 - width)`; small lobbies' units are 64
    /// and 8 times smaller). `Coordinates::to_world` converts them.
    pub raw: [f64; 2],
    /// The position in world units, when the film's coordinates are known.
    pub position: Option<Position>,
    /// Aim yaw in radians, in world axes (from +x toward +y).
    pub yaw: Option<f64>,
    /// On-foot speed, world units a second; None in a vehicle.
    pub speed: Option<f64>,
    /// Health as a fraction of the maximum.
    pub health: Option<f64>,
    /// Shield as a fraction of the maximum, above 1 with an overshield; None
    /// in a mode without shields.
    pub shield: Option<f64>,
    /// In a vehicle: the sample is the vehicle's position.
    pub in_vehicle: bool,
    /// The vehicle, when known: [`Film::vehicle_at`](crate::Film::vehicle_at)
    /// finds its track.
    pub vehicle: Option<EntityId>,
}

/// One life of a biped: from its spawn to its death or the film's end.
///
/// A life is one biped entity. Its owner comes from matching each life's
/// first and last records against the players' death times in the
/// HighlightEvents chunk, so a life the matching can't place has no player.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Life {
    /// The player whose life it is.
    pub player: Option<PlayerId>,
    /// The biped's id, which names this life only.
    pub entity: Option<EntityId>,
    /// The life ended in a death, as the life assignment counts them.
    pub died: bool,
    /// The tick of the kill that ended the life, when the film's kill
    /// events name one.
    pub death: Option<Tick>,
    /// Its samples, in tick order.
    pub samples: Vec<Sample>,
}

impl Life {
    /// The first sample's tick.
    pub fn start(&self) -> Option<Tick> {
        self.samples.first().map(|s| s.tick)
    }

    /// The last sample's tick.
    pub fn end(&self) -> Option<Tick> {
        self.samples.last().map(|s| s.tick)
    }

    /// Whether the life is over at `tick`: after its death, or, for a life
    /// that ended in a death no kill event names, after its last sample.
    pub fn over_at(&self, tick: Tick) -> bool {
        match (self.death, self.end()) {
            (Some(d), _) => tick > d,
            (None, Some(end)) => self.died && tick > end,
            (None, None) => true,
        }
    }

    /// The last sample at or before `tick`, while the life lasts: None
    /// before its first sample and once it is over ([`Life::over_at`]).
    pub fn at(&self, tick: Tick) -> Option<&Sample> {
        if self.over_at(tick) {
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
        Life {
            player: None,
            entity: None,
            died,
            death: death.map(Tick),
            samples: ticks.iter().map(|&t| sample(t)).collect(),
        }
    }

    #[test]
    fn a_life_ends_at_its_death() {
        let l = life(&[10, 20, 30], true, Some(32));
        assert!(l.at(Tick(9)).is_none());
        assert_eq!(l.at(Tick(25)).map(|s| s.tick), Some(Tick(20)));
        assert_eq!(l.at(Tick(32)).map(|s| s.tick), Some(Tick(30)));
        assert!(l.at(Tick(33)).is_none());
        // A death the kill events don't name ends at the last sample.
        let l = life(&[10, 20, 30], true, None);
        assert!(l.at(Tick(30)).is_some() && l.at(Tick(31)).is_none());
        // Samples past the kill (a vehicle's) don't extend the life.
        let l = life(&[10, 20, 30, 90], true, Some(30));
        assert!(l.at(Tick(31)).is_none());
        // A life that didn't end in a death lasts past its last sample.
        let l = life(&[10, 20], false, None);
        assert_eq!(l.at(Tick(1000)).map(|s| s.tick), Some(Tick(20)));
    }
}
