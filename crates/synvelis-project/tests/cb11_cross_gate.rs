//! CB-11 Creative Branch cross-gate headless fixtures (synvelis#288).
//!
//! This is the *runnable headless subset* of the CB-11 coverage
//! inventory (`docs/test-specs/cb-11-cross-gate.md`,
//! `evidence/sv288/coverage.md`): it drives the merged CB-01..06
//! domain adapters, the VCS-02 semantic diff and the RCP branch
//! render-pin resolver together in one project so the cross-domain
//! invariants are pinned, not re-proven per adapter:
//!
//! - fork/edit/adopt/undo/save/reload across `AudioClip`, `MidiClip`,
//!   `InstrumentSlot`, `ProcessingRange` and `StripFields` scopes in a
//!   single project (`TakeUsage` is exercised in its own fixture
//!   because the recording registry has no `ProjectOp` seed path);
//! - adopt = one CAS = one undo unit per domain, with full
//!   pre-adopt restoration on undo and verbatim persistence across
//!   save/reload (provenance digests included);
//! - raw material (`Asset`/`Source`/`Take`/recording registry) is
//!   bitwise untouched by the whole fork/edit/adopt/undo/reload
//!   lifecycle;
//! - `resolve_render_project` pins move `scope_digest` only for the
//!   edited scope while a shared-context edit moves
//!   `shared_context_digest` for every branch — the CB-X-RCP-01
//!   invalidation half that is runnable headlessly;
//! - `compare_snapshots` reports exactly the scoped semantic changes
//!   and never reports raw-material churn;
//! - a stale-scope adopt rejection commits nothing and leaves other
//!   domains' candidates untouched (CB-E2E-05 cross-gate half).
//!
//! Real OS / GUI / plugin / RT / PCM / consumer lanes are `NotRun` —
//! recorded in `evidence/sv288/external-gates.md`. Headless green
//! here is a headless contract pass only, never product acceptance.

use std::collections::BTreeSet;

use ruviform_plugin_core::ParamFlags;
use synvelis_project::apply::ApplyApproval;
use synvelis_project::control::{
    AutomationTimeScope, ControlOwner, ParamMapping, ParameterRole, TargetCapability, ValueKind,
};
use synvelis_project::controller::{Command, ProjectController};
use synvelis_project::diff::{
    ChangeKind, ChangeSet, ComparisonEquality, ComparisonSpec, ObjectKind, compare_snapshots,
};
use synvelis_project::id::CounterIdGenerator;
use synvelis_project::project::Project;
use synvelis_project::routing::{PluginRef, ProcessorBinding};
use synvelis_project::snapshot::{
    BlobProbe, BlobRead, ImmutableSnapshot, SnapshotCancel, SnapshotResolver,
};
use synvelis_project::store::InMemoryStore;
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, ArrangementAdapter, AudioClipAdapter, BranchEditOp,
    BranchEditOpIntent, DomainAdapter, InMemoryBranchContentStore, InstrumentSlotAdapter,
    MidiClipAdapter, NoopRuntimePublish, ProcessingRangeAdapter, ProcessingRangeRef,
    StripDomainMask, StripField, StripFieldsAdapter, TakeUsageAdapter, TakeUsageTarget,
    VariantError, VariantScope, adopt_branch_adapted, decode_candidate, edit_branch_op,
    fork_variant_set, scope_digest,
};
use synvelis_project::{
    Asset, AssetId, BranchId, CaptureSegment, CaptureSegmentId, CaptureStream, CaptureStreamId,
    ChainId, ClipExtent, ClipId, ClipPlacement, ClipPlayback, ClockEpoch, ClockEpochId,
    CompCandidateRef, CompId, CompOp, CompRegionId, Lane, LaneId, MidiDivision, MidiTick,
    MusicalDuration, MusicalRange, MusicalTick, NoteId, PlaybackMapping, ProjectId, ProjectOp,
    ProjectRange, RecordingPass, RecordingPassId, RecordingSession, RecordingSessionId,
    RenderBranchTarget, SessionFrame, SessionRange, SessionState, Source, SourceFrame, SourceId,
    SourceRange, SourceState, Take, TakeGroupId, TakeId, TakeState, TimeAnchor, Track, TrackId,
    VariantSetId, is_issuable_raw, resolve_render_project,
};

// ---------- shared harness (cb02..cb06 conventions, multi-adapter) ----------

/// Picks the merged domain adapter for a scope — the same dispatch a
/// real caller performs; the variant set itself does not pin an
/// adapter, each operation re-validates through the supplied one.
fn adapter_for(scope: &VariantScope) -> Box<dyn DomainAdapter> {
    match scope {
        VariantScope::AudioClip { .. } => Box::new(AudioClipAdapter::new()),
        VariantScope::TakeUsage { .. } => Box::new(TakeUsageAdapter::new()),
        VariantScope::MidiClip { .. } => Box::new(MidiClipAdapter::new()),
        VariantScope::InstrumentSlot { .. } => Box::new(InstrumentSlotAdapter::new()),
        VariantScope::ProcessingRange { .. } => Box::new(ProcessingRangeAdapter::new()),
        VariantScope::StripFields { .. } => Box::new(StripFieldsAdapter::new()),
        VariantScope::Arrangement { .. } => Box::new(ArrangementAdapter::new()),
    }
}

