//! The film's raw layers, for reading the bytes the model is built from.
//!
//! A replay chunk is a sequence of packets, each a 16-byte head and its
//! payload. The head, little-endian: the kind (16 bits), two bytes the
//! library doesn't read, the payload's size (32 bits) and a 64-bit
//! timestamp in microseconds. Kind 0 is a tick packet, one frame of the
//! film; its payload is bit-packed (MSB first) and holds two views:
//!
//! - the **event view**: one-off events (weapon fire, kills, pickups, voice
//!   lines), each a 7-bit type, up to three object handles and a body;
//! - the **entity view**: creates, updates and deletes of replicated
//!   entities, each update listing the components it carries, from the
//!   component registry in chunk 0.
//!
//! Kind 2 is the chunk's seek snapshot: every live entity with its type and,
//! for objects, its tag and position ([`Snapshot`] gives the types and
//! tags). Kind 8 holds the players' structs and
//! kind 12 the bots' records.
//!
//! This layer follows the format as far as it is understood, so it changes
//! more often than the model: it comes with the `raw` feature, outside the
//! crate's semantic versioning, and can break in any release.
//!
//! ```no_run
//! # fn run(film: &filmshell::Film) {
//! let raw = film.raw();
//! for frame in raw.frames().take(10) {
//!     for e in raw.events(&frame) {
//!         let name = filmshell::format::event_name(e.kind).unwrap_or("?");
//!         println!("{} {name} at bit {}", frame.tick, e.bit);
//!     }
//! }
//! # }
//! ```

use crate::TagId;
use crate::container::{Film as Container, Framing};
use crate::model::{EntityId, Tick};

/// MSB-first bit reads over a byte slice. Reads past the end give zero bits.
#[derive(Clone, Copy, Debug)]
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: u64,
}

impl<'a> BitReader<'a> {
    /// A reader at bit 0 of `data`.
    pub fn new(data: &'a [u8]) -> BitReader<'a> {
        BitReader { data, pos: 0 }
    }

    /// A reader at bit `pos`.
    pub fn at(data: &'a [u8], pos: u64) -> BitReader<'a> {
        BitReader { data, pos }
    }

    /// The next bit's position.
    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Moves to bit `pos`.
    pub fn seek(&mut self, pos: u64) {
        self.pos = pos;
    }

    /// Reads `n` bits (at most 32) as an unsigned number, the first bit
    /// most significant.
    ///
    /// # Panics
    ///
    /// If `n` is over 32.
    pub fn read(&mut self, n: u32) -> u32 {
        assert!(n <= 32, "read at most 32 bits at a time");
        let v = crate::bits::bits_at(self.data, self.pos as i64, n);
        self.pos += n as u64;
        v
    }

    /// Reads one bit.
    pub fn read_bit(&mut self) -> bool {
        self.read(1) == 1
    }

    /// Reads `n` bits (at most 64).
    ///
    /// # Panics
    ///
    /// If `n` is over 64.
    pub fn read_u64(&mut self, n: u32) -> u64 {
        assert!(n <= 64, "read at most 64 bits at a time");
        let hi = n.saturating_sub(32);
        let h = self.read(hi) as u64;
        (h << (n - hi)) | self.read(n - hi) as u64
    }

    /// Bits left before the end of the data.
    pub fn remaining(&self) -> u64 {
        (self.data.len() as u64 * 8).saturating_sub(self.pos)
    }
}

/// One packet of a replay chunk.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Packet<'a> {
    /// The chunk's index.
    pub chunk: u32,
    /// The packet kind: 0 a tick, 1 the first of a replay chunk, 2 the seek
    /// snapshot, 8 player structs, 12 bot records.
    pub kind: u16,
    /// The packet's timestamp, microseconds.
    pub timestamp_us: u64,
    /// The payload.
    pub payload: &'a [u8],
    /// The payload's first byte in the chunk.
    pub offset: usize,
}

/// One frame: a tick packet, or on a film framed by its markers (the
/// fallback when a replay chunk's packets don't walk to its end) the span
/// from one marker to the next.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Frame<'a> {
    /// The frame's index, counting the film's frames from 0: the index
    /// [`Clock::tick`](crate::Clock::tick) takes and
    /// [`Raw::for_each_entity_view`] passes to its closure.
    pub index: usize,
    /// Its tick.
    pub tick: Tick,
    /// The chunk it is in.
    pub chunk: u32,
    /// The whole chunk, which bit positions in it index.
    pub chunk_data: &'a [u8],
    /// The payload's first and one-past-last byte in the chunk.
    pub span: (usize, usize),
    /// The packet's timestamp, microseconds; None on a film framed by its
    /// `A0 7B 42` markers, the fallback when a replay chunk's packets don't
    /// walk to its end.
    pub timestamp_us: Option<u64>,
}

