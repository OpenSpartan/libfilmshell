use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::time::Duration;

/// A game tick: 1/60 s of simulation, counted from the film's first frame.
///
/// The film starts before the match does: the match starts when its clock
/// first runs, [`Film::match_start`](crate::Film::match_start).
///
/// Adding or subtracting a number of ticks works as it does for `u32`: going
/// below tick 0 panics with overflow checks on (debug builds by default) and
/// wraps otherwise, so use
/// [`Tick::checked_sub`] or [`Tick::saturating_sub`] where the result can
/// fall below 0. The difference of two ticks is signed:
///
/// ```
/// use filmshell::Tick;
///
/// let spawn = Tick::from_seconds(12.5);
/// let death = spawn + 5 * Tick::PER_SECOND;
/// assert_eq!(death - spawn, 300);
/// assert_eq!(spawn - death, -300);
/// assert_eq!(death.to_string(), "0:17.500");
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tick(pub u32);

impl Tick {
    /// Ticks in a second of game time.
    pub const PER_SECOND: u32 = 60;

    /// The raw tick count.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The tick `seconds` of game time after the film's first frame, to the
    /// nearest tick. Negative and NaN times give tick 0.
    pub fn from_seconds(seconds: f64) -> Tick {
        let t = (seconds * Self::PER_SECOND as f64).round();
        Tick(if t >= 0.0 { t.min(u32::MAX as f64) as u32 } else { 0 })
    }

    /// Seconds since the film's first frame.
    pub fn seconds(self) -> f64 {
        self.0 as f64 / Self::PER_SECOND as f64
    }

    /// Game time since the film's first frame.
    pub fn duration(self) -> Duration {
        let per = Self::PER_SECOND as u64;
        let t = self.0 as u64;
        Duration::from_secs(t / per) + Duration::from_nanos((t % per) * 1_000_000_000 / per)
    }

    /// `ticks` earlier, or None before the film's first frame.
    pub fn checked_sub(self, ticks: u32) -> Option<Tick> {
        self.0.checked_sub(ticks).map(Tick)
    }

    /// `ticks` earlier, stopping at tick 0.
    #[must_use]
    pub fn saturating_sub(self, ticks: u32) -> Tick {
        Tick(self.0.saturating_sub(ticks))
    }
}

impl From<u32> for Tick {
    fn from(v: u32) -> Tick {
        Tick(v)
    }
}

impl Add<u32> for Tick {
    type Output = Tick;

    fn add(self, ticks: u32) -> Tick {
        Tick(self.0 + ticks)
    }
}

impl AddAssign<u32> for Tick {
    fn add_assign(&mut self, ticks: u32) {
        self.0 += ticks;
    }
}

impl Sub<u32> for Tick {
    type Output = Tick;

    fn sub(self, ticks: u32) -> Tick {
        Tick(self.0 - ticks)
    }
}

impl SubAssign<u32> for Tick {
    fn sub_assign(&mut self, ticks: u32) {
        self.0 -= ticks;
    }
}

/// The ticks from `other` to `self`, negative when `other` is later.
impl Sub for Tick {
    type Output = i64;

    fn sub(self, other: Tick) -> i64 {
        self.0 as i64 - other.0 as i64
    }
}

impl fmt::Display for Tick {
    /// `m:ss.fff` of game time since the film's first frame.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = (self.0 as u64 * 1000) / Self::PER_SECOND as u64;
        write!(f, "{}:{:02}.{:03}", ms / 60_000, ms / 1000 % 60, ms % 1000)
    }
}

/// The film's clock: which game tick each frame is, and where the
/// highlights chunk's millisecond times fall.
///
/// A frame is one tick packet of the replay chunks (on a film framed by its
/// markers, the span from one marker to the next: the fallback when a replay
/// chunk's packets don't walk to its end,
/// [`format::Frame`](crate::format::Frame)). At 60 Hz a frame is one tick, on
/// 30 Hz films two, and across a recorder hitch a few more.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clock {
    pub(crate) frame_ticks: Vec<u32>,
    /// Each frame's tick on the clock `HighlightEvents` times run on: the
    /// time since the first frame by the packets' timestamps, in ticks.
    pub(crate) highlight_ticks: Vec<u32>,
    /// The highlight tick of `HighlightEvents` time 0.
    pub(crate) highlight_zero: Option<f64>,
}

/// A pause in the recording that `HighlightEvents` time doesn't count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Hitch {
    /// Where it falls, on the highlight clock (the ticks `HighlightEvents`
    /// times count, not [`Tick`]s).
    pub at: u32,
    /// The game ticks it skipped.
    pub skipped: u32,
}
/// Game ticks per millisecond of `HighlightEvents` time.
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

    /// The highlight clock's tick of `HighlightEvents` time 0.
    ///
    /// The highlights' times count from about the film's first frame, and the
    /// film doesn't say where their 0 falls, so the library places it from
    /// events the film times itself: each death record on the kill event of
    /// that victim, else each medal record on the mode event that names its
    /// player. None when neither places it; the highlights then have no
    /// ticks.
    ///
    /// The highlight clock is the time since the first frame by the packets'
    /// timestamps, in ticks of 1/59.94 s: a recorder hitch skips ticks but no
    /// time ([`Clock::hitches`]), and a server that falls behind runs fewer
    /// ticks than time. Time `ms` is highlight tick `zero + ms * 0.05994`,
    /// fractional, which [`Clock::highlight_tick`] turns into a [`Tick`].
    pub fn highlight_zero(&self) -> Option<f64> {
        self.highlight_zero
    }

    /// The recorder hitches `HighlightEvents` time doesn't count, in order.
    pub fn hitches(&self) -> Vec<Hitch> {
        crate::lives::hitches(&self.frame_ticks).into_iter().map(|(at, skipped)| Hitch { at, skipped }).collect()
    }

    /// The tick of `HighlightEvents` time `ms`, when the highlight clock is
    /// known ([`Clock::highlight_zero`]).
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
