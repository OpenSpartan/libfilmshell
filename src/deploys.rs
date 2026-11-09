//! Equipment use: EquipmentSpawnedObject (event 103) names the equipment and
//! the object it put in the world, but not who used it. The object appears
//! next to its user, so the user is the player nearest its first position.

use crate::components::Params;
use crate::container::Film;
use crate::payload::{for_each_payload, OpKind, Walker};
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

pub fn read(film: &Film, params: Params) -> Vec<Use> {
    let mut walker = Walker::new(&film.chunks[0], params);
    let mut creates: HashMap<u32, Vec<(i64, u32)>> = HashMap::new();
    let mut positions: HashMap<u32, Vec<(i64, (u32, [u32; 3]))>> = HashMap::new();
    let mut events = Vec::new();
    for_each_payload(film, &mut walker, |i, p| {
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
    });
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
