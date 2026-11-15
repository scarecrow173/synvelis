//! CB-08 Comparison Orchestrator headless tests (synvelis#285).
//!
//! Pins the orchestrator contract: pinned `BranchRevisionRef`
//! endpoints, fixed `EvaluationContext`, layered Semantic/Signal/
//! Perceptual separation, explicit staleness, and derived-cache
//! behavior — across all seven `VariantScope` kinds. Real signal,
//! PCM, audition, and perceptual acceptance stay outside this slice
//! and are asserted as honest gaps, never as passes.

use std::collections::BTreeSet;

use ruviform_plugin_core::ParamFlags;
use synvelis_project::apply::ApplyApproval;
use synvelis_project::control::{
    AutomationTimeScope, ControlOwner, ParamMapping, ParameterRole, TargetCapability, ValueKind,
};
use synvelis_project::controller::{Command, ProjectController};
use synvelis_project::diff::{ComparisonEquality, ObjectKind};
use synvelis_project::id::CounterIdGenerator;
use synvelis_project::project::Project;
use synvelis_project::routing::{PluginRef, ProcessorBinding};
use synvelis_project::snapshot::{BlobProbe, BlobRead, SnapshotCancel, SnapshotResolver};
use synvelis_project::variant::{
    AlignmentPolicy, ArrangementAdapter, ArrangementDomainMask, AudioClipAdapter, BranchEditOp,
    BranchRevisionRef, CacheStatus, CompareError, CompareSide, ComparisonFreshness,
    ComparisonOrchestrator, ComparisonSession, DomainAdapter, EvidenceOrigin, EvidenceReason,
    InMemoryBranchContentStore, InstrumentSlotAdapter, LayerState, MidiClipAdapter,
    ProcessingRangeAdapter, ProcessingRangeRef, SessionOptions, SignalAlignment,
    SignalComparisonProvider, SignalDomain, SignalMetric, SignalProviderReport, StaleDimension,
    StripDomainMask, StripField, StripFieldsAdapter, TakeUsageAdapter, TakeUsageTarget,
    VariantScope, adopt_branch_adapted, edit_branch_op, fork_variant_set, scope_digest,
};
use synvelis_project::{
    Asset, AssetId, BranchId, CaptureSegment, CaptureSegmentId, CaptureStream, CaptureStreamId,
    ChainId, ClipExtent, ClipId, ClipPlacement, ClipPlayback, ClockEpoch, ClockEpochId,
    CompCandidateRef, CompId, CompOp, CompRegionId, Lane, LaneId, MidiDivision, MidiTick,
    MusicalDuration, MusicalRange, MusicalTick, NoteId, PlaybackMapping, ProjectId, ProjectOp,
    ProjectRange, RecordingPass, RecordingPassId, RecordingSession, RecordingSessionId,
    SessionFrame, SessionRange, SessionState, Source, SourceFrame, SourceId, SourceRange,
    SourceState, StripId, Take, TakeGroupId, TakeId, TakeState, TimeAnchor, Track, TrackId,
    VariantSetId, is_issuable_raw,
};

// ---------- harness ----------

/// Picks the merged domain adapter for a scope (all seven merged at
/// this baseline — CB-07 arrangement included).
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

struct Cmp {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    orchestrator: ComparisonOrchestrator,
}

