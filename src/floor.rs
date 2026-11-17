//! Floor plans from a navigation mesh: the outline of the floor players walk,
//! in world units, as rings a map viewer fills.
//!
//! A navigation mesh is the walkable surface as convex faces, each a run of
//! edges in order round it. An edge names its start and end vertex and, where
//! another face meets it, that face and its edge there, which runs the other
//! way. An edge no face meets is on the boundary, and the boundary edges chain
//! end to start into rings. The library takes a mesh in these plain terms
//! ([`NavMesh`]), from a reader of a map's files or of a running game. Its
//! vertices are engine units in the mesh's own frame, which its [`Frame`]
//! places in the level.
//!
//! Two faces that meet don't always share the vertices of the edge between
//! them: at a seam a mesh repeats a corner, once per side, at the same place
//! and up to 2 world units apart in height (measured on the baked Forge maps'
//! meshes). Each edge names its opposite edge, though, so the corners an edge
//! and its opposite edge join are one corner, and the boundary chains through
//! it. Chained by vertex alone, a ring would stop at such a corner and the
//! rest of it fall apart into runs that never close.
//!
//! [`NavMesh::rings`] makes the plan. Each ring is in world units (world x is
//! `-10 * y`, world y `10 * x`, world z `10 * z`, as everywhere in the
//! library), a ring with less area than [`MIN_AREA`] is dropped, and the rest
//! are simplified (Douglas-Peucker at [`TOLERANCE`], split at a ring's first
//! corner and the last of those farthest from it). A ring's winding says what
//! it is: counter-clockwise it is floor, clockwise a hole in the floor round
//! it ([`Ring::hole`]). Filled with the nonzero rule the rings are the floor,
//! every storey in one plan, and [`Ring::z`] is the range of heights a ring
//! spans, for a viewer that shows one storey at a time.
//!
//! A mesh baked for a Forge map covers the map's whole canvas, its floor and
//! backdrop too. [`NavMesh::rings_within`] keeps only where players may be: it
//! leaves out the faces in a kill volume and in no safe volume of its kind
//! ([`playable`]), and, given where the game is played (the map's spawn points
//! and objectives), every piece of the mesh away from it.
//!
//! The arithmetic is single precision where a mesh's numbers are, and the
//! same on every platform. [`Ring::rounded`] gives a ring to 0.1 world units,
//! as a plan file holds it:
//!
//! ```text
//! {"z": [lowest, highest], "hole": false, "p": [x0, y0, x1, y1, ...]}
//! ```
//!
//! ```
//! use filmshell::floor::{Edge, Face, NavMesh};
//!
//! // One face: a square 2 engine units across, 1 up.
//! let vertices = vec![[0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [2.0, 2.0, 1.0], [0.0, 2.0, 1.0]];
//! let edges = (0..4).map(|i| Edge { start: i, end: (i + 1) % 4, ..Edge::default() }).collect();
//! let faces = vec![Face { first_edge: 0, edge_count: 4 }];
//! let mesh = NavMesh { vertices, faces, edges, ..NavMesh::default() };
//!
//! let rings = mesh.rings();
//! assert_eq!(rings.len(), 1);
//! assert!(!rings[0].hole);
//! assert_eq!(rings[0].z, [10.0, 10.0]);
//! // Engine (x, y) is world (-10 y, 10 x): a square 20 world units across.
//! assert_eq!(rings[0].p, [0.0, 0.0, 0.0, 20.0, -20.0, 20.0, -20.0, 0.0]);
//! ```

use std::collections::HashMap;
use std::ops::Range;

/// The Douglas-Peucker tolerance, world units: how far a simplified ring may
/// stray from the mesh's boundary.
pub const TOLERANCE: f64 = 0.5;
/// Rings with less area than this, square world units, are crumbs and left
/// out.
pub const MIN_AREA: f64 = 2.0;
/// How far apart (engine units, across) the corners an edge and its opposite
/// edge join may be. Repeated corners lie within 0.0005 of each other. One
/// pair of edges in all the baked Forge maps meets only in part, its corners
/// 0.87 apart, and is left unjoined.
const SAME_CORNER: f32 = 0.01;

/// A navigation mesh in plain terms: its vertices, its faces as runs of
/// edges, and where it sits.
///
/// An edge that names a vertex the mesh doesn't have is left out, as is the
/// part of a face's run of edges past the last edge.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NavMesh {
    /// The vertices, engine units in the mesh's frame: `[x, y, z]`.
    pub vertices: Vec<[f32; 3]>,
    /// The faces, each a run of [`edges`](NavMesh::edges).
    pub faces: Vec<Face>,
    /// The edges, each face's in order round it.
    pub edges: Vec<Edge>,
    /// Where the mesh sits in the level.
    #[cfg_attr(feature = "serde", serde(default))]
    pub frame: Frame,
}

/// A face of a mesh: a convex polygon, its edges a run of the mesh's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Face {
    /// Its first edge, an index into [`NavMesh::edges`].
    pub first_edge: usize,
    /// How many edges it has, from the first on.
    pub edge_count: usize,
}

