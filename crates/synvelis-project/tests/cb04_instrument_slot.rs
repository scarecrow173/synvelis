//! CB-04 Creative Branch instrument-slot adapter fixtures (synvelis#281).
//!
//! Covers the real `InstrumentSlotAdapter` headlessly: fork/edit/adopt
//! of `VariantScope::InstrumentSlot` candidates over a shared MIDI
//! input, processor-owned control-closure confinement, independent
//! plugin-instance/state isolation across candidates, semantic diff +
//! honest `StateCaptureQuality` assessment, shared-context staleness,
//! one-CAS adopt with undo/redo, and typed gates (recording-active,
//! missing plugin, unsupported ops). Real OS / plugin / RT / GUI
//! lanes are `NotRun` — recorded in `evidence/sv281/`.

use std::collections::BTreeMap;

use ruviform_plugin_core::{ParamFlags, ParamId};
use synvelis_project::apply::{ApplyApproval, PluginAvailability};
use synvelis_project::comp::CompOp;
use synvelis_project::control::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, ConflictPolicy,
    ControlOwner, Interpolation, LfoShape, ParamMapping, ParameterRole, Polarity, SourceKind,
    TargetCapability, Transform, ValueKind,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    ChangeKind, ComparisonSpec, ObjectKind, SemanticChange, compare_snapshots,
};
use synvelis_project::host_param::{plugin_parameter_instance_binding, semantic_plugin_binding};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::plugin_state::{
    CaptureMode, CaptureObservation, PluginStateBinding, PluginStateCapture,
    PluginStateCaptureError, PluginStateFailureKind, PluginStateOutcome, PluginStatePort,
    StateCaptureQuality,
};
use synvelis_project::project::Project;
use synvelis_project::range::SourceRange;
use synvelis_project::recording_data::{CaptureStream, ClockEpoch, RecordingSession, SessionState};
use synvelis_project::routing::{PluginRef, ProcessorBinding};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, ContentDigest, FixedSnapshotLocator, ImmutableSnapshot,
    ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness, SnapshotIdentity,
    SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::time::{MusicalDuration, MusicalTick, SourceFrame};
use synvelis_project::variant::{
    AdoptIntent, AdoptOutcome, AdoptSources, BranchContentRef, BranchEditOp, BranchEditOpIntent,
    DomainAdapter, InMemoryBranchContentStore, InstrumentSlotAdapter, InstrumentStateAssessment,
    MidiClipAdapter, NoopRuntimePublish, ProcessingCapability, VariantError, VariantScope,
    adopt_branch_adapted, decode_candidate, edit_branch_op, fork_variant_set, scope_digest,
};
use synvelis_project::{
    BranchId, CaptureStreamId, ClipExtent, ClipId, ClipPlacement, ClipPlayback, ClockEpochId,
    CompId, LaneId, MidiDivision, MidiTick, ParameterTargetId, PlaybackMapping, ProjectOp,
    RecordingSessionId, RenderBranchTarget, SourceId, TimeAnchor, TrackId, VariantSetId,
    is_issuable_raw, resolve_render_project,
};

// ---------- shared harness (cb03_midi_clip.rs conventions) ----------

struct Fx {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    resolver: SnapshotResolver,
    adapter: InstrumentSlotAdapter,
}

