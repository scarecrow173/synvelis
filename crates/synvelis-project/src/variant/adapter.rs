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
    Error, IdGenerator, Operation, Project, ProjectController, ProjectOp,
    comp::CompOp,
    control::ControlOwner,
    diff::{ObjectKind, SemanticObjectRef},
    id::{
        AutomationLaneId, ModulationAssignmentId, ModulationSourceId, ParameterTargetId,
        ProcessorId, StripId,
    },
    variant::{
        VariantError,
        scope::{scope_objects, strip_channel_field, validate_scope_structure},
        types::{StripDomainMask, StripField, VariantScope},
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

/// `true` for the `ProjectOp` variants the instrument-slot candidate
/// surface accepts — the same list
/// [`check_instrument_slot_op_targets`] confines. Kept adjacent so the
/// adapter's validate-gate and the confinement gate cannot drift.
pub(crate) fn is_instrument_slot_op(op: &ProjectOp) -> bool {
    matches!(
        op,
        ProjectOp::SetParameterBase { .. }
            | ProjectOp::AddParameterTarget { .. }
            | ProjectOp::RemoveParameterTarget { .. }
            | ProjectOp::AddAutomationLane { .. }
            | ProjectOp::RemoveAutomationLane { .. }
            | ProjectOp::WriteAutomationRegion { .. }
            | ProjectOp::EraseAutomationRange { .. }
            | ProjectOp::AddModulationSource { .. }
            | ProjectOp::RemoveModulationSource { .. }
            | ProjectOp::AddModulationAssignment { .. }
            | ProjectOp::SetModulationAssignment { .. }
            | ProjectOp::RemoveModulationAssignment { .. }
    )
}

/// Processor IDs owned by `scope` — derived from the scope object set
/// so the creation-owner check can never drift from scope derivation
/// (one processor for `InstrumentSlot`, every range processor for
/// `ProcessingRange`).
fn scoped_processor_ids(
    project: &Project,
    scope: &VariantScope,
) -> Result<BTreeSet<ProcessorId>, VariantError> {
    let objects = scope_objects(project, scope).map_err(VariantError::from)?;
    Ok(objects
        .iter()
        .filter(|o| o.kind == ObjectKind::Processor)
        .filter_map(|o| ProcessorId::from_tagged(&o.stable_id).ok())
        .collect())
}

/// Creation ops (`AddParameterTarget`/`AddModulationSource`) name an
/// owner rather than an entity id: the owner must be a processor the
/// scope owns, otherwise the new entity lands outside the declared
/// scope.
fn require_processor_owner(
    owner: ControlOwner,
    processors: &BTreeSet<ProcessorId>,
    scope: &VariantScope,
) -> Result<(), VariantError> {
    if matches!(owner, ControlOwner::Processor(p) if processors.contains(&p)) {
        Ok(())
    } else {
        Err(VariantError::ScopeViolation {
            detail: format!(
                "edit creates a control entity owned by {owner:?}, outside processor scope {scope:?}"
            ),
        })
    }
}

/// Control-closure ops shared by every candidate surface whose scope
/// owns control entities: parameter base/target, automation
/// lanes/regions and modulation — confined by [`require_scope`]
/// against the scope's owned object set. Creation ops
/// (`AddParameterTarget`/`AddModulationSource`) are *not* handled
/// here: they name an owner rather than an entity id, so each
/// surface resolves owner legality itself before delegating.
fn check_control_closure_op_targets(
    project: &Project,
    scope: &VariantScope,
    op: &ProjectOp,
) -> Result<(), VariantError> {
    match op {
        ProjectOp::SetParameterBase { target, .. }
        | ProjectOp::RemoveParameterTarget { target }
        | ProjectOp::AddAutomationLane { target } => require_scope(
            project,
            scope,
            ObjectKind::ParameterTarget,
            &target.to_tagged(),
        ),
        ProjectOp::RemoveAutomationLane { lane }
        | ProjectOp::WriteAutomationRegion { lane, .. }
        | ProjectOp::EraseAutomationRange { lane, .. } => require_scope(
            project,
            scope,
            ObjectKind::AutomationLane,
            &lane.to_tagged(),
        ),
        ProjectOp::RemoveModulationSource { source } => require_scope(
            project,
            scope,
            ObjectKind::ModulationSource,
            &source.to_tagged(),
        ),
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth_source,
            ..
        } => {
            require_scope(
                project,
                scope,
                ObjectKind::ModulationSource,
                &source.to_tagged(),
            )?;
            require_scope(
                project,
                scope,
                ObjectKind::ParameterTarget,
                &target.to_tagged(),
            )?;
            if let Some(depth) = depth_source {
                require_scope(
                    project,
                    scope,
                    ObjectKind::ParameterTarget,
                    &depth.to_tagged(),
                )?;
            }
            Ok(())
        }
        ProjectOp::SetModulationAssignment { id, .. }
        | ProjectOp::RemoveModulationAssignment { id } => require_scope(
            project,
            scope,
            ObjectKind::ModulationAssignment,
            &id.to_tagged(),
        ),
        other => Err(VariantError::Unsupported {
            detail: format!("edit op is outside the supported candidate surface: {other:?}"),
        }),
    }
}

