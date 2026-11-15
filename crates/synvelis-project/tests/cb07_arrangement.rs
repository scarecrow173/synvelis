//! CB-07 Creative Branch arrangement adapter fixtures (synvelis#284).
//!
//! Covers the real `ArrangementAdapter` headlessly: track-set ×
//! `MusicalRange` × `ArrangementDomainMask` scope confinement,
//! in-range clip/note add-move-delete candidate edits with the
//! move-out/sweep-containment rules, mask-relative capability and
//! boundary inventory (absolute anchors reported, never silently
//! owned), one-CAS adopt with undo/save-reload and stale-scope/
//! recording gates, branch-minted ID lineage, semantic diff
//! classification, and the honest adopt deferral for scope-removal
//! candidates. Real OS / plugin / RT / PCM / GUI timeline lanes are
//! `NotRun` — recorded in `evidence/sv284/`.

use std::collections::BTreeSet;

use synvelis_project::comp::CompOp;
use synvelis_project::controller::{Command, ProjectController};
use synvelis_project::diff::{
    ChangeKind, ComparisonSpec, ObjectKind, SemanticChange, compare_snapshots,
};
use synvelis_project::id::CounterIdGenerator;
use synvelis_project::project::Project;
use synvelis_project::recording_data::{CaptureStream, ClockEpoch, RecordingSession, SessionState};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, ContentDigest, FixedSnapshotLocator, ImmutableSnapshot,
    ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness, SnapshotIdentity,
    SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::{InMemoryStore, SCHEMA_VERSION};
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, ArrangementAdapter, ArrangementBoundaryReason,
    ArrangementDomainMask, BranchEditOp, BranchEditOpIntent, DomainAdapter,
    InMemoryBranchContentStore, NoopRuntimePublish, ProcessingCapability, VariantError,
    VariantScope, adopt_branch_adapted, decode_candidate, edit_branch_op, fork_variant_set,
    scope_digest,
};
use synvelis_project::{
    AssetId, BeatPoint, BranchId, CaptureStreamId, ClipExtent, ClipId, ClipPlacement, ClockEpochId,
    LaneId, MediaDuration, MediaTimeValue, MusicalDuration, MusicalRange, MusicalTick, NoteId,
    PlaybackMapping, ProjectId, ProjectOp, ProjectRevision, RecordingSessionId, RenderBranchTarget,
    SourceFrame, SourceId, SourceRange, TakeGroupId, TimeAnchor, TrackId, VariantSetId,
    resolve_render_project,
};

// ---------- shared harness (cb06_strip_fields.rs conventions) ----------

struct Arrangement {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    resolver: SnapshotResolver,
    adapter: ArrangementAdapter,
}

impl Arrangement {
    fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            resolver: SnapshotResolver::new(),
            adapter: ArrangementAdapter::new(),
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

    fn try_adopt(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
    ) -> Result<synvelis_project::variant::AdoptReceipt, VariantError> {
        let runtime = NoopRuntimePublish;
        let intent = self.intent(set, branch, scope);
        adopt_branch_adapted(
            &mut self.controller,
            &self.adapter,
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

// ---------- fixture: two scoped tracks + one foreign track ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb07-fixture"),
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

struct ArrangementIds {
    track_a: TrackId,
    track_b: TrackId,
    lane_a1: LaneId,
    lane_a2: LaneId,
    lane_b1: LaneId,
    lane_foreign: LaneId,
    source: SourceId,
    /// `[0, 7680)` on `lane_a1` — overlaps the scope range.
    clip_a_in: ClipId,
    /// `[15000, 22680)` on `lane_a1` — partial edge overlap.
    clip_a_edge: ClipId,
    /// `[40000, 47680)` on `lane_a1` — musical but outside `time`.
    clip_a_out: ClipId,
    /// Absolute-anchored clip on `lane_a1` — never scope-owned.
    clip_a_abs: ClipId,
    /// `[9600, 17280)` on `lane_b1` — second scoped track.
    clip_b_in: ClipId,
    /// On `lane_foreign` — outside every scoped track.
    clip_foreign: ClipId,
    /// Clip-local note on `clip_a_in`.
    note_in: NoteId,
    /// Clip-local note on `clip_foreign` — wholly outside the scope.
    note_foreign: NoteId,
}

const BEAT: i64 = 960;
/// Scope window `[1000, 20000)` in musical ticks.
const T0: i64 = 1000;
const T1: i64 = 20000;

fn musical_placement(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(8 * BEAT).unwrap()),
    }
}

fn absolute_placement() -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(MediaTimeValue::new(0, 1, 48_000).unwrap()),
        extent: ClipExtent::Absolute(MediaDuration::new(48_000, 1, 48_000).unwrap()),
    }
}

fn clip_playback() -> synvelis_project::ClipPlayback {
    synvelis_project::ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn add_clip_at(c: &mut ProjectController, lane: LaneId, source: SourceId, tick: i64) -> ClipId {
    apply_op(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: musical_placement(tick),
            playback: clip_playback(),
        },
    );
    c.live()
        .clips
        .values()
        .filter(|clip| clip.lane_id == lane)
        .filter(|clip| {
            matches!(
                clip.placement.anchor,
                TimeAnchor::Musical(t) if t.value() == tick
            )
        })
        .map(|clip| clip.id)
        .next_back()
        .unwrap()
}

