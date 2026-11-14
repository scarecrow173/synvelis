//! CB-02 Creative Branch audio-clip / take-usage adapter fixtures
//! (synvelis#279).
//!
//! Covers the real domain adapters headlessly: `AudioClipAdapter`
//! (clip-local trim/placement/mute/notes with honest capability
//! reporting) and `TakeUsageAdapter` (comping `CompOp` edits confined
//! to the scoped take group, adopt materialization of comping
//! entities, recording/missing-material fail-closed gates). Raw
//! `Source`/`Asset`/`Take`/recording-session data stays shared and
//! bitwise immutable throughout. Real OS / plugin / RT / render lanes
//! are `NotRun` — recorded in `evidence/sv279/`.

use synvelis_project::snapshot::{SnapshotCancel, SnapshotResolver};
use synvelis_project::store::InMemoryStore;
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, AudioClipAdapter, AudioClipCapabilities, BranchEditOp,
    BranchEditOpIntent, DomainAdapter, InMemoryBranchContentStore, NoopRuntimePublish,
    ProcessingCapability, TakeUsageAdapter, TakeUsageTarget, VariantError, VariantScope,
    adopt_branch_adapted, decode_candidate, discard_branch, edit_branch_op, fork_variant_set,
    scope_digest,
};
use synvelis_project::{
    Asset, AssetId, BeatPoint, BranchId, CaptureSegment, CaptureSegmentId, CaptureStream,
    CaptureStreamId, ClipExtent, ClipId, ClipPlacement, ClipPlayback, ClockEpoch, ClockEpochId,
    Command, CompCandidateRef, CompId, CompOp, CompRegionId, CompTransitionId, CounterIdGenerator,
    Error, FadeShape, Lane, LaneId, MusicalDuration, MusicalRange, MusicalTick, PlaybackMapping,
    Project, ProjectController, ProjectId, ProjectOp, ProjectRange, RecordingPass, RecordingPassId,
    RecordingSession, RecordingSessionId, RenderBranchTarget, SessionFrame, SessionRange,
    SessionState, Source, SourceFrame, SourceId, SourceRange, SourceState, StretchRatio, Take,
    TakeGroupId, TakeId, TakeState, TimeAnchor, Track, TrackId, VariantSetId,
    resolve_render_project,
};

const RATE: u32 = 48_000;

// ── shared harness ─────────────────────────────────────────────────

struct Fx {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    adapter: Box<dyn DomainAdapter>,
    resolver: SnapshotResolver,
}

impl Fx {
    fn clip(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            adapter: Box::new(AudioClipAdapter::new()),
            resolver: SnapshotResolver::new(),
        }
    }

    fn take(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            adapter: Box::new(TakeUsageAdapter::new()),
            resolver: SnapshotResolver::new(),
        }
    }

    fn fork(&mut self, scope: VariantScope) -> synvelis_project::variant::ForkReceipt {
        fork_variant_set(
            &mut self.controller,
            self.adapter.as_ref(),
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
        expected: synvelis_project::variant::BranchRevision,
        op: BranchEditOp,
    ) -> Result<synvelis_project::variant::EditReceipt, VariantError> {
        edit_branch_op(
            &mut self.controller,
            self.adapter.as_ref(),
            &mut self.store,
            &BranchEditOpIntent {
                variant_set: set,
                branch,
                expected_branch_revision: expected,
                op,
            },
        )
    }

    fn edit_op(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        expected: synvelis_project::variant::BranchRevision,
        op: BranchEditOp,
    ) -> synvelis_project::variant::EditReceipt {
        self.try_edit_op(set, branch, expected, op).unwrap()
    }

    fn try_edit(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        expected: synvelis_project::variant::BranchRevision,
        op: ProjectOp,
    ) -> Result<synvelis_project::variant::EditReceipt, VariantError> {
        self.try_edit_op(set, branch, expected, BranchEditOp::Project(op))
    }

    fn edit(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        expected: synvelis_project::variant::BranchRevision,
        op: ProjectOp,
    ) -> synvelis_project::variant::EditReceipt {
        self.try_edit(set, branch, expected, op).unwrap()
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

    fn try_adopt(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
    ) -> Result<synvelis_project::variant::AdoptReceipt, VariantError> {
        let intent = self.intent(set, branch, scope);
        let runtime = NoopRuntimePublish;
        adopt_branch_adapted(
            &mut self.controller,
            self.adapter.as_ref(),
            &mut self.store,
            &AdoptSources {
                assets: None,
                plugins: None,
                runtime: &runtime,
            },
            &intent,
        )
    }

    fn adopt(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
    ) -> synvelis_project::variant::AdoptReceipt {
        self.try_adopt(set, branch, scope).unwrap()
    }

    fn candidate(&self, set: VariantSetId, branch: BranchId) -> Project {
        let content = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .clone();
        decode_candidate(&self.store, &content).unwrap()
    }

    /// Canonical render-reuse pin for a branch — the RCP-01 read
    /// adapter path CB-AUDIO-RCP-01 exercises.
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

    /// The effective project a render resolves for a branch.
    fn render_project(&self, set: VariantSetId, branch: BranchId) -> Project {
        resolve_render_project(
            self.controller.live(),
            Some(RenderBranchTarget {
                variant_set: set,
                branch,
                store: &self.store,
            }),
        )
        .unwrap()
        .project
    }
}

// ── clip fixture: two clips sharing one raw asset/source ───────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb02-fixture"),
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

fn clip_placement(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
    }
}

fn clip_playback() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

struct ClipFixture {
    clip_a: ClipId,
    clip_b: ClipId,
    source: SourceId,
    asset: AssetId,
}

/// One track, two lanes, ONE asset+source shared by two audio clips.
fn clip_fixture() -> (ProjectController, ClipFixture) {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let mut lanes: Vec<LaneId> = c.live().lanes.keys().copied().collect();
    lanes.sort();
    let (lane_a, lane_b) = (lanes[0], lanes[1]);
    apply_op(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: "raw-blob-sha-01".into(),
        },
    );
    let asset = *c.live().assets.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "src-meta-digest-01".into(),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    for (lane, tick) in [(lane_a, 0_i64), (lane_b, 40_000_i64)] {
        apply_op(
            &mut c,
            ProjectOp::AddClip {
                lane_id: lane,
                source_id: source,
                placement: clip_placement(tick),
                playback: clip_playback(),
            },
        );
    }
    let clip_of = |lane: LaneId| {
        c.live()
            .clips
            .values()
            .find(|clip| clip.lane_id == lane)
            .unwrap()
            .id
    };
    let fx = ClipFixture {
        clip_a: clip_of(lane_a),
        clip_b: clip_of(lane_b),
        source,
        asset,
    };
    (c, fx)
}

