//! Domain adapter boundary for Creative Branches (synvelis#278, CB-01).
//!
//! The typed contract a domain adapter must provide:
//!
//! - [`DomainAdapter::validate_scope`] — scope legality for the domain
//!   (existence, ownership exclusivity, domain-specific rules);
//! - [`DomainAdapter::fork_candidate`] — produce the immutable
//!   candidate content a new branch starts from;
//! - [`DomainAdapter::edit_candidate`] — apply a typed project op to a
//!   candidate *inside* the declared scope only.
//!
//! [`FakeDomainAdapter`] is the headless fixture implementation: the
//! candidate is a full project clone (domain state only — the variant
//! model is stripped), edits reuse [`ProjectOp`] against the candidate,
//! and scope confinement is enforced by comparing the scope's owned
//! object set before/after the edit. Real domain adapters (CB-02..07)
//! replace the candidate representation with scoped deltas/recipes
//! without changing the engine contract.
//!
//! Candidate edits draw entity IDs through a reserving generator that
//! forwards to the *live* controller generator: branch-minted IDs are
//! ordinary live-space IDs reserved monotonically, so they can never
//! collide with live entities or sibling branches — and the persisted
//! [`BranchLineage`](super::types::BranchLineage) records their
//! provenance for the adopt-time check.

use std::collections::BTreeSet;

use crate::{
    IdGenerator, Operation, Project, ProjectController, ProjectOp,
    diff::{ObjectKind, SemanticObjectRef},
    variant::{
        VariantError,
        scope::{scope_objects, validate_scope_structure},
        types::VariantScope,
    },
};

/// Typed domain-adapter contract for variant candidates.
pub trait DomainAdapter {
    /// Adapter identity for diagnostics/receipts.
    fn name(&self) -> &'static str;

    /// Validates `scope` for `project` — existence, ownership
    /// exclusivity and domain rules. The engine calls this at fork and
    /// re-checks at adopt preflight.
    ///
    /// # Errors
    /// [`VariantError::InvalidScope`]/[`VariantError::ScopeConflict`].
    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError>;

    /// Produces the candidate content a fork starts from. The result
    /// must be domain-valid (`validate_references` clean) and must not
    /// carry variant state.
    ///
    /// # Errors
    /// [`VariantError::Unsupported`]/[`VariantError::Project`].
    fn fork_candidate(&self, project: &Project) -> Result<Project, VariantError>;

    /// Applies `op` to `candidate`, restricted to `scope`'s owned
    /// objects. Fresh entity IDs come from `ids` — the engine passes a
    /// generator backed by the live controller so minted IDs are
    /// collision-free live-space reservations.
    ///
    /// # Errors
    /// [`VariantError::ScopeViolation`]/[`VariantError::Unsupported`]/
    /// [`VariantError::Project`].
    fn edit_candidate(
        &self,
        candidate: &mut Project,
        scope: &VariantScope,
        op: &ProjectOp,
        ids: &mut dyn IdGenerator,
    ) -> Result<(), VariantError>;
}

/// [`IdGenerator`] that forwards every draw to the live controller —
/// branch-local entity creation reserves real live-space IDs so adopt
/// needs no remapping and collisions are impossible by construction.
///
/// `reseed` is a no-op: candidate content shares the live ID space and
/// the live generator's position is the single authority (branch edits
/// never reseed — only `load` does, and candidates never load through
/// the controller).
pub(crate) struct ReservingIdGenerator<'a> {
    controller: &'a mut ProjectController,
}

impl<'a> ReservingIdGenerator<'a> {
    /// Wraps the controller whose generator receives the reservations.
    pub(crate) fn new(controller: &'a mut ProjectController) -> Self {
        Self { controller }
    }
}

impl IdGenerator for ReservingIdGenerator<'_> {
    fn next(&mut self) -> u128 {
        self.controller.draw_raw_id()
    }

    fn peek(&self) -> u128 {
        self.controller.peek_raw_id()
    }

    fn reseed(&mut self, _seed: u128) {
        // No-op by contract — see the type docs.
    }
}

/// The headless fixture adapter: full-project candidates, `ProjectOp`
/// edits confined to the scope's owned object set.
#[derive(Clone, Debug, Default)]
pub struct FakeDomainAdapter;

