//! VCS-06 (issue #97): monitoring-domain fixture rows — monitor
//! declaration diff classification (post-#318 `ObjectKind::
//! TrackMonitorDecl`/`PunchMarks`, `ComparisonDomain::Recording`),
//! the measured `apply_selected` fail-closed result for monitoring
//! kinds, and `project.monitoring` verbatim restore round-trip.
//!
//! Same headless contract as the other `vcs06_*` matrices: real
//! GUI/plugin/consumer lanes remain `NotRun`
//! (see `evidence/reports/vcs-06-fixture-matrix-ledger.md`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use synvelis_project::apply::{
    ApplyApproval, ApplyChangeIntent, ApplyError, ApplyJournal, ApplyJournalState, ApplySources,
    ChangeSetSource, InMemoryApplyJournal, ResolvedChangeSet, apply_selected, target_fingerprint,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    AmbiguityReason, ChangeKind, ChangeSet, ComparisonCompleteness, ComparisonDomain,
    ComparisonDomains, ComparisonEquality, ComparisonIssue, ComparisonSpec, ObjectKind,
    SemanticChange, SemanticTimeRange, compare_snapshots,
};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::monitoring_data::{
    ArmState, MonitorChannelMap, MonitorState, PunchMarks, TrackMonitorDecl,
};
use synvelis_project::ops::ProjectOp;
use synvelis_project::project::Project;
use synvelis_project::restore::{
    MissingAssetsPolicy, RestoreBlobSource, RestoreOutcome, RestorePorts, RestoreRequest,
    restore_as_copy,
};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, BlobProbe, BlobRead, ContentDigest, FixedSnapshotLocator,
    ImmutableSnapshot, ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness,
    SnapshotIdentity, SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::{SCHEMA_VERSION, SnapshotEnvelope};
use synvelis_project::time::MusicalTick;

// ── helpers (mirrors vcs06_* matrix files) ───────────────────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "fixture"),
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

fn compare_with(
    before: &ImmutableSnapshot,
    after: &ImmutableSnapshot,
    domains: &[ComparisonDomain],
) -> ChangeSet {
    let mut spec = spec_for(before, after);
    spec.domains = ComparisonDomains(domains.iter().copied().collect::<BTreeSet<_>>());
    compare_snapshots(before, after, &spec, &SnapshotCancel::new()).unwrap()
}