/// Two scoped tracks (a: 2 lanes, b: 1 lane) + one foreign track,
/// one shared media source, in/out/absolute clips per lane, one
/// clip-local note.
fn arrangement_fixture() -> (ProjectController, ArrangementIds) {
    let mut c = controller();
    for name in ["t-a", "t-b", "t-foreign"] {
        apply_op(&mut c, ProjectOp::AddTrack { name: name.into() });
    }
    let mut tracks: Vec<TrackId> = c.live().tracks.keys().copied().collect();
    tracks.sort();
    let (track_a, track_b, track_foreign) = (tracks[0], tracks[1], tracks[2]);
    for _ in 0..2 {
        apply_op(&mut c, ProjectOp::AddLane { track_id: track_a });
    }
    apply_op(&mut c, ProjectOp::AddLane { track_id: track_b });
    apply_op(
        &mut c,
        ProjectOp::AddLane {
            track_id: track_foreign,
        },
    );
    let lane_ids_of = |track: TrackId| c.live().tracks[&track].lane_ids.clone();
    let a_lanes = lane_ids_of(track_a);
    let b_lanes = lane_ids_of(track_b);
    let x_lanes = lane_ids_of(track_foreign);
    let (lane_main, lane_spare) = (a_lanes[0], a_lanes[1]);
    let lane_b = b_lanes[0];
    let lane_out = x_lanes[0];

    apply_op(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: "raw-blob-sha-07".into(),
        },
    );
    let asset: AssetId = *c.live().assets.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "src-meta-digest-07".into(),
        },
    );
    let source: SourceId = *c.live().sources.keys().next().unwrap();

    let clip_a_in = add_clip_at(&mut c, lane_main, source, 0);
    let clip_a_edge = add_clip_at(&mut c, lane_main, source, 15 * BEAT);
    let clip_a_out = add_clip_at(&mut c, lane_main, source, 40 * BEAT);
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane_main,
            source_id: source,
            placement: absolute_placement(),
            playback: clip_playback(),
        },
    );
    let clip_a_abs = c
        .live()
        .clips
        .values()
        .filter(|clip| clip.lane_id == lane_main)
        .filter(|clip| matches!(clip.placement.anchor, TimeAnchor::Absolute(_)))
        .map(|clip| clip.id)
        .next_back()
        .unwrap();
    let clip_on_b = add_clip_at(&mut c, lane_b, source, 10 * BEAT);
    let clip_foreign = add_clip_at(&mut c, lane_out, source, 0);

    apply_op(
        &mut c,
        ProjectOp::AddNote {
            clip: clip_a_in,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let note_in = *c.live().notes.keys().next_back().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddNote {
            clip: clip_foreign,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let note_foreign = *c.live().notes.keys().next_back().unwrap();

    let ids = ArrangementIds {
        track_a,
        track_b,
        lane_a1: lane_main,
        lane_a2: lane_spare,
        lane_b1: lane_b,
        lane_foreign: lane_out,
        source,
        clip_a_in,
        clip_a_edge,
        clip_a_out,
        clip_a_abs,
        clip_b_in: clip_on_b,
        clip_foreign,
        note_in,
        note_foreign,
    };
    (c, ids)
}

fn mask(clips: bool, notes: bool) -> ArrangementDomainMask {
    ArrangementDomainMask { clips, notes }
}

fn scope_of(ids: &ArrangementIds, fields: ArrangementDomainMask) -> VariantScope {
    VariantScope::Arrangement {
        tracks: vec![ids.track_a, ids.track_b],
        time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
        fields,
    }
}

fn all_scope(ids: &ArrangementIds) -> VariantScope {
    scope_of(ids, mask(true, true))
}

// ===========================================================================
// CB-ARR-01: candidate add/move/edit + note changes fork, edit,
// save→load round-trip; A/B established, sibling isolation, live untouched
// ===========================================================================

#[test]
#[allow(clippy::too_many_lines)] // sequential fork/edit/save-reload lifecycle fixture
fn cb_arr_01_candidate_edits_roundtrip_save_reload_and_isolate() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope.clone());
    let (set, cand) = (forked.variant_set, forked.candidate);

    // Candidate edits: add in-range clip, move scoped clip (still in
    // range), mute, trim extent, add + remove clip-local notes.
    h.edit(
        set,
        cand,
        ProjectOp::AddClip {
            lane_id: ids.lane_a2,
            source_id: ids.source,
            placement: musical_placement(5 * BEAT),
            playback: clip_playback(),
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::MoveClip {
            clip_id: ids.clip_b_in,
            anchor: TimeAnchor::Musical(MusicalTick::new(2 * BEAT)),
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_in,
            muted: true,
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::SetClipPlayback {
            clip_id: ids.clip_a_in,
            playback: synvelis_project::ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::new(2_000).unwrap(),
                    SourceFrame::new(1_000_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::SetClipExtent {
            clip_id: ids.clip_a_edge,
            extent: ClipExtent::Musical(MusicalDuration::new(4 * BEAT).unwrap()),
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::AddNote {
            clip: ids.clip_b_in,
            key: 64,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 2).unwrap(),
        },
    );
    h.edit(set, cand, ProjectOp::RemoveNote { note: ids.note_in });

    let candidate = h.candidate(set, cand);
    // Candidate carries every edit; live carries none (sibling
    // isolation — the adopted branch and foreign content untouched).
    assert!(candidate.clips.values().any(|clip| {
        clip.lane_id == ids.lane_a2
            && matches!(clip.placement.anchor, TimeAnchor::Musical(t) if t.value() == 5 * BEAT)
    }));
    assert_eq!(
        candidate.clips[&ids.clip_b_in].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(2 * BEAT))
    );
    assert!(candidate.clips[&ids.clip_a_in].muted);
    assert_eq!(
        candidate.clips[&ids.clip_a_in]
            .playback
            .source_region
            .start(),
        SourceFrame::new(2_000).unwrap(),
        "Natural playback edit landed on the candidate"
    );
    assert_eq!(
        candidate.clips[&ids.clip_a_edge].placement.extent,
        ClipExtent::Musical(MusicalDuration::new(4 * BEAT).unwrap())
    );
    assert_eq!(
        candidate.notes.len(),
        2,
        "seeded in-scope note removed, foreign note + one added remain"
    );
    assert!(candidate.notes.contains_key(&ids.note_foreign));
    assert!(
        candidate
            .notes
            .values()
            .filter(|n| n.clip != ids.clip_foreign)
            .all(|n| n.clip == ids.clip_b_in)
    );

    let live = h.controller.live();
    assert!(!live.clips[&ids.clip_a_in].muted);
    assert_eq!(live.clips.len(), 6, "no candidate clip leaked to live");
    assert!(live.notes.contains_key(&ids.note_in));
    assert_eq!(
        live.clips[&ids.clip_b_in].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(10 * BEAT))
    );

    // Save → reload: the variant set, scope, branch refs and the
    // candidate payload all survive; the reloaded controller sees the
    // same project truth and the store-held candidate still decodes.
    let mut store = InMemoryStore::new();
    h.controller.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live(), h.controller.live());
    let reloaded = &c2.live().variant_sets[&set];
    assert_eq!(reloaded.scope, scope);
    assert_eq!(
        reloaded.branches[&cand].content,
        h.controller.live().variant_sets[&set].branches[&cand]
            .content
            .clone()
    );
    let decoded = decode_candidate(&h.store, &reloaded.branches[&cand].content).unwrap();
    assert!(decoded.clips[&ids.clip_a_in].muted);
    assert!(!decoded.notes.contains_key(&ids.note_in));
}

// ===========================================================================
// CB-ARR-02: semantic diff classifies clip/note Added/Removed/Modified
// ===========================================================================

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

fn object_changes(
    cs: &synvelis_project::diff::ChangeSet,
    kind: ObjectKind,
) -> Vec<&SemanticChange> {
    cs.changes
        .iter()
        .filter(|ch| ch.object.kind == kind)
        .collect()
}

#[test]
fn cb_arr_02_semantic_diff_classifies_clip_and_note_changes() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope);
    let (set, cand) = (forked.variant_set, forked.candidate);

    h.edit(
        set,
        cand,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_in,
            muted: true,
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::AddClip {
            lane_id: ids.lane_a2,
            source_id: ids.source,
            placement: musical_placement(5 * BEAT),
            playback: clip_playback(),
        },
    );
    h.edit(set, cand, ProjectOp::RemoveNote { note: ids.note_in });

    let before = h.controller.live().clone();
    let after = h.candidate(set, cand);
    let cs = compare(&before, &after);

    let clip_changes = object_changes(&cs, ObjectKind::Clip);
    assert!(
        clip_changes.iter().any(|ch| ch.kind == ChangeKind::Modified
            && ch.object.stable_id == ids.clip_a_in.to_tagged()),
        "muted clip must diff Modified, got {clip_changes:?}"
    );
    assert!(
        clip_changes
            .iter()
            .any(|ch| ch.kind == ChangeKind::Added && ch.object.stable_id.starts_with("clip-")),
        "added clip must diff Added, got {clip_changes:?}"
    );
    // Out-of-scope clips never appear in the diff.
    assert!(
        clip_changes
            .iter()
            .all(|ch| ch.object.stable_id != ids.clip_foreign.to_tagged()
                && ch.object.stable_id != ids.clip_a_out.to_tagged()
                && ch.object.stable_id != ids.clip_a_abs.to_tagged()),
        "non-scope clips must not diff, got {clip_changes:?}"
    );

    let note_changes = object_changes(&cs, ObjectKind::Note);
    assert!(
        note_changes
            .iter()
            .any(|ch| ch.kind == ChangeKind::Removed
                && ch.object.stable_id == ids.note_in.to_tagged()),
        "removed note must diff Removed, got {note_changes:?}"
    );
}