struct Gate {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    resolver: SnapshotResolver,
}

impl Gate {
    fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            resolver: SnapshotResolver::new(),
        }
    }

    fn fork(&mut self, scope: &VariantScope) -> synvelis_project::variant::ForkReceipt {
        let adapter = adapter_for(scope);
        fork_variant_set(
            &mut self.controller,
            adapter.as_ref(),
            &mut self.store,
            &self.resolver,
            scope.clone(),
            &SnapshotCancel::new(),
        )
        .unwrap()
    }

    fn try_edit_op(
        &mut self,
        scope: &VariantScope,
        set: VariantSetId,
        branch: BranchId,
        op: BranchEditOp,
    ) -> Result<(), VariantError> {
        let expected = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .revision();
        let adapter = adapter_for(scope);
        edit_branch_op(
            &mut self.controller,
            adapter.as_ref(),
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

    fn edit(
        &mut self,
        scope: &VariantScope,
        set: VariantSetId,
        branch: BranchId,
        op: BranchEditOp,
    ) {
        self.try_edit_op(scope, set, branch, op).unwrap();
    }

    fn candidate(&self, set: VariantSetId, branch: BranchId) -> Project {
        let content = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .clone();
        decode_candidate(&self.store, &content).unwrap()
    }

    fn content_ref(
        &self,
        set: VariantSetId,
        branch: BranchId,
    ) -> synvelis_project::variant::BranchContentRef {
        self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .clone()
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

    fn try_adopt(
        &mut self,
        scope: &VariantScope,
        intent: &AdoptIntent,
    ) -> Result<synvelis_project::variant::AdoptReceipt, VariantError> {
        let adapter = adapter_for(scope);
        let runtime = NoopRuntimePublish;
        adopt_branch_adapted(
            &mut self.controller,
            adapter.as_ref(),
            &mut self.store,
            &AdoptSources {
                assets: None,
                plugins: None,
                runtime: &runtime,
            },
            intent,
        )
    }

    fn adopt(
        &mut self,
        scope: &VariantScope,
        set: VariantSetId,
        branch: BranchId,
    ) -> synvelis_project::variant::AdoptReceipt {
        let intent = self.intent(set, branch, scope);
        self.try_adopt(scope, &intent).unwrap()
    }

    /// Canonical render-reuse pin — the RCP-01 read-adapter identity.
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

    /// Live-capture snapshot for the VCS-02 semantic diff — every
    /// probed blob reports present so asset availability never adds
    /// `UnresolvedSource` noise.
    fn snap(&self) -> ImmutableSnapshot {
        self.resolver
            .capture_live(
                &self.controller,
                self.controller.live_revision(),
                &SnapshotCancel::new(),
                Some(&AllPresentProbe),
            )
            .unwrap()
    }
}

/// Reports every probed digest as present (vcs02 convention).
struct AllPresentProbe;

impl BlobProbe for AllPresentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Present(vec![0])
    }
}

