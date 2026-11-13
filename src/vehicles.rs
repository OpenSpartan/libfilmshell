//! Vehicles by id: the tag a vehicle id stands for over time (from its
//! creates in the tick packets, and from the snapshots for vehicles created
//! before the film), and which vehicle each part belongs to.
//!
//! A vehicle id is `(generation - 1) * 256 + index - 768`, the id the vehicle
//! updates and `unit_exit_vehicle` use (`replication`, `events::Handle::
//! vehicle_slot`). Some vehicles are built from parts, each a vehicle tag of
//! its own that the vehicle's model attaches (the Falcon's side guns, the
//! Rocket Warthog's launcher; `game::TagInfo::part_of`). A part sends no
//! position updates, and a rider in its seat leaves the part. In the
//! snapshots a part stands where its vehicle stands, which is how a part id
//! is tied to its vehicle's id.

use crate::components::Params;
use crate::container::Film;
use crate::payload::{walk_shared, OpKind, Payload};
use crate::snapshot::{self, ENTITY_TABLE};
use std::collections::HashMap;

/// The registry block of vehicles.
pub const VEHICLE: u32 = 40;
/// The vehicles' entity indexes: the first, and the one after the last.
const FIRST: u32 = 768;
const END: u32 = 1024;
/// Engine units between a part and its vehicle in a snapshot.
const ATTACHED: f32 = 1.0;

/// The vehicle id of an entity, when it is in the vehicle range.
pub fn id(index: u32, generation: u32) -> Option<i32> {
    ((FIRST..END).contains(&index) && generation > 0).then(|| (generation as i32 - 1) * 256 + (index - FIRST) as i32)
}

fn is_part(tag: u32) -> bool {
    crate::weapons::tag(tag).is_some_and(|t| t.part_of.is_some())
}

#[derive(Debug, Default)]
pub struct Vehicles {
    /// Per id, (frame, tag) each time it was created, and when a snapshot
    /// shows it with a tag not known for it by then.
    tags: HashMap<i32, Vec<(i64, u32)>>,
    /// Per part id: (its tag, its vehicle's id).
    parent: HashMap<i32, (u32, i32)>,
    /// Per id, the frames of its deletes, in order.
    deletes: HashMap<i32, Vec<i64>>,
    /// Per id, (frame, forward unit vector in engine axes) of each update
    /// that sends its orientation, in frame order.
    forwards: HashMap<i32, Vec<(i64, [f64; 3])>>,
    /// Per id, (frame, health as a fraction of its maximum) of each update
    /// that sends it, in frame order.
    health: HashMap<i32, Vec<(i64, f64)>>,
}

impl Vehicles {
    /// Packet framing only (the tick packets' creates and the snapshots).
    pub fn read(film: &Film, params: Params) -> Vehicles {
        let mut v = Vehicles::default();
        walk_shared(film, params, &mut [&mut v.reader()]);
        v.finish(film, params);
        v
    }