impl<'a> Frame<'a> {
    /// The payload's bytes.
    pub fn payload(&self) -> &'a [u8] {
        &self.chunk_data[self.span.0..self.span.1]
    }
}

/// An object handle in an event: the entity it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Handle {
    /// The entity index (bipeds from 512, vehicles from 768).
    pub index: u32,
    /// Its 2-bit generation.
    pub generation: u32,
    /// Whether the handle names a unit.
    pub unit: bool,
}

impl Handle {
    /// The biped's or vehicle's id in the model ([`entity_id`]).
    pub fn entity_id(&self) -> Option<EntityId> {
        entity_id(self.index, self.generation)
    }
}

/// The model's [`EntityId`] of a biped's or a vehicle's entity index and
/// generation.
///
/// Bipeds are indexes 512 to 767 and vehicles 768 to 1023. The id is the one
/// [`Life::entity`](crate::Life::entity),
/// [`VehicleTrack::entity`](crate::VehicleTrack::entity) and
/// [`Sample::vehicle`](crate::Sample::vehicle) use. None for other entities and
/// generation 0.
pub fn entity_id(index: u32, generation: u32) -> Option<EntityId> {
    let base = match index {
        512..=767 => 512,
        768..=1023 => 768,
        _ => return None,
    };
    let id = generation.checked_sub(1)? * 256 + index - base;
    u16::try_from(id).ok().map(EntityId)
}

/// One event of a tick packet's event view.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RawEvent {
    /// The event's type.
    pub kind: u32,
    /// Its handles.
    pub handles: [Option<Handle>; 3],
    /// The bit of the type field in the chunk.
    pub bit: i64,
}

/// What an entity-view entry does.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EntityOp {
    /// The entity is created; objects name their tag.
    Create {
        /// The object's tag.
        tag: Option<TagId>,
    },
    /// The entity's components change.
    Update {
        /// The components the update carries: rows of the entity type's
        /// block in the component registry.
        components: Vec<u32>,
    },
    /// The entity is deleted.
    Delete,
}

/// One entity-view entry.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct EntityEntry {
    /// The entity's index.
    pub index: u32,
    /// Its generation.
    pub generation: u32,
    /// Its type (block of the component registry), when known.
    pub entity_type: Option<u32>,
    /// What happens.
    pub op: EntityOp,
    /// The bit of the entry's index field in the chunk.
    pub bit: i64,
}

impl EntityEntry {
    /// The biped's or vehicle's id in the model ([`entity_id`]).
    pub fn entity_id(&self) -> Option<EntityId> {
        entity_id(self.index, self.generation)
    }
}

/// One entity of a seek snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SnapshotEntity {
    /// The entity's index.
    pub index: u32,
    /// Its generation.
    pub generation: u32,
    /// Its type (block of the component registry).
    pub entity_type: u32,
    /// An object's tag.
    pub tag: Option<TagId>,
}

/// A replay chunk's seek snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Snapshot {
    /// The chunk.
    pub chunk: u32,
    /// Every entity alive at the chunk's start, in index order.
    pub entities: Vec<SnapshotEntity>,
}

/// The film's raw layers. See [`crate::Film::raw`].
#[derive(Clone, Copy, Debug)]
pub struct Raw<'a> {
    film: &'a Container,
    has_highlights: bool,
    params: Option<crate::components::Params>,
    encoding: Encoding,
}

