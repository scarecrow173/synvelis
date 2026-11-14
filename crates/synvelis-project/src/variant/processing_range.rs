//! CB-05 domain adapter for [`VariantScope::ProcessingRange`]
//! (synvelis#282).
//!
//! The adapter keeps the CB-01 contract — candidate payloads are full
//! project clones minus variant state, edits are confined to the
//! declared scope, and adopt materializes only scoped entities through
//! the shared plan/preflight/CAS path. What is range-specific:
//!
//! - **Stable semantic boundaries**: [`ProcessingRangeRef`] identifies
//!   the range by `chain` + stable `ProcessorId` start/end — never
//!   ordinals. Chain edits that add/reorder processors keep the same
//!   processor identities in scope (the scope object set is a set of
//!   stable IDs, so membership is position-independent); deleting
//!   `start`/`end` makes [`scope_objects`] fail `NotFound` — the scope
//!   goes Stale/Unresolved and is never silently re-anchored to the
//!   nearest surviving processor (CB-FX-03).
//! - **Initial boundary contract**: `end` must be the chain tail at
//!   fork (synvelis#282: "End=Chain末尾。任意EndはSchema表現可能だが
//!   本Issueでは初期受入不要"). A mid-chain `end` is schema-
//!   representable but deferred — [`validate_scope`] fails it closed
//!   [`VariantError::Unsupported`] rather than silently widening the
//!   declared range (design §6, CB-PROC-02).
//! - **Typed graph ownership**: [`scope_objects`] owns every processor
//!   in `start..=end` plus their ports and processor-owned control
//!   closure, and only *wholly-internal* `RoutingConnection`s.
//!   Upstream/downstream processors, strip ports, sends, boundary
//!   connections and all other routing state stay shared context —
//!   never scope objects, never candidate-editable (CB-FX-02/04).
//! - **Honest candidate surface**: the processor-owned control ops
//!   shared with CB-04 extend across *every* range processor, and
//!   `InsertProcessor` is accepted because its post-edit scope
//!   re-derivation is unambiguous — a processor landing inside
//!   `start..=end` becomes scope-owned with its ports/control
//!   closure, anything else fails `ScopeViolation`. Removal, reorder,
//!   every connection/send/sidechain op and all shared-context edits
//!   stay `Unsupported` — boundary wiring is never candidate-owned
//!   and scope-entity removal is unsupported this slice
//!   ([`ProcessingRangeCapabilities`]).
//! - **Boundary dependencies**: [`ProcessingRangeAdapter::
//!   boundary_dependencies`] reports every routing connection with
//!   exactly one scope-owned endpoint (today: a sidechain/key feed
//!   into a range processor). Boundary connections stay shared live
//!   context — adopt never touches them, candidate edits can never
//!   rewire them, and they move the shared-context digest so a
//!   changed boundary feed stales the comparison context instead of
//!   producing a false equivalence (CB-FX-04, CB-FX-RCP-01).
//! - **Independent plugin instances**: candidates are separate project
//!   clones — `plugin_instances`, `Processor` entities, bindings and
//!   control state are per-candidate values, so duplicated real
//!   plugin instances are never shared mutable DSP at the project
//!   layer. Opaque state stays in content-addressed
//!   [`PluginStateStore`](crate::plugin_state::PluginStateStore)
//!   manifests; the adapter never deep-inspects blobs —
//!   [`ProcessingRangeAdapter::assess_range_state`] records each
//!   processor's achieved [`StateCaptureQuality`](crate::plugin_state::StateCaptureQuality)
//!   or typed [`PluginStateFailureKind`](crate::plugin_state::PluginStateFailureKind)
//!   verbatim (CB-FX-06).
//! - **Adopt gates**: `preflight_adopt` rejects with
//!   [`VariantError::RecordingActive`] while a recording session is
//!   open on a lane the range's owning strip feeds. Plugin
//!   availability and port rewiring stay with the shared
//!   `preflight_plugins`/`preflight_ports` adopt path; a missing or
//!   degraded plugin never blocks project-truth edits/adopts — it
//!   only withholds Exact claims (CB-FX-05).

