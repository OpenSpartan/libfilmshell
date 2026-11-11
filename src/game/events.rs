//! The modes' vehicle destroy and hijack events: the personal scores the
//! mode scripts give for destroying a hostile vehicle and for hijacking one,
//! by their ids in each mode's event tree.

use super::Vehicle;
use std::collections::BTreeSet;

/// A group of mode event scripts, and the ids its event tree gives the
/// vehicle responses.
#[derive(Debug)]
pub(crate) struct EventGroup {
    /// The group's own event scripts, lower-cased.
    pub files: &'static [&'static str],
    /// Each destroy response's id and vehicle.
    pub destroys: &'static [(u32, Vehicle)],
    /// Each hijack response's id and vehicle.
    pub hijacks: &'static [(u32, Vehicle)],
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

    #[test]
    fn a_film_runs_one_group() {
        let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>();
        let btb = group(&names(&["gameevents.lua", "bigteambattle.lua", "btbslayerevents.lua"])).unwrap();
        assert_eq!(btb.files, ["bigteambattle.lua", "btbslayerevents.lua"]);
        // Stockpile shares Big Team Battle's file, and has one of its own.
        let stockpile = group(&names(&["bigteambattle.lua", "btbstockpileevents.lua"])).unwrap();
        assert_eq!(stockpile.files, ["bigteambattle.lua", "btbstockpileevents.lua"]);
        assert!(group(&names(&["bigteambattle.lua"])).is_none());
        assert_eq!(btb.destroys.len(), 13);
        assert_eq!(btb.hijacks.len(), 13);
    }
}