impl Cmp {
    fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            orchestrator: ComparisonOrchestrator::new(16),
        }
    }

    fn fork(&mut self, scope: &VariantScope) -> synvelis_project::variant::ForkReceipt {
        let adapter = adapter_for(scope);
        fork_variant_set(
            &mut self.controller,
            adapter.as_ref(),
            &mut self.store,
            &SnapshotResolver::new(),
            scope.clone(),
            &SnapshotCancel::new(),
        )
        .unwrap()
    }

    fn edit(
        &mut self,
        scope: &VariantScope,
        set: VariantSetId,
        branch: BranchId,
        op: BranchEditOp,
    ) {
        let expected = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .revision();
        let adapter = adapter_for(scope);
        edit_branch_op(
            &mut self.controller,
            adapter.as_ref(),
            &mut self.store,
            &synvelis_project::variant::BranchEditOpIntent {
                variant_set: set,
                branch,
                expected_branch_revision: expected,
                op,
            },
        )
        .unwrap();
    }

    fn live_edit(&mut self, op: ProjectOp) {
        let rev = self.controller.live_revision();
        self.controller
            .apply(Command {
                expected_revision: rev,
                op,
            })
            .unwrap();
    }

    fn pin(&self, set: VariantSetId, branch: BranchId) -> BranchRevisionRef {
        self.orchestrator
            .pin_branch(self.controller.live(), &self.store, set, branch)
            .unwrap()
    }

    fn open(
        &self,
        left: BranchRevisionRef,
        right: BranchRevisionRef,
        options: &SessionOptions,
    ) -> ComparisonSession {
        self.orchestrator
            .open_session(
                self.controller.live(),
                &self.store,
                left,
                right,
                self.controller.live_revision(),
                options,
                Some(&AllPresentProbe),
                &SnapshotCancel::new(),
            )
            .unwrap()
    }

    fn evaluate(
        &mut self,
        session: &ComparisonSession,
        signal: Option<&dyn SignalComparisonProvider>,
    ) -> synvelis_project::variant::ComparisonResult {
        self.orchestrator
            .evaluate(
                session,
                self.controller.live(),
                &self.store,
                self.controller.live_revision(),
                signal,
                &SnapshotCancel::new(),
            )
            .unwrap()
    }

    /// Pin adopted+candidate and open a session in one step.
    fn session_for(
        &mut self,
        receipt: &synvelis_project::variant::ForkReceipt,
        options: &SessionOptions,
    ) -> ComparisonSession {
        let left = self.pin(receipt.variant_set, receipt.adopted);
        let right = self.pin(receipt.variant_set, receipt.candidate);
        self.open(left, right, options)
    }
}

/// Reports every probed digest as present (vcs02/cb11 convention).
struct AllPresentProbe;

impl BlobProbe for AllPresentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Present(vec![0])
    }
}

// ---------- fixture ops ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb08-compare"),
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

fn plugin_binding(desc: &str) -> ProcessorBinding {
    ProcessorBinding::Resolved(PluginRef {
        descriptor: desc.into(),
        version: "1.0".into(),
        capability_revision: 1,
    })
}

struct GateIds {
    clip_audio: ClipId,
    clip_midi: ClipId,
    midi_source: SourceId,
    midi_note: NoteId,
    inst: synvelis_project::ProcessorId,
    fx1: synvelis_project::ProcessorId,
    fx2: synvelis_project::ProcessorId,
    chain1: ChainId,
    param_inst: synvelis_project::ParameterTargetId,
    param_fx1: synvelis_project::ParameterTargetId,
    param_fader1: synvelis_project::ParameterTargetId,
    strip2: synvelis_project::StripId,
    track_arr: TrackId,
    clip_arr: ClipId,
}

/// Six disjoint merged-domain scopes in one project: `AudioClip` +
/// `MidiClip` on track1's lanes, `InstrumentSlot` + `ProcessingRange` on
/// track1's strip chain, `StripFields` on track2's strip, and
/// `Arrangement` on dedicated track3 (cb11 `gate_fixture` extended with an
/// arrangement lane — `TakeUsage` lives in the separate take fixture).
#[allow(clippy::too_many_lines)] // sequential fixture setup reads top-to-bottom (cb11 convention)
fn gate_fixture() -> (ProjectController, GateIds) {
    let mut c = controller();

    apply_op(&mut c, ProjectOp::AddTrack { name: "t1".into() });
    let track1 = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track1 });
    apply_op(&mut c, ProjectOp::AddLane { track_id: track1 });
    let mut lanes: Vec<LaneId> = c.live().lanes.keys().copied().collect();
    lanes.sort();
    let (lane_audio, lane_midi) = (lanes[0], lanes[1]);

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
    let clip_midi = c
        .live()
        .clips
        .values()
        .find(|clip| clip.lane_id == lane_midi)
        .unwrap()
        .id;

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
        synvelis_project::control::ControlOwner::Processor(inst),
        "cutoff",
        0.5,
    );
    let param_fx1 = add_target(
        &mut c,
        synvelis_project::control::ControlOwner::Processor(fx1),
        "drive",
        0.5,
    );
    let param_fader1 =
        strip_channel_param(c.live(), strip1, synvelis_project::ParameterRole::Fader);

    apply_op(&mut c, ProjectOp::AddTrack { name: "t2".into() });
    let track2 = *c.live().tracks.keys().next_back().unwrap();
    apply_op(&mut c, ProjectOp::AddTrackStrip { track_id: track2 });
    let strip2 = c.live().routing.strip_of_track(track2).unwrap().id;

    // Track3: dedicated arrangement scope — one lane, one in-range clip.
    apply_op(&mut c, ProjectOp::AddTrack { name: "t3".into() });
    let track_arr = *c.live().tracks.keys().next_back().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddLane {
            track_id: track_arr,
        },
    );
    let lane_arr = *c.live().lanes.keys().next_back().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane_arr,
            source_id: source_audio,
            placement: clip_placement(0),
            playback: audio_playback(),
        },
    );
    let clip_arr = *c.live().clips.keys().next_back().unwrap();

    let ids = GateIds {
        clip_audio,
        clip_midi,
        midi_source,
        midi_note,
        inst,
        fx1,
        fx2,
        chain1,
        param_inst,
        param_fx1,
        param_fader1,
        strip2,
        track_arr,
        clip_arr,
    };
    (c, ids)
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
            capability: Some(capability(1)),
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