fn audio_scope(clip: ClipId) -> VariantScope {
    VariantScope::AudioClip { clip }
}

fn raw_snapshot(c: &ProjectController) -> serde_json::Value {
    serde_json::json!({
        "assets": c.live().assets,
        "sources": c.live().sources,
        "recording": c.live().recording,
    })
}

// ── take/comp fixture (rcd_04_comp_cas.rs seeding conventions) ──────

struct TakeFixture {
    lane_a: LaneId,
    lane_b: LaneId,
    take_a0: TakeId,
    take_a1: TakeId,
    take_b: TakeId,
    source: SourceId,
    group: TakeGroupId,
    comp: CompId,
    comp_b: CompId,
    group_b: TakeGroupId,
    regions: Vec<CompRegionId>,
    transition: CompTransitionId,
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

/// Seeded like `rcd_04_comp_cas.rs`: two lanes, one shared source,
/// three verified takes (`take_a0`/`take_a1` on `lane_a`, `take_b` on
/// `lane_b`), then two take groups — the scoped one on `lane_a` with a
/// 3-region comp + transition, an unrelated one on `lane_b`.
#[allow(clippy::too_many_lines)] // sequential fixture setup reads top-to-bottom
fn take_fixture(session_state: SessionState) -> (ProjectController, TakeFixture) {
    let mut p = Project::new(ProjectId::new(0), "cb02-take");
    let fx = TakeFixture {
        lane_a: LaneId::new(2),
        lane_b: LaneId::new(3),
        take_a0: TakeId::new(15),
        take_a1: TakeId::new(16),
        take_b: TakeId::new(17),
        source: SourceId::new(5),
        group: TakeGroupId::new(0), // placeholder, filled below
        comp: CompId::new(0),
        comp_b: CompId::new(0),
        group_b: TakeGroupId::new(0),
        regions: Vec::new(),
        transition: CompTransitionId::new(0),
    };

    // structure
    let track_id = TrackId::new(1);
    let asset_id = AssetId::new(4);
    p.tracks.insert(
        track_id,
        Track {
            id: track_id,
            name: "t".into(),
            lane_ids: vec![fx.lane_a, fx.lane_b],
        },
    );
    for (id, track) in [(fx.lane_a, track_id), (fx.lane_b, track_id)] {
        p.lanes.insert(
            id,
            Lane {
                id,
                track_id: track,
                clip_ids: Vec::new(),
            },
        );
    }
    p.assets.insert(
        asset_id,
        Asset {
            id: asset_id,
            digest: "aa".into(),
        },
    );
    p.sources.insert(
        fx.source,
        Source {
            id: fx.source,
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

    // recording registry
    let session_id = RecordingSessionId::new(6);
    let epoch = ClockEpochId::new(18);
    p.recording.sessions.insert(
        session_id,
        RecordingSession {
            id: session_id,
            name: "s".into(),
            state: session_state,
        },
    );
    for (i, id) in [
        RecordingPassId::new(7),
        RecordingPassId::new(8),
        RecordingPassId::new(9),
    ]
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
    for (id, lane) in [
        (CaptureStreamId::new(10), fx.lane_a),
        (CaptureStreamId::new(11), fx.lane_b),
    ] {
        p.recording.streams.insert(
            id,
            CaptureStream {
                id,
                session_id,
                lane_id: lane,
                clock_epoch_id: epoch,
            },
        );
    }
    for (id, stream, pass, s_lo, s_hi) in [
        (
            CaptureSegmentId::new(12),
            CaptureStreamId::new(10),
            RecordingPassId::new(7),
            0_i64,
            30_000_i64,
        ),
        (
            CaptureSegmentId::new(13),
            CaptureStreamId::new(10),
            RecordingPassId::new(8),
            40_000,
            70_000,
        ),
        (
            CaptureSegmentId::new(14),
            CaptureStreamId::new(11),
            RecordingPassId::new(9),
            0,
            1_000,
        ),
    ] {
        p.recording.segments.insert(
            id,
            CaptureSegment {
                id,
                stream_id: stream,
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
                source_id: fx.source,
            },
        );
    }
    for (id, pass, stream, lane, segs) in [
        (
            fx.take_a0,
            RecordingPassId::new(7),
            CaptureStreamId::new(10),
            fx.lane_a,
            vec![CaptureSegmentId::new(12)],
        ),
        (
            fx.take_a1,
            RecordingPassId::new(8),
            CaptureStreamId::new(10),
            fx.lane_a,
            vec![CaptureSegmentId::new(13)],
        ),
        (
            fx.take_b,
            RecordingPassId::new(9),
            CaptureStreamId::new(11),
            fx.lane_b,
            vec![CaptureSegmentId::new(14)],
        ),
    ] {
        p.recording.takes.insert(
            id,
            Take {
                id,
                session_id,
                pass_id: pass,
                stream_id: stream,
                lane_id: lane,
                source_id: fx.source,
                segment_ids: segs,
                state: TakeState::Verified,
            },
        );
    }

    let mut c = ProjectController::new(p, Box::new(CounterIdGenerator::new(1_000)));

    // Scoped group on lane_a: comp with 3 regions + active + transition.
    apply_comp(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: fx.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![fx.take_a0, fx.take_a1],
            source_rate: RATE,
        },
    );
    let mut fx = fx;
    fx.group = *c.live().comp.take_groups.keys().next().unwrap();
    apply_comp(
        &mut c,
        CompOp::CreateComp {
            take_group_id: fx.group,
        },
    );
    fx.comp = *c.live().comp.comps.keys().next().unwrap();
    for (range, candidate) in [
        (
            mr(0, 960),
            CompCandidateRef::Take {
                take_id: fx.take_a0,
                source_offset: SourceFrame::ZERO,
            },
        ),
        (
            mr(960, 1_920),
            CompCandidateRef::ExistingSource {
                source_id: fx.source,
                handle: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(96_000).unwrap())
                    .unwrap(),
                play_from: SourceFrame::new(2_000).unwrap(),
            },
        ),
        (mr(1_920, 2_000), CompCandidateRef::Silence),
    ] {
        apply_comp(
            &mut c,
            CompOp::AddCompRegion {
                comp_id: fx.comp,
                project_range: range,
                candidate,
                mapping: PlaybackMapping::Natural,
            },
        );
    }
    fx.regions = c.live().comp.comps[&fx.comp].region_ids.clone();
    apply_comp(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: fx.group,
            comp_id: Some(fx.comp),
        },
    );
    apply_comp(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: fx.regions[0],
            right_region_id: fx.regions[1],
            overlap_range: mr(900, 1_020),
            fade_out: FadeShape::EqualPower,
            fade_in: FadeShape::Linear,
        },
    );
    fx.transition = *c.live().comp.transitions.keys().next().unwrap();

    // Unrelated group on lane_b — must stay untouched by lane_a adopts.
    apply_comp(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: fx.lane_b,
            target_range: mr(0, 500),
            take_ids: vec![fx.take_b],
            source_rate: RATE,
        },
    );
    fx.group_b = *c
        .live()
        .comp
        .take_groups
        .keys()
        .find(|id| **id != fx.group)
        .unwrap();
    apply_comp(
        &mut c,
        CompOp::CreateComp {
            take_group_id: fx.group_b,
        },
    );
    let comp_b = *c
        .live()
        .comp
        .comps
        .keys()
        .find(|id| **id != fx.comp)
        .unwrap();
    fx.comp_b = comp_b;
    apply_comp(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp_b,
            // take_b's captured handle is only 1_000 source frames —
            // keep the region small enough to fit it.
            project_range: mr(0, 30),
            candidate: CompCandidateRef::Take {
                take_id: fx.take_b,
                source_offset: SourceFrame::ZERO,
            },
            mapping: PlaybackMapping::Natural,
        },
    );
    apply_comp(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: fx.group_b,
            comp_id: Some(comp_b),
        },
    );

    (c, fx)
}