/// An edge of a face, from one corner to the next round it.
///
/// A mesh file's all-ones index for no face or edge is `None` here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Edge {
    /// Its start vertex, an index into [`NavMesh::vertices`].
    pub start: usize,
    /// Its end vertex.
    pub end: usize,
    /// The edge along this one on the face across, running the other way.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub opposite_edge: Option<usize>,
    /// The face across, None on the boundary.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub opposite_face: Option<usize>,
}

/// Where a mesh sits in the level: a rotation and a translation, engine
/// units. A mesh baked where it is used has the identity.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frame {
    /// The rotation's columns: where the mesh's x, y and z axes point.
    pub columns: [[f32; 3]; 3],
    /// Where the mesh's origin is.
    pub translation: [f32; 3],
}

impl Frame {
    /// The frame that leaves a mesh where it is.
    pub const IDENTITY: Frame =
        Frame { columns: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], translation: [0.0; 3] };

    /// A point of the mesh in the level, engine units.
    pub fn apply(&self, v: [f32; 3]) -> [f32; 3] {
        if *self == Frame::IDENTITY {
            return v;
        }
        let [c0, c1, c2] = self.columns;
        [0, 1, 2].map(|k| c0[k] * v[0] + c1[k] * v[1] + c2[k] * v[2] + self.translation[k])
    }
}

impl Default for Frame {
    fn default() -> Frame {
        Frame::IDENTITY
    }
}

/// One ring of a floor plan, world units.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ring {
    /// The lowest and the highest of its boundary's corners.
    pub z: [f64; 2],
    /// Clockwise: a hole in the floor round it.
    pub hole: bool,
    /// Its corners in order, as pairs: `x0, y0, x1, y1, ...`.
    pub p: Vec<f64>,
}

impl Ring {
    /// The ring to 0.1 world units, as a plan file holds it: each number in
    /// single precision, as a mesh's are, to the nearest tenth, and never -0.
    pub fn rounded(&self) -> Ring {
        Ring { z: self.z.map(tenth), hole: self.hole, p: self.p.iter().map(|&v| tenth(v)).collect() }
    }
}

/// `v` to the nearest 0.1 in single precision: the double nearest the tenth,
/// so it prints as the tenth.
fn tenth(v: f64) -> f64 {
    f64::from((v as f32 * 10.0).round()) / 10.0 + 0.0
}

/// A mesh's boundary ([`NavMesh::boundary`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Boundary {
    /// Each ring's corners, indices into [`NavMesh::vertices`], in the order
    /// the boundary runs: the start of each of its edges.
    pub rings: Vec<Vec<usize>>,
    /// How many of them are runs that never came back to their start. A plan
    /// closes such a run with a straight edge, a sliver across the map. None
    /// should be left: a mesh whose edges meet only in part leaves a few short
    /// ones.
    pub open: usize,
}

/// A plan cut to where players may be ([`NavMesh::rings_within`]).
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Within {
    /// The plan's rings.
    pub rings: Vec<Ring>,
    /// How many faces are in volumes players may not stay in.
    pub out_of_play: usize,
    /// How many faces are in pieces of the mesh away from the game.
    pub apart: usize,
    /// How many boundary runs never closed ([`Boundary::open`]).
    pub open: usize,
}

/// Engine axes and units to world: `[-10 y, 10 x, 10 z]`.
fn to_world(v: [f32; 3]) -> [f32; 3] {
    [-10.0 * v[1], 10.0 * v[0], 10.0 * v[2]]
}

/// The class of `x` among joined items, halving paths as it goes.
fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// Joins the classes of `a` and `b`: the lower index stands for both.
fn join(parent: &mut [usize], a: usize, b: usize) {
    let (a, b) = (find(parent, a), find(parent, b));
    if a != b {
        parent[a.max(b)] = a.min(b);
    }
}

impl NavMesh {
    /// A face's edges, as far as the mesh has them.
    fn face_edges(&self, face: &Face) -> Range<usize> {
        let end = face.first_edge.saturating_add(face.edge_count).min(self.edges.len());
        face.first_edge.min(end)..end
    }

    /// A point of the mesh in world units.
    fn world(&self, v: [f32; 3]) -> [f32; 3] {
        to_world(self.frame.apply(v))
    }