fn strip_channel_param(
    project: &Project,
    strip: StripId,
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

fn scope_arr(ids: &GateIds) -> VariantScope {
    VariantScope::Arrangement {
        tracks: vec![ids.track_arr],
        time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(20 * 960)).unwrap(),
        fields: ArrangementDomainMask::ALL,
    }
}

fn gate_scopes(ids: &GateIds) -> Vec<VariantScope> {
    vec![
        VariantScope::AudioClip {
            clip: ids.clip_audio,
        },
        VariantScope::MidiClip {
            clip: ids.clip_midi,
        },
        VariantScope::InstrumentSlot {
            processor: ids.inst,
        },
        VariantScope::ProcessingRange {
            range: ProcessingRangeRef {
                chain: ids.chain1,
                start: ids.fx1,
                end: ids.fx2,
            },
        },
        VariantScope::StripFields {
            strip: ids.strip2,
            fields: StripDomainMask(BTreeSet::from([StripField::Fader, StripField::Mute])),
        },
        scope_arr(ids),
    ]
}

fn gate_edit(scope: &VariantScope, ids: &GateIds) -> BranchEditOp {
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
        VariantScope::Arrangement { .. } => BranchEditOp::Project(ProjectOp::SetClipMuted {
            clip_id: ids.clip_arr,
            muted: true,
        }),
        VariantScope::TakeUsage { .. } => unreachable!("take fixture is separate"),
    }
}

// ---------- take/comp fixture (cb11 take_fixture convention) ----------

struct TakeIds {
    group: TakeGroupId,
    comp: CompId,
    region_silence: CompRegionId,
    take1: TakeId,
}

#[allow(clippy::too_many_lines)] // sequential fixture setup reads top-to-bottom (cb11 convention)
fn take_fixture() -> (ProjectController, TakeIds) {
    let mut p = Project::new(ProjectId::new(0), "cb08-take");
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

// ---------- shared assertions ----------

/// Raw-material kinds the semantic layer must never report as
/// scoped changes (`cb11_e2e_04` invariant). Scoped-domain kinds —
/// `TakeGroup`/`Comp`/`CompRegion`/`CompTransition` for `TakeUsage`,
/// `MidiSource`/`MidiNote` for `MidiClip` — are legitimately diffable
/// and deliberately absent from this list.
fn raw_material_kind(kind: ObjectKind) -> bool {
    matches!(
        kind,
        ObjectKind::Asset
            | ObjectKind::MediaSource
            | ObjectKind::Take
            | ObjectKind::RecordingSession
            | ObjectKind::RecordingPass
            | ObjectKind::CaptureStream
            | ObjectKind::CaptureSegment
            | ObjectKind::ClockEpoch
            | ObjectKind::SmfDocument
            | ObjectKind::AppliedCommit
            | ObjectKind::AppliedImport
    )
}

fn assert_layers_honest(result: &synvelis_project::variant::ComparisonResult) {
    assert_eq!(result.freshness, ComparisonFreshness::Current);
    assert!(result.semantic.state.is_available());
    // No provider configured → honest gap, never fabricated signal.
    assert_eq!(
        result.signal.state,
        LayerState::Unavailable {
            reason: EvidenceReason::NoProvider
        }
    );
    assert_eq!(result.signal.origin, None);
    assert!(!result.signal.stale);
    // Perceptual is interactive-only — always an explicit gap.
    assert_eq!(
        result.perceptual.state,
        LayerState::Unavailable {
            reason: EvidenceReason::RequiresAudition
        }
    );
    assert!(!result.perceptual.stale);
}

// ===========================================================================
// CB-CMP-01: all seven scopes pin revisions + fixed context and return a
// semantic diff; unsupported/unavailable evidence stays typed.
// ===========================================================================

#[test]
fn cb08_cmp_01_six_gate_scopes_pin_context_and_semantic_diff() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let options = SessionOptions::default();

    for (index, scope) in gate_scopes(&ids).into_iter().enumerate() {
        let receipt = cmp.fork(&scope);
        let session = cmp.session_for(&receipt, &options);

        // Both endpoints pinned: revisions + content digests recorded.
        assert_eq!(session.left.variant_set, receipt.variant_set);
        assert_eq!(session.right.variant_set, receipt.variant_set);
        assert_eq!(session.left.branch, receipt.adopted);
        assert_eq!(session.right.branch, receipt.candidate);
        let ctx = &session.pinned_context;
        assert_eq!(ctx.project_revision, cmp.controller.live_revision());
        assert_eq!(ctx.left.branch, receipt.adopted);
        assert_eq!(ctx.right.branch, receipt.candidate);
        assert_eq!(ctx.left.variant_set, receipt.variant_set);
        // Shared-context digest is identical for both pins (same live).
        assert_eq!(
            ctx.left.shared_context_digest,
            ctx.right.shared_context_digest
        );

        // Unedited candidate → no confirmed scoped changes; plugin
        // opaque-state issues may keep overall at Unknown (honest).
        let result = cmp.evaluate(&session, None);
        assert_layers_honest(&result);
        assert_eq!(result.semantic.changeset.changes.len(), 0);

        // Scoped candidate edit → semantic Different, scoped kinds only.
        cmp.edit(
            &scope,
            receipt.variant_set,
            receipt.candidate,
            gate_edit(&scope, &ids),
        );
        let session2 = cmp.session_for(&receipt, &options);
        let result2 = cmp.evaluate(&session2, None);
        assert_eq!(
            result2.semantic.changeset.overall,
            ComparisonEquality::Different,
            "scope {index} ({scope:?}) must report scoped semantic changes"
        );
        assert_ne!(result2.semantic.changeset.changes.len(), 0);
        assert!(
            result2
                .semantic
                .changeset
                .changes
                .iter()
                .all(|change| !raw_material_kind(change.object.kind)),
            "scope {index} leaked raw-material kinds into the diff"
        );
        assert!(matches!(result2.freshness, ComparisonFreshness::Current));
        eprintln!("scope {index} ok: {scope:?}");
    }
}

