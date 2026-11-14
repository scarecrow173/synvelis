//! CB-06 Creative Branch strip-fields adapter fixtures (synvelis#283).
//!
//! Covers the real `StripFieldsAdapter` headlessly: stable `StripId`
//! resolution for track-owned and bus-owned strips with no duplicate
//! DSP, declared-`StripDomainMask` gating of channel fields and the
//! insert chain, typed `SemanticPortId` boundary-dependency
//! enumeration with fail-closed external-wiring ops, one-CAS adopt
//! with undo/save-reload and stale-scope/recording/missing-plugin
//! gates, per-candidate plugin-instance/state honesty, and render-pin
//! identity independence. Real OS / plugin / RT / PCM lanes are
//! `NotRun` — recorded in `evidence/sv283/`.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_plugin_core::ParamFlags;
use synvelis_project::apply::PluginAvailability;
use synvelis_project::control::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, ConflictPolicy,
    ControlOwner, Interpolation, LfoShape, ParamMapping, ParameterRole, Polarity, SourceKind,
    TargetCapability, Transform, ValueKind,
};
use synvelis_project::controller::{Command, ProjectController};
use synvelis_project::diff::{
    ChangeKind, ComparisonSpec, ObjectKind, SemanticChange, compare_snapshots,
};
use synvelis_project::id::CounterIdGenerator;
use synvelis_project::monitoring_data::{
    ArmState, MonitorChannelMap, MonitorState, TrackMonitorDecl,
};
use synvelis_project::plugin_state::{
    CaptureMode, CaptureObservation, PluginStateBinding, PluginStateCapture,
    PluginStateCaptureError, PluginStateFailureKind, PluginStateOutcome, PluginStatePort,
    StateCaptureQuality,
};
use synvelis_project::project::Project;
use synvelis_project::recording_data::{CaptureStream, ClockEpoch, RecordingSession, SessionState};
use synvelis_project::routing::{
    BusKind, ConnectionKind, PluginRef, PortDirection, PortOwner, PortRole, ProcessorBinding,
    SendTap, StripOwner,
};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, ContentDigest, FixedSnapshotLocator, ImmutableSnapshot,
    ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness, SnapshotIdentity,
    SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::{InMemoryStore, SCHEMA_VERSION};
use synvelis_project::variant::{
    AdoptIntent, AdoptOutcome, AdoptSources, BranchContentRef, BranchEditOp, BranchEditOpIntent,
    DomainAdapter, FakeRuntimePublish, InMemoryBranchContentStore, InstrumentStateAssessment,
    NoopRuntimePublish, ProcessingCapability, RuntimePublishState, StripDomainMask, StripField,
    StripFieldsAdapter, VariantError, VariantScope, adopt_branch_adapted, decode_candidate,
    edit_branch_op, fork_variant_set, scope_digest,
};
use synvelis_project::{
    AutomationLaneId, BranchId, BusId, CaptureStreamId, ChainId, ClockEpochId, LaneId,
    ModulationAssignmentId, ModulationSourceId, ParameterTargetId, ProcessorId, ProjectId,
    ProjectOp, ProjectRevision, RecordingSessionId, RenderBranchTarget, RoutingConnectionId,
    SemanticPortId, SendId, StripId, TrackId, VariantSetId, is_issuable_raw,
    resolve_render_project,
};

// ---------- shared harness (cb05_fx_range.rs conventions) ----------

struct Strip {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    resolver: SnapshotResolver,
    adapter: StripFieldsAdapter,
}

impl Strip {
    fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            resolver: SnapshotResolver::new(),
            adapter: StripFieldsAdapter::new(),
        }
    }

    fn fork(&mut self, scope: VariantScope) -> synvelis_project::variant::ForkReceipt {
        fork_variant_set(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &self.resolver,
            scope,
            &SnapshotCancel::new(),
        )
        .unwrap()
    }

    fn try_fork(&mut self, scope: VariantScope) -> Result<(), VariantError> {
        fork_variant_set(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &self.resolver,
            scope,
            &SnapshotCancel::new(),
        )
        .map(|_| ())
    }

    fn try_edit_op(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        op: BranchEditOp,
    ) -> Result<(), VariantError> {
        let expected = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .revision();
        edit_branch_op(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &BranchEditOpIntent {
                variant_set: set,
                branch,
                expected_branch_revision: expected,
                op,
            },
        )
        .map(|_| ())
    }

    fn try_edit(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        op: ProjectOp,
    ) -> Result<(), VariantError> {
        self.try_edit_op(set, branch, BranchEditOp::Project(op))
    }

    fn edit(&mut self, set: VariantSetId, branch: BranchId, op: ProjectOp) {
        self.try_edit(set, branch, op).unwrap();
    }

    fn candidate(&self, set: VariantSetId, branch: BranchId) -> Project {
        let content = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .clone();
        decode_candidate(&self.store, &content).unwrap()
    }

    fn intent(&self, set: VariantSetId, branch: BranchId, scope: &VariantScope) -> AdoptIntent {
        let set_state = &self.controller.live().variant_sets[&set];
        AdoptIntent {
            variant_set: set,
            branch,
            expected_project_revision: self.controller.live_revision(),
            expected_branch_revision: set_state.branches[&branch].content.revision(),
            expected_scope_digest: scope_digest(self.controller.live(), scope).unwrap(),
            approval: synvelis_project::apply::ApplyApproval {
                ticket: "approve".into(),
            },
        }
    }

    fn try_adopt_full(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
        plugins: Option<&dyn PluginAvailability>,
        runtime: &dyn synvelis_project::variant::RuntimePublish,
    ) -> Result<synvelis_project::variant::AdoptReceipt, VariantError> {
        let intent = self.intent(set, branch, scope);
        adopt_branch_adapted(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &AdoptSources {
                assets: None,
                plugins,
                runtime,
            },
            &intent,
        )
    }

    fn try_adopt(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
    ) -> Result<synvelis_project::variant::AdoptReceipt, VariantError> {
        let runtime = NoopRuntimePublish;
        self.try_adopt_full(set, branch, scope, None, &runtime)
    }

    fn adopt(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
    ) -> synvelis_project::variant::AdoptReceipt {
        self.try_adopt(set, branch, scope).unwrap()
    }

    /// Canonical render-reuse pin — the RCP-01 read-adapter identity
    /// the comparison context staleness contract consumes.
    fn render_pin(
        &self,
        set: VariantSetId,
        branch: BranchId,
    ) -> synvelis_project::render_input::BranchRenderPin {
        resolve_render_project(
            self.controller.live(),
            Some(RenderBranchTarget {
                variant_set: set,
                branch,
                store: &self.store,
            }),
        )
        .unwrap()
        .branch
        .unwrap()
    }
}

// ---------- fixture: track strip + bus strip + key strip ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb06-fixture"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply_op(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

struct StripIds {
    track: TrackId,
    lane: LaneId,
    /// Track-owned strip — the primary scope root.
    strip: StripId,
    chain: ChainId,
    /// Insert-chain processors in chain order; `fx2` carries a
    /// sidechain input port for the external key feed.
    fx1: ProcessorId,
    fx2: ProcessorId,
    /// Return bus and its owned strip — the bus-side scope root.
    bus: BusId,
    bus_strip: StripId,
    bus_chain: ChainId,
    /// Sibling track strip — foreign-strip witness and sidechain key
    /// source, never scope-owned.
    key_strip: StripId,
    /// Seeded channel targets on the scoped strip (`Pan` is not
    /// seeded — it registers through `AddParameterTarget`).
    param_fader: ParameterTargetId,
    param_mute: ParameterTargetId,
    param_solo: ParameterTargetId,
    param_fx1: ParameterTargetId,
    param_fx2: ParameterTargetId,
    /// The strip→bus-strip send and the key-strip→fx2 sidechain feed —
    /// the two persistent boundary crossings.
    send: SendId,
    feed: RoutingConnectionId,
}

fn capability(param_id: u32) -> TargetCapability {
    TargetCapability {
        param_id,
        flags_bits: ParamFlags::AUTOMATABLE.bits(),
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.5,
    }
}

fn add_target(
    c: &mut ProjectController,
    owner: ControlOwner,
    name: &str,
    cap: Option<TargetCapability>,
    base: f64,
) -> ParameterTargetId {
    apply_op(
        c,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner,
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: cap,
            base,
        },
    );
    *c.live()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap()
}

fn plugin_binding(desc: &str) -> ProcessorBinding {
    ProcessorBinding::Resolved(PluginRef {
        descriptor: desc.into(),
        version: "1.0".into(),
        capability_revision: 1,
    })
}

/// The seeded `ControlOwner::Strip` + channel-role target of `strip`.
fn strip_channel_param(
    project: &Project,
    strip: StripId,
    role: ParameterRole,
) -> ParameterTargetId {
    project
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == role)
        .map_or_else(
            || panic!("strip {strip} lacks seeded {role:?} target"),
            |t| t.id,
        )
}

