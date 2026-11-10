//! Who rides which vehicle, from the object-parent-state component.
//!
//! The game gathers an object's parent state from the object itself
//! (`142f0b490`, dirty bit 0x400): the parent object's network handle (the
//! object's `+0x18`), the parent's node it hangs from (`+0x145`), the local
//! offset and orientation, and for a unit the seat it sits in (`+0x146`, -1
//! when none: the index `vehicle_test_seat_unit` compares, `1431aef58`). The
//! client applies it with `1406c8990`, which seats the unit in that seat of
//! that vehicle (`1410dd658`) or unseats it. So a biped attached to a vehicle
//! is a rider in it, in the seat named, from the update that attaches it to
//! the one that detaches it, attaches it elsewhere or deletes it.
//!
//! The parent is a kind-1 handle: `[unit:1]`, a 9-bit unit index (bipeds
//! 0-255, vehicles 256-511) and a 2-bit generation, so a vehicle parent's id
//! is `(generation - 1) * 256 + index - 256`, the id `vehicles::id` gives.
//! Parts of a vehicle (a Falcon's side guns, a Wraith's or Scorpion's turret)
//! are vehicles attached to their vehicle the same way, and the Pelican
//! carrying a dropped vehicle has it attached in a seat; `Attachments::root`
//! follows these to the vehicle that moves.

use crate::components::Parent;
use crate::payload::{OpKind, Payload};
use std::collections::HashMap;

/// Entity indexes of bipeds and vehicles.
const BIPEDS: std::ops::Range<u32> = 512..768;
const VEHICLES: std::ops::Range<u32> = 768..1024;

/// One object attached to a vehicle, over a run of frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// The biped's life slot or the vehicle's id.
    pub id: i32,
    /// The first frame attached and the last.
    pub from: i64,
    pub to: i64,
    /// The vehicle id attached to (a part's, when the parent is a part).
    pub vehicle: i32,
    /// The seat, when the parent state names one.
    pub seat: Option<u32>,
    /// How the span ended (read by surveys only).
    #[cfg_attr(not(feature = "research"), allow(dead_code))]
    pub end: End,
    /// A biped any of whose updates lists actor control: AI (a bot,
    /// Firefight AI, a dropship's pilot). Seated, AI sends stubs like a
    /// player, so one update is the test.
    pub ai: bool,
}

/// How a span ended: detached, attached to another parent or seat,
/// deleted, or the film ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Detach,
    Change,
    Delete,
    Film,
}

#[derive(Debug, Default)]
pub struct Attachments {
    /// Bipeds in vehicles, by first frame.
    pub bipeds: Vec<Span>,
    /// Vehicles attached to vehicles (parts, carried vehicles), by first frame.
    pub vehicles: Vec<Span>,
    /// Whether the walker read biped updates, so that their parent state is
    /// known: false where it can't, and rides are inferred.
    pub read: bool,
}

/// What the reader has seen so far.
#[derive(Debug, Default)]
pub struct Seen {
    out: Attachments,
    /// Per entity (index, generation): the open span's first frame, vehicle
    /// and seat.
    open: HashMap<(u32, u32), (i64, i32, Option<u32>)>,
    /// Per life slot: updates, and those listing actor control.
    control: HashMap<i32, (u32, u32)>,
    last: i64,
}

fn slot(index: u32, generation: u32) -> Option<i32> {
    (BIPEDS.contains(&index) && generation > 0).then(|| (generation as i32 - 1) * 256 + (index - BIPEDS.start) as i32)
}

impl Seen {
    fn close(&mut self, key: (u32, u32), o: (i64, i32, Option<u32>), to: i64, end: End) {
        let biped = BIPEDS.contains(&key.0);
        let id = if biped { slot(key.0, key.1) } else { crate::vehicles::id(key.0, key.1) };
        let Some(id) = id else { return };
        let s = Span { id, from: o.0, to, vehicle: o.1, seat: o.2, end, ai: false };
        if biped {
            self.out.bipeds.push(s);
        } else {
            self.out.vehicles.push(s);
        }
    }
}