#[test]
fn cb08_cmp_01_take_usage_scope_pins_and_clean_equal() {
    let (c, ids) = take_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::TakeUsage {
        target: TakeUsageTarget {
            take_group: ids.group,
        },
    };
    let receipt = cmp.fork(&scope);
    let session = cmp.session_for(&receipt, &SessionOptions::default());

    // Plugin-free + all-probed project: unedited compare may claim
    // Equal — the only scope kind where clean equality is reachable
    // headlessly.
    let result = cmp.evaluate(&session, None);
    assert_layers_honest(&result);
    assert_eq!(
        result.semantic.changeset.overall,
        ComparisonEquality::Equal,
        "identical pinned views must compare Equal when evidence is complete"
    );

    cmp.edit(
        &scope,
        receipt.variant_set,
        receipt.candidate,
        BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
            region_id: ids.region_silence,
            candidate: CompCandidateRef::Take {
                take_id: ids.take1,
                // take1's captured source coverage starts at frame
                // 40_000 — the offset must land inside its handle.
                source_offset: SourceFrame::new(40_000).unwrap(),
            },
        }),
    );
    let session2 = cmp.session_for(&receipt, &SessionOptions::default());
    let result2 = cmp.evaluate(&session2, None);
    assert_eq!(
        result2.semantic.changeset.overall,
        ComparisonEquality::Different
    );
    assert!(
        result2
            .semantic
            .changeset
            .changes
            .iter()
            .all(|change| !raw_material_kind(change.object.kind))
    );
}

// ===========================================================================
// CB-CMP-02: context changes mark sessions/results stale — old evidence is
// never presented as current.
// ===========================================================================

