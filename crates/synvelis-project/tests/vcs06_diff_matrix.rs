//! VCS-06 (issue #97): semantic-diff fixture rows not covered by
//! `vcs02_diff.rs` — clip-local note propagation, lane ordering,
//! automation curve shape, asset addition, and MIDI note payload.
//!
//! Same headless contract as VCS-02: `Unknown` is never promoted to
//! `Equal`, identity stays typed, and real GUI/plugin/consumer lanes
//! remain `NotRun` (see `evidence/reports/vcs-06-fixture-matrix-ledger.md`).

use synvelis_project::control::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, ConflictPolicy,
    ControlOwner, Interpolation, ParamMapping, ParameterRole, ValueKind,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    ChangeKind, ChangeSet, ComparisonSpec, ObjectKind, SemanticChange, compare_snapshots,
};
use synvelis_project::id::{CounterIdGenerator, NoteId, ProjectId, TrackId};
use synvelis_project::midi::MidiDivision;
use synvelis_project::ops::ProjectOp;
use synvelis_project::placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor,
};
use synvelis_project::project::{Note, Project};
use synvelis_project::range::SourceRange;
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, BlobProbe, BlobRead, ContentDigest, FixedSnapshotLocator,
    ImmutableSnapshot, ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness,
    SnapshotIdentity, SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::time::{MidiTick, MusicalDuration, MusicalTick, SourceFrame};

// ── helpers (mirrors vcs02_diff.rs) ────────────────────────────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "fixture"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

struct AllPresentProbe;

impl BlobProbe for AllPresentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Present(vec![0])
    }
}

fn snap_live(c: &ProjectController) -> ImmutableSnapshot {
    SnapshotResolver::new()
        .capture_live(
            c,
            c.live_revision(),
            &SnapshotCancel::new(),
            Some(&AllPresentProbe),
        )
        .unwrap()
}

fn spec_for(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ComparisonSpec {
    ComparisonSpec::new(before.identity.clone(), after.identity.clone())
}

fn compare(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ChangeSet {
    compare_snapshots(
        before,
        after,
        &spec_for(before, after),
        &SnapshotCancel::new(),
    )
    .unwrap()
}

fn changes_of(cs: &ChangeSet, kind: ObjectKind) -> Vec<&SemanticChange> {
    cs.changes
        .iter()
        .filter(|c| c.object.kind == kind)
        .collect()
}

fn fields_of(change: &SemanticChange) -> Vec<&str> {
    change
        .evidence
        .fields
        .iter()
        .map(|f| f.field.as_str())
        .collect()
}

fn sha(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn snap_manual(project: Project, tag: &str) -> ImmutableSnapshot {
    let bytes = serde_json::to_vec(&project).unwrap();
    let mut material = tag.as_bytes().to_vec();
    material.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
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
            schema_version: SCHEMA_VERSION,
            projection_version: SCHEMA_VERSION,
            capture_generation: 0,
        },
        origin: SnapshotOrigin::Live,
        projection: ReadOnlyProjectProjection::new(SCHEMA_VERSION, project),
        validation: SnapshotValidation::Valid,
        assets: AssetAvailabilityManifest::default(),
        opaque_state: Vec::new(),
        completeness: SnapshotCompleteness::Complete,
    }
}

fn clip_placement() -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(0)),
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