fn take_scope(group: TakeGroupId) -> VariantScope {
    VariantScope::TakeUsage {
        target: TakeUsageTarget { take_group: group },
    }
}

fn comp_json(c: &ProjectController) -> serde_json::Value {
    serde_json::to_value(&c.live().comp).unwrap()
}

// =====================================================================
// CB-CLIP-01: two processing candidates share one raw asset; fork /
// save / reload leaves blob SHA + source metadata bitwise unchanged.
// =====================================================================

#[test]
fn cb_clip_01_shared_raw_asset_candidates_survive_fork_save_reload() {
    let (c, fx) = clip_fixture();
    let mut fxv = Fx::clip(c);
    let raw_before = raw_snapshot(&fxv.controller);

    // Two variant sets — one per clip — each sharing the same raw
    // asset/source. Candidate edits diverge; live never moves.
    let scope_a = audio_scope(fx.clip_a);
    let receipt_a = fxv.fork(scope_a.clone());
    fxv.edit(
        receipt_a.variant_set,
        receipt_a.candidate,
        synvelis_project::variant::BranchRevision::new(1),
        ProjectOp::SetClipExtent {
            clip_id: fx.clip_a,
            extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
        },
    );
    fxv.edit(
        receipt_a.variant_set,
        receipt_a.candidate,
        synvelis_project::variant::BranchRevision::new(2),
        ProjectOp::MoveClip {
            clip_id: fx.clip_a,
            anchor: TimeAnchor::Musical(MusicalTick::new(960)),
        },
    );

    let scope_b = audio_scope(fx.clip_b);
    let receipt_b = fxv.fork(scope_b.clone());
    fxv.edit(
        receipt_b.variant_set,
        receipt_b.candidate,
        synvelis_project::variant::BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: fx.clip_b,
            muted: true,
        },
    );
    fxv.edit(
        receipt_b.variant_set,
        receipt_b.candidate,
        synvelis_project::variant::BranchRevision::new(2),
        ProjectOp::SetClipPlayback {
            clip_id: fx.clip_b,
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::new(100).unwrap(),
                    SourceFrame::new(500_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );

    // Live domain state untouched by either candidate.
    assert!(!fxv.controller.live().clips[&fx.clip_a].muted);
    assert!(!fxv.controller.live().clips[&fx.clip_b].muted);
    assert_eq!(
        fxv.controller.live().clips[&fx.clip_a].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(0))
    );
    assert_eq!(raw_snapshot(&fxv.controller), raw_before);

    // Candidate payloads diverge per clip while sharing raw material.
    let cand_a = fxv.candidate(receipt_a.variant_set, receipt_a.candidate);
    let cand_b = fxv.candidate(receipt_b.variant_set, receipt_b.candidate);
    assert_eq!(
        cand_a.clips[&fx.clip_a].placement.extent,
        ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap())
    );
    assert!(cand_b.clips[&fx.clip_b].muted);
    assert_eq!(cand_a.assets, cand_b.assets);
    assert_eq!(cand_a.sources, cand_b.sources);
    assert_eq!(cand_a.recording.takes, cand_b.recording.takes);

    // Save → reload: variant bookkeeping and raw material identical.
    let mut store = InMemoryStore::new();
    fxv.controller.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live().variant_sets.len(), 2);
    for set in c2.live().variant_sets.values() {
        for branch in set.branches.values() {
            assert!(matches!(
                branch.content,
                synvelis_project::variant::BranchContentRef::Captured { .. }
                    | synvelis_project::variant::BranchContentRef::Live { .. }
            ));
        }
    }
    assert_eq!(raw_snapshot(&c2), raw_before);
    assert_eq!(c2.live().assets[&fx.asset].digest, "raw-blob-sha-01");
    assert_eq!(
        c2.live().sources[&fx.source].content_digest,
        "src-meta-digest-01"
    );

    // Re-decoding the stored candidates still verifies — the payloads
    // survived the project reload through their content refs.
    let cand_a2 = fxv.candidate(receipt_a.variant_set, receipt_a.candidate);
    assert_eq!(cand_a, cand_a2);
}

// =====================================================================
// CB-CLIP-02: capability report == enforcement — unsupported stretch /
// loop / pitch / fade / external recipe never report success.
// =====================================================================