#[test]
fn cb08_cmp_02_branch_edit_marks_session_and_result_stale() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let receipt = cmp.fork(&scope);
    let session = cmp.session_for(&receipt, &SessionOptions::default());
    let before = cmp.evaluate(&session, None);
    assert_eq!(before.freshness, ComparisonFreshness::Current);

    // Candidate edit after open: branch revision + scope content +
    // payload + project revision all move; shared context does not.
    cmp.edit(
        &scope,
        receipt.variant_set,
        receipt.candidate,
        gate_edit(&scope, &ids),
    );
    let freshness = cmp
        .orchestrator
        .freshness(
            &session,
            cmp.controller.live(),
            &cmp.store,
            cmp.controller.live_revision(),
        )
        .unwrap();
    let ComparisonFreshness::Stale { reasons } = freshness else {
        panic!("edited candidate must stale the session");
    };
    for expected in [
        StaleDimension::BranchRevision {
            side: CompareSide::Right,
            pinned: session.right.revision,
            actual: session.right.revision.next(),
        },
        StaleDimension::ScopeContent {
            side: CompareSide::Right,
        },
        StaleDimension::BranchPayload {
            side: CompareSide::Right,
        },
        StaleDimension::ProjectRevision {
            pinned: session.pinned_context.project_revision,
            actual: cmp.controller.live_revision(),
        },
    ] {
        assert!(
            reasons.contains(&expected),
            "missing stale dimension {expected:?} in {reasons:?}"
        );
    }
    // Scoped candidate edit: no shared-context or tempo drift.
    for unexpected in [
        StaleDimension::SharedContext,
        StaleDimension::Tempo,
        StaleDimension::Plugin,
        StaleDimension::Source,
        StaleDimension::TimeMapping,
    ] {
        assert!(
            !reasons.contains(&unexpected),
            "unexpected stale dimension {unexpected:?} in {reasons:?}"
        );
    }
    // The moved side is only Right — Left pins still match authority.
    assert!(!reasons.iter().any(|r| matches!(
        r,
        StaleDimension::BranchRevision {
            side: CompareSide::Left,
            ..
        } | StaleDimension::ScopeContent {
            side: CompareSide::Left
        } | StaleDimension::BranchPayload {
            side: CompareSide::Left
        }
    )));

    // Re-evaluation keeps producing pin-scoped semantic evidence but
    // flags the result stale — old measurements are never "current".
    let after = cmp.evaluate(&session, None);
    assert!(after.freshness.is_stale());
    assert!(after.semantic.state.is_available());
    assert!(after.signal.stale);
    assert!(after.perceptual.stale);
}

#[test]
fn cb08_cmp_02_shared_context_edit_marks_stale_without_scope_move() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let receipt = cmp.fork(&scope);
    let session = cmp.session_for(&receipt, &SessionOptions::default());

    // Strip1's fader target is shared context for every scope — a
    // live edit there moves shared_context_digest but no scope digest.
    cmp.live_edit(ProjectOp::SetParameterBase {
        target: ids.param_fader1,
        value: 0.1,
    });
    let freshness = cmp
        .orchestrator
        .freshness(
            &session,
            cmp.controller.live(),
            &cmp.store,
            cmp.controller.live_revision(),
        )
        .unwrap();
    let ComparisonFreshness::Stale { reasons } = freshness else {
        panic!("shared-context edit must stale the session");
    };
    assert!(reasons.contains(&StaleDimension::SharedContext));
    assert!(reasons.contains(&StaleDimension::ProjectRevision {
        pinned: session.pinned_context.project_revision,
        actual: cmp.controller.live_revision(),
    }));
    assert!(!reasons.iter().any(|r| matches!(
        r,
        StaleDimension::BranchRevision { .. }
            | StaleDimension::ScopeContent { .. }
            | StaleDimension::BranchPayload { .. }
            | StaleDimension::Tempo
            | StaleDimension::Plugin
            | StaleDimension::Source
            | StaleDimension::TimeMapping
    )));
}

#[test]
fn cb08_cmp_02_tempo_and_source_moves_classify_independently() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let receipt = cmp.fork(&scope);
    let session = cmp.session_for(&receipt, &SessionOptions::default());

    // Tempo-map move → Tempo dimension only (+ project revision).
    cmp.live_edit(ProjectOp::AddTempoEvent {
        tick: 960,
        bpm_milli: 140_000,
    });
    let freshness = cmp
        .orchestrator
        .freshness(
            &session,
            cmp.controller.live(),
            &cmp.store,
            cmp.controller.live_revision(),
        )
        .unwrap();
    let ComparisonFreshness::Stale { reasons } = freshness else {
        panic!("tempo edit must stale the session");
    };
    assert!(reasons.contains(&StaleDimension::Tempo));
    assert!(!reasons.contains(&StaleDimension::SharedContext));

    // Source-material move → Source dimension.
    cmp.live_edit(ProjectOp::RegisterAsset { digest: sha('z') });
    let freshness = cmp
        .orchestrator
        .freshness(
            &session,
            cmp.controller.live(),
            &cmp.store,
            cmp.controller.live_revision(),
        )
        .unwrap();
    let ComparisonFreshness::Stale { reasons } = freshness else {
        panic!("source edit must stale the session");
    };
    assert!(reasons.contains(&StaleDimension::Source));
}

