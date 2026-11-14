//! VCS-07 (issue #457): `variant_sets` semantic-diff classification —
//! `ObjectKind::VariantSet`/`VariantBranch` under the `Variants`
//! domain (policy v4). Pins the compare-surface contract that
//! synvelis#97 measured as a gap: variant-only snapshot differences
//! are typed changes, never silent `Equal`/`Complete`, and the
//! apply surface stays fail-closed `Unsupported`.
//!
//! Same headless contract as the other `vcs06_*`/`vcs02_*` fixtures:
//! real GUI/plugin/consumer lanes remain `NotRun`
//! (see `evidence/reports/vcs-06-fixture-matrix-ledger.md` and
//! `evidence/sv457/`).

use std::collections::BTreeMap;

use synvelis_project::apply::{
    ApplyApproval, ApplyChangeIntent, ApplyError, ApplySources, ChangeSetSource,
    InMemoryApplyJournal, ResolvedChangeSet, apply_selected, target_fingerprint,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    AmbiguityReason, ChangeKind, ChangeSet, ComparisonCompleteness, ComparisonDomain,
    ComparisonDomains, ComparisonEquality, ComparisonIssue, ComparisonSpec, DIFF_POLICY_VERSION,
    ObjectKind, SemanticChange, compare_snapshots,
};
use synvelis_project::id::{BranchId, CounterIdGenerator, ProjectId, TrackId, VariantSetId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::project::Project;
use synvelis_project::range::MusicalRange;
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, BlobProbe, BlobRead, ContentDigest, FixedSnapshotLocator,
    ImmutableSnapshot, ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness,
    SnapshotIdentity, SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::time::MusicalTick;
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, ArrangementDomainMask, Branch, BranchContentRef, BranchLineage,
    BranchRevision, FakeDomainAdapter, ForkReceipt, InMemoryBranchContentStore, NoopRuntimePublish,
    VariantScope, VariantSet, VariantSetRevision, adopt_branch, fork_variant_set, scope_digest,
};

// ── helpers (mirrors vcs06_variant_matrix.rs / vcs05_apply.rs) ────

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

/// Manual snapshot over a caller-mutated `Project` — reaches states a
/// live op sequence cannot produce (cleared `variant_sets`, crafted
/// ID collisions, content-ref swaps).
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

fn pair(before: Project, after: Project) -> (ImmutableSnapshot, ImmutableSnapshot) {
    (snap_manual(before, "before"), snap_manual(after, "after"))
}

fn changes_of(cs: &ChangeSet, kind: ObjectKind) -> Vec<&SemanticChange> {
    cs.changes
        .iter()
        .filter(|c| c.object.kind == kind)
        .collect()
}

fn has_kind(cs: &ChangeSet, kind: ObjectKind, ck: ChangeKind) -> bool {
    cs.changes
        .iter()
        .any(|c| c.object.kind == kind && c.kind == ck)
}

fn fields_of(change: &SemanticChange) -> Vec<&str> {
    change
        .evidence
        .fields
        .iter()
        .map(|f| f.field.as_str())
        .collect()
}

// ── variant-set fixture helpers (mirrors vcs06_variant_matrix.rs) ──

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
/// persisted `variant_sets` state reachable headlessly.
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

// ── (a) set/branch presence: Added and Removed typed changes ──────

#[test]
fn variant_set_and_branch_presence_are_typed_changes() {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let scope = arrangement_scope(track);
    let before = snap_live(&c);
    assert!(before.projection.project().variant_sets.is_empty());

    let mut store = InMemoryBranchContentStore::default();
    fork_set(&mut c, &mut store, scope);
    let after = snap_live(&c);

    // Variant-only difference: 1 VariantSet Added + 2 VariantBranch
    // Added, stable `variant-set-`/`branch-` IDs, `Different`.
    let cs = compare(&before, &after);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);
    assert_eq!(cs.unresolved.len(), 0);
    assert_eq!(cs.changes.len(), 3);
    let sets = changes_of(&cs, ObjectKind::VariantSet);
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].kind, ChangeKind::Added);
    assert!(sets[0].object.stable_id.starts_with("variant-set-"));
    let branches = changes_of(&cs, ObjectKind::VariantBranch);
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().all(|ch| ch.kind == ChangeKind::Added));
    assert!(
        branches
            .iter()
            .all(|ch| ch.object.stable_id.starts_with("branch-"))
    );

    // Reverse direction: set + branches Removed.
    let mut cleared = c.live().clone();
    cleared.variant_sets.clear();
    let (rb, ra) = pair(c.live().clone(), cleared);
    let cs = compare(&rb, &ra);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    assert!(has_kind(&cs, ObjectKind::VariantSet, ChangeKind::Removed));
    let removed_branches = changes_of(&cs, ObjectKind::VariantBranch);
    assert_eq!(removed_branches.len(), 2);
    assert!(
        removed_branches
            .iter()
            .all(|ch| ch.kind == ChangeKind::Removed)
    );
}

// ── (b) adopt swap: set Modified + branch content Replaced ─────────