    /// The faces across a face's edges.
    fn neighbours(&self, f: usize) -> impl Iterator<Item = usize> + '_ {
        let faces = self.faces.len();
        self.face_edges(&self.faces[f]).filter_map(|e| self.edges[e].opposite_face).filter(move |&g| g < faces)
    }

    /// A face's corners in world units, and its centre.
    fn face_world(&self, f: usize) -> (Vec<[f64; 3]>, [f64; 3]) {
        let corners: Vec<[f64; 3]> = self
            .face_edges(&self.faces[f])
            .filter_map(|e| self.vertices.get(self.edges[e].start))
            .map(|&v| self.world(v).map(f64::from))
            .collect();
        let n = corners.len().max(1) as f64;
        let mean = |k: usize| corners.iter().map(|c| c[k]).sum::<f64>() / n;
        let centre = [mean(0), mean(1), mean(2)];
        (corners, centre)
    }

    /// The boundary of the faces `keep` keeps (by face index): an edge of a
    /// kept face is on it when no face is across, or a face `keep` drops.
    ///
    /// The rings come in the order of the faces and edges they start from,
    /// each chained end to start through the corners its edges join.
    pub fn boundary(&self, keep: impl Fn(usize) -> bool) -> Boundary {
        let (vertices, edges) = (self.vertices.len(), self.edges.len());
        let kept: Vec<bool> = (0..self.faces.len()).map(keep).collect();
        // The corners an edge and its opposite edge join are one corner.
        let mut parent: Vec<usize> = (0..vertices).collect();
        for e in self.faces.iter().flat_map(|face| self.face_edges(face)).map(|e| &self.edges[e]) {
            if e.opposite_face.is_none() {
                continue;
            }
            let Some(o) = e.opposite_edge.and_then(|o| self.edges.get(o)) else { continue };
            for (a, b) in [(e.start, o.end), (e.end, o.start)] {
                let (Some(va), Some(vb)) = (self.vertices.get(a), self.vertices.get(b)) else { continue };
                if (va[0] - vb[0]).abs() > SAME_CORNER || (va[1] - vb[1]).abs() > SAME_CORNER {
                    continue;
                }
                join(&mut parent, a, b);
            }
        }
        let corner: Vec<usize> = (0..vertices).map(|v| find(&mut parent, v)).collect();
        // The boundary edges, in the order found, and by start corner.
        let mut boundary = Vec::new();
        let mut from: HashMap<usize, Vec<usize>> = HashMap::new();
        for (f, face) in self.faces.iter().enumerate() {
            if !kept[f] {
                continue;
            }
            for e in self.face_edges(face) {
                let edge = &self.edges[e];
                if edge.start >= vertices || edge.end >= vertices {
                    continue;
                }
                if edge.opposite_face.is_none_or(|g| !kept.get(g).copied().unwrap_or(false)) {
                    from.entry(corner[edge.start]).or_default().push(e);
                    boundary.push(e);
                }
            }
        }
        let mut used = vec![false; edges];
        let (mut rings, mut open) = (Vec::new(), 0);
        for &e0 in &boundary {
            if used[e0] {
                continue;
            }
            let mut ring = Vec::new();
            let mut e = e0;
            loop {
                used[e] = true;
                ring.push(self.edges[e].start);
                let end = corner[self.edges[e].end];
                if end == corner[self.edges[e0].start] || ring.len() > edges {
                    break;
                }
                match from.get(&end).and_then(|next| next.iter().copied().find(|&x| !used[x])) {
                    Some(next) => e = next,
                    None => {
                        open += 1;
                        break;
                    }
                }
            }
            if ring.len() >= 3 {
                rings.push(ring);
            }
        }
        Boundary { rings, open }
    }

    /// The plan of the whole mesh: its boundary's rings in world units,
    /// crumbs dropped, simplified.
    pub fn rings(&self) -> Vec<Ring> {
        self.rings_where(|_| true)
    }

    /// The plan of the faces `keep` keeps (by face index).
    pub fn rings_where(&self, keep: impl Fn(usize) -> bool) -> Vec<Ring> {
        self.rings_of(self.boundary(keep).rings)
    }

    /// Boundary rings by vertex as a plan's rings.
    fn rings_of(&self, boundary: Vec<Vec<usize>>) -> Vec<Ring> {
        let mut out = Vec::new();
        for ring in boundary {
            let world: Vec<[f32; 3]> = ring.iter().map(|&v| self.world(self.vertices[v])).collect();
            let points: Vec<(f64, f64)> = world.iter().map(|v| (f64::from(v[0]), f64::from(v[1]))).collect();
            let a = area(&points);
            if a.abs() < MIN_AREA {
                continue;
            }
            let p = simplify(&points, TOLERANCE);
            if p.len() < 3 {
                continue;
            }
            let low = world.iter().map(|v| v[2]).fold(f32::MAX, f32::min);
            let high = world.iter().map(|v| v[2]).fold(f32::MIN, f32::max);
            out.push(Ring {
                z: [f64::from(low), f64::from(high)],
                hole: a < 0.0,
                p: p.into_iter().flat_map(|(x, y)| [x, y]).collect(),
            });
        }
        out
    }

    /// The plan where players may be. A face goes when the point just above
    /// its centre isn't [`playable`] among `volumes`, and with `anchors`
    /// (where the game is played: a map's spawn points and objectives, world
    /// units) so does every piece of the mesh away from them.
    ///
    /// A piece is a set of faces joined through the edges between them. It
    /// stays when one of its faces comes within 40 world units of an anchor,
    /// or has its centre in the anchors' box grown by 60 across and 30 up and
    /// down: pieces joined only by a jump or a lift stay, a canvas's floor
    /// far below the map, its backdrop and wall tops above the highest spawn
    /// go. With no anchors only the volumes cut the plan.
    pub fn rings_within(&self, volumes: &[Volume], anchors: &[[f64; 3]]) -> Within {
        let faces: Vec<(Vec<[f64; 3]>, [f64; 3])> = (0..self.faces.len()).map(|f| self.face_world(f)).collect();
        let mut keep: Vec<bool> = faces.iter().map(|(_, c)| playable(volumes, [c[0], c[1], c[2] + STANDING])).collect();
        let out_of_play = keep.iter().filter(|k| !**k).count();
        let mut apart = 0;
        if !anchors.is_empty() {
            let mut piece: Vec<usize> = (0..faces.len()).collect();
            for f in (0..faces.len()).filter(|&f| keep[f]) {
                for g in self.neighbours(f).filter(|&g| keep[g]) {
                    join(&mut piece, f, g);
                }
            }
            let range =
                |k: usize| anchors.iter().fold((f64::MAX, f64::MIN), |(lo, hi), a| (lo.min(a[k]), hi.max(a[k])));
            let (x, y, z) = (range(0), range(1), range(2));
            let in_play = |c: &[f64; 3]| {
                c[0] >= x.0 - PLAY_BOX
                    && c[0] <= x.1 + PLAY_BOX
                    && c[1] >= y.0 - PLAY_BOX
                    && c[1] <= y.1 + PLAY_BOX
                    && c[2] >= z.0 - PLAY_HEIGHT
                    && c[2] <= z.1 + PLAY_HEIGHT
            };
            let mut played = vec![false; faces.len()];
            for f in (0..faces.len()).filter(|&f| keep[f]) {
                let p = find(&mut piece, f);
                if !played[p]
                    && (in_play(&faces[f].1) || anchors.iter().any(|&a| face_distance(&faces[f].0, a) <= PLAY_REACH))
                {
                    played[p] = true;
                }
            }
            for f in 0..faces.len() {
                if keep[f] && !played[find(&mut piece, f)] {
                    keep[f] = false;
                    apart += 1;
                }
            }
        }
        let Boundary { rings, open } = self.boundary(|f| keep[f]);
        Within { rings: self.rings_of(rings), out_of_play, apart, open }
    }
}