/// Track → lane → strip → chain `[fx1, fx2(sc-in)]`.
fn track_strip_chain(
    c: &mut ProjectController,
) -> (TrackId, LaneId, StripId, ChainId, ProcessorId, ProcessorId) {
    apply_op(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    let chain = c.live().routing.strips[&strip].chain;
    for (index, name, desc, sidechain) in
        [(0usize, "fx1", "plug-a", false), (1, "fx2", "plug-b", true)]
    {
        apply_op(
            c,
            ProjectOp::InsertProcessor {
                strip_id: strip,
                index,
                name: name.into(),
                binding: plugin_binding(desc),
                sidechain_input: sidechain,
            },
        );
    }
    let ordered = c.live().routing.chains[&chain].processors.clone();
    let [fx1, fx2] = ordered[..] else {
        panic!("fixture chain must hold exactly two processors");
    };
    (track, lane, strip, chain, fx1, fx2)
}

/// Return bus: its own strip + empty chain + seeded channel
/// targets — the bus-side scope root (no automatic route).
fn return_bus(c: &mut ProjectController) -> (BusId, StripId, ChainId) {
    apply_op(
        c,
        ProjectOp::AddBus {
            kind: BusKind::Return,
            name: "ret".into(),
        },
    );
    let bus = c
        .live()
        .routing
        .buses
        .values()
        .find(|b| b.kind == BusKind::Return)
        .map(|b| b.id)
        .unwrap();
    let bus_strip = c.live().routing.buses[&bus].strip;
    let bus_chain = c.live().routing.strips[&bus_strip].chain;
    (bus, bus_strip, bus_chain)
}

/// Sibling track strip: empty chain, auto-routed to master —
/// foreign-strip witness and sidechain key source.
fn key_strip(c: &mut ProjectController, track: TrackId) -> StripId {
    apply_op(c, ProjectOp::AddTrack { name: "key".into() });
    let key_track = *c.live().tracks.keys().find(|t| **t != track).unwrap();
    apply_op(
        c,
        ProjectOp::AddTrackStrip {
            track_id: key_track,
        },
    );
    c.live().routing.strip_of_track(key_track).unwrap().id
}

/// The two persistent boundary crossings: strip→bus-strip `Send`
/// and key-strip→fx2 `Sidechain` feed.
fn boundary_wiring(
    c: &mut ProjectController,
    strip: StripId,
    bus_strip: StripId,
    key_strip: StripId,
    fx2: ProcessorId,
) -> (SendId, RoutingConnectionId) {
    let strip_out = port_of(
        c.live(),
        PortOwner::Strip(strip),
        PortDirection::Output,
        PortRole::Main,
    );
    let bus_in = port_of(
        c.live(),
        PortOwner::Strip(bus_strip),
        PortDirection::Input,
        PortRole::Main,
    );
    apply_op(
        c,
        ProjectOp::AddSend {
            source: strip_out,
            destination: bus_in,
            tap: SendTap::PostFader,
            gain_target: None,
        },
    );
    let send = *c.live().routing.sends.keys().next().unwrap();

    let key_out = port_of(
        c.live(),
        PortOwner::Strip(key_strip),
        PortDirection::Output,
        PortRole::Main,
    );
    let fx2_sc = port_of(
        c.live(),
        PortOwner::Processor(fx2),
        PortDirection::Input,
        PortRole::Sidechain,
    );
    apply_op(
        c,
        ProjectOp::AddConnection {
            source: key_out,
            destination: fx2_sc,
            kind: ConnectionKind::Sidechain,
            channel_map: None,
        },
    );
    let feed = *c.live().routing.connections.keys().next_back().unwrap();
    (send, feed)
}

/// Track → lane → strip → chain `[fx1, fx2(sc-in)]`; a return bus
/// with its own strip/chain; a sibling track strip as key source.
/// Wiring: strip→master main route (automatic), strip→bus-strip
/// `Send`, key-strip→fx2 `Sidechain` feed.
fn strip_fixture(c: &mut ProjectController) -> StripIds {
    let (track, lane, strip, chain, fx1, fx2) = track_strip_chain(c);
    let (bus, bus_strip, bus_chain) = return_bus(c);
    let key = key_strip(c, track);
    let (send, feed) = boundary_wiring(c, strip, bus_strip, key, fx2);

    let param_fader = strip_channel_param(c.live(), strip, ParameterRole::Fader);
    let param_mute = strip_channel_param(c.live(), strip, ParameterRole::Mute);
    let param_solo = strip_channel_param(c.live(), strip, ParameterRole::Solo);
    let param_fx1 = add_target(
        c,
        ControlOwner::Processor(fx1),
        "fx1-cutoff",
        Some(capability(7)),
        0.5,
    );
    let param_fx2 = add_target(
        c,
        ControlOwner::Processor(fx2),
        "fx2-threshold",
        Some(capability(8)),
        0.6,
    );
    StripIds {
        track,
        lane,
        strip,
        chain,
        fx1,
        fx2,
        bus,
        bus_strip,
        bus_chain,
        key_strip: key,
        param_fader,
        param_mute,
        param_solo,
        param_fx1,
        param_fx2,
        send,
        feed,
    }
}

fn mask(fields: &[StripField]) -> StripDomainMask {
    StripDomainMask(fields.iter().copied().collect())
}

fn scope_of(ids: &StripIds, fields: &[StripField]) -> VariantScope {
    VariantScope::StripFields {
        strip: ids.strip,
        fields: mask(fields),
    }
}

fn bus_scope_of(ids: &StripIds, fields: &[StripField]) -> VariantScope {
    VariantScope::StripFields {
        strip: ids.bus_strip,
        fields: mask(fields),
    }
}

/// The port with `owner`/`direction`/`role` — semantic-port lookup for
/// wiring fixtures (send destination, sidechain feed).
fn port_of(
    project: &Project,
    owner: PortOwner,
    direction: PortDirection,
    role: PortRole,
) -> SemanticPortId {
    project
        .routing
        .ports
        .values()
        .find(|p| p.owner == owner && p.direction == direction && p.role == role)
        .map_or_else(
            || panic!("no {owner:?} {direction:?} {role:?} port"),
            |p| p.id,
        )
}

// ===========================================================================
// CB-STRIP-01: stable StripId resolution for Track and Bus strips —
// one DSP strip per owner, no duplicate instantiation
// ===========================================================================

#[test]
fn cb_strip_01_track_and_bus_strip_resolve_same_stable_identity() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);

    // Stable identity: the track resolves to exactly one strip through
    // `strip_of_track`; the bus names its strip directly. Both are
    // `StripOwner`-tagged `StripId`s — the same identity space, never
    // a second DSP strip for a display context.
    let live = c.live();
    assert_eq!(
        live.routing.strips[&ids.strip].owner,
        StripOwner::Track(ids.track)
    );
    assert_eq!(
        live.routing.strips[&ids.bus_strip].owner,
        StripOwner::Bus(ids.bus)
    );
    assert_eq!(live.routing.buses[&ids.bus].strip, ids.bus_strip);
    assert_eq!(
        live.routing.strip_of_track(ids.track).unwrap().id,
        ids.strip
    );

    // Track-strip candidate: the strip entity is byte-identical —
    // same `StripId`, owner, chain and ports; no duplicate strip or
    // chain is materialized inside the candidate.
    let mut h = Strip::new(c);
    let forked = h.fork(scope_of(&ids, &[StripField::Mute]));
    let cand = h.candidate(forked.variant_set, forked.candidate);
    assert_eq!(
        cand.routing.strips[&ids.strip],
        live_strip(h.controller.live(), ids.strip)
    );
    assert_eq!(
        cand.routing.strips.len(),
        h.controller.live().routing.strips.len()
    );
    assert_eq!(
        cand.routing.chains.len(),
        h.controller.live().routing.chains.len()
    );
    assert_eq!(
        cand.plugin_instances.instances.len(),
        h.controller.live().plugin_instances.instances.len()
    );

    // Bus-strip scope on the same project: disjoint owned sets, so
    // both candidates coexist — the bus strip resolves through the
    // same stable `StripId` path with no duplicate DSP.
    let forked_bus = h.fork(bus_scope_of(&ids, &[StripField::Inserts]));
    let bus_cand = h.candidate(forked_bus.variant_set, forked_bus.candidate);
    assert_eq!(
        bus_cand.routing.strips[&ids.bus_strip].owner,
        StripOwner::Bus(ids.bus)
    );
    assert_eq!(bus_cand.routing.buses[&ids.bus].strip, ids.bus_strip);
    assert_eq!(
        bus_cand.plugin_instances.instances.len(),
        h.controller.live().plugin_instances.instances.len()
    );

    // Scope validation fail-closed: empty mask, unknown strip and a
    // foreign scope kind all reject typed — never a widened scope.
    let err = h
        .try_fork(VariantScope::StripFields {
            strip: ids.strip,
            fields: StripDomainMask(BTreeSet::new()),
        })
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Project(_)),
        "empty mask must reject typed, got {err:?}"
    );
    let err = h
        .try_fork(VariantScope::StripFields {
            strip: StripId::new(999_999),
            fields: mask(&[StripField::Mute]),
        })
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Project(_)),
        "unknown strip must reject typed, got {err:?}"
    );
    let foreign = VariantScope::InstrumentSlot { processor: ids.fx1 };
    let err = h.adapter.validate_scope(h.controller.live(), &foreign);
    assert!(
        matches!(err, Err(VariantError::InvalidScope { .. })),
        "foreign scope must reject InvalidScope, got {err:?}"
    );
}

fn live_strip(project: &Project, strip: StripId) -> synvelis_project::routing::ChannelStrip {
    project.routing.strips[&strip].clone()
}

// ===========================================================================
// CB-STRIP-02: declared mask gates every field family — mask-external
// edits reject typed, declared edits land only inside the candidate
// ===========================================================================

