//! Who holds an engine slot at a frame. The listings give each player
//! their id (`roster::Assignment`), and a joiner can take the id of a player
//! who left; what the film says about an id, its statborg's values included,
//! is its holder's.

use crate::roster::Roster;
use std::collections::HashMap;

/// Frames a bot can be in play before the listing that gives it its id:
/// a bot is listed after it spawns, by up to 463 ticks on the test films.
const LISTED_LATE_FRAMES: i64 = 600;

#[derive(Debug, Default)]
pub struct Holders {
    /// Id -> (frame, the player index that holds it from there), in frame
    /// order.
    holders: HashMap<u32, Vec<(i64, u32)>>,
    /// Player index -> the frame they left.
    left: HashMap<u32, i64>,
}

impl Holders {
    pub fn new(roster: &Roster) -> Holders {
        let mut h = Holders::default();
        for a in &roster.ids {
            h.holders.entry(a.id).or_default().push((a.frame, a.player));
        }
        for v in h.holders.values_mut() {
            v.sort_unstable();
        }
        for (i, p) in roster.players.iter().enumerate() {
            if let Some(left) = p.left {
                h.left.insert(i as u32, left);
            }
        }
        h
    }

    /// The player index holding player id `id` at `frame`: the one listed
    /// with it last at or before `frame`, unless they had left by then and
    /// the next holder is listed within `LISTED_LATE_FRAMES` after it (a bot
    /// is listed after it spawns); else the first listed with it (the first
    /// bot listings come a little after the film starts), else `id` itself
    /// when no listing gives it.
    pub fn api(&self, id: u32, frame: i64) -> u32 {
        let Some(h) = self.holders.get(&id) else { return id };
        let Some(i) = h.iter().rposition(|x| x.0 <= frame) else {
            return h.first().map_or(id, |x| x.1);
        };
        let gone = self.left.get(&h[i].1).is_some_and(|&left| left <= frame);
        match h.get(i + 1) {
            Some(next) if gone && next.0 - frame <= LISTED_LATE_FRAMES => next.1,
            _ => h[i].1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roster::{Assignment, RosterPlayer};

    #[test]
    fn a_bot_listed_after_it_spawns_holds_its_id_from_the_spawn() {
        // Player 1 holds id 5 and leaves at frame 100; player 2 is listed with
        // it at 300.
        let mut roster = Roster {
            players: vec![RosterPlayer::default(); 3],
            ids: vec![Assignment { id: 5, frame: 0, player: 1 }, Assignment { id: 5, frame: 300, player: 2 }],
        };
        roster.players[1].left = Some(100);
        let h = Holders::new(&roster);
        assert_eq!(h.api(5, 50), 1);
        assert_eq!(h.api(5, 250), 2, "after player 1 left, the next holder");
        assert_eq!(h.api(5, 400), 2);
        assert_eq!(h.api(7, 50), 7, "an id no listing gives");
        // A holder who hasn't left keeps the id until the next listing.
        roster.players[1].left = None;
        assert_eq!(Holders::new(&roster).api(5, 250), 1);
    }
}