//! Items picked up and weapons lying on the ground, from the tick packets
//! (`payload.rs`).
//!
//! A pickup is a `biped_pickup` event (9): its first handle is the biped
//! (an 8-bit index and generation: the life's slot id), its body the
//! inventory slot (0 and 1 weapons, 2 grenades, 3 equipment) and the
//! item's tag. Before a player spawns the event gives every player the
//! `unarmed` weapon; those are skipped. The biped's own updates say the
//! same for weapons (`weapon-state-type-info` per slot,
//! `components::Fields::weapons`).
//!
//! A weapon in a player's hands isn't replicated as an object of its own.
//! A weapon on the ground is an object (registry block 42) whose create
//! names its tag (`creation::read`) and whose updates move it until it
//! comes to rest. Picking one up is one tick: the event, and the ground
//! object deleted in the same packet. Weapons on racks and pads exist
//! before the recording starts and never move, so the film never names
//! them. A ground weapon deleted with no such pickup timed out (a dropped
//! weapon despawns) or was picked up in a packet the walker didn't read to
//! that entry.

use crate::events::Body;
use crate::payload::{OpKind, Payload};
use std::collections::HashMap;

/// Registry block of weapon objects.
const WEAPON: u32 = 42;
/// biped_pickup.
const BIPED_PICKUP: u32 = 9;
/// The `unarmed` weapon every player is given before spawning.
const UNARMED: u32 = 0x0000_7ca9;

/// An item taken into a biped's inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pickup {
    pub frame: i64,
    /// The biped's slot id (`(generation - 1) * 256 + index`, with its
    /// creates before, `recreate.rs`), as life assignment keys lives.
    pub slot_id: i32,
    /// Inventory slot: 0 and 1 weapons, 2 grenades, 3 equipment.
    pub slot: u32,
    pub tag: u32,
}

/// A weapon object on the ground: created (dropped, or spawned) at `from`,
/// deleted at `to`, resting at `at` (the position codes of its last update,
/// at the level's widths).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ground {
    pub tag: u32,
    pub from: i64,
    pub to: Option<i64>,
    pub at: [u32; 3],
    /// The box `at` is in.
    pub at_box: u32,
    /// Deleted in the packet in which a biped picked up the same weapon.
    pub taken: bool,
}

#[derive(Debug, Default)]
pub struct Inventory {
    pub pickups: Vec<Pickup>,
    pub ground: Vec<Ground>,
}

/// The pickups and ground weapons of `film`. Packet framing only.
pub fn reader(inv: &mut Inventory) -> impl FnMut(usize, &[u8], &Payload) + '_ {
    // Weapon entity to index into inv.ground (while it exists), and created
    // ones not yet positioned.
    let mut live: HashMap<u32, usize> = HashMap::new();
    let mut pending: HashMap<u32, (u32, i64)> = HashMap::new();
    move |frame, _, p| {
        let frame = frame as i64;
        // This packet's pickups and deleted ground weapons, matched after.
        let mut taken: Vec<u32> = Vec::new();
        let mut deleted: Vec<usize> = Vec::new();
        for e in p.events.iter().filter(|e| e.kind == BIPED_PICKUP) {
            let (Body::Pickup { slot, tag: Some(tag) }, Some(h)) = (e.body, e.handles[0]) else { continue };
            let Some(slot_id) = h.biped_index_slot().filter(|_| tag != UNARMED) else { continue };
            inv.pickups.push(Pickup { frame, slot_id, slot, tag });
            taken.push(tag);
        }
        for op in &p.ops {
            match (op.entity_type, &op.kind) {
                (Some(WEAPON), OpKind::Create { tag: Some(t) }) => {
                    live.remove(&op.index);
                    // A misread create names no tag of the game's.
                    if crate::weapons::tag(*t).is_some() {
                        pending.insert(op.index, (*t, frame));
                    }
                }
                (Some(WEAPON), OpKind::Update { fields, .. }) => {
                    let Some((bx, q)) = fields.position else { continue };
                    if let Some(&g) = live.get(&op.index) {
                        inv.ground[g].at = q;
                        inv.ground[g].at_box = bx;
                    } else if let Some((tag, from)) = pending.remove(&op.index) {
                        live.insert(op.index, inv.ground.len());
                        inv.ground.push(Ground { tag, from, to: None, at: q, at_box: bx, taken: false });
                    }
                }
                (Some(WEAPON), OpKind::Delete) => {
                    pending.remove(&op.index);
                    if let Some(g) = live.remove(&op.index) {
                        inv.ground[g].to = Some(frame);
                        deleted.push(g);
                    }
                }
                _ => {}
            }
        }
        for g in deleted {
            if let Some(k) = taken.iter().position(|&t| t == inv.ground[g].tag) {
                taken.swap_remove(k);
                inv.ground[g].taken = true;
            }
        }
    }
}
