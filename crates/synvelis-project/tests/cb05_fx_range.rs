//! CB-05 Creative Branch FX processing-range adapter fixtures
//! (synvelis#282).
//!
//! Covers the real `ProcessingRangeAdapter` headlessly: stable
//! semantic `ProcessorId` boundaries (chain entry and arbitrary
//! processor boundary → chain end), typed graph ownership with
//! shared upstream/downstream, processor-control + in-range insert
//! confinement, boundary-dependency detection with fail-closed
//! external-wiring ops, one-CAS adopt with undo/save-reload and
//! runtime-publish receipt separation, recording-active and
//! stale-scope gates, and per-candidate plugin-instance/state
//! honesty. Real OS / plugin / RT / PCM lanes are `NotRun` —
//! recorded in `evidence/sv282/`.

use std::collections::BTreeMap;

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
use synvelis_project::plugin_state::{
    CaptureMode, CaptureObservation, PluginStateBinding, PluginStateCapture,
    PluginStateCaptureError, PluginStateFailureKind, PluginStateOutcome, PluginStatePort,
    StateCaptureQuality,
};
use synvelis_project::project::Project;
use synvelis_project::recording_data::{CaptureStream, ClockEpoch, RecordingSession, SessionState};
use synvelis_project::routing::{
    ConnectionKind, PluginRef, PortDirection, PortOwner, PortRole, ProcessorBinding, SendTap,
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
    NoopRuntimePublish, ProcessingCapability, ProcessingRangeAdapter, ProcessingRangeRef,
    RuntimePublishState, VariantError, VariantScope, adopt_branch_adapted, decode_candidate,
    edit_branch_op, fork_variant_set, scope_digest,
};
use synvelis_project::{
    BranchId, CaptureStreamId, ChainId, ClockEpochId, LaneId, ParameterTargetId, ProcessorId,
    ProjectId, ProjectOp, ProjectRevision, RecordingSessionId, RenderBranchTarget,
    RoutingConnectionId, SemanticPortId, StripId, VariantSetId, is_issuable_raw,
    resolve_render_project,
};

// ---------- shared harness (cb04_instrument_slot.rs conventions) ----------

struct Fx {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    resolver: SnapshotResolver,
    adapter: ProcessingRangeAdapter,
}