// ===========================================================================
// CB-ARR-03: adopt materializes multi-track/clip changes in 1 CAS/Undo
// ===========================================================================

#[test]
fn cb_arr_03_adopt_materializes_scoped_changes_in_one_cas() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope.clone());
    let (set, cand) = (forked.variant_set, forked.candidate);

    h.edit(
        set,
        cand,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_in,
            muted: true,
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::MoveClip {
            clip_id: ids.clip_b_in,
            anchor: TimeAnchor::Musical(MusicalTick::new(3 * BEAT)),
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::SetClipExtent {
            clip_id: ids.clip_a_edge,
            extent: ClipExtent::Musical(MusicalDuration::new(4 * BEAT).unwrap()),
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::AddNote {
            clip: ids.clip_b_in,
            key: 67,
            start: BeatPoint::new(1, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::AddClip {
            lane_id: ids.lane_a2,
            source_id: ids.source,
            placement: musical_placement(6 * BEAT),
            playback: clip_playback(),
        },
    );

    let live_before = h.controller.live().clone();
    let rev_before = h.controller.live_revision();
    let receipt = h.adopt(set, cand, &scope);
    // Exactly one project CAS: live revision advanced by one.
    assert_eq!(receipt.after_revision.0, rev_before.0 + 1);

    let live = h.controller.live();
    assert!(live.clips[&ids.clip_a_in].muted);
    assert_eq!(
        live.clips[&ids.clip_b_in].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(3 * BEAT))
    );
    assert_eq!(
        live.clips[&ids.clip_a_edge].placement.extent,
        ClipExtent::Musical(MusicalDuration::new(4 * BEAT).unwrap())
    );
    assert_eq!(live.notes.len(), 3, "two seeded + adopted note");
    assert!(live.clips.values().any(|clip| {
        clip.lane_id == ids.lane_a2
            && matches!(clip.placement.anchor, TimeAnchor::Musical(t) if t.value() == 6 * BEAT)
    }));
    // Shared context untouched: foreign/out-of-range/absolute clips,
    // sources and assets are byte-identical to pre-adopt live.
    assert_eq!(
        live.clips[&ids.clip_foreign],
        live_before.clips[&ids.clip_foreign]
    );
    assert_eq!(
        live.clips[&ids.clip_a_out],
        live_before.clips[&ids.clip_a_out]
    );
    assert_eq!(
        live.clips[&ids.clip_a_abs],
        live_before.clips[&ids.clip_a_abs]
    );
    assert_eq!(live.sources, live_before.sources);
    assert_eq!(live.assets, live_before.assets);

    // One undo step reverts the whole composite adopt.
    assert!(h.controller.undo().unwrap());
    assert_eq!(*h.controller.live(), live_before);
    assert!(h.controller.redo().unwrap());
    assert!(h.controller.live().clips[&ids.clip_a_in].muted);
}

// ===========================================================================
// CB-ARR-04: boundary inventory + typed rejects for foreign/out-of-scope
// targets, mask-off ops and unsupported op families
// ===========================================================================

#[test]
fn cb_arr_04_boundary_inventory_reports_non_owned_lane_clips() {
    let (c, ids) = arrangement_fixture();
    let scope = all_scope(&ids);

    let deps = ArrangementAdapter::boundary_dependencies(c.live(), &scope).unwrap();
    let by_clip: BTreeSet<ClipId> = deps.iter().map(|d| d.clip).collect();
    assert_eq!(
        by_clip,
        BTreeSet::from([ids.clip_a_out, ids.clip_a_abs]),
        "only non-owned clips on scoped lanes are boundary"
    );
    let abs = deps.iter().find(|d| d.clip == ids.clip_a_abs).unwrap();
    assert_eq!(abs.reason, ArrangementBoundaryReason::AbsoluteAnchor);
    assert_eq!(abs.lane, ids.lane_a1);
    let out = deps.iter().find(|d| d.clip == ids.clip_a_out).unwrap();
    assert_eq!(out.reason, ArrangementBoundaryReason::OutsideRange);
    // Scope-owned clips and wholly-foreign clips are never boundary.
    assert!(!by_clip.contains(&ids.clip_a_in));
    assert!(!by_clip.contains(&ids.clip_b_in));
    assert!(!by_clip.contains(&ids.clip_foreign));

    // Non-arrangement scope is a typed error, not a panic.
    let err = ArrangementAdapter::boundary_dependencies(
        c.live(),
        &VariantScope::AudioClip {
            clip: ids.clip_a_in,
        },
    )
    .unwrap_err();
    assert!(matches!(err, synvelis_project::Error::Validation(_)));
}

#[test]
fn cb_arr_04_typed_rejects_foreign_and_out_of_scope_targets() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope);
    let (set, cand) = (forked.variant_set, forked.candidate);

    // Foreign clip / out-of-range clip / foreign lane: ScopeViolation.
    for op in [
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_foreign,
            muted: true,
        },
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_out,
            muted: true,
        },
        ProjectOp::MoveClip {
            clip_id: ids.clip_foreign,
            anchor: TimeAnchor::Musical(MusicalTick::new(BEAT)),
        },
        ProjectOp::AddClip {
            lane_id: ids.lane_foreign,
            source_id: ids.source,
            placement: musical_placement(BEAT),
            playback: clip_playback(),
        },
        ProjectOp::RemoveNote {
            note: ids.note_foreign,
        },
    ] {
        let err = h.try_edit(set, cand, op).unwrap_err();
        assert!(
            matches!(err, VariantError::ScopeViolation { .. }),
            "expected ScopeViolation, got {err:?}"
        );
    }

    // Unsupported op families: typed Unsupported, never silent.
    for op in [
        ProjectOp::AddTrack { name: "out".into() },
        ProjectOp::AddLane {
            track_id: ids.track_a,
        },
        ProjectOp::DetachClipSource {
            clip_id: ids.clip_a_in,
        },
        ProjectOp::AddMidiNote {
            source_id: ids.source,
            key: 60,
            velocity: 90,
            start: synvelis_project::MidiTick::new(0).unwrap(),
            len: synvelis_project::MidiTick::new(480).unwrap(),
        },
        ProjectOp::SetClipPlayback {
            clip_id: ids.clip_a_in,
            playback: synvelis_project::ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Loop,
            },
        },
    ] {
        let err = h.try_edit(set, cand, op).unwrap_err();
        assert!(
            matches!(err, VariantError::Unsupported { .. }),
            "expected Unsupported, got {err:?}"
        );
    }

    // Comp ops are a different channel — typed Unsupported.
    let err = h
        .try_edit_op(
            set,
            cand,
            BranchEditOp::Comp(CompOp::UnregisterTakeGroup {
                take_group_id: TakeGroupId::new(1),
            }),
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));

    // Rejected edits leave the candidate payload and live untouched.
    let content = h.controller.live().variant_sets[&set].branches[&cand]
        .content
        .clone();
    let live_before = h.controller.live().clone();
    let _ = h.try_edit(
        set,
        cand,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_foreign,
            muted: true,
        },
    );
    assert_eq!(
        h.controller.live().variant_sets[&set].branches[&cand].content,
        content
    );
    assert_eq!(*h.controller.live(), live_before);
}