impl FakeDomainAdapter {
    /// The adapter singleton.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Requires `stable_id` (a `"<kind>-<hex>"` tagged ID) to be inside the
/// scope's owned object set.
fn require_scope(
    project: &Project,
    scope: &VariantScope,
    kind: ObjectKind,
    stable_id: &str,
) -> Result<(), VariantError> {
    let objects = scope_objects(project, scope).map_err(VariantError::from)?;
    let probe = SemanticObjectRef {
        project_id: project.project_id,
        kind,
        stable_id: stable_id.to_owned(),
        lineage: None,
    };
    if objects.contains(&probe) {
        Ok(())
    } else {
        Err(VariantError::ScopeViolation {
            detail: format!("edit target {stable_id} is outside the declared scope"),
        })
    }
}

/// Pre-edit scope check for ops with explicit targets. Creation ops
/// (`AddClip`, `InsertProcessor`) are verified post-edit instead.
fn check_op_targets(
    project: &Project,
    scope: &VariantScope,
    op: &ProjectOp,
) -> Result<(), VariantError> {
    match op {
        ProjectOp::SetClipMuted { clip_id, .. }
        | ProjectOp::MoveClip { clip_id, .. }
        | ProjectOp::SetClipExtent { clip_id, .. }
        | ProjectOp::SetClipPlayback { clip_id, .. } => {
            require_scope(project, scope, ObjectKind::Clip, &clip_id.to_tagged())
        }
        ProjectOp::AddNote { clip, .. } => {
            require_scope(project, scope, ObjectKind::Clip, &clip.to_tagged())
        }
        ProjectOp::RemoveNote { note } => {
            require_scope(project, scope, ObjectKind::Note, &note.to_tagged())
        }
        ProjectOp::SetStripMute { strip_id, .. } | ProjectOp::SetStripSolo { strip_id, .. } => {
            require_scope(project, scope, ObjectKind::Strip, &strip_id.to_tagged())
        }
        ProjectOp::AddClip { .. } | ProjectOp::InsertProcessor { .. } => Ok(()),
        other => Err(VariantError::Unsupported {
            detail: format!("fake adapter does not support this edit op: {other:?}"),
        }),
    }
}

/// Post-edit confinement: every entity id that appeared during the
/// edit must be inside the scope's owned set.
fn check_created_within_scope(
    candidate: &Project,
    after_objects: &BTreeSet<SemanticObjectRef>,
    before_clips: &BTreeSet<crate::id::ClipId>,
    before_notes: &BTreeSet<crate::id::NoteId>,
    before_procs: &BTreeSet<crate::id::ProcessorId>,
    before_ports: &BTreeSet<crate::id::SemanticPortId>,
) -> Result<(), VariantError> {
    let pid = candidate.project_id;
    let check = |kind: ObjectKind, stable_id: String| -> Result<(), VariantError> {
        let probe = SemanticObjectRef {
            project_id: pid,
            kind,
            stable_id: stable_id.clone(),
            lineage: None,
        };
        if after_objects.contains(&probe) {
            Ok(())
        } else {
            Err(VariantError::ScopeViolation {
                detail: format!("created entity {stable_id} is outside the declared scope"),
            })
        }
    };
    for clip in candidate.clips.keys() {
        if !before_clips.contains(clip) {
            check(ObjectKind::Clip, clip.to_tagged())?;
        }
    }
    for note in candidate.notes.keys() {
        if !before_notes.contains(note) {
            check(ObjectKind::Note, note.to_tagged())?;
        }
    }
    for proc in candidate.routing.processors.keys() {
        if !before_procs.contains(proc) {
            check(ObjectKind::Processor, proc.to_tagged())?;
        }
    }
    for port in candidate.routing.ports.keys() {
        if !before_ports.contains(port) {
            check(ObjectKind::SemanticPort, port.to_tagged())?;
        }
    }
    Ok(())
}

impl DomainAdapter for FakeDomainAdapter {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError> {
        validate_scope_structure(project, scope).map_err(VariantError::from)?;
        // Full object-set derivation enforces ownership rules the
        // structural pass cannot (e.g. MIDI source exclusivity).
        scope_objects(project, scope).map_err(VariantError::from)?;
        Ok(())
    }

    fn fork_candidate(&self, project: &Project) -> Result<Project, VariantError> {
        let mut candidate = project.clone();
        candidate.variant_sets.clear();
        candidate.validate_references()?;
        Ok(candidate)
    }

    fn edit_candidate(
        &self,
        candidate: &mut Project,
        scope: &VariantScope,
        op: &ProjectOp,
        ids: &mut dyn IdGenerator,
    ) -> Result<(), VariantError> {
        check_op_targets(candidate, scope, op)?;
        let before_objects = scope_objects(candidate, scope)?;
        let before_clips: BTreeSet<_> = candidate.clips.keys().copied().collect();
        let before_notes: BTreeSet<_> = candidate.notes.keys().copied().collect();
        let before_procs: BTreeSet<_> = candidate.routing.processors.keys().copied().collect();
        let before_ports: BTreeSet<_> = candidate.routing.ports.keys().copied().collect();

        Operation::apply(op, candidate, ids)?;
        candidate.validate_references()?;

        let after_objects = scope_objects(candidate, scope)?;
        // Scope removals are unsupported in CB-01 — fail closed.
        if let Some(removed) = before_objects.difference(&after_objects).next() {
            return Err(VariantError::Unsupported {
                detail: format!(
                    "scope entity removal is not supported in this slice: {}",
                    removed.stable_id
                ),
            });
        }
        check_created_within_scope(
            candidate,
            &after_objects,
            &before_clips,
            &before_notes,
            &before_procs,
            &before_ports,
        )?;
        Ok(())
    }
}