#[test]
fn adopt_swap_reports_set_modified_and_branch_replaced() {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let scope = arrangement_scope(track);
    let mut store = InMemoryBranchContentStore::default();
    let fork = fork_set(&mut c, &mut store, scope.clone());
    let before = snap_live(&c);

    adopt_branch_of(&mut c, &mut store, fork.variant_set, fork.candidate, &scope);
    let after = snap_live(&c);
    assert_ne!(before.identity.digest, after.identity.digest);

    let cs = compare(&before, &after);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    // The set reports the adopt bookkeeping as `Modified` with field
    // evidence on `adopted` (plus `revision`/`last_adopt`).
    let sets = changes_of(&cs, ObjectKind::VariantSet);
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].kind, ChangeKind::Modified);
    let fields = fields_of(sets[0]);
    assert!(fields.contains(&"adopted"));
    assert!(fields.contains(&"revision"));
    assert!(fields.contains(&"last_adopt"));
    // Both branches' `BranchContentRef`s swap (`Live` mirror ↔
    // frozen `Captured`) — content-identity `Replaced`.
    let branches = changes_of(&cs, ObjectKind::VariantBranch);
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().all(|ch| ch.kind == ChangeKind::Replaced));
    assert!(
        branches
            .iter()
            .all(|ch| fields_of(ch).iter().any(|f| f.starts_with("content.")))
    );
}

// ── (c) BranchContentRef swap → Replaced ───────────────────────────

#[test]
fn branch_content_ref_swap_is_replaced() {
    let (c, _store, _track, set, adopted) = variant_project();
    let mut after_p = c.live().clone();
    // Bump the adopted (Live) branch's content revision — a pure
    // `BranchContentRef` change with no set-level mutation.
    let branch = after_p
        .variant_sets
        .get_mut(&set)
        .unwrap()
        .branches
        .get_mut(&adopted)
        .unwrap();
    match &mut branch.content {
        BranchContentRef::Live { revision } => {
            *revision = BranchRevision::new(revision.raw() + 1);
        }
        other @ BranchContentRef::Captured { .. } => {
            panic!("expected Live content ref, got {other:?}")
        }
    }
    let (b, a) = pair(c.live().clone(), after_p);

    let cs = compare(&b, &a);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    let branches = changes_of(&cs, ObjectKind::VariantBranch);
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].kind, ChangeKind::Replaced);
    assert!(fields_of(branches[0]).contains(&"content.revision"));
    // The set itself is untouched — no `VariantSet` change.
    assert_eq!(changes_of(&cs, ObjectKind::VariantSet).len(), 0);
}

// ── (d) variant-only diff → Different at policy v4 ─────────────────

#[test]
fn variant_only_difference_is_different_at_policy_v4() {
    let (c, _store, _track, _set, _branch) = variant_project();
    let mut before_p = c.live().clone();
    before_p.variant_sets.clear();
    let (b, a) = pair(before_p, c.live().clone());

    let cs = compare(&b, &a);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    assert_eq!(cs.comparison.policy_version, DIFF_POLICY_VERSION);
    assert_eq!(cs.key.policy_version, DIFF_POLICY_VERSION);
    assert!(cs.key.domains.contains(&ComparisonDomain::Variants));
    assert_ne!(cs.changes.len(), 0);
    assert!(
        cs.changes
            .iter()
            .all(|ch| ch.object.kind.domain() == ComparisonDomain::Variants)
    );
}

// ── (e) cross-kind raw collision → AmbiguousIdentity ───────────────

/// Minimal persisted `VariantSet` for crafted collision fixtures —
/// the diff engine reads the projection verbatim and never
/// re-validates variant shape.
fn crafted_variant_set(project_id: ProjectId, raw_set: u128, raw_branch: u128) -> VariantSet {
    let identity = SnapshotIdentity {
        project_id,
        source: SnapshotOrigin::Live,
        locator: FixedSnapshotLocator::Live {
            project_id,
            live_revision: ProjectRevision(0),
        },
        digest: ContentDigest::sha256(b"crafted-origin"),
        schema_version: SCHEMA_VERSION,
        projection_version: SCHEMA_VERSION,
        capture_generation: 0,
    };
    let set_id = VariantSetId::new(raw_set);
    let branch_id = BranchId::new(raw_branch);
    VariantSet {
        id: set_id,
        scope: VariantScope::Arrangement {
            tracks: vec![TrackId::new(1)],
            time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(1)).unwrap(),
            fields: ArrangementDomainMask::ALL,
        },
        origin: identity.clone(),
        branches: BTreeMap::from([(
            branch_id,
            Branch {
                id: branch_id,
                parent: None,
                created_from: identity,
                content: BranchContentRef::Live {
                    revision: BranchRevision::new(0),
                },
                lineage: BranchLineage::default(),
            },
        )]),
        adopted: branch_id,
        revision: VariantSetRevision::new(1),
        origin_objects: Vec::new(),
        last_adopt: None,
    }
}

