//! A type-0 (tick) payload read in order, by its grammar:
//!
//! ```text
//! payload    := [flag:1] EventView EntityView ControlView [padding]
//! EventView  := { [1][type:7] 3 handles  body }*  [0]          (events.rs)
//! EntityView := { op entry }*  op 0
//!   op       := `1` for 3, else `0` and 2 bits
//!   3 update := [index:13][gen:2][baseline:1](+7)[form:1]
//!               form 0: [count:3] count 6-bit components; form 1: 64-bit mask
//!               then each listed component's data (components.rs)
//!   1 create := [index:13][gen:2][type:6] creation data
//!               [has components:1] (a list and its data)
//!   2 delete := [index:13][gen:2][32]
//! ControlView: per-client entries (not read)
//! ```
//!
//! An entity's type is a block of the chunk-0 registry (`registry.rs`):
//! 35 biped, 36 item, 37 equipment, 38/39 rigid bodies, 40 vehicle,
//! 41 projectile, 42 weapon, 43 device; below 35 non-object state (4 the
//! high-frequency entity, 5 players, and so on). Updates carry no type: a
//! [`Walker`] learns it from the creates it sees, and falls back to the
//! index ranges for entities created before the film's first tick packet
//! (512-767 bipeds, 768-1023 vehicles, 123 the high-frequency entity whose
//! update is the tick message).
//!
//! What can't be read yet ends the walk ([`Stop`]): event bodies and create
//! tails whose layout isn't known, components outside `components.rs`,
//! non-object entities. Every type-0 payload carries the tick message (an
//! update of entity 123), so a walk stopped before it resumes there.

use crate::components::{self, Fields, Params, Table};
use crate::container::Film;
use crate::creation::{self, Form};
use crate::events::{self, Bits, Event, Handle, INDEX_BITS};
use crate::registry;
use std::collections::HashMap;

/// Entity types (registry blocks) the walker names.
pub const HIGH_FREQUENCY: u32 = 4;
pub const MANAGED_PLAYER: u32 = 9;
pub const BIPED: u32 = 35;
pub const VEHICLE: u32 = 40;
/// First and last registry block of an object type.
const FIRST_OBJECT: u32 = 35;
const LAST_OBJECT: u32 = 43;

/// The high-frequency entity: its update (component 0, the 8-bit tick
/// counter) is the tick message.
pub const TICK_ENTITY: u32 = 123;
/// The tick message: op 3, entity 123, generation 1, no baseline, a
/// one-entry list `[0]`; its 8-bit counter follows.
const TICK_MESSAGE: u32 = 0b1_0000001111011_01_0_0_001_000000;
const TICK_MESSAGE_BITS: u32 = 27;

/// Why a walk stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Stop {
    /// An event type whose handle table or body isn't known.
    EventBody(u32),
    /// More events than any real view holds: a misread.
    EventCount,
    /// A create of this entity type: its creation data isn't fully known.
    CreateTail(u32),
    /// An update of an entity whose type isn't known: a non-object entity
    /// no create named, or an object no object type reads.
    UnknownEntity,
    /// An update of an object no create named that several object types
    /// read to different ends.
    AmbiguousEntity,
    /// An update of a non-object entity type whose component widths aren't
    /// known.
    NonObject(u32),
    /// An object component (entity type, component) whose block, or this
    /// branch of it, isn't known.
    Component(u32, u32),
    /// A component list that isn't strictly increasing, or a generation 0.
    BadHeader,
    /// The walk ran past the payload.
    Overrun,
    /// An entry ran past the tick message, or the view ended before it.
    PastTick,
}

#[derive(Clone, Debug)]
pub enum OpKind {
    Update {
        list: Vec<u32>,
        fields: Fields,
    },
    Create {
        /// The object's tag id (objects only).
        tag: Option<u32>,
    },
    Delete,
}

/// One entity-view entry.
#[derive(Clone, Debug)]
pub struct EntityOp {
    /// Absolute bit of the entry's index field.
    pub at: i64,
    pub index: u32,
    pub generation: u32,
    /// The entity type (registry block), when known.
    pub entity_type: Option<u32>,
    pub kind: OpKind,
    /// A create's initial components, when it lists any and they parse:
    /// blocks as an update's (`zones.rs` reads navpoints' and zones').
    pub init: Option<Fields>,
    /// A projectile create's player index and shooter's unit, when it names
    /// them.
    pub shooter: (Option<u32>, Option<Handle>),
    /// A managed player's create: the participant it stands for.
    pub participant: Option<u32>,
}