/// Fader base + first-use `Pan` registration + fx param base + a new
/// processor-owned target — the declared channel/insert parameter
/// surface shared by the mask tests and the adopt fixture.
fn scoped_base_and_param_edits(
    h: &mut Strip,
    set: VariantSetId,
    candidate: BranchId,
    ids: &StripIds,
) {
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fader,
            value: 0.3,
        },
    );
    h.edit(
        set,
        candidate,
        ProjectOp::AddParameterTarget {
            name: "Pan".into(),
            owner: ControlOwner::Strip(ids.strip),
            role: ParameterRole::Pan,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let cand = h.candidate(set, candidate);
    let pan = strip_channel_param(&cand, ids.strip, ParameterRole::Pan);
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: pan,
            value: 0.7,
        },
    );
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fx1,
            value: 0.9,
        },
    );
    h.edit(
        set,
        candidate,
        ProjectOp::AddParameterTarget {
            name: "fx1-resonance".into(),
            owner: ControlOwner::Processor(ids.fx1),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(capability(11)),
            base: 0.3,
        },
    );
}

/// Automation lane + region on the scoped fader target.
fn scoped_automation_edit(
    h: &mut Strip,
    set: VariantSetId,
    candidate: BranchId,
    ids: &StripIds,
    live_before: &Project,
) {
    h.edit(
        set,
        candidate,
        ProjectOp::AddAutomationLane {
            target: ids.param_fader,
        },
    );
    let auto_lane = *h
        .candidate(set, candidate)
        .control
        .lanes
        .keys()
        .find(|id| !live_before.control.lanes.contains_key(*id))
        .unwrap();
    h.edit(
        set,
        candidate,
        ProjectOp::WriteAutomationRegion {
            lane: auto_lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(1, 1).unwrap(),
                        value: 0.8,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(3, 1).unwrap(),
                        value: 0.2,
                    },
                ],
            },
            policy: ConflictPolicy::Replace,
        },
    );
}

/// Strip-owned modulation source + assignment onto the scoped fader
/// target — both land inside the candidate's control closure.
fn scoped_modulation_edit(
    h: &mut Strip,
    set: VariantSetId,
    candidate: BranchId,
    ids: &StripIds,
    live_before: &Project,
) {
    h.edit(
        set,
        candidate,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Strip(ids.strip),
            kind: SourceKind::Lfo {
                rate: BeatPoint::new(2, 1).unwrap(),
                shape: LfoShape::Sine,
                bipolar: true,
            },
        },
    );
    let source = *h
        .candidate(set, candidate)
        .control
        .sources
        .keys()
        .find(|id| !live_before.control.sources.contains_key(*id))
        .unwrap();
    h.edit(
        set,
        candidate,
        ProjectOp::AddModulationAssignment {
            source,
            target: ids.param_fader,
            depth: 0.4,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
}

/// Processor-owned modulation source on `fx2` — legal only under
/// `Inserts`.
fn scoped_processor_modulation_edit(
    h: &mut Strip,
    set: VariantSetId,
    candidate: BranchId,
    ids: &StripIds,
) {
    h.edit(
        set,
        candidate,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Processor(ids.fx2),
            kind: SourceKind::Lfo {
                rate: BeatPoint::new(2, 1).unwrap(),
                shape: LfoShape::Sine,
                bipolar: true,
            },
        },
    );
}

/// `InsertProcessor` strictly inside the scoped strip's chain at
/// index 1 — `[fx1, fx2]` → `[fx1, mid, fx2]`.
fn insert_mid_chain_processor(
    h: &mut Strip,
    set: VariantSetId,
    candidate: BranchId,
    ids: &StripIds,
) -> ProcessorId {
    h.edit(
        set,
        candidate,
        ProjectOp::InsertProcessor {
            strip_id: ids.strip,
            index: 1,
            name: "fx-mid".into(),
            binding: plugin_binding("plug-mid"),
            sidechain_input: false,
        },
    );
    h.candidate(set, candidate).routing.chains[&ids.chain]
        .processors
        .iter()
        .copied()
        .find(|p| ![ids.fx1, ids.fx2].contains(p))
        .unwrap()
}

/// Automation lane + region on the scoped *mute* target — the
/// boolean channel's control closure under a `{Mute}` mask.
/// Returns the minted lane for the removal-gate assertion.
fn mute_automation_lane(
    h: &mut Strip,
    set: VariantSetId,
    candidate: BranchId,
    ids: &StripIds,
    live_before: &Project,
) -> AutomationLaneId {
    h.edit(
        set,
        candidate,
        ProjectOp::AddAutomationLane {
            target: ids.param_mute,
        },
    );
    let lane = *h
        .candidate(set, candidate)
        .control
        .lanes
        .keys()
        .find(|id| !live_before.control.lanes.contains_key(*id))
        .unwrap();
    h.edit(
        set,
        candidate,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(1, 1).unwrap(),
                        value: 1.0,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(3, 1).unwrap(),
                        value: 0.0,
                    },
                ],
            },
            policy: ConflictPolicy::Replace,
        },
    );
    lane
}

/// The declared `{Mute}` surface: bool op, direct target base,
/// automation lane + region and a strip-owned modulation source —
/// all applied inside the candidate only. The source has no declared
/// continuous target to assign to under `{Mute}`, so the assignment
/// must fail closed here. Returns the minted automation lane for the
/// removal-gate assertion.
fn declared_mute_surface_edits(
    h: &mut Strip,
    set: VariantSetId,
    candidate: BranchId,
    ids: &StripIds,
    live_before: &Project,
) -> AutomationLaneId {
    h.edit(
        set,
        candidate,
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
    );
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_mute,
            value: 0.0,
        },
    );
    let lane = mute_automation_lane(h, set, candidate, ids, live_before);
    // Strip-owned modulation source: scope-owned for every non-empty
    // mask — but no *declared* continuous target exists to assign it
    // to under `{Mute}`, so the assignment must fail closed.
    h.edit(
        set,
        candidate,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Strip(ids.strip),
            kind: SourceKind::Lfo {
                rate: BeatPoint::new(2, 1).unwrap(),
                shape: LfoShape::Sine,
                bipolar: true,
            },
        },
    );
    let source = *h
        .candidate(set, candidate)
        .control
        .sources
        .keys()
        .find(|id| !live_before.control.sources.contains_key(*id))
        .unwrap();
    let err = h
        .try_edit(
            set,
            candidate,
            ProjectOp::AddModulationAssignment {
                source,
                target: ids.param_fader,
                depth: 0.4,
                depth_source: None,
                polarity: Polarity::Bipolar,
                transform: Transform::Identity,
                enabled: true,
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::ScopeViolation { .. }),
        "assignment onto an undeclared field target must reject, got {err:?}"
    );
    lane
}

/// Every op a `{Mute}`-only mask must reject typed — undeclared
/// channel fields, non-channel/foreign/processor owners and the
/// insert chain.
fn mask_external_reject_ops(ids: &StripIds) -> Vec<ProjectOp> {
    vec![
        ProjectOp::SetStripSolo {
            strip_id: ids.strip,
            soloed: true,
        },
        ProjectOp::SetParameterBase {
            target: ids.param_fader,
            value: 0.3,
        },
        ProjectOp::SetParameterBase {
            target: ids.param_solo,
            value: 1.0,
        },
        ProjectOp::SetParameterBase {
            target: ids.param_fx1,
            value: 0.9,
        },
        // `Pan` is a declared-field *family* but not in this mask —
        // creation rejects Unsupported before singleton/spec checks.
        ProjectOp::AddParameterTarget {
            name: "Pan".into(),
            owner: ControlOwner::Strip(ids.strip),
            role: ParameterRole::Pan,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
        // A strip-owned non-channel role is never scope-owned.
        ProjectOp::AddParameterTarget {
            name: "intruder".into(),
            owner: ControlOwner::Strip(ids.strip),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
        // Foreign strip owner — `Pan` is unseeded on the key strip so
        // validate passes and the scope gate itself must reject.
        ProjectOp::AddParameterTarget {
            name: "foreign".into(),
            owner: ControlOwner::Strip(ids.key_strip),
            role: ParameterRole::Pan,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
        // Processor-owned creation without `Inserts`.
        ProjectOp::AddParameterTarget {
            name: "fx-param".into(),
            owner: ControlOwner::Processor(ids.fx1),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(capability(11)),
            base: 0.3,
        },
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Processor(ids.fx1),
            kind: SourceKind::Lfo {
                rate: BeatPoint::new(2, 1).unwrap(),
                shape: LfoShape::Sine,
                bipolar: true,
            },
        },
        // The insert chain is not declared.
        ProjectOp::InsertProcessor {
            strip_id: ids.strip,
            index: 0,
            name: "fx-mid".into(),
            binding: plugin_binding("plug-mid"),
            sidechain_input: false,
        },
        // Foreign strip insert — also mask-gated first.
        ProjectOp::InsertProcessor {
            strip_id: ids.key_strip,
            index: 0,
            name: "foreign".into(),
            binding: plugin_binding("plug-x"),
            sidechain_input: false,
        },
    ]
}

/// `mask = {Mute}`: the mute channel surface is editable; every other
/// field family, the insert chain and foreign strips fail closed.
#[test]
fn cb_strip_02_mask_gated_channel_edits() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Mute]);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);
    let live_before = h.controller.live().clone();

    let lane = declared_mute_surface_edits(&mut h, set, candidate, &ids, &live_before);

    let cand = h.candidate(set, candidate);
    assert_eq!(cand.control.targets[&ids.param_mute].base, 0.0);
    assert!(
        cand.control
            .lanes
            .values()
            .any(|l| l.target == ids.param_mute)
    );
    assert_eq!(
        h.controller.live().control.targets[&ids.param_mute].base,
        live_before.control.targets[&ids.param_mute].base,
        "live must stay untouched by candidate edits"
    );

    // Mask-external field ops reject typed — the `Strip` entity being
    // scope-owned cannot smuggle an undeclared field edit through.
    for op in mask_external_reject_ops(&ids) {
        let err = h.try_edit(set, candidate, op).unwrap_err();
        assert!(
            matches!(
                err,
                VariantError::Unsupported { .. } | VariantError::ScopeViolation { .. }
            ),
            "mask-external op must reject typed, got {err:?}"
        );
    }

    // Scope-entity removal fails closed through the shared
    // post-edit confinement (the automation lane created above is
    // scope-owned; removing it is unsupported this slice).
    let err = h
        .try_edit(set, candidate, ProjectOp::RemoveAutomationLane { lane })
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "scope-entity removal must reject Unsupported, got {err:?}"
    );

    // Live is bitwise untouched — shared context and every
    // mask-external field.
    assert_eq!(h.controller.live().control, live_before.control);
    assert_eq!(
        serde_json::to_value(&h.controller.live().routing).unwrap(),
        serde_json::to_value(&live_before.routing).unwrap()
    );
}