#[test]
fn cb08_cmp_02_plugin_insert_marks_plugin_dimension_stale() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let receipt = cmp.fork(&scope);
    let session = cmp.session_for(&receipt, &SessionOptions::default());

    // A new processor on the shared strip1 chain adds PluginInstance +
    // Processor shared-context objects → Plugin + SharedContext stale.
    let strip1 = cmp
        .controller
        .live()
        .routing
        .strip_of_track(
            cmp.controller
                .live()
                .tracks
                .values()
                .find(|t| t.name == "t1")
                .unwrap()
                .id,
        )
        .unwrap()
        .id;
    cmp.live_edit(ProjectOp::InsertProcessor {
        strip_id: strip1,
        index: 3,
        name: "extra".into(),
        binding: plugin_binding("plug-x"),
        sidechain_input: false,
    });
    let freshness = cmp
        .orchestrator
        .freshness(
            &session,
            cmp.controller.live(),
            &cmp.store,
            cmp.controller.live_revision(),
        )
        .unwrap();
    let ComparisonFreshness::Stale { reasons } = freshness else {
        panic!("plugin insert must stale the session");
    };
    assert!(reasons.contains(&StaleDimension::Plugin));
    assert!(reasons.contains(&StaleDimension::SharedContext));
}

// ===========================================================================
// Cache: derived reuse is keyed by pinned content; hits are observable and
// any context move misses — never a false reuse.
// ===========================================================================

#[test]
fn cb08_cmp_cache_hit_reuse_and_content_invalidation() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let receipt = cmp.fork(&scope);
    cmp.edit(
        &scope,
        receipt.variant_set,
        receipt.candidate,
        gate_edit(&scope, &ids),
    );
    let session = cmp.session_for(&receipt, &SessionOptions::default());

    let first = cmp.evaluate(&session, None);
    assert_eq!(first.semantic.cache, CacheStatus::Miss);
    let second = cmp.evaluate(&session, None);
    assert_eq!(second.semantic.cache, CacheStatus::Hit);
    assert_eq!(first.semantic.key, second.semantic.key);

    // A second session over the same pins reuses the same derived
    // result — cross-session cache hit on identical acoustic content.
    let session_b = cmp.session_for(&receipt, &SessionOptions::default());
    let third = cmp.evaluate(&session_b, None);
    assert_eq!(third.semantic.cache, CacheStatus::Hit);
    assert_eq!(first.semantic.key, third.semantic.key);

    // Any content move changes the composed-view digests → miss, never
    // stale-as-current reuse.
    cmp.edit(
        &scope,
        receipt.variant_set,
        receipt.candidate,
        BranchEditOp::Project(ProjectOp::SetClipMuted {
            clip_id: ids.clip_audio,
            muted: false,
        }),
    );
    let session_c = cmp.session_for(&receipt, &SessionOptions::default());
    let fourth = cmp.evaluate(&session_c, None);
    assert_eq!(fourth.semantic.cache, CacheStatus::Miss);
    assert_ne!(first.semantic.key, fourth.semantic.key);
}

// ===========================================================================
// Open-time CAS: asserted pins are verified fail-closed.
// ===========================================================================

#[test]
fn cb08_cmp_open_rejects_stale_revision_and_scope_digest() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let receipt = cmp.fork(&scope);
    let left = cmp.pin(receipt.variant_set, receipt.adopted);
    let right = cmp.pin(receipt.variant_set, receipt.candidate);

    // Candidate moves after pinning → open must fail typed, not compare.
    cmp.edit(
        &scope,
        receipt.variant_set,
        receipt.candidate,
        gate_edit(&scope, &ids),
    );
    let err = cmp
        .orchestrator
        .open_session(
            cmp.controller.live(),
            &cmp.store,
            left.clone(),
            right.clone(),
            cmp.controller.live_revision(),
            &SessionOptions::default(),
            Some(&AllPresentProbe),
            &SnapshotCancel::new(),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        CompareError::StaleRevision {
            side: CompareSide::Right,
            ..
        }
    ));

    // Wrong asserted content digest → StaleScope.
    let mut bad = cmp.pin(receipt.variant_set, receipt.candidate);
    bad.content_digest = synvelis_project::snapshot::ContentDigest::sha256(b"forged");
    let left_now = cmp.pin(receipt.variant_set, receipt.adopted);
    let err = cmp
        .orchestrator
        .open_session(
            cmp.controller.live(),
            &cmp.store,
            left_now,
            bad,
            cmp.controller.live_revision(),
            &SessionOptions::default(),
            Some(&AllPresentProbe),
            &SnapshotCancel::new(),
        )
        .unwrap_err();
    assert!(matches!(err, CompareError::StaleScope { .. }));
}