/// Track + two lanes + asset/source + clip + one clip-local note.
fn note_project() -> (ProjectController, TrackId, NoteId) {
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "song".into(),
        },
    );
    apply(
        &mut c,
        ProjectOp::AddTrack {
            name: "drums".into(),
        },
    );
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    apply(&mut c, ProjectOp::RegisterAsset { digest: sha('a') });
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: sha('1'),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: clip_placement(),
            playback: clip_playback(),
        },
    );
    let clip = *c.live().clips.keys().next_back().unwrap();
    apply(
        &mut c,
        ProjectOp::AddNote {
            clip,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let note = *c.live().notes.keys().next_back().unwrap();
    (c, track, note)
}

// ── VCS-DIFF-03: note propagation ─────────────────────────────────

#[test]
fn clip_note_edit_and_addition_are_typed_changes() {
    let (c, _, note) = note_project();
    let before = snap_live(&c);

    let mut project = c.live().clone();
    // Edit the existing note's key and append a second note on the
    // same clip — both must surface as typed Note changes.
    project.notes.get_mut(&note).unwrap().key = 64;
    let clip = project.notes[&note].clip;
    project.notes.insert(
        NoteId::new(0xfeed),
        Note {
            id: NoteId::new(0xfeed),
            clip,
            key: 67,
            start: BeatPoint::new(1, 1).unwrap(),
            len: BeatPoint::new(1, 2).unwrap(),
        },
    );
    let after = snap_manual(project, "after");

    let cs = compare(&before, &after);
    let notes = changes_of(&cs, ObjectKind::Note);
    assert_eq!(notes.len(), 2, "expected Modified + Added note changes");
    assert!(notes.iter().any(|n| n.kind == ChangeKind::Modified));
    assert!(notes.iter().any(|n| n.kind == ChangeKind::Added));
    let edited = notes
        .iter()
        .find(|n| n.kind == ChangeKind::Modified)
        .unwrap();
    assert!(fields_of(edited).contains(&"key"));
}

// ── VCS-DIFF-04: lane ordering ────────────────────────────────────

#[test]
fn lane_order_change_is_reordered_not_modified() {
    let (c, track, _) = note_project();
    let before = snap_live(&c);

    let mut project = c.live().clone();
    let t = project.tracks.get_mut(&track).unwrap();
    assert_eq!(t.lane_ids.len(), 2);
    t.lane_ids.reverse();
    let after = snap_manual(project, "after");

    let cs = compare(&before, &after);
    let tracks = changes_of(&cs, ObjectKind::Track);
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].kind, ChangeKind::Reordered);
    // Same lane set — no Lane entity may appear as Added/Removed.
    assert_eq!(changes_of(&cs, ObjectKind::Lane).len(), 0);
}

// ── VCS-DIFF-06: automation curve shape ───────────────────────────

#[test]
fn automation_interpolation_change_is_modified() {
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let target = *c.live().control.targets.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddAutomationLane { target });
    let lane = *c.live().control.lanes.keys().next_back().unwrap();
    apply(
        &mut c,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Step,
                points: vec![AutomationPoint {
                    pos: BeatPoint::new(0, 1).unwrap(),
                    value: 0.5,
                }],
            },
            policy: ConflictPolicy::Reject,
        },
    );
    let before = snap_live(&c);

    let mut project = c.live().clone();
    project.control.lanes.get_mut(&lane).unwrap().regions[0].interpolation = Interpolation::Linear;
    let after = snap_manual(project, "after");

    let cs = compare(&before, &after);
    let hits = changes_of(&cs, ObjectKind::AutomationLane);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    assert!(fields_of(hits[0]).contains(&"regions"));
}

// ── VCS-DIFF-07: asset addition ───────────────────────────────────

#[test]
fn asset_added_is_typed_add_not_reorder() {
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "song".into(),
        },
    );
    let before = snap_live(&c);
    apply(&mut c, ProjectOp::RegisterAsset { digest: sha('b') });
    let after = snap_live(&c);

    let cs = compare(&before, &after);
    let assets = changes_of(&cs, ObjectKind::Asset);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, ChangeKind::Added);
}

// ── VCS-DIFF-08: MIDI note payload ────────────────────────────────

#[test]
fn midi_note_edit_and_addition_are_typed_changes() {
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let source = *c.live().midi_sources.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: MidiTick::new(0).unwrap(),
            len: MidiTick::new(120).unwrap(),
        },
    );
    let note = *c.live().midi_sources[&source].notes.keys().next().unwrap();
    let before = snap_live(&c);

    // Real ops on the live controller: velocity edit + a second note.
    apply(
        &mut c,
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: note,
            velocity: 90,
        },
    );
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 64,
            velocity: 80,
            start: MidiTick::new(240).unwrap(),
            len: MidiTick::new(60).unwrap(),
        },
    );
    let after = snap_live(&c);

    let cs = compare(&before, &after);
    let notes = changes_of(&cs, ObjectKind::MidiNote);
    assert_eq!(notes.len(), 2, "expected Modified + Added midi note");
    assert!(notes.iter().any(|n| n.kind == ChangeKind::Modified));
    assert!(notes.iter().any(|n| n.kind == ChangeKind::Added));
    let edited = notes
        .iter()
        .find(|n| n.kind == ChangeKind::Modified)
        .unwrap();
    assert!(fields_of(edited).contains(&"velocity"));
}
