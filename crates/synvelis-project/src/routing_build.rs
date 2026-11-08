//! SF-03 slice 1 — semantic→runtime build contract types.
//!
//! This module owns the *boundary contract* between the persisted
//! semantic routing model ([`crate::routing`]) and the Ruviform typed
//! graph/runtime: the build key that binds a prepared plan to an exact
//! project snapshot and capability set, the compile request, the
//! generation-scoped [`RoutingSourceMap`] that resolves stable
//! semantic IDs to transient runtime IDs, and the activation
//! state/receipt evidence that keeps non-RT publish success distinct
//! from RT-observed activation.
//!
//! Runtime IDs (`NodeId`/`PortId`/`ConnectionId`) issued by
//! `ruviform_graph::GraphBuilder` are graph-local and never persisted
//! into `Project`; they exist only inside a [`RoutingSourceMap`]
//! scoped to one build generation. Nothing in this module performs
//! I/O or owns a runtime — it is pure contract and projection.

use std::collections::BTreeMap;

use ruviform_graph::{ConnectionId, NodeId, PortId, PortRef};
use serde::Serialize;

use crate::control::{ParameterRole, ParameterTarget};
use crate::controller::ProjectRevision;
use crate::id::{
    BusId, ClipId, CompId, CompRegionId, CompTransitionId, ProcessorId, ProjectId,
    RoutingConnectionId, SemanticPortId, SendId, StripId,
};
use crate::project::Project;

/// Bumped when the semantic→graph lowering policy changes shape.
/// Part of [`RoutingBuildKey`]: a policy bump invalidates every
/// prepared plan even when the project snapshot is unchanged.
pub const LOWERING_POLICY_VERSION: u32 = 1;

/// Caller correlation id for one compile/publish attempt.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoutingBuildRequestId(pub u64);

/// Identity of one runtime instance — generation numbers are only
/// meaningful within the instance that issued them, so receipts and
/// build keys carry it explicitly.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeInstanceId(pub u64);

/// Device/engine clock epoch a build targets — distinct from the
/// persisted project `ClockEpochId`; runtime-scoped evidence only.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeClockEpochRef(pub u64);

/// Whether the build feeds a realtime engine or an offline render.
/// Kept explicit so preview tolerance and export fail-closed policy
/// can diverge later without changing the contract.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuildMode {
    /// Realtime playback path.
    Realtime,
    /// Offline render path.
    Offline,
}

/// Resource ceiling the compile must respect.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BuildBudget {
    /// Maximum graph nodes the lowering may emit.
    pub max_nodes: usize,
    /// Maximum graph connections the lowering may emit.
    pub max_connections: usize,
    /// Maximum callback frames the compiled plan serves.
    pub max_frames: usize,
}

/// Deterministic fingerprint of the graph-relevant semantic content
/// of a project — routing model, strip/processor control bases,
/// timeline structure and comp registry. Project `name`, view state
/// and automation *curves* are excluded: they cannot change the
/// lowered graph.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoutingFingerprint(pub u64);

/// The full identity of one prepared routing build. Two keys are
/// publish-compatible only when every field matches; [`Self::semantic_eq`]
/// compares everything except `project_revision` for the `NoRebuild`
/// evidence path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingBuildKey {
    /// Owning project.
    pub project_id: ProjectId,
    /// Committed project revision the semantic snapshot came from.
    pub project_revision: ProjectRevision,
    /// Graph-relevant semantic content fingerprint.
    pub routing_fingerprint: RoutingFingerprint,
    /// Resolved-plugin/declared-port capability generation.
    pub capability_generation: u64,
    /// Engine geometry generation (rate/channels/block).
    pub engine_config_generation: u64,
    /// Canonical source format generation.
    pub source_format_generation: u64,
    /// Lowering policy version.
    pub lowering_policy_version: u32,
    /// Realtime vs offline intent.
    pub build_mode: BuildMode,
    /// Target device clock epoch.
    pub device_clock_epoch: RuntimeClockEpochRef,
    /// Runtime instance the plan is built for.
    pub runtime_instance: RuntimeInstanceId,
}