/// The attachments of a walk's payloads, into `seen`; `actor_control` is the
/// biped archetype's actor-control component.
pub fn reader(actor_control: u32, seen: &mut Seen) -> impl FnMut(usize, &[u8], &Payload) + '_ {
    move |frame, _, p| {
        let frame = frame as i64;
        seen.last = frame;
        for op in &p.ops {
            if !(BIPEDS.contains(&op.index) || VEHICLES.contains(&op.index)) {
                continue;
            }
            let key = (op.index, op.generation);
            let f = match &op.kind {
                OpKind::Update { fields, list } => {
                    if let Some(s) = slot(op.index, op.generation) {
                        seen.out.read = true;
                        let c = seen.control.entry(s).or_default();
                        c.0 += 1;
                        c.1 += list.contains(&actor_control) as u32;
                    }
                    fields
                }
                OpKind::Create { .. } => match &op.init {
                    Some(f) => f,
                    None => continue,
                },
                OpKind::Delete => {
                    if let Some(o) = seen.open.remove(&key) {
                        seen.close(key, o, frame, End::Delete);
                    }
                    continue;
                }
            };
            let Some(parent) = f.parent else { continue };
            let now = match (parent, f.parent_generation) {
                (Some(Parent::Unit(u)), Some(g)) => {
                    crate::vehicles::id(u + BIPEDS.start, g).map(|v| (v, f.seat.flatten()))
                }
                _ => None,
            };
            let was = seen.open.get(&key).map(|o| (o.1, o.2));
            if was == now {
                continue;
            }
            if let Some(o) = seen.open.remove(&key) {
                seen.close(key, o, frame - 1, if now.is_some() { End::Change } else { End::Detach });
            }
            if let Some((v, seat)) = now {
                seen.open.insert(key, (frame, v, seat));
            }
        }
    }
}

/// The attachments after the walk: spans still open end at the last frame.
pub fn finish(mut seen: Seen) -> Attachments {
    let mut rest: Vec<_> = std::mem::take(&mut seen.open).into_iter().collect();
    rest.sort_by_key(|x| x.0);
    let last = seen.last;
    for (key, o) in rest {
        seen.close(key, o, last, End::Film);
    }
    let mut out = seen.out;
    for s in out.bipeds.iter_mut() {
        s.ai = seen.control.get(&s.id).is_some_and(|c| c.1 > 0);
    }
    out.bipeds.sort_by_key(|s| (s.from, s.id));
    out.vehicles.sort_by_key(|s| (s.from, s.id));
    out
}

impl Attachments {
    /// The vehicle `id` is attached to at `frame`, if any.
    fn parent(&self, id: i32, frame: i64) -> Option<i32> {
        self.vehicles.iter().find(|s| s.id == id && s.from <= frame && frame <= s.to).map(|s| s.vehicle)
    }

    /// The vehicle that carries `id` at `frame`: `id` itself unless it is
    /// attached to another vehicle then (a part, or a vehicle in a dropship),
    /// followed up to four levels.
    pub fn root(&self, mut id: i32, frame: i64) -> i32 {
        for _ in 0..4 {
            match self.parent(id, frame) {
                Some(p) if p != id => id = p,
                _ => break,
            }
        }
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(id: i32, from: i64, to: i64, vehicle: i32) -> Span {
        Span { id, from, to, vehicle, seat: None, end: End::Detach, ai: false }
    }

    #[test]
    fn a_part_is_carried_by_its_vehicle() {
        // Vehicle 5 is a part of 2 over frames 10-20; 2 rides in 9 (a
        // dropship) over 15-30.
        let a = Attachments { bipeds: Vec::new(), vehicles: vec![span(5, 10, 20, 2), span(2, 15, 30, 9)], read: true };
        assert_eq!(a.root(5, 5), 5);
        assert_eq!(a.root(5, 12), 2);
        assert_eq!(a.root(5, 17), 9);
        assert_eq!(a.root(2, 31), 2);
    }
}