// ===========================================================================
// CB-ARR-05: absolute anchors never owned — reported, editable-never;
// scope digest / render pin identity stays honest
// ===========================================================================

#[test]
fn cb_arr_05_absolute_anchor_reported_and_render_pin_identity() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let caps = h.adapter.capabilities(&mask(true, true));
    assert_eq!(caps.absolute_anchor, ProcessingCapability::Unsupported);
    assert_eq!(caps.midi_source_edit, ProcessingCapability::Unsupported);
    assert_eq!(caps.control_edit, ProcessingCapability::Unsupported);

    let forked = h.fork(scope.clone());
    let (set, adopted, cand) = (forked.variant_set, forked.adopted, forked.candidate);

    // Absolute clip is never scope-owned: editing it directly is a
    // ScopeViolation, and it never appears in the scope digest.
    let err = h
        .try_edit(
            set,
            cand,
            ProjectOp::SetClipMuted {
                clip_id: ids.clip_a_abs,
                muted: true,
            },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::ScopeViolation { .. }));

    // Scoped candidate edit moves only the candidate's scope digest —
    // the adopted/live mirror and shared context stay pinned (the
    // CB-MIDI-04/RCP-01 identity contract at arrangement scope).
    let pin_adopted_0 = h.render_pin(set, adopted);
    let pin_cand_0 = h.render_pin(set, cand);
    assert_eq!(pin_adopted_0.scope_digest, pin_cand_0.scope_digest);
    assert_eq!(
        pin_adopted_0.shared_context_digest,
        pin_cand_0.shared_context_digest
    );
    h.edit(
        set,
        cand,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_in,
            muted: true,
        },
    );
    let pin_adopted_1 = h.render_pin(set, adopted);
    let pin_cand_1 = h.render_pin(set, cand);
    assert_ne!(pin_cand_0.scope_digest, pin_cand_1.scope_digest);
    assert_eq!(pin_adopted_0.scope_digest, pin_adopted_1.scope_digest);
    assert_eq!(
        pin_cand_0.shared_context_digest, pin_cand_1.shared_context_digest,
        "scoped edits never touch shared context"
    );
    assert_eq!(
        pin_adopted_1.shared_context_digest,
        pin_cand_1.shared_context_digest
    );

    // The absolute clip is shared context: mutating it live moves the
    // shared-context digest for every branch — the comparison context
    // stales instead of reporting false equivalence.
    apply_op(
        &mut h.controller,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_abs,
            muted: true,
        },
    );
    let pin_cand_2 = h.render_pin(set, cand);
    assert_ne!(
        pin_cand_1.shared_context_digest,
        pin_cand_2.shared_context_digest
    );
    assert_eq!(pin_cand_1.scope_digest, pin_cand_2.scope_digest);
}

// ===========================================================================
// CB-ARR-06: branch-minted IDs stay live-space unique; rejected edits
// never partially commit; stale scope digest rejects adopt
// ===========================================================================