/// Manual snapshot over a caller-mutated `Project` (mirrors
/// `vcs02_diff.rs`) — used to reach identity states a live
/// `ProjectController` cannot produce, e.g. a foreign `project_id`.
fn snap_manual(project: Project) -> ImmutableSnapshot {
    let bytes = serde_json::to_vec(&project).unwrap();
    ImmutableSnapshot {
        identity: SnapshotIdentity {
            project_id: project.project_id,
            source: SnapshotOrigin::Live,
            locator: FixedSnapshotLocator::Live {
                project_id: project.project_id,
                live_revision: ProjectRevision(0),
            },
            digest: ContentDigest::sha256(&bytes),
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

fn monitor_decl() -> TrackMonitorDecl {
    TrackMonitorDecl {
        lane: None,
        channel_map: MonitorChannelMap::identity(),
        arm: ArmState::On,
        monitor: MonitorState::Auto,
    }
}

struct Src {
    resolved: ResolvedChangeSet,
}

impl ChangeSetSource for Src {
    fn resolve(&self, id: &ContentDigest) -> Option<ResolvedChangeSet> {
        (id == &self.resolved.changeset.key.digest()).then(|| self.resolved.clone())
    }
}

fn sources_for(cs: &ChangeSet, before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> Src {
    Src {
        resolved: ResolvedChangeSet {
            changeset: cs.clone(),
            before: before.clone(),
            after: after.clone(),
        },
    }
}

/// Intent over an unsupported-kind selection: `target_fingerprint`
/// itself fails closed for monitoring kinds, so the expected
/// fingerprint is an unreachable placeholder — `apply_inner`
/// recomputes the fingerprint before comparing it and errors first.
fn unsupported_intent(
    c: &ProjectController,
    cs: &ChangeSet,
    after: &ImmutableSnapshot,
    change: &SemanticChange,
) -> ApplyChangeIntent {
    ApplyChangeIntent {
        change_set_id: cs.key.digest(),
        source_snapshot_id: after.identity.digest,
        target_object: change.object.clone(),
        expected_project_revision: c.live_revision(),
        selected_change_ids: vec![change.id.clone()],
        expected_target_fingerprint: ContentDigest::sha256(b"vcs06-monitor-unsupported"),
        approval: ApplyApproval {
            ticket: "user-approved".into(),
        },
    }
}

fn run_apply(
    c: &mut ProjectController,
    intent: &ApplyChangeIntent,
    src: &Src,
    journal: &mut dyn ApplyJournal,
) -> Result<synvelis_project::apply::ApplyReceipt, ApplyError> {
    let sources = ApplySources {
        changesets: src,
        assets: Some(&AllPresentProbe),
        plugins: None,
    };
    apply_selected(c, intent, &sources, journal, &SnapshotCancel::new())
}

fn journal_key(intent: &ApplyChangeIntent) -> String {
    let digest = ContentDigest::sha256(&serde_json::to_vec(intent).unwrap());
    format!("apply-{}", digest.to_string().replace(':', ""))
}

struct MapBlobs {
    map: BTreeMap<String, Vec<u8>>,
}

impl RestoreBlobSource for MapBlobs {
    fn read_blob(&self, digest: &str, _cancel: &SnapshotCancel) -> Result<BlobRead, String> {
        Ok(match self.map.get(digest) {
            Some(b) => BlobRead::Present(b.clone()),
            None => BlobRead::Absent,
        })
    }
    fn stat_blob(&self, digest: &str) -> Result<Option<u64>, String> {
        Ok(self.map.get(digest).map(|b| b.len() as u64))
    }
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-vcs06-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn request(snapshot: ImmutableSnapshot, output_dir: PathBuf, restore_id: &str) -> RestoreRequest {
    RestoreRequest {
        restore_id: restore_id.into(),
        snapshot,
        output_dir,
        new_project_id: ProjectId::new(777),
        missing_assets: MissingAssetsPolicy::Fail,
    }
}

// ── VCS-DIFF-09: TrackMonitorDecl typed changes ──────────────────

#[test]
fn track_monitor_decl_add_modify_clear_are_typed_changes() {
    assert_eq!(
        ObjectKind::TrackMonitorDecl.domain(),
        ComparisonDomain::Recording
    );
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let before = snap_live(&c);

    // Declaration upsert → Added, keyed on the borrowed TrackId.
    apply_op(
        &mut c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: monitor_decl(),
        },
    );
    let declared = snap_live(&c);
    let ab = compare(&before, &declared);
    let hits = changes_of(&ab, ObjectKind::TrackMonitorDecl);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Added);
    assert!(hits[0].before.is_none());
    assert_eq!(hits[0].object.stable_id, track.to_tagged());
    // The declaration is not double-reported on its owner track.
    assert_eq!(changes_of(&ab, ObjectKind::Track).len(), 0);
    assert_eq!(ab.overall, ComparisonEquality::Different);

    // Field edits on the same declaration → Modified with evidence.
    let mut next = monitor_decl();
    next.arm = ArmState::Off;
    next.monitor = MonitorState::On;
    next.channel_map = MonitorChannelMap {
        channels: vec![0, 1, 2, 3],
    };
    apply_op(&mut c, ProjectOp::SetTrackMonitor { track, decl: next });
    let edited = snap_live(&c);
    let bc = compare(&declared, &edited);
    let hits = changes_of(&bc, ObjectKind::TrackMonitorDecl);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    assert_eq!(hits[0].object.stable_id, track.to_tagged());
    let fields = fields_of(hits[0]);
    assert!(fields.contains(&"arm"), "{fields:?}");
    assert!(fields.contains(&"monitor"), "{fields:?}");
    assert!(
        fields.iter().any(|f| f.starts_with("channel_map")),
        "{fields:?}"
    );

    // Clear → Removed.
    apply_op(&mut c, ProjectOp::ClearTrackMonitor { track });
    let cleared = snap_live(&c);
    let cd = compare(&edited, &cleared);
    let hits = changes_of(&cd, ObjectKind::TrackMonitorDecl);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Removed);
    assert_eq!(hits[0].object.stable_id, track.to_tagged());
    assert!(hits[0].after.is_none());

    // Identical monitoring data compares Equal — AA is a real
    // compare that emits no changes, not a skipped diff.
    let aa = compare(&edited, &edited);
    assert_eq!(aa.overall, ComparisonEquality::Equal);
    assert_eq!(aa.changes.len(), 0);

    // Excluding the Recording domain hides monitor-only
    // differences: the same pair reports Equal.
    let filtered = compare_with(&edited, &cleared, &[ComparisonDomain::Project]);
    assert_eq!(filtered.overall, ComparisonEquality::Equal);
    assert_eq!(filtered.changes.len(), 0);
    let kept = compare_with(&edited, &cleared, &[ComparisonDomain::Recording]);
    assert_eq!(kept.overall, ComparisonEquality::Different);
    assert_eq!(changes_of(&kept, ObjectKind::TrackMonitorDecl).len(), 1);

    // Cross-project snapshots stay Unknown — never a fabricated
    // diff over monitoring state.
    let mut foreign = c.live().clone();
    foreign.project_id = ProjectId::new(2);
    let foreign_snap = snap_manual(foreign);
    let cross = compare(&cleared, &foreign_snap);
    assert!(matches!(cross.overall, ComparisonEquality::Unknown(_)));
    assert!(cross.unresolved.iter().any(|i| matches!(
        i,
        ComparisonIssue::AmbiguousIdentity {
            reason: AmbiguityReason::CrossProject,
            ..
        }
    )));
}

// ── VCS-DIFF-10: PunchMarks declaration-presence changes ─────────

#[test]
fn punch_marks_presence_transitions_are_typed_changes() {
    assert_eq!(ObjectKind::PunchMarks.domain(), ComparisonDomain::Recording);
    let mut c = controller();
    let before = snap_live(&c);

    // Undeclared → declared = Added, carrying the declared [in, out).
    apply_op(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: Some(MusicalTick::new(1920)),
        },
    );
    let punched = snap_live(&c);
    let ab = compare(&before, &punched);
    let hits = changes_of(&ab, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Added);
    assert!(hits[0].before.is_none());
    let Some(SemanticTimeRange::Musical(range)) = hits[0].affected_range else {
        panic!("added punch declaration must report a musical range");
    };
    assert_eq!(range.start(), MusicalTick::new(960));
    assert_eq!(range.end(), MusicalTick::new(1920));

    // Declared → declared (mark edit) = Modified with field evidence.
    apply_op(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(480)),
            punch_out: Some(MusicalTick::new(1920)),
        },
    );
    let moved = snap_live(&c);
    let bc = compare(&punched, &moved);
    let hits = changes_of(&bc, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    let fields = fields_of(hits[0]);
    assert!(fields.contains(&"punch_in"), "{fields:?}");
    assert!(!fields.contains(&"punch_out"), "{fields:?}");

    // Declared → undeclared = Removed, still reporting the span the
    // removed declaration covered (in=480, out=1920 at `moved`).
    apply_op(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: None,
            punch_out: None,
        },
    );
    let cleared = snap_live(&c);
    let cd = compare(&moved, &cleared);
    let hits = changes_of(&cd, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Removed);
    assert!(hits[0].after.is_none());
    let Some(SemanticTimeRange::Musical(range)) = hits[0].affected_range else {
        panic!("removed punch declaration must report its covered range");
    };
    assert_eq!(range.start(), MusicalTick::new(480));
    assert_eq!(range.end(), MusicalTick::new(1920));

    // A partial declaration is still a typed presence change, but
    // no musical range is fabricated for it.
    apply_op(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: None,
        },
    );
    let partial = snap_live(&c);
    let de = compare(&cleared, &partial);
    let hits = changes_of(&de, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Added);
    assert!(
        hits[0].affected_range.is_none(),
        "partial declaration must not fabricate a range: {:?}",
        hits[0].affected_range
    );

    // An inverted declaration is likewise reported without a range.
    apply_op(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(1920)),
            punch_out: Some(MusicalTick::new(960)),
        },
    );
    let inverted = snap_live(&c);
    let ef = compare(&partial, &inverted);
    let hits = changes_of(&ef, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    assert!(
        hits[0].affected_range.is_none(),
        "inverted declaration must not fabricate a range: {:?}",
        hits[0].affected_range
    );
}