fn compare(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ChangeSet {
    compare_snapshots(
        before,
        after,
        &ComparisonSpec::new(before.identity.clone(), after.identity.clone()),
        &SnapshotCancel::new(),
    )
    .unwrap()
}

// ---------- fixture: five merged domains in one project ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb11-gate"),
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

/// Valid-format fixture digest — the manifest probe marks malformed
/// digests `Unknown`, which would poison comparisons (vcs02 convention).
fn sha(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn clip_placement(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
    }
}

fn audio_playback() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn midi_playback() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(4 * 960).unwrap())
            .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
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
) -> synvelis_project::ParameterTargetId {
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

/// The seeded `ControlOwner::Strip` + channel-role target of `strip`
/// (cb06 convention).
fn strip_channel_param(
    project: &Project,
    strip: synvelis_project::StripId,
    role: ParameterRole,
) -> synvelis_project::ParameterTargetId {
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

struct GateIds {
    clip_audio: ClipId,
    clip_midi: ClipId,
    midi_source: SourceId,
    midi_note: NoteId,
    chain1: ChainId,
    inst: synvelis_project::ProcessorId,
    fx1: synvelis_project::ProcessorId,
    fx2: synvelis_project::ProcessorId,
    param_inst: synvelis_project::ParameterTargetId,
    param_fx1: synvelis_project::ParameterTargetId,
    /// Strip1's seeded fader target — shared context for every scope.
    param_fader1: synvelis_project::ParameterTargetId,
    strip2: synvelis_project::StripId,
    param_mute2: synvelis_project::ParameterTargetId,
}

/// One project carrying five disjoint merged-domain scopes:
/// `AudioClip` + `MidiClip` on track1's two lanes, `InstrumentSlot` +
/// `ProcessingRange` on track1's strip chain `[inst, fx1, fx2]`, and
/// `StripFields` on track2's strip. The MIDI source is exclusively
/// owned by the MIDI clip (CB-03 exclusivity rule); the media source
/// is shared raw material; strip1's channel targets are the
/// shared-context witness no scope owns.
#[allow(clippy::too_many_lines)] // sequential fixture setup reads top-to-bottom (cb02 convention)
fn gate_fixture() -> (ProjectController, GateIds) {
    let mut c = controller();

    // Track1: two lanes.
    apply_op(&mut c, ProjectOp::AddTrack { name: "t1".into() });
    let track1 = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track1 });
    apply_op(&mut c, ProjectOp::AddLane { track_id: track1 });
    let mut lanes: Vec<LaneId> = c.live().lanes.keys().copied().collect();
    lanes.sort();
    let (lane_audio, lane_midi) = (lanes[0], lanes[1]);

    // Shared media source → audio clip on lane_audio.
    apply_op(&mut c, ProjectOp::RegisterAsset { digest: sha('a') });
    let asset = *c.live().assets.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: sha('1'),
        },
    );
    let source_audio = *c.live().sources.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane_audio,
            source_id: source_audio,
            placement: clip_placement(0),
            playback: audio_playback(),
        },
    );
    let clip_audio = *c.live().clips.keys().next().unwrap();

    // Exclusive MIDI source → MIDI clip on lane_midi.
    apply_op(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let midi_source = *c.live().midi_sources.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: midi_source,
            key: 60,
            velocity: 100,
            start: MidiTick::new(0).unwrap(),
            len: MidiTick::new(120).unwrap(),
        },
    );
    let midi_note = *c.live().midi_sources[&midi_source]
        .notes
        .keys()
        .next()
        .unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane_midi,
            source_id: midi_source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
            },
            playback: midi_playback(),
        },
    );
    let clip_midi = c
        .live()
        .clips
        .values()
        .find(|clip| clip.lane_id == lane_midi)
        .unwrap()
        .id;

    // Track1 strip: chain [inst, fx1, fx2] — InstrumentSlot owns
    // `inst`, ProcessingRange owns `fx1..=fx2`.
    apply_op(&mut c, ProjectOp::AddTrackStrip { track_id: track1 });
    let strip1 = c.live().routing.strip_of_track(track1).unwrap().id;
    let chain1 = c.live().routing.strips[&strip1].chain;
    for (index, name, desc) in [
        (0usize, "synth", "plug-synth"),
        (1usize, "fx-a", "plug-a"),
        (2usize, "fx-b", "plug-b"),
    ] {
        apply_op(
            &mut c,
            ProjectOp::InsertProcessor {
                strip_id: strip1,
                index,
                name: name.into(),
                binding: plugin_binding(desc),
                sidechain_input: false,
            },
        );
    }
    let ordered = c.live().routing.chains[&chain1].processors.clone();
    let [inst, fx1, fx2] = ordered[..] else {
        panic!("fixture chain must hold exactly three processors");
    };
    let param_inst = add_target(
        &mut c,
        ControlOwner::Processor(inst),
        "cutoff",
        Some(capability(7)),
        0.5,
    );
    let param_fx1 = add_target(
        &mut c,
        ControlOwner::Processor(fx1),
        "drive",
        Some(capability(3)),
        0.5,
    );
    let param_fader1 = strip_channel_param(c.live(), strip1, ParameterRole::Fader);

    // Track2 strip: the StripFields scope root (channel fields only —
    // no Inserts, so its chain stays shared context).
    apply_op(&mut c, ProjectOp::AddTrack { name: "t2".into() });
    let track2 = *c.live().tracks.keys().next_back().unwrap();
    apply_op(&mut c, ProjectOp::AddTrackStrip { track_id: track2 });
    let strip2 = c.live().routing.strip_of_track(track2).unwrap().id;
    let param_mute2 = strip_channel_param(c.live(), strip2, ParameterRole::Mute);

    let ids = GateIds {
        clip_audio,
        clip_midi,
        midi_source,
        midi_note,
        chain1,
        inst,
        fx1,
        fx2,
        param_inst,
        param_fx1,
        param_fader1,
        strip2,
        param_mute2,
    };
    (c, ids)
}

fn scope_audio(clip: ClipId) -> VariantScope {
    VariantScope::AudioClip { clip }
}

fn scope_midi(clip: ClipId) -> VariantScope {
    VariantScope::MidiClip { clip }
}

fn scope_inst(processor: synvelis_project::ProcessorId) -> VariantScope {
    VariantScope::InstrumentSlot { processor }
}

fn scope_fx(
    chain: ChainId,
    start: synvelis_project::ProcessorId,
    end: synvelis_project::ProcessorId,
) -> VariantScope {
    VariantScope::ProcessingRange {
        range: ProcessingRangeRef { chain, start, end },
    }
}

fn scope_strip(strip: synvelis_project::StripId) -> VariantScope {
    VariantScope::StripFields {
        strip,
        fields: StripDomainMask(BTreeSet::from([StripField::Fader, StripField::Mute])),
    }
}

/// Serialized shared-raw witness: media assets, media sources and the
/// whole recording registry — entities no merged scope may ever own.
/// (`MidiSource`/notes are deliberately absent: for `MidiClip` scopes
/// they ARE scope-owned candidate content, cb-03 semantics.)
fn raw_snapshot(c: &ProjectController) -> serde_json::Value {
    serde_json::json!({
        "assets": c.live().assets,
        "sources": c.live().sources,
        "recording": c.live().recording,
    })
}