#[test]
fn cb_clip_02_capability_report_matches_edit_enforcement() {
    let caps: AudioClipCapabilities = AudioClipAdapter::new().capabilities();
    assert_eq!(caps.trim, ProcessingCapability::Supported);
    assert_eq!(caps.placement, ProcessingCapability::Supported);
    assert_eq!(caps.mute, ProcessingCapability::Supported);
    assert_eq!(caps.clip_notes, ProcessingCapability::Supported);
    assert_eq!(caps.loop_playback, ProcessingCapability::Unsupported);
    assert_eq!(caps.stretch, ProcessingCapability::Unsupported);
    assert_eq!(caps.pitch, ProcessingCapability::Unsupported);
    assert_eq!(caps.fade, ProcessingCapability::Unsupported);
    assert_eq!(caps.external_recipe, ProcessingCapability::Unsupported);

    let (c, fx) = clip_fixture();
    let mut fxv = Fx::clip(c);
    let scope = audio_scope(fx.clip_a);
    let receipt = fxv.fork(scope);
    let rev = synvelis_project::variant::BranchRevision::new(1);
    let live_rev = fxv.controller.live_revision();

    // Representable-but-unlowered mappings reject typed Unsupported —
    // the same verdict the capability report advertises.
    for mapping in [
        PlaybackMapping::Loop,
        PlaybackMapping::SimpleStretch(StretchRatio::new(2, 1).unwrap()),
    ] {
        let err = fxv
            .try_edit(
                receipt.variant_set,
                receipt.candidate,
                rev,
                ProjectOp::SetClipPlayback {
                    clip_id: fx.clip_a,
                    playback: ClipPlayback {
                        source_region: clip_playback().source_region,
                        mapping,
                    },
                },
            )
            .unwrap_err();
        assert!(
            matches!(err, VariantError::Unsupported { .. }),
            "mapping {mapping:?} must reject Unsupported, got {err:?}"
        );
    }
    // Rejected edits advance neither live nor branch revision.
    assert_eq!(fxv.controller.live_revision(), live_rev);
    assert_eq!(
        fxv.controller.live().variant_sets[&receipt.variant_set].branches[&receipt.candidate]
            .content
            .revision(),
        rev
    );

    // Supported edits still land — display and enforcement agree.
    fxv.edit(
        receipt.variant_set,
        receipt.candidate,
        rev,
        ProjectOp::SetClipExtent {
            clip_id: fx.clip_a,
            extent: ClipExtent::Musical(MusicalDuration::new(2 * 960).unwrap()),
        },
    );
    fxv.edit(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(2),
        ProjectOp::AddNote {
            clip: fx.clip_a,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let cand = fxv.candidate(receipt.variant_set, receipt.candidate);
    assert_eq!(cand.notes.len(), 1);
    assert_eq!(cand.notes.values().next().unwrap().clip, fx.clip_a);
    assert!(fxv.controller.live().notes.is_empty());
}

// =====================================================================
// CB-TAKE-01: take selection vs processing are distinct edits; region
// handles / fades of untouched entities are never silently overwritten.
// =====================================================================

#[test]
fn cb_take_01_take_selection_and_processing_are_distinct_scoped_edits() {
    let (c, fx) = take_fixture(SessionState::Finalized);
    let mut fxv = Fx::take(c);
    let scope = take_scope(fx.group);
    let receipt = fxv.fork(scope.clone());

    let comp_before = comp_json(&fxv.controller);

    // Edit 1 — take *selection*: region 0 re-adopts take_a1 instead of
    // take_a0. This is candidate choice, not processing.
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(1),
        BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
            region_id: fx.regions[0],
            candidate: CompCandidateRef::Take {
                take_id: fx.take_a1,
                // take_a1's captured coverage is source [40000, 70000);
                // play_from is an absolute source frame.
                source_offset: SourceFrame::new(40_000).unwrap(),
            },
        }),
    );
    let cand1 = fxv.candidate(receipt.variant_set, receipt.candidate);
    assert_eq!(
        cand1.comp.regions[&fx.regions[0]].candidate,
        CompCandidateRef::Take {
            take_id: fx.take_a1,
            // take_a1's captured coverage is source [40000, 70000);
            // play_from is an absolute source frame.
            source_offset: SourceFrame::new(40_000).unwrap(),
        }
    );
    // Untouched regions keep their candidates/handles bit-identical —
    // no silent overwrite of region 1's ExistingSource handle or
    // region 2's Silence.
    assert_eq!(
        cand1.comp.regions[&fx.regions[1]],
        fxv.controller.live().comp.regions[&fx.regions[1]]
    );
    assert_eq!(
        cand1.comp.regions[&fx.regions[2]],
        fxv.controller.live().comp.regions[&fx.regions[2]]
    );
    // Fades/transitions are untouched by a selection edit.
    assert_eq!(
        cand1.comp.transitions,
        fxv.controller.live().comp.transitions
    );
    // Live comp registry never moved.
    assert_eq!(comp_json(&fxv.controller), comp_before);

    // Edit 2 — *processing*: resize region 1's adopted span. Same
    // take material, different usage — a different scope digest than
    // the selection edit produced.
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(2),
        BranchEditOp::Comp(CompOp::SetCompRegionRange {
            region_id: fx.regions[2],
            project_range: mr(1_920, 1_990),
        }),
    );
    let cand2 = fxv.candidate(receipt.variant_set, receipt.candidate);
    assert_eq!(
        cand2.comp.regions[&fx.regions[2]].project_range,
        mr(1_920, 1_990)
    );
    // The selection edit survives — edits compose, they don't revert.
    assert_eq!(
        cand2.comp.regions[&fx.regions[0]].candidate,
        CompCandidateRef::Take {
            take_id: fx.take_a1,
            // take_a1's captured coverage is source [40000, 70000);
            // play_from is an absolute source frame.
            source_offset: SourceFrame::new(40_000).unwrap(),
        }
    );

    // Selection vs processing yield distinct scope identities —
    // compare surfaces can tell them apart (CB-TAKE-01).
    let digest_selection = scope_digest(&cand1, &scope).unwrap();
    let digest_processing = scope_digest(&cand2, &scope).unwrap();
    assert_ne!(digest_selection, digest_processing);
    assert_ne!(
        digest_processing,
        fxv.render_pin(receipt.variant_set, receipt.adopted)
            .scope_digest
    );
}

// =====================================================================
// CB-TAKE-02: adopt materializes only the scoped comp closure in 1 CAS
// + 1 undo; raw recording / recovery data / the other lane's group
// stay bitwise unchanged.
// =====================================================================