/// The shoelace area, positive counter-clockwise.
fn area(p: &[(f64, f64)]) -> f64 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let ((x0, y0), (x1, y1)) = (p[i], p[(i + 1) % p.len()]);
        a += x0 * y1 - x1 * y0;
    }
    a / 2.0
}

// sqrt, not hypot: hypot comes from the platform's libm, and the same mesh
// must give the same plan everywhere.
fn length(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}

fn segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    length(p.0 - a.0 - t * dx, p.1 - a.1 - t * dy)
}

/// Douglas-Peucker on a closed ring, split at its first corner and the last
/// of those farthest from it. A stretch's farthest corner from the line
/// between its ends stays when it is farther than `tolerance`, and splits
/// the stretch in two.
fn simplify(p: &[(f64, f64)], tolerance: f64) -> Vec<(f64, f64)> {
    if p.len() <= 4 {
        return p.to_vec();
    }
    let from_first = |k: usize| length(p[k].0 - p[0].0, p[k].1 - p[0].1);
    let far = (1..p.len()).fold(1, |best, k| if from_first(k) >= from_first(best) { k } else { best });
    let mut keep = vec![false; p.len() + 1];
    keep[0] = true;
    keep[far] = true;
    let mut closed = p.to_vec();
    closed.push(p[0]);
    // Each stretch stands alone, so the order they are taken in doesn't
    // matter, and a stack keeps a long ring off the call stack.
    let mut stretches = vec![(0, far), (far, p.len())];
    while let Some((i, j)) = stretches.pop() {
        if j <= i + 1 {
            continue;
        }
        let (mut best, mut at) = (0.0, i);
        for k in i + 1..j {
            let d = segment_distance(closed[k], closed[i], closed[j]);
            if d > best {
                (best, at) = (d, k);
            }
        }
        if best > tolerance {
            keep[at] = true;
            stretches.push((i, at));
            stretches.push((at, j));
        }
    }
    (0..p.len()).filter(|&i| keep[i]).map(|i| p[i]).collect()
}

/// What a volume does to the players in it: a kill volume kills them (a soft
/// one after a countdown), a safe volume keeps the kill volumes of its kind
/// out of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum VolumeKind {
    /// Kills a player at once.
    Kill,
    /// Kills a player who stays in it past a countdown.
    SoftKill,
    /// Keeps kill volumes out of it.
    Safe,
    /// Keeps soft kill volumes out of it.
    SoftSafe,
}

