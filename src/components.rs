//! Component data of an object update, read field by field the way the game
//! reads it.
//!
//! An update is `[3 class][8 index][2 generation][5 count][count 6-bit
//! components][data]`, the data one block per listed component, and the next
//! update follows after 3 bits (`100`). This module reads the blocks of the
//! components it knows; an update that lists any other component can't be
//! parsed past it, and `parse` returns the blocks before it.
//!
//! The layouts are the game's own readers'. A few widths there are run-time
//! values (`Params`); their defaults are the values with which updates parse
//! to the next update.
//!
//! Updates are found by scanning (`replication.rs`); this module reads their
//! blocks, for each slot's state (`SlotStates`: aim, speed, health,
//! shields).

use crate::bits::{bits_at, Shifted};
use crate::container::Film;
use crate::creation;
use crate::registry::{self, Schema};
use crate::replication::{components as list_of, for_each_object_update, Kind};
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

/// The components this module can read, by registry name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    Position,
    Velocity,
    ForwardAndUp,
    AngularVelocity,
    RegionState,
    Constraint,
    MaximumVitalities,
    LowFrequency,
    PhysicsFlags,
    FrameConfiguration,
    BodyVitality,
    ShieldVitality,
    UnitControl,
    ActorControl,
    ActorState,
    Aim,
    GrenadeCounts,
    UnitMalleable,
    UnitLowFrequency,
    CommandTick,
    Equipment,
    Stun,
    Camo,
    Crouch,
    WeaponAmmo,
    WeaponRounds,
    WeaponOverheated,
    DesiredWeaponSet,
    DesiredGrenadeSet,
    DesiredAbilitySet,
    ControlContext,
    MapEditorFlag,
    EmpTimer,
    BipedLowFrequency,
    BipedMalleable,
    MobilityAction,
    AbilityEnergy,
    Ability,
    AbilityMalleable,
    AbilityNonPredicted,
    Slide,
    DamageSections,
    MultiplayerProperties,
    ParentState,
    DeadState,
    Scale,
    Dissolver,
    SimulationState,
    TypeInfo,
    PosturePhysics,
    BipedAction,
    // Items, equipment and rigid bodies.
    ObjectPosition,
    ObjectVelocity,
    ItemAtRest,
    ItemIgnorePlayer,
    EquipmentDeployed,
    EquipmentActivated,
    EquipmentControlSignal,
    EquipmentCreator,
    EquipmentEnergy,
    EquipmentBeingHacked,
    EquipmentDelayTicks,
    EquipmentCharges,
    EquipmentTrackedHandles,
    EquipmentCommandTick,
    EquipmentInfiniteUses,
    RigidBodyTransforms,
    AnimatedMesh,
    // Vehicle archetype.
    ForwardAndUpDynamic,
    AngularVelocityDynamic,
    VehicleEmpTimer,
    DevicePosition,
    WeaponObjectAmmo,
    SimulationPlayback,
    TurretTriggers,
    ProjectileAtRest,
    TurretAim,
    VehicleOpenState,
    TurretTarget,
    VehicleSentry,
    VehicleAutoTurret,
    VehicleTurretParent,
    VehicleSeatOverride,
    AirDropFlight,
    Warp,
    VehicleLowFrequency,
    ProjectileTether,
    ProjectileCommandTick,
    ProjectileDeceleration,
    DeviceAnimationName,
    DeviceAnimationControl,
    DevicePositionGroup,
    DevicePower,
    DeviceInteraction,
    DeviceTime8,
    DeviceActionStrings,
    DeviceHealthCharges,
    DeviceBit,
    DispenserMonitors,
    DispenserFlags,
    AnimationLayerSettings,
    AnimationLayerState,
    DispenserState,
    DispenserTimer,
    DeviceTransitionVelocity,
    DeviceMachineFlags,
    VehicleTypeState,
    VehicleTypePhysics,
    Skip1,
    Skip2,
    Skip3,
    Skip4,
    Skip5,
    Skip6,
    Skip7,
    Skip8,
    Skip10,
    Skip15,
    Skip16,
    Skip19,
    Skip21,
    Skip24,
    Skip32,
    Skip40,
    Skip64,
    Skip72,
    PlayerDesiredSeat,
    PlayerRespawnObject,
    PlayerDesiredLocation,
    PlayerMalleable,
    PlayerAimAssist,
    PlayerFrameConfiguration,
    StatCurrentRound,
    StatFinalizedRounds,
    CustomInputPrompt,
    CrewOrder,
    CrewMarkedObjects,
    MusicVariables,
    MusicState,
    SplashStatic,
    PersonalAi,
    Bits1,
    Bits2,
    Bits3,
    Bits4,
    Bits5,
    Bits8,
    Bits9,
    Bits10,
    Bits12,
    Bits14,
    Bits16,
    Bits17,
    Bits32,
    Bits37,
    Bits64,
    Bits128,
    TeamMapping,
    CurrentRound,
    ScreenSequence,
    Alliance,
    KillVolumes,
    Letterbox,
    ManagedTimers,
    ScenarioIntro,
    KeyframeLow,
    InteractionFilter,
    Rtpc,
    FormattedText,
    DistanceFilters,
    BoolFilters,
    NavpointText,
    NavpointTimers,
    NavpointOffset,
    NavpointVisualStates,
    PropertyValue,
    MaskedProperty,
    CutsceneFlag,
    EffectState,
    SoundPlacement,
    FlockDestination,
    FlockDanger,
    PhysicsState,
    PowerframeSelection,
    SupplyBlocked,
    SupplyBusy,
    RespawnBlock,
    TacmapLod,
    TacmapDungeon,
    TacmapLights,
    TacmapWaypoint,
    TacmapQueued,
    TacmapPlayers,
    TacmapTether,
    TacmapOverride,
    TacmapTooltip,
    MatchflowSequence,
    MatchflowFocus,
    NavpointStateGroup,
    VehicleWeaponSet,
    RadialVisibility,
}