pub use crate::Encoding;

impl<'a> Raw<'a> {
    pub(crate) fn new(
        film: &'a Container,
        has_highlights: bool,
        params: Option<crate::components::Params>,
        encoding: Encoding,
    ) -> Raw<'a> {
        Raw { film, has_highlights, params, encoding }
    }

    /// How the tick packets carry positions.
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Chunk 0, inflated.
    pub fn header_chunk(&self) -> &'a [u8] {
        &self.film.chunks[0]
    }

    /// The replay chunks, inflated, with their indices.
    pub fn replay_chunks(&self) -> impl Iterator<Item = (u32, &'a [u8])> + use<'a> {
        let n = self.film.chunks.len() - usize::from(self.has_highlights);
        self.film.chunks[..n].iter().enumerate().skip(1).map(|(i, c)| (i as u32, c.as_slice()))
    }

    /// The highlights chunk, inflated.
    pub fn highlights_chunk(&self) -> Option<&'a [u8]> {
        self.has_highlights.then(|| self.film.chunks[self.film.chunks.len() - 1].as_slice())
    }

    /// Every packet of the replay chunks, in order. A chunk whose packets
    /// don't walk to its last byte has none.
    pub fn packets(&self) -> impl Iterator<Item = Packet<'a>> + use<'a> {
        let film = self.film;
        self.replay_chunks().flat_map(move |(ci, c)| {
            film.packets(ci as usize).unwrap_or(&[]).iter().map(move |p| Packet {
                chunk: ci,
                kind: p.kind,
                timestamp_us: p.ts_us,
                payload: &c[p.start..p.end],
                offset: p.start,
            })
        })
    }

    /// Every frame, in order.
    pub fn frames(&self) -> impl Iterator<Item = Frame<'a>> + use<'a> {
        let film = self.film;
        let chunk_of = move |c: &[u8]| film.chunks.iter().position(|x| x.as_ptr() == c.as_ptr()).unwrap_or(0) as u32;
        film.frames().map(move |f| Frame {
            index: f.index,
            tick: Tick(film.frame_ticks().get(f.index).copied().unwrap_or(0)),
            chunk: chunk_of(f.chunk),
            chunk_data: f.chunk,
            span: (f.start, f.end),
            timestamp_us: f.ts_us,
        })
    }

    /// The event view of `frame`, as far as it reads. Empty when the film's
    /// component layouts aren't known (they come with the position field
    /// widths) or its frames aren't tick packets.
    pub fn events(&self, frame: &Frame<'_>) -> Vec<RawEvent> {
        let Some(p) = self.params else { return Vec::new() };
        if self.film.framing != Framing::Packets || frame.span.1 - frame.span.0 < 2 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut r = crate::events::Bits { c: frame.chunk_data, at: 8 * frame.span.0 as i64 + 1 };
        let _ = crate::events::walk(&mut r, (&p).into(), &mut out);
        out.iter()
            .map(|e| RawEvent {
                kind: e.kind,
                handles: e
                    .handles
                    .map(|h| h.map(|h| Handle { index: h.index, generation: h.generation, unit: h.unit })),
                bit: e.at,
            })
            .collect()
    }

    /// Walks every frame's entity view in order, calling `f` with the frame
    /// index and its entries. Entities alive at a chunk's start are typed
    /// from its snapshot. Nothing when the film's component layouts aren't
    /// known (they come with the position field widths) or its frames aren't
    /// tick packets.
    pub fn for_each_entity_view(&self, mut f: impl FnMut(usize, &[EntityEntry])) {
        let Some(p) = self.params else { return };
        let mut walker = crate::payload::Walker::new(&self.film.chunks[0], p);
        let mut entries = Vec::new();
        crate::payload::for_each_payload(self.film, &mut walker, |i, payload| {
            entries.clear();
            entries.extend(payload.ops.iter().map(|op| EntityEntry {
                index: op.index,
                generation: op.generation,
                entity_type: op.entity_type,
                op: match &op.kind {
                    crate::payload::OpKind::Create { tag } => EntityOp::Create { tag: tag.map(TagId) },
                    crate::payload::OpKind::Update { list, .. } => EntityOp::Update { components: list.clone() },
                    crate::payload::OpKind::Delete => EntityOp::Delete,
                },
                bit: op.at,
            }));
            f(i, &entries);
        });
    }

    /// Every replay chunk's seek snapshot.
    pub fn snapshots(&self) -> Vec<Snapshot> {
        let mut out = Vec::new();
        for (ci, c) in self.replay_chunks() {
            let Some(ps) = self.film.packets(ci as usize) else { continue };
            for pk in ps.iter().filter(|p| p.kind == crate::snapshot::ENTITY_TABLE) {
                let entities = crate::snapshot::entity_table(c, pk.start, pk.end)
                    .into_iter()
                    .map(|e| SnapshotEntity {
                        index: e.index,
                        generation: e.generation,
                        entity_type: e.entity_type,
                        tag: crate::snapshot::creation(c, &e).map(|x| TagId(x.0.tag)),
                    })
                    .collect();
                out.push(Snapshot { chunk: ci, entities });
            }
        }
        out
    }

    /// The component registry: each entity type's component names, by row.
    pub fn component_names(&self) -> Vec<Vec<String>> {
        crate::registry::blocks(&self.film.chunks[0])
    }
}