impl Fx {
    fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            resolver: SnapshotResolver::new(),
            adapter: ProcessingRangeAdapter::new(),
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

// ---------- fixture: track strip FX chain + sibling key strip ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb05-fixture"),
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

struct FxIds {
    lane: LaneId,
    strip: StripId,
    chain: ChainId,
    /// Upstream processor before the range — shared context witness.
    pre: ProcessorId,
    /// Range processors in chain order; `fx3` is the chain tail.
    fx1: ProcessorId,
    fx2: ProcessorId,
    fx3: ProcessorId,
    /// Sibling strip (empty chain) — key-source and foreign-insert
    /// witness, never scope-owned.
    key_strip: StripId,
    param_pre: ParameterTargetId,
    param_fx1: ParameterTargetId,
    param_fx2: ParameterTargetId,
    param_fx3: ParameterTargetId,
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

/// Track → lane → strip → chain `[pre, fx1, fx2, fx3]` where
/// `fx1..=fx3` is the declared range (end = chain tail) and `pre` is
/// shared upstream. `fx2` carries a sidechain input port so the
/// boundary-dependency fixture can wire an external key feed.
/// A sibling track strip with an empty chain witnesses foreign
/// inserts and serves as the sidechain key source.
fn fx_fixture(c: &mut ProjectController) -> FxIds {
    apply_op(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    let chain = c.live().routing.strips[&strip].chain;
    for (index, name, desc, sidechain) in [
        (0usize, "pre", "plug-pre", false),
        (1, "fx1", "plug-a", false),
        (2, "fx2", "plug-b", true),
        (3, "fx3", "plug-c", false),
    ] {
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
    let [pre, fx1, fx2, fx3] = ordered[..] else {
        panic!("fixture chain must hold exactly four processors");
    };

    // Sibling strip: empty chain, main output already routed to
    // master — shared context only.
    apply_op(c, ProjectOp::AddTrack { name: "key".into() });
    let key_track = *c.live().tracks.keys().find(|t| **t != track).unwrap();
    apply_op(
        c,
        ProjectOp::AddTrackStrip {
            track_id: key_track,
        },
    );
    let key_strip = c.live().routing.strip_of_track(key_track).unwrap().id;

    let param_pre = add_target(
        c,
        ControlOwner::Processor(pre),
        "pre-gain",
        Some(capability(1)),
        0.4,
    );
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
    let param_fx3 = add_target(
        c,
        ControlOwner::Processor(fx3),
        "fx3-mix",
        Some(capability(9)),
        0.7,
    );
    FxIds {
        lane,
        strip,
        chain,
        pre,
        fx1,
        fx2,
        fx3,
        key_strip,
        param_pre,
        param_fx1,
        param_fx2,
        param_fx3,
    }
}

fn scope_of(ids: &FxIds) -> VariantScope {
    VariantScope::ProcessingRange {
        range: ProcessingRangeRef {
            chain: ids.chain,
            start: ids.fx1,
            end: ids.fx3,
        },
    }
}

fn range_ref(ids: &FxIds) -> ProcessingRangeRef {
    ProcessingRangeRef {
        chain: ids.chain,
        start: ids.fx1,
        end: ids.fx3,
    }
}

/// The port with `owner`/`direction`/`role` — semantic-port lookup for
/// wiring fixtures (sidechain key feed).
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
        .map_or_else(|| panic!("no {owner:?} {direction:?} {role:?} port"), |p| p.id)
}

/// Wires `key_strip`'s main output into `fx2`'s sidechain input — the
/// external key feed that crosses the declared range boundary.
fn wire_sidechain_feed(c: &mut ProjectController, ids: &FxIds) -> RoutingConnectionId {
    let key_out = port_of(
        c.live(),
        PortOwner::Strip(ids.key_strip),
        PortDirection::Output,
        PortRole::Main,
    );
    let fx2_sc = port_of(
        c.live(),
        PortOwner::Processor(ids.fx2),
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
    *c.live().routing.connections.keys().next_back().unwrap()
}

// ===========================================================================
// CB-FX-01: chain entry and arbitrary processor boundary → chain end
// ===========================================================================

/// CB-FX-01: fork two candidates — chain entry → tail and an arbitrary
/// processor boundary → tail. The engine's `VariantSet` holds
/// adopted+candidate; the two *scopes* are exercised on separate
/// projects because overlapping ranges conflict by design (CB-R-03-4).
/// Boundary legality is proven by `validate_scope`: stable
/// `ProcessorId`s resolve through `range_processors`.
#[test]
fn cb_fx_01_entry_and_mid_boundary_fork_to_chain_end() {
    // Mid boundary → chain end.
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);
    let forked = h.fork(scope_of(&ids));
    let cand = h.candidate(forked.variant_set, forked.candidate);
    // Candidate is a full clone carrying the whole chain.
    assert_eq!(cand.routing.chains[&ids.chain].processors.len(), 4);

    // Chain entry → chain end on a fresh project (the two scopes
    // overlap, so they cannot coexist in one VariantSet namespace —
    // `scopes_conflict` would reject the second fork).
    let mut c2 = controller();
    let ids2 = fx_fixture(&mut c2);
    let mut h2 = Fx::new(c2);
    let entry_scope = VariantScope::ProcessingRange {
        range: ProcessingRangeRef {
            chain: ids2.chain,
            start: ids2.pre,
            end: ids2.fx3,
        },
    };
    let forked2 = h2.fork(entry_scope);
    let cand2 = h2.candidate(forked2.variant_set, forked2.candidate);
    assert_eq!(cand2.routing.chains[&ids2.chain].processors.len(), 4);

    // Overlapping scopes on one project conflict — never silently
    // double-owned.
    let err = h2
        .try_fork(VariantScope::ProcessingRange {
            range: ProcessingRangeRef {
                chain: ids2.chain,
                start: ids2.fx2,
                end: ids2.fx3,
            },
        })
        .unwrap_err();
    assert!(
        matches!(err, VariantError::ScopeConflict { .. }),
        "overlapping processing ranges must conflict, got {err:?}"
    );
}

/// Boundary legality matrix: `end` must be the chain tail (initial
/// contract — arbitrary mid-chain ends are deferred `Unsupported`),
/// `start`/`end` must resolve inside one chain and stay ordered —
/// never ordinal-only, never silently re-anchored.
#[test]
fn cb_fx_01_boundary_validation_fail_closed() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);

    // Mid-chain end: schema-representable but outside the initial
    // boundary contract → typed Unsupported, not a silent widen.
    let mid_end = VariantScope::ProcessingRange {
        range: ProcessingRangeRef {
            chain: ids.chain,
            start: ids.fx1,
            end: ids.fx2,
        },
    };
    let err = h.try_fork(mid_end).unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "mid-chain end must reject Unsupported, got {err:?}"
    );

    // Reversed order: start after end in chain order → Validation.
    let reversed = VariantScope::ProcessingRange {
        range: ProcessingRangeRef {
            chain: ids.chain,
            start: ids.fx3,
            end: ids.fx1,
        },
    };
    let err = h.try_fork(reversed).unwrap_err();
    assert!(
        matches!(err, VariantError::Project(_)),
        "reversed range must reject typed, got {err:?}"
    );

    // Deleted/unknown processor identity → NotFound — semantic IDs,
    // never ordinals: there is no position-based fallback.
    let missing = VariantScope::ProcessingRange {
        range: ProcessingRangeRef {
            chain: ids.chain,
            start: ProcessorId::new(999_999),
            end: ids.fx3,
        },
    };
    let err = h.try_fork(missing).unwrap_err();
    assert!(
        matches!(err, VariantError::Project(_)),
        "unknown start must reject typed, got {err:?}"
    );

    // A foreign scope variant is rejected by the adapter's type gate.
    let audio_scope = VariantScope::InstrumentSlot { processor: ids.pre };
    let err = h.adapter.validate_scope(h.controller.live(), &audio_scope);
    assert!(
        matches!(err, Err(VariantError::InvalidScope { .. })),
        "foreign scope must reject InvalidScope, got {err:?}"
    );
}

