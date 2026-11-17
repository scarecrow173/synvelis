//! VCS-06 (issue #97): variant-set fixture rows — schema v14
//! `Project::variant_sets` (synvelis#278 Creative Branch). Since
//! synvelis#457 the variant model is a first-class VCS-02 compare
//! surface (`ObjectKind::VariantSet`/`VariantBranch`, `Variants`
//! domain, policy v4), so the diff rows assert typed changes; since
//! synvelis#458 the `restore_as_copy` row asserts verbatim
//! `variant_sets` preservation with source-project provenance
//! (VCS-RESTORE-10 `Pass`).
//!
//! Same headless contract as the other `vcs06_*` matrices: real
//! GUI/plugin/consumer lanes remain `NotRun`
//! (see `evidence/reports/vcs-06-fixture-matrix-ledger.md`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use synvelis_project::apply::ApplyApproval;
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    ChangeKind, ChangeSet, ComparisonCompleteness, ComparisonEquality, ComparisonSpec, ObjectKind,
    SemanticChange, compare_snapshots,
};
use synvelis_project::id::{BranchId, CounterIdGenerator, ProjectId, TrackId, VariantSetId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::project::Project;
use synvelis_project::range::MusicalRange;
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
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, ArrangementDomainMask, FakeDomainAdapter, ForkReceipt,
    InMemoryBranchContentStore, NoopRuntimePublish, VariantScope, adopt_branch, fork_variant_set,
    scope_digest,
};

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

/// Manual snapshot over a caller-mutated `Project` (mirrors
/// `vcs02_diff.rs`/`vcs06_plugin_instance_matrix.rs`) — used to reach
/// states a live `ProjectController` op sequence cannot produce,
/// e.g. a renamed track with no rename op.
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

fn fields_of(change: &SemanticChange) -> Vec<&str> {
    change
        .evidence
        .fields
        .iter()
        .map(|f| f.field.as_str())
        .collect()
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

// ── variant-set fixture helpers (mirrors cb01_variant.rs) ─────────

/// Minimal valid arrangement scope: one track, a wide musical
/// range, all field families.
fn arrangement_scope(track: TrackId) -> VariantScope {
    VariantScope::Arrangement {
        tracks: vec![track],
        time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(100_000)).unwrap(),
        fields: ArrangementDomainMask::ALL,
    }
}

fn fork_set(
    c: &mut ProjectController,
    store: &mut InMemoryBranchContentStore,
    scope: VariantScope,
) -> ForkReceipt {
    fork_variant_set(
        c,
        &FakeDomainAdapter::new(),
        store,
        &SnapshotResolver::new(),
        scope,
        &SnapshotCancel::new(),
    )
    .unwrap()
}

/// Adopts `branch` of `set` — exercises the full set-state mutation
/// surface (adopted pointer swap, frozen `Captured` branch ref,
/// `last_adopt` record, revision bump) through the real engine.
fn adopt_branch_of(
    c: &mut ProjectController,
    store: &mut InMemoryBranchContentStore,
    set: VariantSetId,
    branch: BranchId,
    scope: &VariantScope,
) {
    let intent = {
        let set_state = &c.live().variant_sets[&set];
        AdoptIntent {
            variant_set: set,
            branch,
            expected_project_revision: c.live_revision(),
            expected_branch_revision: set_state.branches[&branch].content.revision(),
            expected_scope_digest: scope_digest(c.live(), scope).unwrap(),
            approval: ApplyApproval {
                ticket: "approve".into(),
            },
        }
    };
    let sources = AdoptSources {
        assets: None,
        plugins: None,
        runtime: &NoopRuntimePublish,
    };
    adopt_branch(c, store, &sources, &intent).unwrap();
}

/// Track + forked variant set + adopted candidate — the richest
/// persisted `variant_sets` state reachable headlessly (Live
/// adopted branch, frozen Captured branch, `last_adopt` record).
fn variant_project() -> (
    ProjectController,
    InMemoryBranchContentStore,
    TrackId,
    VariantSetId,
    BranchId,
) {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let scope = arrangement_scope(track);
    let mut store = InMemoryBranchContentStore::default();
    let fork = fork_set(&mut c, &mut store, scope.clone());
    adopt_branch_of(&mut c, &mut store, fork.variant_set, fork.candidate, &scope);
    (c, store, track, fork.variant_set, fork.candidate)
}