impl Component {
    fn from_name(name: &str) -> Option<Component> {
        use Component::*;
        Some(match name {
            "object-position-dynamic-precision-component" => Position,
            "object-translational-velocity-dynamic-precision-component" => Velocity,
            "object-forward-and-up-component" => ForwardAndUp,
            "object-angular-velocity-component" => AngularVelocity,
            "object-region-state-component" => RegionState,
            "object-constraint-component" => Constraint,
            "object-maximum-vitalities-component" => MaximumVitalities,
            "object-low-frequency-component" => LowFrequency,
            "object-physics-flags-component" => PhysicsFlags,
            "object-frame-configuration-component" => FrameConfiguration,
            "object-body-vitality-component" => BodyVitality,
            "object-shield-vitality-component" => ShieldVitality,
            "unit-control-component" => UnitControl,
            "unit-actor-control-component" => ActorControl,
            "unit-actor-state-component" => ActorState,
            "unit-desired-aiming-vector-component" => Aim,
            "unit-grenade-counts-component" => GrenadeCounts,
            "unit-malleable-property-component" => UnitMalleable,
            "unit-low-frequency-component" => UnitLowFrequency,
            "unit-command-tick-component" => CommandTick,
            "unit-equipment-component" => Equipment,
            "unit-stun-component" => Stun,
            "unit-active-camo-state-component" => Camo,
            "unit-crouch-component" => Crouch,
            "weapon-state-ammo" => WeaponAmmo,
            "weapon-state-rounds-inventory" => WeaponRounds,
            "weapon-state-overheated" => WeaponOverheated,
            "biped-desired-weapon-set" => DesiredWeaponSet,
            "biped-desired-grenade-set-component" => DesiredGrenadeSet,
            "biped-desired-ability-set-component" => DesiredAbilitySet,
            "biped-control-context-component" => ControlContext,
            "biped-map-editor-flag-component" => MapEditorFlag,
            "biped-emp-timer-component" => EmpTimer,
            "biped-low-frequency-data-component" => BipedLowFrequency,
            "biped-malleable-property-component" => BipedMalleable,
            "biped-mobility-action-component" => MobilityAction,
            "biped-spartan-ability-energy-component" => AbilityEnergy,
            "biped-spartan-ability-component" => Ability,
            "biped-spartan-ability-malleable-property-component" => AbilityMalleable,
            "biped-spartan-ability-non-predicted-state" => AbilityNonPredicted,
            "biped-slide-component" => Slide,
            "object-damage-sections-component" => DamageSections,
            "object-multiplayer-properties-component" => MultiplayerProperties,
            "object-parent-state-component" => ParentState,
            "object-dead-state-component" => DeadState,
            "object-scale-component" => Scale,
            "object-dissolver-component" => Dissolver,
            "simulation-state-component" => SimulationState,
            "weapon-state-type-info" => TypeInfo,
            "biped-posture-physics-component" => PosturePhysics,
            "biped-action-component" => BipedAction,
            "object-position-component" => ObjectPosition,
            "object-translational-velocity-component" => ObjectVelocity,
            "item-at-rest-component" => ItemAtRest,
            "item-ignore-player-component" => ItemIgnorePlayer,
            "equipment-deployed-component" => EquipmentDeployed,
            "equipment-activated-component" => EquipmentActivated,
            "equipment-control-signal-component" => EquipmentControlSignal,
            "equipment-creator-component" => EquipmentCreator,
            "equipment-energy-component" => EquipmentEnergy,
            "equipment-being-hacked-component" => EquipmentBeingHacked,
            "equipment-energy-delay-ticks-left-component" => EquipmentDelayTicks,
            "equipment-charges-remaining-component" => EquipmentCharges,
            "equipment-tracked-object-handles-stack-component" => EquipmentTrackedHandles,
            "equipment-command-tick-component" => EquipmentCommandTick,
            "equipment-has-infinite-uses-component" => EquipmentInfiniteUses,
            "generic-rigid-body-transforms-component" => RigidBodyTransforms,
            "animated-mesh-dynamic-state-component" => AnimatedMesh,
            "object-forward-and-up-dynamic-precision-component" => ForwardAndUpDynamic,
            "object-angular-velocity-dynamic-precision-component" => AngularVelocityDynamic,
            "vehicle-emp-timer-component" => VehicleEmpTimer,
            "vehicle-weapon-set-component" => VehicleWeaponSet,
            "device-position-component" => DevicePosition,
            "weapon-ammo-component" => WeaponObjectAmmo,
            "simulation-state-playback-component" => SimulationPlayback,
            "vehicle-auto-turret-triggers-component" => TurretTriggers,
            "projectile-at-rest-state" => ProjectileAtRest,
            "vehicle-auto-turret-aiming-vector-component" => TurretAim,
            "vehicle-transformed-or-desired-open-state-changed-component" => VehicleOpenState,
            "vehicle-auto-turret-target-component" => TurretTarget,
            "vehicle-sentry-state-component" => VehicleSentry,
            "vehicle-auto-turret-component" => VehicleAutoTurret,
            "vehicle-equipment-turret-parent-component" => VehicleTurretParent,
            "vehicle-seats-override-pitch-component" => VehicleSeatOverride,
            "vehicle-seats-override-yaw-component" => VehicleSeatOverride,
            "air-drop-flight-component" => AirDropFlight,
            "warp-component" => Warp,
            "vehicle-low-frequency-component" => VehicleLowFrequency,
            "projectile-tether-state" => ProjectileTether,
            "projectile-command_tick" => ProjectileCommandTick,
            "projectile-deceleration-disabled-state" => ProjectileDeceleration,
            "device-position-animation-name-component" => DeviceAnimationName,
            "device-position-animation-control-component" => DeviceAnimationControl,
            "device-position-group-component" => DevicePositionGroup,
            "device-power-component" => DevicePower,
            "device-power-group-component" => DevicePower,
            "device-interaction-in-progress-component" => DeviceInteraction,
            "device-exclusive-user-component" => DeviceInteraction,
            "device-interaction-hold-time-component" => DeviceTime8,
            "device-interaction-start-time-override-component" => DeviceTime8,
            "device-control-action-string-override-component" => DeviceActionStrings,
            "device-health-station-charges-component" => DeviceHealthCharges,
            "device-health-station-in-use-component" => DeviceBit,
            "device-in-primary-mode-component" => DeviceBit,
            "device-dispenser-require-los-component" => DeviceBit,
            "device-dispenser-monitors-changed-component" => DispenserMonitors,
            "device-dispenser-state-flags-component" => DispenserFlags,
            "device-animation-layer-settings-component" => AnimationLayerSettings,
            "device-animation-layer-state-component" => AnimationLayerState,
            "device-dispenser-state-component" => DispenserState,
            "device-object-dispenser-timer-component" => DispenserTimer,
            "device-position-transition-velocity-component" => DeviceTransitionVelocity,
            "device-machine-flags-component" => DeviceMachineFlags,
            "vehicle-type-state-component" => VehicleTypeState,
            "vehicle-type-physics-component" => VehicleTypePhysics,
            "player-early-respawn-requested-component" => Skip1,
            "player-vehicle-entrance-ban-component" => Skip1,
            "player-active-in-game-component" => Skip1,
            "player-pending-join-in-progress-spawn-component" => Skip1,
            "player-allowed-to-quit-component" => Skip1,
            "managed-player-show-active-mission-name-in-hud-component" => Skip1,
            "player-target-tracking-detection-component" => Skip2,
            "player-waypoint-component" => Skip3,
            "player-engine-loadout-index-component" => Skip3,
            "managed-player-team-designator-component" => Skip4,
            "managed-player-flags-component" => Skip4,
            "player-respawn-safety-component" => Skip5,
            "player-last-betrayer-component" => Skip6,
            "player-lives-remaining-component" => Skip7,
            "managed-player-campaign-progress-component" => Skip8,
            "crew-orders-off-flags-component" => Skip8,
            "player-unsafe-respawn-timer-component" => Skip10,
            "player-soft-kill-timer-component" => Skip15,
            "player-desired-respawn-player-component" => Skip16,
            "player-supply-lines-currency-simulation-component" => Skip16,
            "player-control-aiming-component" => Skip19,
            "player-respawn-timer-component" => Skip21,
            "managed-object-networked-splash-message-dynamic-component" => Skip24,
            "player-representation-component" => Skip32,
            "player-power-frame-points-component" => Skip32,
            "managed-player-back-button-scoreboard-flair-component" => Skip32,
            "managed-player-current-season-component" => Skip32,
            "statborg-entry-index-and-type-component" => Skip40,
            "player-engine-loadout-component" => Skip64,
            "managed-player-color-override-component" => Skip64,
            "managed-player-forge-weather-effect-overrides-component" => Skip64,
            "managed-player-active-mission-name-component" => Skip64,
            "statborg-round-outcomes-component" => Skip64,
            "player-fade-properties-component" => Skip72,
            "player-desired-respawn-seat-component" => PlayerDesiredSeat,
            "player-primary-respawn-object-component" => PlayerRespawnObject,
            "player-desired-respawn-location-component" => PlayerDesiredLocation,
            "player-malleable-properties-simulation-component" => PlayerMalleable,
            "player-aim-assist-component" => PlayerAimAssist,
            "player-desired-frame-configuration-component" => PlayerFrameConfiguration,
            "statborg-current-round-value-stat-component" => StatCurrentRound,
            "statborg-finalized-rounds-values-stat-component" => StatFinalizedRounds,
            "managed-player-custom-input-prompt-widget" => CustomInputPrompt,
            "crew-order-component" => CrewOrder,
            "crew-marked-objects-component" => CrewMarkedObjects,
            "music-variables-component" => MusicVariables,
            "music-state-component" => MusicState,
            "managed-object-networked-splash-message-static-component" => SplashStatic,
            "personal-ai-data-component" => PersonalAi,
            "flock-destroying-component" => Bits1,
            "flock-emitting-component" => Bits1,
            "flock-fleeing-component" => Bits1,
            "flock-forced-respawn-component" => Bits1,
            "game-engine-game-finished-component" => Bits1,
            "managed-objective-enabled-component" => Bits1,
            "managed-objective-forced-update-component" => Bits1,
            "managed-objective-is-new-and-unseen-component" => Bits1,
            "managed-objective-is-only-one-item-unlocked-component" => Bits1,
            "tacmap-fasttravelstate" => Bits1,
            "managed-object-flags-component" => Bits2,
            "game-engine-current-state-component" => Bits3,
            "managed-radialmenu-enabled-component" => Bits1,
            "managed-radialmenu-identifier-component" => Bits32,
            "managed-radialmenu-item-count-component" => Bits32,
            "managed-radialmenu-iconindex-component" => Bits32,
            "managed-radialmenu-title-component" => FormattedText,
            "managed-radialmenu-description-component" => FormattedText,
            "managed-radialmenu-visibility-component" => RadialVisibility,
            "managed-objective-state-component" => Bits3,
            "flock-current-destination-component" => Bits4,
            "managed-objective-message-type-component" => Bits4,
            "managed-navpoint-override-flags" => Bits5,
            "flock-relevancy-component" => Bits8,
            "high-frequency" => Bits8,
            "managed-navpoint-bottom-progress" => Bits8,
            "managed-navpoint-docking-order-component" => Bits8,
            "managed-navpoint-flags-component" => Bits8,
            "managed-navpoint-radial-progress" => Bits8,
            "managed-navpoint-top-progress" => Bits8,
            "managed-objective-outro-phase-duration-component" => Bits8,
            "managed-objective-priority-component" => Bits8,
            "matchflow-isplaying-flags-component" => Bits8,
            "tacmap-missioncount" => Bits9,
            "game-engine-round-condition-flags-component" => Bits10,
            "tacmap-cameraheading" => Bits12,
            "managed-objective-timers-component" => Bits14,
            "tacmap-backmenulocked-state" => Bits16,
            "managed-navpoint-manual-timer-current-duration-component" => Bits17,
            "managed-navpoint-manual-timer-initial-duration-component" => Bits17,
            "managed-navpoint-docking-group-name-component" => Bits32,
            "managed-navpoint-object-marker" => Bits32,
            "managed-navpoint-sub-type-component" => Bits32,
            "managed-object-boundary-color-component" => Bits32,
            "managed-object-boundary-visibility-component" => Bits32,
            "managed-object-looping-sound-component" => Bits32,
            "managed-object-navpoint-component" => Bits32,
            "managed-object-networked-property-component" => Bits32,
            "managed-object-property-name-component" => Bits32,
            "managed-objective-color-component" => Bits32,
            "managed-objective-object-reference-component" => Bits32,
            "managed-objective-parent-objective-component" => Bits32,
            "managed-objective-progress-component" => Bits32,
            "managed-objective-required-progress-component" => Bits32,
            "managed-objective-sub-objective-entities-component" => Bits32,
            "managed-objective-type-component" => Bits32,
            "supply-lines-item-unlocked-component" => Bits32,
            "tacmap-mapscale" => Bits32,
            "tacmap-settingstag" => Bits32,
            "game-engine-campaign-timer-component" => Bits37,
            "game-engine-grace-period-time-left-component" => Bits37,
            "game-engine-round-timer-component" => Bits37,
            "game-engine-sudden-death-time-left-component" => Bits37,
            "game-engine-shared-team-lives-component" => Bits64,
            "tacmap-missionmarkerstate" => Bits64,
            "game-engine-soft-ceilings-component" => Bits128,
            "game-engine-team-mapping-component" => TeamMapping,
            "game-engine-current-round-component" => CurrentRound,
            "game-engine-screen-sequence-component" => ScreenSequence,
            "game-engine-alliance-component" => Alliance,
            "game-engine-disabled-kill-volume-flags-component" => KillVolumes,
            "GameEngineComposerLetterboxComponent" => Letterbox,
            "managed-engine-timers-component" => ManagedTimers,
            "scenario-intro-component" => ScenarioIntro,
            "low-frequency" => KeyframeLow,
            "managed-object-interaction-filter-component" => InteractionFilter,
            "managed-objective-interaction-filter-component" => InteractionFilter,
            "managed-navpoint-visibility-filter-component" => InteractionFilter,
            "managed-navpoint-docking-filter-component" => InteractionFilter,
            "managed-object-rtpc-component" => Rtpc,
            "managed-objective-formatted-text-component" => FormattedText,
            "managed-objective-secondary-formatted-text-component" => FormattedText,
            "managed-navpoint-visibility-distance-filters-component" => DistanceFilters,
            "managed-navpoint-visible-offscreen-filters-component" => BoolFilters,
            "managed-navpoint-can-be-occluded-filters-component" => BoolFilters,
            "managed-navpoint-formatted-text-component" => NavpointText,
            "managed-navpoint-timers-component" => NavpointTimers,
            "managed-navpoint-position-offset" => NavpointOffset,
            "flock-position-component" => NavpointOffset,
            "managed-navpoint-visual-states-component" => NavpointVisualStates,
            "managed-object-property-component" => PropertyValue,
            "managed-object-player-masked-property-component" => MaskedProperty,
            "nav-cutscene-flag-component" => CutsceneFlag,
            "effect-state-data-component" => EffectState,
            "sound-placement-state-data-component" => SoundPlacement,
            "flock-destination-component" => FlockDestination,
            "flock-remembered-danger-component" => FlockDanger,
            "physics-state-component" => PhysicsState,
            "powerframe-player-selection-data-component" => PowerframeSelection,
            "supply-lines-blocked-status-component" => SupplyBlocked,
            "supply-lines-busy-state-component" => SupplyBusy,
            "managed-object-participant-respawn-block-component" => RespawnBlock,
            "tacmap-iconlodthresholds" => TacmapLod,
            "tacmap-dungeonstate" => TacmapDungeon,
            "tacmap-lockedlights" => TacmapLights,
            "tacmap-waypointstate" => TacmapWaypoint,
            "tacmap-queuedreplaymission" => TacmapQueued,
            "tacmap-mapdismissallock" => TacmapPlayers,
            "tacmap-playercontrol" => TacmapPlayers,
            "tacmap-cooptetherarea" => TacmapTether,
            "tacmap-backmenu-openoverride" => TacmapOverride,
            "tacmap-tooltip-state" => TacmapTooltip,
            "matchflow-sequence-data-component" => MatchflowSequence,
            "matchflow-focus-data-component" => MatchflowFocus,
            "managed-navpoint-visual-state-groups-component-0" => NavpointStateGroup,
            "managed-navpoint-visual-state-groups-component-1" => NavpointStateGroup,
            "managed-navpoint-visual-state-groups-component-2" => NavpointStateGroup,
            "managed-navpoint-visual-state-groups-component-3" => NavpointStateGroup,
            "managed-navpoint-visual-state-groups-component-4" => NavpointStateGroup,
            "managed-navpoint-visual-state-groups-component-5" => NavpointStateGroup,
            "managed-navpoint-visual-state-groups-component-6" => NavpointStateGroup,
            "managed-navpoint-visual-state-groups-component-7" => NavpointStateGroup,
            _ => return None,
        })
    }
}

/// The box of a position in the game's default box: three
/// 22-bit fields, in no box of the level.
pub const DEFAULT_BOX: u32 = 0xffff;
/// Bits of each field of a position relative to a parent: a box of plus or
/// minus 100 units at the precision's step, 1/120 of a unit.
const ATTACHED_BITS: u32 = 14;

/// Bits between one update's last data block and the next update.
pub const SEPARATOR: i64 = 3;

/// One archetype's components by index, from the chunk-0 registry, and the
/// game's object type for it (two readers branch on it: biped 0x23,
/// vehicle 0x28).
#[derive(Clone, Debug)]
pub struct Table(Vec<Option<Component>>, u32);

impl Table {
    /// The table of the registry block with a row name starting `marker`
    /// ("biped-", "vehicle-", as `Schema::read` finds them); None if absent.
    pub fn read(chunk0: &[u8], marker: &str) -> Option<Table> {
        let blocks = registry::blocks(chunk0);
        let b = blocks.iter().find(|b| b.iter().any(|n| n.starts_with(marker)))?;
        let object_type = match marker {
            "biped-" => 0x23,
            "vehicle-" => 0x28,
            _ => 0,
        };
        Some(Table(b.iter().map(|n| Component::from_name(n)).collect(), object_type))
    }

    /// The table of registry block `entity_type` (its rows' names), for
    /// the entity type an entity-create names.
    pub fn from_rows(rows: &[String], entity_type: u32) -> Table {
        Table(rows.iter().map(|n| Component::from_name(n)).collect(), entity_type)
    }