/// Processor-owned control ops shared by the instrument-slot (CB-04)
/// and FX-range (CB-05) candidate surfaces: parameter base/target
/// edits, automation lanes/regions and modulation — confined by
/// [`require_scope`] against the scope's owned object set and by
/// [`require_processor_owner`] against the scope's processor set.
/// Every other op fails `Unsupported` (fail-closed: shared inputs,
/// chain structure and wiring are never candidate-owned).
fn check_processor_control_op_targets(
    project: &Project,
    scope: &VariantScope,
    op: &ProjectOp,
) -> Result<(), VariantError> {
    if !matches!(
        scope,
        VariantScope::InstrumentSlot { .. } | VariantScope::ProcessingRange { .. }
    ) {
        return Err(VariantError::InvalidScope {
            detail: format!(
                "processor-control target check requires a processor scope, got {scope:?}"
            ),
        });
    }
    match op {
        ProjectOp::AddParameterTarget { owner, .. }
        | ProjectOp::AddModulationSource { owner, .. } => {
            let processors = scoped_processor_ids(project, scope)?;
            require_processor_owner(*owner, &processors, scope)
        }
        _ => check_control_closure_op_targets(project, scope, op),
    }
}

/// `true` for the `ProjectOp` variants the FX-range candidate surface
/// accepts — the shared processor-control list plus `InsertProcessor`,
/// the one structure op whose post-edit scope derivation is
/// unambiguous. Kept adjacent to [`check_processing_range_op_targets`]
/// so the validate-gate and confinement gate cannot drift.
pub(crate) fn is_processing_range_op(op: &ProjectOp) -> bool {
    is_instrument_slot_op(op) || matches!(op, ProjectOp::InsertProcessor { .. })
}

/// FX-range candidate surface (CB-05): the shared processor-owned
/// control ops extended across *every* processor in the declared
/// range, plus `InsertProcessor` — a processor landing inside
/// `start..=end` becomes scope-owned with its ports and control
/// closure (post-edit `processing_range_objects` re-derivation),
/// anything else fails `ScopeViolation` via
/// [`check_created_within_scope`]. Removal, reorder, every
/// connection/send/sidechain op and all shared-context edits stay
/// `Unsupported` — boundary wiring is never candidate-owned and
/// scope-entity removal is unsupported this slice.
fn check_processing_range_op_targets(
    project: &Project,
    scope: &VariantScope,
    op: &ProjectOp,
) -> Result<(), VariantError> {
    let VariantScope::ProcessingRange { range } = scope else {
        return Err(VariantError::InvalidScope {
            detail: format!(
                "processing-range target check requires a ProcessingRange scope, got {scope:?}"
            ),
        });
    };
    if let ProjectOp::InsertProcessor { strip_id, .. } = op {
        // The strip must own the scoped chain — an insert into any
        // other chain lands outside the declared range. The *position*
        // check is post-edit: `processing_range_objects` re-derives
        // membership from stable processor IDs, so an in-range insert
        // is scope-owned and anything else fails ScopeViolation.
        let strip = project
            .routing
            .strips
            .get(strip_id)
            .ok_or_else(|| Error::NotFound(format!("strip {strip_id}")))?;
        if strip.chain != range.chain {
            return Err(VariantError::ScopeViolation {
                detail: format!(
                    "insert targets strip {strip_id} on chain {}, outside range chain {}",
                    strip.chain, range.chain
                ),
            });
        }
        return Ok(());
    }
    check_processor_control_op_targets(project, scope, op)
}