/// The five domain scopes of `gate_fixture`, in fork order.
fn all_scopes(ids: &GateIds) -> Vec<VariantScope> {
    vec![
        scope_audio(ids.clip_audio),
        scope_midi(ids.clip_midi),
        scope_inst(ids.inst),
        scope_fx(ids.chain1, ids.fx1, ids.fx2),
        scope_strip(ids.strip2),
    ]
}

/// One domain-meaningful candidate edit per scope — the smallest op
/// each merged adapter accepts that moves scoped content.
fn domain_edit(scope: &VariantScope, ids: &GateIds) -> BranchEditOp {
    match scope {
        VariantScope::AudioClip { .. } => BranchEditOp::Project(ProjectOp::SetClipMuted {
            clip_id: ids.clip_audio,
            muted: true,
        }),
        VariantScope::MidiClip { .. } => BranchEditOp::Project(ProjectOp::SetMidiNoteVelocity {
            source_id: ids.midi_source,
            note_id: ids.midi_note,
            velocity: 64,
        }),
        VariantScope::InstrumentSlot { .. } => BranchEditOp::Project(ProjectOp::SetParameterBase {
            target: ids.param_inst,
            value: 0.9,
        }),
        VariantScope::ProcessingRange { .. } => {
            BranchEditOp::Project(ProjectOp::SetParameterBase {
                target: ids.param_fx1,
                value: 0.25,
            })
        }
        VariantScope::StripFields { .. } => BranchEditOp::Project(ProjectOp::SetStripMute {
            strip_id: ids.strip2,
            muted: true,
        }),
        VariantScope::Arrangement { .. } => unreachable!("no arrangement adapter"),
        VariantScope::TakeUsage { .. } => unreachable!("take fixture is separate"),
    }
}

// ===========================================================================
// CB-E2E-01 (Project axis): five domains fork/edit in one project —
// candidate edits stay inside their scope and never touch live domain
// state or sibling candidates.
// ===========================================================================

#[test]
fn cb11_e2e_01_fork_edit_isolation_across_five_domains() {
    let (c, ids) = gate_fixture();
    let mut g = Gate::new(c);
    let scopes = all_scopes(&ids);
    let domain_before = raw_snapshot(&g.controller);

    // Fork all five scopes — disjoint ownership must not collide.
    let mut forks = Vec::new();
    for scope in &scopes {
        forks.push(g.fork(scope));
    }
    let set_ids: BTreeSet<VariantSetId> = forks.iter().map(|f| f.variant_set).collect();
    assert_eq!(set_ids.len(), 5, "each domain forks its own variant set");

    // One candidate edit per domain.
    let mut edited = Vec::new();
    for (scope, forked) in scopes.iter().zip(forks.iter()) {
        g.edit(
            scope,
            forked.variant_set,
            forked.candidate,
            domain_edit(scope, &ids),
        );
        edited.push((forked.variant_set, forked.candidate));
    }

    // Live raw/domain witnesses: fork+edit moved only variant
    // bookkeeping — assets, sources, MIDI sources, recording and the
    // clip/strip/processor domain entities are untouched. (The full
    // domain JSON is asserted in the adopt test; here the raw witness
    // plus candidate isolation is the point.)
    assert_eq!(
        raw_snapshot(&g.controller),
        domain_before,
        "fork/edit must never touch shared raw material"
    );

    // Each edited candidate's scope digest moved; sibling candidates
    // kept their fork-time payload digests (content immutability).
    let mut digests = BTreeSet::new();
    for (set, branch) in &edited {
        let content = g.content_ref(*set, *branch);
        let synvelis_project::variant::BranchContentRef::Captured { scope_digest, .. } = content
        else {
            panic!("unedited-then-edited candidate must stay Captured");
        };
        digests.insert(scope_digest);
    }
    assert_eq!(
        digests.len(),
        5,
        "five disjoint scopes must yield five distinct scope digests"
    );
}

// ===========================================================================
// CB-E2E-01 (Project axis): adopt = 1 CAS = 1 undo unit per domain;
// undo×5 restores the exact pre-adopt project; redo×5 restores the
// post-adopt project; save/reload preserves sets, lineage and payload
// digests verbatim.
// ===========================================================================