    pub fn get(&self, index: u32) -> Option<Component> {
        self.0.get(index as usize).copied().flatten()
    }
}

/// Widths the game's readers take from run-time state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    /// Position fields c1, c2, Z: the level's widths.
    pub widths: [u32; 3],
    /// The version argument several readers compare (`> 1`, `> 2`, `> 3`).
    pub version: u32,
    /// Bits of an object handle's index (`[present:1]`, then index and a
    /// 2-bit generation): the entity index's 13 (`events::INDEX_BITS`).
    pub handle_bits: u32,
    /// Whether the vehicle orientation block has its extra precision bit
    /// (it does: without it vehicle updates don't parse to the next one).
    pub vehicle_hi_bit: bool,
    /// Bits of a position's box index: the level's `box_bits` (1 unless it
    /// lists more than two structure BSPs; Live Fire: 2).
    pub box_bits: u32,
    /// Reading a packet-2 snapshot: the game's position readers take raw
    /// floats there, and two components have their own readers.
    pub snapshot: bool,
    /// In a snapshot: this vehicle has its type state and physics.
    pub vehicle_type: bool,
    /// Widths of positions in another box than the play space's, by box
    /// index; all zero where the play space's widths apply.
    pub box_widths: [[u32; 3]; 4],
    /// The play space's box index: positions in it convert with the level's
    /// box; others are in another BSP's.
    pub play_box: u32,
}

impl Params {
    pub fn new(widths: [u32; 3]) -> Params {
        Params {
            widths,
            version: 4,
            handle_bits: crate::events::INDEX_BITS,
            snapshot: false,
            vehicle_hi_bit: true,
            vehicle_type: false,
            box_bits: 1,
            box_widths: [[0; 3]; 4],
            play_box: 0,
        }
    }

    /// The level's box-index width and its other boxes' widths.
    pub fn set_level(&mut self, level: Option<&crate::levels::LevelBox>) {
        self.box_bits = level.map_or(1, |l| l.box_bits);
        self.play_box = level.map_or(0, |l| l.box_index);
        self.box_widths = [[0; 3]; 4];
        for o in level.map_or(&[][..], |l| &l.others[..]) {
            if let Some(e) = self.box_widths.get_mut(o.index as usize) {
                *e = o.widths;
            }
        }
    }
}

/// A quantized float as the game's reader decodes it: `bits` bits over
/// [min, max]. `odd` uses 2^bits - 1 codes so that the midpoint is exact;
/// `exact` makes the first and last code the ends of the range.
pub fn dequantize(q: u32, bits: u32, min: f64, max: f64, odd: bool, exact: bool) -> f64 {
    let codes = (1u64 << bits) as f64 - if odd { 1.0 } else { 0.0 };
    if odd && 2.0 * q as f64 == codes - 1.0 {
        return (min + max) / 2.0;
    }
    if exact {
        if q == 0 {
            return min;
        }
        if q as f64 == codes - 1.0 {
            return max;
        }
        let step = (max - min) / (codes - 2.0);
        return min + (q as f64 - 1.0) * step + 0.5 * step;
    }
    let step = (max - min) / codes;
    min + q as f64 * step + 0.5 * step
}

/// A 19-bit direction code as a unit vector (engine x, y, z): a cube map,
/// `face * 87381 + i * 294 + j`. Face 0-2 is +x, +y, +z, 3-5 the same axes
/// negative; i and j index a 293-cell grid over -1 to 1 on the face's other
/// two axes (in axis order), the middle cell exactly 0. 87,381 is
/// `floor(2^19 / 6)`; the side, 294, is a run-time value, the one with which
/// the velocity direction follows the movement heading and no code falls off
/// the grid.
pub fn direction(code: u32) -> [f64; 3] {
    const PER_FACE: u32 = 87381;
    const SIDE: u32 = 294;
    let rem = code % PER_FACE;
    let step = 2.0 / (SIDE - 1) as f64;
    let grid = |k: u32| if 2 * k == SIDE - 2 { 0.0 } else { (k as f64 + 0.5) * step - 1.0 };
    let (u, v) = (grid(rem / SIDE), grid(rem % SIDE));
    let d = match code / PER_FACE {
        0 => [1.0, u, v],
        1 => [u, 1.0, v],
        2 => [u, v, 1.0],
        3 => [-1.0, u, v],
        4 => [u, -1.0, v],
        _ => [u, v, -1.0],
    };
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    [d[0] / l, d[1] / l, d[2] / l]
}

/// Speed (engine units per second) from the velocity block's 10-bit
/// magnitude code: logarithmic over [0.03, 350], the first and last code
/// exact, every other code the middle of its step.
pub fn speed(q: u32) -> f64 {
    const MIN: f64 = 0.03;
    const MAX: f64 = 350.0;
    if q == 0 {
        return MIN;
    }
    if q >= 1023 {
        return MAX;
    }
    let shift = 1.0 - MIN;
    ((q as f64 + 0.5) * (shift + MAX).ln() / 1024.0).exp() - shift
}

/// A velocity block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity {
    /// Engine units per second.
    pub speed: f64,
    /// The 19-bit direction code (a cube-map unit vector: `direction`)
    /// of the usual form; None for zero velocity and for the raw-float form.
    pub direction: Option<u32>,
}

/// What the known components of one update carry. Raw codes are kept next
/// to the converted values where the conversion isn't checked yet.
#[derive(Clone, Debug, Default)]
pub struct Fields {
    /// A non-object update's blocks: (row, bit). Objects' aren't kept.
    pub blocks: Vec<(u32, i64)>,
    /// Position block box index (0 = the level's first BSP) and the three
    /// N-bit codes; absolute positions only.
    pub position: Option<(u32, [u32; 3])>,
    pub velocity: Option<Velocity>,
    /// Body vitality, -1 to 1 (health fraction; 0 or less is dead).
    pub health: Option<f64>,
    /// Shield vitality, 0 to 4 (fraction of the base shield; above 1 an overshield),
    /// and the 16-bit field after it.
    pub shield: Option<(f64, u32)>,
    /// From the maximum vitalities: whether the shield's maximum is zero.
    pub shieldless: Option<bool>,
    /// Aim yaw (0 to 2 pi) and pitch (-pi to pi), radians: the first vector.
    pub aim: Option<(f64, f64)>,
    /// The command tick, an 8-bit counter that steps once per game tick.
    pub command_tick: Option<u32>,
    /// Crouch amount, 0 to 1.
    pub crouch: Option<f64>,
    /// Active camo: the 3-bit state.
    pub camo: Option<u32>,
    /// A unit's weapon slots (weapon-state-type-info, in table order):
    /// Some(Some((tag id, variant string id))) holding that weapon,
    /// Some(None) empty, None not in this update.
    pub weapons: [Option<Option<(u32, u32)>>; 4],
    /// biped-desired-weapon-set: its first 3 bits.
    pub weapon_set: Option<u32>,
    /// biped-desired-weapon-set: its two optional 2-bit weapon slots.
    pub weapon_slots: Option<(Option<u32>, Option<u32>)>,
    /// biped-spartan-ability: its 2-bit state. 1 is a thruster burst (it
    /// lasts 24 frames, and players holding a Thruster enter it), 2 the
    /// sprint (on-foot speed at the sprint ceiling), 3 a grapple with its
    /// target.
    pub ability: Option<u32>,
    /// unit-control: Some(Some(id)) a player controls the unit, the engine
    /// player id of the events and the scoreboard; Some(None) no player does.
    pub controller: Option<Option<u32>>,
    /// Object parent state: Some(Some(parent)) attached to `parent`,
    /// Some(None) detached (a dropped or placed item).
    pub parent: Option<Option<Parent>>,
    /// With an attached `parent`: its handle's 2-bit generation (a vehicle
    /// parent's id is `(generation - 1) * 256 + index - 256`) and the
    /// parent's node it hangs from (the object's node index, `142f0b490`).
    pub parent_generation: Option<u32>,
    pub parent_node: Option<u32>,
    /// The unit's seat in its parent (the index `vehicle_test_seat_unit`
    /// compares, `1431aef58`), when parent state is listed: Some(Some(seat))
    /// seated, Some(None) not.
    pub seat: Option<Option<u32>>,
    /// A unit's weapon slots' state, by slot as `weapons` (the `n`th row of
    /// each of weapon-state-ammo, -rounds-inventory and -overheated): the
    /// rounds loaded (0 when the reader gets none) and a 0 to 1 fraction the
    /// reader takes for some weapons,
    pub loaded: [Option<(u32, Option<f64>)>; 4],
    /// the rounds in reserve,
    pub reserve: [Option<u32>; 4],
    /// and the heat, 0 to 1, with the two flags after it (bits 1 and 2 of
    /// the slot's flags byte), the first in the high bit.
    pub heat: [Option<(f64, u32)>; 4],
    /// unit-grenade-counts: the count of each grenade type, in the order of
    /// the throws' 3-bit types less one (frag, plasma, dynamo, spike).
    pub grenades: Option<[u32; 4]>,
    /// An equipment object's energy, 0 to 1, the ticks before it recharges,
    /// its charges and the player that made it (Some(None): none).
    pub energy: Option<f64>,
    pub recharge: Option<u32>,
    pub charges: Option<u32>,
    pub creator: Option<Option<u32>>,
}

/// The object an object is attached to (a kind-1 handle): a unit (biped or
/// vehicle) by its 9-bit unit index, or any object by its 13-bit index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parent {
    Unit(u32),
    Object(u32),
}

/// One update's data blocks as far as they could be read.
#[derive(Clone, Debug, Default)]
pub struct Parsed {
    /// The fields of the blocks read.
    pub fields: Fields,
    /// The bit after the last block; None when a block could not be read (an
    /// unknown component, or a branch whose layout isn't known), so the
    /// fields hold only the blocks before it.
    pub end: Option<i64>,
    /// The component whose block could not be read, when `end` is None.
    pub failed: Option<u32>,
    /// Where each listed component's block starts.
    pub starts: Vec<(u32, i64)>,
}

/// Read the data blocks of an update whose `list` (component indices)
/// starts at absolute bit `at` of `c`, in list order until one can't be read.
pub fn parse(c: &[u8], at: i64, list: &[u32], table: &Table, p: &Params) -> Parsed {
    parse_versions(c, at, list, table, &[], p)
}

/// The bit after an action body of `variant` at `at` (the biped's action
/// list, and NetworkedActionRequest, event 102); None when its layout
/// isn't known.
pub fn action_end(c: &[u8], at: i64, variant: u32, p: &Params) -> Option<i64> {
    let mut r = Reader { c, at, object_type: 0, slot: 0, row_version: 0 };
    r.action(variant, p)?;
    Some(r.at)
}

/// One player's message in the control view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlMessage {
    pub player: u32,
    /// The control block's two 6-bit throttle values, 0-62 with 31 the centre.
    pub throttle: Option<[u32; 2]>,
    /// The unit block's first field: which of its parts follow.
    pub state: Option<u32>,
    /// A kind-1 message's body.
    pub command_at: Option<i64>,
    /// The control block's raw fields, for naming: `[a, value]` (2 bits),
    /// two optional floats, an optional 5-bit value.
    pub head: [Option<u32>; 4],
    /// The firing plan when present: three groups of flags, 3, then the two
    /// optional 2-bit values.
    pub plan: Option<[u32; 6]>,
}

/// The control view at bit `at` of `c`: its messages, and the
/// bit after its last `[more:1]`. None at a kind whose layout isn't read.
pub fn control_view(c: &[u8], at: i64, p: &Params) -> (Vec<ControlMessage>, Option<i64>) {
    let mut r = Reader { c, at, object_type: 0, slot: 0, row_version: 0 };
    let mut out = Vec::new();
    while r.flag() {
        let kind = r.bits(2);
        // `[set:1]`, 7 bits: the message this one is relative to.
        let baseline = r.opt(7).is_some();
        // Kind 2.
        if kind == 2 {
            r.opt(5 + 8);
            continue;
        }
        if kind == 3 {
            return (out, None);
        }
        let player = r.bits(5);
        let mut m = ControlMessage { player, ..Default::default() };
        // Kind 1: `[set:1][n:2]` and n command records.
        if kind == 1 {
            m.command_at = Some(r.at);
            if r.flag() {
                let mut relative = baseline;
                for _ in 0..r.bits(2) {
                    r.command_record(relative, p);
                    relative = true;
                }
            }
            out.push(m);
            continue;
        }
        if r.flag() && r.control_block(p, &mut m).is_none() {
            return (out, None);
        }
        if r.flag() {
            r.unit_block(p, &mut m);
        }
        out.push(m);
    }
    (out, Some(r.at))
}

