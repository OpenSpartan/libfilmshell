use std::fmt;

/// A game tick: 1/60 s of simulation, counted from the film's first frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Tick(pub u32);

impl Tick {
    /// Ticks in a second of game time.
    pub const PER_SECOND: u32 = 60;

    /// Seconds since the film's first frame.
    pub fn seconds(self) -> f64 {
        self.0 as f64 / Self::PER_SECOND as f64
    }
}

impl fmt::Display for Tick {
    /// `m:ss.fff` of game time.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = (self.0 as u64 * 1000) / Self::PER_SECOND as u64;
        write!(f, "{}:{:02}.{:03}", ms / 60_000, ms / 1000 % 60, ms % 1000)
    }
}

/// The film's clock: which game tick each frame is, and where the
/// HighlightEvents chunk's millisecond times fall.
///
/// A frame is one tick packet of the replay chunks. At 60 Hz a frame is one
/// tick, on 30 Hz films two, and across a recorder hitch a few more.
#[derive(Clone, Debug, Default)]
pub struct Clock {
    pub(crate) frame_ticks: Vec<u32>,
    /// Each frame's tick without the ticks hitches skipped: the clock
    /// HighlightEvents times run on.
    pub(crate) highlight_ticks: Vec<u32>,
    /// The highlight tick of HighlightEvents time 0.
    pub(crate) highlight_zero: Option<f64>,
}

/// Game ticks per millisecond of HighlightEvents time.
const TICKS_PER_MS: f64 = crate::lives::TICKS_PER_MS;

impl Clock {
    /// Frames in the film.
    pub fn frame_count(&self) -> usize {
        self.frame_ticks.len()
    }

    /// The tick of frame `frame`.
    pub fn tick(&self, frame: usize) -> Option<Tick> {
        self.frame_ticks.get(frame).map(|&t| Tick(t))
    }

    /// The last frame at or before `tick`.
    pub fn frame_at(&self, tick: Tick) -> Option<usize> {
        self.frame_ticks.partition_point(|&t| t <= tick.0).checked_sub(1)
    }

    /// The last frame's tick.
    pub fn end(&self) -> Tick {
        Tick(self.frame_ticks.last().copied().unwrap_or(0))
    }

    /// Ticks per frame: 1 on a 60 Hz film, 2 on a 30 Hz one.
    pub fn ticks_per_frame(&self) -> u32 {
        let mut votes = [0usize; 5];
        for w in self.frame_ticks.windows(2) {
            if let Some(v) = votes.get_mut((w[1] - w[0]) as usize) {
                *v += 1;
            }
        }
        (1..5).max_by_key(|&d| (votes[d], std::cmp::Reverse(d))).unwrap_or(1) as u32
    }

    /// Every frame's tick.
    pub fn frame_ticks(&self) -> &[u32] {
        &self.frame_ticks
    }

    /// The tick of HighlightEvents time `ms`, when the film's kills fix the
    /// highlight clock.
    pub fn highlight_tick(&self, ms: u32) -> Option<Tick> {
        let at = (self.highlight_zero? + ms as f64 * TICKS_PER_MS).round().max(0.0) as u32;
        let k = self.highlight_ticks.partition_point(|&t| t <= at).checked_sub(1)?;
        Some(Tick(self.frame_ticks[k] + at - self.highlight_ticks[k]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_print_as_game_time() {
        assert_eq!(Tick(60 * 75 + 30).to_string(), "1:15.500");
    }

    #[test]
    fn highlight_times() {
        let c =
            Clock { frame_ticks: vec![0, 1, 2, 6, 7], highlight_ticks: vec![0, 1, 2, 3, 4], highlight_zero: Some(0.0) };
        assert_eq!(c.highlight_tick(0), Some(Tick(0)));
        assert_eq!(c.highlight_tick(50), Some(Tick(6)));
        assert_eq!(c.frame_at(Tick(5)), Some(2));
        assert_eq!(c.ticks_per_frame(), 1);
    }
}
