//! Who holds an engine player id at a frame. The listings give each player
//! their id (`roster::Assignment`), and a joiner can take the id of a player
//! who left; what the film says about an id, its statborg's values included,
//! is its holder's.

use crate::roster::Roster;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Holders {
    /// Id -> (frame, the player index that holds it from there), in frame
    /// order.
    holders: HashMap<u32, Vec<(i64, u32)>>,
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
        h
    }

    /// The player index holding player id `id` at `frame`: the one listed
    /// with it last at or before `frame`, else the first listed with it (the
    /// first bot listings come a little after the film starts), else `id`
    /// itself when no listing gives it.
    pub fn api(&self, id: u32, frame: i64) -> u32 {
        let Some(h) = self.holders.get(&id) else { return id };
        h.iter().rev().find(|x| x.0 <= frame).or(h.first()).map_or(id, |x| x.1)
    }
}