#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn cb_take_02_adopt_writes_only_scoped_comp_entities_single_cas_undo() {
    let (c, fx) = take_fixture(SessionState::Finalized);
    let mut fxv = Fx::take(c);
    let scope = take_scope(fx.group);
    let receipt = fxv.fork(scope.clone());

    // Candidate edits: take selection on comp1's region + a brand-new
    // comp (minted id → lineage) activated on the group.
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(1),
        BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
            region_id: fx.regions[0],
            candidate: CompCandidateRef::Take {
                take_id: fx.take_a1,
                // take_a1's captured coverage is source [40000, 70000);
                // play_from is an absolute source frame.
                source_offset: SourceFrame::new(40_000).unwrap(),
            },
        }),
    );
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(2),
        BranchEditOp::Comp(CompOp::CreateComp {
            take_group_id: fx.group,
        }),
    );
    let cand = fxv.candidate(receipt.variant_set, receipt.candidate);
    let new_comp = *cand
        .comp
        .comps
        .keys()
        .find(|id| **id != fx.comp && cand.comp.comps[*id].take_group_id == fx.group)
        .expect("candidate minted a second comp");
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(3),
        BranchEditOp::Comp(CompOp::AddCompRegion {
            comp_id: new_comp,
            project_range: mr(1_960, 2_000),
            candidate: CompCandidateRef::Silence,
            mapping: PlaybackMapping::Natural,
        }),
    );
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(4),
        BranchEditOp::Comp(CompOp::SetActiveComp {
            take_group_id: fx.group,
            comp_id: Some(new_comp),
        }),
    );

    // Pre-adopt snapshots: raw + unrelated material must not move.
    let raw_before = raw_snapshot(&fxv.controller);
    let comp_before = comp_json(&fxv.controller);
    let group_b_before =
        serde_json::to_value(&fxv.controller.live().comp.take_groups[&fx.group_b]).unwrap();
    let comp_b_before =
        serde_json::to_value(&fxv.controller.live().comp.comps[&fx.comp_b]).unwrap();
    let rev_before = fxv.controller.live_revision();

    let adopt = fxv.adopt(receipt.variant_set, receipt.candidate, &scope);
    assert_eq!(
        adopt.outcome,
        synvelis_project::variant::AdoptOutcome::Applied
    );
    assert_eq!(adopt.project.undo_steps, 1);
    assert_eq!(adopt.after_revision.0, rev_before.0 + 1);

    // Scoped materialization: new comp + its region landed under the
    // SAME minted id (no remap), active pointer flipped, region
    // candidate re-adopted to take_a1.
    let live = fxv.controller.live();
    assert!(live.comp.comps.contains_key(&new_comp));
    assert_eq!(
        live.comp.take_groups[&fx.group].active_comp_id,
        Some(new_comp)
    );
    assert_eq!(
        live.comp.regions[&fx.regions[0]].candidate,
        CompCandidateRef::Take {
            take_id: fx.take_a1,
            // take_a1's captured coverage is source [40000, 70000);
            // play_from is an absolute source frame.
            source_offset: SourceFrame::new(40_000).unwrap(),
        }
    );
    let new_region = live.comp.comps[&new_comp].region_ids[0];
    assert_eq!(
        live.comp.regions[&new_region].candidate,
        CompCandidateRef::Silence
    );

    // Raw recording/session/take/source/asset data and the unrelated
    // lane_b group are bitwise unchanged.
    assert_eq!(raw_snapshot(&fxv.controller), raw_before);
    assert_eq!(
        serde_json::to_value(&fxv.controller.live().comp.take_groups[&fx.group_b]).unwrap(),
        group_b_before
    );
    assert_eq!(
        serde_json::to_value(&fxv.controller.live().comp.comps[&fx.comp_b]).unwrap(),
        comp_b_before
    );

    // One undo restores the exact pre-adopt comp registry.
    assert!(fxv.controller.undo().unwrap());
    assert_eq!(comp_json(&fxv.controller), comp_before);
    assert_eq!(raw_snapshot(&fxv.controller), raw_before);
    // Redo re-applies identically — IDs included.
    assert!(fxv.controller.redo().unwrap());
    assert!(fxv.controller.live().comp.comps.contains_key(&new_comp));
    assert_eq!(
        fxv.controller.live().comp.take_groups[&fx.group].active_comp_id,
        Some(new_comp)
    );
}

// =====================================================================
// CB-TAKE-03: recording-active / missing source / unresolvable source
// time / out-of-scope comp ops all reject typed — never silently.
// =====================================================================

#[test]
fn cb_take_03_recording_active_adopt_rejects_typed() {
    let (c, fx) = take_fixture(SessionState::Open);
    let mut fxv = Fx::take(c);
    let scope = take_scope(fx.group);
    let receipt = fxv.fork(scope.clone());
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(1),
        BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
            region_id: fx.regions[0],
            candidate: CompCandidateRef::Take {
                take_id: fx.take_a1,
                // take_a1's captured coverage is source [40000, 70000);
                // play_from is an absolute source frame.
                source_offset: SourceFrame::new(40_000).unwrap(),
            },
        }),
    );
    let live_before = fxv.controller.live().clone();
    let rev_before = fxv.controller.live_revision();

    let err = fxv
        .try_adopt(receipt.variant_set, receipt.candidate, &scope)
        .unwrap_err();
    assert!(
        matches!(err, VariantError::RecordingActive { .. }),
        "open recording session on the group lane must reject RecordingActive, got {err:?}"
    );
    assert_eq!(*fxv.controller.live(), live_before);
    assert_eq!(fxv.controller.live_revision(), rev_before);
}

