//! What a zone's navpoint says: its owner, the capture under way, whether it
//! is contested or counting down, and its letter.
//!
//! A navpoint is the game's world marker. What it shows is a set of visual
//! state groups, each a name, a state (both string ids of the `nvpt` tag's
//! names) and a filter whose clauses say who sees it. A film carries the
//! groups in its navpoint entities' components; a running game holds the same
//! groups in memory. [`ZoneMarks::read`] is how the library reads a zone's
//! groups out of films, for a reader of either.

/// A filter clause, as films carry it.
///
/// `kind` 2 is a player mask, bit 31 minus each player's index (a running
/// game holds bit p instead: [`ZoneMarks`] only asks whether a mask names
/// anyone); 3 a team, its id plus one (0 none). `negated` turns it into "all
/// but".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clause {
    /// 2 a player mask, 3 a team.
    pub kind: u32,
    /// Everyone but what it names.
    pub negated: bool,
    /// The mask, or the team plus one.
    pub value: u32,
}

/// One visual state group: its name and state, and its filter's clauses.
#[derive(Clone, Copy, Debug)]
pub struct StateGroup<'a> {
    /// The group's name, a string id: [`HAS_PROGRESS`], [`HAS_TEXT`], [`CONTROL_POINT`], ...
    pub name: u32,
    /// Its state, a string id: [`CONTESTED`], [`INCOMING`], a letter's, ...
    pub state: u32,
    /// Its filter's clauses.
    pub clauses: &'a [Clause],
}

/// The group that carries a capture: its team captures, its state is
/// [`CONTESTED`] while the zone is contested.
pub const HAS_PROGRESS: u32 = 0x0b55_6074;
/// The group that carries a zone's text: [`INCOMING`] while a hill counts down
/// to going live; its second player mask is who contests.
pub const HAS_TEXT: u32 = 0x7244_9896;
/// The group whose state is the zone's letter: the string id of 'a', 'b', ...
pub const CONTROL_POINT: u32 = 0x2946_0f42;
/// `has_progress`'s state while its zone is contested.
pub const CONTESTED: u32 = 0x63e8_e53f;
/// `has_text`'s state while its hill counts down to going live: the
/// navpoint's radial progress is then the countdown, not a capture.
pub const INCOMING: u32 = 0x897e_705d;

/// What a zone's navpoint says (King of the Hill's hills, Strongholds' zones).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ZoneMarks {
    /// The team that holds it: its first group's team.
    pub owner: Option<u8>,
    /// The team whose capture its progress is.
    pub capturing: Option<u8>,
    /// Players of more than one team inside.
    pub contested: bool,
    /// A hill counting down to going live.
    pub countdown: bool,
    /// 'A' to 'Z'.
    pub letter: Option<char>,
}

impl ZoneMarks {
    /// Reads one of the navpoint's groups into the marks, in the order the
    /// navpoint holds them. The first group's team is the owner. Of the others,
    /// by name: `has_progress`'s team is the capturing team and its state says
    /// whether the zone is contested; `has_text`'s state says whether a hill
    /// counts down, and a second player mask that names anyone contests it
    /// (Ranked King of the Hill sets no contested state); `control_point`'s
    /// state is the letter.
    pub fn read(&mut self, first: bool, group: &StateGroup<'_>) {
        if first {
            self.owner = team(group.clauses);
            return;
        }
        match group.name {
            HAS_PROGRESS => {
                self.capturing = team(group.clauses);
                self.contested = group.state == CONTESTED;
            }
            HAS_TEXT => {
                self.countdown = group.state == INCOMING;
                let mut masks = group.clauses.iter().filter(|c| c.kind == 2).map(|c| c.value);
                if masks.nth(1).is_some_and(|m| m != 0 && m != u32::MAX) {
                    self.contested = true;
                }
            }
            CONTROL_POINT => {
                if let Some(letter) = letter_of(group.state) {
                    self.letter = Some(letter);
                }
            }
            _ => {}
        }
    }
}

/// The players a filter's first player mask names (a zone's docking filter:
/// who is inside), 0 for none. A mask naming everyone is the filter's resting
/// value, so none too.
pub fn players(clauses: &[Clause]) -> u32 {
    clauses.iter().find(|c| c.kind == 2 && !c.negated).map_or(0, |c| if c.value == u32::MAX { 0 } else { c.value })
}

/// The first team clause's team.
fn team(clauses: &[Clause]) -> Option<u8> {
    clauses.iter().find(|c| c.kind == 3 && !c.negated).and_then(|c| u8::try_from(c.value).ok()?.checked_sub(1))
}

/// The letter whose string id is `id`, 'A' to 'Z'.
pub fn letter_of(id: u32) -> Option<char> {
    (b'a'..=b'z').find(|&c| crate::game::murmur3(&[c]) == id).map(|c| (c - 32) as char)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn clause(kind: u32, value: u32) -> Clause {
        Clause { kind, negated: false, value }
    }

    #[test]
    fn a_zone_reads_its_owner_capture_contest_countdown_and_letter() {
        let mut marks = ZoneMarks::default();
        marks.read(true, &StateGroup { name: 0, state: 0, clauses: &[clause(3, 2)] });
        let progress = [clause(3, 1)];
        marks.read(false, &StateGroup { name: HAS_PROGRESS, state: CONTESTED, clauses: &progress });
        let b = crate::game::murmur3(b"b");
        marks.read(false, &StateGroup { name: CONTROL_POINT, state: b, clauses: &[] });
        assert_eq!(
            marks,
            ZoneMarks { owner: Some(1), capturing: Some(0), contested: true, countdown: false, letter: Some('B') }
        );

        // Ranked: no contested state, but a second player mask that names someone.
        let mut ranked = ZoneMarks::default();
        let text = [clause(2, 0x30), clause(2, 0x10)];
        ranked.read(false, &StateGroup { name: HAS_TEXT, state: INCOMING, clauses: &text });
        assert!(ranked.contested && ranked.countdown);
        let resting = [clause(2, 0x30), clause(2, u32::MAX)];
        let mut idle = ZoneMarks::default();
        idle.read(false, &StateGroup { name: HAS_TEXT, state: 0, clauses: &resting });
        assert!(!idle.contested && !idle.countdown);
    }

    #[test]
    fn a_docking_mask_naming_everyone_names_nobody() {
        assert_eq!(players(&[clause(2, 0x30)]), 0x30);
        assert_eq!(players(&[clause(2, u32::MAX)]), 0);
        assert_eq!(players(&[Clause { kind: 2, negated: true, value: 1 }]), 0);
    }
}
