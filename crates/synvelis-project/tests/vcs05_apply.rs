//! VCS-05 Safe Apply Selected Change — headless fixtures (synvelis#96).
//!
//! Covers the apply acceptance surface: selected-change apply with a
//! single undo step, approval/identity/revision/fingerprint CAS,
//! fail-closed unsupported/missing/plugin/port cases, idempotent
//! replay, and journal ACK-gap reconciliation. Real OS / provider /
//! GUI / engine E2E is `NotRun` (synvelis#97).

use synvelis_project::apply::{
    ApplyApproval, ApplyChangeIntent, ApplyError, ApplyJournal, ApplyJournalRecord,
    ApplyJournalState, ApplyOutcome, ApplyReceipt, ApplySources, ChangeSetSource,
    InMemoryApplyJournal, ResolvedChangeSet, apply_selected, target_fingerprint,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    ChangeId, ChangeKind, ChangeSet, ComparisonSpec, ObjectKind, SemanticChange, SemanticObjectRef,
    compare_snapshots,
};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::project::Project;
use synvelis_project::snapshot::{
    BlobProbe, BlobRead, ContentDigest, ImmutableSnapshot, SnapshotCancel, SnapshotResolver,
    SnapshotValidation,
};

// ── fixtures (mirrors vcs02_diff.rs conventions) ───────────────────

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

struct AbsentProbe;

impl BlobProbe for AbsentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Absent
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

fn base_project() -> ProjectController {
    let mut c = controller();
    apply_op(
        &mut c,
        ProjectOp::SetProjectName {
            name: "song".into(),
        },
    );
    apply_op(
        &mut c,
        ProjectOp::AddTrack {
            name: "drums".into(),
        },
    );
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    c
}

/// `(before_controller, before_snap, after_snap, changeset)` where the
/// after state adds one track named "vox".
fn added_track_pair() -> (
    ProjectController,
    ImmutableSnapshot,
    ImmutableSnapshot,
    ChangeSet,
) {
    let before_c = base_project();
    let before = snap_live(&before_c);

    let mut after_c = base_project();
    apply_op(&mut after_c, ProjectOp::AddTrack { name: "vox".into() });
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);
    (before_c, before, after, cs)
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

fn find_change(cs: &ChangeSet, kind: ObjectKind, ck: ChangeKind) -> &SemanticChange {
    cs.changes
        .iter()
        .find(|c| c.object.kind == kind && c.kind == ck)
        .expect("fixture change missing")
}

fn intent_for(
    c: &ProjectController,
    cs: &ChangeSet,
    after: &ImmutableSnapshot,
    changes: &[&SemanticChange],
) -> ApplyChangeIntent {
    let mut objects: Vec<SemanticObjectRef> = changes.iter().map(|ch| ch.object.clone()).collect();
    objects.push(changes[0].object.clone()); // target_object = first selected object
    let fp = target_fingerprint(c.live(), &objects).unwrap();
    ApplyChangeIntent {
        change_set_id: cs.key.digest(),
        source_snapshot_id: after.identity.digest,
        target_object: changes[0].object.clone(),
        expected_project_revision: c.live_revision(),
        selected_change_ids: changes.iter().map(|ch| ch.id.clone()).collect(),
        expected_target_fingerprint: fp,
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
) -> Result<ApplyReceipt, ApplyError> {
    let sources = ApplySources {
        changesets: src,
        assets: Some(&AllPresentProbe),
        plugins: None,
    };
    apply_selected(c, intent, &sources, journal, &SnapshotCancel::new())
}

// ── VCS-APPLY-01: selected apply + single undo ─────────────────────

#[test]
fn apply_added_track_commits_one_undo_step() {
    let (mut c, before, after, cs) = added_track_pair();
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let intent = intent_for(&c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let before_tracks = c.live().tracks.len();
    let receipt = run_apply(&mut c, &intent, &src, &mut journal).unwrap();

    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    assert_eq!(receipt.applied_change_ids, vec![change.id.clone()]);
    assert!(receipt.effects.live_applied);
    assert!(receipt.effects.single_undo_step);
    assert!(receipt.effects.untouched.saved_head_unchanged);
    assert!(receipt.effects.untouched.vcs_index_untouched);
    assert!(receipt.effects.untouched.git_worktree_untouched);
    assert!(!receipt.effects.engine_applied);
    assert_eq!(c.live().tracks.len(), before_tracks + 1);
    assert_eq!(receipt.after_revision.0, receipt.before_revision.0 + 1);

    // Exactly one undo step restores the prior state.
    assert!(c.undo().unwrap());
    assert_eq!(c.live().tracks.len(), before_tracks);
    assert!(c.redo().unwrap());
    assert_eq!(c.live().tracks.len(), before_tracks + 1);
}

#[test]
fn apply_project_rename_only_touches_name() {
    let mut before_c = base_project();
    let before = snap_live(&before_c);
    let mut after_c = base_project();
    apply_op(
        &mut after_c,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);
    let change = find_change(&cs, ObjectKind::Project, ChangeKind::Modified);

    let intent = intent_for(&before_c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();
    run_apply(&mut before_c, &intent, &src, &mut journal).unwrap();

    assert_eq!(before_c.live().name, "renamed");
    assert_eq!(before_c.live().tracks.len(), 1);
    assert_eq!(before_c.live().ppqn, after_c.live().ppqn);
}

// ── VCS-APPLY-02: CAS / approval / identity ────────────────────────

#[test]
fn unapproved_intent_fails_closed() {
    let (mut c, before, after, cs) = added_track_pair();
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let mut intent = intent_for(&c, &cs, &after, &[change]);
    intent.approval.ticket = String::new();
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let err = run_apply(&mut c, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::Unapproved));
    assert_eq!(c.live().tracks.len(), 1);
}

#[test]
fn stale_revision_fails_closed() {
    let (mut c, before, after, cs) = added_track_pair();
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let mut intent = intent_for(&c, &cs, &after, &[change]);
    intent.expected_project_revision = ProjectRevision(intent.expected_project_revision.0 + 99);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let err = run_apply(&mut c, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::StaleRevision { .. }));
    assert_eq!(c.live().tracks.len(), 1);
}

#[test]
fn fingerprint_drift_fails_closed() {
    // Project-rename pair: the fingerprinted object IS the project
    // singleton, so a post-capture rename is detected drift.
    let mut before_c = base_project();
    let before = snap_live(&before_c);
    let mut after_c = base_project();
    apply_op(
        &mut after_c,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);
    let change = find_change(&cs, ObjectKind::Project, ChangeKind::Modified);
    let intent = intent_for(&before_c, &cs, &after, &[change]);
    // Drift the live target after the intent was captured.
    apply_op(
        &mut before_c,
        ProjectOp::SetProjectName {
            name: "drifted".into(),
        },
    );
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let err = run_apply(&mut before_c, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::FingerprintMismatch { .. }));
    assert_eq!(before_c.live().name, "drifted");
}