    /// [`Vehicles::read`]'s creates, for a shared walk; [`Vehicles::finish`]
    /// adds the snapshots after it.
    pub fn reader(&mut self) -> impl FnMut(usize, &[u8], &Payload) + '_ {
        move |i, _, p| {
            for op in &p.ops {
                let Some(id) = id(op.index, op.generation).filter(|_| op.entity_type.is_none_or(|e| e == VEHICLE))
                else {
                    continue;
                };
                match &op.kind {
                    OpKind::Create { tag: Some(t) } => self.tags.entry(id).or_default().push((i as i64, *t)),
                    OpKind::Delete => self.deletes.entry(id).or_default().push(i as i64),
                    OpKind::Update { fields, .. } => {
                        if let Some(forward) = fields.orientation.and_then(|o| o.forward()) {
                            self.forwards.entry(id).or_default().push((i as i64, forward));
                        }
                        if let Some(h) = fields.health {
                            self.health.entry(id).or_default().push((i as i64, h));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The snapshots' vehicles and parts, after the walk.
    pub fn finish(&mut self, film: &Film, params: Params) {
        let v = self;
        let ctx = snapshot::Context::new(&film.chunks[0], params, params.box_bits);
        let mut frame = 0i64;
        for (ci, chunk) in film.chunks.iter().enumerate().skip(1) {
            let Some(ps) = film.packets(ci) else { continue };
            for pk in ps {
                if pk.kind == 0 {
                    frame += 1;
                }
                if pk.kind != ENTITY_TABLE {
                    continue;
                }
                let mut here: Vec<(i32, u32, Option<[f32; 3]>)> = Vec::new();
                for e in snapshot::entities(chunk, pk.start, pk.end, &ctx) {
                    let Some(id) = id(e.index, e.generation).filter(|_| e.entity_type == VEHICLE) else { continue };
                    let Some((made, state)) = snapshot::creation(chunk, &e) else { continue };
                    v.note(id, frame, made.tag);
                    here.push((id, made.tag, snapshot::raw_position(chunk, &e, state)));
                }
                v.attach(&here);
            }
        }
        for t in v.tags.values_mut() {
            t.sort_unstable();
        }
    }

    /// A snapshot shows `id` as `tag` at `frame`: noted unless the latest
    /// create or sighting before it says so already (the creates are read
    /// first, so a vehicle created before the film is known from its first
    /// snapshot).
    fn note(&mut self, id: i32, frame: i64, tag: u32) {
        let t = self.tags.entry(id).or_default();
        let known = t.iter().filter(|x| x.0 <= frame).max_by_key(|x| x.0).map(|x| x.1);
        if known != Some(tag) {
            t.push((frame, tag));
        }
    }

    /// Frames from which `id` stands for a new vehicle: its creates and new
    /// tags, in order.
    pub fn starts(&self, id: i32) -> &[(i64, u32)] {
        self.tags.get(&id).map_or(&[], Vec::as_slice)
    }

    /// When the vehicle `id` named at `frame` left the map: its first delete
    /// at or after `frame`, or the id's next start, when that comes first
    /// (the id then names another vehicle, so this one is gone). None if
    /// neither follows.
    pub fn removed(&self, id: i32, frame: i64) -> Option<i64> {
        let delete = self.deletes.get(&id).and_then(|d| d.iter().find(|&&f| f >= frame).copied());
        let start = self.starts(id).iter().map(|s| s.0).find(|&f| f > frame);
        delete.into_iter().chain(start).min()
    }

    /// The forward directions `id` sent from frame `from` up to `until`
    /// (not inclusive), engine axes.
    pub fn forwards(&self, id: i32, from: i64, until: Option<i64>) -> &[(i64, [f64; 3])] {
        let all = self.forwards.get(&id).map_or(&[][..], Vec::as_slice);
        let a = all.partition_point(|x| x.0 < from);
        let b = until.map_or(all.len(), |u| all.partition_point(|x| x.0 < u));
        &all[a..b.max(a)]
    }

    /// The health readings `id` sent from frame `from` up to `until` (not
    /// inclusive).
    pub fn health(&self, id: i32, from: i64, until: Option<i64>) -> &[(i64, f64)] {
        let all = self.health.get(&id).map_or(&[][..], Vec::as_slice);
        let a = all.partition_point(|x| x.0 < from);
        let b = until.map_or(all.len(), |u| all.partition_point(|x| x.0 < u));
        &all[a..b.max(a)]
    }

    /// Ties each part in a snapshot to the vehicle standing where it does.
    fn attach(&mut self, here: &[(i32, u32, Option<[f32; 3]>)]) {
        for &(id, tag, at) in here.iter().filter(|x| is_part(x.1)) {
            if self.parent.get(&id).is_some_and(|p| p.0 == tag) {
                continue;
            }
            let Some(at) = at.filter(|a| *a != [0.0; 3]) else { continue };
            let near = here
                .iter()
                .filter(|x| !is_part(x.1))
                .filter_map(|&(j, _, p)| {
                    let p = p?;
                    let d = (0..3).map(|k| (p[k] - at[k]).powi(2)).sum::<f32>().sqrt();
                    (d <= ATTACHED).then_some((d, j))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, j)) = near {
                self.parent.insert(id, (tag, j));
            }
        }
    }

    /// The tag `id` stood for at `frame`: the latest known by then, else the
    /// first known after.
    pub fn tag(&self, id: i32, frame: i64) -> Option<u32> {
        let t = self.tags.get(&id)?;
        t.iter().rev().find(|x| x.0 <= frame).or(t.first()).map(|x| x.1)
    }

    /// The vehicle a ride in `id` is a ride in: its vehicle when `id` is a
    /// part tied to one, else `id`.
    pub fn resolve(&self, id: i32) -> i32 {
        self.parent.get(&id).map_or(id, |p| p.1)
    }
}