use std::collections::BTreeSet;

use crate::{
    Error, IdGenerator, Operation, Project,
    diff::ObjectKind,
    id::{RoutingConnectionId, SemanticPortId},
    plugin_state::{CaptureMode, PluginStatePort},
    routing::ConnectionKind,
    variant::{
        BranchEditOp, DomainAdapter, InstrumentSlotAdapter, InstrumentStateAssessment,
        ProcessingCapability, ProcessingRangeRef, VariantError, VariantScope,
        adapter::{edit_scoped_project_op, fork_candidate_project, is_processing_range_op},
        audio::{lane_recording_open, strip_recording_gate_lanes},
        scope::{range_processors, scope_objects, validate_scope_structure},
    },
};

/// Honest processing-capability report for [`ProcessingRangeAdapter`]
/// candidates (CB-FX-04/06): display truth equals enforcement truth.
/// Every `Supported` verdict has an accepted edit path; every
/// `Unsupported` verdict is rejected typed — unsupported range work
/// is never reported as editable or silently normalized away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessingRangeCapabilities {
    /// `SetParameterBase` — normalized base edit on a range-owned
    /// processor target (rejected by the op itself on `READONLY`
    /// capabilities).
    pub param_base: ProcessingCapability,
    /// `AddParameterTarget` owned by any processor in the range.
    pub param_target_add: ProcessingCapability,
    /// `RemoveParameterTarget` — scope-entity removal is unsupported in
    /// this slice (typed `Unsupported` at the confinement layer).
    pub param_target_remove: ProcessingCapability,
    /// `AddAutomationLane`/`WriteAutomationRegion`/
    /// `EraseAutomationRange` on lanes of range-owned targets.
    pub automation_edit: ProcessingCapability,
    /// `RemoveAutomationLane` — scope-entity removal unsupported.
    pub automation_remove: ProcessingCapability,
    /// Range-processor-owned `AddModulationSource`,
    /// `AddModulationAssignment` and `SetModulationAssignment` whose
    /// source/target/depth-source all stay inside the scoped closure.
    pub modulation_edit: ProcessingCapability,
    /// `RemoveModulationSource`/`RemoveModulationAssignment` —
    /// scope-entity removal unsupported.
    pub modulation_remove: ProcessingCapability,
    /// `InsertProcessor` landing strictly inside `start..=end` — the
    /// new processor, its ports and control closure become
    /// scope-owned; an insert landing outside the range fails
    /// `ScopeViolation` post-edit.
    pub fx_insert_inside: ProcessingCapability,
    /// `RemoveProcessor`/`ReorderProcessor` — chain structure is
    /// shared context, never candidate-owned (reorder would silently
    /// diverge: chain order is not a scope object).
    pub chain_structure: ProcessingCapability,
    /// Routing/connection/send/sidechain/bus edits — boundary and
    /// external wiring stays shared and is never candidate-owned
    /// (CB-FX-04: no unauthorized external routing changes, no
    /// implicit double-mix).
    pub routing: ProcessingCapability,
    /// Plugin rebind — descriptor/version/`capability_revision`
    /// change. No `ProjectOp` mutates `Processor.binding`; a binding
    /// change is a different authored identity, never a candidate
    /// edit.
    pub plugin_rebind: ProcessingCapability,
    /// Preset selection — plugin-internal opaque state; only the
    /// `PluginStatePort` seam can express it, never a `ProjectOp`.
    pub preset_select: ProcessingCapability,
    /// A mid-chain `end` — schema-representable (`ProcessingRangeRef`
    /// keeps stable IDs for any contiguous range) but deferred by the
    /// initial boundary contract; `validate_scope` fails it closed
    /// `Unsupported`.
    pub mid_chain_end: ProcessingCapability,
}