impl RoutingBuildKey {
    /// Whether `self` and `other` describe the same executable graph
    /// ignoring only the project revision — the `NoRebuild` evidence
    /// comparison (a rename or view-only change keeps this true).
    #[must_use]
    pub fn semantic_eq(&self, other: &Self) -> bool {
        self.project_id == other.project_id
            && self.routing_fingerprint == other.routing_fingerprint
            && self.capability_generation == other.capability_generation
            && self.engine_config_generation == other.engine_config_generation
            && self.source_format_generation == other.source_format_generation
            && self.lowering_policy_version == other.lowering_policy_version
            && self.build_mode == other.build_mode
            && self.device_clock_epoch == other.device_clock_epoch
            && self.runtime_instance == other.runtime_instance
    }
}

/// Everything a caller supplies to start one build. The semantic
/// snapshot itself is the live `&Project` passed alongside — the
/// request pins *which* revision/capability set it must match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingCompileRequest {
    /// Caller correlation id echoed into receipts.
    pub request_id: RoutingBuildRequestId,
    /// The committed revision the caller believes is live.
    pub expected_project_revision: ProjectRevision,
    /// Realtime vs offline intent.
    pub mode: BuildMode,
    /// Resource ceiling for the compile.
    pub resource_budget: BuildBudget,
    /// Runtime instance being built for.
    pub runtime_instance: RuntimeInstanceId,
    /// Target device clock epoch.
    pub device_clock_epoch: RuntimeClockEpochRef,
}

/// Why a semantic entity could not be resolved to a runtime object —
/// explicit, never a silent fallback.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AvailabilityReason {
    /// A plugin/processor instance is not available for this build.
    PluginUnavailable,
    /// The resolved binding drifted from its declared capability.
    CapabilityMismatch,
    /// A required runtime instance was not supplied.
    InstanceMissing,
}

/// Why a semantic entity is outside this slice's lowerable domain.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnsupportedReason {
    /// Signal domain the lowering does not carry.
    DomainUnsupported,
    /// Port/channel layout the engine cannot represent.
    FormatUnsupported,
    /// Routing topology the lowering refuses to reinterpret.
    TopologyUnsupported,
}

/// Why a semantic entity produced no runtime object.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NotLoweredReason {
    /// The owning strip is muted/solo-excluded under `audible_strips`.
    MutedStrip,
    /// The semantic connection is disabled.
    DisabledConnection,
    /// Deliberately out of this slice's scope.
    OutOfScope,
}

/// Resolution of one semantic entity to its runtime counterpart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MappingResolution<T> {
    /// Mapped to a live runtime ID.
    Resolved(T),
    /// Entity exists but its runtime object is unavailable.
    Unavailable(AvailabilityReason),
    /// Entity is outside the lowerable domain.
    Unsupported(UnsupportedReason),
    /// Entity intentionally produced nothing.
    NotLowered(NotLoweredReason),
}

/// A semantic signal-bearing entity — the stable ID side of the
/// `SourceMap`'s node table.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SemanticNodeRef {
    /// A channel strip.
    Strip(StripId),
    /// A bus (resolved through its strip).
    Bus(BusId),
    /// A chain processor.
    Processor(ProcessorId),
}

/// What a lowered playback node plays — the stable provenance side of
/// [`RoutingPlaybackRef`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlaybackContentRef {
    /// A project clip.
    Clip(ClipId),
    /// An adopted comp region's base read.
    CompSegment {
        /// Owning comp.
        comp_id: CompId,
        /// Comp region.
        region_id: CompRegionId,
    },
    /// A comp transition's extended read.
    CompOverlap {
        /// Owning comp.
        comp_id: CompId,
        /// Comp transition.
        transition_id: CompTransitionId,
        /// Which side of the overlap.
        side: PlaybackOverlapSide,
    },
}