#[test]
fn cb_arr_06_branch_ids_unique_and_stale_scope_rejects() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope.clone());
    let (set, cand) = (forked.variant_set, forked.candidate);

    h.edit(
        set,
        cand,
        ProjectOp::AddClip {
            lane_id: ids.lane_a2,
            source_id: ids.source,
            placement: musical_placement(7 * BEAT),
            playback: clip_playback(),
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::AddNote {
            clip: ids.clip_a_in,
            key: 62,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 4).unwrap(),
        },
    );

    let branch = &h.controller.live().variant_sets[&set].branches[&cand];
    assert_eq!(branch.lineage.local_ids.len(), 2, "clip + note minted");
    let minted: Vec<String> = branch.lineage.local_ids.keys().cloned().collect();
    assert!(minted.iter().any(|id| id.starts_with("clip-")));
    assert!(minted.iter().any(|id| id.starts_with("note-")));
    // No minted id collides with any live entity.
    let live_ids: BTreeSet<String> = h
        .controller
        .live()
        .clips
        .keys()
        .map(|id| id.to_tagged())
        .chain(h.controller.live().notes.keys().map(|id| id.to_tagged()))
        .collect();
    for id in &minted {
        assert!(!live_ids.contains(id), "minted id {id} collides with live");
    }

    // Stale scope: a live scoped edit after the intent is pinned
    // rejects StaleScope — no auto-rebase, no partial mutation.
    let intent = h.intent(set, cand, &scope);
    apply_op(
        &mut h.controller,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_edge,
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

// ===========================================================================
// CB-ARR-07: recording gate — open session on a scoped lane rejects
// adopt; a session on a foreign lane does not gate
// ===========================================================================

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
fn cb_arr_07_recording_gate_scoped_vs_foreign_lane() {
    let (c, ids) = arrangement_fixture();
    // Open session on a scoped lane (lane_b1 belongs to track_b).
    let c = with_open_recording(&c, ids.lane_b1);
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope.clone());
    h.edit(
        forked.variant_set,
        forked.candidate,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_in,
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
        "open recording on a scoped lane must reject RecordingActive, got {err:?}"
    );
    assert_eq!(*h.controller.live(), live_before);
    assert_eq!(h.controller.live_revision(), rev_before);

    // Same edit on a project whose only open session sits on the
    // foreign lane — outside every scoped track — adopts cleanly.
    let (c2, ids2) = arrangement_fixture();
    let c2 = with_open_recording(&c2, ids2.lane_foreign);
    let mut h2 = Arrangement::new(c2);
    let scope2 = all_scope(&ids2);
    let forked2 = h2.fork(scope2.clone());
    h2.edit(
        forked2.variant_set,
        forked2.candidate,
        ProjectOp::SetClipMuted {
            clip_id: ids2.clip_a_in,
            muted: true,
        },
    );
    h2.adopt(forked2.variant_set, forked2.candidate, &scope2);
    assert!(h2.controller.live().clips[&ids2.clip_a_in].muted);
}

// ===========================================================================
// CB-ARR-08: mask gating — clips-only / notes-only scopes confine the
// edit surface to the declared families
// ===========================================================================

#[test]
#[allow(clippy::too_many_lines)] // two mask variants across independent controllers
fn cb_arr_08_mask_gating_clips_only_and_notes_only() {
    // clips-only scope on its own controller — mask variants of the
    // same window would otherwise scope-conflict.
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);

    // clips-only: clip ops accepted, note ops rejected typed.
    let clips_scope = scope_of(&ids, mask(true, false));
    let caps = h.adapter.capabilities(&mask(true, false));
    assert_eq!(caps.clip_add, ProcessingCapability::Supported);
    assert_eq!(caps.note_add, ProcessingCapability::Unsupported);
    let f1 = h.fork(clips_scope);
    h.edit(
        f1.variant_set,
        f1.candidate,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_in,
            muted: true,
        },
    );
    h.edit(
        f1.variant_set,
        f1.candidate,
        ProjectOp::RemoveClip {
            clip_id: ids.clip_a_edge,
        },
    );
    // AddNote on an owned clip: mask verdict Unsupported.
    let err = h
        .try_edit(
            f1.variant_set,
            f1.candidate,
            ProjectOp::AddNote {
                clip: ids.clip_a_in,
                key: 60,
                start: BeatPoint::new(0, 1).unwrap(),
                len: BeatPoint::new(1, 1).unwrap(),
            },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));
    // RemoveNote: the notes family is undeclared — the mask verdict
    // precedes target confinement, matching the capability report.
    let err = h
        .try_edit(
            f1.variant_set,
            f1.candidate,
            ProjectOp::RemoveNote { note: ids.note_in },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));

    // notes-only scope on a fresh controller: note ops accepted on
    // owned clips, clip-structure ops rejected Unsupported (clip
    // entities stay confined for ownership but carry no edit surface).
    let (c2, ids2) = arrangement_fixture();
    let mut h2 = Arrangement::new(c2);
    let notes_scope = scope_of(&ids2, mask(false, true));
    let caps = h2.adapter.capabilities(&mask(false, true));
    assert_eq!(caps.note_add, ProcessingCapability::Supported);
    assert_eq!(caps.clip_add, ProcessingCapability::Unsupported);
    let f2 = h2.fork(notes_scope.clone());
    h2.edit(
        f2.variant_set,
        f2.candidate,
        ProjectOp::AddNote {
            clip: ids2.clip_b_in,
            key: 65,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    h2.edit(
        f2.variant_set,
        f2.candidate,
        ProjectOp::RemoveNote { note: ids2.note_in },
    );
    for op in [
        ProjectOp::SetClipMuted {
            clip_id: ids2.clip_a_in,
            muted: true,
        },
        ProjectOp::RemoveClip {
            clip_id: ids2.clip_a_in,
        },
        ProjectOp::AddClip {
            lane_id: ids2.lane_a2,
            source_id: ids2.source,
            placement: musical_placement(BEAT),
            playback: clip_playback(),
        },
    ] {
        let err = h2.try_edit(f2.variant_set, f2.candidate, op).unwrap_err();
        assert!(
            matches!(err, VariantError::Unsupported { .. }),
            "expected Unsupported under notes-only mask, got {err:?}"
        );
    }
    // The candidate carries a scope-object removal (the deleted
    // note) — adopt defers typed like every removal candidate this
    // slice; live stays untouched.
    let live_before = h2.controller.live().clone();
    let err = h2
        .try_adopt(f2.variant_set, f2.candidate, &notes_scope)
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));
    assert_eq!(*h2.controller.live(), live_before);

    // A pure-addition notes-only candidate adopts its note edit in
    // one CAS — additions are not removals (fresh controller: the
    // identical scope would conflict with f2's set).
    let (c3, ids3) = arrangement_fixture();
    let mut h3 = Arrangement::new(c3);
    let notes_scope3 = scope_of(&ids3, mask(false, true));
    let f3 = h3.fork(notes_scope3.clone());
    h3.edit(
        f3.variant_set,
        f3.candidate,
        ProjectOp::AddNote {
            clip: ids3.clip_b_in,
            key: 66,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    h3.adopt(f3.variant_set, f3.candidate, &notes_scope3);
    assert_eq!(h3.controller.live().notes.len(), 3);
}

// ===========================================================================
// CB-ARR-09: move-out-of-range and sweep containment — typed rejects,
// never silent re-scope or out-of-window deletion
// ===========================================================================

#[test]
fn cb_arr_09_move_out_and_sweep_containment_reject_typed() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope);
    let (set, cand) = (forked.variant_set, forked.candidate);

    // MoveClip carrying a scoped clip fully out of `time`: the clip
    // still exists but leaves the owned set — an untracked live
    // mutation, rejected Unsupported (not a deletion).
    let err = h
        .try_edit(
            set,
            cand,
            ProjectOp::MoveClip {
                clip_id: ids.clip_a_in,
                anchor: TimeAnchor::Musical(MusicalTick::new(40 * BEAT)),
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "move-out-of-range must reject Unsupported, got {err:?}"
    );

    // MoveClip on the edge clip likewise leaves the window.
    let err = h
        .try_edit(
            set,
            cand,
            ProjectOp::MoveClip {
                clip_id: ids.clip_a_edge,
                anchor: TimeAnchor::Musical(MusicalTick::new(25 * BEAT)),
            },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));

    // SetClipExtent shrinking clip_a_in [0,7680) below the window
    // start (extent 500 → [0,500), no overlap with [1000,20000)):
    // same move-out rule through the extent channel.
    let err = h
        .try_edit(
            set,
            cand,
            ProjectOp::SetClipExtent {
                clip_id: ids.clip_a_in,
                extent: ClipExtent::Musical(MusicalDuration::new(500).unwrap()),
            },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));

    // AddClip placed outside `time` on a scoped lane: the created
    // entity is outside the owned set — ScopeViolation.
    let err = h
        .try_edit(
            set,
            cand,
            ProjectOp::AddClip {
                lane_id: ids.lane_a2,
                source_id: ids.source,
                placement: musical_placement(40 * BEAT),
                playback: clip_playback(),
            },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::ScopeViolation { .. }));

    // RemoveClipsInRange wider than `time` would delete out-of-scope
    // clips the object diff cannot see — containment is required.
    for (lanes, range, label) in [
        (
            vec![ids.lane_a1],
            MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(T1)).unwrap(),
            "range starts before scope",
        ),
        (
            vec![ids.lane_a1],
            MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(30 * BEAT)).unwrap(),
            "range ends after scope",
        ),
        (
            vec![ids.lane_a1, ids.lane_foreign],
            MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
            "foreign lane in sweep",
        ),
    ] {
        let err = h
            .try_edit(set, cand, ProjectOp::RemoveClipsInRange { lanes, range })
            .unwrap_err();
        assert!(
            matches!(err, VariantError::ScopeViolation { .. }),
            "{label}: expected ScopeViolation, got {err:?}"
        );
    }

    // A contained sweep deletes only in-range musical clips: the
    // absolute-anchored and out-of-range clips survive untouched.
    h.edit(
        set,
        cand,
        ProjectOp::RemoveClipsInRange {
            lanes: vec![ids.lane_a1],
            range: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
        },
    );
    let candidate = h.candidate(set, cand);
    assert!(!candidate.clips.contains_key(&ids.clip_a_in));
    assert!(!candidate.clips.contains_key(&ids.clip_a_edge));
    assert!(candidate.clips.contains_key(&ids.clip_a_out));
    assert!(candidate.clips.contains_key(&ids.clip_a_abs));
    assert_eq!(candidate.notes.len(), 1, "note cascade with clip");
    assert!(candidate.notes.contains_key(&ids.note_foreign));
    // clip_b_in on the other scoped track is untouched by a lane_a1
    // sweep.
    assert!(candidate.clips.contains_key(&ids.clip_b_in));
}

