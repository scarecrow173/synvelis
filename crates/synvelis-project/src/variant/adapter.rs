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
    apply::entity,
    comp::CompOp,
    control::ControlOwner,
    diff::{ObjectKind, SemanticObjectRef},
    id::{
        AutomationLaneId, ClipId, LaneId, ModulationAssignmentId, ModulationSourceId,
        ParameterTargetId, ProcessorId, RoutingConnectionId, SemanticPortId, SendId, StripId,
        TrackId,
    },
    ops::clip_beats,
    placement::TimeAnchor,
    routing::{ConnectionKind, PortOwner},
    time::MusicalTick,
    variant::{
        VariantError,
        scope::{scope_objects, scope_value, strip_channel_field, validate_scope_structure},
        types::{ArrangementDomainMask, StripDomainMask, StripField, VariantScope},
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
            | ProjectOp::SetProcessorBinding { .. }
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

/// `true` when `port`'s owner is the scoped strip — or, for
/// non-send routes, a processor the scope owns. This mirrors the
/// source-side ownership rule in `scope::strip_fields_objects`: a
/// candidate may author the outgoing edge of its own strip/chain but
/// never an edge sourced outside the scope.
fn strip_routing_source(
    project: &Project,
    scope: &VariantScope,
    strip: &StripId,
    port_id: SemanticPortId,
    allow_processor: bool,
) -> Result<bool, VariantError> {
    let Some(port) = project.routing.ports.get(&port_id) else {
        return Ok(false);
    };
    match port.owner {
        PortOwner::Strip(owner) => Ok(owner == *strip),
        PortOwner::Processor(owner) if allow_processor => {
            let processors = scoped_processor_ids(project, scope)?;
            Ok(processors.contains(&owner))
        }
        PortOwner::Processor(_) => Ok(false),
    }
}

/// Rejects an op whose declared `StripField` family is absent from
/// the scope's domain mask — the mask-relative capability verdict.
fn require_strip_field(
    fields: &StripDomainMask,
    field: StripField,
    family: &str,
) -> Result<(), VariantError> {
    if fields.owns(field) {
        Ok(())
    } else {
        Err(VariantError::Unsupported {
            detail: format!("{family} edits are outside the declared domain mask {fields:?}"),
        })
    }
}

/// The `StripField` family a routing [`ConnectionKind`] belongs to —
/// the mask-relative gate for connection/send ops (synvelis#550).
fn routing_field(kind: &ConnectionKind) -> StripField {
    match kind {
        ConnectionKind::Send(_) => StripField::Sends,
        ConnectionKind::Main => StripField::MainRoute,
        ConnectionKind::Sidechain => StripField::Sidechain,
    }
}

/// synvelis#550 routing surface: mask-gated by the field family the
/// op's routing kind belongs to, then confined by scope ownership —
/// a foreign send/connection/bus fails `ScopeViolation`, an
/// undeclared field family fails `Unsupported` (the
/// mask-relative capability verdict). Returns `None` for
/// non-routing ops so the caller's match stays flat.
fn check_strip_routing_op_targets(
    project: &Project,
    scope: &VariantScope,
    strip: &StripId,
    fields: &StripDomainMask,
    op: &ProjectOp,
) -> Option<Result<(), VariantError>> {
    let result = match op {
        ProjectOp::AddSend {
            source,
            gain_target,
            ..
        } => require_strip_field(fields, StripField::Sends, "sends").and_then(|()| {
            if !strip_routing_source(project, scope, strip, *source, false)? {
                return Err(VariantError::ScopeViolation {
                    detail: format!(
                        "send source port {source} is not owned by scoped strip {strip}"
                    ),
                });
            }
            // Apply re-owns the gain target to the send — adopting a
            // target outside the scope would transplant foreign
            // ownership into live at adopt, so the target must be
            // scope-owned (the send control closure).
            if let Some(target) = gain_target {
                require_scope(
                    project,
                    scope,
                    ObjectKind::ParameterTarget,
                    &target.to_tagged(),
                )?;
            }
            Ok(())
        }),
        ProjectOp::RemoveSend { send_id } | ProjectOp::SetSendTap { send_id, .. } => {
            require_strip_field(fields, StripField::Sends, "sends").and_then(|()| {
                require_scope(project, scope, ObjectKind::SendConfig, &send_id.to_tagged())
            })
        }
        ProjectOp::SetSendGainTarget {
            send_id,
            gain_target,
        } => require_strip_field(fields, StripField::Sends, "sends").and_then(|()| {
            require_scope(project, scope, ObjectKind::SendConfig, &send_id.to_tagged())?;
            // Same confinement as `AddSend` — apply re-owns the
            // target, so it must already be scope-owned.
            if let Some(target) = gain_target {
                require_scope(
                    project,
                    scope,
                    ObjectKind::ParameterTarget,
                    &target.to_tagged(),
                )?;
            }
            // synvelis#552: a rebind to a *different* target is a
            // positive edit — apply releases the previous `SendLevel`
            // seal (target + its control closure) inside the same
            // apply, and the departed scope objects land in the
            // removal check as supported deletions. Re-asserting the
            // bound target or clearing to `None` release identically.
            Ok(())
        }),
        ProjectOp::AddConnection { source, kind, .. } => {
            require_strip_field(fields, routing_field(kind), "connections").and_then(|()| {
                if strip_routing_source(project, scope, strip, *source, true)? {
                    Ok(())
                } else {
                    Err(VariantError::ScopeViolation {
                        detail: format!(
                            "connection source port {source} is outside the scoped strip's routing"
                        ),
                    })
                }
            })
        }
        ProjectOp::RemoveConnection { connection_id }
        | ProjectOp::SetConnectionEnabled { connection_id, .. }
        | ProjectOp::SetConnectionChannelMap { connection_id, .. } => {
            let Some(conn) = project.routing.connections.get(connection_id) else {
                return Some(Err(
                    Error::NotFound(format!("connection {connection_id}")).into()
                ));
            };
            require_strip_field(fields, routing_field(&conn.kind), "connections").and_then(|()| {
                require_scope(
                    project,
                    scope,
                    ObjectKind::RoutingConnection,
                    &connection_id.to_tagged(),
                )
            })
        }
        ProjectOp::ConfigureBus { bus_id, .. } => {
            require_strip_field(fields, StripField::Bus, "bus")
                .and_then(|()| require_scope(project, scope, ObjectKind::Bus, &bus_id.to_tagged()))
        }
        _ => return None,
    };
    Some(result)
}

/// Strip-fields scope-removal policy (synvelis#550): a scope object
/// may leave the owned set only through a real deletion whose kind
/// the apply surface can materialize — every removed object must be
/// absent from the post-edit candidate (a move-out is rejected) and
/// of a removal-supported kind. `RemoveProcessor` and other
/// structure ops either fail the reference oracle earlier or land
/// here as typed `Unsupported` — required-closure deletions the op
/// surface cannot express stay rejected, never cascaded silently.
pub(crate) fn check_strip_fields_scope_removals(
    candidate: &Project,
    removed: &BTreeSet<SemanticObjectRef>,
) -> Result<(), VariantError> {
    for object in removed {
        if !entity::removal_supported_kind(object.kind) {
            return Err(VariantError::Unsupported {
                detail: format!(
                    "scope entity removal is not supported for {:?}: {}",
                    object.kind, object.stable_id
                ),
            });
        }
        if scope_value(candidate, object)
            .map_err(VariantError::from)?
            .is_some()
        {
            return Err(VariantError::Unsupported {
                detail: format!(
                    "scope entity removal requires the entity itself to be deleted: {}",
                    object.stable_id
                ),
            });
        }
    }
    Ok(())
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
        // Plugin rebind (sv#513): the processor entity itself is
        // scope-owned on every processor scope, so the binding edit
        // confines exactly like a control-closure target. Ports,
        // chain membership and wiring are untouched by the op.
        ProjectOp::SetProcessorBinding { processor_id, .. } => require_scope(
            project,
            scope,
            ObjectKind::Processor,
            &processor_id.to_tagged(),
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
                | ProjectOp::AddSend { .. }
                | ProjectOp::RemoveSend { .. }
                | ProjectOp::SetSendTap { .. }
                | ProjectOp::SetSendGainTarget { .. }
                | ProjectOp::AddConnection { .. }
                | ProjectOp::RemoveConnection { .. }
                | ProjectOp::SetConnectionEnabled { .. }
                | ProjectOp::SetConnectionChannelMap { .. }
                | ProjectOp::ConfigureBus { .. }
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
/// otherwise). Routing ops are mask-gated per field family and
/// confined source-side (synvelis#550): sends, main routes and
/// sidechains authored by the scoped strip or a scope-owned chain
/// processor; incoming/foreign-source edges and undeclared families
/// stay `Unsupported`/`ScopeViolation` — external wiring is never
/// candidate-owned.
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
    // synvelis#550 routing surface — delegated wholesale; the helper
    // returns `None` for non-routing ops.
    if let Some(result) = check_strip_routing_op_targets(project, scope, strip, fields, op) {
        return result;
    }
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
        ProjectOp::SetProcessorBinding { processor_id, .. } => {
            // Processor entities are scope-owned only under `Inserts`
            // — same mask gate as `InsertProcessor`.
            if !fields.owns(StripField::Inserts) {
                return Err(VariantError::Unsupported {
                    detail: format!(
                        "processor binding edits are outside the declared domain mask {fields:?}"
                    ),
                });
            }
            require_scope(
                project,
                scope,
                ObjectKind::Processor,
                &processor_id.to_tagged(),
            )
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
            // Send-level targets join the send's control closure —
            // legal only for a scope-owned send under `Sends` (the
            // issue-#122 create-then-bind flow).
            ControlOwner::Send(send_id) => {
                require_strip_field(fields, StripField::Sends, "sends")?;
                require_scope(project, scope, ObjectKind::SendConfig, &send_id.to_tagged())
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
            ControlOwner::Send(send_id) => {
                require_strip_field(fields, StripField::Sends, "sends")?;
                require_scope(project, scope, ObjectKind::SendConfig, &send_id.to_tagged())
            }
            _ => {
                let processors = scoped_processor_ids(project, scope)?;
                require_processor_owner(*owner, &processors, scope)
            }
        },
        _ => check_control_closure_op_targets(project, scope, op),
    }
}

/// `true` for the `ProjectOp` variants the arrangement candidate
/// surface accepts — the same list [`check_arrangement_op_targets`]
/// confines. Kept adjacent so the adapter's validate-gate and the
/// confinement gate cannot drift.
pub(crate) fn is_arrangement_op(op: &ProjectOp) -> bool {
    matches!(
        op,
        ProjectOp::AddClip { .. }
            | ProjectOp::RemoveClip { .. }
            | ProjectOp::SplitClip { .. }
            | ProjectOp::MoveClip { .. }
            | ProjectOp::SetClipExtent { .. }
            | ProjectOp::SetClipPlayback { .. }
            | ProjectOp::SetClipMuted { .. }
            | ProjectOp::RemoveClipsInRange { .. }
            | ProjectOp::AddNote { .. }
            | ProjectOp::RemoveNote { .. }
    )
}

/// Lanes owned by the arrangement scope's selected tracks — the
/// confinement set `AddClip`/`RemoveClipsInRange` placements resolve
/// against. Track/lane structure itself is never scope-owned; only
/// the clips overlapping `time` on these lanes are.
fn arrangement_scoped_lanes(project: &Project, tracks: &[TrackId]) -> BTreeSet<LaneId> {
    tracks
        .iter()
        .filter_map(|track| project.tracks.get(track))
        .flat_map(|track| track.lane_ids.iter().copied())
        .collect()
}

/// `fields.clips`/`fields.notes` capability verdict for an op family
/// the scope did not declare — `Unsupported`, matching the mask
/// report (a capability verdict, never a silent normalize).
fn require_arrangement_mask(
    fields: ArrangementDomainMask,
    family: &str,
    declared: bool,
) -> Result<(), VariantError> {
    if declared {
        Ok(())
    } else {
        Err(VariantError::Unsupported {
            detail: format!("{family} ops are outside the declared arrangement mask {fields:?}"),
        })
    }
}

/// Arrangement candidate surface (CB-07): clip-structure ops under
/// the `clips` mask, clip-local note ops under the `notes` mask —
/// every other `ProjectOp`/`CompOp` stays `Unsupported` (typed
/// capability verdict). Entity-targeted ops resolve through
/// [`require_scope`] against the range-derived object set (foreign
/// target → `ScopeViolation`, owned-but-undeclared mask →
/// `Unsupported`, matching the strip bool-op precedent). Creation
/// and sweep ops confine their *placement* instead: `AddClip` needs
/// a scoped-track lane and `RemoveClipsInRange` needs scoped lanes
/// plus a `range` contained in `time` — containment, not overlap,
/// because a wider sweep would delete out-of-scope clips the
/// object-set diff cannot see. Whether the resulting clip actually
/// lands inside `time` is verified post-edit by the created-within-
/// scope check.
fn check_arrangement_op_targets(
    project: &Project,
    scope: &VariantScope,
    op: &ProjectOp,
) -> Result<(), VariantError> {
    let VariantScope::Arrangement {
        tracks,
        time,
        fields,
    } = scope
    else {
        return Err(VariantError::InvalidScope {
            detail: format!(
                "arrangement target check requires an Arrangement scope, got {scope:?}"
            ),
        });
    };
    match op {
        // Mask verdict precedes target confinement everywhere: an op
        // family the scope did not declare is `Unsupported` regardless
        // of where its target sits — matching the mask-relative
        // capability report. A declared family with a foreign target
        // still fails `ScopeViolation`.
        ProjectOp::SetClipMuted { clip_id, .. }
        | ProjectOp::MoveClip { clip_id, .. }
        | ProjectOp::SetClipExtent { clip_id, .. }
        | ProjectOp::SetClipPlayback { clip_id, .. }
        | ProjectOp::RemoveClip { clip_id } => {
            require_arrangement_mask(*fields, "clip", fields.clips)?;
            require_scope(project, scope, ObjectKind::Clip, &clip_id.to_tagged())
        }
        ProjectOp::SplitClip { clip_id, at, .. } => {
            require_arrangement_mask(*fields, "clip", fields.clips)?;
            require_scope(project, scope, ObjectKind::Clip, &clip_id.to_tagged())?;
            if !fields.notes {
                check_split_note_migration(project, *clip_id, *at)?;
            }
            Ok(())
        }
        ProjectOp::AddNote { clip, .. } => {
            require_arrangement_mask(*fields, "note", fields.notes)?;
            require_scope(project, scope, ObjectKind::Clip, &clip.to_tagged())
        }
        ProjectOp::RemoveNote { note } => {
            require_arrangement_mask(*fields, "note", fields.notes)?;
            require_scope(project, scope, ObjectKind::Note, &note.to_tagged())
        }
        ProjectOp::AddClip { lane_id, .. } => {
            require_arrangement_mask(*fields, "clip", fields.clips)?;
            if arrangement_scoped_lanes(project, tracks).contains(lane_id) {
                Ok(())
            } else {
                Err(VariantError::ScopeViolation {
                    detail: format!("add-clip lane {lane_id} is outside the scoped tracks' lanes"),
                })
            }
        }
        ProjectOp::RemoveClipsInRange { lanes, range } => {
            require_arrangement_mask(*fields, "clip", fields.clips)?;
            let scoped = arrangement_scoped_lanes(project, tracks);
            for lane in lanes {
                if !scoped.contains(lane) {
                    return Err(VariantError::ScopeViolation {
                        detail: format!("sweep lane {lane} is outside the scoped tracks' lanes"),
                    });
                }
            }
            if time.start() <= range.start() && range.end() <= time.end() {
                Ok(())
            } else {
                Err(VariantError::ScopeViolation {
                    detail: format!(
                        "sweep range [{}, {}) exceeds the declared arrangement range [{}, {})",
                        range.start().value(),
                        range.end().value(),
                        time.start().value(),
                        time.end().value()
                    ),
                })
            }
        }
        other => Err(VariantError::Unsupported {
            detail: format!(
                "edit op is outside the supported arrangement candidate surface: {other:?}"
            ),
        }),
    }
}

/// `SplitClip` repartitions clip-local notes onto the new clip — a
/// mutation of entities the `clips` mask alone does not own. Under a
/// `clips`-only scope those notes sit outside the scope object set,
/// so a migration the candidate expresses but adopt cannot
/// materialize would silently diverge live from the adopted branch.
/// Reject typed when any note sits at or past the split point
/// (straddles are already refused by op validation); notes fully
/// before the split stay on the kept clip and need no `notes`
/// surface.
fn check_split_note_migration(
    project: &Project,
    clip_id: ClipId,
    at: MusicalTick,
) -> Result<(), VariantError> {
    let Some(clip) = project.clips.get(&clip_id) else {
        return Ok(());
    };
    let TimeAnchor::Musical(start) = clip.placement.anchor else {
        return Ok(());
    };
    let offset = clip_beats(project, at.ticks_since(start)?)?;
    if project
        .notes
        .values()
        .any(|note| note.clip == clip_id && note.start >= offset)
    {
        return Err(VariantError::Unsupported {
            detail: format!(
                "split of clip {clip_id} migrates clip-local notes past the split \
                 point; the notes mask is not declared"
            ),
        });
    }
    Ok(())
}

/// Pre-edit scope check for ops with explicit targets. Creation ops
/// (`AddClip`, `InsertProcessor`) are verified post-edit instead.
/// [`VariantScope::InstrumentSlot`], [`VariantScope::ProcessingRange`],
/// [`VariantScope::StripFields`] and [`VariantScope::Arrangement`]
/// dispatch to their domain surfaces — shared inputs, chain
/// structure and wiring stay outside every candidate.
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
        VariantScope::Arrangement { .. } => {
            return check_arrangement_op_targets(project, scope, op);
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
    connections: BTreeSet<RoutingConnectionId>,
    sends: BTreeSet<SendId>,
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
            connections: project.routing.connections.keys().copied().collect(),
            sends: project.routing.sends.keys().copied().collect(),
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
    // Routing creations (synvelis#550 strip candidates): `AddSend`/
    // `AddConnection` mint edges and send configs — both must land
    // inside the scope's owned set.
    for connection in candidate.routing.connections.keys() {
        if !before.connections.contains(connection) {
            check(ObjectKind::RoutingConnection, connection.to_tagged())?;
        }
    }
    for send in candidate.routing.sends.keys() {
        if !before.sends.contains(send) {
            check(ObjectKind::SendConfig, send.to_tagged())?;
        }
    }
    Ok(())
}

/// Optional domain capability guard applied before a confined
/// project-op candidate edit.
pub(crate) type OpGuard<'a> = Option<&'a dyn Fn(&ProjectOp) -> Result<(), VariantError>>;

/// Post-edit scope-removal policy for range-scoped candidates: given
/// the candidate *after* the edit and the scope objects the edit
/// removed, decide whether the loss is an authorized candidate edit
/// or an untracked live mutation. `None` keeps the entity-rooted
/// invariant — any scope-object loss fails `Unsupported`. The CB-07
/// arrangement adapter supplies its own rule so in-range clip
/// deletion stays expressible while move-outs remain rejected.
pub(crate) type ScopeRemovalCheck =
    fn(&Project, &BTreeSet<SemanticObjectRef>) -> Result<(), VariantError>;

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
    edit_scoped_project_op_with_removal(candidate, scope, op, ids, guard, None)
}

/// [`edit_scoped_project_op`] with an explicit scope-removal policy —
/// the seam range-scoped domains (CB-07 arrangement) use to express
/// in-range deletions the entity-rooted invariant would reject.
/// Every other confinement step is identical.
pub(crate) fn edit_scoped_project_op_with_removal(
    candidate: &mut Project,
    scope: &VariantScope,
    op: &ProjectOp,
    ids: &mut dyn IdGenerator,
    guard: OpGuard<'_>,
    removal: Option<ScopeRemovalCheck>,
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
    let removed: BTreeSet<SemanticObjectRef> =
        before_objects.difference(&after_objects).cloned().collect();
    match removal {
        Some(check) => check(candidate, &removed)?,
        None => {
            // Scope removals are unsupported in this slice — fail closed.
            if let Some(first) = removed.iter().next() {
                return Err(VariantError::Unsupported {
                    detail: format!(
                        "scope entity removal is not supported in this slice: {}",
                        first.stable_id
                    ),
                });
            }
        }
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
