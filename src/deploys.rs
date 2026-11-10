//! Equipment use: EquipmentSpawnedObject (event 103) names the equipment and
//! the object it put in the world, but not who used it. The object appears
//! next to its user, so the user is the player nearest its first position.

use crate::payload::{OpKind, Payload};
use std::collections::HashMap;

/// The event's type.
const EQUIPMENT_SPAWNED_OBJECT: u32 = 103;
/// Frames after the event the object's first position may come.
const FIRST_POSITION_FRAMES: i64 = 30;

/// One use: the frame, the equipment's tag, the object's tag and its first
/// position (box index, codes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Use {
    pub frame: i64,
    pub equipment: u32,
    pub object: u32,
    pub at: [u32; 3],
    /// The box `at` is in.
    pub at_box: u32,
}

/// What the walk gives: each entity's creates and positions, and the
/// equipment events.
#[derive(Default, Debug)]
pub struct Seen {
    creates: HashMap<u32, Vec<(i64, u32)>>,
    positions: HashMap<u32, Vec<(i64, (u32, [u32; 3]))>>,
    events: Vec<(i64, u32, u32)>,
}

pub fn reader(seen: &mut Seen) -> impl FnMut(usize, &[u8], &Payload) + '_ {
    let Seen { creates, positions, events } = seen;
    move |i, _, p| {
        for op in &p.ops {
            match &op.kind {
                OpKind::Create { tag: Some(t) } => creates.entry(op.index).or_default().push((i as i64, *t)),
                OpKind::Update { fields, .. } => {
                    if let Some(pos) = fields.position {
                        positions.entry(op.index).or_default().push((i as i64, pos));
                    }
                }
                _ => {}
            }
        }
        for e in p.events.iter().filter(|e| e.kind == EQUIPMENT_SPAWNED_OBJECT) {
            if let (Some(a), Some(b)) = (e.handles[0], e.handles[1]) {
                events.push((i as i64, a.entity(), b.entity()));
            }
        }
    }
}

/// Each equipment event with the equipment, the object it put down and
/// where the object first stood.
pub fn finish(seen: Seen) -> Vec<Use> {
    let Seen { creates, positions, events } = seen;
    let mut out = Vec::new();
    for (f, a, b) in events {
        // The equipment's latest create before; the object's nearest the event.
        let equipment = creates.get(&a).and_then(|c| c.iter().rev().find(|x| x.0 <= f)).map(|x| x.1);
        let object = creates.get(&b).and_then(|c| c.iter().min_by_key(|x| (x.0 - f).abs())).map(|x| x.1);
        let at = positions
            .get(&b)
            .and_then(|v| v.iter().find(|x| x.0 >= f && x.0 - f <= FIRST_POSITION_FRAMES))
            .map(|x| x.1);
        if let (Some(equipment), Some(object), Some((at_box, at))) = (equipment, object, at) {
            out.push(Use { frame: f, equipment, object, at, at_box });
        }
    }
    out
}