/// The game's name of event type `kind`, from the executable's event
/// definitions (`"action_weapon_fire"`, `"PlayerKilledEvent"`). The names are
/// the game's own, spelling included.
pub fn event_name(kind: u32) -> Option<&'static str> {
    EVENT_NAMES.get(kind as usize).copied()
}

/// The game's name of entity type `entity_type`, a block of chunk 0's component
/// registry.
///
/// The names are the executable's entity definitions' (`"biped"`, `"weapon"`,
/// `"game-engine-statborg"`). The game names types 2 and 10 both
/// `"managed-engine-object"`.
pub fn entity_type_name(entity_type: u32) -> Option<&'static str> {
    ENTITY_TYPE_NAMES.get(entity_type as usize).copied().filter(|n| !n.is_empty())
}
/// The game's name of each event type, by type.
const EVENT_NAMES: [&str; 123] = [
    "damage_aftermath",
    "damage_section_response",
    "restore_damage_section",
    "item_detonate",
    "item_detonate_countdown",
    "projectile_detonate",
    "projectile_impact_effect",
    "projectile_object_impact_effect",
    "biped_board_vehicle",
    "biped_pickup",
    "weapon_effect",
    "weapon_empty_click",
    "biped_melee_clang",
    "motor_system_interruption",
    "PlayEffectOnObject",
    "Script",
    "ShowDebugText",
    "Allegiance",
    "MusicTrigger",
    "CollectibleUnlockEvent",
    "incident",
    "unit_zoom",
    "unit_exit_vehicle",
    "authority_ignored_predicted_position",
    "trade_weapon",
    "device_touch",
    "deviceRelease",
    "controlToggleResponse",
    "biped_debug_teleport",
    "prediction_determinism_msg",
    "biped_equipment_activation",
    "equipment_teleport_request",
    "unit_teleported",
    "vehicle_auto_turret_choose_target",
    "PromptToBootGriefer",
    "request_weapon_fire",
    "action_weapon_fire",
    "weapon_overheat",
    "weapon_reload",
    "biped_throw_initiate",
    "biped_melee_initiate",
    "vehicle_trick",
    "biped_dodge",
    "initiate_mobility_action",
    "weapon_pickup",
    "weapon_put_away",
    "weapon_drop",
    "weapon_throw",
    "weapon_tether_request",
    "vehicle_flip",
    "request_ai_mount_exit",
    "biped_throw_release",
    "biped_melee_damage",
    "unit_enter_vehicle",
    "unit_switch_seat",
    "game_engine_request_boot_player",
    "request_projectile_attach",
    "biped_pickup_item_request",
    "projectile_supercombine_request",
    "object_refresh",
    "RequestChangeFrameConfiguration",
    "player_forge_action",
    "player_loadout_request",
    "biped_laser_designation",
    "player_set_respawn_target_transform",
    "player_set_orbiting_camera_target",
    "PlayerEmote",
    "player_force_base_respawn",
    "supply_request",
    "CampaignMapStateUpdate",
    "AIPhase",
    "AIRequestIdleTransitionTime",
    "AILand",
    "AIJuke",
    "AISetMotorProgram",
    "AIDialog",
    "Dialogue2D",
    "DebugSendCameraPosition",
    "ai_jump",
    "networked_ai_action",
    "networked_ai_effect",
    "PlayerGameEvent",
    "PlayerGameEventSmall",
    "TeamGameEvent",
    "TeamGameEventSmall",
    "PlayerKilledEvent",
    "EngineClientEvent",
    "SaveGame",
    "RevertMap",
    "CancelCinematic",
    "ClientOnlyShowComplete",
    "ClientResourcesLoadComplete",
    "BetrayResponse",
    "activate_spartan_ability",
    "CrewSetTargetObject",
    "CrewOrderPositionAdd",
    "NetworkedCrewEventType",
    "SaveToUGCService",
    "Equipment",
    "SelectedSpawnZoneChangedEvent",
    "PowerUpApplied",
    "LoadForgeObjectGroup",
    "NetworkedActionRequest",
    "EquipmentSpawnedObject",
    "EquipmentKnockbackPlayer",
    "EquipmentObjectKnockedBack",
    "ObjectCollisionDamage",
    "player_forge_user_string_action",
    "NavpointRequest",
    "PersonalAILifceycleEffect",
    "ObjectDeterministicDamageAcceleration",
    "QueueNextShow",
    "SetDifficultyAndSkulls",
    "FOBClientInput",
    "MusicMarker",
    "synchronized_teleport",
    "teleport_effects",
    "EquipmentTranslocatorTeleportEffects",
    "repair_complete",
    "EquipmentKnockbackRequest",
    "PlayerCalloutRequest",
    "PlayerForgeableCustomAction",
    "PlayerTriggerRadialMenu",
];