/// A routing connection with exactly one scope-owned endpoint — the
/// boundary dependency a processing range leaves shared (CB-FX-04).
/// Boundary connections are never scope objects: they stay live
/// shared context that adopt never touches and candidate edits can
/// never rewire. The inventory is the typed evidence a comparison or
/// preflight consumer needs to prove there is no silent rewiring and
/// no false audio equivalence — a changed boundary feed moves the
/// shared-context digest and stales the comparison instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessingBoundaryDependency {
    /// The crossing connection.
    pub connection: RoutingConnectionId,
    /// The connection kind (`Send`/`Main` cannot occur today — strip
    /// ports are never scope-owned — but the field stays honest if the
    /// routing model grows processor-owned sources).
    pub kind: ConnectionKind,
    /// `true` when the scope-owned endpoint is the connection source;
    /// `false` when it is the destination (today always `false`: only
    /// strip outputs are connection sources, and strips are never
    /// range-owned — so a boundary connection is an external feed
    /// *into* the range, e.g. a sidechain key).
    pub source_in_scope: bool,
}

/// Domain adapter for [`VariantScope::ProcessingRange`]: processor-
/// owned control edits plus in-range processor insertion on
/// full-project candidates. The scoped `Processor`s, their
/// `SemanticPort`s, wholly-internal `RoutingConnection`s and their
/// control closures transplant through the shared adopt surface;
/// `plugin_instances` stays live-side — the apply write path re-syncs
/// it from the transplanted `Processor` binding (`sync_processor`:
/// unchanged plugin keeps its id, rebind mints a new one, unresolved
/// drops it).
#[derive(Clone, Debug, Default)]
pub struct ProcessingRangeAdapter;

impl ProcessingRangeAdapter {
    /// The adapter singleton.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The capability report candidate surfaces display (CB-FX-04/06).
    /// Every `Supported` verdict has an accepted edit path; every
    /// `Unsupported` verdict is rejected typed by the confinement
    /// layer.
    #[must_use]
    pub fn capabilities(&self) -> ProcessingRangeCapabilities {
        ProcessingRangeCapabilities {
            param_base: ProcessingCapability::Supported,
            param_target_add: ProcessingCapability::Supported,
            param_target_remove: ProcessingCapability::Unsupported,
            automation_edit: ProcessingCapability::Supported,
            automation_remove: ProcessingCapability::Unsupported,
            modulation_edit: ProcessingCapability::Supported,
            modulation_remove: ProcessingCapability::Unsupported,
            fx_insert_inside: ProcessingCapability::Supported,
            chain_structure: ProcessingCapability::Unsupported,
            routing: ProcessingCapability::Unsupported,
            plugin_rebind: ProcessingCapability::Unsupported,
            preset_select: ProcessingCapability::Unsupported,
            mid_chain_end: ProcessingCapability::Unsupported,
        }
    }

    /// Boundary dependencies of `range` inside `project`: every routing
    /// connection with exactly one scope-owned endpoint, in stable
    /// connection-id order (CB-FX-04). Wholly-internal connections are
    /// scope objects and wholly-external ones are plain shared
    /// context — only the crossing set is reported. Detection errors
    /// propagate when the range itself no longer resolves (deleted
    /// `start`/`end` → `NotFound`, never a guessed re-anchor).
    ///
    /// # Errors
    /// [`Error::NotFound`]/[`Error::Validation`] from scope derivation.
    pub fn boundary_dependencies(
        project: &Project,
        range: &ProcessingRangeRef,
    ) -> Result<Vec<ProcessingBoundaryDependency>, Error> {
        let scope = VariantScope::ProcessingRange { range: *range };
        let objects = scope_objects(project, &scope)?;
        let owned_ports: BTreeSet<SemanticPortId> = objects
            .iter()
            .filter(|o| o.kind == ObjectKind::SemanticPort)
            .filter_map(|o| SemanticPortId::from_tagged(&o.stable_id).ok())
            .collect();
        let mut out = Vec::new();
        for conn in project.routing.connections.values() {
            let source_in_scope = owned_ports.contains(&conn.source);
            let dest_in_scope = owned_ports.contains(&conn.destination);
            if source_in_scope == dest_in_scope {
                continue;
            }
            out.push(ProcessingBoundaryDependency {
                connection: conn.id,
                kind: conn.kind,
                source_in_scope,
            });
        }
        Ok(out)
    }

