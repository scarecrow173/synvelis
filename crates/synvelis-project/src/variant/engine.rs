//! Creative Branch engine (synvelis#278, CB-01).
//!
//! All variant mutations flow through [`ProjectController`] — the
//! single writer — as [`VariantOp`] commands, so Fork/Edit/Adopt/
//! Discard inherit the existing CAS, scratch-clone atomicity, undo/redo
//! and save/load contracts unchanged. Candidate payload bytes live
//! *outside* the project snapshot in a [`BranchContentStore`]; the
//! persisted model only references them by digest.
//!
//! Key invariants enforced here:
//!
//! - Non-adopted branch edits never touch live domain state — they
//!   produce a new immutable candidate payload plus a content
//!   revision bump, committed as one CAS.
//! - Editing the adopted branch is a normal live edit plus branch
//!   bookkeeping in the *same* CAS — live and the adopted candidate
//!   can never diverge.
//! - Adopt materializes only the scope's owned objects through the
//!   apply machinery (typed entity transplant + membership reconcile +
//!   preflight), guarded by project/branch/scope CAS. Stale or
//!   conflicting intents fail closed with no partial mutation.
//! - Project commit and runtime activation are separate receipts: a
//!   post-commit publish failure leaves the project committed and is
//!   reported on [`AdoptReceipt::runtime`], never rolled back or
//!   disguised as live runtime state.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    adapter::{DomainAdapter, ReservingIdGenerator},
    error::VariantError,
    scope,
    store::{self, BranchContentStore},
    types::{
        AdoptIntent, AdoptOutcome, AdoptReceipt, AdoptRecord, Branch, BranchContentRef,
        BranchLineage, BranchRevision, DiscardReceipt, EditReceipt, ForkReceipt,
        ProjectCommitEvidence, RuntimePublish, RuntimePublishEvidence, RuntimePublishRequest,
        RuntimePublishState, VariantScope, VariantSet, VariantSetRevision,
    },
};
use crate::{
    Command, Error, IdGenerator, Operation, Project, ProjectController, ProjectOp,
    apply::{
        PluginAvailability, entity,
        plan::{self, PlannedWrite},
    },
    diff::ChangeId,
    diff::ChangeKind,
    id::{BranchId, VariantSetId},
    snapshot::{BlobProbe, ContentDigest, SnapshotCancel, SnapshotResolver},
    store::SCHEMA_VERSION,
};

// ---------------------------------------------------------------------------
// adopt sources / receipts plumbing
// ---------------------------------------------------------------------------

/// Ports an adopt needs beyond the controller and content store.
pub struct AdoptSources<'a> {
    /// Optional asset-availability probe for newly introduced digests.
    /// `None` → every probed digest is `Unknown` → fail closed.
    pub assets: Option<&'a dyn BlobProbe>,
    /// Optional plugin-availability probe for resolved bindings.
    pub plugins: Option<&'a dyn PluginAvailability>,
    /// Runtime publish port — invoked *after* a successful project CAS.
    pub runtime: &'a dyn RuntimePublish,
}

/// No-op runtime port: every publish reports `NotRequested`.
#[derive(Clone, Debug, Default)]
pub struct NoopRuntimePublish;

impl RuntimePublish for NoopRuntimePublish {
    fn publish(&self, _request: &RuntimePublishRequest) -> RuntimePublishEvidence {
        RuntimePublishEvidence {
            state: RuntimePublishState::NotRequested,
            detail: None,
        }
    }
}

/// Deterministic fake runtime port for fixtures — returns a configured
/// outcome so tests can exercise the committed-project/pending-runtime
/// split without a real engine.
#[derive(Clone, Debug)]
pub struct FakeRuntimePublish {
    outcome: RuntimePublishState,
}

impl FakeRuntimePublish {
    /// A port that always reports `state`.
    #[must_use]
    pub const fn new(state: RuntimePublishState) -> Self {
        Self { outcome: state }
    }
}

impl RuntimePublish for FakeRuntimePublish {
    fn publish(&self, _request: &RuntimePublishRequest) -> RuntimePublishEvidence {
        let detail = match &self.outcome {
            RuntimePublishState::Pending { reason } | RuntimePublishState::Failed { reason } => {
                Some(reason.clone())
            }
            _ => None,
        };
        RuntimePublishEvidence {
            state: self.outcome.clone(),
            detail,
        }
    }
}

// ---------------------------------------------------------------------------
// VariantOp — the single-writer operation for all variant mutations
// ---------------------------------------------------------------------------

