//! Capture the Flag events from PlayerGameEventSmall (82), the game's
//! scoring and announcement events.
//!
//! The event's id is an index into the game's event list, its last 32 bits
//! a mask of the players it is about (bit 31 - player index). Six ids are
//! the flag events in Capture the Flag. The same ids turn up in other modes
//! with other meanings, so they are read only when the mode is CTF. A grab
//! carries where the flag was, and the second event of a return where the
//! returner stood.

use crate::container::Film;
use crate::events::{Body, Value};
use crate::payload::for_each_event_view;

/// Objective kinds.
pub const GRAB: u8 = 0;
pub const STEAL: u8 = 1;
pub const CAPTURE: u8 = 2;
pub const RETURN: u8 = 3;
pub const CARRIER_KILLED: u8 = 4;
pub const CAPTURE_ASSIST: u8 = 5;

/// Event id to kind in CTF, with the stat each id counts toward.
const CTF: [(u32, u8); 6] = [
    (0x2df, GRAB),           // FlagGrabs
    (0x2cf, STEAL),          // FlagSteals
    (0x2a9, CAPTURE),        // FlagCaptures
    (0x2be, RETURN),         // FlagReturns
    (0x2ed, CARRIER_KILLED), // FlagCarriersKilled
    (0x2f1, CAPTURE_ASSIST), // FlagCaptureAssists
];
/// The return's second event, which carries the returner's position.
const RETURN_AT: u32 = 0x2bf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Objective {
    pub frame: i64,
    pub kind: u8,
    pub player: i32,
    /// Position codes in the level box, when the event carries them.
    pub at: Option<[u32; 3]>,
}

/// The film's objective events in frame order; empty unless `ctf`.
pub fn read(film: &Film, widths: [u32; 3], ctf: bool) -> Vec<Objective> {
    let mut out = Vec::new();
    if !ctf {
        return out;
    }
    let pos = |g: &crate::events::GameEvent| {
        g.values.iter().flatten().find_map(|(_, v)| if let Value::Pos(p) = v { *p } else { None })
    };
    for_each_event_view(film, widths, |frame, evs| {
        let start = out.len();
        for e in evs {
            let Body::Game(g) = e.body else { continue };
            let players = (0..32).filter(|p| g.last & (0x8000_0000u32 >> p) != 0);
            if g.id == RETURN_AT {
                for p in players {
                    if let Some(o) =
                        out[start..].iter_mut().find(|o| o.kind == RETURN && o.player == p && o.at.is_none())
                    {
                        o.at = pos(&g);
                    }
                }
                continue;
            }
            let Some(&(_, kind)) = CTF.iter().find(|c| c.0 == g.id) else { continue };
            for p in players {
                out.push(Objective { frame: frame as i64, kind, player: p, at: pos(&g) });
            }
        }
    });
    out
}