// ===========================================================================
// CB-FX-02: shared upstream flows to both candidates; B edits stay
// inside the range and never touch A or shared downstream
// ===========================================================================

/// Base-value edit on a mid-range processor plus a new range-owned
/// parameter target on `fx1`.
fn scoped_base_and_param_edits(
    h: &mut Fx,
    set: VariantSetId,
    candidate: BranchId,
    ids: &FxIds,
) {
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fx2,
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

/// Automation lane + region on the `fx3` target — scoped control edit.
fn scoped_automation_edit(
    h: &mut Fx,
    set: VariantSetId,
    candidate: BranchId,
    ids: &FxIds,
    live_before: &Project,
) {
    h.edit(
        set,
        candidate,
        ProjectOp::AddAutomationLane {
            target: ids.param_fx3,
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

/// Modulation source on `fx2` + assignment onto the `fx1` target.
fn scoped_modulation_edit(
    h: &mut Fx,
    set: VariantSetId,
    candidate: BranchId,
    ids: &FxIds,
    live_before: &Project,
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
            target: ids.param_fx1,
            depth: 0.4,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
}

/// `InsertProcessor` strictly inside `start..=end` — chain
/// `[pre, fx1, fx2, fx3]`, range `fx1..=fx3`, index 2 lands inside.
/// Returns the minted processor ID.
fn insert_mid_range_processor(
    h: &mut Fx,
    set: VariantSetId,
    candidate: BranchId,
    ids: &FxIds,
) -> ProcessorId {
    h.edit(
        set,
        candidate,
        ProjectOp::InsertProcessor {
            strip_id: ids.strip,
            index: 2,
            name: "fx-mid".into(),
            binding: plugin_binding("plug-mid"),
            sidechain_input: false,
        },
    );
    h.candidate(set, candidate).routing.chains[&ids.chain]
        .processors
        .iter()
        .copied()
        .find(|p| ![ids.pre, ids.fx1, ids.fx2, ids.fx3].contains(p))
        .unwrap()
}

/// Rich in-range candidate edits shared by CB-FX-02/04/05/06: base
/// edit, a new range-owned parameter, automation lane + region, a
/// modulation source + assignment, and an in-range `InsertProcessor`.
fn rich_candidate_edits(
    h: &mut Fx,
    set: VariantSetId,
    candidate: BranchId,
    ids: &FxIds,
    live_before: &Project,
) -> ProcessorId {
    scoped_base_and_param_edits(h, set, candidate, ids);
    scoped_automation_edit(h, set, candidate, ids, live_before);
    scoped_modulation_edit(h, set, candidate, ids, live_before);
    insert_mid_range_processor(h, set, candidate, ids)
}

#[test]
fn cb_fx_02_shared_context_flows_b_edits_stay_in_range() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);

    let live_before = h.controller.live().clone();
    let forked = h.fork(scope.clone());
    let (set, adopted, candidate) = (forked.variant_set, forked.adopted, forked.candidate);

    let inserted = rich_candidate_edits(&mut h, set, candidate, &ids, &live_before);

    let cand = h.candidate(set, candidate);
    // Candidate carries every edit; live is bitwise untouched —
    // upstream `pre`, shared downstream wiring and the sibling strip
    // are never candidate-owned.
    assert_eq!(cand.control.targets[&ids.param_fx2].base, 0.9);
    assert_eq!(live_before.control.targets[&ids.param_fx2].base, 0.6);
    assert!(
        cand.control
            .targets
            .values()
            .any(|t| t.owner == ControlOwner::Processor(ids.fx1) && t.name == "fx1-resonance")
    );
    assert!(
        cand.control
            .lanes
            .values()
            .any(|l| l.target == ids.param_fx3)
    );
    assert!(
        cand.control
            .assignments
            .values()
            .any(|a| a.target == ids.param_fx1)
    );
    // The in-range insert became scope-owned: it sits inside the
    // candidate's resolved range and carries its own ports/instance.
    let cand_chain = &cand.routing.chains[&ids.chain].processors;
    let pos_fx1 = cand_chain.iter().position(|p| *p == ids.fx1).unwrap();
    let pos_fx3 = cand_chain.iter().position(|p| *p == ids.fx3).unwrap();
    let pos_new = cand_chain.iter().position(|p| *p == inserted).unwrap();
    assert!(pos_fx1 < pos_new && pos_new < pos_fx3);
    assert!(cand.plugin_instances.instances.contains_key(&inserted));
    assert!(
        !h.controller
            .live()
            .routing
            .processors
            .contains_key(&inserted)
    );

    // Live is untouched: control state, routing graph, plugin
    // instances — the whole shared context.
    assert_eq!(h.controller.live().control, live_before.control);
    assert_eq!(
        serde_json::to_value(&h.controller.live().routing).unwrap(),
        serde_json::to_value(&live_before.routing).unwrap()
    );
    assert_eq!(
        h.controller.live().plugin_instances,
        live_before.plugin_instances
    );

    // Render pins: adopted (A) and candidate (B) share the same
    // shared-context identity; only the scoped digest diverges.
    let pin_a = h.render_pin(set, adopted);
    let pin_b = h.render_pin(set, candidate);
    assert_eq!(
        pin_a.shared_context_digest, pin_b.shared_context_digest,
        "both candidates must see the same shared upstream/downstream"
    );
    assert_ne!(
        pin_a.scope_digest, pin_b.scope_digest,
        "the scoped candidate edit must move only the candidate identity"
    );

    // Shared upstream edit on live: the `pre` processor's parameter
    // is outside the range — it moves the shared-context digest for
    // BOTH candidates (new comparison context → stale, never an
    // Exact re-display of the old one).
    apply_op(
        &mut h.controller,
        ProjectOp::SetParameterBase {
            target: ids.param_pre,
            value: 0.95,
        },
    );
    let pin_adopted_after = h.render_pin(set, adopted);
    let pin_candidate_after = h.render_pin(set, candidate);
    assert_ne!(
        pin_a.shared_context_digest, pin_adopted_after.shared_context_digest
    );
    assert_ne!(
        pin_b.shared_context_digest, pin_candidate_after.shared_context_digest
    );
    assert_eq!(
        pin_adopted_after.shared_context_digest, pin_candidate_after.shared_context_digest,
        "upstream edits propagate identically to every branch context"
    );
    // Scoped identities are unmoved by the shared edit.
    assert_eq!(pin_a.scope_digest, pin_adopted_after.scope_digest);
    assert_eq!(pin_b.scope_digest, pin_candidate_after.scope_digest);
}

// ===========================================================================
// CB-FX-03: stable semantic boundaries survive chain edits; deletion
// goes Stale/Unresolved — never silently re-anchored
// ===========================================================================

#[test]
fn cb_fx_03_stable_boundary_survives_fx_add_and_reorder() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);
    let forked = h.fork(scope.clone());
    let (set, _adopted, candidate) = (forked.variant_set, forked.adopted, forked.candidate);

    let digest_before = scope_digest(h.controller.live(), &scope).unwrap();
    let pin_before = h.render_pin(set, candidate);

    // FX added at the chain head (upstream of the range): the range's
    // processor set is unchanged — stable IDs keep the same scope
    // identity while the new processor joins shared context.
    apply_op(
        &mut h.controller,
        ProjectOp::InsertProcessor {
            strip_id: ids.strip,
            index: 0,
            name: "fx-new-head".into(),
            binding: plugin_binding("plug-new"),
            sidechain_input: false,
        },
    );
    assert_eq!(
        scope_digest(h.controller.live(), &scope).unwrap(),
        digest_before,
        "adding FX outside the range must not move the scope identity"
    );
    let pin_added = h.render_pin(set, candidate);
    assert_ne!(
        pin_before.shared_context_digest, pin_added.shared_context_digest,
        "the added processor is shared context — comparison goes stale"
    );
    assert_eq!(pin_before.scope_digest, pin_added.scope_digest);

    // Reordering range processors keeps every boundary resolved through
    // stable IDs — never ordinal re-anchoring. The scope owns a *set* of
    // semantic objects, so interior order is not part of scope identity:
    // the digest is unchanged (the range follows its processors, not
    // positions). Chain order itself is shared routing context, so the
    // reorder moves the shared-context pin — comparison goes stale.
    //
    // The declared range fx1..=fx3 has only one interior processor, so
    // extend the chain: fx4 appended at the tail widens the resolvable
    // range to fx1..=fx4, giving fx2/fx3 a swappable interior pair.
    apply_op(
        &mut h.controller,
        ProjectOp::InsertProcessor {
            strip_id: ids.strip,
            index: 5,
            name: "fx4".into(),
            binding: plugin_binding("plug-d"),
            sidechain_input: false,
        },
    );
    let fx4 = *h.controller.live().routing.chains[&ids.chain]
        .processors
        .last()
        .unwrap();
    let wide = VariantScope::ProcessingRange {
        range: ProcessingRangeRef {
            chain: ids.chain,
            start: ids.fx1,
            end: fx4,
        },
    };
    let wide_before = scope_digest(h.controller.live(), &wide).unwrap();
    let pin_before_reorder = h.render_pin(set, candidate);
    // Swap the interior pair fx2/fx3 — membership is unchanged.
    apply_op(
        &mut h.controller,
        ProjectOp::ReorderProcessor {
            chain_id: ids.chain,
            processor_id: ids.fx2,
            new_index: 4,
        },
    );
    let wide_after = scope_digest(h.controller.live(), &wide).unwrap();
    assert_eq!(
        wide_before, wide_after,
        "interior reorder must preserve the stable scope identity"
    );
    // For the *declared* fx1..=fx3 range the same swap pushed fx2 past
    // the fx3 boundary — it exited the range, so scoped membership
    // changed and both pin digests move. Boundary-crossing reorder is a
    // scoped change; strictly-interior reorder (wide scope above) is
    // not. Either way the pin stales — never silently equivalent.
    let pin_after_reorder = h.render_pin(set, candidate);
    assert_ne!(
        pin_before_reorder.scope_digest, pin_after_reorder.scope_digest,
        "fx2 exited the declared range — scoped identity moved"
    );
    assert_ne!(
        pin_before_reorder.shared_context_digest, pin_after_reorder.shared_context_digest,
        "chain order is shared context — the reorder stales the pin"
    );
}

