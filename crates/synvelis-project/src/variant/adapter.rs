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
    comp::CompOp,
    diff::{ObjectKind, SemanticObjectRef},
    variant::{
        VariantError,
        scope::{scope_objects, validate_scope_structure},
        types::VariantScope,
    },
};

/// The operation payload a branch edit may carry.
///
/// [`ProjectOp`] covers the arrangement/media/control/routing surface.
/// Domain op surfaces that are *not* `ProjectOp` variants — comping
/// (`CompOp` is a distinct [`Operation`], deliberately kept out of
/// `ProjectOp` so downstream exhaustive matches stay stable) — travel
/// through their own variant here. Later domain adapters (CB-03..07)
/// extend this union the same way.
#[derive(Clone, Debug)]
pub enum BranchEditOp {
    /// A project-level operation.
    Project(ProjectOp),
    /// A comping-registry operation.
    Comp(CompOp),
}

impl Operation for BranchEditOp {
    fn validate(&self, project: &Project) -> Result<(), crate::Error> {
        match self {
            Self::Project(op) => op.validate(project),
            Self::Comp(op) => op.validate(project),
        }
    }

    fn apply(&self, project: &mut Project, ids: &mut dyn IdGenerator) -> Result<(), crate::Error> {
        match self {
            Self::Project(op) => op.apply(project, ids),
            Self::Comp(op) => op.apply(project, ids),
        }
    }
}

/// Typed domain-adapter contract for variant candidates.
pub trait DomainAdapter {
    /// Adapter identity for diagnostics/receipts.
    fn name(&self) -> &'static str;

    /// Validates `scope` for `project` — existence, ownership
    /// exclusivity and domain rules. The engine calls this at fork.
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
        op: &BranchEditOp,
        ids: &mut dyn IdGenerator,
    ) -> Result<(), VariantError>;

    /// Domain adopt preflight — evaluated inside `adopt_branch` after
    /// the candidate payload and scope digest are verified, before any
    /// transplant is planned or committed. Domain rules that must gate
    /// *live* mutation (e.g. a recording session open on the affected
    /// lane) live here; the default imposes no extra constraints.
    ///
    /// # Errors
    /// [`VariantError::Unsupported`]/[`VariantError::Project`] — any
    /// typed rejection fails the adopt closed before mutation.
    fn preflight_adopt(
        &self,
        _live: &Project,
        _candidate: &Project,
        _scope: &VariantScope,
    ) -> Result<(), VariantError> {
        Ok(())
    }
}

/// Candidate content a fork starts from: the live project minus
/// variant state. Every domain adapter that keeps full-project
/// candidates shares this base.
pub(crate) fn fork_candidate_project(project: &Project) -> Result<Project, VariantError> {
    let mut candidate = project.clone();
    candidate.variant_sets.clear();
    candidate.validate_references()?;
    Ok(candidate)
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
        // MIDI note ops are source-addressed: the source must be a
        // scope-owned `MidiSource` and any named note a scope-owned
        // `MidiNote`. Source-level ops (`AddMidiSource`,
        // `RemoveMidiSource`, `ImportMidiSmf`, `DetachClipSource`)
        // create entities outside the scope or rebind the clip's
        // source, so they fall through to `Unsupported` — detach on
        // live before fork is the supported editable-copy path.
        ProjectOp::AddMidiNote { source_id, .. } => require_scope(
            project,
            scope,
            ObjectKind::MidiSource,
            &source_id.to_tagged(),
        ),
        ProjectOp::RemoveMidiNote { source_id, note_id }
        | ProjectOp::SetMidiNoteVelocity {
            source_id, note_id, ..
        }
        | ProjectOp::MoveMidiNote {
            source_id, note_id, ..
        } => {
            require_scope(
                project,
                scope,
                ObjectKind::MidiSource,
                &source_id.to_tagged(),
            )?;
            require_scope(project, scope, ObjectKind::MidiNote, &note_id.to_tagged())
        }
        ProjectOp::AddClip { .. } | ProjectOp::InsertProcessor { .. } => Ok(()),
        other => Err(VariantError::Unsupported {
            detail: format!("edit op is outside the supported candidate surface: {other:?}"),
        }),
    }
}

