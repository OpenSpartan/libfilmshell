//! The modes' event trees: what each mode script game event is. A game
//! event's id is the position of its response in a depth-first walk of the
//! mode's event scripts, so the same id means different things in different
//! modes; the tables here are those walks, built from the game's scripts,
//! for each group of event scripts a film can run.

use super::{Medal, Vehicle};
use std::collections::BTreeSet;

/// A response in a mode's event tree: what a game event is, as the mode's
/// scripts define it. [`Film::game_response`](crate::Film::game_response)
/// finds a game event's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct GameResponse {
    /// The scripts' name for it (`ghostDestroyResponse`).
    pub name: &'static str,
    /// The medal it awards.
    pub medal: Option<Medal>,
    /// The personal score it gives: the points, and the text the game shows
    /// with them ("Killed %pe"), empty when it shows none. `%pa` and `%pe`
    /// stand for players, whom the event's player arguments name.
    pub score: Option<(i32, &'static str)>,
    /// What the announcer or the personal AI says, in English.
    pub announcer: Option<&'static str>,
    /// Whether it writes a line to the kill feed.
    pub kill_feed: bool,
    /// The string ids of its own text, when its text isn't built when sent.
    pub(crate) text: &'static [u32],
}

/// A group of mode event scripts, and the ids its event tree gives the
/// vehicle responses and every response.
#[derive(Debug)]
pub(crate) struct EventGroup {
    /// The group's own event scripts, lower-cased.
    pub files: &'static [&'static str],
    /// Each destroy response's id and vehicle.
    pub destroys: &'static [(u32, Vehicle)],
    /// Each hijack response's id and vehicle.
    pub hijacks: &'static [(u32, Vehicle)],
    /// Each id in order, its response (an index into `RESPONSES`) and
    /// whether the id is unsure: one the walk can't place for certain (inside
    /// a script's loop, or in Firefight's own files).
    pub responses: &'static [(u32, u16, bool)],
}

impl EventGroup {
    /// The response of the event `id` whose own text is `text`. The event's
    /// text wins: a response whose static text isn't the event's is a
    /// neighbour's, and an unsure id counts only when its text is the
    /// event's.
    pub fn response(&self, id: u32, text: Option<u32>) -> Option<&'static GameResponse> {
        let i = self.responses.binary_search_by_key(&id, |r| r.0).ok()?;
        let (_, r, unsure) = self.responses[i];
        let response = super::events_data::RESPONSES.get(usize::from(r))?;
        let says = text.is_some_and(|t| response.text.contains(&t));
        let contradicts = text.is_some() && !response.text.is_empty() && !says;
        (!contradicts && (!unsure || says)).then_some(response)
    }
}

/// The group whose event scripts are exactly those of every group's that the
/// film's `scripts` (lower-cased file names) include.
pub(crate) fn group(scripts: &BTreeSet<String>) -> Option<&'static EventGroup> {
    let groups = super::events_data::EVENT_GROUPS;
    let have: BTreeSet<&str> =
        groups.iter().flat_map(|g| g.files.iter().copied()).filter(|&f| scripts.contains(f)).collect();
    groups.iter().find(|g| g.files.iter().copied().collect::<BTreeSet<_>>() == have)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::events_data::{EVENT_GROUPS, RESPONSES};

    fn names(n: &[&str]) -> BTreeSet<String> {
        n.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_film_runs_one_group() {
        let btb = group(&names(&["gameevents.lua", "bigteambattle.lua", "btbslayerevents.lua"])).unwrap();
        assert_eq!(btb.files, ["bigteambattle.lua", "btbslayerevents.lua"]);
        // Stockpile shares Big Team Battle's file, and has one of its own.
        let stockpile = group(&names(&["bigteambattle.lua", "btbstockpileevents.lua"])).unwrap();
        assert_eq!(stockpile.files, ["bigteambattle.lua", "btbstockpileevents.lua"]);
        assert!(group(&names(&["bigteambattle.lua"])).is_none());
        assert_eq!(btb.destroys.len(), 13);
        assert_eq!(btb.hijacks.len(), 13);
    }

    #[test]
    fn ids_are_in_order_and_name_responses() {
        for g in EVENT_GROUPS {
            assert!(g.responses.windows(2).all(|w| w[0].0 < w[1].0), "{:?}", g.files);
            assert!(g.responses.iter().all(|r| usize::from(r.1) < RESPONSES.len()));
        }
        // A destroy response names its personal score.
        let btb = group(&names(&["bigteambattle.lua", "btbslayerevents.lua"])).unwrap();
        let (id, _) = btb.destroys[3];
        let ghost = btb.response(id, None).unwrap();
        assert_eq!(ghost.name, "ghostDestroyResponse");
        assert_eq!(ghost.score.map(|s| s.0), Some(50));
    }
}