/// Internal [`Operation`] carrying every variant mutation. Engine
/// functions pre-compute all content-store writes *before* applying so
/// `apply` stays a pure project+ids transform; orphaned store bytes on
/// failure are harmless (content-addressed, unreferenced).
#[derive(Clone, Debug)]
pub(crate) enum VariantOp {
    /// Insert a fully-formed variant set (ids already drawn).
    Fork {
        /// The set to insert.
        set: VariantSet,
    },
    /// A live edit through the adopted branch: apply `inner` to the
    /// project, then — only when the scope fingerprint actually moved —
    /// bump the branch's live content revision and the set revision.
    EditAdopted {
        /// Set containing the branch.
        set: VariantSetId,
        /// The adopted branch.
        branch: BranchId,
        /// Branch content revision the caller observed (CAS).
        expected_branch_revision: BranchRevision,
        /// The set's scope (for the fingerprint comparison).
        scope: VariantScope,
        /// The live domain op to apply.
        inner: ProjectOp,
    },
    /// Swap a non-adopted branch's captured content and merge minted-ID
    /// lineage — one CAS, one undo.
    EditCandidate {
        /// Set containing the branch.
        set: VariantSetId,
        /// The candidate branch.
        branch: BranchId,
        /// Branch content revision the caller observed (CAS).
        expected_branch_revision: BranchRevision,
        /// The new captured content reference.
        new_content: BranchContentRef,
        /// `(tagged stable id, revision)` pairs minted by this edit.
        new_local_ids: Vec<(String, BranchRevision)>,
    },
    /// Materialize a candidate's scoped objects into live, then swap
    /// the adopted pointer and freeze the previous adopted branch —
    /// all inside one CAS.
    Adopt {
        /// Set being adopted into.
        set: VariantSetId,
        /// Branch becoming adopted.
        branch: BranchId,
        /// Branch content revision the caller observed (CAS).
        expected_branch_revision: BranchRevision,
        /// Planned entity transplants (apply-surface kinds only).
        writes: Vec<PlannedWrite>,
        /// The candidate project used for membership reconcile.
        after: Box<Project>,
        /// New `Live` content ref for the adopted branch.
        new_content: BranchContentRef,
        /// `(previous adopted branch, frozen captured ref)` — `None`
        /// only in degenerate single-branch sets (unreachable: adopt
        /// requires a non-adopted branch).
        frozen: Option<(BranchId, BranchContentRef)>,
        /// Project-side adopt evidence persisted on the set.
        record: AdoptRecord,
    },
    /// Remove a non-adopted branch.
    Discard {
        /// Set containing the branch.
        set: VariantSetId,
        /// Branch to discard.
        branch: BranchId,
    },
}

fn set_of<'a>(project: &'a Project, id: &VariantSetId) -> Result<&'a VariantSet, Error> {
    project
        .variant_sets
        .get(id)
        .ok_or_else(|| Error::NotFound(format!("variant set {id}")))
}

fn branch_of<'a>(set: &'a VariantSet, id: &BranchId) -> Result<&'a Branch, Error> {
    set.branches
        .get(id)
        .ok_or_else(|| Error::NotFound(format!("branch {id}")))
}

fn check_branch_revision(branch: &Branch, expected: BranchRevision) -> Result<(), Error> {
    let actual = branch.content.revision();
    if actual != expected {
        return Err(Error::StaleRevision {
            expected: expected.raw(),
            actual: actual.raw(),
        });
    }
    Ok(())
}

impl Operation for VariantOp {
    fn validate(&self, project: &Project) -> Result<(), Error> {
        match self {
            Self::Fork { set } => {
                if project.variant_sets.contains_key(&set.id) {
                    return Err(Error::Validation(format!(
                        "variant set {} already exists",
                        set.id
                    )));
                }
                scope::validate_scope_structure(project, &set.scope)?;
                for existing in project.variant_sets.values() {
                    if scope::scopes_conflict(project, &existing.scope, &set.scope)? {
                        return Err(Error::Validation(format!(
                            "scope conflicts with variant set {}",
                            existing.id
                        )));
                    }
                }
                validate_set_shape(project, set)
            }
            Self::EditAdopted {
                set,
                branch,
                expected_branch_revision,
                inner,
                ..
            } => {
                let s = set_of(project, set)?;
                let b = branch_of(s, branch)?;
                if s.adopted != *branch {
                    return Err(Error::Validation(
                        "edit-adopted targets a non-adopted branch".into(),
                    ));
                }
                check_branch_revision(b, *expected_branch_revision)?;
                inner.validate(project)
            }
            Self::EditCandidate {
                set,
                branch,
                expected_branch_revision,
                new_content,
                ..
            } => {
                let s = set_of(project, set)?;
                let b = branch_of(s, branch)?;
                if s.adopted == *branch {
                    return Err(Error::Validation(
                        "candidate edit targets the adopted branch".into(),
                    ));
                }
                check_branch_revision(b, *expected_branch_revision)?;
                if new_content.revision() != expected_branch_revision.next() {
                    return Err(Error::Validation(
                        "candidate content revision does not advance".into(),
                    ));
                }
                Ok(())
            }
            Self::Adopt {
                set,
                branch,
                expected_branch_revision,
                after,
                ..
            } => {
                let s = set_of(project, set)?;
                let b = branch_of(s, branch)?;
                if s.adopted == *branch {
                    return Err(Error::Validation(
                        "adopt targets the already-adopted branch".into(),
                    ));
                }
                check_branch_revision(b, *expected_branch_revision)?;
                if after.project_id != project.project_id {
                    return Err(Error::Validation(
                        "adopt candidate belongs to a different project".into(),
                    ));
                }
                Ok(())
            }
            Self::Discard { set, branch } => {
                let s = set_of(project, set)?;
                branch_of(s, branch)?;
                if s.adopted == *branch {
                    return Err(Error::Validation(
                        "cannot discard the adopted branch".into(),
                    ));
                }
                if s.branches.len() <= 1 {
                    return Err(Error::Validation(
                        "cannot discard the last branch of a variant set".into(),
                    ));
                }
                Ok(())
            }
        }
    }