/// `mask = {Fader, Pan, Inserts}` (no `Mute`/`Solo`): channel fields
/// gate independently — `SetStripMute` rejects while fader/pan and
/// the whole insert surface stay editable.
#[test]
fn cb_strip_02_inserts_and_channel_fields_gate_independently() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(
        &ids,
        &[StripField::Fader, StripField::Pan, StripField::Inserts],
    );
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);
    let live_before = h.controller.live().clone();

    // Declared channel + insert surface: fader/pan/fx params,
    // fader automation + modulation, an in-chain insert and a
    // processor-owned modulation source.
    scoped_base_and_param_edits(&mut h, set, candidate, &ids);
    scoped_automation_edit(&mut h, set, candidate, &ids, &live_before);
    scoped_modulation_edit(&mut h, set, candidate, &ids, &live_before);
    scoped_processor_modulation_edit(&mut h, set, candidate, &ids);
    let inserted = insert_mid_chain_processor(&mut h, set, candidate, &ids);

    let cand = h.candidate(set, candidate);
    let pan = strip_channel_param(&cand, ids.strip, ParameterRole::Pan);
    assert_eq!(cand.control.targets[&ids.param_fader].base, 0.3);
    assert_eq!(cand.control.targets[&pan].base, 0.7);
    assert_eq!(cand.control.targets[&ids.param_fx1].base, 0.9);
    let chain = &cand.routing.chains[&ids.chain].processors;
    let pos_new = chain.iter().position(|p| *p == inserted).unwrap();
    assert!(
        chain.iter().position(|p| *p == ids.fx1).unwrap() < pos_new
            && pos_new < chain.iter().position(|p| *p == ids.fx2).unwrap()
    );
    assert!(cand.plugin_instances.instances.contains_key(&inserted));
    assert!(
        !h.controller
            .live()
            .routing
            .processors
            .contains_key(&inserted)
    );

    // Undeclared bool fields and out-of-scope control entities reject
    // typed; chain structure ops stay Unsupported.
    for op in undeclared_field_reject_ops(h.controller.live(), &ids) {
        let err = h.try_edit(set, candidate, op).unwrap_err();
        assert!(
            matches!(
                err,
                VariantError::Unsupported { .. } | VariantError::ScopeViolation { .. }
            ),
            "out-of-mask op must reject typed, got {err:?}"
        );
    }
    assert_eq!(h.controller.live().control, live_before.control);
}

/// Ops a `{Fader, Pan, Inserts}` mask must reject typed — undeclared
/// bool fields, foreign-strip and undeclared-target control writes,
/// and chain-structure/foreign-insert ops.
fn undeclared_field_reject_ops(project: &Project, ids: &StripIds) -> Vec<ProjectOp> {
    vec![
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
        ProjectOp::SetStripSolo {
            strip_id: ids.strip,
            soloed: true,
        },
        ProjectOp::SetParameterBase {
            target: ids.param_mute,
            value: 1.0,
        },
        ProjectOp::SetParameterBase {
            target: strip_channel_param(project, ids.key_strip, ParameterRole::Fader),
            value: 0.1,
        },
        ProjectOp::ReorderProcessor {
            chain_id: ids.chain,
            processor_id: ids.fx2,
            new_index: 0,
        },
        ProjectOp::RemoveProcessor {
            processor_id: ids.fx2,
        },
        ProjectOp::InsertProcessor {
            strip_id: ids.key_strip,
            index: 0,
            name: "foreign".into(),
            binding: plugin_binding("plug-x"),
            sidechain_input: false,
        },
    ]
}

/// Two independent mask candidates — `{Mute}` on the track strip and
/// `{Inserts}` on the bus strip — edit, adopt and stay isolated:
/// each adopt touches only its declared scope.
#[test]
fn cb_strip_02_independent_masks_edit_and_adopt_isolated() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);

    let forked_a = h.fork(scope_of(&ids, &[StripField::Mute]));
    let forked_b = h.fork(bus_scope_of(&ids, &[StripField::Inserts]));
    let (set_a, cand_a) = (forked_a.variant_set, forked_a.candidate);
    let (set_b, cand_b) = (forked_b.variant_set, forked_b.candidate);

    h.edit(
        set_a,
        cand_a,
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
    );
    h.edit(
        set_b,
        cand_b,
        ProjectOp::InsertProcessor {
            strip_id: ids.bus_strip,
            index: 0,
            name: "bus-fx".into(),
            binding: plugin_binding("plug-bus"),
            sidechain_input: false,
        },
    );
    let bus_inserted = h.candidate(set_b, cand_b).routing.chains[&ids.bus_chain]
        .processors
        .iter()
        .copied()
        .next()
        .unwrap();

    // Adopt A: only the track strip's mute target moves.
    let receipt = h.adopt(set_a, cand_a, &scope_of(&ids, &[StripField::Mute]));
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(
        h.controller.live().control.targets[&ids.param_mute].base,
        1.0
    );
    assert!(
        h.controller.live().routing.chains[&ids.bus_chain]
            .processors
            .is_empty(),
        "track-strip adopt must not touch the bus chain"
    );
    assert_eq!(
        h.controller.live().control.targets[&ids.param_fader].base,
        1.0,
        "undeclared fader field stays at its seeded unity"
    );

    // Adopt B: only the bus strip's insert chain moves.
    let receipt = h.adopt(set_b, cand_b, &bus_scope_of(&ids, &[StripField::Inserts]));
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    let live = h.controller.live();
    assert!(
        live.routing.chains[&ids.bus_chain]
            .processors
            .contains(&bus_inserted)
    );
    assert!(live.plugin_instances.instances.contains_key(&bus_inserted));
    assert_eq!(live.control.targets[&ids.param_mute].base, 1.0);
    assert_eq!(
        live.routing.chains[&ids.chain].processors,
        vec![ids.fx1, ids.fx2],
        "bus-strip adopt must not touch the track chain"
    );
}

/// Live-side LFO source for the assignment-containment tests —
/// an ordinary op applied before fork, outside any candidate scope.
fn live_lfo_source(c: &mut ProjectController, owner: ControlOwner) -> ModulationSourceId {
    let before: BTreeSet<ModulationSourceId> = c.live().control.sources.keys().copied().collect();
    apply_op(
        c,
        ProjectOp::AddModulationSource {
            owner,
            kind: SourceKind::Lfo {
                rate: BeatPoint::new(2, 1).unwrap(),
                shape: LfoShape::Sine,
                bipolar: true,
            },
        },
    );
    *c.live()
        .control
        .sources
        .keys()
        .find(|id| !before.contains(id))
        .unwrap()
}

/// Live-side assignment `source → target` — same pre-fork plumbing.
fn live_assignment(
    c: &mut ProjectController,
    source: ModulationSourceId,
    target: ParameterTargetId,
) -> ModulationAssignmentId {
    live_assignment_depth(c, source, target, None)
}