// ── VCS-APPLY-07: monitoring kinds fail apply closed ─────────────

#[test]
fn monitor_kind_selection_fails_apply_closed() {
    // before: one plain track. after: same track plus a monitor
    // declaration and punch marks — a `Complete`/`Different`
    // monitor-only changeset so `resolve_verified` reaches the
    // fingerprint CAS stage where the whitelist rejects.
    let mut before_c = controller();
    apply_op(&mut before_c, ProjectOp::AddTrack { name: "vox".into() });
    let before = snap_live(&before_c);

    let mut after_c = controller();
    apply_op(&mut after_c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *after_c.live().tracks.keys().next().unwrap();
    apply_op(
        &mut after_c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: monitor_decl(),
        },
    );
    apply_op(
        &mut after_c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: Some(MusicalTick::new(1920)),
        },
    );
    let after = snap_live(&after_c);

    let cs = compare(&before, &after);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    let decl_change = changes_of(&cs, ObjectKind::TrackMonitorDecl)[0];
    let punch_change = changes_of(&cs, ObjectKind::PunchMarks)[0];

    // `target_fingerprint` (the pub API callers use to build intents)
    // itself fails closed for both monitoring kinds — measured.
    let err =
        target_fingerprint(before_c.live(), std::slice::from_ref(&decl_change.object)).unwrap_err();
    assert_eq!(
        err,
        ApplyError::Unsupported {
            kind: ObjectKind::TrackMonitorDecl,
            change: ChangeKind::Modified,
            detail: "object kind outside the apply surface".into(),
        }
    );
    let err = target_fingerprint(before_c.live(), std::slice::from_ref(&punch_change.object))
        .unwrap_err();
    assert!(
        matches!(
            err,
            ApplyError::Unsupported {
                kind: ObjectKind::PunchMarks,
                ..
            }
        ),
        "{err:?}"
    );

    // `apply_selected` — same fail-closed at the fingerprint CAS
    // stage (before `plan_changes`, so `change` reports `Modified`
    // even for the `Added` selection). Live stays at the before
    // state and the journal records `Failed`.
    let src = sources_for(&cs, &before, &after);
    let rev_before = before_c.live_revision();

    let mut journal = InMemoryApplyJournal::new();
    let intent = unsupported_intent(&before_c, &cs, &after, decl_change);
    let err = run_apply(&mut before_c, &intent, &src, &mut journal).unwrap_err();
    assert_eq!(
        err,
        ApplyError::Unsupported {
            kind: ObjectKind::TrackMonitorDecl,
            change: ChangeKind::Modified,
            detail: "object kind outside the apply surface".into(),
        }
    );
    assert!(before_c.live().monitoring.tracks.is_empty());
    assert_eq!(before_c.live_revision(), rev_before);
    let record = journal.load(&journal_key(&intent)).unwrap().unwrap();
    assert!(
        matches!(record.state, ApplyJournalState::Failed { .. }),
        "{record:?}"
    );

    let mut journal = InMemoryApplyJournal::new();
    let intent = unsupported_intent(&before_c, &cs, &after, punch_change);
    let err = run_apply(&mut before_c, &intent, &src, &mut journal).unwrap_err();
    assert!(
        matches!(
            err,
            ApplyError::Unsupported {
                kind: ObjectKind::PunchMarks,
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(before_c.live().monitoring.punch, PunchMarks::default());
    assert_eq!(before_c.live_revision(), rev_before);
    let record = journal.load(&journal_key(&intent)).unwrap().unwrap();
    assert!(
        matches!(record.state, ApplyJournalState::Failed { .. }),
        "{record:?}"
    );
}

// ── VCS-RESTORE-07: monitoring verbatim round-trip ───────────────

#[test]
fn restore_as_copy_preserves_monitoring_verbatim() {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    // Fully non-default declaration incl. a lane binding.
    let decl = TrackMonitorDecl {
        lane: Some(lane),
        channel_map: MonitorChannelMap {
            channels: vec![2, 3],
        },
        arm: ArmState::On,
        monitor: MonitorState::On,
    };
    apply_op(
        &mut c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: decl.clone(),
        },
    );
    apply_op(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: Some(MusicalTick::new(1920)),
        },
    );
    let snap = snap_live(&c);

    let parent = tmp_dir("monitor");
    let dest = parent.join("copy-m");
    let blobs = MapBlobs {
        map: BTreeMap::default(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
    };
    let receipt = restore_as_copy(
        &request(snap, dest.clone(), "r-m"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(receipt.outcome, RestoreOutcome::Published);
    assert_eq!(receipt.source_project_id, ProjectId::new(1));
    assert_eq!(receipt.new_project_id, ProjectId::new(777));
    assert_ne!(receipt.new_project_id, receipt.source_project_id);
    assert!(receipt.effects.source.project_unchanged);
    assert!(receipt.effects.source.saved_head_unchanged);
    assert!(receipt.effects.git.index_untouched);
    assert!(receipt.effects.git.worktree_untouched);

    // Readback: the published snapshot envelope carries
    // `project.monitoring` verbatim (fork identity is the only diff).
    let head = std::fs::read_to_string(dest.join("HEAD")).unwrap();
    let snap_bytes = std::fs::read(dest.join("snapshots").join(head.trim())).unwrap();
    let envelope: SnapshotEnvelope = serde_json::from_slice(&snap_bytes).unwrap();
    assert_eq!(envelope.project_id, ProjectId::new(777));
    assert_ne!(envelope.project_id, c.live().project_id);
    assert_eq!(envelope.project.monitoring, c.live().monitoring);
    let restored = &envelope.project.monitoring.tracks[&track];
    assert_eq!(restored.lane, Some(lane));
    assert_eq!(restored.arm, ArmState::On);
    assert_eq!(restored.monitor, MonitorState::On);
    assert_eq!(restored.channel_map.channels(), &[2, 3]);
    assert_eq!(
        envelope.project.monitoring.punch.punch_in,
        Some(MusicalTick::new(960))
    );
    assert_eq!(
        envelope.project.monitoring.punch.punch_out,
        Some(MusicalTick::new(1920))
    );
    let _ = std::fs::remove_dir_all(&parent);
}