    fn apply(&self, project: &mut Project, ids: &mut dyn IdGenerator) -> Result<(), Error> {
        match self {
            Self::Fork { set } => {
                project.variant_sets.insert(set.id, set.clone());
            }
            Self::EditAdopted {
                set,
                branch,
                scope,
                inner,
                ..
            } => {
                let fp_before = scope::scope_fingerprint(project, scope)?;
                Operation::apply(inner, project, ids)?;
                let fp_after = scope::scope_fingerprint(project, scope)?;
                if fp_before != fp_after {
                    let s = project
                        .variant_sets
                        .get_mut(set)
                        .ok_or_else(|| Error::NotFound(format!("variant set {set}")))?;
                    let b = s
                        .branches
                        .get_mut(branch)
                        .ok_or_else(|| Error::NotFound(format!("branch {branch}")))?;
                    let next = b.content.revision().next();
                    b.content = BranchContentRef::Live { revision: next };
                    s.revision = s.revision.next();
                }
            }
            Self::EditCandidate {
                set,
                branch,
                new_content,
                new_local_ids,
                ..
            } => {
                let s = project
                    .variant_sets
                    .get_mut(set)
                    .ok_or_else(|| Error::NotFound(format!("variant set {set}")))?;
                let b = s
                    .branches
                    .get_mut(branch)
                    .ok_or_else(|| Error::NotFound(format!("branch {branch}")))?;
                b.content = new_content.clone();
                for (stable_id, revision) in new_local_ids {
                    b.lineage.local_ids.insert(stable_id.clone(), *revision);
                }
                s.revision = s.revision.next();
            }
            Self::Adopt {
                set,
                branch,
                after,
                writes,
                new_content,
                frozen,
                record,
                ..
            } => {
                entity::write_all(project, after, writes, ids)?;
                let s = project
                    .variant_sets
                    .get_mut(set)
                    .ok_or_else(|| Error::NotFound(format!("variant set {set}")))?;
                if let Some((old_id, frozen_ref)) = frozen
                    && let Some(old) = s.branches.get_mut(old_id)
                {
                    old.content = frozen_ref.clone();
                }
                s.adopted = *branch;
                if let Some(b) = s.branches.get_mut(branch) {
                    b.content = new_content.clone();
                }
                s.revision = s.revision.next();
                s.last_adopt = Some(record.clone());
            }
            Self::Discard { set, branch } => {
                let s = project
                    .variant_sets
                    .get_mut(set)
                    .ok_or_else(|| Error::NotFound(format!("variant set {set}")))?;
                s.branches.remove(branch);
                s.revision = s.revision.next();
            }
        }
        Ok(())
    }
}

/// Structural invariants of a persisted/constructed set — shared by
/// `validate` and the load oracle.
pub(crate) fn validate_set_shape(project: &Project, set: &VariantSet) -> Result<(), Error> {
    if set.branches.is_empty() {
        return Err(Error::Validation(format!(
            "variant set {} has no branches",
            set.id
        )));
    }
    if !set.branches.contains_key(&set.adopted) {
        return Err(Error::Validation(format!(
            "variant set {} adopted branch {} missing",
            set.id, set.adopted
        )));
    }
    for (bid, branch) in &set.branches {
        if branch.id != *bid {
            return Err(Error::Validation(format!(
                "branch key {bid} does not match branch id {}",
                branch.id
            )));
        }
        if let Some(parent) = branch.parent
            && !set.branches.contains_key(&parent)
        {
            return Err(Error::Validation(format!(
                "branch {bid} parent {parent} missing"
            )));
        }
        if branch.created_from.project_id != project.project_id {
            return Err(Error::Validation(format!(
                "branch {bid} origin belongs to a different project"
            )));
        }
        if branch.content.revision().raw() == 0 {
            return Err(Error::Validation(format!(
                "branch {bid} content revision is zero"
            )));
        }
        for revision in branch.lineage.local_ids.values() {
            if revision.raw() == 0 {
                return Err(Error::Validation(format!(
                    "branch {bid} lineage revision is zero"
                )));
            }
        }
        // Adopted ⇔ Live content; non-adopted ⇔ Captured — the mirror
        // invariant that keeps live and candidate from diverging.
        let is_adopted = *bid == set.adopted;
        match (&branch.content, is_adopted) {
            (BranchContentRef::Live { .. }, true) | (BranchContentRef::Captured { .. }, false) => {}
            _ => {
                return Err(Error::Validation(format!(
                    "branch {bid} content kind violates the adopted/captured invariant"
                )));
            }
        }
    }
    if set.origin.project_id != project.project_id {
        return Err(Error::Validation(format!(
            "variant set {} origin belongs to a different project",
            set.id
        )));
    }
    if set.revision.raw() == 0 {
        return Err(Error::Validation(format!(
            "variant set {} revision is zero",
            set.id
        )));
    }
    Ok(())
}