/// A volume's shape across its up axis, world units.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum VolumeShape {
    /// A box.
    Box {
        /// Its whole length along the volume's forward axis.
        length: f64,
        /// Its whole width across it.
        width: f64,
    },
    /// A cylinder round the volume's up axis.
    Cylinder {
        /// Its radius.
        radius: f64,
    },
}

/// A volume a map places, world units: its centre, its axes (unit vectors,
/// which may point any way), its shape across the up axis, and how far it
/// reaches above and below its centre along it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Volume {
    /// What it does.
    pub kind: VolumeKind,
    /// Its centre.
    pub centre: [f64; 3],
    /// Its forward axis.
    pub forward: [f64; 3],
    /// Its up axis.
    pub up: [f64; 3],
    /// Its shape across the up axis.
    pub shape: VolumeShape,
    /// How far it reaches above its centre, along its up axis.
    pub top: f64,
    /// How far below.
    pub bottom: f64,
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Volume {
    /// Whether the volume holds `p`, world units.
    pub fn contains(&self, p: [f64; 3]) -> bool {
        let d = [p[0] - self.centre[0], p[1] - self.centre[1], p[2] - self.centre[2]];
        let h = dot(d, self.up);
        if h > self.top || h < -self.bottom {
            return false;
        }
        // What is left across the up axis.
        let a = [d[0] - h * self.up[0], d[1] - h * self.up[1], d[2] - h * self.up[2]];
        match self.shape {
            VolumeShape::Cylinder { radius } => dot(a, a) <= radius * radius,
            VolumeShape::Box { length, width } => {
                let along = dot(a, self.forward);
                let across = (dot(a, a) - along * along).max(0.0).sqrt();
                along.abs() <= length / 2.0 && across <= width / 2.0
            }
        }
    }
}

/// Whether players may stay at `p`, world units, among `volumes`.
///
/// They may when `p` is in no kill volume, or every kill volume it is in has
/// a safe volume of its kind round `p` too (a safe volume keeps out kill
/// volumes, a soft one soft kill volumes). Measured on the published Forge
/// maps: of the spawn points inside a kill volume, all but 20 of 1,249 are
/// inside a safe volume of its kind.
pub fn playable(volumes: &[Volume], p: [f64; 3]) -> bool {
    let inside = |kind| volumes.iter().any(|v| v.kind == kind && v.contains(p));
    let deadly = (inside(VolumeKind::Kill) && !inside(VolumeKind::Safe))
        || (inside(VolumeKind::SoftKill) && !inside(VolumeKind::SoftSafe));
    !deadly
}

/// How far above a face's centre its point stands, world units: a player's
/// feet are on the floor, and a kill volume under a floor stops short of it.
const STANDING: f64 = 1.0;
/// How near an anchor a piece of the mesh comes to stay, world units
/// ([`NavMesh::rings_within`]).
const PLAY_REACH: f64 = 40.0;
/// How far the anchors' box grows across, and up and down, world units.
const PLAY_BOX: f64 = 60.0;
const PLAY_HEIGHT: f64 = 30.0;