/// `live_assignment` with an explicit hierarchical depth driver.
fn live_assignment_depth(
    c: &mut ProjectController,
    source: ModulationSourceId,
    target: ParameterTargetId,
    depth_source: Option<ParameterTargetId>,
) -> ModulationAssignmentId {
    let before: BTreeSet<ModulationAssignmentId> =
        c.live().control.assignments.keys().copied().collect();
    apply_op(
        c,
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth: 0.4,
            depth_source,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    *c.live()
        .control
        .assignments
        .keys()
        .find(|id| !before.contains(id))
        .unwrap()
}

/// CB-STRIP-02 containment: a modulation assignment's audible effect
/// lands on its `target`, so source-side affiliation alone never
/// makes it scope-owned. A strip-owned LFO driving the *undeclared*
/// Fader field must reject `Set`/`Remove` typed, while the same
/// source onto the declared `Pan` target stays editable — otherwise
/// a candidate edit (and its adopt transplant) would reach a
/// mask-external parameter. (`Mute` itself cannot host modulation —
/// its target is Boolean-kind — so `Pan` is the declared continuous
/// endpoint.)
#[test]
fn cb_strip_02_assignment_effect_target_gates_ownership() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    // First-use `Pan` target — declared by the mask, Continuous-kind.
    apply_op(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "Pan".into(),
            owner: ControlOwner::Strip(ids.strip),
            role: ParameterRole::Pan,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let pan = strip_channel_param(c.live(), ids.strip, ParameterRole::Pan);
    let source = live_lfo_source(&mut c, ControlOwner::Strip(ids.strip));
    let escape = live_assignment(&mut c, source, ids.param_fader);
    let legit = live_assignment(&mut c, source, pan);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Mute, StripField::Pan]);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);

    for op in [
        ProjectOp::SetModulationAssignment {
            id: escape,
            depth: Some(0.9),
            enabled: None,
        },
        ProjectOp::RemoveModulationAssignment { id: escape },
    ] {
        let err = h.try_edit(set, candidate, op).unwrap_err();
        assert!(
            matches!(err, VariantError::ScopeViolation { .. }),
            "assignment escaping to an undeclared field must reject, got {err:?}"
        );
    }

    h.edit(
        set,
        candidate,
        ProjectOp::SetModulationAssignment {
            id: legit,
            depth: Some(0.9),
            enabled: Some(false),
        },
    );
    let cand = h.candidate(set, candidate);
    assert_eq!(cand.control.assignments[&legit].depth, 0.9);
    assert!(!cand.control.assignments[&legit].enabled);
}

/// Same containment through the processor closure: under `Inserts`
/// an in-chain modulation source is scope-owned, but an assignment
/// whose effect target is the strip's undeclared channel field is
/// not — editing it would reach outside the mask.
#[test]
fn cb_strip_02_processor_assignment_effect_gates_ownership() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let source = live_lfo_source(&mut c, ControlOwner::Processor(ids.fx1));
    let legit = live_assignment(&mut c, source, ids.param_fx1);
    let escape = live_assignment(&mut c, source, ids.param_fader);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Inserts]);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);

    let err = h
        .try_edit(
            set,
            candidate,
            ProjectOp::SetModulationAssignment {
                id: escape,
                depth: Some(0.9),
                enabled: None,
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::ScopeViolation { .. }),
        "processor-sourced assignment onto an undeclared strip field must reject, got {err:?}"
    );

    h.edit(
        set,
        candidate,
        ProjectOp::SetModulationAssignment {
            id: legit,
            depth: Some(0.9),
            enabled: Some(false),
        },
    );
    let cand = h.candidate(set, candidate);
    assert_eq!(cand.control.assignments[&legit].depth, 0.9);
    assert!(!cand.control.assignments[&legit].enabled);
}

/// `depth_source` is a second parameter endpoint: an assignment whose
/// effect target is declared but whose depth driver is *not* must not
/// be scope-owned — a candidate edit would hang the declared field's
/// modulation depth on an undeclared target. Under `{Fader, Pan}`
/// (no `Inserts`) the fx1 processor param is outside the scope.
#[test]
fn cb_strip_02_assignment_depth_source_gates_ownership() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    apply_op(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "Pan".into(),
            owner: ControlOwner::Strip(ids.strip),
            role: ParameterRole::Pan,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let pan = strip_channel_param(c.live(), ids.strip, ParameterRole::Pan);
    let source = live_lfo_source(&mut c, ControlOwner::Strip(ids.strip));
    let legit = live_assignment_depth(&mut c, source, pan, Some(ids.param_fader));
    let escape = live_assignment_depth(&mut c, source, pan, Some(ids.param_fx1));
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Fader, StripField::Pan]);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);

    for op in [
        ProjectOp::SetModulationAssignment {
            id: escape,
            depth: Some(0.9),
            enabled: None,
        },
        ProjectOp::RemoveModulationAssignment { id: escape },
    ] {
        let err = h.try_edit(set, candidate, op).unwrap_err();
        assert!(
            matches!(err, VariantError::ScopeViolation { .. }),
            "assignment with an undeclared depth driver must reject, got {err:?}"
        );
    }

    h.edit(
        set,
        candidate,
        ProjectOp::SetModulationAssignment {
            id: legit,
            depth: Some(0.9),
            enabled: None,
        },
    );
    let cand = h.candidate(set, candidate);
    assert_eq!(cand.control.assignments[&legit].depth, 0.9);
}

// ===========================================================================
// CB-STRIP-03: typed SemanticPortId boundary inventory — sends,
// sidechains and the main route are enumerated, never rewired
// ===========================================================================

/// The strip's main route (strip out → master in) and the send
/// (strip out → bus-strip in) cross the boundary for *every* mask —
/// strip ports are scope-affiliated. The sidechain feed crosses only
/// when `Inserts` makes the fx2 port a scope object.
#[test]
fn cb_strip_03_boundary_dependencies_typed_and_mask_relative() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let h = Strip::new(c);

    // mask = {Mute}: only strip-port crossings are affiliated.
    let scope = scope_of(&ids, &[StripField::Mute]);
    let deps = StripFieldsAdapter::boundary_dependencies(h.controller.live(), &scope).unwrap();
    assert_eq!(deps.len(), 2, "main route + send only: {deps:?}");
    let route = deps
        .iter()
        .find(|d| d.kind == ConnectionKind::Main)
        .expect("main route dep");
    assert!(route.source_in_scope && !route.destination_in_scope);
    let send_dep = deps
        .iter()
        .find(|d| matches!(d.kind, ConnectionKind::Send(_)))
        .expect("send dep");
    assert_eq!(
        send_dep.connection,
        h_send_connection(h.controller.live(), ids.send)
    );
    assert!(send_dep.source_in_scope && !send_dep.destination_in_scope);

    // mask = {Fader, Inserts}: the fx2 sidechain port becomes a scope
    // object — the external key feed now crosses the boundary with
    // the destination affiliated.
    let scope_inserts = scope_of(&ids, &[StripField::Fader, StripField::Inserts]);
    let deps =
        StripFieldsAdapter::boundary_dependencies(h.controller.live(), &scope_inserts).unwrap();
    assert_eq!(deps.len(), 3, "route + send + sidechain feed: {deps:?}");
    let feed_dep = deps
        .iter()
        .find(|d| d.kind == ConnectionKind::Sidechain)
        .expect("sidechain feed dep");
    assert_eq!(feed_dep.connection, ids.feed);
    assert!(!feed_dep.source_in_scope && feed_dep.destination_in_scope);

    // Bus-strip scope: the send's *destination* port is affiliated —
    // the same connection is a boundary dep from the other side.
    let bus_scope = bus_scope_of(&ids, &[StripField::Inserts]);
    let deps = StripFieldsAdapter::boundary_dependencies(h.controller.live(), &bus_scope).unwrap();
    assert_eq!(deps.len(), 1, "only the send touches bus ports: {deps:?}");
    assert_eq!(deps[0].connection, send_dep.connection);
    assert!(!deps[0].source_in_scope && deps[0].destination_in_scope);
}

fn h_send_connection(project: &Project, send: SendId) -> RoutingConnectionId {
    project.routing.sends[&send].connection
}

/// Ops touching boundary connections, sends, taps and channel maps —
/// plus new external wiring — must reject typed.
fn boundary_wiring_reject_ops(project: &Project, ids: &StripIds) -> Vec<ProjectOp> {
    let strip_out = port_of(
        project,
        PortOwner::Strip(ids.strip),
        PortDirection::Output,
        PortRole::Main,
    );
    let master_in = project
        .routing
        .connections
        .values()
        .find(|conn| conn.source == strip_out && conn.kind == ConnectionKind::Main)
        .map(|conn| conn.destination)
        .expect("strip main route");
    vec![
        // Touching boundary connections directly.
        ProjectOp::RemoveConnection {
            connection_id: ids.feed,
        },
        ProjectOp::SetConnectionEnabled {
            connection_id: ids.feed,
            enabled: false,
        },
        ProjectOp::SetConnectionChannelMap {
            connection_id: ids.feed,
            channel_map: None,
        },
        ProjectOp::SetConnectionEnabled {
            connection_id: h_send_connection(project, ids.send),
            enabled: false,
        },
        // New external wiring: sidechain, duplicate main, send.
        ProjectOp::AddConnection {
            source: port_of(
                project,
                PortOwner::Strip(ids.key_strip),
                PortDirection::Output,
                PortRole::Main,
            ),
            destination: port_of(
                project,
                PortOwner::Processor(ids.fx1),
                PortDirection::Input,
                PortRole::Main,
            ),
            kind: ConnectionKind::Sidechain,
            channel_map: None,
        },
        ProjectOp::AddConnection {
            source: strip_out,
            destination: master_in,
            kind: ConnectionKind::Main,
            channel_map: None,
        },
        ProjectOp::AddSend {
            source: strip_out,
            destination: master_in,
            tap: SendTap::PreFader,
            gain_target: None,
        },
        // Send surface.
        ProjectOp::RemoveSend { send_id: ids.send },
        ProjectOp::SetSendTap {
            send_id: ids.send,
            tap: SendTap::PreFader,
        },
        ProjectOp::SetSendGainTarget {
            send_id: ids.send,
            gain_target: None,
        },
    ]
}