/// `parse` with the registry's per-row versions, which a snapshot passes to
/// each component's reader. A component that compares its row's
/// version reads 0 when `versions` is empty.
pub fn parse_versions(c: &[u8], at: i64, list: &[u32], table: &Table, versions: &[u32], p: &Params) -> Parsed {
    let mut r = Reader { c, at, object_type: table.1, slot: 0, row_version: 0 };
    let mut fields = Fields::default();
    let mut starts = Vec::with_capacity(list.len());
    for &i in list {
        starts.push((i, r.at));
        r.row_version = versions.get(i as usize).copied().unwrap_or(0);
        let mut row = *p;
        if !versions.is_empty() {
            row.version = r.row_version;
        }
        // A unit's weapon slot: the component's place among the rows of the
        // same component (weapon-state-type-info, -ammo, ...).
        let comp = table.get(i);
        r.slot = table.0[..(i as usize).min(table.0.len())].iter().filter(|c| **c == comp).count();
        if table.get(i).and_then(|comp| r.block(comp, &row, &mut fields)).is_none() {
            return Parsed { fields, end: None, failed: Some(i), starts };
        }
    }
    Parsed { fields, end: Some(r.at), failed: None, starts }
}

struct Reader<'a> {
    c: &'a [u8],
    at: i64,
    /// The game's object type of the archetype being read (`Table`).
    object_type: u32,
    /// The weapon slot of the component being read (`parse`).
    slot: usize,
    /// The version of the component's registry row.
    row_version: u32,
}