#[test]
#[allow(clippy::too_many_lines)] // sequential adopt/undo/redo/save lifecycle fixture (cb03 convention)
fn cb11_e2e_01_adopt_single_cas_undo_save_reload_across_domains() {
    let (c, ids) = gate_fixture();
    let mut g = Gate::new(c);
    let scopes = all_scopes(&ids);

    let mut forks = Vec::new();
    for scope in &scopes {
        let f = g.fork(scope);
        g.edit(scope, f.variant_set, f.candidate, domain_edit(scope, &ids));
        forks.push(f);
    }

    let pre_adopt = g.controller.live().clone();
    let mut post_adopt_digests = Vec::new();

    // Adopt all five — each is exactly one project CAS pushing one
    // undo step, with project-commit and runtime-publish receipts
    // kept separate.
    for (scope, forked) in scopes.iter().zip(forks.iter()) {
        let rev_before = g.controller.live_revision();
        let receipt = g.adopt(scope, forked.variant_set, forked.candidate);
        assert!(receipt.project.committed);
        assert_eq!(receipt.project.undo_steps, 1, "adopt is one undo unit");
        assert_eq!(
            receipt.after_revision.0,
            rev_before.0 + 1,
            "adopt commits exactly one CAS"
        );
        // The adopted candidate becomes the set's Live branch; the
        // previous live-mirror freezes to Captured.
        let set = &g.controller.live().variant_sets[&forked.variant_set];
        assert_eq!(set.adopted, forked.candidate);
        assert_eq!(set.last_adopt.as_ref().unwrap().branch, forked.candidate);
        post_adopt_digests.push(g.content_ref(forked.variant_set, forked.candidate));
    }

    let post_adopt = g.controller.live().clone();

    // Scoped content materialized: muted clip, retimed MIDI note,
    // both processor params, muted strip2.
    let live = g.controller.live();
    assert!(live.clips[&ids.clip_audio].muted);
    assert_eq!(
        live.midi_sources[&ids.midi_source].notes[&ids.midi_note].velocity,
        64
    );
    assert_eq!(live.control.targets[&ids.param_inst].base, 0.9);
    assert_eq!(live.control.targets[&ids.param_fx1].base, 0.25);
    assert_eq!(
        live.control.targets[&ids.param_mute2].base, 1.0,
        "strip2 mute target must reflect the adopted mute"
    );

    // Raw material untouched by the whole lifecycle.
    assert_eq!(
        raw_snapshot(&g.controller),
        raw_snapshot_of(&pre_adopt),
        "adopt must not touch shared raw material"
    );

    // Undo ×5 → exact pre-adopt project (variant bookkeeping included).
    for _ in 0..5 {
        assert!(g.controller.undo().unwrap());
    }
    assert_eq!(
        *g.controller.live(),
        pre_adopt,
        "five undos must restore the exact pre-adopt project"
    );

    // Redo ×5 → exact post-adopt project.
    for _ in 0..5 {
        assert!(g.controller.redo().unwrap());
    }
    assert_eq!(
        *g.controller.live(),
        post_adopt,
        "five redos must restore the exact post-adopt project"
    );

    // Provenance verbatim: every adopted branch reports a Live ref —
    // its content identity is the live scope by construction.
    for digest in &post_adopt_digests {
        assert!(
            matches!(
                digest,
                synvelis_project::variant::BranchContentRef::Live { .. }
            ),
            "adopted branch must be Live, got {digest:?}"
        );
    }

    // Save → reload: variant sets, adopted pointers, content refs and
    // the whole project round-trip verbatim.
    let mut store = InMemoryStore::new();
    g.controller.save(&mut store).unwrap();
    let mut reloaded = ProjectController::new(
        Project::new(ProjectId::new(1), "cb11-gate"),
        Box::new(CounterIdGenerator::new(1)),
    );
    reloaded.load(&store).unwrap();
    assert_eq!(
        *reloaded.live(),
        *g.controller.live(),
        "save/reload must round-trip the project verbatim"
    );
    assert_eq!(reloaded.live().variant_sets.len(), 5);
    for forked in &forks {
        let set = &reloaded.live().variant_sets[&forked.variant_set];
        assert_eq!(set.adopted, forked.candidate);
        assert!(
            set.last_adopt.is_some(),
            "adopt record must persist across reload"
        );
        assert_eq!(set.last_adopt.as_ref().unwrap().branch, forked.candidate);
        // The adopted candidate stays Live after reload — its content
        // identity is the live scope by construction.
        let adopted_ref = &set.branches[&forked.candidate].content;
        assert!(
            matches!(
                adopted_ref,
                synvelis_project::variant::BranchContentRef::Live { .. }
            ),
            "adopted branch stays Live after reload"
        );
        // The frozen previous live-mirror keeps its Captured payload.
        assert!(
            matches!(
                set.branches[&forked.adopted].content,
                synvelis_project::variant::BranchContentRef::Captured { .. }
            ),
            "previous live-mirror freezes to Captured"
        );
    }
}

fn raw_snapshot_of(p: &Project) -> serde_json::Value {
    serde_json::json!({
        "assets": p.assets,
        "sources": p.sources,
        "recording": p.recording,
    })
}

// ===========================================================================
// CB-E2E-01 (Take axis): TakeUsage comp edit → adopt → undo →
// save/reload with the recording registry bitwise untouched.
// ===========================================================================

struct TakeIds {
    group: TakeGroupId,
    comp: CompId,
    /// Region initially `Silence` — the candidate edit retargets it.
    region_silence: CompRegionId,
    take1: TakeId,
}

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