#[test]
fn cb_fx_03_boundary_deletion_is_unresolved_or_stale() {
    // Start boundary deleted → the range no longer resolves: typed
    // NotFound, never a silent re-anchor to the nearest processor.
    // (No variant set here — the project oracle itself refuses to
    // delete an entity while a declared scope references it; this leg
    // proves the *resolver* goes Unresolved on a boundary-less chain.)
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let scope = scope_of(&ids);
    apply_op(
        &mut c,
        ProjectOp::RemoveParameterTarget {
            target: ids.param_fx1,
        },
    );
    apply_op(
        &mut c,
        ProjectOp::RemoveProcessor {
            processor_id: ids.fx1,
        },
    );
    let h = Fx::new(c);
    let err = scope_digest(h.controller.live(), &scope).unwrap_err();
    assert!(
        matches!(err, synvelis_project::Error::NotFound(_)),
        "deleted start boundary must be Unresolved (NotFound), got {err:?}"
    );

    // Mid-range deletion → the range still resolves but its content
    // changed: the pinned scope digest goes Stale and adopt rejects —
    // the deleted processor is never silently reconnected.
    let mut c2 = controller();
    let ids2 = fx_fixture(&mut c2);
    let scope2 = scope_of(&ids2);
    let mut h2 = Fx::new(c2);
    let forked2 = h2.fork(scope2.clone());
    h2.edit(
        forked2.variant_set,
        forked2.candidate,
        ProjectOp::SetParameterBase {
            target: ids2.param_fx2,
            value: 0.9,
        },
    );
    // Pin the adopt intent BEFORE the live deletion — the stale
    // expected digest is what adopt must reject on. Deleting a
    // non-boundary range member is allowed by the oracle (only the
    // declared start/end IDs are pinned), so live moves on.
    let mut intent = h2.intent(forked2.variant_set, forked2.candidate, &scope2);
    apply_op(
        &mut h2.controller,
        ProjectOp::RemoveParameterTarget {
            target: ids2.param_fx2,
        },
    );
    apply_op(
        &mut h2.controller,
        ProjectOp::RemoveProcessor {
            processor_id: ids2.fx2,
        },
    );
    // The range resolves to {fx1, fx3} — smaller but still valid;
    // the live scope digest moved, so the pinned intent is Stale.
    // Refresh only the project-revision pin so the scope-digest gate
    // is the check under test.
    assert!(scope_digest(h2.controller.live(), &scope2).is_ok());
    intent.expected_project_revision = h2.controller.live_revision();
    let live_before = h2.controller.live().clone();
    let err = adopt_branch_adapted(
        &mut h2.controller,
        &h2.adapter,
        &mut h2.store,
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
        "mid-range deletion must reject StaleScope, got {err:?}"
    );
    assert_eq!(*h2.controller.live(), live_before);
}