// ── VCS-DIFF-11: variant_sets compare surface (synvelis#457) ─────
//
// `Project::variant_sets` (schema v14) diffs as `VariantSet`
// entities plus nested `VariantBranch` entities under the
// `Variants` domain — the gap this row pinned before #457 (silent
// `Equal`/`Complete`, silent drop in mixed diffs) is closed. The
// detailed kind/field matrix lives in `vcs07_variant_diff.rs`.

#[test]
fn variant_set_presence_and_mutation_are_typed_changes() {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let scope = arrangement_scope(track);
    let before = snap_live(&c);
    assert!(before.projection.project().variant_sets.is_empty());

    // Fork: the only authored-state delta is `variant_sets` gaining
    // one set (plus `id_watermark` allocator bookkeeping, which no
    // adapter reads either).
    let mut store = InMemoryBranchContentStore::default();
    let fork = fork_set(&mut c, &mut store, scope.clone());
    let forked = snap_live(&c);
    assert_eq!(forked.projection.project().variant_sets.len(), 1);

    // The persisted bytes genuinely differ — this is NOT the AA
    // fast path; the adapters ran and classified the insertion.
    assert_ne!(before.identity.digest, forked.identity.digest);
    assert_ne!(
        before.projection.project().variant_sets,
        forked.projection.project().variant_sets
    );

    // One `VariantSet` Added + two `VariantBranch` Added (adopted +
    // candidate) — variant-only differences are typed changes.
    let cs = compare(&before, &forked);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);
    assert_eq!(cs.unresolved.len(), 0);
    let set_adds: Vec<_> = cs
        .changes
        .iter()
        .filter(|ch| ch.object.kind == ObjectKind::VariantSet)
        .collect();
    assert_eq!(set_adds.len(), 1);
    assert_eq!(set_adds[0].kind, ChangeKind::Added);
    let branch_adds: Vec<_> = cs
        .changes
        .iter()
        .filter(|ch| ch.object.kind == ObjectKind::VariantBranch)
        .collect();
    assert_eq!(branch_adds.len(), 2);
    assert!(branch_adds.iter().all(|ch| ch.kind == ChangeKind::Added));

    // Adopt rewrites set metadata (adopted pointer, frozen Captured
    // branch ref, `last_adopt`, revision) — a `Modified` set change
    // plus `Replaced` branch content refs.
    adopt_branch_of(&mut c, &mut store, fork.variant_set, fork.candidate, &scope);
    let adopted = snap_live(&c);
    assert_ne!(forked.identity.digest, adopted.identity.digest);
    assert_ne!(
        forked.projection.project().variant_sets,
        adopted.projection.project().variant_sets
    );
    let cs = compare(&forked, &adopted);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    assert!(cs.changes.iter().any(|ch| {
        ch.object.kind == ObjectKind::VariantSet
            && ch.kind == ChangeKind::Modified
            && fields_of(ch).contains(&"adopted")
    }));
    assert!(cs.changes.iter().any(|ch| {
        ch.object.kind == ObjectKind::VariantBranch && ch.kind == ChangeKind::Replaced
    }));
}

#[test]
fn mixed_diff_reports_track_and_variant_changes() {
    let (c, _store, track, _set, _branch) = variant_project();
    // `before` predates the variant set: rebuild the pre-fork state
    // by snapshotting a clone with `variant_sets` cleared — the
    // only authored delta vs `after` is then variant state plus the
    // track rename.
    let mut before_project = c.live().clone();
    before_project.variant_sets.clear();
    let before = snap_manual(before_project, "var-before");

    let mut after_project = c.live().clone();
    after_project.tracks.get_mut(&track).unwrap().name = "renamed".into();
    let after = snap_manual(after_project, "var-after");

    // Both deltas are real and BOTH are reported: the Track
    // `Modified` plus the variant-set/branch insertions.
    assert_ne!(
        before.projection.project().variant_sets,
        after.projection.project().variant_sets
    );
    let cs = compare(&before, &after);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    assert!(cs.changes.iter().any(|ch| {
        ch.object.kind == ObjectKind::Track
            && ch.kind == ChangeKind::Modified
            && fields_of(ch).contains(&"name")
    }));
    assert!(
        cs.changes
            .iter()
            .any(|ch| ch.object.kind == ObjectKind::VariantSet)
    );
    assert!(
        cs.changes
            .iter()
            .any(|ch| ch.object.kind == ObjectKind::VariantBranch)
    );
}