/// How far `p` is from a face (a convex polygon): across, to its outline or
/// nothing inside it, and up or down, to its range of heights.
fn face_distance(corners: &[[f64; 3]], p: [f64; 3]) -> f64 {
    let n = corners.len();
    let xy = |c: &[f64; 3]| (c[0], c[1]);
    let side = |k: usize| {
        let (a, b) = (xy(&corners[k]), xy(&corners[(k + 1) % n]));
        (b.0 - a.0) * (p[1] - a.1) - (b.1 - a.1) * (p[0] - a.0)
    };
    let inside = (0..n).all(|k| side(k) >= 0.0) || (0..n).all(|k| side(k) <= 0.0);
    let across = if inside {
        0.0
    } else {
        (0..n)
            .map(|k| segment_distance((p[0], p[1]), xy(&corners[k]), xy(&corners[(k + 1) % n])))
            .fold(f64::MAX, f64::min)
    };
    let (lo, hi) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(c[2]), hi.max(c[2])));
    let up = (lo - p[2]).max(p[2] - hi).max(0.0);
    (across * across + up * up).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mesh of faces given by their corners in order, each edge's opposite
    /// found by position, as a seam's repeated corners lie.
    fn mesh(vertices: &[[f32; 3]], faces: &[&[usize]]) -> NavMesh {
        let mut out = NavMesh { vertices: vertices.to_vec(), ..NavMesh::default() };
        let mut owner = Vec::new();
        for (f, corners) in faces.iter().enumerate() {
            out.faces.push(Face { first_edge: out.edges.len(), edge_count: corners.len() });
            for k in 0..corners.len() {
                out.edges.push(Edge { start: corners[k], end: corners[(k + 1) % corners.len()], ..Edge::default() });
                owner.push(f);
            }
        }
        let xy = |v: usize| (vertices[v][0], vertices[v][1]);
        for i in 0..out.edges.len() {
            let (start, end) = (out.edges[i].start, out.edges[i].end);
            if let Some(j) =
                (0..out.edges.len()).find(|&j| xy(out.edges[j].start) == xy(end) && xy(out.edges[j].end) == xy(start))
            {
                out.edges[i].opposite_edge = Some(j);
                out.edges[i].opposite_face = Some(owner[j]);
            }
        }
        out
    }

    /// Two squares side by side, the right one repeating the corners they
    /// share (4 and 5) a little higher.
    fn seam() -> NavMesh {
        let v = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.1],
            [1.0, 1.0, 0.1],
            [2.0, 0.0, 0.1],
            [2.0, 1.0, 0.1],
        ];
        mesh(&v, &[&[0, 1, 2, 3], &[4, 6, 7, 5]])
    }

    fn points(ring: &Ring) -> Vec<(f64, f64)> {
        ring.p.chunks_exact(2).map(|c| (c[0], c[1])).collect()
    }

    #[test]
    fn a_ring_chains_through_a_corner_repeated_at_a_seam() {
        let m = seam();
        let b = m.boundary(|_| true);
        assert_eq!((b.rings.len(), b.open), (1, 0), "{:?}", b.rings);
        assert_eq!(b.rings[0], vec![0, 4, 6, 7, 2, 3]);
        let rings = m.rings();
        assert_eq!(rings.len(), 1);
        assert!(!rings[0].hole);
        // Both heights, the repeated corners' too.
        assert_eq!(rings[0].z, [0.0, f64::from(1.0f32)]);
        // The corners along the seam fall on straight sides and go.
        assert_eq!(points(&rings[0]), [(0.0, 0.0), (0.0, 20.0), (-10.0, 20.0), (-10.0, 0.0)]);
        assert!((area(&points(&rings[0])) - 200.0).abs() < 1e-9);

        // Without the opposite edges the seam's corners are two, and every
        // run stops at one.
        let mut unlinked = m.clone();
        for e in &mut unlinked.edges {
            e.opposite_edge = None;
        }
        let b = unlinked.boundary(|_| true);
        assert_eq!((b.rings.len(), b.open), (1, 3), "{:?}", b.rings);
    }

    /// Two squares side by side, the right one's left corners `dy` engine
    /// units up from the left one's right corners, the edges between them
    /// linked as a mesh links them.
    fn offset(dy: f32) -> NavMesh {
        let v = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, dy, 0.0],
            [1.0, 1.0 + dy, 0.0],
            [2.0, dy, 0.0],
            [2.0, 1.0 + dy, 0.0],
        ];
        let mut m = mesh(&v, &[&[0, 1, 2, 3], &[4, 6, 7, 5]]);
        let (left, right) = (1, 7);
        m.edges[left].opposite_edge = Some(right);
        m.edges[left].opposite_face = Some(1);
        m.edges[right].opposite_edge = Some(left);
        m.edges[right].opposite_face = Some(0);
        m
    }

    #[test]
    fn corners_farther_apart_than_a_repeat_stay_two() {
        let b = offset(0.005).boundary(|_| true);
        assert_eq!((b.rings.len(), b.open), (1, 0), "{:?}", b.rings);
        // Edges that meet only in part.
        let b = offset(0.5).boundary(|_| true);
        assert!(b.open > 0, "{b:?}");
    }

    #[test]
    fn dropped_faces_leave_their_neighbours_a_boundary() {
        let v = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0]];
        let m = mesh(&v, &[&[0, 1, 4, 3], &[1, 2, 5, 4]]);
        let all = m.boundary(|_| true);
        assert_eq!((all.rings.len(), all.rings[0].len(), all.open), (1, 6, 0));
        assert_eq!(m.boundary(|f| f == 0).rings, vec![vec![0, 1, 4, 3]]);
        assert!(m.boundary(|_| false).rings.is_empty());
        assert_eq!((m.neighbours(0).collect::<Vec<_>>(), m.neighbours(1).collect::<Vec<_>>()), (vec![1], vec![0]));
        assert_eq!(m.rings_where(|f| f == 1).len(), 1);
    }

    #[test]
    fn a_floor_round_a_gap_is_a_ring_and_a_hole() {
        // Three by three squares, the middle one missing.
        let v: Vec<[f32; 3]> = (0..16).map(|i| [(i % 4) as f32, (i / 4) as f32, 0.0]).collect();
        let square = |x: usize, y: usize| [y * 4 + x, y * 4 + x + 1, (y + 1) * 4 + x + 1, (y + 1) * 4 + x];
        let squares: Vec<[usize; 4]> = (0..3)
            .flat_map(|y| (0..3).map(move |x| (x, y)))
            .filter(|&(x, y)| (x, y) != (1, 1))
            .map(|(x, y)| square(x, y))
            .collect();
        let faces: Vec<&[usize]> = squares.iter().map(|s| &s[..]).collect();
        let rings = mesh(&v, &faces).rings();
        assert_eq!(rings.len(), 2);
        let (outer, inner) = (&rings[0], &rings[1]);
        assert!(!outer.hole && inner.hole);
        assert!((area(&points(outer)) - 900.0).abs() < 1e-9);
        assert!((area(&points(inner)) + 100.0).abs() < 1e-9);
        // The outer ring's twelve corners simplify to the four that turn it.
        assert_eq!(outer.p.len(), 8);
    }

    #[test]
    fn a_clockwise_ring_is_a_hole_and_crumbs_are_dropped() {
        let hole = mesh(&[[0.0, 0.0, 0.0], [0.0, 2.0, 0.0], [2.0, 2.0, 0.0], [2.0, 0.0, 0.0]], &[&[0, 1, 2, 3]]);
        assert!(hole.rings()[0].hole);
        let crumb = mesh(&[[0.0, 0.0, 0.0], [0.1, 0.0, 0.0], [0.0, 0.1, 0.0]], &[&[0, 1, 2]]);
        assert!(crumb.rings().is_empty());
    }

    #[test]
    fn straight_runs_simplify_to_their_corners() {
        // A 4 by 1 strip whose long side carries corners 0.001 off the line
        // (0.01 world units, under the tolerance), and one 0.06 off (0.6 wu).
        let v = [
            [0.0, 0.0, 0.0],
            [1.0, 0.001, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, -0.06, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let strip = mesh(&v, &[&[0, 1, 2, 3, 4, 5, 6]]);
        let ring = &strip.rings()[0];
        let x = |p: &(f64, f64)| p.1 / 10.0;
        assert_eq!(points(ring).iter().map(x).collect::<Vec<_>>(), [0.0, 3.0, 4.0, 4.0, 0.0]);
    }

    #[test]
    fn a_frame_places_its_mesh() {
        let frame =
            Frame { columns: [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], translation: [10.0, 0.0, 0.0] };
        assert_eq!(frame.apply([1.0, 0.0, 2.0]), [10.0, 1.0, 2.0]);
        assert_eq!(Frame::IDENTITY.apply([1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
        assert_eq!(Frame::default(), Frame::IDENTITY);
        let mut m = mesh(&[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]], &[&[0, 1, 2, 3]]);
        let here = m.rings();
        m.frame = Frame { translation: [1.0, 0.0, 0.5], ..Frame::IDENTITY };
        let moved = m.rings();
        // One engine unit along x is ten world units along y.
        let shifted: Vec<f64> = here[0].p.chunks_exact(2).flat_map(|c| [c[0], c[1] + 10.0]).collect();
        assert_eq!(moved[0].p, shifted);
        assert_eq!(moved[0].z, [5.0, 5.0]);
    }

    #[test]
    fn a_ring_rounds_to_tenths_as_a_plan_prints_them() {
        let ring =
            Ring { z: [f64::from(-0.04f32), f64::from(12.25f32)], hole: true, p: vec![f64::from(-123.456f32), 0.07] };
        let r = ring.rounded();
        assert_eq!((r.z, r.hole), ([0.0, 12.3], true));
        assert!(r.z[0].is_sign_positive());
        assert_eq!(r.p, [-123.5, 0.1]);
        // The double nearest each tenth, as a writer that rounds single
        // precision prints it.
        for i in -200_000..200_000 {
            let v = i as f32 * 0.0137;
            let printed: f64 = format!("{:.1}", (v * 10.0).round() / 10.0).parse().unwrap();
            assert_eq!(tenth(f64::from(v)), printed + 0.0, "{v}");
        }
    }

    #[test]
    fn a_broken_mesh_gives_a_plan_without_panicking() {
        let v = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]];
        let mut m = mesh(&v, &[&[0, 1, 2, 3]]);
        m.edges.push(Edge { start: 9, end: 0, opposite_edge: Some(99), opposite_face: Some(0) });
        m.edges[0].opposite_edge = Some(4);
        m.edges[0].opposite_face = Some(7);
        m.faces.push(Face { first_edge: 3, edge_count: 5 });
        m.faces.push(Face { first_edge: 50, edge_count: usize::MAX });
        m.faces.push(Face { first_edge: usize::MAX, edge_count: 3 });
        let volume = Volume {
            kind: VolumeKind::Kill,
            centre: [100.0, 100.0, 0.0],
            forward: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            shape: VolumeShape::Cylinder { radius: 1.0 },
            top: 1.0,
            bottom: 1.0,
        };
        let _ = m.rings();
        let _ = m.rings_within(&[volume], &[[0.0, 0.0, 0.0]]);
        assert!(NavMesh::default().rings().is_empty());
        assert!(NavMesh::default().rings_within(&[], &[[0.0; 3]]).rings.is_empty());
    }

    fn volume(kind: VolumeKind, centre: [f64; 3], shape: VolumeShape) -> Volume {
        Volume { kind, centre, forward: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], shape, top: 5.0, bottom: 5.0 }
    }

    #[test]
    fn a_volume_holds_what_its_axes_and_extents_cover() {
        let slab = Volume {
            // A kill plane lying under a floor: forward points down, up along +y.
            forward: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            top: 100.0,
            bottom: 100.0,
            ..volume(VolumeKind::Kill, [0.0, 0.0, -20.0], VolumeShape::Box { length: 20.0, width: 200.0 })
        };
        assert!(slab.contains([50.0, -90.0, -15.0]));
        assert!(!slab.contains([50.0, 0.0, -5.0]), "above it");
        assert!(!slab.contains([110.0, 0.0, -20.0]), "past its width");
        let pit = volume(VolumeKind::Kill, [0.0, 0.0, 0.0], VolumeShape::Cylinder { radius: 10.0 });
        assert!(pit.contains([6.0, 7.0, 4.0]));
        assert!(!pit.contains([8.0, 7.0, 0.0]));
        assert!(!pit.contains([0.0, 0.0, 6.0]));
    }

    #[test]
    fn a_safe_volume_keeps_out_the_kill_volumes_of_its_kind() {
        let everywhere = VolumeShape::Box { length: 1000.0, width: 1000.0 };
        let near = VolumeShape::Box { length: 20.0, width: 20.0 };
        let kill = volume(VolumeKind::Kill, [0.0, 0.0, 0.0], everywhere);
        let safe = volume(VolumeKind::Safe, [0.0, 0.0, 0.0], near);
        let soft_safe = volume(VolumeKind::SoftSafe, [0.0, 0.0, 0.0], near);
        assert!(playable(&[], [0.0, 0.0, 0.0]));
        assert!(!playable(&[kill], [0.0, 0.0, 0.0]));
        assert!(playable(&[kill, safe], [0.0, 0.0, 0.0]));
        assert!(!playable(&[kill, safe], [50.0, 0.0, 0.0]));
        assert!(!playable(&[kill, soft_safe], [0.0, 0.0, 0.0]));
        let soft = Volume { kind: VolumeKind::SoftKill, ..kill };
        assert!(playable(&[soft, soft_safe], [0.0, 0.0, 0.0]));
        assert!(!playable(&[soft, safe], [0.0, 0.0, 0.0]));
    }

    #[test]
    fn pieces_away_from_the_game_and_faces_in_kill_volumes_go() {
        // Engine units: three squares side by side at height 1, and a fourth
        // apart from them, beside them but far below.
        let v = [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [2.0, 0.0, 1.0],
            [3.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [2.0, 1.0, 1.0],
            [3.0, 1.0, 1.0],
            [0.0, 3.0, -50.0],
            [1.0, 3.0, -50.0],
            [1.0, 4.0, -50.0],
            [0.0, 4.0, -50.0],
        ];
        let nav = mesh(&v, &[&[0, 1, 5, 4], &[1, 2, 6, 5], &[2, 3, 7, 6], &[8, 9, 10, 11]]);
        // A spawn on the first square (world: x = -10 ey, y = 10 ex).
        let spawn = [-5.0, 5.0, 10.0];
        let all = nav.rings_within(&[], &[]);
        assert_eq!((all.rings.len(), all.out_of_play, all.apart, all.open), (2, 0, 0, 0));
        assert_eq!(all.rings, nav.rings());
        let played = nav.rings_within(&[], &[spawn]);
        assert_eq!((played.rings.len(), played.apart), (1, 1));
        // A kill volume over the last square cuts it off.
        let pit = Volume {
            kind: VolumeKind::Kill,
            centre: [-5.0, 25.0, 10.0],
            forward: [1.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            shape: VolumeShape::Box { length: 9.0, width: 9.0 },
            top: 5.0,
            bottom: 5.0,
        };
        let cut = nav.rings_within(&[pit], &[spawn]);
        assert_eq!((cut.rings.len(), cut.out_of_play, cut.apart), (1, 1, 1));
        assert!((area(&points(&cut.rings[0])) - 200.0).abs() < 1e-6, "{:?}", cut.rings[0]);
    }

    #[test]
    fn a_piece_apart_but_near_the_game_stays() {
        // Two squares 2 engine units apart, the spawn on the first: the
        // second is a jump away.
        let v = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [3.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 1.0, 0.0],
            [3.0, 1.0, 0.0],
        ];
        let nav = mesh(&v, &[&[0, 1, 2, 3], &[4, 5, 6, 7]]);
        let within = nav.rings_within(&[], &[[-5.0, 5.0, 0.0]]);
        assert_eq!((within.rings.len(), within.apart), (2, 0));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_ring_serializes_as_a_plan_holds_it() {
        let ring = Ring { z: [1.5, 2.0], hole: false, p: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0] };
        let json = serde_json::to_string(&ring).unwrap();
        assert_eq!(json, r#"{"z":[1.5,2.0],"hole":false,"p":[0.0,1.0,2.0,3.0,4.0,5.0]}"#);
        assert_eq!(serde_json::from_str::<Ring>(&json).unwrap(), ring);
        let m = seam();
        let back: NavMesh = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back, m);
    }
}