// ===========================================================================
// CB-ARR-10: SplitClip — both halves must stay scope-owned
// ===========================================================================

#[test]
fn cb_arr_10_split_clip_scope_membership() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope.clone());
    let (set, cand) = (forked.variant_set, forked.candidate);

    // In-range split of clip_a_in [0,7680) at tick 3840: both halves
    // overlap `time` — accepted, B is scope-owned.
    h.edit(
        set,
        cand,
        ProjectOp::SplitClip {
            clip_id: ids.clip_a_in,
            at: MusicalTick::new(4 * BEAT),
            clip_b_source_offset: SourceFrame::new(384_000).unwrap(),
        },
    );
    let candidate = h.candidate(set, cand);
    assert_eq!(candidate.clips.len(), 7, "split adds one clip");
    let split_b = candidate
        .clips
        .values()
        .filter(|clip| {
            clip.lane_id == ids.lane_a1
                && clip.id != ids.clip_a_in
                && matches!(clip.placement.anchor, TimeAnchor::Musical(t) if t.value() == 4 * BEAT)
        })
        .map(|clip| clip.id)
        .next()
        .unwrap();
    // The seeded note stayed on A (start 0 < split point); the
    // foreign-track note is untouched shared context.
    assert!(candidate.notes.contains_key(&ids.note_in));
    assert_eq!(candidate.notes[&ids.note_in].clip, ids.clip_a_in);
    assert_eq!(candidate.notes[&ids.note_foreign].clip, ids.clip_foreign);

    // Split whose kept half leaves the range: clip_a_edge
    // [15000,22680) split at 21000 → A=[15000,21000) still overlaps,
    // B=[21000,22680) does not — creation outside scope rejects.
    let err = h
        .try_edit(
            set,
            cand,
            ProjectOp::SplitClip {
                clip_id: ids.clip_a_edge,
                at: MusicalTick::new(21_000),
                clip_b_source_offset: SourceFrame::new(500_000).unwrap(),
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::ScopeViolation { .. }),
        "split-half outside scope must reject ScopeViolation, got {err:?}"
    );

    // Adopt the successful split — the new clip materializes under
    // its minted id.
    h.adopt(set, cand, &scope);
    assert!(h.controller.live().clips.contains_key(&split_b));
    let live_a = h.controller.live().clips[&ids.clip_a_in].clone();
    assert_eq!(
        live_a.placement.extent,
        ClipExtent::Musical(MusicalDuration::new(4 * BEAT).unwrap())
    );
}

// ===========================================================================
// CB-ARR-11: deletion candidates are editable/diffable but adopt +
// render-resolution defer typed — the shared no-removal invariant is
// slice-wide, never silently bypassed
// ===========================================================================