/// One payload as far as it could be read.
#[derive(Clone, Debug, Default)]
pub struct Payload {
    pub events: Vec<Event>,
    /// Where the event view stopped, if it didn't reach its end.
    pub event_stop: Option<Stop>,
    /// The entity view's first bit, when the event view was walked to it.
    pub entity_start: Option<i64>,
    pub ops: Vec<EntityOp>,
    /// Stops the walk resumed past at the tick message: (stop, from bit,
    /// tick message bit).
    pub skipped: Vec<(Stop, i64, i64)>,
    /// Where the entity walk stopped for good; None when it read the
    /// terminating op 0.
    pub stop: Option<Stop>,
    /// The bit after the last entry read (after op 0 when `stop` is None).
    pub end: i64,
}

impl Payload {}

/// The bit of the first tick message in `c[start..end]` (bits), if any.
pub fn find_tick_message(c: &[u8], start: i64, end: i64) -> Option<i64> {
    (start..end - TICK_MESSAGE_BITS as i64 - 8).find(|&q| crate::bits::bits_at(c, q, TICK_MESSAGE_BITS) == TICK_MESSAGE)
}

/// The managed engine participant: one per player, its blocks kept.
const MANAGED_PARTICIPANT: u32 = 47;
/// Devices (doors, lifts, machines): their blocks kept too.
const DEVICE: u32 = 43;
/// Crates, Strongholds' zones and the hills among them: their blocks kept
/// for the zone's multiplayer properties and orientation (`zones.rs`).
pub const CRATE: u32 = 38;

/// Types whose creates carry no `[set:1]` component list. The create reader
/// reads the list only when the definition's initial component mask
/// (`+0x30`) is non-empty; these definitions' mask is empty. Every other
/// type's mask has bits set (bits 0 to the row count, and others).
const NO_INITIAL_COMPONENTS: [u32; 10] = [7, 13, 14, 20, 23, 28, 29, 46, 47, 49];

/// Entity type `supply_lines_items` (64 rows of supply-lines-item-unlocked).
const SUPPLY_LINES_ITEMS: u32 = 27;

/// Bits of a non-object component, where known.
fn non_object_width(entity_type: u32, component: u32) -> Option<u32> {
    match (entity_type, component) {
        // The high-frequency entity's tick counter.
        (HIGH_FREQUENCY, 0) => Some(8),
        // Supply-lines items: each row a 32-bit set of unlocked flags, read a
        // bit at a time.
        (SUPPLY_LINES_ITEMS, _) => Some(32),
        _ => None,
    }
}

/// Reads payloads in film order, keeping each entity's type from the
/// creates it has seen.
#[derive(Debug)]
pub struct Walker {
    /// Component tables by entity type (registry block).
    tables: Vec<Table>,
    /// The registry's row versions, by entity type.
    versions: Vec<Vec<u32>>,
    pub params: Params,
    types: HashMap<u32, u32>,
    /// The next chunk's snapshot: index -> (type, generation). Breaks a tie
    /// between the types an untyped entity's update fits.
    ahead: HashMap<u32, (u32, u32)>,
    /// The entity and the types that fit, at the last `AmbiguousEntity`.
    ambiguous: std::cell::RefCell<Option<(u32, Vec<(u32, i64)>)>>,
}

impl Walker {
    pub fn new(chunk0: &[u8], params: Params) -> Walker {
        let tables =
            registry::blocks(chunk0).iter().enumerate().map(|(k, rows)| Table::from_rows(rows, k as u32)).collect();
        Walker {
            tables,
            versions: registry::versions(chunk0),
            params,
            types: HashMap::new(),
            ahead: HashMap::new(),
            ambiguous: Default::default(),
        }
    }

    /// Take the entity types of a chunk's snapshot (`snapshot.rs`): every
    /// entity alive at the chunk's start, including those no create in the
    /// tick packets named. Replaces what the creates taught.
    pub fn seed(&mut self, entities: &[crate::snapshot::Entity]) {
        self.types.clear();
        self.types.extend(entities.iter().map(|e| (e.index, e.entity_type)));
    }

    /// The entities the next snapshot lists, for `infer_type`'s ties.
    pub fn seed_ahead(&mut self, entities: &[crate::snapshot::Entity]) {
        self.ahead.clear();
        self.ahead.extend(entities.iter().map(|e| (e.index, (e.entity_type, e.generation))));
    }

