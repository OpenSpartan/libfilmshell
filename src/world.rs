//! What stood on the map: vehicles, weapons, equipment and devices, from the
//! type-2 snapshots (`snapshot.rs`).
//!
//! A snapshot lists every object alive at a chunk's start, every 20 s. An
//! object is kept when it stays in one place across two snapshots, so what
//! moves through (a driven vehicle, a weapon dropped mid-fight) leaves no
//! ghost. Held items have no position of their own (they read 0, 0, 0), and
//! crates and scenery are left out.

use crate::components::Params;
use crate::container::Film;
use crate::snapshot::{self, ENTITY_TABLE};
use std::collections::HashMap;

/// Entity types kept: equipment, vehicles, weapons, devices.
const KEPT: [u32; 4] = [37, 40, 42, 43];
/// Snapshots an object must stay in place for.
const MIN_SEEN: u32 = 2;
/// Engine units an object may drift between snapshots and still be in place.
const SAME_PLACE: f32 = 0.05;

/// One object in one place, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldObject {
    pub tag: u32,
    pub entity_type: u32,
    pub x: f64,
    pub y: f64,
    /// The first snapshot's frame it was there.
    pub from: i64,
    /// The first snapshot's frame it was gone; None when still there.
    pub to: Option<i64>,
}

struct Track {
    tag: u32,
    entity_type: u32,
    at: [f32; 3],
    from: i64,
    seen: u32,
    snapshot: u32,
}

impl Track {
    fn object(&self, to: Option<i64>) -> Option<WorldObject> {
        (self.seen >= MIN_SEEN).then(|| WorldObject {
            tag: self.tag,
            entity_type: self.entity_type,
            x: -10.0 * self.at[1] as f64,
            y: 10.0 * self.at[0] as f64,
            from: self.from,
            to,
        })
    }
}

/// The objects of `film`, by first frame, then position. Packet framing only.
pub fn read(film: &Film, params: Params) -> Vec<WorldObject> {
    let ctx = snapshot::Context::new(&film.chunks[0], params, params.box_bits);
    let mut live: HashMap<(u32, u32), Track> = HashMap::new();
    let mut out = Vec::new();
    let mut frame = 0i64;
    let mut snapshot = 0u32;
    for (ci, chunk) in film.chunks.iter().enumerate().skip(1) {
        let Some(ps) = film.packets(ci) else { continue };
        let start = frame;
        for pk in ps {
            if pk.kind == 0 {
                frame += 1;
            }
            if pk.kind != ENTITY_TABLE {
                continue;
            }
            snapshot += 1;
            for e in snapshot::entities(chunk, pk.start, pk.end, &ctx) {
                if !KEPT.contains(&e.entity_type) || e.flags != 3 {
                    continue;
                }
                let Some((made, state)) = snapshot::creation(chunk, &e) else { continue };
                // A vehicle's part (a turret) stands where the vehicle does.
                if crate::weapons::tag(made.tag).is_some_and(|t| t.part_of.is_some()) {
                    continue;
                }
                let Some(at) = snapshot::raw_position(chunk, &e, state) else { continue };
                if at == [0.0, 0.0, 0.0] {
                    continue;
                }
                let key = (e.index, e.generation);
                match live.get_mut(&key) {
                    Some(t) if t.tag == made.tag && (0..3).all(|k| (t.at[k] - at[k]).abs() <= SAME_PLACE) => {
                        t.seen += 1;
                        t.snapshot = snapshot;
                    }
                    other => {
                        if let Some(t) = other {
                            out.extend(t.object(Some(start)));
                        }
                        live.insert(
                            key,
                            Track { tag: made.tag, entity_type: e.entity_type, at, from: start, seen: 1, snapshot },
                        );
                    }
                }
            }
            live.retain(|_, t| {
                let here = t.snapshot == snapshot;
                if !here {
                    out.extend(t.object(Some(start)));
                }
                here
            });
        }
    }
    out.extend(live.values().filter_map(|t| t.object(None)));
    out.sort_by(|a, b| (a.from, a.tag).cmp(&(b.from, b.tag)).then(a.x.total_cmp(&b.x)).then(a.y.total_cmp(&b.y)));
    out
}