#[test]
fn cb08_cmp_open_rejects_unknown_and_cross_set_endpoints() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope_a = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let scope_b = VariantScope::MidiClip {
        clip: ids.clip_midi,
    };
    let ra = cmp.fork(&scope_a);
    let rb = cmp.fork(&scope_b);

    // Unknown branch.
    let mut bogus = cmp.pin(ra.variant_set, ra.adopted);
    bogus.branch = BranchId::new(999_999);
    let err = cmp
        .orchestrator
        .open_session(
            cmp.controller.live(),
            &cmp.store,
            bogus,
            cmp.pin(ra.variant_set, ra.candidate),
            cmp.controller.live_revision(),
            &SessionOptions::default(),
            None,
            &SnapshotCancel::new(),
        )
        .unwrap_err();
    assert!(matches!(err, CompareError::UnknownBranch { .. }));

    // Cross-set endpoints → ScopeMismatch (different scopes cannot
    // share a comparison).
    let err = cmp
        .orchestrator
        .open_session(
            cmp.controller.live(),
            &cmp.store,
            cmp.pin(ra.variant_set, ra.adopted),
            cmp.pin(rb.variant_set, rb.candidate),
            cmp.controller.live_revision(),
            &SessionOptions::default(),
            None,
            &SnapshotCancel::new(),
        )
        .unwrap_err();
    assert!(matches!(err, CompareError::ScopeMismatch { .. }));
}

#[test]
fn cb08_cmp_open_is_read_only_and_session_is_derived() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let receipt = cmp.fork(&scope);
    let revision_before = cmp.controller.live_revision();
    let project_before = cmp.controller.live().clone();

    let session = cmp.session_for(&receipt, &SessionOptions::default());
    let _ = cmp.evaluate(&session, None);

    // Compare never mutates: revision, live project, undo depth, and
    // branch contents are all untouched.
    assert_eq!(cmp.controller.live_revision(), revision_before);
    assert_eq!(cmp.controller.live(), &project_before);
    assert_eq!(
        cmp.controller.live().variant_sets[&receipt.variant_set].branches[&receipt.candidate]
            .content
            .revision(),
        session.right.revision
    );
}

// ===========================================================================
// Signal provider seam: honest gaps, plugin-state gate, validation.
// ===========================================================================

/// Mock provider returning one finite metric row — visibly `Mock`
/// origin, never real evidence.
struct MockSignal;

impl SignalComparisonProvider for MockSignal {
    fn compare_signal(
        &self,
        _request: &synvelis_project::variant::SignalCompareRequest<'_>,
    ) -> SignalProviderReport {
        SignalProviderReport {
            state: LayerState::Available,
            origin: EvidenceOrigin::Mock,
            alignment: SignalAlignment::Aligned,
            metrics: vec![SignalMetric {
                name: "pcm.rms".into(),
                domain: SignalDomain::SourceFrame,
                before: 0.5,
                after: 0.4,
                delta: -0.1,
                unit: "linear".into(),
            }],
            provenance: None,
        }
    }
}

struct NonFiniteSignal;

impl SignalComparisonProvider for NonFiniteSignal {
    fn compare_signal(
        &self,
        _request: &synvelis_project::variant::SignalCompareRequest<'_>,
    ) -> SignalProviderReport {
        SignalProviderReport {
            state: LayerState::Available,
            origin: EvidenceOrigin::Mock,
            alignment: SignalAlignment::Aligned,
            metrics: vec![SignalMetric {
                name: "pcm.peak".into(),
                domain: SignalDomain::ProjectFrame,
                before: f64::NAN,
                after: 0.0,
                delta: 0.0,
                unit: "linear".into(),
            }],
            provenance: None,
        }
    }
}

#[test]
fn cb08_cmp_signal_provider_gates_plugin_state_and_quality() {
    // Plugin-free take project: mock provider evidence flows through
    // with its Mock origin visibly attached.
    let (c, ids) = take_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::TakeUsage {
        target: TakeUsageTarget {
            take_group: ids.group,
        },
    };
    let receipt = cmp.fork(&scope);
    let session = cmp.session_for(&receipt, &SessionOptions::default());
    let result = cmp.evaluate(&session, Some(&MockSignal));
    assert_eq!(result.signal.state, LayerState::Available);
    assert_eq!(result.signal.origin, Some(EvidenceOrigin::Mock));
    assert_eq!(result.signal.alignment, SignalAlignment::Aligned);
    assert_eq!(result.signal.metrics.len(), 1);
    assert!(!result.signal.stale);

    // Gate fixture carries processors → opaque plugin state is
    // Unknown headlessly → provider evidence is downgraded, never
    // presented as measured-exact.
    let (c2, ids2) = gate_fixture();
    let mut cmp2 = Cmp::new(c2);
    let scope2 = VariantScope::AudioClip {
        clip: ids2.clip_audio,
    };
    let receipt2 = cmp2.fork(&scope2);
    let session2 = cmp2.session_for(&receipt2, &SessionOptions::default());
    let result2 = cmp2.evaluate(&session2, Some(&MockSignal));
    assert_eq!(
        result2.signal.state,
        LayerState::Unavailable {
            reason: EvidenceReason::PluginStateUnknown
        }
    );

    // Non-finite provider metrics fail quality validation.
    let result3 = cmp.evaluate(&session, Some(&NonFiniteSignal));
    assert_eq!(
        result3.signal.state,
        LayerState::Unknown {
            reason: EvidenceReason::EvidenceQuality
        }
    );
}