    /// The type of entity `index`: from its last create, else its range.
    pub fn entity_type(&self, index: u32) -> Option<u32> {
        if let Some(&t) = self.types.get(&index) {
            return Some(t);
        }
        match index {
            TICK_ENTITY => Some(HIGH_FREQUENCY),
            512..=767 => Some(BIPED),
            768..=1023 => Some(VEHICLE),
            _ => None,
        }
    }

    /// Walk the payload `c[start..end]` (bytes).
    pub fn walk(&mut self, c: &[u8], start: usize, end: usize) -> Payload {
        let (start, end) = (8 * start as i64, 8 * end as i64);
        let mut p = Payload::default();
        let mut r = Bits { c, at: start + 1 };
        match events::walk(&mut r, (&self.params).into(), &mut p.events) {
            Ok(x) => p.entity_start = Some(x),
            Err(s) => p.event_stop = Some(s),
        }
        let tick = find_tick_message(c, start, end);
        let mut at = match (p.entity_start, tick) {
            (Some(x), _) => x,
            (None, Some(t)) => {
                p.skipped.push((p.event_stop.unwrap_or(Stop::Overrun), start + 1, t));
                t
            }
            (None, None) => {
                p.stop = p.event_stop;
                p.end = start + 1;
                return p;
            }
        };
        // The tick message is an entry of its own: a walk from before it
        // must land on it, so an entry that runs past it (or an op 0 before
        // it) is a misread, and the walk resumes there.
        loop {
            let limit = tick.filter(|&t| t > at);
            let step = match self.entry(c, at, end, limit.unwrap_or(end), &mut p.ops) {
                Ok(Some(next)) if limit.is_some_and(|t| next > t) => Err(Stop::PastTick),
                Ok(None) if limit.is_some() => Err(Stop::PastTick),
                other => other,
            };
            match step {
                Ok(Some(next)) => at = next,
                Ok(None) => {
                    p.end = at + 3;
                    return p;
                }
                Err(s) => match limit {
                    Some(t) => {
                        p.skipped.push((s, at, t));
                        at = t;
                    }
                    None => {
                        p.stop = Some(s);
                        p.end = at;
                        return p;
                    }
                },
            }
        }
    }

    /// One entity-view entry at `at`: Ok(Some(next entry)), Ok(None) at the
    /// terminating op 0, or the stop.
    /// `limit` bounds where a plausible entry may end (the tick message
    /// ahead, else the payload's end).
    fn entry(&mut self, c: &[u8], at: i64, end: i64, limit: i64, ops: &mut Vec<EntityOp>) -> Result<Option<i64>, Stop> {
        if at >= end {
            return Err(Stop::Overrun);
        }
        let mut r = Bits { c, at };
        let op = if r.flag() { 3 } else { r.bits(2) };
        let index_at = r.at;
        let index = r.bits(INDEX_BITS);
        let generation = r.bits(2);
        let shooter = std::cell::Cell::new((None, None));
        let participant = std::cell::Cell::new(None);
        let mut push = |entity_type, kind| {
            ops.push(EntityOp {
                at: index_at,
                index,
                generation,
                entity_type,
                kind,
                init: None,
                shooter: shooter.get(),
                participant: participant.get(),
            })
        };
        match op {
            0 => return Ok(None),
            2 => {
                r.bits(32);
                push(self.entity_type(index), OpKind::Delete);
            }
            1 => {
                let t = r.bits(6);
                if !generation_fits(index, generation) || !create_fits(index, t) {
                    return Err(Stop::BadHeader);
                }
                self.types.insert(index, t);
                if !(FIRST_OBJECT..=LAST_OBJECT).contains(&t) {
                    if t == MANAGED_PLAYER {
                        participant.set(creation::managed_player_participant(c, r.at));
                    }
                    push(Some(t), OpKind::Create { tag: None });
                    let Some(end) = creation::manager_end(c, r.at, t) else {
                        return Err(Stop::CreateTail(t));
                    };
                    r.at = end;
                } else {
                    let Some(made) = creation::read(c, r.at, t, Form::Tick((&self.params).into())) else {
                        return Err(Stop::CreateTail(t));
                    };
                    shooter.set(made.shooter);
                    push(Some(t), OpKind::Create { tag: Some(made.tag) });
                    r.at = made.end;
                }
                if NO_INITIAL_COMPONENTS.contains(&t) {
                    return Ok(Some(r.at));
                }
                if r.flag() {
                    let list = listed_components(&mut r);
                    if !list.is_empty() {
                        let (fields, next) = self.data(c, r.at, t, &list);
                        if next.is_ok() {
                            if let Some(op) = ops.last_mut() {
                                op.init = Some(fields);
                            }
                        }
                        return next.map(Some);
                    }
                }
            }
            _ => {
                if !generation_fits(index, generation) {
                    return Err(Stop::BadHeader);
                }
                let list = component_list(&mut r)?;
                let t = match self.entity_type(index) {
                    Some(t) => t,
                    None => self.infer_type(c, r.at, index, generation, &list, limit)?,
                };
                let (fields, next) = self.data(c, r.at, t, &list);
                push(Some(t), OpKind::Update { list, fields });
                return next.map(Some);
            }
        }
        if r.at > end {
            return Err(Stop::Overrun);
        }
        Ok(Some(r.at))
    }