/// `true` for the `ProjectOp` variants the strip-fields candidate
/// surface accepts — the shared control-closure set plus the two
/// strip channel ops and `InsertProcessor`. The declared
/// [`StripDomainMask`] decides per-op/target legality inside
/// [`check_strip_fields_op_targets`]; this predicate only bounds the
/// variant family so the validate-gate and confinement gate cannot
/// drift.
pub(crate) fn is_strip_fields_op(op: &ProjectOp) -> bool {
    is_instrument_slot_op(op)
        || matches!(
            op,
            ProjectOp::SetStripMute { .. }
                | ProjectOp::SetStripSolo { .. }
                | ProjectOp::InsertProcessor { .. }
        )
}

/// `SetStripMute`/`SetStripSolo` gate: the `Strip` entity is always
/// scope-owned, so [`require_scope`] alone cannot see the field mask
/// — the op mutates the strip's singleton Boolean `ParameterTarget`,
/// which is scope-owned only when its field family is declared.
/// An undeclared field rejects `Unsupported` (a capability verdict,
/// matching the mask report); a foreign strip rejects
/// `ScopeViolation` through [`require_scope`].
fn check_strip_bool_op(
    project: &Project,
    scope: &VariantScope,
    strip_id: &StripId,
    field: StripField,
    fields: &StripDomainMask,
    label: &str,
) -> Result<(), VariantError> {
    require_scope(project, scope, ObjectKind::Strip, &strip_id.to_tagged())?;
    if fields.owns(field) {
        Ok(())
    } else {
        Err(VariantError::Unsupported {
            detail: format!("strip {label} is outside the declared domain mask {fields:?}"),
        })
    }
}

/// Strip-fields candidate surface (CB-06): mask-gated channel ops,
/// `InsertProcessor` under `Inserts`, and the shared control-closure
/// ops re-resolved against *this* scope's owned set — channel
/// targets exist as scope objects only for their declared field
/// family, processor control entities only under `Inserts`.
/// Creation ops (`AddParameterTarget`/`AddModulationSource`) get a
/// strip-aware owner check: a strip channel role needs its field
/// declared; any other owner resolves through the scope-derived
/// processor set (chain processors under `Inserts`, empty
/// otherwise). Every routing/send/sidechain/bus op stays
/// `Unsupported` — external wiring is never candidate-owned.
fn check_strip_fields_op_targets(
    project: &Project,
    scope: &VariantScope,
    op: &ProjectOp,
) -> Result<(), VariantError> {
    let VariantScope::StripFields { strip, fields } = scope else {
        return Err(VariantError::InvalidScope {
            detail: format!(
                "strip-fields target check requires a StripFields scope, got {scope:?}"
            ),
        });
    };
    match op {
        ProjectOp::SetStripMute { strip_id, .. } => {
            check_strip_bool_op(project, scope, strip_id, StripField::Mute, fields, "mute")
        }
        ProjectOp::SetStripSolo { strip_id, .. } => {
            check_strip_bool_op(project, scope, strip_id, StripField::Solo, fields, "solo")
        }
        ProjectOp::InsertProcessor { strip_id, .. } => {
            if !fields.owns(StripField::Inserts) {
                return Err(VariantError::Unsupported {
                    detail: format!("insert chain is outside the declared domain mask {fields:?}"),
                });
            }
            let target = project
                .routing
                .strips
                .get(strip_id)
                .ok_or_else(|| Error::NotFound(format!("strip {strip_id}")))?;
            let scoped = project
                .routing
                .strips
                .get(strip)
                .ok_or_else(|| Error::NotFound(format!("strip {strip}")))?;
            if target.chain != scoped.chain {
                return Err(VariantError::ScopeViolation {
                    detail: format!(
                        "insert targets strip {strip_id} on chain {}, outside scoped strip {strip} chain {}",
                        target.chain, scoped.chain
                    ),
                });
            }
            Ok(())
        }
        ProjectOp::AddParameterTarget { owner, role, .. } => match owner {
            ControlOwner::Strip(owner_strip) if *owner_strip == *strip => {
                match strip_channel_field(*role) {
                    Some(field) if fields.owns(field) => Ok(()),
                    Some(_) => Err(VariantError::Unsupported {
                        detail: format!(
                            "strip channel role {role:?} is outside the declared domain mask {fields:?}"
                        ),
                    }),
                    None => Err(VariantError::ScopeViolation {
                        detail: format!(
                            "strip-owned role {role:?} is not a channel field — never scope-owned"
                        ),
                    }),
                }
            }
            _ => {
                let processors = scoped_processor_ids(project, scope)?;
                require_processor_owner(*owner, &processors, scope)
            }
        },
        ProjectOp::AddModulationSource { owner, .. } => match owner {
            // Strip-owned modulation sources are scope-owned for
            // every non-empty mask (`strip_fields_objects` scopes
            // them mask-independently) — the mask filters
            // channel-field *targets*, not strip-level plumbing.
            ControlOwner::Strip(owner_strip) if *owner_strip == *strip => Ok(()),
            _ => {
                let processors = scoped_processor_ids(project, scope)?;
                require_processor_owner(*owner, &processors, scope)
            }
        },
        _ => check_control_closure_op_targets(project, scope, op),
    }
}