// ===========================================================================
// CB-FX-04: boundary dependencies detected; external wiring ops fail
// closed — no false equivalence, no implicit double-mix
// ===========================================================================

#[test]
fn cb_fx_04_boundary_dependency_detected_and_shared() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let feed = wire_sidechain_feed(&mut c, &ids);
    let mut h = Fx::new(c);
    let range = range_ref(&ids);

    // Typed inventory: exactly the sidechain feed crosses the
    // boundary — source (key strip out) is shared, destination
    // (fx2 sidechain in) is scope-owned.
    let deps = ProcessingRangeAdapter::boundary_dependencies(h.controller.live(), &range).unwrap();
    assert_eq!(deps.len(), 1, "only the sidechain feed crosses: {deps:?}");
    assert_eq!(deps[0].connection, feed);
    assert_eq!(deps[0].kind, ConnectionKind::Sidechain);
    assert!(!deps[0].source_in_scope);

    // The boundary connection is NOT scope-owned — it stays shared
    // context and moves the shared-context digest when it changes.
    let scope = scope_of(&ids);
    let forked = h.fork(scope.clone());
    let (set, candidate) = (forked.variant_set, forked.candidate);
    let pin_before = h.render_pin(set, candidate);
    apply_op(
        &mut h.controller,
        ProjectOp::SetConnectionEnabled {
            connection_id: feed,
            enabled: false,
        },
    );
    let deps_after =
        ProcessingRangeAdapter::boundary_dependencies(h.controller.live(), &range).unwrap();
    assert_eq!(
        deps_after.len(),
        1,
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

/// Every op that could touch boundary/external wiring — sends,
/// sidechains, multi-output routes, foreign chain structure, strip
/// state. Each must reject typed; nothing may apply or normalize.
fn external_wiring_ops(project: &Project, ids: &FxIds, feed: RoutingConnectionId) -> Vec<ProjectOp> {
    let key_out = port_of(
        project,
        PortOwner::Strip(ids.key_strip),
        PortDirection::Output,
        PortRole::Main,
    );
    let fx2_in = port_of(
        project,
        PortOwner::Processor(ids.fx2),
        PortDirection::Input,
        PortRole::Main,
    );
    let strip_out = port_of(
        project,
        PortOwner::Strip(ids.strip),
        PortDirection::Output,
        PortRole::Main,
    );
    // The strip's default Main route lands on the master strip's main
    // input — find that destination through the existing connection.
    let master_in = project
        .routing
        .connections
        .values()
        .find(|conn| conn.source == strip_out)
        .map(|conn| conn.destination)
        .expect("strip main route to master");
    vec![
        // Touching the boundary connection itself.
        ProjectOp::RemoveConnection {
            connection_id: feed,
        },
        ProjectOp::SetConnectionEnabled {
            connection_id: feed,
            enabled: false,
        },
        ProjectOp::SetConnectionChannelMap {
            connection_id: feed,
            channel_map: None,
        },
        // New external wiring: sidechain, main route, send.
        ProjectOp::AddConnection {
            source: key_out,
            destination: fx2_in,
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
            tap: SendTap::PostFader,
            gain_target: None,
        },
        // Chain structure outside candidate ownership.
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
        // Strip/bus-level edits are shared context.
        ProjectOp::SetStripMute {
            strip_id: ids.strip,
            muted: true,
        },
    ]
}

#[test]
fn cb_fx_04_external_wiring_ops_reject_typed() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let feed = wire_sidechain_feed(&mut c, &ids);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);

    // Every op that could touch boundary/external wiring rejects
    // Unsupported — sends, sidechains, multi-output wiring, foreign
    // chain structure, strip/bus edits. Nothing is silently applied
    // or normalized (CB-FX-04, CB-PROC-02).
    let ops = external_wiring_ops(h.controller.live(), &ids, feed);
    for op in ops {
        let err = h.try_edit(set, candidate, op).unwrap_err();
        assert!(
            matches!(
                err,
                VariantError::Unsupported { .. } | VariantError::ScopeViolation { .. }
            ),
            "external wiring op must reject typed, got {err:?}"
        );
    }

    // Out-of-scope control entities: the upstream `pre` param and a
    // foreign-owner creation are ScopeViolation — the range owns only
    // its processors' closure.
    for op in [
        ProjectOp::SetParameterBase {
            target: ids.param_pre,
            value: 0.1,
        },
        ProjectOp::AddParameterTarget {
            name: "intruder".into(),
            owner: ControlOwner::Processor(ids.pre),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    ] {
        let err = h.try_edit(set, candidate, op).unwrap_err();
        assert!(
            matches!(err, VariantError::ScopeViolation { .. }),
            "out-of-scope control op must reject ScopeViolation, got {err:?}"
        );
    }

    // Scope-entity removal fails closed.
    let err = h
        .try_edit(
            set,
            candidate,
            ProjectOp::RemoveParameterTarget {
                target: ids.param_fx1,
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "scope-entity removal must reject Unsupported, got {err:?}"
    );

    // Adopt materializes only scoped entities: the boundary feed and
    // every external connection survive byte-identical — no implicit
    // double-mix, no rewiring.
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fx2,
            value: 0.9,
        },
    );
    let connections_before = h.controller.live().routing.connections.clone();
    let receipt = h.adopt(set, candidate, &scope_of(&ids));
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(
        h.controller.live().routing.connections,
        connections_before,
        "adopt must never create or rewire connections outside the range"
    );
    assert_eq!(
        h.controller.live().control.targets[&ids.param_fx2].base,
        0.9
    );
}