impl Reader<'_> {
    fn bits(&mut self, w: u32) -> u32 {
        let v = bits_at(self.c, self.at, w.min(32));
        self.at += w as i64;
        v
    }
    fn flag(&mut self) -> bool {
        self.bits(1) == 1
    }
    fn skip(&mut self, w: u32) {
        self.at += w as i64;
    }
    /// `[set:1]`, then `w` bits when set.
    fn opt(&mut self, w: u32) -> Option<u32> {
        self.flag().then(|| self.bits(w))
    }
    /// `[none:1]`, then `w` bits when not none.
    fn unless(&mut self, w: u32) -> Option<u32> {
        (!self.flag()).then(|| self.bits(w))
    }
    /// An object handle: `[present:1]`, then index and generation.
    fn handle(&mut self, p: &Params) {
        if self.flag() {
            self.skip(p.handle_bits + 2);
        }
    }
    /// A kind-1 object handle wrapped in `[present:1]` (actor control's
    /// targets).
    fn handle_kind1(&mut self) {
        if self.flag() {
            self.kind1();
        }
    }
    /// `[present:1]` and a handle of `kind`.
    fn handle_of(&mut self, kind: u32, p: &Params) {
        match kind {
            0 => self.handle(p),
            1 => self.handle_kind1(),
            2 | 3 | 5 => self.handle5(),
            4 | 6 => self.handle_units(),
            _ => {
                if self.flag() {
                    self.skip(crate::events::INDEX_BITS + 2);
                }
            }
        }
    }
    /// A filter: `[mask:4]`, 1 or 32 bits, and a condition for
    /// each bit of the mask. None on a condition of a kind that carries data.
    fn filter(&mut self, narrow: bool, p: &Params) -> Option<u32> {
        let mask = self.bits(4);
        self.skip(if narrow { 1 } else { 32 });
        for _ in 0..mask.count_ones() {
            // `[kind:4]`, then `[negate:1]` and the kind's body.
            let kind = self.bits(4);
            if kind == 0 {
                continue;
            }
            self.skip(1);
            match kind {
                1 => self.skip(1),
                2 | 9 => self.skip(32),
                3 => self.skip(4),
                4 => self.skip(9),
                5 => {
                    for _ in 0..self.bits(3) {
                        self.unless(13);
                    }
                }
                6 => {
                    for _ in 0..self.bits(4) {
                        self.handle_of(0, p);
                    }
                }
                7 => self.skip(4),
                8 => self.skip(8),
                10 => self.skip(4 + 4 + 32),
                11 => {
                    self.unless(5);
                }
                _ => return None,
            }
        }
        Some(mask)
    }
    /// A networked property: `[kind:4]` and its value. Kinds 1
    /// to 6 hold a value on a shared property, the rest one on a per-player
    /// one (`masked`); either way the other reads nothing.
    fn property_value(&mut self, masked: bool) {
        let kind = self.bits(4);
        if kind != 0 && (kind < 7) != masked {
            self.skip(creation::property_bits(kind));
        }
    }
    /// A firing plan's target: `[set:1]`, then an object or
    /// a place, its offset and a vector.
    fn firing_target(&mut self, p: &Params) {
        if !self.flag() {
            return;
        }
        match self.bits(2) {
            1 => {
                self.kind1();
                self.opt(6);
            }
            2 => self.bare_handle(2, p),
            _ => {}
        }
        if !self.flag() {
            self.skip(4 + 4);
            if !self.flag() {
                return;
            }
        }
        self.vec(15, 7);
    }
    /// An AI's firing plan after its `[set:1]`.
    fn firing_plan(&mut self, p: &Params) -> Option<()> {
        self.firing_plan_fields(p).map(|_| ())
    }
    fn firing_plan_fields(&mut self, p: &Params) -> Option<[u32; 6]> {
        let (mut first, mut second) = (0, 0);
        let mut out = [0; 6];
        if self.flag() {
            first = self.bits(3);
            second = self.bits(3);
            out[0] = first;
            out[1] = second;
        }
        if self.flag() {
            let (a, b) = (self.bits(2), self.bits(2));
            out[2] = a;
            out[3] = b;
            first |= a;
            second |= b;
        }
        if self.flag() {
            // Two flags, then two optional values (8 and 10 bits), each read
            // when its flag is set.
            self.skip(2);
            self.opt(8);
            self.opt(10);
            match self.bits(2) {
                0 => {
                    self.point(p)?;
                }
                1 => self.skip(19),
                _ => {}
            }
        }
        out[4] = self.bits(3);
        if first != 0 {
            self.unless(2);
        }
        if second != 0 {
            self.unless(2);
        }
        out[5] = self.flag() as u32;
        self.at -= 1;
        self.firing_target(p);
        Some(out)
    }
    /// A handle of `kind` after its own presence bit.
    fn bare_handle(&mut self, kind: u32, p: &Params) {
        let index = match kind {
            0 => p.handle_bits,
            2 | 3 | 5 => 8,
            4 | 6 => 9,
            _ => crate::events::INDEX_BITS,
        };
        self.skip(index + 2);
    }
    /// A delta-coded float: `[lead:5]`, the bits that differ.
    fn delta_float(&mut self) {
        let lead = self.bits(5);
        self.skip(32 - lead);
    }
    /// A kind-1 command record, `relative` to the one before.
    fn command_record(&mut self, relative: bool, p: &Params) {
        if !relative {
            self.skip(32 + 7);
        } else {
            if self.flag() {
                self.opt(6);
            } else {
                self.skip(32);
            }
            self.opt(7);
        }
        if self.flag() {
            return;
        }
        if self.flag() {
            self.bare_handle(0, p);
            self.unless(10);
        }
        if self.flag() {
            self.bare_handle(4, p);
        }
        if relative {
            for _ in 0..5 {
                if self.flag() {
                    for _ in 0..3 {
                        self.delta_float();
                    }
                }
            }
        } else {
            self.skip(5 * 96);
        }
        if self.opt(8).is_some() {
            self.skip(4);
        }
        self.skip(1);
        if self.flag() && self.flag() {
            self.bare_handle(2, p);
        }
    }
    /// The control view's first block.
    fn control_block(&mut self, p: &Params, m: &mut ControlMessage) -> Option<()> {
        m.head[0] = self.opt(2);
        m.throttle = Some([self.bits(6), self.bits(6)]);
        m.head[1] = self.opt(5);
        m.head[2] = self.opt(6);
        m.head[3] = self.opt(5);
        if self.flag() {
            m.plan = Some(self.firing_plan_fields(p)?);
        }
        Some(())
    }
    /// The control view's second block.
    fn unit_block(&mut self, p: &Params, m: &mut ControlMessage) {
        let state = self.bits(5);
        m.state = Some(state);
        self.skip(2 + 17 + 16);
        self.handle_of(1, p);
        if state & 2 == 0 {
            return;
        }
        self.handle_of(0, p);
        self.skip(2 + 3);
        for _ in 0..self.bits(3) {
            self.handle_of(0, p);
            self.skip(4 + 4);
        }
        self.skip(5);
        self.skip(3);
        self.unless(2);
        self.unless(2);
        self.skip(6 + 3 + 3);
        self.unless(6);
        if self.flag() {
            self.skip(0);
        }
        let _ = self.point(p);
        self.orient();
        self.vec(19, 10);
        self.vec(19, 8);
        let target = self.flag();
        if target {
            self.skip(p.handle_bits + 2);
        } else {
            self.skip(11);
        }
        self.skip(7);
    }
    /// A statistic: `[size:2]`, then `8 << size` bits.
    fn stat_value(&mut self) {
        let size = self.bits(2);
        self.skip(8 << size);
    }
    /// A formatted string: `[set:1]`, a string id and up to
    /// seven parameters.
    fn text(&mut self) {
        if self.flag() {
            self.skip(32);
            for _ in 0..self.bits(3) {
                self.text_parameter();
            }
        }
    }
    /// `[kind:3]`: 0 nothing, 1 a player, 2 `[real:1]` a value,
    /// 3 a string id, else 32 bits.
    fn text_parameter(&mut self) {
        match self.bits(3) {
            0 => {}
            1 => {
                self.unless(5);
            }
            2 => {
                let real = self.flag();
                self.skip(if real { 24 } else { 32 });
            }
            _ => self.skip(32),
        }
    }
    /// A unit handle (kind 4: 9-bit index) wrapped in `[present:1]`.
    fn handle_units(&mut self) {
        if self.flag() {
            self.skip(9 + 2);
        }
    }
    /// A kind-1 handle: `[units:1]`, then a 9-bit index into the unit range
    /// (bipeds and vehicles), or when 0 a 13-bit entity index, and the 2-bit
    /// generation.
    fn kind1(&mut self) {
        let w = if self.flag() { 9 } else { crate::events::INDEX_BITS };
        self.skip(w + 2);
    }
    /// A position: `[nobox:1]` (1: the game's default box, three 22-bit
    /// fields), then the box index (`p.box_bits` wide: 1 unless the level
    /// lists more than two structure BSPs) and three fields at the box's
    /// widths. The index counts the level's BSP list (Live Fire's play space
    /// is element 1).
    fn point(&mut self, p: &Params) -> Option<(u32, [u32; 3])> {
        if p.snapshot {
            self.skip(96);
            return Some((0, [0; 3]));
        }
        if self.flag() {
            return Some((DEFAULT_BOX, [self.bits(22), self.bits(22), self.bits(22)]));
        }
        let b = self.bits(p.box_bits);
        let w = match p.box_widths.get(b as usize) {
            Some(&x) if x[0] > 0 => x,
            _ => p.widths,
        };
        Some((b, [self.bits(w[0]), self.bits(w[1]), self.bits(w[2])]))
    }
    /// `[zero:1]`, else a `dir`-bit direction and a `mag`-bit magnitude.
    fn vec(&mut self, dir: u32, mag: u32) {
        if !self.flag() {
            self.skip(dir + mag);
        }
    }
    /// A kind-5 handle (8-bit index) wrapped in `[present:1]`.
    fn handle5(&mut self) {
        if self.flag() {
            self.skip(8 + 2);
        }
    }
    /// A target place: a position, or (`attached`) a 2-bit box of the
    /// game's own, three 13-bit fields and an optional 16-bit value.
    fn place(&mut self, p: &Params, attached: bool) -> Option<()> {
        if attached {
            self.skip(2 + 3 * 13);
            self.opt(16);
        } else {
            self.point(p)?;
        }
        Some(())
    }
    /// An orientation: `[default:1]` (else a 19-bit forward) and an 8-bit roll.
    fn orient(&mut self) {
        if !self.flag() {
            self.skip(19);
        }
        self.skip(8);
    }
    /// `[id:1]` (a 32-bit configuration id, a 6-bit count n - 1 and n flags),
    /// then 3 times (`[none:1]` else 6 bits, two optional 12-bit floats).
    fn frame_configuration(&mut self) {
        if self.opt(32).is_some() {
            let n = self.bits(6) + 1;
            self.skip(n);
        }
        for _ in 0..3 {
            self.unless(6);
            self.opt(12);
            self.opt(12);
        }
    }
    /// A direction-and-magnitude vector: `[zero:1]`, else a 19-bit
    /// direction and a `mag`-bit magnitude.
    fn vector(&mut self, mag: u32) {
        if !self.flag() {
            self.skip(19 + mag);
        }
    }

    /// The body of one action of the biped's action list, by its 5-bit
    /// variant (0-25); None for a variant whose body isn't known.
    fn action(&mut self, variant: u32, p: &Params) -> Option<()> {
        match variant {
            0 => {
                self.handle(p);
                self.opt(8);
            }
            1 => {
                self.opt(8);
                self.skip(8 + 8 + 32 + 15);
            }
            2 => {
                self.skip(8);
                self.opt(8);
                self.skip(16);
            }
            3 => self.skip(2 + 15),
            4 => {
                self.skip(32 + 8 + 15);
                self.vec(19, 10);
                self.opt(8);
                self.skip(16);
            }
            5 => {
                self.skip(32);
                self.opt(8);
                self.skip(16 + 8);
            }
            6 => {
                self.opt(8);
                self.skip(32);
            }
            7 => {
                self.opt(8);
                self.skip(3 + 15);
            }
            8..=10 => {
                if self.flag() {
                    self.skip(19);
                }
                self.skip(32 + 4);
                if variant == 8 {
                    self.skip(4 + 1);
                }
            }
            11 => {
                self.opt(8);
                self.skip(19 + 2);
            }
            12 => self.skip(32 + 1),
            13 => {
                self.skip(96 + 19);
                self.opt(8);
            }
            14 => self.skip(8),
            15 | 22 => {
                self.opt(8);
            }
            16 => self.vec(19, 10),
            17 => {
                self.skip(15 + 15 + 1);
                self.opt(8);
            }
            18 => {
                if self.flag() {
                    return None;
                }
                self.skip(2 + 32 + 1 + 16 + 8);
                self.opt(8);
            }
            19 => {
                self.skip(32 + 8);
                self.opt(8);
            }
            20 => self.skip(15),
            21 => {
                self.opt(8);
                self.skip(32 + 4);
            }
            23 => {
                self.opt(8);
                self.skip(32 + 1);
            }
            24 => {
                self.opt(8);
                self.skip(32 + 2);
            }
            25 => {
                self.skip(32 + 16 + 32 + 1 + 8);
                self.opt(8);
            }
            _ => return None,
        }
        Some(())
    }

    /// One component's block; None when its layout (or this branch of it)
    /// isn't known.
    fn block(&mut self, comp: Component, p: &Params, f: &mut Fields) -> Option<()> {
        use Component::*;
        match comp {
            // The snapshot's own reader: `[a][b]`, 0 and 0 then
            // `[c]` and the raw floats, and the 2 bits every position ends on.
            Position if p.snapshot => {
                if self.bits(2) != 0 {
                    return None;
                }
                self.skip(1 + 96 + 2);
            }
            Position => {
                // [A][B][C][D]: 0000 = absolute, a level box, indexed.
                if self.bits(4) != 0 {
                    return None;
                }
                // Fields at the box's own widths, as `point` reads them.
                let bx = self.bits(p.box_bits);
                let w = match p.box_widths.get(bx as usize) {
                    Some(&x) if x[0] > 0 => x,
                    _ => p.widths,
                };
                let q = [self.bits(w[0]), self.bits(w[1]), self.bits(w[2])];
                self.skip(2);
                f.position = Some((bx, q));
            }
            Velocity => {
                f.velocity = Some(if self.flag() {
                    // Three float32, their bytes in stream order, little-endian.
                    let v: Vec<f64> = (0..3).map(|_| f32::from_le_bytes(self.bits(32).to_be_bytes()) as f64).collect();
                    self::Velocity { speed: v.iter().map(|x| x * x).sum::<f64>().sqrt(), direction: None }
                } else if self.flag() {
                    self::Velocity { speed: 0.0, direction: None }
                } else {
                    let direction = Some(self.bits(19));
                    self::Velocity { speed: speed(self.bits(10)), direction }
                });
            }
            // [default:1] (else a 19-bit forward) and an 8-bit roll.
            ForwardAndUp => {
                if !self.flag() {
                    self.skip(19);
                }
                self.skip(8);
            }
            AngularVelocity => self.vector(8),
            // Items, equipment and rigid bodies (classes 4-7).
            ObjectPosition if p.snapshot => {
                // `[attached:1]`, the raw floats, when attached a
                // parent handle and an optional 11 bits, and 2 bits.
                let attached = self.flag();
                self.skip(96);
                if attached {
                    self.handle(p);
                    if self.flag() {
                        self.skip(11);
                    }
                }
                self.skip(2);
            }
            // `[attached:1]`; a world position, or one local to
            // the parent (kept out of `f.position`), its parent handle and an
            // optional 11 bits; then 2 bits.
            ObjectPosition => {
                if self.flag() {
                    self.skip(3 * ATTACHED_BITS);
                    self.handle(p);
                    self.opt(11);
                } else {
                    f.position = Some(self.point(p)?);
                }
                self.skip(2);
            }
            ObjectVelocity => self.vector(10),
            ItemAtRest | EquipmentDeployed | EquipmentInfiniteUses => self.skip(1),
            // `[none:1]`, else a 5-bit player index.
            ItemIgnorePlayer => {
                self.unless(5);
            }
            EquipmentCreator => f.creator = Some(self.unless(5)),
            EquipmentActivated => {
                if self.flag() {
                    self.handle_units();
                } else {
                    self.skip(3);
                }
            }
            EquipmentControlSignal => {
                self.skip(4);
                self.handle_units();
            }
            EquipmentEnergy => f.energy = Some(dequantize(self.bits(14), 14, 0.0, 1.0, false, true)),
            EquipmentBeingHacked => self.skip(8),
            EquipmentCharges => f.charges = Some(self.bits(8)),
            // Ticks, below 0x3fe.
            EquipmentDelayTicks => f.recharge = Some(self.bits(10)),
            EquipmentTrackedHandles => {
                let n = self.bits(4);
                for _ in 0..n {
                    self.handle(p);
                }
            }
            // `[same:1]`, then one or two optional 8-bit ticks.
            EquipmentCommandTick => {
                let n = if self.flag() { 1 } else { 2 };
                for _ in 0..n {
                    self.opt(8);
                }
            }
            // `[mask:8]`, then per set bit an orientation and a position.
            RigidBodyTransforms => {
                let mask = self.bits(8);
                for _ in 0..mask.count_ones() {
                    self.orient();
                    let q = self.point(p)?;
                    f.position.get_or_insert(q);
                }
            }
            AnimatedMesh => self.skip(8 + 1 + 16),
            RegionState => {
                let values = self.flag();
                let n = self.bits(6);
                self.skip(3 * n + if values { 10 * n } else { 0 });
            }
            Constraint => {
                let n = self.bits(5);
                self.skip(2 * n);
            }
            MaximumVitalities => {
                let flags = self.bits(5);
                f.shieldless = Some(false);
                for (bit, tail) in [(4, 1), (8, 1), (0x10, 0)] {
                    if flags & bit != 0 {
                        for k in 0..3 {
                            let v = self.opt(12);
                            if bit == 8 && k == 0 && v == Some(0) {
                                f.shieldless = Some(true);
                            }
                        }
                        self.skip(tail);
                    }
                }
                self.skip(3);
            }
            LowFrequency => {
                if self.bits(2) < 2 {
                    self.skip(15);
                }
                self.skip(10);
                let n = self.bits(6);
                for _ in 0..n {
                    if self.flag() {
                        self.skip(1);
                    } else {
                        self.skip(24);
                    }
                }
                self.skip(7);
                self.unless(4);
                if self.flag() {
                    self.skip(1);
                    self.opt(12);
                    self.skip(1);
                }
                let m = self.bits(3);
                if m & 3 == 3 {
                    self.skip(7);
                    if m & 4 != 0 {
                        self.skip(24);
                    }
                }
                if self.flag() {
                    self.skip(35);
                    self.opt(32);
                    self.skip(14);
                }
            }
            PhysicsFlags => self.skip(5),
            FrameConfiguration => self.frame_configuration(),
            // [count:6] (at most 32), each `[present:1]` (a 7-bit float and
            // 16 bits).
            DamageSections => {
                let n = self.bits(6);
                if n > 32 {
                    return None;
                }
                for _ in 0..n {
                    if self.flag() {
                        self.skip(7 + 16);
                    }
                }
            }
            // `[none:1]`, else a 5-bit type and a value the game reads by
            // reflection: its layout isn't in the reader.
            DevicePosition => {
                self.skip(14 + 1);
            }
            WeaponObjectAmmo => {
                self.skip(8 + 11 + 12);
            }
            SimulationPlayback => {
                if self.flag() {
                    self.skip(4 + 32 + 32);
                }
            }
            TurretTriggers => {
                self.skip(3);
            }
            ProjectileAtRest => {
                self.skip(1);
                self.opt(19);
            }
            TurretAim => {
                self.skip(19);
            }
            VehicleOpenState => {
                self.skip(1 + 8);
            }
            TurretTarget => {
                self.handle_kind1();
            }
            VehicleSentry => {
                self.skip(3 + 1);
            }
            VehicleAutoTurret => {
                self.skip(2);
            }
            VehicleTurretParent => {
                self.handle(p);
            }
            VehicleSeatOverride => {
                self.skip(8 + 8);
            }
            AirDropFlight => {
                self.skip(2 + 14 + 8);
            }
            Warp => {
                let a = self.flag();
                let b = self.flag();
                if a {
                    self.skip(8);
                }
                if b {
                    self.skip(8);
                }
            }
            VehicleLowFrequency => {
                self.unless(5);
            }
            ProjectileTether => {
                self.skip(1);
            }
            ProjectileCommandTick => {
                self.opt(8);
            }
            ProjectileDeceleration => {
                self.skip(1);
            }
            DeviceAnimationName => {
                self.skip(32 + 10);
            }
            DeviceAnimationControl => {
                self.skip(256);
            }
            DevicePositionGroup => {
                self.skip(32);
                self.opt(8);
            }
            DevicePower => {
                self.skip(14);
            }
            DeviceInteraction => {
                self.handle_kind1();
            }
            DeviceTime8 => {
                self.skip(8);
            }
            DeviceActionStrings => {
                self.skip(64);
            }
            DeviceHealthCharges => {
                self.skip(1 + 6);
            }
            DeviceBit => {
                self.skip(1);
            }
            DispenserMonitors => {
                let n = self.bits(8);
                if n >= 9 {
                    return None;
                }
                for _ in 0..n {
                    self.handle_kind1();
                }
            }
            DispenserFlags => {
                self.skip(5);
            }
            AnimationLayerSettings => {
                if self.flag() {
                    for _ in 0..8 {
                        if self.opt(32).is_some() {
                            self.skip(14 + 14 + 10 + 14 + 2);
                        }
                    }
                }
            }
            AnimationLayerState => {
                if self.flag() {
                    for _ in 0..8 {
                        if self.flag() && self.bits(10) != 0 {
                            self.skip(14);
                        }
                    }
                }
            }
            DispenserState => {
                for _ in 0..2 {
                    if self.flag() {
                        self.handle(p);
                    }
                    self.skip(3);
                }
            }
            DispenserTimer => {
                for _ in 0..2 {
                    if self.flag() {
                        self.skip(10 + 10 + 5 + 10);
                    }
                }
            }
            DeviceTransitionVelocity => {
                self.skip(18);
            }
            DeviceMachineFlags => {
                self.skip(9);
            }
            // Read when the vehicle's state has this on: `[state:2]`, 6 more
            // bits on 1 and 3.
            // An update lists it only then; a snapshot says nothing
            // (`Params::vehicle_type`, set by `snapshot::walk`).
            VehicleTypeState => {
                if (!p.snapshot || p.vehicle_type) && matches!(self.bits(2), 1 | 3) {
                    self.skip(6);
                }
            }
            // On the same gate: `[raw:1]`, 3 times 96 raw bits; else
            // forward and up and an angular velocity, in a snapshot too.
            VehicleTypePhysics => {
                if !p.snapshot || p.vehicle_type {
                    if self.flag() {
                        self.skip(3 * 96);
                    } else {
                        self.orient();
                        self.vector(8);
                    }
                }
            }
            Skip1 => {
                self.skip(1);
            }
            Skip2 => {
                self.skip(2);
            }
            Skip3 => {
                self.skip(3);
            }
            Skip4 => {
                self.skip(4);
            }
            Skip5 => {
                self.skip(5);
            }
            Skip6 => {
                self.skip(6);
            }
            Skip7 => {
                self.skip(7);
            }
            Skip8 => {
                self.skip(8);
            }
            Skip10 => {
                self.skip(10);
            }
            Skip15 => {
                self.skip(5 + 5 + 5);
            }
            Skip16 => {
                self.skip(16);
            }
            Skip19 => {
                self.skip(19);
            }
            Skip21 => {
                self.skip(1 + 10 + 10);
            }
            Skip24 => {
                self.skip(24);
            }
            Skip32 => {
                self.skip(32);
            }
            Skip40 => {
                self.skip(32 + 8);
            }
            Skip64 => {
                self.skip(64);
            }
            Skip72 => {
                self.skip(6 * 12);
            }
            PlayerDesiredSeat => {
                self.handle_of(3, p);
                self.skip(7);
            }
            PlayerRespawnObject => {
                self.handle_of(8, p);
            }
            PlayerDesiredLocation => {
                if self.flag() {
                    self.point(p)?;
                    self.skip(19);
                }
            }
            PlayerMalleable => {
                self.skip(3);
                for _ in 0..6 {
                    self.opt(12);
                }
                self.skip(9);
            }
            PlayerAimAssist => {
                self.handle_of(1, p);
                self.skip(1);
            }
            PlayerFrameConfiguration => {
                self.frame_configuration();
            }
            StatCurrentRound => {
                self.skip(5 + 5);
                self.stat_value();
                self.stat_value();
                let a = self.flag();
                let b = self.flag();
                if a {
                    self.stat_value();
                }
                if b {
                    self.stat_value();
                }
            }
            StatFinalizedRounds => {
                let mask = self.bits(32);
                for _ in 0..2 * mask.count_ones() {
                    if !self.flag() {
                        self.stat_value();
                    }
                }
            }
            CustomInputPrompt => {
                if self.flag() {
                    self.skip(2);
                    self.text();
                }
            }
            CrewOrder => {
                self.skip(3);
                if self.flag() {
                    self.point(p)?;
                }
            }
            CrewMarkedObjects => {
                self.handle_of(0, p);
            }
            MusicVariables => {
                for _ in 0..32 {
                    if !self.flag() {
                        self.skip(2 + 32 + 32);
                    }
                }
            }
            MusicState => {
                self.skip(1 + 32);
                for _ in 0..2 {
                    self.skip(32);
                    self.opt(32);
                }
                if self.flag() {
                    self.skip(1 + 16 + 16);
                }
                if self.flag() {
                    self.skip(6 * 16);
                }
            }
            SplashStatic => {
                self.text();
                self.skip(24);
                if self.flag() {
                    let kind = self.bits(4);
                    let items = self.bits(3);
                    self.skip(items * (16 + 8 + 8 + 1 + 32));
                    let titles = self.bits(2);
                    for _ in 0..titles {
                        self.skip(32 + 16);
                        if self.flag() {
                            self.text_parameter();
                        }
                    }
                    if titles > 0 {
                        self.skip(8);
                    }
                    self.skip(3);
                    if kind != 0 {
                        self.skip(32);
                    }
                    self.opt(32);
                    self.opt(3 + 6 + 32);
                }
            }
            PersonalAi => {
                if self.bits(32) != u32::MAX && self.flag() {
                    self.skip(12);
                }
            }
            Bits1 => {
                self.skip(1);
            }
            Bits2 => {
                self.skip(2);
            }
            Bits3 => {
                self.skip(3);
            }
            Bits4 => {
                self.skip(4);
            }
            Bits5 => {
                self.skip(5);
            }
            // Keyframing's own reader, of the same name.
            Bits8 if self.object_type == 3 => self.skip(16 + 8 + 2),
            Bits8 => {
                self.skip(8);
            }
            Bits9 => {
                self.skip(9);
            }
            Bits10 => {
                self.skip(10);
            }
            Bits12 => {
                self.skip(12);
            }
            Bits14 => {
                self.skip(14);
            }
            Bits16 => {
                self.skip(16);
            }
            Bits17 => {
                self.skip(17);
            }
            Bits32 => {
                self.skip(32);
            }
            Bits37 => {
                self.skip(37);
            }
            Bits64 => {
                self.skip(64);
            }
            Bits128 => {
                self.skip(128);
            }
            TeamMapping => {
                self.skip(8 + 9 + 9);
                let mask = self.bits(8);
                self.skip(8 + 8);
                self.skip(4 * mask.count_ones());
            }
            CurrentRound => {
                self.unless(5);
            }
            ScreenSequence => {
                self.skip(4 + 8);
            }
            Alliance => {
                let mut set = 0;
                for _ in 0..32 {
                    set += self.bits(1);
                }
                self.skip(set * 64);
            }
            KillVolumes => {
                let n = self.bits(13);
                self.skip(n);
            }
            Letterbox => {
                self.skip(1 + 16);
                for _ in 0..4 {
                    self.unless(7);
                }
                for _ in 0..4 {
                    self.opt(16);
                }
            }
            ManagedTimers => {
                let mask = u64::from(self.bits(32)) << 32 | u64::from(self.bits(32));
                for _ in 0..mask.count_ones() {
                    let kind = self.bits(2);
                    if kind != 0 {
                        self.skip(16 + 16 + 5 + if kind == 1 { 16 } else { 0 });
                    }
                }
            }
            ScenarioIntro => {
                self.skip(7 + 1);
            }
            KeyframeLow => {
                self.point(p)?;
                self.orient();
                self.skip(16 + 8 + 2);
                for _ in 0..self.bits(6) {
                    let f = self.bits(3);
                    if f & 1 != 0 {
                        self.point(p)?;
                    }
                    if f & 2 != 0 {
                        self.orient();
                    }
                    self.skip(16 + 5);
                }
            }
            InteractionFilter => {
                self.filter(self.row_version > 1, p)?;
            }
            Rtpc => {
                if self.bits(32) != 0 {
                    self.skip(22);
                }
            }
            FormattedText => {
                self.text();
            }
            DistanceFilters => {
                let wide = self.row_version > 2;
                let mask = self.filter(wide, p)?;
                self.skip(16 + 16);
                for _ in 0..mask.count_ones() {
                    self.skip(16 + 16);
                    if !wide {
                        self.skip(4);
                    }
                }
                self.skip(mask.count_ones() * (2 + wide as u32));
            }
            BoolFilters => {
                let wide = self.row_version > 1;
                let mask = self.filter(wide, p)?;
                self.skip(1);
                for _ in 0..mask.count_ones() {
                    self.skip(1);
                    if !wide {
                        self.skip(4);
                    }
                }
                self.skip(mask.count_ones() * (2 + wide as u32));
            }
            NavpointText => {
                for _ in 0..self.bits(8) {
                    self.skip(32);
                    self.text();
                }
            }
            NavpointTimers => {
                self.skip(2 * 7);
            }
            NavpointOffset => {
                self.point(p)?;
            }
            NavpointVisualStates => {
                if self.row_version < 2 {
                    let mask = self.bits(8);
                    for _ in 0..mask.count_ones() {
                        self.skip(32);
                        let m = self.filter(false, p)?;
                        self.skip(32);
                        self.skip(m.count_ones() * (32 + 4 + 2));
                    }
                }
            }
            PropertyValue => self.property_value(false),
            MaskedProperty => self.property_value(true),
            CutsceneFlag => {
                if self.flag() {
                    self.unless(16);
                    self.point(p)?;
                    self.skip(32 + 32 + 16 + 8 + 2 + 32);
                }
            }
            EffectState => {
                self.opt(32);
                self.handle_of(0, p);
                self.skip(32);
                if self.flag() {
                    self.handle_of(0, p);
                    self.skip(32);
                }
            }
            SoundPlacement => {
                self.unless(20);
                self.skip(1);
            }
            FlockDestination => {
                self.skip(1);
                self.point(p)?;
                if self.row_version > 1 {
                    self.skip(2);
                }
            }
            FlockDanger => {
                let d = self.bits(3);
                self.skip(8);
                if d != 0 {
                    self.skip(19);
                }
            }
            PhysicsState => {
                self.skip(32);
                self.opt(32);
            }
            PowerframeSelection => {
                for _ in 0..self.bits(6) {
                    self.unless(5);
                    self.skip(256);
                }
            }
            SupplyBlocked => {
                self.skip(2);
            }
            SupplyBusy => {
                let n = self.bits(5) + 1;
                self.skip(n);
            }
            RespawnBlock => {
                self.skip(32);
                self.unless(5);
            }
            TacmapLod => {
                let n = self.bits(5);
                self.skip(n * 12);
            }
            TacmapDungeon => {
                self.skip(1 + 96);
            }
            TacmapLights => {
                self.skip(4 * (32 + 96));
            }
            TacmapWaypoint => {
                self.skip(1 + 32);
                self.point(p)?;
                if self.row_version > 1 {
                    self.skip(1);
                }
            }
            TacmapQueued => {
                for _ in 0..32 {
                    self.unless(5);
                    self.skip(32 + 1);
                }
            }
            TacmapPlayers => {
                for _ in 0..32 {
                    self.unless(5);
                }
            }
            TacmapTether => {
                self.point(p)?;
                self.skip(12 + 12);
            }
            TacmapOverride => {
                for _ in 0..32 {
                    self.unless(5);
                    self.skip(2);
                }
            }
            TacmapTooltip => {
                for _ in 0..32 {
                    self.unless(5);
                    self.skip(1 + 32 + 32);
                }
            }
            MatchflowSequence => {
                self.skip(4 + 4 * 32);
            }
            MatchflowFocus => {
                self.skip(6 + 4);
            }
            // a count, a mask of its items and 32 bits; each
            // item in the mask `[none:1]` else a player, and a flag.
            RadialVisibility => {
                let count = self.bits(32);
                let mask = self.bits(32);
                self.skip(32);
                if count > 32 {
                    return None;
                }
                for _ in 0..(mask & ((1u64 << count) - 1) as u32).count_ones() {
                    self.unless(5);
                    self.skip(1);
                }
            }
            NavpointStateGroup => {
                // `[set:1]`, then a filter in its later form.
                if self.flag() {
                    self.skip(32);
                    let m = self.filter(true, p)?;
                    self.skip(32);
                    self.skip(m.count_ones() * (32 + 3));
                }
            }
            // `[empty:1]`, else a 5-bit variant and, unless it is
            // 0, a Bond CompactBinary v2 struct, which opens with its length.
            MultiplayerProperties => {
                if !self.flag() && self.bits(5) != 0 {
                    // A varint of at most five bytes: 35 bits.
                    let mut len = 0u64;
                    let mut shift = 0;
                    loop {
                        let b = self.bits(8);
                        len |= u64::from(b & 0x7f) << shift;
                        shift += 7;
                        if b & 0x80 == 0 || shift > 28 {
                            break;
                        }
                    }
                    self.at += 8 * len as i64;
                }
            }
            ParentState => {
                if self.flag() {
                    // Attached: the parent (a bare kind-1 handle), the
                    // parent's node it hangs from, an optional second index,
                    // two flags, a local offset (3 times 16 bits), an
                    // orientation and a flag (`142f0b490` gathers them).
                    let unit = self.flag();
                    let index = self.bits(if unit { 9 } else { crate::events::INDEX_BITS });
                    f.parent_generation = Some(self.bits(2));
                    f.parent = Some(Some(if unit { Parent::Unit(index) } else { Parent::Object(index) }));
                    f.parent_node = Some(self.bits(16));
                    self.opt(16);
                    self.skip(2 + 48);
                    self.orient();
                    self.skip(1);
                } else {
                    f.parent = Some(None);
                    if p.version < 2 {
                        self.handle(p);
                        self.opt(11);
                    }
                }
                // The unit's seat in its parent, when seated.
                f.seat = Some(self.opt(6));
                self.skip(1);
                if p.version <= 2 || self.object_type == 0x23 {
                    self.skip(3);
                }
            }
            // `[dead:1]`, then for units a damage report: who and how (ids,
            // enums, a 10-bit direction, the place, two 5-bit amounts).
            DeadState => {
                self.skip(1);
                if self.object_type == 0x23 || self.object_type == 0x28 {
                    self.opt(32);
                    let flags = self.bits(8);
                    self.unless(5);
                    self.unless(5);
                    self.skip(4 + 3);
                    if self.flag() {
                        self.opt(32);
                        self.skip(3);
                        self.unless(6);
                    }
                    self.skip(4 + 4);
                    self.unless(10);
                    self.opt(10);
                    if flags & 0x10 != 0 {
                        self.skip(2 + 3 * 14);
                    }
                    self.skip(5 + 5);
                    self.opt(32);
                }
                if self.object_type == 0x23 {
                    self.skip(1);
                }
            }
            // `[unit:1]`, else a 15-bit scale and `[animated:1]` (a 15-bit
            // target, a 12-bit time and a 5-bit curve).
            Scale => {
                if !self.flag() {
                    self.skip(15);
                    if self.flag() {
                        self.skip(15 + 12 + 5);
                    }
                }
            }
            // A 4-bit type (13: none), else 96 raw bits, a 12-bit float, a flag.
            Dissolver => {
                if self.bits(4) != 13 {
                    self.skip(96 + 12 + 1);
                }
            }
            // `[present:1]`, then two optional 5-bit ids, eight 16-bit floats
            // and two 2-bit fields, an orientation and a position.
            SimulationState => {
                if self.flag() {
                    self.unless(5);
                    self.unless(5);
                    self.skip(4 * 16 + 2 + 2 + 4 * 16);
                    self.orient();
                    self.point(p)?;
                }
            }
            // `[set:1]`: a 32-bit tag id, a 32-bit variant, small enums, a
            // list of ids and a frame configuration; then for every slot an
            // optional byte and the weapon-set fields.
            TypeInfo => {
                let slot = self.slot.min(3);
                f.weapons[slot] = Some(None);
                if self.flag() {
                    let tag = self.bits(32);
                    let variant = self.bits(32);
                    f.weapons[slot] = Some(Some((tag, variant)));
                    self.skip(12 + 7);
                    self.opt(10);
                    self.skip(1);
                    if self.flag() {
                        let n = self.bits(4);
                        for _ in 0..n {
                            self.opt(32);
                        }
                    } else {
                        self.opt(32);
                    }
                    self.frame_configuration();
                    self.unless(5);
                }
                // The block ends with the desired weapon set's fields.
                self.opt(8);
                f.weapon_set = Some(self.bits(3));
                let a = self.unless(2);
                f.weapon_slots = Some((a, self.unless(2)));
            }
            BodyVitality => {
                f.health = Some(dequantize(self.bits(8), 8, -1.0, 1.0, true, true));
                self.skip(3);
            }
            ShieldVitality => {
                let s = dequantize(self.bits(8), 8, 0.0, 4.0, false, true);
                if self.flag() {
                    self.opt(12);
                    self.opt(12);
                }
                let t = self.bits(16);
                self.skip(4);
                f.shield = Some((s, t));
            }
            // The controlling player, when there is one, then two fields not
            // read.
            UnitControl => {
                let mut player = None;
                if self.flag() {
                    player = Some(self.bits(5));
                    self.opt(6);
                }
                self.opt(32);
                f.controller = Some(player);
            }
            // Mode 1 is the whole block. The others: an optional 32-bit
            // value, two small fields, a movement vector, a direction index,
            // flags with an optional 9-bit angle, a control structure (its
            // filled form isn't fully known) and two target handles.
            ActorControl => {
                if self.bits(3) == 1 {
                    return Some(());
                }
                self.opt(32);
                self.skip(1 + 6);
                self.vector(10);
                self.skip(19 + 1);
                self.unless(9);
                if self.flag() {
                    self.firing_plan(p)?;
                }
                for _ in 0..if p.version > 1 { 2 } else { 1 } {
                    self.handle_kind1();
                }
            }
            // Two words, a state index, a 4-bit enum and five target slots.
            // A filled slot: [object:1][full:1], a 32-bit word, then an
            // object handle, a full record (flags, two 10-bit floats, a
            // place, ids, a handle) or a bare place; an 8-bit field unless
            // full; an optional 32-bit word.
            ActorState => {
                self.skip(64 + 12 + 4);
                for _ in 0..5 {
                    if !self.flag() {
                        continue;
                    }
                    let (object, full) = (self.flag(), self.flag());
                    self.skip(32);
                    if object {
                        self.handle(p);
                    } else if full {
                        self.skip(1 + 2 + 10 + 10);
                        self.point(p)?;
                        self.opt(32);
                        self.skip(32);
                        self.handle(p);
                    } else {
                        self.point(p)?;
                    }
                    if !full {
                        self.skip(8);
                    }
                    self.opt(32);
                }
            }
            Aim => {
                let two = !self.flag();
                let yaw = dequantize(self.bits(12), 12, 0.0, TAU, false, false);
                let pitch = dequantize(self.bits(11), 11, -PI, PI, false, false);
                self.skip(1);
                if two {
                    self.skip(24);
                }
                f.aim = Some((yaw, pitch));
            }
            // A count, then a byte per grenade type.
            GrenadeCounts => {
                let mut counts = [0; 4];
                for k in 0..self.bits(3) as usize {
                    let n = self.bits(8);
                    if let Some(c) = counts.get_mut(k) {
                        *c = n;
                    }
                }
                f.grenades = Some(counts);
            }
            UnitMalleable => {
                self.opt(12);
                if p.version > 2 {
                    self.skip(1);
                }
                for _ in 0..7 {
                    self.opt(12);
                }
                self.skip(4 + (p.version > 2) as u32 + (p.version > 3) as u32);
                for _ in 0..4 {
                    self.opt(12);
                }
            }
            UnitLowFrequency => {
                self.skip(1);
                self.handle(p);
                self.skip(4);
                let n = self.bits(4);
                for _ in 0..n {
                    self.handle(p);
                }
                self.skip(2);
            }
            CommandTick => {
                f.command_tick = self.opt(8);
                if self.flag() {
                    // A second form (two tick values and flags) whose length
                    // isn't known: stop rather than guess.
                    return None;
                }
            }
            Equipment => {
                self.skip(3);
                let n = self.bits(3);
                for _ in 0..n {
                    self.handle(p);
                }
            }
            Stun => self.skip(16 + 12 + 12),
            Camo => {
                f.camo = Some(self.bits(3));
                if !self.flag() && !self.flag() {
                    self.skip(12);
                }
                for _ in 0..6 {
                    self.opt(12);
                }
            }
            Crouch => {
                self.skip(1);
                f.crouch = Some(dequantize(self.bits(10), 10, 0.0, 1.0, false, true));
            }
            WeaponAmmo => {
                let rounds = self.unless(8).unwrap_or(0);
                let fraction = self.unless(12).map(|q| dequantize(q, 12, 0.0, 1.0, false, true));
                f.loaded[self.slot.min(3)] = Some((rounds, fraction));
            }
            WeaponRounds => f.reserve[self.slot.min(3)] = Some(self.bits(11)),
            WeaponOverheated => {
                let heat = dequantize(self.bits(7), 7, 0.0, 1.0, false, true);
                f.heat[self.slot.min(3)] = Some((heat, self.bits(2)));
            }
            DesiredWeaponSet | VehicleWeaponSet => {
                f.weapon_set = Some(self.bits(3));
                let a = self.unless(2);
                f.weapon_slots = Some((a, self.unless(2)));
            }
            DesiredGrenadeSet => self.skip(9),
            DesiredAbilitySet => {
                self.skip(3);
                self.unless(6);
            }
            ControlContext => self.skip(3),
            MapEditorFlag | EmpTimer | VehicleEmpTimer => self.skip(8),
            BipedLowFrequency => {
                self.skip(3);
                self.opt(64);
            }
            BipedMalleable => {
                self.opt(8);
                for _ in 0..11 {
                    self.opt(12);
                }
                self.skip(8 + (p.version > 1) as u32);
                let n = self.bits(5);
                self.skip(n);
            }
            // `[target:1][b:1]`; with a target, its handle and the movement:
            // an optional 10-bit index, an optional start (place and
            // orientation), 96 raw bits, a place, four triples of 12-bit
            // fields, two 24-bit directions, two 10-bit floats, small fields.
            MobilityAction => {
                let target = self.flag();
                self.skip(1);
                if target {
                    self.handle(p);
                    self.opt(10);
                    if !self.flag() {
                        self.point(p)?;
                        self.orient();
                    }
                    self.skip(96);
                    self.point(p)?;
                    self.skip(3 * 36 + 2 * 24 + 36 + 2 * 10 + 1 + 7 + 2 + 1);
                }
            }
            // A 2-bit kind, each with its own body of ids, places and
            // 19-bit directions.
            PosturePhysics => match self.bits(2) {
                0 => {
                    if !self.flag() {
                        match self.bits(2) {
                            1 => self.skip(15 + 1 + 1 + 2),
                            0 => {}
                            _ => {
                                self.skip(15 + 2 + 1 + 32);
                                self.opt(19);
                            }
                        }
                    }
                }
                1 => {
                    if self.bits(2) != 0 {
                        self.skip(15 + 2 + 2);
                        self.point(p)?;
                        self.skip(19);
                    }
                }
                2 => {
                    self.point(p)?;
                    self.skip(19);
                    if self.flag() {
                        self.handle(p);
                    } else {
                        self.skip(32);
                    }
                    self.skip(15);
                }
                _ => {
                    self.skip(15);
                    if self.flag() {
                        self.point(p)?;
                        self.skip(19);
                    }
                    self.skip(32 + 1 + 1);
                }
            },
            // A list of up to 15 actions over a 96-bit mask: each a 7-bit
            // type and a 5-bit variant (0-5) with its body; then a 2-bit
            // sub-state for each of the mask's first 73 bits that is set;
            // then 96 bits.
            BipedAction => {
                let w = [self.bits(32), self.bits(32), self.bits(32)];
                let n = self.bits(4);
                for _ in 0..n {
                    self.skip(7);
                    let variant = self.bits(5);
                    self.action(variant, p)?;
                }
                for _ in 0..w[0].count_ones() + w[1].count_ones() + (w[2] & 0x1ff).count_ones() {
                    self.opt(2);
                }
                self.skip(96);
            }
            // a 3-bit mask, then 7 bits for each ability it names
            // (the rest stay 0x7f).
            AbilityEnergy => {
                let mask = self.bits(3);
                self.skip(7 * mask.count_ones());
            }
            // `[state:2]`: 1 carries 2 bits and a 24-bit direction; 3 (when
            // version > 1) a target: an object or a place, and an optional
            // second place.
            Ability => match self.bits(2) {
                1 => {
                    f.ability = Some(1);
                    self.skip(2 + 24)
                }
                3 if p.version > 1 => {
                    f.ability = Some(3);
                    if self.flag() {
                        let flags = self.bits(6);
                        if flags & 1 != 0 {
                            if self.flag() {
                                self.skip(11 + 2);
                                if flags & 0x10 != 0 {
                                    self.place(p, true)?;
                                }
                            } else {
                                self.point(p)?;
                            }
                        }
                    }
                    if self.flag() {
                        self.point(p)?;
                    }
                }
                s => f.ability = Some(s),
            },
            AbilityMalleable => {
                self.opt(12);
                self.opt(8);
                for _ in 0..3 {
                    self.opt(12);
                }
                self.skip(2);
            }
            // `[state:2]`; 3 is an ability in flight: a 3-bit kind, a source
            // handle and its place, 6 bits, and a body per kind (handles,
            // 24-bit directions, vectors with 12-bit magnitudes, places).
            AbilityNonPredicted => {
                if self.bits(2) == 3 {
                    let kind = self.bits(3) + 1;
                    let attached = self.flag();
                    if attached {
                        self.kind1();
                    }
                    self.place(p, attached)?;
                    self.skip(6);
                    match kind {
                        1 => {
                            self.opt(8);
                        }
                        2 => {
                            self.handle5();
                            self.opt(8);
                        }
                        3 => {
                            self.handle(p);
                            self.handle5();
                            for _ in 0..3 {
                                self.vec(24, 12);
                            }
                            self.skip(24 + 9);
                        }
                        4 | 5 => {
                            self.handle5();
                            self.vec(24, 12);
                            self.point(p)?;
                            self.skip(24 + 9);
                        }
                        6 => {
                            if self.opt(8).is_none() {
                                self.handle5();
                            }
                            self.handle(p);
                            self.vec(24, 12);
                            self.vec(24, 12);
                            self.skip(1 + 24);
                        }
                        _ => {}
                    }
                }
                if p.version > 1 {
                    self.skip(3);
                }
            }
            Slide => {
                if self.flag() {
                    self.vector(10);
                    self.skip(8 + if p.version != 0 { 8 } else { 0 } + 8);
                }
            }
            ForwardAndUpDynamic => {
                let raw = self.flag();
                let baseline = !raw && self.flag();
                let high = p.vehicle_hi_bit && self.flag();
                if raw {
                    // Forward and up, three float32 each.
                    self.skip(192);
                    return Some(());
                }
                if high {
                    // A 30-bit direction (unless default) and a 30-bit roll.
                    if !self.flag() {
                        self.skip(30);
                    }
                    self.skip(30);
                    return Some(());
                }
                if baseline {
                    if !self.flag() {
                        if self.flag() {
                            self.skip(8);
                        } else {
                            self.skip(19);
                        }
                    }
                    if self.flag() {
                        self.skip(4);
                    } else {
                        self.skip(8);
                    }
                } else {
                    if !self.flag() {
                        self.skip(19);
                    }
                    self.skip(8);
                }
            }
            AngularVelocityDynamic => {
                if self.flag() {
                    self.skip(96);
                } else {
                    self.vector(8);
                }
            }
        }
        Some(())
    }
}