#[test]
fn cb_arr_11_delete_candidate_editable_but_adopt_defers_typed() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);
    let forked = h.fork(scope.clone());
    let (set, cand) = (forked.variant_set, forked.candidate);

    // In-range deletion IS an accepted candidate edit (CB-ARR-01
    // surface): RemoveNote + RemoveClip (whose note cascade then has
    // nothing left to delete).
    h.edit(set, cand, ProjectOp::RemoveNote { note: ids.note_in });
    h.edit(
        set,
        cand,
        ProjectOp::RemoveClip {
            clip_id: ids.clip_a_in,
        },
    );
    let candidate = h.candidate(set, cand);
    assert!(!candidate.clips.contains_key(&ids.clip_a_in));
    assert_eq!(candidate.notes.len(), 1, "only the foreign note remains");
    assert!(candidate.notes.contains_key(&ids.note_foreign));

    // Adopt materialization of scope removals is deferred slice-wide:
    // the shared planner rejects typed Unsupported before any
    // mutation — live, revision and branch state all unchanged.
    let live_before = h.controller.live().clone();
    let rev_before = h.controller.live_revision();
    let err = h.try_adopt(set, cand, &scope).unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "removal-candidate adopt must reject typed, got {err:?}"
    );
    assert_eq!(*h.controller.live(), live_before);
    assert_eq!(h.controller.live_revision(), rev_before);

    // Render resolution of the same candidate fails the same typed
    // gate — a delete candidate is never silently half-materialized.
    let err = resolve_render_project(
        h.controller.live(),
        Some(RenderBranchTarget {
            variant_set: set,
            branch: cand,
            store: &h.store,
        }),
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            synvelis_project::render_input::RenderInputError::Unsupported { .. }
        ),
        "render resolution must reject typed, got {err:?}"
    );
}

// ===========================================================================
// CB-ARR-12: sibling isolation — disjoint track scopes fork and edit
// independently under one controller
// ===========================================================================

#[test]
fn cb_arr_12_disjoint_track_scopes_edit_independently() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope_a = VariantScope::Arrangement {
        tracks: vec![ids.track_a],
        time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
        fields: mask(true, true),
    };
    let scope_b = VariantScope::Arrangement {
        tracks: vec![ids.track_b],
        time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
        fields: mask(true, true),
    };
    let fa = h.fork(scope_a.clone());
    let fb = h.fork(scope_b.clone());

    h.edit(
        fa.variant_set,
        fa.candidate,
        ProjectOp::SetClipMuted {
            clip_id: ids.clip_a_in,
            muted: true,
        },
    );
    h.edit(
        fb.variant_set,
        fb.candidate,
        ProjectOp::MoveClip {
            clip_id: ids.clip_b_in,
            anchor: TimeAnchor::Musical(MusicalTick::new(BEAT)),
        },
    );

    // A's candidate never sees B's edit and vice versa.
    let ca = h.candidate(fa.variant_set, fa.candidate);
    let cb = h.candidate(fb.variant_set, fb.candidate);
    assert!(ca.clips[&ids.clip_a_in].muted);
    assert_eq!(
        ca.clips[&ids.clip_b_in].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(10 * BEAT)),
        "A candidate keeps B's clip at origin"
    );
    assert_eq!(
        cb.clips[&ids.clip_b_in].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(BEAT))
    );
    assert!(!cb.clips[&ids.clip_a_in].muted);

    // Each adopts independently — disjoint scopes never conflict.
    h.adopt(fa.variant_set, fa.candidate, &scope_a);
    assert!(h.controller.live().clips[&ids.clip_a_in].muted);
    assert_eq!(
        h.controller.live().clips[&ids.clip_b_in].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(10 * BEAT)),
        "A adopt must not touch B's track"
    );
    h.adopt(fb.variant_set, fb.candidate, &scope_b);
    assert_eq!(
        h.controller.live().clips[&ids.clip_b_in].placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(BEAT))
    );
}

// ===========================================================================
// Scope-conflict + fork validation: overlapping arrangement scopes and
// malformed scopes reject typed at fork
// ===========================================================================

#[test]
fn cb_arr_fork_scope_conflict_and_invalid_scope_reject() {
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let scope = all_scope(&ids);

    // Malformed scopes first — no variant sets exist yet, so the
    // structure validator (not the conflict oracle) owns the verdict:
    // empty tracks, duplicate tracks, empty range, empty mask,
    // unknown track — all typed project errors.
    for bad in [
        VariantScope::Arrangement {
            tracks: vec![],
            time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
            fields: mask(true, true),
        },
        VariantScope::Arrangement {
            tracks: vec![ids.track_a, ids.track_a],
            time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
            fields: mask(true, true),
        },
        VariantScope::Arrangement {
            tracks: vec![ids.track_a],
            time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T0)).unwrap(),
            fields: mask(true, true),
        },
        VariantScope::Arrangement {
            tracks: vec![ids.track_a],
            time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
            fields: mask(false, false),
        },
        VariantScope::Arrangement {
            tracks: vec![TrackId::new(999_999)],
            time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
            fields: mask(true, true),
        },
    ] {
        let err = h.try_fork(bad).unwrap_err();
        assert!(
            matches!(err, VariantError::Project(_)),
            "malformed scope must reject Project, got {err:?}"
        );
    }

    h.fork(scope.clone());

    // A second arrangement scope overlapping the first (shared track
    // + overlapping window + overlapping clips) conflicts typed.
    let overlap = VariantScope::Arrangement {
        tracks: vec![ids.track_a],
        time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
        fields: mask(true, false),
    };
    let err = h.try_fork(overlap).unwrap_err();
    assert!(
        matches!(
            err,
            VariantError::ScopeConflict { .. } | VariantError::Project(_)
        ),
        "overlapping arrangement scope must reject, got {err:?}"
    );

    // Adapter scope-kind guard: a non-arrangement scope is InvalidScope.
    let err = h
        .adapter
        .validate_scope(
            h.controller.live(),
            &VariantScope::AudioClip {
                clip: ids.clip_a_in,
            },
        )
        .unwrap_err();
    assert!(matches!(err, VariantError::InvalidScope { .. }));
}

// ===========================================================================
// Review hardening: split note-migration mask gate, PPQN scope re-projection,
// adapter↔scope binding and boundary structural validation
// ===========================================================================