#[test]
fn cb_take_03_missing_source_and_unknown_source_time_reject() {
    let (c, fx) = take_fixture(SessionState::Finalized);
    let mut fxv = Fx::take(c);
    let scope = take_scope(fx.group);
    let receipt = fxv.fork(scope);

    // Missing source: a candidate region cannot reference a source id
    // the project does not have — typed rejection at edit time.
    let err = fxv
        .try_edit_op(
            receipt.variant_set,
            receipt.candidate,
            synvelis_project::variant::BranchRevision::new(1),
            BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
                region_id: fx.regions[1],
                candidate: CompCandidateRef::ExistingSource {
                    source_id: SourceId::new(999),
                    handle: SourceRange::try_new(
                        SourceFrame::ZERO,
                        SourceFrame::new(1_000).unwrap(),
                    )
                    .unwrap(),
                    play_from: SourceFrame::ZERO,
                },
            }),
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Project(Error::NotFound(_))),
        "missing source must reject typed NotFound, got {err:?}"
    );

    // Source time unknown / unmappable: play_from outside the declared
    // handle is rejected by the comp oracle — no guessed mapping.
    let err = fxv
        .try_edit_op(
            receipt.variant_set,
            receipt.candidate,
            synvelis_project::variant::BranchRevision::new(1),
            BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
                region_id: fx.regions[1],
                candidate: CompCandidateRef::ExistingSource {
                    source_id: fx.source,
                    handle: SourceRange::try_new(
                        SourceFrame::ZERO,
                        SourceFrame::new(1_000).unwrap(),
                    )
                    .unwrap(),
                    play_from: SourceFrame::new(50_000).unwrap(), // outside handle
                },
            }),
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Project(Error::Validation(_))),
        "unmappable source time must reject typed, got {err:?}"
    );

    // Referenced take that does not exist — same typed rejection.
    let err = fxv
        .try_edit_op(
            receipt.variant_set,
            receipt.candidate,
            synvelis_project::variant::BranchRevision::new(1),
            BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
                region_id: fx.regions[0],
                candidate: CompCandidateRef::Take {
                    take_id: TakeId::new(999),
                    source_offset: SourceFrame::ZERO,
                },
            }),
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::Project(_)), "got {err:?}");

    // Adopt-side guard: a candidate whose comp closure references
    // shared material live lost fails closed in preflight (typed
    // NotFound) before any write is planned.
    let (c2, fx2) = take_fixture(SessionState::Finalized);
    let mut fxv_alt = Fx::take(c2);
    let scope2 = take_scope(fx2.group);
    let receipt2 = fxv_alt.fork(scope2.clone());
    let mut crafted = fxv_alt.candidate(receipt2.variant_set, receipt2.candidate);
    crafted
        .comp
        .take_groups
        .get_mut(&fx2.group)
        .unwrap()
        .take_ids
        .push(TakeId::new(999));
    let adapter = TakeUsageAdapter::new();
    let err = adapter
        .preflight_adopt(fxv_alt.controller.live(), &crafted, &scope2)
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Project(Error::NotFound(_))),
        "lost take reference must reject typed NotFound, got {err:?}"
    );
    // Same for a region candidate pointing at a vanished source.
    let mut crafted = fxv_alt.candidate(receipt2.variant_set, receipt2.candidate);
    let region = crafted.comp.regions.get_mut(&fx2.regions[1]).unwrap();
    region.candidate = CompCandidateRef::ExistingSource {
        source_id: SourceId::new(999),
        handle: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(1_000).unwrap()).unwrap(),
        play_from: SourceFrame::ZERO,
    };
    let err = adapter
        .preflight_adopt(fxv_alt.controller.live(), &crafted, &scope2)
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Project(Error::NotFound(_))),
        "lost source reference must reject typed NotFound, got {err:?}"
    );
}

#[test]
fn cb_take_03_out_of_scope_and_unsupported_comp_ops_reject() {
    let (c, fx) = take_fixture(SessionState::Finalized);
    let mut fxv = Fx::take(c);
    let scope = take_scope(fx.group);
    let receipt = fxv.fork(scope.clone());
    let rev = synvelis_project::variant::BranchRevision::new(1);
    let comp_before = comp_json(&fxv.controller);

    // Project ops are not the take-usage edit surface.
    let err = fxv
        .try_edit(
            receipt.variant_set,
            receipt.candidate,
            rev,
            ProjectOp::SetProjectName { name: "x".into() },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));

    // Registering a NEW take group creates an entity outside the
    // scoped group — scope violation, not a silent add.
    let err = fxv
        .try_edit_op(
            receipt.variant_set,
            receipt.candidate,
            rev,
            BranchEditOp::Comp(CompOp::RegisterTakeGroup {
                lane_id: fx.lane_b,
                target_range: mr(0, 100),
                take_ids: vec![fx.take_b],
                source_rate: RATE,
            }),
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::ScopeViolation { .. }),
        "foreign take-group registration must reject ScopeViolation, got {err:?}"
    );

    // An op targeting the OTHER group's comp is out of scope.
    let err = fxv
        .try_edit_op(
            receipt.variant_set,
            receipt.candidate,
            rev,
            BranchEditOp::Comp(CompOp::SetActiveComp {
                take_group_id: fx.group_b,
                comp_id: Some(fx.comp_b),
            }),
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::ScopeViolation { .. }),
        "foreign-group comp op must reject ScopeViolation, got {err:?}"
    );

    // Comping removals fail closed Unsupported in this slice.
    let err = fxv
        .try_edit_op(
            receipt.variant_set,
            receipt.candidate,
            rev,
            BranchEditOp::Comp(CompOp::RemoveCompRegion {
                region_id: fx.regions[2],
            }),
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "comp removal must reject Unsupported, got {err:?}"
    );

    // Non-Natural region mapping: representable but unlowered —
    // honest Unsupported, matching clip-side capability truth.
    let err = fxv
        .try_edit_op(
            receipt.variant_set,
            receipt.candidate,
            rev,
            BranchEditOp::Comp(CompOp::AddCompRegion {
                comp_id: fx.comp,
                project_range: mr(2_000, 2_100),
                candidate: CompCandidateRef::Silence,
                mapping: PlaybackMapping::Loop,
            }),
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "Loop region mapping must reject Unsupported, got {err:?}"
    );

    // None of the rejections moved live or the branch.
    assert_eq!(comp_json(&fxv.controller), comp_before);
    assert_eq!(
        fxv.controller.live().variant_sets[&receipt.variant_set].branches[&receipt.candidate]
            .content
            .revision(),
        rev
    );
}

// =====================================================================
// CB-TAKE-04: non-adopted candidates stay re-comparable until explicit
// Discard; undo/redo/reload restores identical source lineage.
// =====================================================================