/// Pre-edit scope check for ops with explicit targets. Creation ops
/// (`AddClip`, `InsertProcessor`) are verified post-edit instead.
/// [`VariantScope::InstrumentSlot`] and [`VariantScope::ProcessingRange`]
/// dispatch to the processor-owned control surface — shared inputs,
/// chain structure and wiring stay outside every processor candidate.
fn check_op_targets(
    project: &Project,
    scope: &VariantScope,
    op: &ProjectOp,
) -> Result<(), VariantError> {
    match scope {
        VariantScope::InstrumentSlot { .. } => {
            return check_processor_control_op_targets(project, scope, op);
        }
        VariantScope::ProcessingRange { .. } => {
            return check_processing_range_op_targets(project, scope, op);
        }
        VariantScope::StripFields { .. } => {
            return check_strip_fields_op_targets(project, scope, op);
        }
        _ => {}
    }
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
    control_targets: BTreeSet<ParameterTargetId>,
    automation_lanes: BTreeSet<AutomationLaneId>,
    modulation_sources: BTreeSet<ModulationSourceId>,
    modulation_assignments: BTreeSet<ModulationAssignmentId>,
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
            control_targets: project.control.targets.keys().copied().collect(),
            automation_lanes: project.control.lanes.keys().copied().collect(),
            modulation_sources: project.control.sources.keys().copied().collect(),
            modulation_assignments: project.control.assignments.keys().copied().collect(),
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
    // Control-closure creations (instrument-slot candidates): every
    // new target/lane/source/assignment must land inside the scope's
    // owned set — an op minting control entities for another owner
    // fails closed here even if its pre-edit targets resolved.
    for target in candidate.control.targets.keys() {
        if !before.control_targets.contains(target) {
            check(ObjectKind::ParameterTarget, target.to_tagged())?;
        }
    }
    for lane in candidate.control.lanes.keys() {
        if !before.automation_lanes.contains(lane) {
            check(ObjectKind::AutomationLane, lane.to_tagged())?;
        }
    }
    for source in candidate.control.sources.keys() {
        if !before.modulation_sources.contains(source) {
            check(ObjectKind::ModulationSource, source.to_tagged())?;
        }
    }
    for assignment in candidate.control.assignments.keys() {
        if !before.modulation_assignments.contains(assignment) {
            check(ObjectKind::ModulationAssignment, assignment.to_tagged())?;
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