fn apply_comp(c: &mut ProjectController, op: CompOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

/// Trimmed `cb02_audio_take.rs::take_fixture`: one lane, two verified
/// takes on a `Finalized` session (adopt-eligible), one take group
/// whose comp has a `Take` region and a `Silence` region. Manual
/// seeding because the recording registry has no `ProjectOp` path.
#[allow(clippy::too_many_lines)] // sequential fixture setup reads top-to-bottom (cb02 convention)
fn take_fixture() -> (ProjectController, TakeIds) {
    let mut p = Project::new(ProjectId::new(0), "cb11-take");
    let lane = LaneId::new(2);
    let source = SourceId::new(5);
    let take0 = TakeId::new(15);
    let take1 = TakeId::new(16);
    let track_id = TrackId::new(1);
    let asset_id = AssetId::new(4);

    p.tracks.insert(
        track_id,
        Track {
            id: track_id,
            name: "t".into(),
            lane_ids: vec![lane],
        },
    );
    p.lanes.insert(
        lane,
        Lane {
            id: lane,
            track_id,
            clip_ids: Vec::new(),
        },
    );
    p.assets.insert(
        asset_id,
        Asset {
            id: asset_id,
            digest: "aa".into(),
        },
    );
    p.sources.insert(
        source,
        Source {
            id: source,
            asset_id,
            content_digest: "aa".into(),
            state: SourceState::Committed,
        },
    );
    p.clock_epochs.insert(
        ClockEpochId::new(18),
        ClockEpoch {
            id: ClockEpochId::new(18),
            description: "mock".into(),
        },
    );

    let session_id = RecordingSessionId::new(6);
    let epoch = ClockEpochId::new(18);
    p.recording.sessions.insert(
        session_id,
        RecordingSession {
            id: session_id,
            name: "s".into(),
            state: SessionState::Finalized,
        },
    );
    for (i, id) in [RecordingPassId::new(7), RecordingPassId::new(8)]
        .into_iter()
        .enumerate()
    {
        p.recording.passes.insert(
            id,
            RecordingPass {
                id,
                session_id,
                index: u32::try_from(i).unwrap(),
            },
        );
    }
    p.recording.streams.insert(
        CaptureStreamId::new(10),
        CaptureStream {
            id: CaptureStreamId::new(10),
            session_id,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    for (id, pass, s_lo, s_hi) in [
        (
            CaptureSegmentId::new(12),
            RecordingPassId::new(7),
            0_i64,
            30_000_i64,
        ),
        (
            CaptureSegmentId::new(13),
            RecordingPassId::new(8),
            40_000,
            70_000,
        ),
    ] {
        p.recording.segments.insert(
            id,
            CaptureSegment {
                id,
                stream_id: CaptureStreamId::new(10),
                pass_id: pass,
                session_range: SessionRange::try_new(
                    SessionFrame::new(s_lo).unwrap(),
                    SessionFrame::new(s_hi).unwrap(),
                )
                .unwrap(),
                source_range: SourceRange::try_new(
                    SourceFrame::new(s_lo).unwrap(),
                    SourceFrame::new(s_hi).unwrap(),
                )
                .unwrap(),
                source_id: source,
            },
        );
    }
    for (id, pass, segs) in [
        (
            take0,
            RecordingPassId::new(7),
            vec![CaptureSegmentId::new(12)],
        ),
        (
            take1,
            RecordingPassId::new(8),
            vec![CaptureSegmentId::new(13)],
        ),
    ] {
        p.recording.takes.insert(
            id,
            Take {
                id,
                session_id,
                pass_id: pass,
                stream_id: CaptureStreamId::new(10),
                lane_id: lane,
                source_id: source,
                segment_ids: segs,
                state: TakeState::Verified,
            },
        );
    }

    let mut c = ProjectController::new(p, Box::new(CounterIdGenerator::new(1_000)));
    let mut ids = TakeIds {
        group: TakeGroupId::new(0),
        comp: CompId::new(0),
        region_silence: CompRegionId::new(0),
        take1,
    };

    apply_comp(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: lane,
            target_range: mr(0, 2_000),
            take_ids: vec![take0, take1],
            source_rate: 48_000,
        },
    );
    ids.group = *c.live().comp.take_groups.keys().next().unwrap();
    apply_comp(
        &mut c,
        CompOp::CreateComp {
            take_group_id: ids.group,
        },
    );
    ids.comp = *c.live().comp.comps.keys().next().unwrap();
    for (range, candidate) in [
        (
            mr(0, 960),
            CompCandidateRef::Take {
                take_id: take0,
                source_offset: SourceFrame::ZERO,
            },
        ),
        (mr(960, 2_000), CompCandidateRef::Silence),
    ] {
        apply_comp(
            &mut c,
            CompOp::AddCompRegion {
                comp_id: ids.comp,
                project_range: range,
                candidate,
                mapping: PlaybackMapping::Natural,
            },
        );
    }
    let regions = c.live().comp.comps[&ids.comp].region_ids.clone();
    ids.region_silence = regions[1];
    apply_comp(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: ids.group,
            comp_id: Some(ids.comp),
        },
    );
    (c, ids)
}

#[test]
fn cb11_e2e_01_take_comp_adopt_undo_save_reload_raw_immutable() {
    let (c, ids) = take_fixture();
    let scope = VariantScope::TakeUsage {
        target: TakeUsageTarget {
            take_group: ids.group,
        },
    };
    let mut g = Gate::new(c);
    let raw_before = raw_snapshot(&g.controller);

    let forked = g.fork(&scope);
    let region =
        g.candidate(forked.variant_set, forked.candidate).comp.comps[&ids.comp].region_ids[1];
    assert_eq!(region, ids.region_silence);

    // Candidate edit: retarget the Silence region to take1 — a
    // `BranchEditOp::Comp` op confined to the scoped take group.
    g.try_edit_op(
        &scope,
        forked.variant_set,
        forked.candidate,
        BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
            region_id: region,
            candidate: CompCandidateRef::Take {
                take_id: ids.take1,
                // take1's captured coverage is source [40000, 70000);
                // source_offset is an absolute source frame (cb02).
                source_offset: SourceFrame::new(40_000).unwrap(),
            },
        }),
    )
    .unwrap();
    assert_eq!(
        raw_snapshot(&g.controller),
        raw_before,
        "comp candidate edit must not touch the recording registry"
    );

    let pre_adopt = g.controller.live().clone();
    let receipt = g.adopt(&scope, forked.variant_set, forked.candidate);
    assert!(receipt.project.committed);
    assert_eq!(receipt.project.undo_steps, 1);

    // Adopted comp region now references take1 in live.
    let live_region = &g.controller.live().comp.regions[&ids.region_silence];
    assert!(
        matches!(
            &live_region.candidate,
            CompCandidateRef::Take { take_id, .. } if *take_id == ids.take1
        ),
        "adopted region must carry the candidate's take reference"
    );
    assert_eq!(
        raw_snapshot(&g.controller),
        raw_before,
        "adopt must not touch takes/sources/segments"
    );

    // Undo restores pre-adopt exactly; redo restores post-adopt.
    let post_adopt = g.controller.live().clone();
    assert!(g.controller.undo().unwrap());
    assert_eq!(*g.controller.live(), pre_adopt);
    assert!(g.controller.redo().unwrap());
    assert_eq!(*g.controller.live(), post_adopt);

    // Save/reload round-trips comp + variant bookkeeping verbatim.
    let mut store = InMemoryStore::new();
    g.controller.save(&mut store).unwrap();
    let mut reloaded = ProjectController::new(
        Project::new(ProjectId::new(1), "cb11-take"),
        Box::new(CounterIdGenerator::new(1)),
    );
    reloaded.load(&store).unwrap();
    assert_eq!(*reloaded.live(), *g.controller.live());
    assert_eq!(raw_snapshot_of(reloaded.live()), raw_before);
}