/// An object update header at absolute bit `bp`: an object index of class
/// 2 or higher (2 bipeds, 3 vehicles, 4-7 other objects), a generation, and
/// 1-7 strictly increasing component indices.
pub fn update_header_at(c: &[u8], bp: i64) -> bool {
    let h = bits_at(c, bp, 18);
    if h >> 15 < 2 || (h >> 5) & 3 == 0 {
        return false;
    }
    let count = (h & 31) as i64;
    (1..=7).contains(&count) && (1..count).all(|m| bits_at(c, bp + 18 + 6 * m, 6) > bits_at(c, bp + 12 + 6 * m, 6))
}

/// The absolute bit where the data of the update `g` (as
/// `for_each_object_update` dispatches it) starts, after a `count`-entry
/// component list.
pub fn data_start(g: &Shifted<'_>, count: usize) -> i64 {
    8 * (g.q as i64 + 5) + g.sh as i64 + 6 * count as i64 - 22
}

/// The biped updates of `film` with their component lists (stubs as
/// `[aim, command tick]`): f(g, frame, slot, list).
fn for_each_biped_update(film: &Film, mut f: impl FnMut(&Shifted<'_>, i64, i32, &[u32]) -> bool) {
    let schema = Schema::read(&film.chunks[0]);
    let stub = [schema.biped.aim, schema.biped.command_tick];
    for_each_object_update(film, |g, _gp, frame, kind, id, off| match kind {
        Kind::Vehicle => true,
        Kind::Stub => f(g, frame, id, &stub),
        Kind::Biped => f(g, frame, id, &list_of(g, off).collect::<Vec<_>>()),
    });
}

/// Z's width when the level catalogue doesn't give it: the width at which
/// the most biped updates parse to an end that the next update follows
/// [`SEPARATOR`] bits later, over the first 3,000 updates that parse to the
/// end. None unless one width wins at least half of them.
pub fn detect_z_width(film: &Film, c1: u32, c2: u32) -> Option<u32> {
    let table = Table::read(&film.chunks[0], "biped-")?;
    const CANDS: std::ops::RangeInclusive<u32> = 8..=24;
    let mut hits = vec![0usize; CANDS.count()];
    let mut n = 0;
    for_each_biped_update(film, |g, _frame, _slot, list| {
        if list.first() != Some(&0) {
            return true;
        }
        let start = data_start(g, list.len());
        let mut any = false;
        for (k, z) in CANDS.enumerate() {
            if let Some(end) = parse(g.c, start, list, &table, &Params::new([c1, c2, z])).end {
                any = true;
                if update_header_at(g.c, end + SEPARATOR) {
                    hits[k] += 1;
                }
            }
        }
        n += any as usize;
        n < 3000
    });
    let (k, best) = hits.iter().enumerate().max_by_key(|(_, h)| **h)?;
    (*best * 2 >= n && n > 0).then(|| *CANDS.start() + k as u32)
}

/// A biped's state from its updates: each value carried forward from the
/// last update that listed its component (an update lists only what changed).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    /// Aim yaw and pitch, radians, engine frame (yaw from engine +X toward +Y).
    pub aim: Option<(f64, f64)>,
    /// Engine units per second.
    pub speed: Option<f64>,
    /// The last position: its box and its fields.
    pub position: Option<(u32, [u32; 3])>,
}