#[test]
fn cb08_cmp_signal_unaligned_policy_and_stale_flag() {
    let (c, ids) = take_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::TakeUsage {
        target: TakeUsageTarget {
            take_group: ids.group,
        },
    };
    let receipt = cmp.fork(&scope);

    // Declared Unaligned policy refuses signal evidence outright.
    let options = SessionOptions {
        alignment: AlignmentPolicy::Unaligned,
        ..SessionOptions::default()
    };
    let session = cmp.session_for(&receipt, &options);
    let result = cmp.evaluate(&session, Some(&MockSignal));
    assert_eq!(
        result.signal.state,
        LayerState::Unavailable {
            reason: EvidenceReason::AlignmentPolicy
        }
    );

    // Stale context flags aux evidence without erasing the semantic
    // layer's pin-scoped result.
    let options2 = SessionOptions::default();
    let session2 = cmp.session_for(&receipt, &options2);
    cmp.edit(
        &scope,
        receipt.variant_set,
        receipt.candidate,
        BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
            region_id: ids.region_silence,
            candidate: CompCandidateRef::Take {
                take_id: ids.take1,
                source_offset: SourceFrame::new(40_000).unwrap(),
            },
        }),
    );
    let result2 = cmp.evaluate(&session2, Some(&MockSignal));
    assert!(result2.freshness.is_stale());
    assert!(result2.signal.stale);
    assert!(result2.perceptual.stale);
    assert!(result2.semantic.state.is_available());
}

// ===========================================================================
// Adopt/runtime separation: compare is read-only derived state; adopt is a
// separate CAS path this slice does not conflate.
// ===========================================================================

#[test]
fn cb08_cmp_adopted_branch_compares_against_candidate() {
    let (c, ids) = gate_fixture();
    let mut cmp = Cmp::new(c);
    let scope = VariantScope::AudioClip {
        clip: ids.clip_audio,
    };
    let receipt = cmp.fork(&scope);
    cmp.edit(
        &scope,
        receipt.variant_set,
        receipt.candidate,
        gate_edit(&scope, &ids),
    );

    // Adopt the candidate through the existing CAS path, then compare
    // the (now Live) adopted branch against itself — proving the
    // orchestrator handles Live content refs, not only Captured ones.
    let live_before = cmp.controller.live_revision();
    let adapter = adapter_for(&scope);
    let runtime = synvelis_project::variant::NoopRuntimePublish;
    let set_state = &cmp.controller.live().variant_sets[&receipt.variant_set];
    let intent = synvelis_project::variant::AdoptIntent {
        variant_set: receipt.variant_set,
        branch: receipt.candidate,
        expected_project_revision: live_before,
        expected_branch_revision: set_state.branches[&receipt.candidate].content.revision(),
        expected_scope_digest: scope_digest(cmp.controller.live(), &scope).unwrap(),
        approval: ApplyApproval {
            ticket: "approve".into(),
        },
    };
    adopt_branch_adapted(
        &mut cmp.controller,
        adapter.as_ref(),
        &mut cmp.store,
        &synvelis_project::variant::AdoptSources {
            assets: None,
            plugins: None,
            runtime: &runtime,
        },
        &intent,
    )
    .unwrap();

    // Post-adopt: the adopted endpoint resolves through the Live
    // content-ref path. Comparing a pin against itself yields empty
    // changes and current freshness.
    let adopted_ref = cmp.pin(receipt.variant_set, receipt.adopted);
    let session = cmp.open(adopted_ref.clone(), adopted_ref, &SessionOptions::default());
    let result = cmp.evaluate(&session, None);
    assert_eq!(result.semantic.changeset.changes.len(), 0);
    assert_eq!(result.freshness, ComparisonFreshness::Current);
}