/// Entity-id sets captured before a confined edit — the baseline
/// `check_created_within_scope` diffs against to catch entities the
/// op created outside the declared scope.
struct PreEditIds {
    clips: BTreeSet<crate::id::ClipId>,
    notes: BTreeSet<crate::id::NoteId>,
    procs: BTreeSet<crate::id::ProcessorId>,
    ports: BTreeSet<crate::id::SemanticPortId>,
    midi_sources: BTreeSet<crate::id::SourceId>,
    midi_notes: BTreeSet<crate::id::NoteId>,
}

impl PreEditIds {
    fn capture(project: &Project) -> Self {
        Self {
            clips: project.clips.keys().copied().collect(),
            notes: project.notes.keys().copied().collect(),
            procs: project.routing.processors.keys().copied().collect(),
            ports: project.routing.ports.keys().copied().collect(),
            midi_sources: project.midi_sources.keys().copied().collect(),
            midi_notes: project
                .midi_sources
                .values()
                .flat_map(|source| source.notes.keys().copied())
                .collect(),
        }
    }
}

/// Post-edit confinement: every entity id that appeared during the
/// edit must be inside the scope's owned set.
fn check_created_within_scope(
    candidate: &Project,
    after_objects: &BTreeSet<SemanticObjectRef>,
    before: &PreEditIds,
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
        if !before.clips.contains(clip) {
            check(ObjectKind::Clip, clip.to_tagged())?;
        }
    }
    for note in candidate.notes.keys() {
        if !before.notes.contains(note) {
            check(ObjectKind::Note, note.to_tagged())?;
        }
    }
    for proc in candidate.routing.processors.keys() {
        if !before.procs.contains(proc) {
            check(ObjectKind::Processor, proc.to_tagged())?;
        }
    }
    for port in candidate.routing.ports.keys() {
        if !before.ports.contains(port) {
            check(ObjectKind::SemanticPort, port.to_tagged())?;
        }
    }
    for source in candidate.midi_sources.keys() {
        if !before.midi_sources.contains(source) {
            check(ObjectKind::MidiSource, source.to_tagged())?;
        }
    }
    for note in candidate
        .midi_sources
        .values()
        .flat_map(|source| source.notes.keys())
    {
        if !before.midi_notes.contains(note) {
            check(ObjectKind::MidiNote, note.to_tagged())?;
        }
    }
    Ok(())
}

/// Optional domain capability guard applied before a confined
/// project-op candidate edit.
pub(crate) type OpGuard<'a> = Option<&'a dyn Fn(&ProjectOp) -> Result<(), VariantError>>;

/// Shared confined-edit flow for `ProjectOp` candidate edits: optional
/// domain capability guard, pre-edit target confinement, apply,
/// post-edit scope-removal/creation confinement. Domain adapters that
/// accept project ops (the fake fixture, the audio-clip adapter)
/// share this one implementation so confinement rules cannot drift.
pub(crate) fn edit_scoped_project_op(
    candidate: &mut Project,
    scope: &VariantScope,
    op: &ProjectOp,
    ids: &mut dyn IdGenerator,
    guard: OpGuard<'_>,
) -> Result<(), VariantError> {
    if let Some(guard) = guard {
        guard(op)?;
    }
    check_op_targets(candidate, scope, op)?;
    let before_objects = scope_objects(candidate, scope)?;
    let before_ids = PreEditIds::capture(candidate);

    Operation::apply(op, candidate, ids)?;
    candidate.validate_references()?;

    let after_objects = scope_objects(candidate, scope)?;
    // Scope removals are unsupported in this slice — fail closed.
    if let Some(removed) = before_objects.difference(&after_objects).next() {
        return Err(VariantError::Unsupported {
            detail: format!(
                "scope entity removal is not supported in this slice: {}",
                removed.stable_id
            ),
        });
    }
    check_created_within_scope(candidate, &after_objects, &before_ids)?;
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
        fork_candidate_project(project)
    }

    fn edit_candidate(
        &self,
        candidate: &mut Project,
        scope: &VariantScope,
        op: &BranchEditOp,
        ids: &mut dyn IdGenerator,
    ) -> Result<(), VariantError> {
        let BranchEditOp::Project(project_op) = op else {
            return Err(VariantError::Unsupported {
                detail: format!("fake adapter does not support this edit op: {op:?}"),
            });
        };
        edit_scoped_project_op(candidate, scope, project_op, ids, None)
    }
}