/// Every biped slot's state over the film.
#[derive(Debug)]
pub struct SlotStates(HashMap<i32, Vec<(i64, State)>>, HashMap<i32, Vec<i64>>);

/// A jump: the vertical speed (engine units a second) rises to at least
/// JUMP_UP from under JUMP_FROM between two of a slot's updates. A Spartan's
/// jump leaves the ground at about 3.0 and slows by about 0.1 a frame.
const JUMP_UP: f64 = 2.5;
const JUMP_FROM: f64 = 1.5;

impl SlotStates {
    /// Read every biped update (stubs too). Blocks after one this module
    /// can't read are skipped, so a value listed after an unknown component
    /// stays as it was.
    pub fn read(film: &Film, p: &Params) -> SlotStates {
        let mut by: HashMap<i32, Vec<(i64, State)>> = HashMap::new();
        let mut jumps: HashMap<i32, Vec<i64>> = HashMap::new();
        let mut vz: HashMap<i32, f64> = HashMap::new();
        let Some(table) = Table::read(&film.chunks[0], "biped-") else { return SlotStates(by, jumps) };
        for_each_biped_update(film, |g, frame, slot, list| {
            let x = parse(g.c, data_start(g, list.len()), list, &table, p).fields;
            let v = by.entry(slot).or_default();
            let mut s = v.last().map(|x| x.1).unwrap_or_default();
            s.aim = x.aim.or(s.aim);
            s.speed = x.velocity.map(|x| x.speed).or(s.speed);
            s.position = x.position.or(s.position);
            if let Some(v) = x.velocity {
                let up = v.direction.map_or(0.0, |d| direction(d)[2]) * v.speed;
                let before = vz.insert(slot, up).unwrap_or(0.0);
                if up >= JUMP_UP && before < JUMP_FROM && jumps.get(&slot).and_then(|j| j.last()) != Some(&frame) {
                    jumps.entry(slot).or_default().push(frame);
                }
            }
            match v.last_mut() {
                Some(last) if last.0 == frame => last.1 = s,
                _ => v.push((frame, s)),
            }
            true
        });
        SlotStates(by, jumps)
    }