/// Side of a comp transition overlap.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlaybackOverlapSide {
    /// Left (outgoing) side.
    Left,
    /// Right (incoming) side.
    Right,
}

/// Generation-scoped binding of one lowered playback node back to its
/// semantic provenance and destination port.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RoutingPlaybackRef {
    /// What the node plays.
    pub content: PlaybackContentRef,
    /// Runtime node ID (graph-local, never persisted).
    pub node_id: NodeId,
    /// The node's audio output port.
    pub out_port: PortId,
    /// Semantic port this playback feeds (the owning strip's main
    /// input).
    pub destination: SemanticPortId,
}

/// The semantic→runtime map for exactly one build generation. Every
/// runtime ID in here is transient: a new lowering reissues them and
/// this map is replaced wholesale — stale maps must never be applied
/// to a different generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingSourceMap {
    /// Owning project.
    pub project_id: ProjectId,
    /// Committed revision the semantic snapshot came from — must
    /// agree with the `Project` used to build the graph.
    pub project_revision: ProjectRevision,
    /// The build key this map belongs to.
    pub build_key: RoutingBuildKey,
    /// Compiled plan generation the IDs are scoped to.
    pub generation: u64,
    /// Semantic entity → runtime node resolution.
    pub semantic_nodes: BTreeMap<SemanticNodeRef, MappingResolution<NodeId>>,
    /// Semantic port → runtime port resolution.
    pub ports: BTreeMap<SemanticPortId, MappingResolution<PortRef>>,
    /// Semantic connection → runtime connection resolution.
    pub connections: BTreeMap<RoutingConnectionId, MappingResolution<ConnectionId>>,
    /// Send → resolved tap port (pre-fader dry out or post-fader main
    /// out), recorded separately because one semantic port resolves
    /// to two runtime ports depending on the send's tap.
    pub send_taps: BTreeMap<SendId, PortRef>,
    /// Playback node provenance bindings.
    pub playback_bindings: Vec<RoutingPlaybackRef>,
}

/// Explicit build/activation failure taxonomy. This slice covers the
/// variants the lowering/activation path can actually produce; the
/// full Issue #106 classification lands in a later slice.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RoutingFault {
    /// The committed project revision moved past the prepared key.
    StaleProjectRevision,
    /// Plugin/port capability generation drifted.
    StaleCapability,
    /// A temporary (pre-commit) ID leaked into a mapping.
    TemporaryIdMismatch,
    /// A clip/comp source could not be resolved for playback.
    MissingSource,
    /// A playback mapping the graph cannot lower.
    UnsupportedMapping,
    /// Port channel count/layout disagrees with the engine format.
    ChannelMismatch,
    /// A processor has no usable plugin/processor instance.
    PluginUnavailable,
    /// The compile factory had no instance for a processor node.
    MissingProcessorInstance,
    /// The typed compiler rejected the prepared spec.
    CompileFailed,
    /// Generic graph validation rejected the spec.
    ValidationFailed,
    /// The runtime command channel could not hold the plan delta.
    PublishChannelFull,
    /// An observed generation disagrees with the published plan.
    GenerationMismatch,
    /// The engine is stopped — no frames can be processed.
    EngineStopped,
    /// A plan event arrived for a stale generation.
    StaleEvent,
}