#[test]
fn unknown_change_id_fails_closed() {
    let (mut c, before, after, cs) = added_track_pair();
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let mut intent = intent_for(&c, &cs, &after, &[change]);
    intent
        .selected_change_ids
        .push(ChangeId("bogus-change".into()));
    // Fingerprint must cover the bogus object? No — fingerprint covers
    // planned objects; the bogus id never plans, so recompute stays
    // valid. The UnknownChange check fires in plan_changes.
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let err = run_apply(&mut c, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::UnknownChange { .. }));
    assert_eq!(c.live().tracks.len(), 1);
}

#[test]
fn wrong_source_snapshot_identity_fails() {
    let (mut c, before, after, cs) = added_track_pair();
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let mut intent = intent_for(&c, &cs, &after, &[change]);
    intent.source_snapshot_id = before.identity.digest; // before, not after
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let err = run_apply(&mut c, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::ChangeSetMismatch { .. }));
    assert_eq!(c.live().tracks.len(), 1);
}

// ── VCS-APPLY-03: fail-closed unsupported / missing ────────────────

#[test]
fn removed_change_is_unsupported() {
    let mut before_c = base_project();
    let before = snap_live(&before_c);
    let lane = *before_c.live().lanes.keys().next().unwrap();
    let track = *before_c.live().tracks.keys().next().unwrap();
    apply_op(&mut before_c, ProjectOp::RemoveLane { lane_id: lane });
    apply_op(&mut before_c, ProjectOp::RemoveTrack { track_id: track });
    let after = snap_live(&before_c);
    // NOTE: before_c is now at `after`; rebuild a controller at `before`.
    let mut live_c = base_project();
    let cs = compare(&before, &after);
    let removed = cs
        .changes
        .iter()
        .find(|ch| ch.kind == ChangeKind::Removed)
        .expect("fixture needs a Removed change");
    let intent = intent_for(&live_c, &cs, &after, &[removed]);
    let src = Src {
        resolved: ResolvedChangeSet {
            changeset: cs.clone(),
            before: before.clone(),
            after: after.clone(),
        },
    };
    let mut journal = InMemoryApplyJournal::new();
    let err = run_apply(&mut live_c, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::Unsupported { .. }));
    assert_eq!(live_c.live().tracks.len(), 1);
}

#[test]
fn missing_asset_fails_closed() {
    // After-state registers a new asset; the blob probe reports Absent.
    let mut before_c = base_project();
    let before = snap_live(&before_c);
    let mut after_c = base_project();
    apply_op(
        &mut after_c,
        ProjectOp::RegisterAsset {
            digest: format!("sha256:{}", "b".repeat(64)),
        },
    );
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);
    let change = find_change(&cs, ObjectKind::Asset, ChangeKind::Added);
    let intent = intent_for(&before_c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AbsentProbe),
        plugins: None,
    };
    let err = apply_selected(
        &mut before_c,
        &intent,
        &sources,
        &mut journal,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        ApplyError::MissingAsset { .. } | ApplyError::AssetUnknown { .. }
    ));
    assert!(before_c.live().assets.is_empty());
}