// ===========================================================================
// CB-FX-05 + CB-FX-RCP-01: adopt declared range only — one CAS/Undo,
// save/reload, runtime publish failure observed separately, captured
// payload immutability
// ===========================================================================

#[test]
fn cb_fx_05_adopt_range_only_one_cas_undo_save_reload() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);
    let forked = h.fork(scope.clone());
    let (set, adopted_a, candidate_b) = (forked.variant_set, forked.adopted, forked.candidate);

    let live_before = h.controller.live().clone();
    let inserted = rich_candidate_edits(&mut h, set, candidate_b, &ids, &live_before);
    let rev_before = h.controller.live_revision();
    let connections_before = live_before.routing.connections.clone();
    let strips_before = serde_json::to_value(&live_before.routing.strips).unwrap();
    let sends_before = serde_json::to_value(&live_before.routing.sends).unwrap();

    let receipt = h.adopt(set, candidate_b, &scope);
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(receipt.project.undo_steps, 1);
    assert!(receipt.project.committed);
    assert_eq!(receipt.after_revision.0, rev_before.0 + 1);

    let live = h.controller.live();
    // Declared range transplanted: scoped control state + the
    // in-range insert landed; upstream/downstream/strip/send state is
    // bitwise unchanged — adopt touched nothing outside the range.
    assert_eq!(live.control.targets[&ids.param_fx2].base, 0.9);
    assert!(
        live.control
            .targets
            .values()
            .any(|t| t.owner == ControlOwner::Processor(ids.fx1) && t.name == "fx1-resonance")
    );
    let chain = &live.routing.chains[&ids.chain].processors;
    let pos_new = chain.iter().position(|p| *p == inserted).unwrap();
    assert!(chain.iter().position(|p| *p == ids.fx1).unwrap() < pos_new);
    assert!(pos_new < chain.iter().position(|p| *p == ids.fx3).unwrap());
    assert_eq!(live.routing.connections, connections_before);
    assert_eq!(
        serde_json::to_value(&live.routing.strips).unwrap(),
        strips_before
    );
    assert_eq!(
        serde_json::to_value(&live.routing.sends).unwrap(),
        sends_before
    );
    // Upstream `pre` param and its processor are untouched.
    assert_eq!(live.control.targets[&ids.param_pre].base, 0.4);
    assert_eq!(
        serde_json::to_value(&live.routing.processors[&ids.pre]).unwrap(),
        serde_json::to_value(&live_before.routing.processors[&ids.pre]).unwrap()
    );

    // The previously adopted branch froze as a captured candidate
    // retaining the pre-adopt (A) scoped state.
    let set_state = &live.variant_sets[&set];
    let frozen = &set_state.branches[&adopted_a].content;
    let BranchContentRef::Captured { .. } = frozen else {
        panic!("previously adopted branch must freeze to captured content");
    };

    // One undo restores the exact pre-adopt scope; redo re-applies —
    // candidates survive both directions.
    assert!(h.controller.undo().unwrap());
    assert_eq!(
        h.controller.live().control.targets[&ids.param_fx2].base,
        0.6
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
        h.controller.live().control.targets[&ids.param_fx2].base,
        0.9
    );
    assert!(
        h.controller
            .live()
            .routing
            .processors
            .contains_key(&inserted)
    );
    assert_eq!(h.controller.live().variant_sets[&set].adopted, candidate_b);

    // Save → reload: variant sets, scope declarations and live
    // content round-trip (candidate payloads live in the external
    // content store — the project snapshot carries digest refs only).
    let mut store = InMemoryStore::new();
    h.controller.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live(), h.controller.live());
    assert!(
        c2.live()
            .variant_sets
            .values()
            .any(|s| s.scope == scope_of(&ids))
    );
}