    /// The type of an object entity no create has named (created before
    /// the film's first tick packet): the one object type whose table reads
    /// the update to a plausible next entry ([`Walker::plausible`]). Types
    /// that read it to the same end are equivalent for the walk (most
    /// lists name only the components every object type shares); the first
    /// is taken. Not kept for the entity's later updates, which it would
    /// mistype.
    fn infer_type(&self, c: &[u8], at: i64, index: u32, generation: u32, list: &[u32], end: i64) -> Result<u32, Stop> {
        if index < 1024 {
            return Err(Stop::UnknownEntity);
        }
        let mut fits: Vec<(u32, i64)> = Vec::new();
        for t in (FIRST_OBJECT..=LAST_OBJECT).filter(|&t| create_fits(index, t)) {
            if let (_, Ok(x)) = self.data(c, at, t, list) {
                if self.plausible(c, x, end, 2) {
                    fits.push((t, x));
                }
            }
        }
        match fits.as_slice() {
            [] => Err(Stop::UnknownEntity),
            [(t, _)] => Ok(*t),
            [(t, x), rest @ ..] if rest.iter().all(|f| f.1 == *x) => Ok(*t),
            // The next snapshot lists the same entity: its type, if it fits.
            _ => match self.ahead.get(&index) {
                Some(&(t, g)) if g == generation && fits.iter().any(|f| f.0 == t) => Ok(t),
                _ => {
                    *self.ambiguous.borrow_mut() = Some((index, fits.clone()));
                    Err(Stop::AmbiguousEntity)
                }
            },
        }
    }

    /// Whether an entity-view entry plausibly starts at `x`, before `end`:
    /// the tick message (at `end` too); the terminating op 0; a create that
    /// fits the index space; a delete; or an update header with a valid list
    /// whose data (when its type is known) ends on another plausible entry,
    /// `depth` entries deep.
    fn plausible(&self, c: &[u8], x: i64, end: i64, depth: u32) -> bool {
        if x <= end && crate::bits::bits_at(c, x, TICK_MESSAGE_BITS) == TICK_MESSAGE {
            return true;
        }
        if x >= end {
            return false;
        }
        let mut r = Bits { c, at: x };
        let op = if r.flag() { 3 } else { r.bits(2) };
        let index = r.bits(INDEX_BITS);
        // The generation: any of the four can be real (generation_fits).
        r.bits(2);
        match op {
            0 => true,
            1 => create_fits(index, r.bits(6)),
            2 => true,
            _ => {
                // A misread that lands on the mask form reads 64 chance bits
                // as an always valid list, so a mask is no evidence here.
                let Ok(list) = component_list(&mut r) else { return false };
                if list.len() > 7 {
                    return false;
                }
                match self.entity_type(index) {
                    Some(t) if depth > 0 => match self.data(c, r.at, t, &list).1 {
                        Ok(next) => self.plausible(c, next, end, depth - 1),
                        Err(_) => true,
                    },
                    _ => true,
                }
            }
        }
    }