    /// `slot`'s jumps, frames in order.
    pub fn jumps(&self, slot: i32) -> &[i64] {
        self.1.get(&slot).map_or(&[], |v| v.as_slice())
    }

    /// `slot`'s state at `frame`: as of its last update at or before it.
    pub fn at(&self, slot: i32, frame: i64) -> Option<State> {
        let v = self.0.get(&slot)?;
        let i = v.partition_point(|x| x.0 <= frame);
        (i > 0).then(|| v[i - 1].1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dequantize_matches_the_readers_cases() {
        // Health: odd, exact: ends and midpoint are exact.
        assert_eq!(dequantize(0, 8, -1.0, 1.0, true, true), -1.0);
        assert_eq!(dequantize(254, 8, -1.0, 1.0, true, true), 1.0);
        assert_eq!(dequantize(127, 8, -1.0, 1.0, true, true), 0.0);
        // Plain: middle of the step.
        assert_eq!(dequantize(0, 12, 0.0, 4096.0, false, false), 0.5);
    }

    /// MSB-first bytes from a string of '0'/'1' (other characters ignored),
    /// padded with zeros.
    fn bytes(bits: &str) -> Vec<u8> {
        let mut b: Vec<u8> = bits.bytes().filter(|c| *c == b'0' || *c == b'1').map(|c| c - b'0').collect();
        b.resize(b.len() + 64, 0);
        b.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |v, (i, &x)| v | x << (7 - i))).collect()
    }

    /// The bits `rows` read from the start of `bits`, in a table of those rows.
    fn read(rows: &[&str], bits: &str, p: &Params) -> (Option<i64>, Fields) {
        let table = Table::from_rows(&rows.iter().map(|r| r.to_string()).collect::<Vec<_>>(), 40);
        let list: Vec<u32> = (0..rows.len() as u32).collect();
        let parsed = parse(&bytes(bits), 0, &list, &table, p);
        (parsed.end, parsed.fields)
    }

    #[test]
    fn an_object_position_is_world_or_attached() {
        let p = Params::new([15, 15, 15]);
        let rows = ["object-position-component"];
        // World: no box flag, box 0, three 15-bit fields, 2 bits.
        let (end, f) = read(&rows, &format!("0 0 0 {:015b} {:015b} {:015b} 00", 1, 2, 3), &p);
        assert_eq!((end, f.position), (Some(3 + 45 + 2), Some((0, [1, 2, 3]))));
        // The default box: three 22-bit fields.
        let (end, f) = read(&rows, &format!("0 1 {:022b} {:022b} {:022b} 00", 4, 5, 6), &p);
        assert_eq!((end, f.position), (Some(2 + 66 + 2), Some((DEFAULT_BOX, [4, 5, 6]))));
        // Attached: three 14-bit local fields, a parent handle, 11 bits.
        let (end, f) = read(&rows, &format!("1 {} 1 {:013b} 01 1 {:011b} 00", "0".repeat(42), 1100, 7), &p);
        assert_eq!((end, f.position), (Some(1 + 42 + 16 + 12 + 2), None));
    }

    #[test]
    fn vehicle_type_components_read_when_listed() {
        let rows = ["vehicle-type-state-component", "vehicle-type-physics-component"];
        // State 1 and its 6 bits; physics: not raw, default forward, roll,
        // zero angular velocity.
        let bits = "01 000000 0 1 00000000 1";
        assert_eq!(read(&rows, bits, &Params::new([15, 15, 15])).0, Some(8 + 1 + 1 + 8 + 1));
        // A snapshot reads them only for a vehicle that has them.
        let mut p = Params::new([15, 15, 15]);
        p.snapshot = true;
        assert_eq!(read(&rows, bits, &p).0, Some(0));
        p.vehicle_type = true;
        assert_eq!(read(&rows, bits, &p).0, Some(19));
    }

    #[test]
    fn radial_menu_visibility_reads_the_masked_items() {
        // Three items, the first and third in the mask: one a player, one none.
        let bits = format!("{:032b} {:032b} {:032b} 0 00011 1 1 0", 3, 0b101, 0);
        let (end, _) = read(&["managed-radialmenu-visibility-component"], &bits, &Params::new([15, 15, 15]));
        assert_eq!(end, Some(96 + 7 + 2));
    }
}