/// The game's name of each entity type, by type; empty where the game has none.
const ENTITY_TYPE_NAMES: [&str; 50] = [
    "forge-engine-globals",
    "campaign_engine_globals",
    "managed-engine-object",
    "game-keyframing-system",
    "game-time-system",
    "game-engine-player",
    "game-engine-statborg",
    "ForgeSimEntity",
    "",
    "managed-engine-player",
    "managed-engine-object",
    "managed-engine-objective",
    "managed-engine-navpoint",
    "managed-engine-networked-property",
    "engine-crew",
    "nav_point_scripting",
    "game_composer",
    "game_music",
    "scripted_effects",
    "scripted_sound_placements",
    "spawn-filter",
    "game_flock",
    "game_physics",
    "selectable-spawn-filter-manager",
    "persistence",
    "game_powerframe",
    "supply_lines_blocked",
    "supply_lines_items",
    "game_narrative_moment",
    "managed-engine-respawn-manager",
    "tac_map_icon",
    "tac_map_mission",
    "tac_map_areaofinterest",
    "tac_map_display_asset",
    "tac_map_settings",
    "biped",
    "item",
    "equipment",
    "generic",
    "generic-garbage",
    "vehicle",
    "projectile",
    "weapon",
    "device",
    "asset",
    "game_matchflow",
    "StateBroker",
    "managed-engine-participant",
    "forge-engine-player-data",
    "managed-engine-radialmenu",
];

#[cfg(test)]
mod tests {
    use super::BitReader;

    #[test]
    fn bits_read_msb_first() {
        let mut r = BitReader::new(&[0b1010_0000, 0xff]);
        assert!(r.read_bit());
        assert_eq!(r.read(3), 0b010);
        assert_eq!(r.read(8), 0b0000_1111);
        assert_eq!(r.read_u64(40), 0b1111 << 36);
        assert_eq!(r.remaining(), 0);
    }
}