/// Ops touching shared context outside the declared mask — chain
/// structure, bus structure and track monitoring — must reject typed.
fn shared_context_reject_ops(ids: &StripIds) -> Vec<ProjectOp> {
    vec![
        // Chain structure outside candidate ownership.
        ProjectOp::ReorderProcessor {
            chain_id: ids.chain,
            processor_id: ids.fx2,
            new_index: 0,
        },
        ProjectOp::RemoveProcessor {
            processor_id: ids.fx2,
        },
        // Bus/track shared context.
        ProjectOp::ConfigureBus {
            bus_id: ids.bus,
            name: Some("renamed".into()),
            kind: None,
        },
        ProjectOp::AddBus {
            kind: BusKind::Group,
            name: "grp".into(),
        },
        ProjectOp::RemoveBus { bus_id: ids.bus },
        ProjectOp::SetTrackMonitor {
            track: ids.track,
            decl: TrackMonitorDecl {
                lane: None,
                channel_map: MonitorChannelMap::identity(),
                arm: ArmState::Off,
                monitor: MonitorState::Off,
            },
        },
    ]
}

/// Every op that could touch boundary/external wiring — connections,
/// sends, taps, channel maps, bus/track structure, chain structure —
/// rejects typed. Nothing applies, normalizes or rewires.
#[test]
fn cb_strip_03_external_wiring_ops_reject_typed() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Fader, StripField::Inserts]);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);

    for op in boundary_wiring_reject_ops(h.controller.live(), &ids)
        .into_iter()
        .chain(shared_context_reject_ops(&ids))
    {
        let err = h.try_edit(set, candidate, op).unwrap_err();
        assert!(
            matches!(
                err,
                VariantError::Unsupported { .. } | VariantError::ScopeViolation { .. }
            ),
            "external wiring op must reject typed, got {err:?}"
        );
    }

    // Adopt materializes only scoped entities: every connection and
    // send survives byte-identical — no implicit reconnection, no
    // double-mix, no rewiring (CB-STRIP-03/RCP-01).
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fx1,
            value: 0.9,
        },
    );
    let connections_before = h.controller.live().routing.connections.clone();
    let sends_before = serde_json::to_value(&h.controller.live().routing.sends).unwrap();
    let receipt = h.adopt(
        set,
        candidate,
        &scope_of(&ids, &[StripField::Fader, StripField::Inserts]),
    );
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(
        h.controller.live().routing.connections,
        connections_before,
        "adopt must never create or rewire connections"
    );
    assert_eq!(
        serde_json::to_value(&h.controller.live().routing.sends).unwrap(),
        sends_before
    );
    assert_eq!(
        h.controller.live().control.targets[&ids.param_fx1].base,
        0.9
    );
}

/// A boundary feed change on live moves the shared-context digest —
/// the comparison stales instead of reporting false equivalence —
/// while the scope digest stays put.
#[test]
fn cb_strip_03_boundary_change_stales_shared_context_only() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Fader, StripField::Inserts]);
    let forked = h.fork(scope.clone());
    let (set, candidate) = (forked.variant_set, forked.candidate);

    let pin_before = h.render_pin(set, candidate);
    apply_op(
        &mut h.controller,
        ProjectOp::SetConnectionEnabled {
            connection_id: ids.feed,
            enabled: false,
        },
    );
    let deps = StripFieldsAdapter::boundary_dependencies(h.controller.live(), &scope).unwrap();
    assert_eq!(
        deps.len(),
        3,
        "disabled connections still cross the boundary"
    );
    let pin_after = h.render_pin(set, candidate);
    assert_eq!(pin_before.scope_digest, pin_after.scope_digest);
    assert_ne!(
        pin_before.shared_context_digest, pin_after.shared_context_digest,
        "boundary feed changes must stale the comparison context — \
         never a false audio equivalence"
    );
}

// ===========================================================================
// CB-STRIP-04: adopt declared mask only — one CAS/undo, save/reload,
// stale-scope/recording/missing-plugin gates
// ===========================================================================

/// Rich candidate edits across every declared field family.
fn rich_candidate_edits(
    h: &mut Strip,
    set: VariantSetId,
    candidate: BranchId,
    ids: &StripIds,
    live_before: &Project,
) -> ProcessorId {
    h.edit(
        set,
        candidate,
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
    );
    scoped_base_and_param_edits(h, set, candidate, ids);
    scoped_automation_edit(h, set, candidate, ids, live_before);
    scoped_modulation_edit(h, set, candidate, ids, live_before);
    scoped_processor_modulation_edit(h, set, candidate, ids);
    insert_mid_chain_processor(h, set, candidate, ids)
}

#[test]
fn cb_strip_04_adopt_mask_only_one_cas_undo_save_reload() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(
        &ids,
        &[
            StripField::Fader,
            StripField::Pan,
            StripField::Mute,
            StripField::Inserts,
        ],
    );
    let forked = h.fork(scope.clone());
    let (set, adopted_a, candidate_b) = (forked.variant_set, forked.adopted, forked.candidate);

    let live_before = h.controller.live().clone();
    let inserted = rich_candidate_edits(&mut h, set, candidate_b, &ids, &live_before);
    let rev_before = h.controller.live_revision();

    let receipt = h.adopt(set, candidate_b, &scope);
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(receipt.project.undo_steps, 1);
    assert!(receipt.project.committed);
    assert_eq!(receipt.after_revision.0, rev_before.0 + 1);

    assert_adopted_mask_state(h.controller.live(), &ids, inserted);
    assert_shared_state_untouched(h.controller.live(), &live_before, &ids);

    // The previously adopted branch froze as captured content.
    let set_state = &h.controller.live().variant_sets[&set];
    let frozen = &set_state.branches[&adopted_a].content;
    let BranchContentRef::Captured { .. } = frozen else {
        panic!("previously adopted branch must freeze to captured content");
    };

    // One undo restores the exact pre-adopt scope; redo re-applies —
    // candidates survive both directions.
    assert_adopt_undo_redo(&mut h, set, candidate_b, &ids, inserted);

    // Save → reload: variant sets and live content round-trip.
    let mut store = InMemoryStore::new();
    h.controller.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live(), h.controller.live());
    assert!(c2.live().variant_sets.values().any(|s| s.scope
        == scope_of(
            &ids,
            &[
                StripField::Fader,
                StripField::Pan,
                StripField::Mute,
                StripField::Inserts,
            ]
        )));
}

/// The declared mask transplanted onto live: channel bases, the
/// first-use Pan target, insert-chain membership, automation and
/// modulation — and nothing else.
fn assert_adopted_mask_state(live: &Project, ids: &StripIds, inserted: ProcessorId) {
    assert_eq!(live.control.targets[&ids.param_mute].base, 1.0);
    assert_eq!(live.control.targets[&ids.param_fader].base, 0.3);
    let pan = strip_channel_param(live, ids.strip, ParameterRole::Pan);
    assert_eq!(live.control.targets[&pan].base, 0.7);
    assert_eq!(live.control.targets[&ids.param_fx1].base, 0.9);
    let chain = &live.routing.chains[&ids.chain].processors;
    let pos_new = chain.iter().position(|p| *p == inserted).unwrap();
    assert!(chain.iter().position(|p| *p == ids.fx1).unwrap() < pos_new);
    assert!(pos_new < chain.iter().position(|p| *p == ids.fx2).unwrap());
    assert!(
        live.control
            .lanes
            .values()
            .any(|l| l.target == ids.param_fader)
    );
    assert!(
        live.control
            .assignments
            .values()
            .any(|a| a.target == ids.param_fader)
    );
}

/// Undeclared/shared state bitwise unchanged by the adopt —
/// connections, sends, strip records and the sibling strip's seeded
/// channel state all match the pre-edit baseline.
fn assert_shared_state_untouched(live: &Project, before: &Project, ids: &StripIds) {
    assert_eq!(live.control.targets[&ids.param_solo].base, 0.0);
    assert_eq!(live.control.targets[&ids.param_fx2].base, 0.6);
    assert_eq!(live.routing.connections, before.routing.connections);
    assert_eq!(
        serde_json::to_value(&live.routing.sends).unwrap(),
        serde_json::to_value(&before.routing.sends).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&live.routing.strips).unwrap(),
        serde_json::to_value(&before.routing.strips).unwrap()
    );
    let key_fader = strip_channel_param(live, ids.key_strip, ParameterRole::Fader);
    assert_eq!(live.control.targets[&key_fader].base, 1.0);
}

/// One undo restores the exact pre-adopt scope; redo re-applies —
/// candidates survive both directions.
fn assert_adopt_undo_redo(
    h: &mut Strip,
    set: VariantSetId,
    candidate_b: BranchId,
    ids: &StripIds,
    inserted: ProcessorId,
) {
    assert!(h.controller.undo().unwrap());
    assert_eq!(
        h.controller.live().control.targets[&ids.param_mute].base,
        0.0
    );
    assert_eq!(
        h.controller.live().control.targets[&ids.param_fader].base,
        1.0
    );
    assert!(
        !h.controller
            .live()
            .routing
            .processors
            .contains_key(&inserted)
    );
    assert!(h.controller.redo().unwrap());
    assert_eq!(
        h.controller.live().control.targets[&ids.param_mute].base,
        1.0
    );
    assert!(
        h.controller
            .live()
            .routing
            .processors
            .contains_key(&inserted)
    );
    assert_eq!(h.controller.live().variant_sets[&set].adopted, candidate_b);
}