/// CB-FX-05 runtime half: a failed runtime publish is a *separate*
/// receipt — the project commit stands (Last-good/Pending
/// observation), live is never rolled back.
#[test]
fn cb_fx_05_runtime_publish_failure_keeps_project_committed() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fx3,
            value: 0.15,
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
    // Project truth committed; the failure is observed on the runtime
    // receipt only — live holds the adopted value and undo still works.
    assert_eq!(
        h.controller.live().control.targets[&ids.param_fx3].base,
        0.15
    );
    assert!(h.controller.undo().unwrap());
    assert_eq!(
        h.controller.live().control.targets[&ids.param_fx3].base,
        0.7
    );
}

/// CB-FX-05 stale gate: a live scoped edit between intent and adopt
/// fails `StaleScope` — no auto-rebase, no partial mutation.
#[test]
fn cb_fx_05_stale_scope_digest_rejects_adopt() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fx2,
            value: 0.9,
        },
    );
    let intent = h.intent(forked.variant_set, forked.candidate, &scope);
    // Live scoped mutation after the intent was pinned — refresh only
    // the project-revision pin so the scope-digest gate is exercised.
    apply_op(
        &mut h.controller,
        ProjectOp::SetParameterBase {
            target: ids.param_fx1,
            value: 0.11,
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

// ===========================================================================
// CB-FX-05 recording gate + CB-FX-06 plugin-instance/state honesty
// ===========================================================================

/// Injects an open recording session + capture stream on `lane`
/// directly into project state (the recording registry is runtime
/// data with no `ProjectOp` — the cb02/cb04 fixture convention).
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

#[test]
fn cb_fx_05_adopt_while_recording_rejects_typed() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let lane = ids.lane;
    let c = with_open_recording(&c, lane);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fx2,
            value: 0.9,
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

/// Plugin-availability probe reporting every binding unavailable.
struct NoPlugins;
impl PluginAvailability for NoPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        false
    }
}

/// Plugin-availability probe reporting every binding available.
struct YesPlugins;
impl PluginAvailability for YesPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        true
    }
}