// ── VCS-RESTORE-10: variant_sets restore-as-copy (synvelis#458) ────
//
// `fork_project` clones the whole `Project` and renumbers only the
// top-level `project_id`; nested `VariantSet::origin` /
// `Branch::created_from` `SnapshotIdentity`s keep the *source*
// project id verbatim — authored provenance, never rewritten
// (design `variant-sets-vcs-restore` D1–D3). The persisted/load
// oracle (`validate_persisted`) accepts cross-project provenance
// while the live `VariantOp::Fork` boundary keeps the same-project
// rule, so `restore_as_copy` publishes and the envelope readback
// proves `variant_sets` verbatim preservation.

#[test]
fn restore_as_copy_preserves_variant_sets_verbatim() {
    let (c, _store, _track, set, _branch) = variant_project();
    let snap = snap_live(&c);
    assert!(snap.projection.project().variant_sets.contains_key(&set));

    let parent = tmp_dir("varrestore");
    let dest = parent.join("copy-var");
    let blobs = MapBlobs {
        map: BTreeMap::default(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
        branch_content: None,
    };
    let rev_before = c.live_revision();

    let receipt = restore_as_copy(
        &request(snap, dest.clone(), "r-var"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap();

    // Published: the staged-envelope readback validation accepts the
    // forked project — variant provenance names the source project
    // verbatim and is not a same-project violation at the persisted
    // boundary (synvelis#458).
    assert_eq!(receipt.outcome, RestoreOutcome::Published);
    assert_eq!(receipt.new_project_id, ProjectId::new(777));
    assert_eq!(receipt.source_project_id, c.live().project_id);
    assert_eq!(receipt.stable_ids.counts.get("variant-set"), Some(&1));
    assert_eq!(receipt.stable_ids.counts.get("branch"), Some(&2));

    // Envelope readback: HEAD → snapshot → project.
    let head = std::fs::read_to_string(dest.join("HEAD")).unwrap();
    let snap_bytes = std::fs::read(dest.join("snapshots").join(head.trim())).unwrap();
    let envelope: SnapshotEnvelope = serde_json::from_slice(&snap_bytes).unwrap();
    assert_eq!(envelope.project_id, ProjectId::new(777));
    assert_eq!(envelope.project.project_id, ProjectId::new(777));

    // variant_sets verbatim: every issued VariantSetId/BranchId,
    // branch/set revision, BranchContentRef, last_adopt and the
    // origin/created_from provenance is preserved — only the
    // top-level project_id was renumbered.
    assert_eq!(
        envelope.project.variant_sets,
        c.live().variant_sets,
        "variant_sets must survive restore-as-copy verbatim"
    );
    let restored_set = &envelope.project.variant_sets[&set];
    let source_set = &c.live().variant_sets[&set];
    assert!(restored_set.last_adopt.is_some());
    assert_eq!(restored_set.last_adopt, source_set.last_adopt);
    // Provenance keeps naming the SOURCE project — preservation is
    // the design intent, not rewrite (synvelis#458).
    assert_eq!(restored_set.origin.project_id, c.live().project_id);
    for branch in restored_set.branches.values() {
        assert_eq!(branch.created_from.project_id, c.live().project_id);
    }
    // The published project passes the same load oracle `load`
    // enforces — the copy is openable, not just publishable.
    envelope.project.validate_references().unwrap();

    // Source invariance: restore consumed only the immutable
    // snapshot — the live controller and its variant set are
    // untouched.
    assert_eq!(c.live_revision(), rev_before);
    assert!(c.live().variant_sets.contains_key(&set));
    let _ = std::fs::remove_dir_all(&parent);
}