    /// Probes `port` for every processor in `range` and records each
    /// achieved [`StateCaptureQuality`](crate::plugin_state::StateCaptureQuality)
    /// or typed [`PluginStateFailureKind`](crate::plugin_state::PluginStateFailureKind)
    /// verbatim — the comparison/adopt honesty seam for CB-FX-06
    /// (same TIM-04 vocabulary as the CB-04 instrument assessment;
    /// results follow chain order). The blobs themselves are
    /// content-digested for identity only; the adapter never inspects
    /// plugin-internal state.
    ///
    /// # Errors
    /// [`Error::NotFound`]/[`Error::Validation`] when the range no
    /// longer resolves.
    pub fn assess_range_state(
        port: &mut dyn PluginStatePort,
        project: &Project,
        range: &ProcessingRangeRef,
        mode: CaptureMode,
    ) -> Result<Vec<InstrumentStateAssessment>, Error> {
        Ok(range_processors(project, range)?
            .into_iter()
            .map(|processor| InstrumentSlotAdapter::assess_state(port, processor, mode))
            .collect())
    }
}

impl DomainAdapter for ProcessingRangeAdapter {
    fn name(&self) -> &'static str {
        "processing-range"
    }

    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError> {
        let VariantScope::ProcessingRange { range } = scope else {
            return Err(VariantError::InvalidScope {
                detail: format!(
                    "processing-range adapter requires a ProcessingRange scope, got {scope:?}"
                ),
            });
        };
        validate_scope_structure(project, scope).map_err(VariantError::from)?;
        // Initial boundary contract (design §6 / synvelis#282): `end`
        // is the chain tail — "Track Strip FX Chain の入口から
        // Processor 間を経て Chain 末尾まで". Arbitrary mid-chain
        // ends are schema-representable (`ProcessingRangeRef` keeps
        // stable IDs for any contiguous range) but deferred: fail
        // closed `Unsupported`, never silently accept a wider or
        // narrower declared range than the contract defines. This is
        // a fork-entry gate — stable-ID resolution keeps an existing
        // scope valid when the chain later grows past `end`.
        let chain = project
            .routing
            .chains
            .get(&range.chain)
            .ok_or_else(|| Error::NotFound(format!("chain {}", range.chain)))?;
        if chain.processors.last() != Some(&range.end) {
            return Err(VariantError::Unsupported {
                detail: format!(
                    "processing range end {} is not the tail of chain {} — \
                     arbitrary mid-chain ends are deferred by the initial boundary contract",
                    range.end, range.chain
                ),
            });
        }
        // Full object-set derivation enforces the processor + port +
        // control-closure + internal-connection ownership contract.
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
                detail: format!("processing-range candidates accept project ops only: {op:?}"),
            });
        };
        // The op's own contract — READONLY rejection, discrete-kind
        // modulation refusal, TimedEvent capability, insert index
        // bounds — runs here because the shared confined flow applies
        // without `Operation::validate` (the controller's
        // validate-then-apply order is reproduced explicitly for the
        // accepted surface only).
        if is_processing_range_op(project_op) {
            project_op.validate(candidate)?;
        }
        edit_scoped_project_op(candidate, scope, project_op, ids, None)
    }

    fn preflight_adopt(
        &self,
        live: &Project,
        _candidate: &Project,
        scope: &VariantScope,
    ) -> Result<(), VariantError> {
        let VariantScope::ProcessingRange { range } = scope else {
            return Ok(());
        };
        // The range's owning chain resolves to its strip; a missing
        // chain/processor is reported by the adopt planner's own
        // scope derivation — the recording gate only applies to a
        // live range. Plugin availability/port rewiring stay with the
        // shared adopt preflights — a missing plugin never blocks
        // project truth, it only withholds Exact claims (CB-FX-05).
        if let Some(strip) = live
            .routing
            .strips
            .values()
            .find(|s| s.chain == range.chain)
        {
            for lane in strip_recording_gate_lanes(live, strip.id) {
                if lane_recording_open(live, lane) {
                    return Err(VariantError::RecordingActive {
                        detail: format!(
                            "recording session open on lane {lane} feeding processing range {}..={}",
                            range.start, range.end
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}