#[test]
fn cb_take_04_candidate_reusable_until_discard_and_lineage_survives_reload() {
    let (c, fx) = take_fixture(SessionState::Finalized);
    let mut fxv = Fx::take(c);
    let scope = take_scope(fx.group);
    let receipt = fxv.fork(scope.clone());
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(1),
        BranchEditOp::Comp(CompOp::SetCompRegionCandidate {
            region_id: fx.regions[0],
            candidate: CompCandidateRef::Take {
                take_id: fx.take_a1,
                // take_a1's captured coverage is source [40000, 70000);
                // play_from is an absolute source frame.
                source_offset: SourceFrame::new(40_000).unwrap(),
            },
        }),
    );

    // The non-adopted candidate resolves through the render read
    // adapter — re-comparison works without adopting.
    let cand_project = fxv.render_project(receipt.variant_set, receipt.candidate);
    assert_eq!(
        cand_project.comp.regions[&fx.regions[0]].candidate,
        CompCandidateRef::Take {
            take_id: fx.take_a1,
            // take_a1's captured coverage is source [40000, 70000);
            // play_from is an absolute source frame.
            source_offset: SourceFrame::new(40_000).unwrap(),
        }
    );
    // Its render view shares live raw material — takes/sources are
    // the same entities, never transplanted copies.
    assert_eq!(
        cand_project.recording.takes,
        fxv.controller.live().recording.takes
    );
    assert_eq!(cand_project.sources, fxv.controller.live().sources);

    // A second edit keeps the candidate comparable — digests move.
    let pin1 = fxv.render_pin(receipt.variant_set, receipt.candidate);
    fxv.edit_op(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(2),
        BranchEditOp::Comp(CompOp::SetCompTransition {
            transition_id: fx.transition,
            // Tight overlap: region 1's ExistingSource play_from=2000
            // must still cover the pre-boundary overhang.
            overlap_range: mr(930, 990),
            fade_out: FadeShape::Linear,
            fade_in: FadeShape::EqualPower,
        }),
    );
    let pin2 = fxv.render_pin(receipt.variant_set, receipt.candidate);
    assert_ne!(pin1.scope_digest, pin2.scope_digest);

    // Undo/redo of an unrelated live op leaves the candidate's
    // content ref and lineage untouched.
    let content_before = fxv.controller.live().variant_sets[&receipt.variant_set].branches
        [&receipt.candidate]
        .content
        .clone();
    let lineage_before = fxv.controller.live().variant_sets[&receipt.variant_set].branches
        [&receipt.candidate]
        .lineage
        .clone();
    apply_op(
        &mut fxv.controller,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    assert!(fxv.controller.undo().unwrap());
    assert!(fxv.controller.redo().unwrap());
    let set = &fxv.controller.live().variant_sets[&receipt.variant_set];
    assert_eq!(set.branches[&receipt.candidate].content, content_before);
    assert_eq!(set.branches[&receipt.candidate].lineage, lineage_before);

    // Save → reload: identical content ref + lineage — the same
    // source lineage is restored, not re-derived.
    let mut store = InMemoryStore::new();
    fxv.controller.save(&mut store).unwrap();
    let mut c2 = ProjectController::new(
        Project::new(ProjectId::new(0), "cb02-take"),
        Box::new(CounterIdGenerator::new(1_000)),
    );
    c2.load(&store).unwrap();
    let set2 = &c2.live().variant_sets[&receipt.variant_set];
    assert_eq!(set2.branches[&receipt.candidate].content, content_before);
    assert_eq!(set2.branches[&receipt.candidate].lineage, lineage_before);
    // The reloaded candidate still decodes and still selects take_a1.
    let content = set2.branches[&receipt.candidate].content.clone();
    let cand = decode_candidate(&fxv.store, &content).unwrap();
    assert_eq!(
        cand.comp.regions[&fx.regions[0]].candidate,
        CompCandidateRef::Take {
            take_id: fx.take_a1,
            // take_a1's captured coverage is source [40000, 70000);
            // play_from is an absolute source frame.
            source_offset: SourceFrame::new(40_000).unwrap(),
        }
    );

    // Explicit Discard is the only way the candidate goes away —
    // live and the adopted branch are untouched.
    let live_before = fxv.controller.live().clone();
    discard_branch(&mut fxv.controller, receipt.variant_set, receipt.candidate).unwrap();
    let set = &fxv.controller.live().variant_sets[&receipt.variant_set];
    assert_eq!(set.branches.len(), 1);
    assert!(set.branches.contains_key(&receipt.adopted));
    // Live domain state identical — discard never writes domain data.
    let mut expected = live_before;
    expected.variant_sets = fxv.controller.live().variant_sets.clone();
    assert_eq!(*fxv.controller.live(), expected);
}

// =====================================================================
// CB-AUDIO-RCP-01: branch render resolution gives each candidate its
// own reuse identity; scoped edits invalidate only the scope digest —
// shared context and raw material stay identical; adopt/undo move the
// identity across the adopt boundary without gaps.
// =====================================================================

#[test]
fn cb_audio_rcp_01_clip_branch_render_reuse_and_partial_invalidation() {
    let (c, fx) = clip_fixture();
    let mut fxv = Fx::clip(c);
    let scope = audio_scope(fx.clip_a);
    let receipt = fxv.fork(scope.clone());

    // Pre-edit: adopted (Live) and captured candidate resolve to the
    // same scope identity — identical content, identical reuse key.
    let pin_adopted = fxv.render_pin(receipt.variant_set, receipt.adopted);
    let pin_cand = fxv.render_pin(receipt.variant_set, receipt.candidate);
    assert_eq!(pin_adopted.scope_digest, pin_cand.scope_digest);
    assert_eq!(
        pin_adopted.shared_context_digest,
        pin_cand.shared_context_digest
    );

    // Trim edit: only the scoped clip moves — scope digest changes,
    // shared context (everything outside the clip scope) does not.
    fxv.edit(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(1),
        ProjectOp::SetClipExtent {
            clip_id: fx.clip_a,
            extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
        },
    );
    let pin_cand_trim = fxv.render_pin(receipt.variant_set, receipt.candidate);
    assert_ne!(pin_cand.scope_digest, pin_cand_trim.scope_digest);
    assert_eq!(
        pin_adopted.shared_context_digest, pin_cand_trim.shared_context_digest,
        "scoped clip edit must not invalidate shared context"
    );
    // The composed render view carries the candidate's trim while raw
    // material resolves identically to live.
    let composed = fxv.render_project(receipt.variant_set, receipt.candidate);
    assert_eq!(
        composed.clips[&fx.clip_a].placement.extent,
        ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap())
    );
    assert_eq!(composed.sources, fxv.controller.live().sources);
    assert_eq!(composed.assets, fxv.controller.live().assets);
    assert_eq!(
        composed.recording.takes,
        fxv.controller.live().recording.takes
    );

    // Placement edit moves the identity again — no silent no-op.
    fxv.edit(
        receipt.variant_set,
        receipt.candidate,
        synvelis_project::variant::BranchRevision::new(2),
        ProjectOp::MoveClip {
            clip_id: fx.clip_a,
            anchor: TimeAnchor::Musical(MusicalTick::new(960)),
        },
    );
    let pin_cand_move = fxv.render_pin(receipt.variant_set, receipt.candidate);
    assert_ne!(pin_cand_trim.scope_digest, pin_cand_move.scope_digest);

    // Adopt: the candidate branch becomes the adopted one — its
    // render identity is the new live scope content, so the verified
    // render stays valid across the adopt boundary. The previous
    // adopted branch freezes with the pre-adopt scope content.
    fxv.adopt(receipt.variant_set, receipt.candidate, &scope);
    assert_eq!(
        fxv.render_pin(receipt.variant_set, receipt.candidate)
            .scope_digest,
        pin_cand_move.scope_digest
    );
    assert_eq!(
        fxv.render_pin(receipt.variant_set, receipt.adopted)
            .scope_digest,
        pin_adopted.scope_digest
    );
    assert!(!fxv.controller.live().clips[&fx.clip_a].muted);
    assert_eq!(
        fxv.controller.live().clips[&fx.clip_a].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(960))
    );

    // Undo restores live AND the render identity together.
    assert!(fxv.controller.undo().unwrap());
    let pin_adopted_undone = fxv.render_pin(receipt.variant_set, receipt.adopted);
    assert_eq!(pin_adopted_undone.scope_digest, pin_adopted.scope_digest);
    assert_eq!(
        fxv.controller.live().clips[&fx.clip_a].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(0))
    );
    // Raw material never moved through the whole sequence.
    let live = fxv.controller.live();
    assert_eq!(
        live.sources[&fx.source].content_digest,
        "src-meta-digest-01"
    );
    assert_eq!(live.assets[&fx.asset].digest, "raw-blob-sha-01");
}

