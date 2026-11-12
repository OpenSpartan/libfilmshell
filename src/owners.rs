//! Who owns each biped life: a unit's unit-control component names the
//! player controlling it. A life's biped is created with it set to its
//! player, and it is cleared when the player dies. The id is the engine
//! player id the events and the scoreboard use (read from the films: on
//! every reference film the creates' ids agree with the kill events and the
//! death times). Bots are players and are named too; the AI of Firefight and
//! the minigames isn't.

use crate::payload::{OpKind, Payload};
use std::collections::HashMap;

/// Entity indexes of bipeds.
const BIPEDS: std::ops::Range<u32> = 512..768;

/// What the film says of one biped life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Control {
    /// The first player that controls it: (frame, engine slot).
    pub player: Option<(i64, u32)>,
    /// Where it was created: (frame, box index, position codes).
    pub spawn: Option<(i64, u32, [u32; 3])>,
    /// The first frame after that no player controls it, or it is deleted.
    pub end: Option<i64>,
    /// Whether `end` is the player giving it up, which a death does, rather
    /// than its deletion.
    pub released: bool,
}

/// Life slot -> what its creates and updates say.
pub type Controls = HashMap<i32, Control>;

/// A life whose player the film names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    /// The player's roster index.
    pub player: u32,
    pub spawn: Option<(i64, u32, [u32; 3])>,
    pub end: Option<i64>,
    /// See [`Control::released`].
    pub released: bool,
}

/// The lives of `controls` that a player controls, with the players' roster
/// indexes from `holders`.
pub fn owners(controls: &Controls, holders: &crate::holders::Holders) -> HashMap<i32, Owner> {
    controls
        .iter()
        .filter_map(|(&slot, c)| {
            let (frame, id) = c.player?;
            Some((slot, Owner { player: holders.api(id, frame), spawn: c.spawn, end: c.end, released: c.released }))
        })
        .collect()
}

/// The controls of a walk's payloads, into `out`.
pub fn reader(out: &mut Controls) -> impl FnMut(usize, &[u8], &Payload) + '_ {
    move |frame, _, p| {
        let frame = frame as i64;
        for op in &p.ops {
            if !BIPEDS.contains(&op.index) || op.generation == 0 {
                continue;
            }
            let slot = (op.generation as i32 - 1) * 256 + (op.index - BIPEDS.start) as i32;
            let fields = match &op.kind {
                OpKind::Update { fields, .. } => fields,
                OpKind::Create { .. } => {
                    let Some(f) = &op.init else { continue };
                    if let Some((bx, q)) = f.position {
                        out.entry(slot).or_default().spawn.get_or_insert((frame, bx, q));
                    }
                    f
                }
                OpKind::Delete => {
                    if let Some(c) = out.get_mut(&slot).filter(|c| c.player.is_some()) {
                        c.end.get_or_insert(frame);
                    }
                    continue;
                }
            };
            match fields.controller {
                Some(Some(id)) => {
                    out.entry(slot).or_default().player.get_or_insert((frame, id));
                }
                Some(None) => {
                    if let Some(c) = out.get_mut(&slot).filter(|c| c.player.is_some() && c.end.is_none()) {
                        c.end = Some(frame);
                        c.released = true;
                    }
                }
                None => {}
            }
        }
    }
}