    /// The data of an update of an entity of type `t` listing `list` at
    /// `at`: the fields read, and the bit after the data or the stop.
    fn data(&self, c: &[u8], at: i64, t: u32, list: &[u32]) -> (Fields, Result<i64, Stop>) {
        if t < FIRST_OBJECT {
            if let Some(widths) = list.iter().map(|&k| non_object_width(t, k)).collect::<Option<Vec<u32>>>() {
                // Fixed widths: each block starts where the last ended.
                let mut blocks = Vec::with_capacity(list.len());
                let mut b = at;
                for (&k, &w) in list.iter().zip(&widths) {
                    blocks.push((k, b));
                    b += w as i64;
                }
                return (Fields { blocks, ..Fields::default() }, Ok(b));
            }
            let (Some(table), Some(versions)) = (self.tables.get(t as usize), self.versions.get(t as usize)) else {
                return (Fields::default(), Err(Stop::NonObject(t)));
            };
            let parsed = components::parse_versions(c, at, list, table, versions, &self.params);
            let next = parsed.end.ok_or(Stop::Component(t, parsed.failed.unwrap_or(list[0])));
            return (Fields { blocks: parsed.starts, ..Fields::default() }, next);
        }
        let Some(table) = self.tables.get(t as usize) else {
            return (Fields::default(), Err(Stop::Component(t, list[0])));
        };
        let parsed = components::parse(c, at, list, table, &self.params);
        let next = parsed.end.ok_or(Stop::Component(t, parsed.failed.unwrap_or(list[0])));
        let mut fields = parsed.fields;
        if t == MANAGED_PARTICIPANT || t == DEVICE || t == CRATE {
            fields.blocks = parsed.starts;
        }
        (fields, next)
    }
}

/// A generation is 2 bits and wraps 1, 2, 3, 0, so any 0 can be real: the
/// fast-recycled pool above the vehicles reaches it soonest, and Warzone's
/// managed non-objects, below index 512, do too.
fn generation_fits(_index: u32, _generation: u32) -> bool {
    true
}

/// Whether a create of type `t` at `index` fits the index space: objects
/// at 512 and up (bipeds 512-767, vehicles 768-1023, the rest above),
/// non-object entities below 512. A create that doesn't is a misread, and
/// would mistype the index's later updates.
fn create_fits(index: u32, t: u32) -> bool {
    match index {
        // The high-frequency entity, whose update is the tick message: a
        // create naming another type there is a misread, and kept, it would
        // misread every later tick message of the chunk.
        TICK_ENTITY => t == HIGH_FREQUENCY,
        // Non-objects: below the object range, and past it (asset 44,
        // game_matchflow 45, StateBroker 46, ...).
        0..=511 => t < FIRST_OBJECT || (44..=49).contains(&t),
        512..=767 => t == BIPED,
        768..=1023 => t == VEHICLE,
        // The pool above also holds the managed entities (radial menus, 49,
        // on Warzone).
        _ => {
            matches!(t, 9..=14 | 29 | 47 | 49)
                || (FIRST_OBJECT..=LAST_OBJECT).contains(&t) && t != BIPED && t != VEHICLE
        }
    }
}

/// A list of components: `[form:1]`; 0: a 3-bit count and that
/// many 6-bit indices; 1: a 64-bit mask, bit i for component i.
fn listed_components(r: &mut Bits<'_>) -> Vec<u32> {
    if r.flag() {
        let mask = ((r.bits(32) as u64) << 32) | r.bits(32) as u64;
        (0..64).filter(|i| mask >> i & 1 == 1).collect()
    } else {
        let n = r.bits(3);
        (0..n).map(|_| r.bits(6)).collect()
    }
}

/// An update's component list after its index and generation: `[baseline:1]`
/// (7 more bits when set), then the list, strictly increasing and not empty.
fn component_list(r: &mut Bits<'_>) -> Result<Vec<u32>, Stop> {
    if r.flag() {
        r.bits(7);
    }
    let list = listed_components(r);
    if list.is_empty() || list.windows(2).any(|w| w[1] <= w[0]) {
        return Err(Stop::BadHeader);
    }
    Ok(list)
}

/// A reader of walked payloads: f(frame index, chunk, payload).
pub type Reader<'a> = dyn FnMut(usize, &[u8], &Payload) + 'a;