#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn cb_audio_rcp_01_take_branch_render_reuse_and_partial_invalidation() {
    let (c, fx) = take_fixture(SessionState::Finalized);
    let mut fxv = Fx::take(c);
    let scope = take_scope(fx.group);
    let receipt = fxv.fork(scope.clone());

    let pin_adopted = fxv.render_pin(receipt.variant_set, receipt.adopted);
    let pin_cand = fxv.render_pin(receipt.variant_set, receipt.candidate);
    assert_eq!(pin_adopted.scope_digest, pin_cand.scope_digest);

    // Take selection + range resize (trim) + transition fade
    // (crossfade) — three scoped edits, each moving the identity.
    let mut last = pin_cand.scope_digest;
    for (i, op) in [
        CompOp::SetCompRegionCandidate {
            region_id: fx.regions[0],
            candidate: CompCandidateRef::Take {
                take_id: fx.take_a1,
                // take_a1's captured coverage is source [40000, 70000);
                // play_from is an absolute source frame.
                source_offset: SourceFrame::new(40_000).unwrap(),
            },
        },
        CompOp::SetCompRegionRange {
            region_id: fx.regions[2],
            project_range: mr(1_920, 1_990),
        },
        CompOp::SetCompTransition {
            transition_id: fx.transition,
            // Tight overlap: region 1's ExistingSource play_from=2000
            // must still cover the pre-boundary overhang.
            overlap_range: mr(930, 990),
            fade_out: FadeShape::Linear,
            fade_in: FadeShape::EqualPower,
        },
    ]
    .into_iter()
    .enumerate()
    {
        fxv.edit_op(
            receipt.variant_set,
            receipt.candidate,
            synvelis_project::variant::BranchRevision::new(i as u64 + 1),
            BranchEditOp::Comp(op),
        );
        let pin = fxv.render_pin(receipt.variant_set, receipt.candidate);
        assert_ne!(
            last, pin.scope_digest,
            "edit {i} must move the scope identity"
        );
        last = pin.scope_digest;
    }
    // Shared context (everything outside the take group) never moved.
    assert_eq!(
        pin_adopted.shared_context_digest,
        fxv.render_pin(receipt.variant_set, receipt.candidate)
            .shared_context_digest
    );

    // The composed render view materializes candidate comp state over
    // live shared raw material — takes/sources identical to live.
    let composed = fxv.render_project(receipt.variant_set, receipt.candidate);
    assert_eq!(
        composed.comp.regions[&fx.regions[0]].candidate,
        CompCandidateRef::Take {
            take_id: fx.take_a1,
            // take_a1's captured coverage is source [40000, 70000);
            // play_from is an absolute source frame.
            source_offset: SourceFrame::new(40_000).unwrap(),
        }
    );
    assert_eq!(
        composed.recording.takes,
        fxv.controller.live().recording.takes
    );
    assert_eq!(composed.sources, fxv.controller.live().sources);
    assert_eq!(composed.assets, fxv.controller.live().assets);
    // The unrelated lane_b group resolves identically in both views.
    assert_eq!(
        serde_json::to_value(&composed.comp.take_groups[&fx.group_b]).unwrap(),
        serde_json::to_value(&fxv.controller.live().comp.take_groups[&fx.group_b]).unwrap()
    );

    // Adopt → the candidate branch becomes adopted and carries the
    // candidate identity; the previous adopted branch freezes with
    // the pre-adopt content. Undo restores live comp state, the
    // adopted pointer, and the render identity together.
    let scope_digest_candidate = last;
    fxv.adopt(receipt.variant_set, receipt.candidate, &scope);
    assert_eq!(
        fxv.render_pin(receipt.variant_set, receipt.candidate)
            .scope_digest,
        scope_digest_candidate
    );
    assert_eq!(
        fxv.render_pin(receipt.variant_set, receipt.adopted)
            .scope_digest,
        pin_adopted.scope_digest
    );
    assert_eq!(
        fxv.controller.live().comp.regions[&fx.regions[0]].candidate,
        CompCandidateRef::Take {
            take_id: fx.take_a1,
            // take_a1's captured coverage is source [40000, 70000);
            // play_from is an absolute source frame.
            source_offset: SourceFrame::new(40_000).unwrap(),
        }
    );
    assert!(fxv.controller.undo().unwrap());
    assert_eq!(
        fxv.render_pin(receipt.variant_set, receipt.adopted)
            .scope_digest,
        pin_adopted.scope_digest
    );
    assert_eq!(
        fxv.controller.live().comp.regions[&fx.regions[0]].candidate,
        CompCandidateRef::Take {
            take_id: fx.take_a0,
            source_offset: SourceFrame::ZERO,
        }
    );
    // Raw recording material is bitwise identical end to end.
    let live = fxv.controller.live();
    for take in live.recording.takes.values() {
        assert_eq!(take.state, TakeState::Verified);
    }
    assert_eq!(live.sources[&fx.source].content_digest, "aa");
    assert_eq!(live.assets.values().next().unwrap().digest, "aa");
}