/// Activation evidence state — `Configured`/`Validated`/`Prepared` are
/// non-RT milestones, `Published` is the non-RT `GraphRuntime::publish`
/// success point, and `Active` requires RT-observed generation
/// evidence. They are never conflated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutingActivationState {
    /// Semantic snapshot accepted; nothing built yet.
    Configured {
        /// Project revision configured.
        project_revision: ProjectRevision,
    },
    /// Generic graph validation passed.
    Validated {
        /// Project revision validated.
        project_revision: ProjectRevision,
    },
    /// Compile finished; plan generation assigned but not published.
    Prepared {
        /// Project revision prepared.
        project_revision: ProjectRevision,
        /// Planned (compiled) generation.
        planned_generation: u64,
    },
    /// Non-RT publish succeeded — *not* proof of RT activation.
    Published {
        /// Project revision published.
        project_revision: ProjectRevision,
        /// Published generation awaiting RT observation.
        generation: u64,
    },
    /// RT callback observed the published generation.
    Active {
        /// Project revision active.
        project_revision: ProjectRevision,
        /// Active generation.
        generation: u64,
        /// Clock epoch the activation was observed in.
        epoch: RuntimeClockEpochRef,
    },
    /// Activation cannot proceed; the previous active plan remains
    /// last-good.
    Blocked {
        /// Why activation is blocked.
        fault: RoutingFault,
        /// Last generation observed active, if any.
        last_good_generation: Option<u64>,
    },
    /// Activation attempted and failed; last-good retained.
    Failed {
        /// What failed.
        fault: RoutingFault,
        /// Last generation observed active, if any.
        last_good_generation: Option<u64>,
    },
    /// This build's plan was superseded or retired.
    Retired,
}

/// One activation evidence record — the honest answer to "how far did
/// this build actually get?" `published_generation` and
/// `observed_active_generation` are separate fields precisely so a
/// non-RT publish success can never masquerade as RT activation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingActivationReceipt {
    /// Caller correlation id, when the build came from a request.
    pub request_id: Option<RoutingBuildRequestId>,
    /// Build key the receipt refers to.
    pub build_key: Option<RoutingBuildKey>,
    /// Current evidence state.
    pub state: RoutingActivationState,
    /// Generation the compile planned.
    pub planned_generation: Option<u64>,
    /// Generation `GraphRuntime::publish` accepted.
    pub published_generation: Option<u64>,
    /// Generation the RT side was observed running.
    pub observed_active_generation: Option<u64>,
    /// Last generation confirmed active before any failure.
    pub last_good_generation: Option<u64>,
    /// Frames processed while this receipt's plan was active.
    pub frames_observed: u64,
    /// Failure detail when `state` is `Blocked`/`Failed`.
    pub fault: Option<RoutingFault>,
}

/// FNV-1a 64 over the canonical JSON projection of the
/// graph-relevant project content. Deterministic across processes;
/// collisions are possible in principle but the fingerprint is a
/// *change detector*, not a cryptographic identity — the full
/// [`RoutingBuildKey`] equality check still gates publish.
#[must_use]
pub fn routing_fingerprint(project: &Project) -> RoutingFingerprint {
    /// Trimmed target projection: display `name` cannot change the
    /// graph, so it is excluded from the fingerprint.
    #[derive(Serialize)]
    struct TargetProjection<'a> {
        id: &'a crate::id::ParameterTargetId,
        owner: crate::control::ControlOwner,
        role: ParameterRole,
        time_scope: crate::control::AutomationTimeScope,
        kind: crate::control::ValueKind,
        mapping: crate::control::ParamMapping,
        capability: Option<crate::control::TargetCapability>,
        base: f64,
    }

    let targets: Vec<TargetProjection<'_>> = project
        .control
        .targets
        .values()
        .map(|t: &ParameterTarget| TargetProjection {
            id: &t.id,
            owner: t.owner,
            role: t.role,
            time_scope: t.time_scope,
            kind: t.kind,
            mapping: t.mapping,
            capability: t.capability,
            base: t.base,
        })
        .collect();
    let projection = serde_json::json!({
        "routing": project.routing,
        "targets": targets,
        "tracks": project.tracks,
        "lanes": project.lanes,
        "clips": project.clips,
        "comp": project.comp,
    });
    let bytes = serde_json::to_vec(&projection).unwrap_or_default();
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    RoutingFingerprint(hash)
}