// ── VCS-APPLY-04: idempotent replay / ACK gap ──────────────────────

#[test]
fn second_apply_replays_receipt_without_double_mutation() {
    let (mut c, before, after, cs) = added_track_pair();
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let intent = intent_for(&c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let first = run_apply(&mut c, &intent, &src, &mut journal).unwrap();
    assert_eq!(first.outcome, ApplyOutcome::Applied);
    let tracks_after_first = c.live().tracks.len();

    let second = run_apply(&mut c, &intent, &src, &mut journal).unwrap();
    assert_eq!(second.outcome, ApplyOutcome::Replayed);
    assert_eq!(second.applied_change_ids, first.applied_change_ids);
    assert_eq!(c.live().tracks.len(), tracks_after_first);
    // Still exactly one undo step back to the original state.
    assert!(c.undo().unwrap());
    assert_eq!(c.live().tracks.len(), tracks_after_first - 1);
}

/// Journal that persists `Pending` but drops terminal records —
/// simulates a crash/ACK gap after the mutation committed.
struct LossyJournal {
    inner: InMemoryApplyJournal,
}

impl ApplyJournal for LossyJournal {
    fn load(
        &mut self,
        key: &str,
    ) -> Result<Option<ApplyJournalRecord>, synvelis_project::apply::ApplyJournalError> {
        self.inner.load(key)
    }
    fn store(
        &mut self,
        record: ApplyJournalRecord,
    ) -> Result<(), synvelis_project::apply::ApplyJournalError> {
        match record.state {
            ApplyJournalState::Pending => self.inner.store(record),
            _ => Ok(()), // drop Applied/Failed — receipt "lost"
        }
    }
}

#[test]
fn ack_gap_reconciles_to_replayed() {
    let (mut c, before, after, cs) = added_track_pair();
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let intent = intent_for(&c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);

    let mut lossy = LossyJournal {
        inner: InMemoryApplyJournal::new(),
    };
    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AllPresentProbe),
        plugins: None,
    };
    // First attempt commits but the Applied receipt is dropped.
    let first = apply_selected(
        &mut c,
        &intent,
        &sources,
        &mut lossy,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(first.outcome, ApplyOutcome::Applied);
    let tracks = c.live().tracks.len();

    // Retry: journal still Pending; live already equals after-state →
    // reconciled Replayed, never a second mutation.
    let second = apply_selected(
        &mut c,
        &intent,
        &sources,
        &mut lossy,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(second.outcome, ApplyOutcome::Replayed);
    assert_eq!(c.live().tracks.len(), tracks);
}

#[test]
fn pending_without_commit_retries_normally() {
    // Seed a Pending journal entry for THIS intent's key while live is
    // still at the before-state (simulates a crash after Pending was
    // journaled but before the mutation committed). The retry must
    // proceed — not false-Replay, not false-fail.
    let (mut c, before, after, cs) = added_track_pair();
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let intent = intent_for(&c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut lossy = LossyJournal {
        inner: InMemoryApplyJournal::new(),
    };
    let intent_bytes = serde_json::to_vec(&intent).unwrap();
    let intent_digest = ContentDigest::sha256(&intent_bytes);
    let key = format!("apply-{}", intent_digest.to_string().replace(':', ""));
    lossy
        .inner
        .store(ApplyJournalRecord {
            key,
            intent_digest,
            state: ApplyJournalState::Pending,
        })
        .unwrap();

    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AllPresentProbe),
        plugins: None,
    };
    let receipt = apply_selected(
        &mut c,
        &intent,
        &sources,
        &mut lossy,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    assert_eq!(c.live().tracks.len(), 2);
}

// ── VCS-APPLY-05: partial/degraded snapshots fail closed ───────────

#[test]
fn degraded_endpoint_snapshot_fails_closed() {
    let (mut c, before, after, cs) = added_track_pair();
    // Craft a degraded before-endpoint: identity/digest binding is
    // unchanged, only the validation verdict degrades — the engine
    // must refuse to apply from unverified endpoints.
    let mut degraded_before = before.clone();
    degraded_before.validation = SnapshotValidation::Degraded;
    let change = find_change(&cs, ObjectKind::Track, ChangeKind::Added);
    let intent = intent_for(&c, &cs, &after, &[change]);
    let src = Src {
        resolved: ResolvedChangeSet {
            changeset: cs.clone(),
            before: degraded_before,
            after: after.clone(),
        },
    };
    let mut journal = InMemoryApplyJournal::new();
    let err = run_apply(&mut c, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::NotApplicable { .. }));
    assert_eq!(c.live().tracks.len(), 1);
}