// ===========================================================================
// CB-X-RCP-01 (runnable half) + CB-E2E-04 (pin level): render pins move
// scope_digest only for the edited scope; a shared-context live edit
// moves shared_context_digest for every branch while scope digests
// and candidate payloads stay fixed.
// ===========================================================================

#[test]
fn cb11_x_rcp_01_render_pin_scope_vs_shared_context() {
    let (c, ids) = gate_fixture();
    let mut g = Gate::new(c);
    let scopes = all_scopes(&ids);

    let mut forks = Vec::new();
    for scope in &scopes {
        let f = g.fork(scope);
        g.edit(scope, f.variant_set, f.candidate, domain_edit(scope, &ids));
        forks.push(f);
    }

    // Per set: candidate pin differs from adopted pin on scope_digest
    // (the edit moved scoped content) but shares shared_context_digest
    // (same live context, same scope boundary). Payload digest is the
    // captured store key for candidates, None for the Live branch.
    for forked in &forks {
        let pin_adopted = g.render_pin(forked.variant_set, forked.adopted);
        let pin_candidate = g.render_pin(forked.variant_set, forked.candidate);
        assert_ne!(
            pin_adopted.scope_digest, pin_candidate.scope_digest,
            "candidate edit must move the composed scope digest"
        );
        assert_eq!(
            pin_adopted.shared_context_digest, pin_candidate.shared_context_digest,
            "branches of one set share the live shared context"
        );
        assert_eq!(pin_adopted.payload_digest, None);
        let synvelis_project::variant::BranchContentRef::Captured { payload_digest, .. } =
            g.content_ref(forked.variant_set, forked.candidate)
        else {
            panic!("candidate must stay Captured before adopt");
        };
        assert_eq!(pin_candidate.payload_digest, Some(payload_digest));
    }

    // Shared-context live edit: strip1's fader target is outside every
    // scope — it must move shared_context_digest for ALL sets' branches
    // and must NOT move any scope digest or candidate payload.
    let payloads_before: Vec<_> = forks
        .iter()
        .map(|f| g.content_ref(f.variant_set, f.candidate))
        .collect();
    let pins_before: Vec<_> = forks
        .iter()
        .map(|f| {
            (
                g.render_pin(f.variant_set, f.adopted),
                g.render_pin(f.variant_set, f.candidate),
            )
        })
        .collect();

    apply_op(
        &mut g.controller,
        ProjectOp::SetParameterBase {
            target: ids.param_fader1,
            value: 0.7,
        },
    );

    for (forked, (pin_a_before, pin_c_before)) in forks.iter().zip(pins_before.iter()) {
        let pin_a = g.render_pin(forked.variant_set, forked.adopted);
        let pin_c = g.render_pin(forked.variant_set, forked.candidate);
        assert_ne!(
            pin_a.shared_context_digest, pin_a_before.shared_context_digest,
            "shared-context edit must stale every branch's comparison context"
        );
        assert_ne!(
            pin_c.shared_context_digest,
            pin_c_before.shared_context_digest
        );
        assert_eq!(
            pin_a.scope_digest, pin_a_before.scope_digest,
            "shared-context edit must not move scope digests"
        );
        assert_eq!(pin_c.scope_digest, pin_c_before.scope_digest);
    }
    for (forked, content) in forks.iter().zip(payloads_before.iter()) {
        assert_eq!(
            &g.content_ref(forked.variant_set, forked.candidate),
            content,
            "candidate payloads are immutable across live shared-context edits"
        );
    }
}