/// CB-STRIP-04 runtime half: a failed runtime publish is a *separate*
/// receipt — the project commit stands, live is never rolled back.
#[test]
fn cb_strip_04_runtime_publish_failure_keeps_project_committed() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Mute]);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
    );

    let failing = FakeRuntimePublish::new(RuntimePublishState::Failed {
        reason: "rt engine offline".into(),
    });
    let receipt = h
        .try_adopt_full(forked.variant_set, forked.candidate, &scope, None, &failing)
        .unwrap();
    assert!(receipt.project.committed);
    assert_eq!(receipt.project.undo_steps, 1);
    assert!(matches!(
        receipt.runtime.state,
        RuntimePublishState::Failed { .. }
    ));
    assert_eq!(
        h.controller.live().control.targets[&ids.param_mute].base,
        1.0
    );
    assert!(h.controller.undo().unwrap());
    assert_eq!(
        h.controller.live().control.targets[&ids.param_mute].base,
        0.0
    );
}

/// CB-STRIP-04 stale gate: a live scoped edit between intent and adopt
/// fails `StaleScope` — no auto-rebase, no partial mutation.
#[test]
fn cb_strip_04_stale_scope_digest_rejects_adopt() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Mute, StripField::Fader]);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fader,
            value: 0.3,
        },
    );
    let intent = h.intent(forked.variant_set, forked.candidate, &scope);
    // Live scoped mutation after the intent was pinned — refresh only
    // the project-revision pin so the scope-digest gate is exercised.
    apply_op(
        &mut h.controller,
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
    );
    let mut intent = intent;
    intent.expected_project_revision = h.controller.live_revision();
    let live_before = h.controller.live().clone();
    let err = adopt_branch_adapted(
        &mut h.controller,
        &h.adapter,
        &mut h.store,
        &AdoptSources {
            assets: None,
            plugins: None,
            runtime: &NoopRuntimePublish,
        },
        &intent,
    )
    .unwrap_err();
    assert!(
        matches!(err, VariantError::StaleScope { .. }),
        "stale scope digest must reject typed, got {err:?}"
    );
    assert_eq!(*h.controller.live(), live_before);
}

/// CB-STRIP-04 stale gate, chain-order variant: `ProcessorChain` is
/// scope-owned under `Inserts` — its `processors` list is the only
/// carrier of DSP order — so a live reorder moves the scope digest
/// and adopt fails `StaleScope` instead of positionally merging
/// candidate inserts into an order nobody authored.
#[test]
fn cb_strip_04_live_chain_reorder_stale_scope_rejects_adopt() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Inserts]);
    let forked = h.fork(scope.clone());
    insert_mid_chain_processor(&mut h, forked.variant_set, forked.candidate, &ids);
    let intent = h.intent(forked.variant_set, forked.candidate, &scope);
    // Live reorder of the scoped chain after the intent was pinned —
    // refresh only the project-revision pin so the scope gate is
    // exercised (same convention as the entity-value stale test).
    apply_op(
        &mut h.controller,
        ProjectOp::ReorderProcessor {
            chain_id: ids.chain,
            processor_id: ids.fx2,
            new_index: 0,
        },
    );
    let mut intent = intent;
    intent.expected_project_revision = h.controller.live_revision();
    let live_before = h.controller.live().clone();
    let err = synvelis_project::variant::adopt_branch_adapted(
        &mut h.controller,
        &h.adapter,
        &mut h.store,
        &AdoptSources {
            assets: None,
            plugins: None,
            runtime: &NoopRuntimePublish,
        },
        &intent,
    )
    .unwrap_err();
    assert!(
        matches!(err, VariantError::StaleScope { .. }),
        "live chain reorder must stale the scope digest, got {err:?}"
    );
    assert_eq!(*h.controller.live(), live_before);
}

/// Injects an open recording session + capture stream on `lane`
/// directly into project state (the recording registry is runtime
/// data with no `ProjectOp` — the cb02/cb04/cb05 fixture convention).
fn with_open_recording(c: &ProjectController, lane: LaneId) -> ProjectController {
    let mut p = c.live().clone();
    let epoch = ClockEpochId::new(9003);
    p.clock_epochs.insert(
        epoch,
        ClockEpoch {
            id: epoch,
            description: "mock".into(),
        },
    );
    let session = RecordingSessionId::new(9001);
    p.recording.sessions.insert(
        session,
        RecordingSession {
            id: session,
            name: "s".into(),
            state: SessionState::Open,
        },
    );
    let stream = CaptureStreamId::new(9002);
    p.recording.streams.insert(
        stream,
        CaptureStream {
            id: stream,
            session_id: session,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    ProjectController::new(p, Box::new(CounterIdGenerator::new(10_000)))
}

/// CB-STRIP-04 recording gate: an open session on the strip's owning
/// lane rejects `RecordingActive` before any mutation.
#[test]
fn cb_strip_04_adopt_while_recording_rejects_typed() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let lane = ids.lane;
    let c = with_open_recording(&c, lane);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Mute]);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
    );
    let live_before = h.controller.live().clone();
    let rev_before = h.controller.live_revision();

    let err = h
        .try_adopt(forked.variant_set, forked.candidate, &scope)
        .unwrap_err();
    assert!(
        matches!(err, VariantError::RecordingActive { .. }),
        "open recording session on the owning lane must reject RecordingActive, got {err:?}"
    );
    assert_eq!(*h.controller.live(), live_before);
    assert_eq!(h.controller.live_revision(), rev_before);
}

/// Plugin-availability probes for the missing-plugin adopt gate.
struct NoPlugins;
impl PluginAvailability for NoPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        false
    }
}

struct YesPlugins;
impl PluginAvailability for YesPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        true
    }
}

/// Plugin availability is a typed adopt gate (CB-STRIP-04): an
/// inserted processor whose binding the probe reports unavailable
/// fails closed BEFORE any mutation; reporting it available adopts —
/// project truth does not depend on runtime/plugin presence beyond
/// the typed gate.
#[test]
fn cb_strip_04_missing_plugin_gate_rejects_before_mutation() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Inserts]);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::InsertProcessor {
            strip_id: ids.strip,
            index: 0,
            name: "fx-head".into(),
            binding: plugin_binding("plug-ghost"),
            sidechain_input: false,
        },
    );

    let live_before_adopt = h.controller.live().clone();
    let err = h
        .try_adopt_full(
            forked.variant_set,
            forked.candidate,
            &scope,
            Some(&NoPlugins),
            &NoopRuntimePublish,
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::MissingPlugin { .. }),
        "unavailable scoped plugin must reject MissingPlugin, got {err:?}"
    );
    assert_eq!(*h.controller.live(), live_before_adopt);

    let receipt = h
        .try_adopt_full(
            forked.variant_set,
            forked.candidate,
            &scope,
            Some(&YesPlugins),
            &NoopRuntimePublish,
        )
        .unwrap();
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(
        h.controller.live().routing.chains[&ids.chain]
            .processors
            .len(),
        3
    );
}

// ===========================================================================
// CB-STRIP-05: shared model regressions — Bus/Return/Main/Solo/
// Automation behavior outside the variant path is unchanged (the
// SF-06 oracle suite covers the routing model itself)
// ===========================================================================

#[test]
fn cb_strip_05_shared_strip_model_unchanged_outside_scope() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Mute]);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);
    h.edit(
        set,
        candidate,
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
    );

    // Live-side strip ops on *other* strips keep working normally —
    // the variant scope never gates shared project edits.
    let key_fader = strip_channel_param(h.controller.live(), ids.key_strip, ParameterRole::Fader);
    apply_op(
        &mut h.controller,
        ProjectOp::SetParameterBase {
            target: key_fader,
            value: 0.8,
        },
    );
    apply_op(
        &mut h.controller,
        ProjectOp::SetStripSolo {
            strip_id: ids.bus_strip,
            soloed: true,
        },
    );
    let bus_solo = strip_channel_param(h.controller.live(), ids.bus_strip, ParameterRole::Solo);
    assert_eq!(h.controller.live().control.targets[&bus_solo].base, 1.0);

    // Adopt touches only the declared mask: the key-strip fader edit
    // (shared context made after fork) survives untouched, the bus
    // solo stays, sends/connections stay.
    let receipt = h.adopt(set, candidate, &scope_of(&ids, &[StripField::Mute]));
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    let live = h.controller.live();
    assert_eq!(live.control.targets[&ids.param_mute].base, 1.0);
    assert_eq!(live.control.targets[&key_fader].base, 0.8);
    assert_eq!(live.control.targets[&bus_solo].base, 1.0);
    assert_eq!(live.routing.sends.len(), 1);
    assert_eq!(live.routing.connections.len(), 4);
}

// ===========================================================================
// CB-STRIP-06 + CB-STRIP-RCP-01: plugin-instance/state honesty,
// render-pin identity, capability truth, captured-payload immutability
// ===========================================================================

/// Scripted plugin-state port (`plugin_state.rs` test conventions):
/// per-processor binding + scripted capture outcomes.
#[derive(Default)]
struct MockPort {
    bindings: BTreeMap<ProcessorId, PluginStateBinding>,
    captures: BTreeMap<ProcessorId, Result<PluginStateCapture, PluginStateCaptureError>>,
}

impl MockPort {
    fn bind(&mut self, processor: ProcessorId, quality: StateCaptureQuality) {
        self.bindings.insert(
            processor,
            PluginStateBinding {
                descriptor: "plug-fx".into(),
                version: "1.0".into(),
                capability_revision: 1,
                host_runtime_instance: "mock-host".into(),
                graph_generation: 3,
            },
        );
        self.captures.insert(
            processor,
            Ok(PluginStateCapture {
                blob: b"opaque-state".to_vec(),
                achieved: quality,
                observation: CaptureObservation {
                    processing: quality == StateCaptureQuality::BestEffort,
                    stable_boundary: quality != StateCaptureQuality::BestEffort,
                },
                detail: None,
            }),
        );
    }
}