/// Per-processor state assessment over the candidate's resolved range:
/// one verdict per range processor in chain order, quality/failure
/// recorded verbatim — never upgraded, never normalized to success.
fn assert_range_state_assessments(
    cand: &Project,
    ids: &FxIds,
    inserted: ProcessorId,
) {
    let mut port = MockPort::default();
    for p in [ids.fx1, ids.fx2, ids.fx3] {
        port.bind(p, StateCaptureQuality::Coordinated);
    }
    port.bind(inserted, StateCaptureQuality::BestEffort);
    let assessments = ProcessingRangeAdapter::assess_range_state(
        &mut port,
        cand,
        &range_ref(ids),
        CaptureMode::Coordinated,
    )
    .unwrap();
    // range_ref still resolves on the *candidate* chain — the insert
    // sits inside start..=end so the range grew by one processor.
    assert_eq!(assessments.len(), 4);
    let by_proc: BTreeMap<ProcessorId, &InstrumentStateAssessment> =
        assessments.iter().map(|a| (a.processor, a)).collect();
    for p in [ids.fx1, ids.fx2, ids.fx3] {
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
    let assessments = ProcessingRangeAdapter::assess_range_state(
        &mut port,
        cand,
        &range_ref(ids),
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

/// Plugin availability is a typed adopt gate: reporting every resolved
/// binding unavailable fails closed BEFORE any mutation; reporting
/// them available applies the adopt — project truth does not depend
/// on runtime/plugin presence beyond the typed gate.
fn assert_plugin_gate_adopt(
    h: &mut Fx,
    set: VariantSetId,
    candidate: BranchId,
    scope: &VariantScope,
    ids: &FxIds,
    inserted: ProcessorId,
) {
    let live_before_adopt = h.controller.live().clone();
    let err = h
        .try_adopt_full(
            set,
            candidate,
            scope,
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
            set,
            candidate,
            scope,
            Some(&YesPlugins),
            &NoopRuntimePublish,
        )
        .unwrap();
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(
        h.controller.live().control.targets[&ids.param_fx2].base,
        0.9
    );
    // The adopted in-range processor kept a real persisted instance
    // identity — live and candidate never shared mutable DSP state.
    assert!(
        h.controller
            .live()
            .plugin_instances
            .instances
            .contains_key(&inserted)
    );
}

/// CB-FX-06 project-side contract: each candidate carries independent
/// `plugin_instances`/`Processor` entities and bindings — an in-range
/// insert mints its own instance inside the candidate only — and the
/// per-processor state assessment records achieved quality verbatim
/// (never upgraded, never faked). Real plugin/PCM/PDC/RT comparison
/// is `NotRun` (evidence gate table).
#[test]
fn cb_fx_06_plugin_instances_independent_state_assessment() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);
    let forked = h.fork(scope.clone());
    let (set, candidate) = (forked.variant_set, forked.candidate);

    // Candidate clone keeps the same persisted instance identities —
    // per-candidate project state, never aliased mutable DSP.
    let cand = h.candidate(set, candidate);
    for p in [ids.fx1, ids.fx2, ids.fx3] {
        assert_eq!(
            cand.plugin_instances.instances[&p],
            h.controller.live().plugin_instances.instances[&p]
        );
    }
    assert!(cand.plugin_instances.instances.contains_key(&ids.pre));

    // In-range insert mints a new instance inside the candidate only.
    let live_before = h.controller.live().clone();
    let inserted = rich_candidate_edits(&mut h, set, candidate, &ids, &live_before);
    let cand = h.candidate(set, candidate);
    assert!(cand.plugin_instances.instances.contains_key(&inserted));
    assert!(
        !live_before
            .plugin_instances
            .instances
            .contains_key(&inserted)
    );

    // Per-processor state assessment: one verdict per range processor
    // in chain order, quality/failure recorded verbatim.
    assert_range_state_assessments(&cand, &ids, inserted);

    // Plugin availability is a typed adopt gate (CB-FX-06): unavailable
    // bindings fail closed before any mutation; available ones adopt.
    assert_plugin_gate_adopt(&mut h, set, candidate, &scope, &ids, inserted);
}

// ===========================================================================
// CB-FX-RCP-01: render-pin identity independence + captured payload
// immutability + semantic diff surface
// ===========================================================================

#[test]
fn cb_fx_rcp_01_render_identity_independent_captured_immutable() {
    let mut c = controller();
    let ids = fx_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(&ids);
    let forked = h.fork(scope.clone());
    let (set, candidate) = (forked.variant_set, forked.candidate);

    // The captured payload digest is the store key — live shared edits
    // must never mutate candidate bytes (immutability contract).
    let content = h.controller.live().variant_sets[&set].branches[&candidate]
        .content
        .clone();
    let BranchContentRef::Captured { payload_digest, .. } = &content else {
        panic!("candidate must be captured content");
    };
    let digest_at_fork = *payload_digest;
    apply_op(
        &mut h.controller,
        ProjectOp::SetParameterBase {
            target: ids.param_pre,
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
    // The stored bytes still decode to the fork-time project — the
    // upstream edit is NOT retroactively baked into the candidate.
    let cand = h.candidate(set, candidate);
    assert_eq!(cand.control.targets[&ids.param_pre].base, 0.4);

    // RCP independence: the branch render pin composes candidate scope
    // over live shared context through the same adopt-planning path —
    // no RenderBoundary/cutset dependency. Unsupported external-port
    // forms stay typed (capability + boundary inventory), never
    // silently substituted.
    let pin = h.render_pin(set, candidate);
    assert!(pin.payload_digest.is_some());
    let caps = h.adapter.capabilities();
    assert_eq!(caps.routing, ProcessingCapability::Unsupported);
    assert_eq!(caps.mid_chain_end, ProcessingCapability::Unsupported);
    assert_eq!(caps.fx_insert_inside, ProcessingCapability::Supported);
    assert_eq!(caps.param_base, ProcessingCapability::Supported);

    // Semantic diff: scoped edits surface as typed changes —
    // project truth, never a signal/render substitution.
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_fx2,
            value: 0.9,
        },
    );
    let cs = compare(&h.controller.live().clone(), &h.candidate(set, candidate));
    let param_changes: Vec<&SemanticChange> = cs
        .changes
        .iter()
        .filter(|ch| ch.object.kind == ObjectKind::ParameterTarget)
        .collect();
    assert!(param_changes.iter().any(|ch| {
        ch.object.stable_id == ids.param_fx2.to_tagged() && ch.kind == ChangeKind::Modified
    }));
}

// ---------- diff harness (cb04 snap_manual/compare conventions) ----

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