// ===========================================================================
// CB-E2E-04 (diff level) + VCS-02 oracle: the adopt's materialized
// changes are exactly the scoped semantic changes — raw-material
// kinds never appear.
// ===========================================================================

#[test]
fn cb11_e2e_04_semantic_diff_reports_scoped_changes_only() {
    let (c, ids) = gate_fixture();
    let mut g = Gate::new(c);
    let scopes = all_scopes(&ids);

    let mut forks = Vec::new();
    for scope in &scopes {
        let f = g.fork(scope);
        g.edit(scope, f.variant_set, f.candidate, domain_edit(scope, &ids));
        forks.push(f);
    }

    let before = g.snap();
    for (scope, forked) in scopes.iter().zip(forks.iter()) {
        g.adopt(scope, forked.variant_set, forked.candidate);
    }
    let after = g.snap();
    let cs = compare(&before, &after);

    assert_eq!(
        cs.overall,
        ComparisonEquality::Different,
        "five adopted domain edits must diff as Different"
    );

    let has = |kind: ObjectKind, ck: ChangeKind| {
        cs.changes
            .iter()
            .any(|ch| ch.object.kind == kind && ch.kind == ck)
    };
    // Scoped semantic changes materialized by the adopts.
    assert!(
        has(ObjectKind::Clip, ChangeKind::Modified),
        "audio clip mute"
    );
    assert!(
        has(ObjectKind::MidiNote, ChangeKind::Modified),
        "MIDI note velocity"
    );
    assert!(
        has(ObjectKind::ParameterTarget, ChangeKind::Modified),
        "processor param adopts"
    );
    // Raw material and recording registry must never appear in the
    // semantic diff — they are shared context, not scoped content.
    for forbidden in [
        ObjectKind::Asset,
        ObjectKind::MediaSource,
        ObjectKind::Take,
        ObjectKind::RecordingSession,
        ObjectKind::RecordingPass,
        ObjectKind::CaptureStream,
        ObjectKind::CaptureSegment,
        ObjectKind::ClockEpoch,
        ObjectKind::SmfDocument,
    ] {
        assert!(
            !cs.changes.iter().any(|ch| ch.object.kind == forbidden),
            "raw-material kind {forbidden:?} must never appear in scoped diff"
        );
    }
}

// ===========================================================================
// CB-E2E-05 (cross-gate half): a stale-scope adopt rejection commits
// nothing — live is bitwise untouched, the revision does not advance,
// and a sibling domain's candidate stays adoptable.
// ===========================================================================

#[test]
fn cb11_e2e_05_stale_reject_is_atomic_and_domain_isolated() {
    let (c, ids) = gate_fixture();
    let mut g = Gate::new(c);

    let scope_strip = scope_strip(ids.strip2);
    let scope_midi = scope_midi(ids.clip_midi);

    let forked_strip = g.fork(&scope_strip);
    g.edit(
        &scope_strip,
        forked_strip.variant_set,
        forked_strip.candidate,
        BranchEditOp::Project(ProjectOp::SetStripMute {
            strip_id: ids.strip2,
            muted: true,
        }),
    );
    let forked_midi = g.fork(&scope_midi);
    g.edit(
        &scope_midi,
        forked_midi.variant_set,
        forked_midi.candidate,
        BranchEditOp::Project(ProjectOp::SetMidiNoteVelocity {
            source_id: ids.midi_source,
            note_id: ids.midi_note,
            velocity: 64,
        }),
    );

    // Pin the strip adopt intent, then stale it with a live scoped
    // edit (cb06 convention: refresh only the project-revision pin so
    // the scope-digest gate is the one exercised). The live mute must
    // actually change scoped content or the digest stays put.
    let mut intent = g.intent(
        forked_strip.variant_set,
        forked_strip.candidate,
        &scope_strip,
    );
    apply_op(
        &mut g.controller,
        ProjectOp::SetStripMute {
            strip_id: ids.strip2,
            muted: true,
        },
    );
    intent.expected_project_revision = g.controller.live_revision();

    let live_before = g.controller.live().clone();
    let rev_before = g.controller.live_revision();
    let midi_payload_before = g.content_ref(forked_midi.variant_set, forked_midi.candidate);

    let err = g.try_adopt(&scope_strip, &intent).unwrap_err();
    assert!(
        matches!(err, VariantError::StaleScope { .. }),
        "stale scope digest must reject typed, got {err:?}"
    );
    assert_eq!(
        *g.controller.live(),
        live_before,
        "rejected adopt must not partially commit"
    );
    assert_eq!(
        g.controller.live_revision(),
        rev_before,
        "rejected adopt must not advance the project revision"
    );
    assert_eq!(
        g.content_ref(forked_midi.variant_set, forked_midi.candidate),
        midi_payload_before,
        "rejected adopt must not disturb sibling domain candidates"
    );

    // The sibling domain still adopts cleanly afterwards.
    let receipt = g.adopt(&scope_midi, forked_midi.variant_set, forked_midi.candidate);
    assert!(receipt.project.committed);
    assert_eq!(
        g.controller.live().midi_sources[&ids.midi_source].notes[&ids.midi_note].velocity,
        64
    );
}
