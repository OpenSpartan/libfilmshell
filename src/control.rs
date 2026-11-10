//! What each player pressed: the movement input in the tick packets' control
//! view (`components::control_view`). Each player's forward and left throttle, -31 to 31, and a stance
//! value (the record's optional 5-bit field), kept when they change. Only
//! packets the view reads to their last byte count.

use crate::components::{control_view, Params};
use crate::container::Film;
use crate::payload::Payload;
use std::collections::BTreeMap;

/// Player index -> (frame, forward, left, stance or -1), at each change.
pub type Inputs = BTreeMap<u32, Vec<(i64, i32, i32, i32)>>;

pub fn reader<'a>(film: &'a Film, params: Params, out: &'a mut Inputs) -> impl FnMut(usize, &[u8], &Payload) + 'a {
    let frames: Vec<_> = film.frames().collect();
    let at: BTreeMap<usize, usize> = frames.iter().enumerate().map(|(k, f)| (f.index, k)).collect();
    move |i, _, p| {
        if p.stop.is_some() {
            return;
        }
        let Some(f) = at.get(&i).map(|&k| &frames[k]) else { return };
        let (messages, end) = control_view(f.chunk, p.end, &params);
        if end.is_none_or(|e| !(0..8).contains(&(8 * f.end as i64 - e))) {
            return;
        }
        for m in messages {
            let Some([a, b]) = m.throttle else { continue };
            let (forward, left) = (a as i32 - 31, b as i32 - 31);
            let stance = m.head[3].map_or(-1, |s| s as i32);
            let series = out.entry(m.player).or_default();
            if series.last().is_none_or(|l| (l.1, l.2, l.3) != (forward, left, stance)) {
                series.push((i as i64, forward, left, stance));
            }
        }
    }
}