#[test]
fn cb_arr_13_split_clip_notes_mask_gate() {
    // A clip-local note past the split point makes SplitClip repartition
    // note parentage — under a clips-only mask those notes are not
    // scope-owned, so the migration must reject typed rather than
    // diverge candidate from adopt.
    let (mut c, ids) = arrangement_fixture();
    apply_op(
        &mut c,
        ProjectOp::AddNote {
            clip: ids.clip_a_in,
            key: 62,
            start: BeatPoint::new(6, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let mut h = Arrangement::new(c);
    let clips_scope = scope_of(&ids, mask(true, false));
    let forked = h.fork(clips_scope);
    let (set, cand) = (forked.variant_set, forked.candidate);

    // Split at beat 5 (tick 4800 at ppqn 960): the beat-6 note migrates
    // to clip B — clips-only mask rejects Unsupported.
    let err = h
        .try_edit(
            set,
            cand,
            ProjectOp::SplitClip {
                clip_id: ids.clip_a_in,
                at: MusicalTick::new(5 * BEAT),
                clip_b_source_offset: SourceFrame::new(500_000).unwrap(),
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::Unsupported { .. }),
        "migrating split under clips-only must reject Unsupported, got {err:?}"
    );

    // Split at beat 7: both notes stay on the kept clip — accepted.
    h.edit(
        set,
        cand,
        ProjectOp::SplitClip {
            clip_id: ids.clip_a_in,
            at: MusicalTick::new(7 * BEAT),
            clip_b_source_offset: SourceFrame::new(500_000).unwrap(),
        },
    );
    let candidate = h.candidate(set, cand);
    let lane_clips = candidate
        .clips
        .values()
        .filter(|clip| clip.lane_id == ids.lane_a1)
        .count();
    assert_eq!(lane_clips, 5, "split adds exactly one clip on lane_a1");

    // Under the full mask the same migrating split is expressible —
    // the notes are scope-owned and adopt materializes the
    // re-parenting with no divergence.
    let (mut c2, ids2) = arrangement_fixture();
    apply_op(
        &mut c2,
        ProjectOp::AddNote {
            clip: ids2.clip_a_in,
            key: 62,
            start: BeatPoint::new(6, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let mut h2 = Arrangement::new(c2);
    let full_scope = all_scope(&ids2);
    let f2 = h2.fork(full_scope.clone());
    h2.edit(
        f2.variant_set,
        f2.candidate,
        ProjectOp::SplitClip {
            clip_id: ids2.clip_a_in,
            at: MusicalTick::new(5 * BEAT),
            clip_b_source_offset: SourceFrame::new(500_000).unwrap(),
        },
    );
    h2.adopt(f2.variant_set, f2.candidate, &full_scope);
    let split_b = h2
        .controller
        .live()
        .clips
        .values()
        .find(|clip| {
            clip.lane_id == ids2.lane_a1
                && matches!(
                    clip.placement.anchor,
                    TimeAnchor::Musical(t) if t.value() == 5 * BEAT
                )
        })
        .map(|clip| clip.id)
        .unwrap();
    let migrated = h2
        .controller
        .live()
        .notes
        .values()
        .find(|note| note.key == 62)
        .unwrap();
    assert_eq!(migrated.clip, split_b, "adopted split re-parents the note");
}

#[test]
fn cb_arr_14_ppqn_reprojects_arrangement_scope_time() {
    // `SetProjectPpqn` lifts every persisted musical-tick value —
    // the arrangement scope's `time` window is one of them. Leaving
    // it on the old grid would silently retarget the declared
    // selection while clips convert underneath it.
    let (c, ids) = arrangement_fixture();
    let mut h = Arrangement::new(c);
    let forked = h.fork(all_scope(&ids));
    let set = forked.variant_set;

    apply_op(
        &mut h.controller,
        ProjectOp::SetProjectPpqn {
            old_ppqn: 960,
            new_ppqn: 480,
            policy: synvelis_project::PpqnConversionPolicy::Exact,
        },
    );

    let stored = &h.controller.live().variant_sets[&set].scope;
    let VariantScope::Arrangement { time, .. } = stored else {
        panic!("expected Arrangement scope, got {stored:?}");
    };
    assert_eq!(time.start().value(), T0 / 2);
    assert_eq!(time.end().value(), T1 / 2);

    // Clip anchors converted on the same grid — ownership derivation
    // still classifies the same boundary evidence.
    let clip = &h.controller.live().clips[&ids.clip_a_in];
    assert!(matches!(
        clip.placement.anchor,
        TimeAnchor::Musical(t) if t.value() == 0
    ));
    let deps = ArrangementAdapter::boundary_dependencies(h.controller.live(), stored).unwrap();
    let by_clip: BTreeSet<ClipId> = deps.iter().map(|d| d.clip).collect();
    assert_eq!(
        by_clip,
        BTreeSet::from([ids.clip_a_out, ids.clip_a_abs]),
        "boundary inventory is stable across the PPQN re-projection"
    );
}

#[test]
fn cb_arr_15_adapter_scope_binding_and_boundary_validation() {
    let (c, ids) = arrangement_fixture();
    let h = Arrangement::new(c);

    // The engine pairs adapter and scope independently: the
    // arrangement removal policy must never run against a scope this
    // adapter does not own — a mismatched edit is InvalidScope, not
    // a silently permissive candidate edit.
    let foreign_scope = VariantScope::AudioClip {
        clip: ids.clip_a_in,
    };
    let mut candidate = h.adapter.fork_candidate(h.controller.live()).unwrap();
    let mut id_draws = CounterIdGenerator::new(1);
    let err = h
        .adapter
        .edit_candidate(
            &mut candidate,
            &foreign_scope,
            &BranchEditOp::Project(ProjectOp::RemoveNote { note: ids.note_in }),
            &mut id_draws,
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::InvalidScope { .. }),
        "mismatched adapter/scope must reject InvalidScope, got {err:?}"
    );

    // boundary_dependencies enforces the same structural contract as
    // validate_scope — a scope naming a deleted track fails typed
    // instead of degrading to a silently partial report.
    let bad_scope = VariantScope::Arrangement {
        tracks: vec![TrackId::new(999_999)],
        time: MusicalRange::try_new(MusicalTick::new(T0), MusicalTick::new(T1)).unwrap(),
        fields: mask(true, true),
    };
    let err =
        ArrangementAdapter::boundary_dependencies(h.controller.live(), &bad_scope).unwrap_err();
    assert!(
        matches!(err, synvelis_project::Error::NotFound(_)),
        "malformed boundary scope must reject NotFound, got {err:?}"
    );
}