#[test]
fn variant_ids_colliding_with_entity_raws_are_ambiguous() {
    // before: variant set raw 77 + branch raw 78; after: tracks
    // claiming the same raws — `raw_kind_map` registers variant IDs
    // as entity-issued, so the cross-kind claim is ambiguous, never
    // `Equal` and never a fabricated change.
    let mut before = Project::new(ProjectId::new(1), "p");
    before.variant_sets.insert(
        VariantSetId::new(77),
        crafted_variant_set(ProjectId::new(1), 77, 78),
    );
    let mut after = Project::new(ProjectId::new(1), "p");
    for raw in [77u128, 78] {
        after.tracks.insert(
            TrackId::new(raw),
            synvelis_project::project::Track {
                id: TrackId::new(raw),
                name: format!("t{raw}"),
                lane_ids: vec![],
            },
        );
    }
    let (b, a) = pair(before, after);
    let cs = compare(&b, &a);

    // No entity may diff across the collision.
    assert_eq!(changes_of(&cs, ObjectKind::VariantSet).len(), 0);
    assert_eq!(changes_of(&cs, ObjectKind::VariantBranch).len(), 0);
    assert_eq!(changes_of(&cs, ObjectKind::Track).len(), 0);
    let cross_kind = cs
        .unresolved
        .iter()
        .filter(|i| {
            matches!(
                i,
                ComparisonIssue::AmbiguousIdentity {
                    reason: AmbiguityReason::CrossKindId,
                    ..
                }
            )
        })
        .count();
    // set(77) + branch(78) on the before side, tracks(77/78) after.
    assert_eq!(cross_kind, 4);
    assert!(matches!(cs.overall, ComparisonEquality::Unknown(_)));
}

// ── (f) domain exclusion gates the variant surface ─────────────────

#[test]
fn variants_domain_exclusion_hides_variant_only_difference() {
    let (c, _store, _track, _set, _branch) = variant_project();
    let mut before_p = c.live().clone();
    before_p.variant_sets.clear();
    let (b, a) = pair(before_p, c.live().clone());

    // `Variants` excluded → the variant-only delta is invisible again
    // (domain filter semantics, VCS-DIFF-09 precedent).
    let mut spec = spec_for(&b, &a);
    let mut domains = ComparisonDomains::all().0;
    domains.remove(&ComparisonDomain::Variants);
    spec.domains = ComparisonDomains(domains);
    let cs = compare_snapshots(&b, &a, &spec, &SnapshotCancel::new()).unwrap();
    assert_eq!(cs.overall, ComparisonEquality::Equal);
    assert_eq!(cs.changes.len(), 0);
    assert_eq!(cs.unresolved.len(), 0);

    // `Variants` included → `Different`.
    let cs = compare(&b, &a);
    assert_eq!(cs.overall, ComparisonEquality::Different);
    assert_ne!(cs.changes.len(), 0);
}

// ── (g) apply surface stays fail-closed Unsupported ────────────────

struct Src {
    resolved: ResolvedChangeSet,
}

impl ChangeSetSource for Src {
    fn resolve(&self, id: &ContentDigest) -> Option<ResolvedChangeSet> {
        (id == &self.resolved.changeset.key.digest()).then(|| self.resolved.clone())
    }
}

#[test]
fn variant_changes_stay_fail_closed_on_apply_surface() {
    let (mut c, _store, _track, _set, _branch) = variant_project();
    let mut before_p = c.live().clone();
    before_p.variant_sets.clear();
    let before = snap_manual(before_p, "apply-before");
    let after = snap_live(&c);
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|ch| ch.object.kind == ObjectKind::VariantSet)
        .expect("variant set change missing");

    // `target_fingerprint` refuses variant kinds outright — the
    // apply whitelist never learned them.
    let objects = vec![change.object.clone()];
    let err = target_fingerprint(c.live(), &objects).unwrap_err();
    assert!(
        matches!(
            err,
            ApplyError::Unsupported {
                kind: ObjectKind::VariantSet,
                ..
            }
        ),
        "expected Unsupported for VariantSet, got {err:?}"
    );

    // `apply_selected` fails closed at the same boundary and leaves
    // the project byte-identical.
    let intent = ApplyChangeIntent {
        change_set_id: cs.key.digest(),
        source_snapshot_id: after.identity.digest,
        target_object: change.object.clone(),
        expected_project_revision: c.live_revision(),
        selected_change_ids: vec![change.id.clone()],
        expected_target_fingerprint: ContentDigest::sha256(b"unused"),
        approval: ApplyApproval {
            ticket: "approve".into(),
        },
    };
    let src = Src {
        resolved: ResolvedChangeSet {
            changeset: cs,
            before: before.clone(),
            after: after.clone(),
        },
    };
    let mut journal = InMemoryApplyJournal::new();
    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AllPresentProbe),
        plugins: None,
    };
    let revision = c.live_revision();
    let sets = c.live().variant_sets.clone();
    let err = apply_selected(
        &mut c,
        &intent,
        &sources,
        &mut journal,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(
        matches!(err, ApplyError::Unsupported { .. }),
        "expected Unsupported, got {err:?}"
    );
    assert_eq!(c.live_revision(), revision);
    assert_eq!(c.live().variant_sets, sets);
}
