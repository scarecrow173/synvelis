//! VCS-06 (issue #97): variant-set fixture rows — schema v14
//! `Project::variant_sets` (synvelis#278 Creative Branch) is a
//! persisted domain with **no** `ObjectKind` variant kind and no
//! `raw_kind_map` enumeration, so variant-only snapshot differences
//! are invisible to `compare_snapshots` (the observed `Equal`/
//! `Complete` result is pinned honestly — a known compare-surface
//! gap, never promoted to `Pass`). `restore_as_copy` measured
//! behavior for variant-set projects is recorded as observed.
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
    MissingAssetsPolicy, RestoreBlobSource, RestoreError, RestorePorts, RestoreRequest,
    restore_as_copy,
};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, BlobProbe, BlobRead, ContentDigest, FixedSnapshotLocator,
    ImmutableSnapshot, ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness,
    SnapshotIdentity, SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::SCHEMA_VERSION;
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

// ── VCS-DIFF-11: variant_sets compare-surface gap (honest pin) ───
//
// `Project::variant_sets` (schema v14) is persisted authored state
// but has no `ObjectKind` and is absent from `raw_kind_map` and
// every domain adapter's view. Snapshot pairs that differ *only*
// in variant state therefore observe `Equal`/`Complete` — the
// same misreport shape the plugin-instance gap had before
// `ObjectKind::PluginInstance` was added (synvelis#324). These
// tests pin the observed contract; they are evidence for the
// ledger's `Blocked` row, not a `Pass` claim.

#[test]
fn variant_set_presence_and_mutation_are_invisible_to_diff() {
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
    // fast path; the adapters ran and still found nothing.
    assert_ne!(before.identity.digest, forked.identity.digest);
    assert_ne!(
        before.projection.project().variant_sets,
        forked.projection.project().variant_sets
    );

    // KNOWN GAP — pinned observation, ledger `Blocked`: a real
    // persisted variant-set insertion reports `Equal`/`Complete`.
    let cs = compare(&before, &forked);
    assert_eq!(cs.overall, ComparisonEquality::Equal);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);
    assert_eq!(cs.changes.len(), 0);
    assert_eq!(cs.unresolved.len(), 0);

    // Adopt rewrites set metadata (adopted pointer, frozen Captured
    // branch ref, `last_adopt`, revision) — equally invisible.
    adopt_branch_of(&mut c, &mut store, fork.variant_set, fork.candidate, &scope);
    let adopted = snap_live(&c);
    assert_ne!(forked.identity.digest, adopted.identity.digest);
    assert_ne!(
        forked.projection.project().variant_sets,
        adopted.projection.project().variant_sets
    );
    let cs = compare(&forked, &adopted);
    assert_eq!(cs.overall, ComparisonEquality::Equal);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);
    assert_eq!(cs.changes.len(), 0);
    assert_eq!(cs.unresolved.len(), 0);
}

#[test]
fn mixed_diff_reports_track_change_but_drops_variant_set() {
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

    // Both deltas are real: variant_sets differ AND the track name
    // differs — yet the ChangeSet reports only the Track change.
    assert_ne!(
        before.projection.project().variant_sets,
        after.projection.project().variant_sets
    );
    let cs = compare(&before, &after);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    assert_eq!(cs.changes.len(), 1);
    assert_eq!(cs.changes[0].object.kind, ObjectKind::Track);
    assert_eq!(cs.changes[0].kind, ChangeKind::Modified);
    assert!(fields_of(&cs.changes[0]).contains(&"name"));
}

// ── VCS-RESTORE-10: variant_sets restore-as-copy (measured) ──────
//
// `fork_project` clones the whole `Project` and renumbers only the
// top-level `project_id`; nested `VariantSet::origin` /
// `Branch::created_from` `SnapshotIdentity`s keep the *source*
// project id verbatim. `validate_set_shape` then rejects the fork
// (`origin belongs to a different project`) inside
// `validate_staged`, so `restore_as_copy` fails closed with
// `RestoreError::Validation` — staging retained, destination
// absent. Recorded as measured `Fail` (capability gap), not Pass.

#[test]
fn restore_as_copy_fails_closed_on_variant_set_origin_mismatch() {
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
    };
    let rev_before = c.live_revision();
    let err = restore_as_copy(
        &request(snap, dest.clone(), "r-var"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();

    // Measured: staged-envelope readback validation rejects the
    // forked project because variant-set origin identities still
    // name the source project — fail-closed `Validation`, never a
    // partial publish.
    match &err {
        RestoreError::Validation { detail } => {
            assert!(
                detail.contains("origin belongs to a different project"),
                "unexpected validation detail: {detail}"
            );
        }
        other => panic!("expected RestoreError::Validation, got {other:?}"),
    }
    assert!(!dest.exists(), "failed restore must not publish");
    let staging = parent.join(".copy-var.restore-r-var");
    assert!(
        staging.exists(),
        "failed restore keeps staging as the recovery record"
    );
    // Source invariance: restore consumed only the immutable
    // snapshot — the live controller and its variant set are
    // untouched.
    assert!(c.live().variant_sets.contains_key(&set));
    assert_eq!(c.live_revision(), rev_before);
    let _ = std::fs::remove_dir_all(&parent);
}
