//! Who holds an engine player id at a frame. An id belongs to the player
//! with that index, until a joiner or a bot takes it over from a player who
//! left (`roster::Takeover`); what the film says about the id after that,
//! its statborg's values included, is the new holder's.

use crate::roster::Roster;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Holders {
    /// Id -> (frame, the player index that holds it from there).
    takeovers: HashMap<u32, Vec<(i64, u32)>>,
}

impl Holders {
    pub fn new(roster: &Roster) -> Holders {
        let mut h = Holders::default();
        for t in &roster.takeovers {
            h.takeovers.entry(t.id).or_default().push((t.frame, t.player));
        }
        for v in h.takeovers.values_mut() {
            v.sort_unstable();
        }
        h
    }

    /// The player index holding player id `id` at `frame`.
    pub fn api(&self, id: u32, frame: i64) -> u32 {
        let later = self.takeovers.get(&id).and_then(|t| t.iter().rev().find(|x| x.0 <= frame));
        later.map_or(id, |x| x.1)
    }
}