impl PluginStatePort for MockPort {
    fn describe(&self, processor: ProcessorId) -> Option<PluginStateBinding> {
        self.bindings.get(&processor).cloned()
    }

    fn capture(
        &mut self,
        processor: ProcessorId,
        _mode: CaptureMode,
    ) -> Result<PluginStateCapture, PluginStateCaptureError> {
        self.captures
            .get(&processor)
            .cloned()
            .unwrap_or(Err(PluginStateCaptureError::Failed(
                PluginStateFailureKind::Unavailable,
            )))
    }

    fn restore(
        &mut self,
        _processor: ProcessorId,
        _bytes: &[u8],
    ) -> Result<(), PluginStateCaptureError> {
        Ok(())
    }
}

/// Per-processor state assessment over the insert chain: one verdict
/// per chain processor in chain order, quality/failure recorded
/// verbatim — never upgraded, never normalized to success. A mask
/// without `Inserts` still resolves the chain for assessment (the
/// read seam is mask-independent; only the *edit* surface is gated).
#[test]
fn cb_strip_06_insert_state_assessment_verbatim() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let scope = scope_of(&ids, &[StripField::Inserts]);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);
    let inserted = insert_mid_chain_processor(&mut h, set, candidate, &ids);
    let cand = h.candidate(set, candidate);

    let mut port = MockPort::default();
    port.bind(ids.fx1, StateCaptureQuality::Coordinated);
    port.bind(ids.fx2, StateCaptureQuality::Coordinated);
    port.bind(inserted, StateCaptureQuality::BestEffort);
    let assessments = StripFieldsAdapter::assess_insert_state(
        &mut port,
        &cand,
        ids.strip,
        CaptureMode::Coordinated,
    )
    .unwrap();
    assert_eq!(assessments.len(), 3);
    let by_proc: BTreeMap<ProcessorId, &InstrumentStateAssessment> =
        assessments.iter().map(|a| (a.processor, a)).collect();
    for p in [ids.fx1, ids.fx2] {
        let a = by_proc[&p];
        assert!(matches!(
            a.outcome,
            PluginStateOutcome::Captured { quality, .. } if quality == StateCaptureQuality::Coordinated
        ));
        assert!(a.exact_eligible());
    }
    let degraded = by_proc[&inserted];
    assert!(matches!(
        degraded.outcome,
        PluginStateOutcome::Captured { quality, .. } if quality == StateCaptureQuality::BestEffort
    ));
    assert!(
        !degraded.exact_eligible(),
        "BestEffort must never be promoted to exact-eligible"
    );
    // Typed failure stays typed — never normalized to success.
    port.captures.insert(
        ids.fx1,
        Err(PluginStateCaptureError::Failed(
            PluginStateFailureKind::Timeout,
        )),
    );
    let assessments = StripFieldsAdapter::assess_insert_state(
        &mut port,
        &cand,
        ids.strip,
        CaptureMode::Coordinated,
    )
    .unwrap();
    let failed = assessments.iter().find(|a| a.processor == ids.fx1).unwrap();
    assert!(matches!(
        failed.outcome,
        PluginStateOutcome::Failed { kind } if kind == PluginStateFailureKind::Timeout
    ));
    assert!(!failed.exact_eligible());
}

/// Honest capability report for a declared mask — every Supported
/// verdict has an accepted edit path, every Unsupported verdict
/// rejects typed — plus the narrower-`{Mute}` truth probe.
fn assert_mask_capabilities(adapter: &StripFieldsAdapter, fields: &[StripField]) {
    let caps = adapter.capabilities(&mask(fields));
    assert_eq!(caps.fader, ProcessingCapability::Supported);
    assert_eq!(caps.pan, ProcessingCapability::Supported);
    assert_eq!(caps.mute, ProcessingCapability::Supported);
    assert_eq!(caps.inserts, ProcessingCapability::Supported);
    assert_eq!(caps.solo, ProcessingCapability::Unsupported);
    assert_eq!(caps.automation_edit, ProcessingCapability::Supported);
    assert_eq!(caps.modulation_edit, ProcessingCapability::Supported);
    assert_eq!(caps.entity_remove, ProcessingCapability::Unsupported);
    assert_eq!(caps.routing, ProcessingCapability::Unsupported);
    assert_eq!(caps.plugin_rebind, ProcessingCapability::Unsupported);
    assert_eq!(caps.preset_select, ProcessingCapability::Unsupported);
    // A narrower mask reports narrower truth — `{Mute}` only.
    let narrow = adapter.capabilities(&mask(&[StripField::Mute]));
    assert_eq!(narrow.fader, ProcessingCapability::Unsupported);
    assert_eq!(narrow.inserts, ProcessingCapability::Unsupported);
    assert_eq!(narrow.mute, ProcessingCapability::Supported);
}

/// CB-STRIP-RCP-01: render-pin identity independence, honest
/// capability report matching enforcement, captured-payload
/// immutability and semantic diff surface.
#[test]
fn cb_strip_rcp_01_render_identity_capability_diff() {
    let mut c = controller();
    let ids = strip_fixture(&mut c);
    let mut h = Strip::new(c);
    let fields = [
        StripField::Fader,
        StripField::Pan,
        StripField::Mute,
        StripField::Inserts,
    ];
    let scope = scope_of(&ids, &fields);
    let forked = h.fork(scope);
    let (set, adopted, candidate) = (forked.variant_set, forked.adopted, forked.candidate);

    // Honest capability report: every Supported verdict has an
    // accepted edit path; every Unsupported verdict rejects typed.
    assert_mask_capabilities(&h.adapter, &fields);

    // The captured payload digest is the store key — live shared
    // edits must never mutate candidate bytes.
    let content = h.controller.live().variant_sets[&set].branches[&candidate]
        .content
        .clone();
    let BranchContentRef::Captured { payload_digest, .. } = &content else {
        panic!("candidate must be captured content");
    };
    let digest_at_fork = *payload_digest;
    let key_fader = strip_channel_param(h.controller.live(), ids.key_strip, ParameterRole::Fader);
    apply_op(
        &mut h.controller,
        ProjectOp::SetParameterBase {
            target: key_fader,
            value: 0.77,
        },
    );
    let content_after = h.controller.live().variant_sets[&set].branches[&candidate]
        .content
        .clone();
    let BranchContentRef::Captured {
        payload_digest: after,
        ..
    } = &content_after
    else {
        panic!("candidate must stay captured");
    };
    assert_eq!(
        *after, digest_at_fork,
        "captured payload bytes are immutable"
    );

    // Render pins: adopted (A) and candidate share the same
    // shared-context identity; only the scoped digest diverges after
    // a candidate edit.
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fader,
            value: 0.3,
        },
    );
    let pin_a = h.render_pin(set, adopted);
    let pin_b = h.render_pin(set, candidate);
    assert_eq!(
        pin_a.shared_context_digest, pin_b.shared_context_digest,
        "both candidates must see the same shared routing context"
    );
    assert_ne!(
        pin_a.scope_digest, pin_b.scope_digest,
        "the scoped candidate edit must move only the candidate identity"
    );

    // Semantic diff: the scoped edit surfaces as a typed change —
    // project truth, never a signal/render substitution.
    let cs = compare(&h.controller.live().clone(), &h.candidate(set, candidate));
    let param_changes: Vec<&SemanticChange> = cs
        .changes
        .iter()
        .filter(|ch| ch.object.kind == ObjectKind::ParameterTarget)
        .collect();
    assert!(param_changes.iter().any(|ch| {
        ch.object.stable_id == ids.param_fader.to_tagged() && ch.kind == ChangeKind::Modified
    }));
}

// ---------- diff harness (cb04/cb05 snap_manual/compare conventions) ----

fn snap_manual(project: Project, schema_version: u32, tag: &str) -> ImmutableSnapshot {
    let bytes = serde_json::to_vec(&project).unwrap();
    let mut material = tag.as_bytes().to_vec();
    material.extend_from_slice(&schema_version.to_le_bytes());
    material.extend_from_slice(&bytes);
    ImmutableSnapshot {
        identity: SnapshotIdentity {
            project_id: project.project_id,
            source: SnapshotOrigin::Live,
            locator: FixedSnapshotLocator::Live {
                project_id: project.project_id,
                live_revision: ProjectRevision(0),
            },
            digest: ContentDigest::sha256(&material),
            schema_version,
            projection_version: SCHEMA_VERSION,
            capture_generation: 0,
        },
        origin: SnapshotOrigin::Live,
        projection: ReadOnlyProjectProjection::new(schema_version, project),
        validation: SnapshotValidation::Valid,
        assets: AssetAvailabilityManifest::default(),
        opaque_state: Vec::new(),
        completeness: SnapshotCompleteness::Complete,
    }
}

fn compare(before: &Project, after: &Project) -> synvelis_project::diff::ChangeSet {
    let a = snap_manual(before.clone(), SCHEMA_VERSION, "before");
    let b = snap_manual(after.clone(), SCHEMA_VERSION, "after");
    compare_snapshots(
        &a,
        &b,
        &ComparisonSpec::new(a.identity.clone(), b.identity.clone()),
        &SnapshotCancel::new(),
    )
    .unwrap()
}