/// Load-oracle entry point — called from `Project::validate_references`.
pub(crate) fn validate_persisted(project: &Project) -> Result<(), Error> {
    for (key, set) in &project.variant_sets {
        if *key != set.id {
            return Err(Error::Validation(format!(
                "variant set key {key} does not match id {}",
                set.id
            )));
        }
        validate_set_shape(project, set)?;
        scope::validate_scope_structure(project, &set.scope)?;
    }
    // Pairwise scope conflict — two persisted sets may never own the
    // same object.
    let sets: Vec<&VariantSet> = project.variant_sets.values().collect();
    for (i, a) in sets.iter().enumerate() {
        for b in sets.iter().skip(i + 1) {
            if scope::scopes_conflict(project, &a.scope, &b.scope)? {
                return Err(Error::Validation(format!(
                    "variant sets {} and {} own overlapping scopes",
                    a.id, b.id
                )));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// engine entry points
// ---------------------------------------------------------------------------

/// Forks a new [`VariantSet`] over `scope`: an adopted live-mirror
/// branch plus one candidate branch holding an immutable payload
/// snapshot. One controller CAS, one undo step.
///
/// # Errors
/// [`VariantError::InvalidScope`]/[`VariantError::ScopeConflict`]/
/// [`VariantError::Snapshot`]/[`VariantError::Store`]/
/// [`VariantError::Project`].
pub fn fork_variant_set(
    controller: &mut ProjectController,
    adapter: &dyn DomainAdapter,
    store: &mut dyn BranchContentStore,
    resolver: &SnapshotResolver,
    scope: VariantScope,
    cancel: &SnapshotCancel,
) -> Result<ForkReceipt, VariantError> {
    adapter.validate_scope(controller.live(), &scope)?;
    for existing in controller.live().variant_sets.values() {
        if scope::scopes_conflict(controller.live(), &existing.scope, &scope)? {
            return Err(VariantError::ScopeConflict {
                variant_set: existing.id.to_tagged(),
                detail: format!("new scope overlaps {}", existing.id),
            });
        }
    }
    let rev = controller.live_revision();
    let snapshot = resolver.capture_live(controller, rev, cancel, None)?;
    let origin = snapshot.identity.clone();
    let candidate = adapter.fork_candidate(controller.live())?;
    let payload = store::encode_candidate(&candidate)?;
    let payload_digest = ContentDigest::sha256(&payload);
    store.put(&payload_digest, &payload)?;
    let scope_digest = scope::scope_fingerprint(&candidate, &scope)?;
    let origin_objects: Vec<_> = scope::scope_objects(controller.live(), &scope)?
        .into_iter()
        .collect();

    let set_id = VariantSetId::new(controller.draw_raw_id());
    let adopted_id = BranchId::new(controller.draw_raw_id());
    let candidate_id = BranchId::new(controller.draw_raw_id());
    let mut branches = BTreeMap::new();
    branches.insert(
        adopted_id,
        Branch {
            id: adopted_id,
            parent: None,
            created_from: origin.clone(),
            content: BranchContentRef::Live {
                revision: BranchRevision::new(1),
            },
            lineage: BranchLineage::default(),
        },
    );
    branches.insert(
        candidate_id,
        Branch {
            id: candidate_id,
            parent: None,
            created_from: origin.clone(),
            content: BranchContentRef::Captured {
                revision: BranchRevision::new(1),
                payload_digest,
                scope_digest,
                schema_version: SCHEMA_VERSION,
            },
            lineage: BranchLineage::default(),
        },
    );
    let set = VariantSet {
        id: set_id,
        scope,
        origin,
        branches,
        adopted: adopted_id,
        revision: VariantSetRevision::new(1),
        origin_objects,
        last_adopt: None,
    };
    controller.apply(Command {
        expected_revision: rev,
        op: VariantOp::Fork { set },
    })?;
    Ok(ForkReceipt {
        variant_set: set_id,
        adopted: adopted_id,
        candidate: candidate_id,
        project_revision: controller.live_revision(),
    })
}

/// Diffs the scope's owned objects between `live` and `candidate` and
/// plans the entity transplants an adopt would commit.
///
/// Fail-closed rules:
///
/// - candidate-side removal of an entity present at fork and still
///   live → [`VariantError::Unsupported`] (no guessing);
/// - owned kinds outside the apply surface (comping) →
///   [`VariantError::Unsupported`] until domain adapters land;
/// - a scoped entity absent from live that is neither branch-minted
///   (lineage) nor from the fork origin →
///   [`VariantError::AmbiguousLineage`].
///
/// # Errors
/// [`VariantError::Unsupported`]/[`VariantError::AmbiguousLineage`]/
/// [`VariantError::Project`] (scope extraction failures).
pub(crate) fn plan_adopt_writes(
    live: &Project,
    candidate: &Project,
    set: &VariantSet,
    branch: &Branch,
    scope: &VariantScope,
) -> Result<Vec<PlannedWrite>, VariantError> {
    let cand_scope = scope::scope_objects(candidate, scope)?;
    let live_scope = scope::scope_objects(live, scope)?;
    let origin_ids: BTreeSet<String> = set
        .origin_objects
        .iter()
        .map(|o| o.stable_id.clone())
        .collect();

    // Candidate-side removal of an entity that existed at fork and is
    // still live → unsupported in this slice (fail closed, no guess).
    for origin in &set.origin_objects {
        if live_scope.contains(origin) && !cand_scope.contains(origin) {
            return Err(VariantError::Unsupported {
                detail: format!(
                    "scope entity removal is not supported: {}",
                    origin.stable_id
                ),
            });
        }
    }

    let mut writes: Vec<PlannedWrite> = Vec::new();
    for cref in &cand_scope {
        if !entity::supported_kind(cref.kind) {
            return Err(VariantError::Unsupported {
                detail: format!(
                    "adopt materialization for {:?} is deferred to domain adapters",
                    cref.kind
                ),
            });
        }
        let cval = scope::scope_value(candidate, cref)?;
        let lval = scope::scope_value(live, cref)?;
        if cval == lval {
            continue;
        }
        let is_added = lval.is_none();
        if is_added
            && !branch.lineage.local_ids.contains_key(&cref.stable_id)
            && !origin_ids.contains(&cref.stable_id)
        {
            return Err(VariantError::AmbiguousLineage {
                detail: format!(
                    "scoped entity {} has no provable lineage (not live, not branch-minted, not from origin)",
                    cref.stable_id
                ),
            });
        }
        let change_kind = if is_added {
            ChangeKind::Added
        } else {
            ChangeKind::Modified
        };
        let payload = entity::payload_for(live, candidate, cref, change_kind, is_added)
            .map_err(VariantError::from)?;
        writes.push(PlannedWrite {
            object: cref.clone(),
            change_id: ChangeId(format!("adopt-{:04}", writes.len())),
            change_kind,
            payload,
        });
    }
    Ok(writes)
}

/// Intent for [`edit_branch`].
pub struct BranchEditIntent {
    /// Set containing the branch.
    pub variant_set: VariantSetId,
    /// Branch to edit (adopted or candidate).
    pub branch: BranchId,
    /// Branch content revision the caller observed (CAS).
    pub expected_branch_revision: BranchRevision,
    /// The domain op to apply — live for the adopted branch, candidate
    /// content for a non-adopted one.
    pub op: ProjectOp,
}

/// Edits one branch. Adopted branch → live edit + mirror bookkeeping in
/// one CAS. Non-adopted → new immutable candidate payload; live is
/// untouched.
///
/// # Errors
/// [`VariantError::StaleBranchRevision`]/scope/adapter/store/project
/// failures — all fail closed with no partial mutation.
pub fn edit_branch(
    controller: &mut ProjectController,
    adapter: &dyn DomainAdapter,
    store: &mut dyn BranchContentStore,
    intent: &BranchEditIntent,
) -> Result<EditReceipt, VariantError> {
    let (is_adopted, scope, content_rev) = {
        let live = controller.live();
        let set = live
            .variant_sets
            .get(&intent.variant_set)
            .ok_or_else(|| Error::NotFound(format!("variant set {}", intent.variant_set)))?;
        let branch = set
            .branches
            .get(&intent.branch)
            .ok_or_else(|| Error::NotFound(format!("branch {}", intent.branch)))?;
        (
            set.adopted == intent.branch,
            set.scope.clone(),
            branch.content.revision(),
        )
    };
    if content_rev != intent.expected_branch_revision {
        return Err(VariantError::StaleBranchRevision {
            expected: intent.expected_branch_revision.raw(),
            actual: content_rev.raw(),
        });
    }
    let rev = controller.live_revision();
    if is_adopted {
        controller.apply(Command {
            expected_revision: rev,
            op: VariantOp::EditAdopted {
                set: intent.variant_set,
                branch: intent.branch,
                expected_branch_revision: intent.expected_branch_revision,
                scope,
                inner: intent.op.clone(),
            },
        })?;
        let new_rev = controller
            .live()
            .variant_sets
            .get(&intent.variant_set)
            .and_then(|s| s.branches.get(&intent.branch))
            .map(|b| b.content.revision());
        let changed = new_rev.is_some_and(|r| r != content_rev);
        return Ok(EditReceipt {
            variant_set: intent.variant_set,
            branch: intent.branch,
            content_revision: changed.then_some(new_rev.unwrap_or(content_rev)),
            project_revision: controller.live_revision(),
        });
    }

    // Non-adopted: load candidate, edit inside the scope, store the new
    // immutable payload, then commit the ref swap as one CAS.
    let content = {
        let live = controller.live();
        let set = live
            .variant_sets
            .get(&intent.variant_set)
            .ok_or_else(|| Error::NotFound(format!("variant set {}", intent.variant_set)))?;
        set.branches
            .get(&intent.branch)
            .ok_or_else(|| Error::NotFound(format!("branch {}", intent.branch)))?
            .content
            .clone()
    };
    let mut candidate = store::decode_candidate(store, &content)?;
    let before_objects = scope::scope_objects(&candidate, &scope)?;
    let new_revision = intent.expected_branch_revision.next();
    {
        let mut id_draws = ReservingIdGenerator::new(controller);
        adapter.edit_candidate(&mut candidate, &scope, &intent.op, &mut id_draws)?;
    }
    candidate.id_watermark = controller.peek_raw_id();
    candidate.validate_references()?;
    let after_objects = scope::scope_objects(&candidate, &scope)?;
    let new_local_ids: Vec<(String, BranchRevision)> = after_objects
        .difference(&before_objects)
        .map(|o| (o.stable_id.clone(), new_revision))
        .collect();
    let payload = store::encode_candidate(&candidate)?;
    let payload_digest = ContentDigest::sha256(&payload);
    store.put(&payload_digest, &payload)?;
    let scope_digest = scope::scope_fingerprint(&candidate, &scope)?;
    let new_content = BranchContentRef::Captured {
        revision: new_revision,
        payload_digest,
        scope_digest,
        schema_version: SCHEMA_VERSION,
    };
    controller.apply(Command {
        expected_revision: rev,
        op: VariantOp::EditCandidate {
            set: intent.variant_set,
            branch: intent.branch,
            expected_branch_revision: intent.expected_branch_revision,
            new_content,
            new_local_ids,
        },
    })?;
    Ok(EditReceipt {
        variant_set: intent.variant_set,
        branch: intent.branch,
        content_revision: Some(new_revision),
        project_revision: controller.live_revision(),
    })
}

/// Everything [`adopt_branch`] needs from the live project before it
/// can plan the transplant: identity of the set/branch/scope, the
/// branch's declared content and revision, and the live scope
/// fingerprint the intent was checked against.
struct AdoptGuard {
    before_rev: crate::controller::ProjectRevision,
    set_id: VariantSetId,
    branch_id: BranchId,
    scope: VariantScope,
    content: BranchContentRef,
    branch_rev: BranchRevision,
    is_adopted: bool,
    live_scope_digest: ContentDigest,
}

/// CAS + approval + scope-fingerprint preflight for adopt. Fails closed
/// before any mutation when the intent is stale or unapproved.
fn adopt_guard(
    controller: &ProjectController,
    intent: &AdoptIntent,
) -> Result<AdoptGuard, VariantError> {
    let before_rev = controller.live_revision();
    if intent.expected_project_revision != before_rev {
        return Err(VariantError::StaleProjectRevision {
            expected: intent.expected_project_revision.0,
            actual: before_rev.0,
        });
    }
    if intent.approval.ticket.trim().is_empty() {
        return Err(VariantError::Unapproved);
    }
    let (set_id, branch_id, scope, content, branch_rev, is_adopted) = {
        let live = controller.live();
        let set = live
            .variant_sets
            .get(&intent.variant_set)
            .ok_or_else(|| Error::NotFound(format!("variant set {}", intent.variant_set)))?;
        let branch = set
            .branches
            .get(&intent.branch)
            .ok_or_else(|| Error::NotFound(format!("branch {}", intent.branch)))?;
        (
            set.id,
            branch.id,
            set.scope.clone(),
            branch.content.clone(),
            branch.content.revision(),
            set.adopted == intent.branch,
        )
    };
    if branch_rev != intent.expected_branch_revision {
        return Err(VariantError::StaleBranchRevision {
            expected: intent.expected_branch_revision.raw(),
            actual: branch_rev.raw(),
        });
    }
    let live_scope_digest = scope::scope_fingerprint(controller.live(), &scope)?;
    if live_scope_digest != intent.expected_scope_digest {
        return Err(VariantError::StaleScope {
            expected: intent.expected_scope_digest,
            actual: live_scope_digest,
        });
    }
    Ok(AdoptGuard {
        before_rev,
        set_id,
        branch_id,
        scope,
        content,
        branch_rev,
        is_adopted,
        live_scope_digest,
    })
}

/// Loads the candidate payload and re-verifies its declared scope
/// fingerprint — a mismatch means the stored bytes are not what the
/// branch claims, so adoption fails closed.
fn verified_candidate(
    store: &dyn BranchContentStore,
    content: &BranchContentRef,
    scope: &VariantScope,
) -> Result<Project, VariantError> {
    let candidate = store::decode_candidate(store, content)?;
    if let BranchContentRef::Captured { scope_digest, .. } = content {
        let actual = scope::scope_fingerprint(&candidate, scope)?;
        if *scope_digest != actual {
            return Err(VariantError::CandidateUnavailable {
                detail: format!("candidate scope digest {scope_digest} != recomputed {actual}"),
            });
        }
    }
    Ok(candidate)
}

/// Captures the pre-adopt live scope as the outgoing adopted branch's
/// immutable payload. Runs *before* the adopt CAS mutates live.
fn freeze_adopted_branch(
    controller: &ProjectController,
    store: &mut dyn BranchContentStore,
    set_id: &VariantSetId,
    old_adopted: BranchId,
    scope_digest: ContentDigest,
) -> Result<BranchContentRef, VariantError> {
    let mut frozen_project = controller.live().clone();
    frozen_project.variant_sets.clear();
    let frozen_bytes = store::encode_candidate(&frozen_project)?;
    let frozen_digest = ContentDigest::sha256(&frozen_bytes);
    store.put(&frozen_digest, &frozen_bytes)?;
    let old_branch_revision = controller
        .live()
        .variant_sets
        .get(set_id)
        .and_then(|s| s.branches.get(&old_adopted))
        .map_or(BranchRevision::new(1), |b| b.content.revision());
    Ok(BranchContentRef::Captured {
        revision: old_branch_revision.next(),
        payload_digest: frozen_digest,
        scope_digest,
        schema_version: SCHEMA_VERSION,
    })
}

/// Adopts `intent.branch`'s scoped candidate content into live: one
/// controller CAS materializing every scoped entity write, swapping
/// the adopted pointer, and freezing the previous adopted branch —
/// then a separate runtime-publish receipt.
///
/// # Errors
/// Typed stale/conflict/unsupported/ambiguous rejections — all fail
/// closed before any mutation.
pub fn adopt_branch(
    controller: &mut ProjectController,
    store: &mut dyn BranchContentStore,
    sources: &AdoptSources<'_>,
    intent: &AdoptIntent,
) -> Result<AdoptReceipt, VariantError> {
    let guard = adopt_guard(controller, intent)?;
    if guard.is_adopted {
        // Same-candidate re-adopt: no CAS, no undo, honest receipt.
        return Ok(AdoptReceipt {
            variant_set: guard.set_id,
            branch: guard.branch_id,
            before_revision: guard.before_rev,
            after_revision: guard.before_rev,
            objects: 0,
            outcome: AdoptOutcome::NoChange,
            project: ProjectCommitEvidence {
                committed: false,
                revision: guard.before_rev,
                undo_steps: 0,
            },
            runtime: RuntimePublishEvidence {
                state: RuntimePublishState::NotRequested,
                detail: None,
            },
        });
    }

    let candidate = verified_candidate(store, &guard.content, &guard.scope)?;

    // Plan the transplants against the live project, then run the
    // same asset/plugin/port preflight the apply surface uses.
    let (writes, old_adopted) = {
        let live = controller.live();
        let set = live
            .variant_sets
            .get(&guard.set_id)
            .ok_or_else(|| Error::NotFound(format!("variant set {}", guard.set_id)))?;
        let branch = set
            .branches
            .get(&guard.branch_id)
            .ok_or_else(|| Error::NotFound(format!("branch {}", guard.branch_id)))?;
        let writes = plan_adopt_writes(live, &candidate, set, branch, &guard.scope)?;
        plan::preflight_assets(live, &writes, sources.assets, &candidate)?;
        plan::preflight_plugins(live, &writes, sources.plugins)?;
        plan::preflight_ports(live, &candidate, &writes)?;
        (writes, set.adopted)
    };

    let frozen_ref = freeze_adopted_branch(
        controller,
        store,
        &guard.set_id,
        old_adopted,
        guard.live_scope_digest,
    )?;
    let new_content = BranchContentRef::Live {
        revision: guard.branch_rev.next(),
    };
    let record = AdoptRecord {
        branch: guard.branch_id,
        branch_revision: guard.branch_rev.next(),
        objects: u32::try_from(writes.len()).unwrap_or(u32::MAX),
        outcome: AdoptOutcome::Applied,
    };

    let object_count = writes.len();
    controller.apply(Command {
        expected_revision: intent.expected_project_revision,
        op: VariantOp::Adopt {
            set: guard.set_id,
            branch: guard.branch_id,
            expected_branch_revision: intent.expected_branch_revision,
            writes,
            after: Box::new(candidate),
            new_content,
            frozen: Some((old_adopted, frozen_ref)),
            record,
        },
    })?;
    let after_rev = controller.live_revision();

    // Runtime activation is a separate receipt domain — a failure here
    // never rolls back or falsifies the committed project state.
    let runtime_digest = scope::scope_fingerprint(controller.live(), &guard.scope)?;
    let runtime = sources.runtime.publish(&RuntimePublishRequest {
        project_id: controller.live().project_id,
        revision: after_rev,
        variant_set: guard.set_id,
        branch: guard.branch_id,
        scope_digest: runtime_digest,
    });
    Ok(AdoptReceipt {
        variant_set: guard.set_id,
        branch: guard.branch_id,
        before_revision: guard.before_rev,
        after_revision: after_rev,
        objects: u32::try_from(object_count).unwrap_or(u32::MAX),
        outcome: AdoptOutcome::Applied,
        project: ProjectCommitEvidence {
            committed: true,
            revision: after_rev,
            undo_steps: 1,
        },
        runtime,
    })
}

/// Discards a non-adopted branch. Its candidate payload stays in the
/// content store unreferenced (harmless orphan); live is untouched.
///
/// # Errors
/// [`VariantError::Project`] on missing branch / adopted-branch /
/// last-branch rejection.
pub fn discard_branch(
    controller: &mut ProjectController,
    variant_set: VariantSetId,
    branch: BranchId,
) -> Result<DiscardReceipt, VariantError> {
    let rev = controller.live_revision();
    controller.apply(Command {
        expected_revision: rev,
        op: VariantOp::Discard {
            set: variant_set,
            branch,
        },
    })?;
    Ok(DiscardReceipt {
        variant_set,
        branch,
        project_revision: controller.live_revision(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClipId, MusicalRange, MusicalTick, ProjectOp,
        id::CounterIdGenerator,
        placement::{ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor},
        range::SourceRange,
        time::{MusicalDuration, SourceFrame},
        variant::ArrangementDomainMask,
    };

    fn controller() -> ProjectController {
        ProjectController::new(
            Project::new(crate::ProjectId::new(1), "t"),
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

    /// A crafted candidate carrying a scoped clip that is neither live,
    /// nor branch-minted, nor from the fork origin must be rejected as
    /// ambiguous lineage — never guessed into an `Added` transplant.
    #[test]
    fn ambiguous_lineage_is_rejected() {
        let mut c = controller();
        apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
        let track = *c.live().tracks.keys().next().unwrap();
        apply(&mut c, ProjectOp::AddLane { track_id: track });
        let lane = *c.live().lanes.keys().next().unwrap();
        apply(&mut c, ProjectOp::RegisterAsset { digest: "a".into() });
        let asset = *c.live().assets.keys().next().unwrap();
        apply(
            &mut c,
            ProjectOp::RegisterSource {
                asset_id: asset,
                content_digest: "d".into(),
            },
        );
        let source = *c.live().sources.keys().next().unwrap();
        apply(
            &mut c,
            ProjectOp::AddClip {
                lane_id: lane,
                source_id: source,
                placement: ClipPlacement {
                    anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                    extent: ClipExtent::Musical(MusicalDuration::new(960).unwrap()),
                },
                playback: ClipPlayback {
                    source_region: SourceRange::try_new(
                        SourceFrame::ZERO,
                        SourceFrame::new(960).unwrap(),
                    )
                    .unwrap(),
                    mapping: PlaybackMapping::Natural,
                },
            },
        );

        let scope = VariantScope::Arrangement {
            tracks: vec![track],
            time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(1920)).unwrap(),
            fields: ArrangementDomainMask {
                clips: true,
                notes: false,
            },
        };
        let mut store = store::InMemoryBranchContentStore::default();
        let receipt = fork_variant_set(
            &mut c,
            &crate::variant::FakeDomainAdapter::new(),
            &mut store,
            &SnapshotResolver::new(),
            scope.clone(),
            &SnapshotCancel::new(),
        )
        .unwrap();

        // Decode the candidate and inject a foreign clip — same
        // placement (inside the scope), an id live never issued and the
        // branch never minted.
        let set = c.live().variant_sets[&receipt.variant_set].clone();
        let branch = set.branches[&receipt.candidate].clone();
        let mut candidate = store::decode_candidate(&store, &branch.content).unwrap();
        let foreign_id = ClipId::new(candidate.id_watermark + 10_000);
        let mut foreign = candidate.clips.values().next().unwrap().clone();
        foreign.id = foreign_id;
        candidate.clips.insert(foreign_id, foreign);
        if let Some(l) = candidate.lanes.get_mut(&lane) {
            l.clip_ids.push(foreign_id);
        }

        let err = plan_adopt_writes(c.live(), &candidate, &set, &branch, &scope).unwrap_err();
        assert!(
            matches!(err, VariantError::AmbiguousLineage { .. }),
            "expected AmbiguousLineage, got {err:?}"
        );
    }
}
