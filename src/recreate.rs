//! Bipeds and vehicles the game creates again under an entity index and
//! generation it used before.
//!
//! An index and its 2-bit generation name one object for the whole match on
//! most films: the generation counts the index's wraps, so a life's id
//! `(generation - 1) * 256 + index` names one life. A game that resets its
//! objects starts them over from the first index, generation and all: Action
//! Sack does at most of its minigame rounds (measured on films: 81 creates of
//! an index and generation used before in one Action Sack match, none in 97
//! others). Each such create is a new object, so an id counts them: [`SPAN`]
//! more for each create of its index and generation before its own.

use crate::events::{handle_kinds, Event, Handle};
use crate::payload::{EntityOp, OpKind};
use std::collections::{HashMap, HashSet};

/// How far an id moves for each time its index and generation were created
/// again: past every id of the three generations.
pub const SPAN: i32 = 768;

/// The bipeds' entity indexes, and the vehicles'.
const BIPEDS: std::ops::Range<u32> = 512..768;
const VEHICLES: std::ops::Range<u32> = 768..1024;

/// A biped's id: `(generation - 1) * 256 + index - 512`, [`SPAN`] more for
/// each `instance` (the creates of its index and generation before its own).
pub fn biped_id(index: u32, generation: u32, instance: u32) -> Option<i32> {
    id(BIPEDS, index, generation, instance)
}

/// A vehicle's id: `(generation - 1) * 256 + index - 768`, [`SPAN`] more for
/// each `instance`, as [`biped_id`].
pub fn vehicle_id(index: u32, generation: u32, instance: u32) -> Option<i32> {
    id(VEHICLES, index, generation, instance)
}

fn id(range: std::ops::Range<u32>, index: u32, generation: u32, instance: u32) -> Option<i32> {
    (range.contains(&index) && generation > 0)
        .then(|| (generation as i32 - 1) * 256 + (index - range.start) as i32 + SPAN * instance as i32)
}

/// The creates of the bipeds' and vehicles' indexes and generations after
/// their first, as a walk reads them.
#[derive(Clone, Debug, Default)]
pub struct Recreated {
    created: HashSet<(u32, u32)>,
    /// (entity index, generation) -> the frame of each create after the
    /// first, in order.
    again: HashMap<(u32, u32), Vec<i64>>,
}

impl Recreated {
    /// How many times entity `index` with `generation` had been created
    /// again by the end of `frame`.
    pub fn instance(&self, index: u32, generation: u32, frame: i64) -> u32 {
        self.again.get(&(index, generation)).map_or(0, |f| f.partition_point(|&x| x <= frame) as u32)
    }

    /// Whether any index and generation was created again.
    pub fn any(&self) -> bool {
        !self.again.is_empty()
    }

    /// Take the creates of a payload walked at `frame`, and number what it
    /// names: an entry by the creates of its object before it (a delete of
    /// the old object before the new one's create names the old), a handle
    /// by the creates to the end of the frame.
    pub fn stamp(&mut self, frame: i64, ops: &mut [EntityOp], events: &mut [Event]) {
        for op in ops.iter_mut() {
            if !(BIPEDS.start..VEHICLES.end).contains(&op.index) || op.generation == 0 {
                continue;
            }
            let key = (op.index, op.generation);
            if matches!(op.kind, OpKind::Create { .. }) && !self.created.insert(key) {
                self.again.entry(key).or_default().push(frame);
            }
            op.instance = self.instance(op.index, op.generation, frame);
        }
        // A projectile's create names its shooter's unit (a kind-1 handle).
        for h in ops.iter_mut().filter_map(|op| op.shooter.1.as_mut()) {
            h.instance = self.of_handle(h, 1, frame);
        }
        self.stamp_events(frame, events);
    }

    /// Number the handles of the events read at `frame` by the creates to its
    /// end.
    pub fn stamp_events(&self, frame: i64, events: &mut [Event]) {
        for e in events {
            let Some(kinds) = handle_kinds(e.kind) else { continue };
            for (h, kind) in e.handles.iter_mut().zip(kinds) {
                if let Some(h) = h {
                    h.instance = self.of_handle(h, kind, frame);
                }
            }
        }
    }

    fn of_handle(&self, h: &Handle, kind: u32, frame: i64) -> u32 {
        h.unit_entity(kind).map_or(0, |e| self.instance(e, h.generation, frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_count_the_creates_of_their_index_before() {
        assert_eq!(biped_id(512, 1, 0), Some(0));
        assert_eq!(biped_id(767, 3, 0), Some(767));
        assert_eq!(biped_id(513, 1, 2), Some(1 + 2 * SPAN));
        assert_eq!(vehicle_id(770, 2, 1), Some(256 + 2 + SPAN));
        assert_eq!(biped_id(768, 1, 0), None, "a vehicle's index");
        assert_eq!(biped_id(512, 0, 0), None, "generation 0 names nothing");
    }

    #[test]
    fn an_object_created_again_is_a_new_one_from_its_create() {
        let mut r = Recreated::default();
        let mut ops = |frame: i64, kinds: Vec<OpKind>| {
            let mut v: Vec<EntityOp> = kinds.into_iter().map(|kind| EntityOp::test(512, 1, kind)).collect();
            r.stamp(frame, &mut v, &mut []);
            v.iter().map(|o| o.instance).collect::<Vec<_>>()
        };
        assert_eq!(ops(10, vec![OpKind::Create { tag: None }]), [0]);
        assert_eq!(ops(20, vec![OpKind::Delete, OpKind::Create { tag: None }]), [0, 1], "the delete is the old one's");
        assert_eq!(ops(30, vec![OpKind::Create { tag: None }]), [2]);
        assert!(r.any());
        assert_eq!(r.instance(512, 1, 19), 0);
        assert_eq!(r.instance(512, 1, 20), 1);
        assert_eq!(r.instance(512, 2, 40), 0, "another generation");
    }
}