/// One walk of every type-0 payload of `film`, each payload handed to every
/// reader in turn. A walk depends only on the film and `params`, so the
/// readers that need one share it.
pub fn walk_shared(film: &Film, params: Params, readers: &mut [&mut Reader<'_>]) {
    let mut walker = Walker::new(&film.chunks[0], params);
    for_each_payload_in(film, &mut walker, |i, c, p| {
        for r in readers.iter_mut() {
            r(i, c, p);
        }
    });
}

/// Every type-0 payload of `film` walked in order: f(frame index, payload).
/// Packet framing only (a frame is a packet); nothing under marker framing.
#[cfg_attr(not(any(feature = "raw", test)), allow(dead_code))]
pub fn for_each_payload(film: &Film, walker: &mut Walker, mut f: impl FnMut(usize, &Payload)) {
    for_each_payload_in(film, walker, |i, _, p| f(i, p));
}

/// As `for_each_payload`, with the chunk the payload is in, whose bits its
/// blocks index: f(frame index, chunk, payload).
pub fn for_each_payload_in(film: &Film, walker: &mut Walker, mut f: impl FnMut(usize, &[u8], &Payload)) {
    if film.framing != crate::container::Framing::Packets {
        return;
    }
    // Each chunk's snapshot types every entity alive at its start.
    let ctx = crate::snapshot::Context::new(&film.chunks[0], walker.params, walker.params.box_bits);
    let mut seeds: HashMap<*const u8, Vec<crate::snapshot::Entity>> = HashMap::new();
    for ci in 1..film.chunks.len() {
        let Some(ps) = film.packets(ci) else { continue };
        let c = &film.chunks[ci];
        if let Some(pk) = ps.iter().find(|p| p.kind == crate::snapshot::ENTITY_TABLE) {
            seeds.insert(c.as_ptr(), crate::snapshot::entities(c, pk.start, pk.end, &ctx));
        }
    }
    // Each chunk's successor, for its snapshot.
    let next: HashMap<*const u8, *const u8> = film.chunks.windows(2).map(|w| (w[0].as_ptr(), w[1].as_ptr())).collect();
    let mut chunk = std::ptr::null();
    for fr in film.frames() {
        if fr.chunk.as_ptr() != chunk {
            chunk = fr.chunk.as_ptr();
            if let Some(e) = seeds.get(&chunk) {
                walker.seed(e);
            }
            match next.get(&chunk).and_then(|n| seeds.get(n)) {
                Some(e) => walker.seed_ahead(e),
                None => walker.seed_ahead(&[]),
            }
        }
        if fr.end - fr.start < 2 {
            continue;
        }
        let mut p = walker.walk(fr.chunk, fr.start, fr.end);
        if p.stop == Some(Stop::AmbiguousEntity) {
            if let Some(q) = resolve(walker, fr.chunk, fr.start, fr.end, 0) {
                p = q;
            }
        }
        f(fr.index, fr.chunk, &p);
    }
}

/// Ambiguous entities resolved in one packet, one inside the other.
const RESOLVE_DEPTH: u32 = 32;

/// A packet stopped at an ambiguous entity, read again under each type it
/// fits. When every candidate that reads to the end of the entity view (the
/// control view ending on the packet's last byte) puts the entries at the
/// same bits: that walk, the entity's own entries untyped. Else None.
fn resolve(walker: &mut Walker, c: &[u8], start: usize, end: usize, depth: u32) -> Option<Payload> {
    let (index, mut fits) = walker.ambiguous.borrow_mut().take()?;
    // Candidates whose update ends at the same bit read the rest of the
    // packet alike: one walk for each end.
    fits.sort_by_key(|f| f.1);
    fits.dedup_by_key(|f| f.1);
    let candidates: Vec<u32> = fits.iter().map(|f| f.0).collect();
    // Each walk types what it creates, so each starts from the types as they
    // were, and only the walk taken keeps its creates: a wrong candidate can
    // read a create that isn't there.
    let saved = walker.types.clone();
    let mut good = Vec::new();
    for t in candidates {
        walker.types = saved.clone();
        walker.types.insert(index, t);
        let mut q = walker.walk(c, start, end);
        // A second ambiguous entity further on: the same, one level down.
        if q.stop == Some(Stop::AmbiguousEntity) && depth < RESOLVE_DEPTH {
            if let Some(r) = resolve(walker, c, start, end, depth + 1) {
                q = r;
            }
        }
        let (_, e) = components::control_view(c, q.end, &walker.params);
        if q.stop.is_none() && e.is_some_and(|e| (0..8).contains(&(8 * end as i64 - e))) {
            let mut types = std::mem::take(&mut walker.types);
            types.remove(&index);
            good.push((t, q, types));
        }
    }
    walker.types = saved;
    // The rest of the packet is read alike under every candidate only when
    // each puts every entry at the same bit. The type isn't kept: a lone
    // candidate that reads to the end can still be the wrong one, and kept,
    // it misreads every later packet of the chunk.
    let same = good
        .iter()
        .all(|g| g.1.end == good[0].1.end && g.1.ops.iter().map(|o| o.at).eq(good[0].1.ops.iter().map(|o| o.at)));
    if good.is_empty() || !same {
        return None;
    }
    let (_, mut q, types) = good.swap_remove(0);
    walker.types = types;
    for op in q.ops.iter_mut().filter(|o| o.index == index) {
        op.entity_type = None;
    }
    Some(q)
}

/// The event views of every type-0 payload of `film`: f(frame index, the
/// events read), game events the game sent again left out
/// ([`events::Resends`]). Packet framing only.
pub fn for_each_event_view(film: &Film, widths: impl Into<events::Layout>, mut f: impl FnMut(usize, &[Event])) {
    let widths = widths.into();
    if film.framing != crate::container::Framing::Packets {
        return;
    }
    let mut out = Vec::new();
    let mut resends = events::Resends::default();
    for fr in film.frames() {
        if fr.end - fr.start < 2 {
            continue;
        }
        out.clear();
        let mut r = Bits { c: fr.chunk, at: 8 * fr.start as i64 + 1 };
        let _ = events::walk(&mut r, widths, &mut out);
        out.retain(|e| if let events::Body::Game(g) = &e.body { resends.fresh(fr.index, g) } else { true });
        f(fr.index, &out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MSB-first bytes from a string of '0'/'1' (other characters ignored).
    fn bytes(bits: &str) -> Vec<u8> {
        let b: Vec<u8> = bits.bytes().filter(|c| *c == b'0' || *c == b'1').map(|c| c - b'0').collect();
        b.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |v, (i, &x)| v | x << (7 - i))).collect()
    }

    fn walker() -> Walker {
        Walker {
            tables: Vec::new(),
            versions: Vec::new(),
            params: Params::new([15, 15, 15]),
            types: HashMap::new(),
            ahead: HashMap::new(),
            ambiguous: Default::default(),
        }
    }

    #[test]
    fn a_tick_only_payload() {
        // flag, no events, the tick message with counter 0x2a, op 0.
        let c = bytes("0 0 1_0000001111011_01_0_0_001_000000 00101010 000 00");
        let p = walker().walk(&c, 0, c.len());
        assert_eq!(p.stop, None);
        assert!(p.skipped.is_empty());
        assert_eq!(p.entity_start, Some(2));
        assert_eq!(p.ops.len(), 1);
        assert_eq!((p.ops[0].index, p.ops[0].generation, p.ops[0].entity_type), (TICK_ENTITY, 1, Some(HIGH_FREQUENCY)));
        assert_eq!(p.end, 2 + 27 + 8 + 3);
    }

    #[test]
    fn a_create_is_read_to_its_end() {
        // A weapon (type 42) at index 1100, by the game's reader: two version
        // flags, 9 bits, the tag, default variant, no 18-bit or 13-bit value,
        // 2 + 5 bits, no attachments, no owner, no name block; 12 + 7 bits, no
        // tag list, no player. No components. Then the tick message.
        let data = format!("0 0 {:09b} {:032b} 1 0 0 00 00000 000 0 0 {:012b} {:07b} 0 0 1", 0, 0x2b1824d5u32, 0, 0);
        let create = format!("001 {:013b} 01 101010 {data} 0", 1100);
        let c = bytes(&format!("0 0 {create} 1_0000001111011_01_0_0_001_000000 00000001 000"));
        let mut w = walker();
        let p = w.walk(&c, 0, c.len());
        assert_eq!(p.stop, None);
        assert!(p.skipped.is_empty());
        match &p.ops[0].kind {
            OpKind::Create { tag } => assert_eq!(*tag, Some(0x2b1824d5)),
            k => panic!("{k:?}"),
        }
        assert_eq!(w.entity_type(1100), Some(42));
    }

    #[test]
    fn a_non_object_create_resumes_at_the_tick_message() {
        // A create of type 7 at index 52: its reader isn't written.
        let create = format!("001 {:013b} 01 000111 0 1 0 1", 52);
        let c = bytes(&format!("0 0 {create} 1_0000001111011_01_0_0_001_000000 00000001 000"));
        let mut w = walker();
        let p = w.walk(&c, 0, c.len());
        assert_eq!(p.stop, None);
        assert_eq!(p.skipped.len(), 1);
        assert_eq!(p.skipped[0].0, Stop::CreateTail(7));
        assert_eq!(w.entity_type(52), Some(7));
    }

    #[test]
    fn a_misread_create_is_rejected() {
        assert!(create_fits(600, BIPED));
        assert!(!create_fits(600, 42));
        assert!(!create_fits(52, 42));
        assert!(create_fits(52, 5));
        // Radial menus live in the pool; the tick entity is never retyped.
        assert!(create_fits(2365, 49));
        assert!(!create_fits(TICK_ENTITY, 2));
        assert!(create_fits(TICK_ENTITY, HIGH_FREQUENCY));
    }

    #[test]
    fn a_generation_0_update_is_read() {
        // Entity 52, a supply-lines items entity (32 bits a row), at
        // generation 0, then the tick message.
        let update = format!("1 {:013b} 00 0 0 001 000000 {:032b}", 52, 7);
        let c = bytes(&format!("0 0 {update} 1_0000001111011_01_0_0_001_000000 00000001 000"));
        let mut w = walker();
        w.types.insert(52, SUPPLY_LINES_ITEMS);
        let p = w.walk(&c, 0, c.len());
        assert_eq!(p.stop, None);
        assert!(p.skipped.is_empty());
        assert_eq!((p.ops[0].index, p.ops[0].generation), (52, 0));
    }

    /// On a local film nearly every event view walks to an entity view,
    /// and nearly every payload's walk reads the tick message.
    #[test]
    fn reference_film_payloads_walk() {
        let Some(chunks) = crate::testing::chunks("f9129808") else { return };
        let film = Film::new(chunks, crate::container::Framing::Packets);
        let level = crate::header::read(&film.chunks[0]).level.and_then(crate::levels::level).unwrap();
        let widths = level.widths;
        let mut w = Walker::new(&film.chunks[0], Params::new(widths));
        let (mut views, mut walked, mut ticks, mut fire) = (0, 0, 0, 0);
        for_each_payload(&film, &mut w, |_, p| {
            if !p.events.is_empty() || p.event_stop.is_some() {
                views += 1;
                walked += p.event_stop.is_none() as usize;
            }
            ticks += p.ops.iter().any(|o| o.index == TICK_ENTITY) as usize;
            fire += p.events.iter().filter(|e| e.kind == events::WEAPON_FIRE).count();
        });
        assert!(walked * 1000 >= views * 995, "{walked} of {views}");
        let frames = film.frames().count();
        assert!(ticks * 100 >= frames * 99, "{ticks} of {frames}");
        assert_eq!(fire, 2854);
    }

    /// A grapple whose target is an object reads on to the payload's end:
    /// the object's handle is 13 bits and the generation.
    #[test]
    fn grapples_at_objects_read_through() {
        let mut grapples = 0;
        for prefix in ["c3790060", "de65111b", "f0eca791"] {
            let Some(chunks) = crate::testing::chunks(prefix) else { continue };
            let film = Film::new(chunks, crate::container::Framing::Packets);
            let level = crate::header::read(&film.chunks[0]).level.and_then(crate::levels::level).unwrap();
            let mut params = Params::new(level.widths);
            params.set_level(Some(level));
            let mut w = Walker::new(&film.chunks[0], params);
            for_each_payload(&film, &mut w, |frame, p| {
                let grapple = |o: &EntityOp| match &o.kind {
                    OpKind::Update { fields, .. } => fields.grapple_object,
                    _ => false,
                };
                if p.ops.iter().any(grapple) {
                    grapples += 1;
                    assert_eq!(p.stop, None, "{prefix} frame {frame}");
                }
            });
        }
        assert!(crate::testing::film_dirs().is_empty() || grapples > 0);
    }

    #[test]
    fn a_shared_walk_hands_each_reader_the_same_payloads() {
        let Some(chunks) = crate::testing::chunks("f9129808") else { return };
        let film = Film::new(chunks, crate::container::Framing::Packets);
        let widths = crate::header::read(&film.chunks[0]).level.and_then(crate::levels::level).unwrap().widths;
        let params = Params::new(widths);
        let summary = |i: usize, p: &Payload| (i, p.ops.len(), p.events.len(), p.end, p.stop);
        let mut alone = Vec::new();
        for_each_payload(&film, &mut Walker::new(&film.chunks[0], params), |i, p| alone.push(summary(i, p)));
        let (mut a, mut b) = (Vec::new(), Vec::new());
        walk_shared(&film, params, &mut [&mut |i, _, p| a.push(summary(i, p)), &mut |i, _, p| b.push(summary(i, p))]);
        assert!(!alone.is_empty());
        assert_eq!(a, alone);
        assert_eq!(b, alone);
    }
}