impl Fx {
    fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            resolver: SnapshotResolver::new(),
            adapter: InstrumentSlotAdapter::new(),
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
            approval: ApplyApproval {
                ticket: "approve".into(),
            },
        }
    }

    fn try_adopt_plugins(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
        plugins: Option<&dyn PluginAvailability>,
    ) -> Result<synvelis_project::variant::AdoptReceipt, VariantError> {
        let intent = self.intent(set, branch, scope);
        let runtime = NoopRuntimePublish;
        adopt_branch_adapted(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &AdoptSources {
                assets: None,
                plugins,
                runtime: &runtime,
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
        self.try_adopt_plugins(set, branch, scope, None)
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

// ---------- fixture: shared MIDI input → instrument slot ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb04-fixture"),
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

struct InstIds {
    track: TrackId,
    lane: LaneId,
    clip: ClipId,
    midi_source: SourceId,
    /// The instrument slot under test (first processor on the strip).
    inst: synvelis_project::id::ProcessorId,
    /// A second processor on the same chain — sibling context that
    /// must never be candidate-owned.
    sibling: synvelis_project::id::ProcessorId,
    /// `ProcessorParam` target WITH capability (stage-1 bindable).
    param_public: ParameterTargetId,
    /// `ProcessorParam` target WITHOUT capability (private param —
    /// stage-1 binding must fail closed, never guess).
    param_private: ParameterTargetId,
    /// A target owned by the sibling processor — out of scope.
    param_sibling: ParameterTargetId,
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

/// Track → lane → MIDI source + notes + clip (the shared input every
/// instrument candidate reads but none owns).
fn midi_clip_fixture(c: &mut ProjectController) -> (TrackId, LaneId, ClipId, SourceId) {
    apply_op(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(
        c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let midi_source = *c.live().midi_sources.keys().next().unwrap();
    for (key, velocity, start, len) in [
        (60_u8, 100_u8, 0_i64, 120_i64),
        (64_u8, 80_u8, 240_i64, 96_i64),
    ] {
        apply_op(
            c,
            ProjectOp::AddMidiNote {
                source_id: midi_source,
                key,
                velocity,
                start: MidiTick::new(start).unwrap(),
                len: MidiTick::new(len).unwrap(),
            },
        );
    }
    apply_op(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: midi_source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(4 * 960).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    (track, lane, clip, midi_source)
}

/// Track strip with two resolved plugin processors (`inst` = the slot
/// under test, `sibling` = confinement witness).
fn strip_fixture(
    c: &mut ProjectController,
    track: TrackId,
) -> (
    synvelis_project::id::ProcessorId,
    synvelis_project::id::ProcessorId,
) {
    apply_op(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    let plugin = |d: &str| {
        ProcessorBinding::Resolved(PluginRef {
            descriptor: d.into(),
            version: "1.0".into(),
            capability_revision: 1,
        })
    };
    for (index, name, desc) in [
        (0usize, "synth-a", "plug-synth"),
        (1usize, "fx-delay", "plug-fx"),
    ] {
        apply_op(
            c,
            ProjectOp::InsertProcessor {
                strip_id: strip,
                index,
                name: name.into(),
                binding: plugin(desc),
                sidechain_input: false,
            },
        );
    }
    let procs: Vec<_> = c.live().routing.processors.keys().copied().collect();
    (procs[0], procs[1])
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

/// Track → lane → MIDI source + clip (shared input) → track strip →
/// two resolved plugin processors. `inst` is the slot under test;
/// `sibling` proves scope confinement.
fn instrument_fixture(c: &mut ProjectController) -> InstIds {
    let (track, lane, clip, midi_source) = midi_clip_fixture(c);
    let (inst, sibling) = strip_fixture(c, track);
    let param_public = add_target(
        c,
        ControlOwner::Processor(inst),
        "cutoff",
        Some(capability(7)),
        0.5,
    );
    let param_private = add_target(c, ControlOwner::Processor(inst), "hidden", None, 0.25);
    let param_sibling = add_target(
        c,
        ControlOwner::Processor(sibling),
        "fx-param",
        Some(capability(3)),
        0.5,
    );
    InstIds {
        track,
        lane,
        clip,
        midi_source,
        inst,
        sibling,
        param_public,
        param_private,
        param_sibling,
    }
}

fn scope_of(inst: synvelis_project::id::ProcessorId) -> VariantScope {
    VariantScope::InstrumentSlot { processor: inst }
}

/// Serialized shared-MIDI witness — the input candidates must never
/// mutate (CB-INST-01).
fn midi_state_json(c: &ProjectController) -> serde_json::Value {
    serde_json::json!({
        "midi_sources": c.live().midi_sources,
        "clips": c.live().clips,
    })
}

fn new_targets(candidate: &Project, live: &Project) -> Vec<ParameterTargetId> {
    candidate
        .control
        .targets
        .keys()
        .filter(|id| !live.control.targets.contains_key(*id))
        .copied()
        .collect()
}

// ===========================================================================
// CB-INST-01: two instrument candidates from one MIDI input — isolated
// plugin-instance/control state, shared MIDI source untouched
// ===========================================================================

/// The rich candidate-B edit sequence shared by CB-INST-01/02/04:
/// base edit, new plugin param, automation lane + region, modulation
/// source + assignment — every op confined to the scoped closure.
fn rich_candidate_edits(
    h: &mut Fx,
    set: VariantSetId,
    candidate: BranchId,
    ids: &InstIds,
    live_before: &Project,
) {
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_public,
            value: 0.9,
        },
    );
    h.edit(
        set,
        candidate,
        ProjectOp::AddParameterTarget {
            name: "resonance".into(),
            owner: ControlOwner::Processor(ids.inst),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(capability(8)),
            base: 0.3,
        },
    );
    h.edit(
        set,
        candidate,
        ProjectOp::AddAutomationLane {
            target: ids.param_public,
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
    h.edit(
        set,
        candidate,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Processor(ids.inst),
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
            target: ids.param_public,
            depth: 0.4,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
}

/// The engine's `VariantSet` holds adopted+candidate; "2候補" is
/// exercised across the fork → edit → adopt → freeze lifecycle plus a
/// sibling scope — the same convention cb03 documents. Each candidate
/// keeps its own scope digest; candidate edits never touch live, the
/// shared MIDI source, or sibling-processor objects.
#[test]
fn cb_inst_01_candidates_isolated_shared_midi_untouched() {
    let mut c = controller();
    let ids = instrument_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(ids.inst);

    let live_before = h.controller.live().clone();
    let forked = h.fork(scope.clone());
    let (set, _adopted, candidate) = (forked.variant_set, forked.adopted, forked.candidate);
    let pin_fork = h.render_pin(set, candidate);

    rich_candidate_edits(&mut h, set, candidate, &ids, &live_before);

    let cand = h.candidate(set, candidate);
    // Candidate carries the edits; live does not.
    assert_eq!(cand.control.targets[&ids.param_public].base, 0.9);
    assert_eq!(live_before.control.targets[&ids.param_public].base, 0.5);
    assert_eq!(new_targets(&cand, &live_before).len(), 1);
    assert!(
        cand.control
            .lanes
            .values()
            .any(|l| l.target == ids.param_public)
    );
    assert!(
        cand.control
            .assignments
            .values()
            .any(|a| a.target == ids.param_public)
    );
    // Live control closure untouched.
    assert_eq!(h.controller.live().control, live_before.control);
    // Shared MIDI input bitwise identical in live and candidate.
    assert_eq!(midi_state_json(&h.controller), midi_state_json_of(&cand));
    // Sibling processor + its target untouched in both.
    assert_eq!(
        cand.routing.processors[&ids.sibling],
        live_before.routing.processors[&ids.sibling]
    );
    assert_eq!(
        cand.control.targets[&ids.param_sibling],
        live_before.control.targets[&ids.param_sibling]
    );
    // Plugin-instance identity is per-candidate project state: the
    // clone keeps the same persisted identity, and edits cannot alias
    // it to the sibling or drop it.
    assert_eq!(
        cand.plugin_instances.instances[&ids.inst],
        live_before.plugin_instances.instances[&ids.inst]
    );
    assert!(cand.plugin_instances.instances.contains_key(&ids.sibling));
    // Scope digests diverge; render pin proves scoped vs shared split —
    // fork-time pin vs post-edit pin.
    assert_ne!(
        scope_digest(h.controller.live(), &scope).unwrap(),
        scope_digest(&cand, &scope).unwrap()
    );
    let pin_after_edit = h.render_pin(set, candidate);
    assert_ne!(pin_fork.scope_digest, pin_after_edit.scope_digest);
    assert_eq!(
        pin_fork.shared_context_digest, pin_after_edit.shared_context_digest,
        "scoped edits must not move the shared-context identity"
    );
}

fn midi_state_json_of(p: &Project) -> serde_json::Value {
    serde_json::json!({
        "midi_sources": p.midi_sources,
        "clips": p.clips,
    })
}

// ===========================================================================
// CB-INST-02: semantic diff + honest state quality + capability report
// ===========================================================================

#[test]
fn cb_inst_02_semantic_diff_state_quality_and_capabilities() {
    let mut c = controller();
    let ids = instrument_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(ids.inst);
    let forked = h.fork(scope);
    let (set, candidate) = (forked.variant_set, forked.candidate);

    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_public,
            value: 0.9,
        },
    );
    h.edit(
        set,
        candidate,
        ProjectOp::AddParameterTarget {
            name: "resonance".into(),
            owner: ControlOwner::Processor(ids.inst),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(capability(8)),
            base: 0.3,
        },
    );

    // Semantic diff: project-truth changes surface as typed changes —
    // never a signal/render substitution.
    let cs = compare(&h.controller.live().clone(), &h.candidate(set, candidate));
    let param_changes: Vec<&SemanticChange> = cs
        .changes
        .iter()
        .filter(|ch| ch.object.kind == ObjectKind::ParameterTarget)
        .collect();
    assert!(param_changes.iter().any(|ch| {
        ch.object.stable_id == ids.param_public.to_tagged() && ch.kind == ChangeKind::Modified
    }));
    assert!(param_changes.iter().any(|ch| ch.kind == ChangeKind::Added));

    // Private parameter: stage-1 binding fails closed — `Unavailable`,
    // never a guessed ParamId.
    let cand = h.candidate(set, candidate);
    let err = semantic_plugin_binding(&cand, ids.param_private).unwrap_err();
    assert!(
        matches!(err, synvelis_project::Error::Validation(_)),
        "capability-less target must reject the semantic binding, got {err:?}"
    );
    // Public parameter binds to (processor, plugin, param_id) and the
    // persisted instance identity resolves.
    let semantic = semantic_plugin_binding(&cand, ids.param_public).unwrap();
    assert_eq!(semantic.processor, ids.inst);
    assert_eq!(semantic.plugin.descriptor, "plug-synth");
    assert_eq!(semantic.param_id, ParamId(7));
    let bound = plugin_parameter_instance_binding(&cand, ids.param_public).unwrap();
    assert_eq!(
        bound.instance,
        Some(cand.plugin_instances.instances[&ids.inst].id)
    );

    // Capability report: display truth == enforcement truth.
    let caps = h.adapter.capabilities();
    assert_eq!(caps.param_base, ProcessingCapability::Supported);
    assert_eq!(caps.automation_edit, ProcessingCapability::Supported);
    assert_eq!(caps.plugin_rebind, ProcessingCapability::Unsupported);
    assert_eq!(caps.preset_select, ProcessingCapability::Unsupported);
    assert_eq!(caps.routing, ProcessingCapability::Unsupported);
    assert_eq!(caps.midi_input, ProcessingCapability::Unsupported);

    // State assessment records the port's achieved quality verbatim.
    assert_state_assessments(ids.inst);
}

/// The TIM-04 quality vocabulary recorded verbatim: exact-eligible
/// qualities (`Quiescent`/`Coordinated`) vs degraded `BestEffort`,
/// `Unsupported` and typed `Failed` — never upgraded, never faked.
fn assert_state_assessments(inst: synvelis_project::id::ProcessorId) {
    for (quality, exact) in [
        (StateCaptureQuality::Coordinated, true),
        (StateCaptureQuality::Quiescent, true),
        (StateCaptureQuality::BestEffort, false),
    ] {
        let mut port = MockPort::default();
        port.bind(inst, quality);
        let assessment =
            InstrumentSlotAdapter::assess_state(&mut port, inst, CaptureMode::Coordinated);
        assert!(matches!(
            assessment.outcome,
            PluginStateOutcome::Captured { quality: q, .. } if q == quality
        ));
        assert_eq!(assessment.exact_eligible(), exact);
        assert_eq!(
            assessment.binding.as_ref().unwrap().descriptor,
            "plug-synth"
        );
    }
    for error in [
        PluginStateCaptureError::Unsupported,
        PluginStateCaptureError::Failed(PluginStateFailureKind::Timeout),
    ] {
        let mut port = MockPort::default();
        port.captures.insert(inst, Err(error.clone()));
        let assessment =
            InstrumentSlotAdapter::assess_state(&mut port, inst, CaptureMode::Coordinated);
        assert!(!assessment.exact_eligible());
        match error {
            PluginStateCaptureError::Unsupported => {
                assert_eq!(assessment.outcome, PluginStateOutcome::Unsupported);
            }
            PluginStateCaptureError::Failed(kind) => assert!(matches!(
                assessment.outcome,
                PluginStateOutcome::Failed { kind: k } if k == kind
            )),
        }
    }
}

// ===========================================================================
// CB-INST-03: shared-context pin + stale detection (headless identity half)
// ===========================================================================

/// The render pin separates scope identity from shared context: a
/// scoped candidate edit moves `scope_digest` only; a shared MIDI
/// source edit moves `shared_context_digest` for *every* branch —
/// the typed staleness signal comparison/render reuse consumes
/// (CB-INST-03; orchestration itself is CB-08).
#[test]
fn cb_inst_03_shared_midi_edit_stale_context_detected() {
    let mut c = controller();
    let ids = instrument_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(ids.inst);
    let forked = h.fork(scope.clone());
    let (set, candidate) = (forked.variant_set, forked.candidate);

    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_public,
            value: 0.9,
        },
    );

    let pin_scoped = h.render_pin(set, candidate);
    let live_scope_before = scope_digest(h.controller.live(), &scope).unwrap();

    // Shared-context edit on live: the MIDI source feeding the slot is
    // NOT scope content — every branch's comparison context goes stale
    // while the scoped digest stays put.
    apply_op(
        &mut h.controller,
        ProjectOp::AddMidiNote {
            source_id: ids.midi_source,
            key: 67,
            velocity: 70,
            start: MidiTick::new(480).unwrap(),
            len: MidiTick::new(48).unwrap(),
        },
    );

    let pin_after = h.render_pin(set, candidate);
    assert_eq!(
        pin_scoped.scope_digest, pin_after.scope_digest,
        "shared-context edits must not move the scoped render identity"
    );
    assert_ne!(
        pin_scoped.shared_context_digest, pin_after.shared_context_digest,
        "shared MIDI edit must stale every branch's comparison context"
    );
    // The adopted/live scope fingerprint is likewise unmoved — the
    // staleness is context-level, not a scope CAS failure.
    assert_eq!(
        live_scope_before,
        scope_digest(h.controller.live(), &scope).unwrap()
    );
    // The candidate payload itself is untouched by the live edit.
    let cand = h.candidate(set, candidate);
    assert_eq!(cand.control.targets[&ids.param_public].base, 0.9);
}

// ===========================================================================
// CB-INST-04: adopt B — slot+binding reference validation, one CAS,
// A/C and MIDI source unchanged, undo/redo keeps candidates
// ===========================================================================

#[test]
fn cb_inst_04_adopt_swaps_slot_one_cas_undo_redo() {
    let mut c = controller();
    let ids = instrument_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(ids.inst);
    let forked = h.fork(scope.clone());
    let (set, adopted_a, candidate_b) = (forked.variant_set, forked.adopted, forked.candidate);

    h.edit(
        set,
        candidate_b,
        ProjectOp::SetParameterBase {
            target: ids.param_public,
            value: 0.9,
        },
    );
    h.edit(
        set,
        candidate_b,
        ProjectOp::AddParameterTarget {
            name: "resonance".into(),
            owner: ControlOwner::Processor(ids.inst),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(capability(8)),
            base: 0.3,
        },
    );

    let midi_before = midi_state_json(&h.controller);
    let sibling_before =
        serde_json::to_value(&h.controller.live().routing.processors[&ids.sibling]).unwrap();
    let instance_before = h.controller.live().plugin_instances.instances[&ids.inst].id;
    let rev_before = h.controller.live_revision();

    let receipt = h.adopt(set, candidate_b, &scope);
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(receipt.project.undo_steps, 1);
    assert_eq!(receipt.after_revision.0, rev_before.0 + 1);

    let live = h.controller.live();
    // Slot+binding transplant: scoped control state landed; the
    // processor's plugin binding (and therefore its persisted
    // PluginInstanceId — `sync_processor` keeps the id for an
    // unchanged plugin) is preserved, never silently re-minted.
    assert_eq!(live.control.targets[&ids.param_public].base, 0.9);
    assert!(live.control.targets.values().any(|t| {
        t.owner == ControlOwner::Processor(ids.inst)
            && t.name == "resonance"
            && t.role == ParameterRole::ProcessorParam
    }));
    assert_eq!(
        live.plugin_instances.instances[&ids.inst].id, instance_before,
        "unchanged plugin binding must keep its persisted instance id"
    );
    // MIDI source and sibling processor bitwise unchanged.
    assert_eq!(midi_state_json(&h.controller), midi_before);
    assert_eq!(
        serde_json::to_value(&live.routing.processors[&ids.sibling]).unwrap(),
        sibling_before
    );

    // The previously adopted branch froze as a captured candidate
    // retaining the pre-adopt (A) scoped state.
    let set_state = &live.variant_sets[&set];
    let frozen = &set_state.branches[&adopted_a].content;
    let BranchContentRef::Captured { .. } = frozen else {
        panic!("previously adopted branch must freeze to captured content");
    };
    let frozen_project = decode_candidate(&h.store, frozen).unwrap();
    assert_eq!(frozen_project.control.targets[&ids.param_public].base, 0.5);
    assert!(
        !frozen_project
            .control
            .targets
            .values()
            .any(|t| t.name == "resonance")
    );

    // One undo restores the exact pre-adopt scope; redo re-applies —
    // candidates survive both directions.
    assert!(h.controller.undo().unwrap());
    assert_eq!(
        h.controller.live().control.targets[&ids.param_public].base,
        0.5
    );
    assert!(
        !h.controller
            .live()
            .control
            .targets
            .values()
            .any(|t| { t.owner == ControlOwner::Processor(ids.inst) && t.name == "resonance" })
    );
    assert!(h.controller.redo().unwrap());
    assert_eq!(
        h.controller.live().control.targets[&ids.param_public].base,
        0.9
    );
    // The adopted branch is still the (former) candidate; the frozen
    // branch still carries A's content.
    assert_eq!(h.controller.live().variant_sets[&set].adopted, candidate_b);
}

// ===========================================================================
// CB-INST-05: typed gates — recording-active, missing plugin, state
// inability, unsupported/out-of-scope ops — live stays untouched
// ===========================================================================

struct NoPlugins;

impl PluginAvailability for NoPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        false
    }
}

/// Injects an open recording session + capture stream on `lane`
/// directly into project state (the recording registry is runtime
/// data with no `ProjectOp` — the cb02 fixture convention).
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
fn cb_inst_05_adopt_while_recording_rejects_typed() {
    let mut c = controller();
    let ids = instrument_fixture(&mut c);
    let lane = ids.lane;
    let c = with_open_recording(&c, lane);
    let mut h = Fx::new(c);
    let scope = scope_of(ids.inst);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_public,
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

#[test]
fn cb_inst_05_missing_plugin_and_state_gates_are_honest() {
    let mut c = controller();
    let ids = instrument_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(ids.inst);
    let forked = h.fork(scope.clone());
    let (set, candidate) = (forked.variant_set, forked.candidate);
    h.edit(
        set,
        candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_public,
            value: 0.9,
        },
    );

    // Missing plugin: the candidate introduces NO new resolved
    // binding — the scoped plugin identity is unchanged, so the
    // project-truth adopt proceeds (runtime availability is a
    // separate receipt domain, never a project blocker).
    let receipt = h
        .try_adopt_plugins(set, candidate, &scope, Some(&NoPlugins))
        .unwrap();
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    assert_eq!(
        h.controller.live().control.targets[&ids.param_public].base,
        0.9
    );
}

#[test]
fn cb_inst_05_unsupported_and_out_of_scope_ops_reject_typed() {
    let mut c = controller();
    let ids = instrument_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(ids.inst);
    let forked = h.fork(scope.clone());
    let (set, candidate) = (forked.variant_set, forked.candidate);

    // Non-Project edit payloads are rejected before any apply.
    let err = h
        .try_edit_op(
            set,
            candidate,
            BranchEditOp::Comp(CompOp::RemoveComp {
                comp_id: CompId::new(999),
            }),
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "comp edits are outside the instrument surface, got {err:?}"
    );

    assert_surface_rejections(&mut h, set, candidate, &ids);

    // Live and candidate are untouched by every rejection.
    assert_eq!(
        h.controller.live().control.targets[&ids.param_public].base,
        0.5
    );
    assert_eq!(midi_state_json(&h.controller), {
        let c = h.candidate(set, candidate);
        midi_state_json_of(&c)
    });

    // Scope-variant mirror: the adapter rejects foreign scopes typed.
    let audio_scope = VariantScope::MidiClip { clip: ids.clip };
    let err = h
        .adapter
        .validate_scope(h.controller.live(), &audio_scope)
        .unwrap_err();
    assert!(matches!(err, VariantError::InvalidScope { .. }));
    let midi_adapter = MidiClipAdapter::new();
    let err = midi_adapter
        .validate_scope(h.controller.live(), &scope)
        .unwrap_err();
    assert!(matches!(err, VariantError::InvalidScope { .. }));
}

/// Every op outside the instrument-slot candidate surface rejects
/// typed — shared-context mutations `Unsupported`, out-of-scope
/// control entities `ScopeViolation`, scope-entity removal
/// `Unsupported`. Nothing is silently applied or normalized.
fn assert_surface_rejections(h: &mut Fx, set: VariantSetId, candidate: BranchId, ids: &InstIds) {
    // Shared-context mutations: chain structure, MIDI input — all
    // Unsupported (fail-closed, never silently applied).
    for op in [
        ProjectOp::RemoveMidiSource {
            source_id: ids.midi_source,
        },
        ProjectOp::AddMidiNote {
            source_id: ids.midi_source,
            key: 60,
            velocity: 90,
            start: MidiTick::new(0).unwrap(),
            len: MidiTick::new(60).unwrap(),
        },
    ] {
        let err = h.try_edit(set, candidate, op).unwrap_err();
        assert!(
            matches!(err, VariantError::Unsupported { .. }),
            "shared-context op must reject Unsupported, got {err:?}"
        );
    }

    // Processor insert is chain structure — never candidate-owned.
    let err = h
        .try_edit(
            set,
            candidate,
            ProjectOp::InsertProcessor {
                strip_id: h
                    .controller
                    .live()
                    .routing
                    .strip_of_track(ids.track)
                    .unwrap()
                    .id,
                index: 0,
                name: "extra".into(),
                binding: ProcessorBinding::Unresolved {
                    descriptor: "plug-x".into(),
                },
                sidechain_input: false,
            },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));

    // Control ops naming entities outside the scoped closure are
    // ScopeViolation — including creation ops for another owner.
    for op in [
        ProjectOp::SetParameterBase {
            target: ids.param_sibling,
            value: 0.1,
        },
        ProjectOp::AddParameterTarget {
            name: "intruder".into(),
            owner: ControlOwner::Processor(ids.sibling),
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

    // Scope-entity removal fails closed (Unsupported) — candidate
    // content may add but never silently drop owned objects.
    let err = h
        .try_edit(
            set,
            candidate,
            ProjectOp::RemoveParameterTarget {
                target: ids.param_private,
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "scope-entity removal must reject Unsupported, got {err:?}"
    );
}

// ===========================================================================
// CB-INST-RCP-01: Missing Plugin × State/Port quality matrix —
// Safe Edit/Adopt vs Unknown Reject vs Exact Artifact vs BitExact
// ===========================================================================

/// Scripted plugin-state port (`plugin_state.rs` test conventions):
/// per-processor binding + scripted capture outcomes.
#[derive(Default)]
struct MockPort {
    bindings: BTreeMap<synvelis_project::id::ProcessorId, PluginStateBinding>,
    captures: BTreeMap<
        synvelis_project::id::ProcessorId,
        Result<PluginStateCapture, PluginStateCaptureError>,
    >,
}

impl MockPort {
    fn bind(&mut self, processor: synvelis_project::id::ProcessorId, quality: StateCaptureQuality) {
        self.bindings.insert(
            processor,
            PluginStateBinding {
                descriptor: "plug-synth".into(),
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
    fn describe(&self, processor: synvelis_project::id::ProcessorId) -> Option<PluginStateBinding> {
        self.bindings.get(&processor).cloned()
    }

    fn capture(
        &mut self,
        processor: synvelis_project::id::ProcessorId,
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
        _processor: synvelis_project::id::ProcessorId,
        _bytes: &[u8],
    ) -> Result<(), PluginStateCaptureError> {
        Ok(())
    }
}

/// One matrix cell: fresh fixture → fork → scoped edit → scripted
/// state assessment → adopt under the given plugin-availability probe.
/// A variant set owns its scope for its whole lifetime, so each cell
/// gets its own project — there is no re-fork on a live scope.
fn rcp_cell(
    plugins: Option<&dyn PluginAvailability>,
    capture: Result<StateCaptureQuality, PluginStateCaptureError>,
) -> (
    InstrumentStateAssessment,
    synvelis_project::variant::AdoptReceipt,
) {
    let mut c = controller();
    let ids = instrument_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = scope_of(ids.inst);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetParameterBase {
            target: ids.param_public,
            value: 0.9,
        },
    );

    let mut port = MockPort::default();
    match capture {
        Ok(quality) => port.bind(ids.inst, quality),
        Err(error) => {
            port.captures.insert(ids.inst, Err(error));
        }
    }
    let assessment =
        InstrumentSlotAdapter::assess_state(&mut port, ids.inst, CaptureMode::Coordinated);
    let receipt = h
        .try_adopt_plugins(forked.variant_set, forked.candidate, &scope, plugins)
        .expect("project-truth adopt must never be blocked by state/plugin absence");
    assert_eq!(receipt.outcome, AdoptOutcome::Applied);
    (assessment, receipt)
}

/// The matrix CB-INST-RCP-01 demands: for every (plugin availability ×
/// state outcome) cell the adapter keeps the three distinctions
/// honest — Safe Edit/Adopt (project truth never blocked by runtime
/// absence), Exact Artifact (a captured blob+digest exists) vs
/// `BitExact` regenerate (`exact_eligible` only at `Quiescent`/
/// `Coordinated` — `BestEffort` is never promoted, and
/// Unsupported/Failed carry no artifact at all).
#[test]
fn cb_inst_rcp_01_missing_plugin_x_state_quality_matrix() {
    let cells: [(
        &str,
        Result<StateCaptureQuality, PluginStateCaptureError>,
        bool,
        bool,
    ); 5] = [
        (
            "coordinated",
            Ok(StateCaptureQuality::Coordinated),
            true,
            true,
        ),
        ("quiescent", Ok(StateCaptureQuality::Quiescent), true, true),
        (
            "best_effort",
            Ok(StateCaptureQuality::BestEffort),
            false,
            true,
        ),
        (
            "unsupported",
            Err(PluginStateCaptureError::Unsupported),
            false,
            false,
        ),
        (
            "failed",
            Err(PluginStateCaptureError::Failed(
                PluginStateFailureKind::Crashed,
            )),
            false,
            false,
        ),
    ];
    for (name, capture, expect_exact, expect_artifact) in cells {
        for plugins in [None, Some(&NoPlugins as &dyn PluginAvailability)] {
            let (assessment, _receipt) = rcp_cell(plugins, capture.clone());
            assert_eq!(
                assessment.exact_eligible(),
                expect_exact,
                "cell {name}: BitExact regenerate eligibility"
            );
            let is_artifact = matches!(assessment.outcome, PluginStateOutcome::Captured { .. });
            assert_eq!(
                is_artifact, expect_artifact,
                "cell {name}: Exact Artifact existence"
            );
            // The recorded outcome stays in the TIM-04 vocabulary —
            // quality/failure kinds are never normalized away.
            match capture {
                Ok(quality) => assert!(matches!(
                    assessment.outcome,
                    PluginStateOutcome::Captured { quality: q, .. } if q == quality
                )),
                Err(PluginStateCaptureError::Unsupported) => {
                    assert_eq!(assessment.outcome, PluginStateOutcome::Unsupported);
                }
                Err(PluginStateCaptureError::Failed(kind)) => assert!(matches!(
                    assessment.outcome,
                    PluginStateOutcome::Failed { kind: k } if k == kind
                )),
            }
        }
    }
}

// ---------- diff harness (cb03 snap_manual/compare conventions) ----

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
