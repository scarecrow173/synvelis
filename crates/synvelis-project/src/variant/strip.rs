//! CB-06 domain adapter for [`VariantScope::StripFields`]
//! (synvelis#283).
//!
//! The adapter keeps the CB-01 contract — candidate payloads are full
//! project clones minus variant state, edits are confined to the
//! declared scope, and adopt materializes only scoped entities through
//! the shared plan/preflight/CAS path. What is strip-specific:
//!
//! - **Stable strip identity**: [`VariantScope::StripFields`] roots at
//!   a `StripId` — the same stable identity a `Track` or a `Bus`
//!   resolves to through `Strip.owner`. Track-owned and bus-owned
//!   strips share one adapter path; no second DSP strip is ever
//!   instantiated for a track display (CB-STRIP-01).
//! - **Declared field mask**: [`StripDomainMask`] enumerates the owned
//!   field families (`Fader`/`Pan`/`Mute`/`Solo`/`Inserts`). Channel
//!   fields own the matching singleton `ParameterTarget`
//!   (`ControlOwner::Strip`) plus its automation/modulation closure;
//!   `Inserts` owns the strip's whole insert chain — the
//!   `ProcessorChain` entity itself (the sole carrier of DSP order,
//!   so a live reorder stales the scope digest instead of merging
//!   positionally), its processors, ports, processor-owned control
//!   closures and wholly-internal connections. [`scope_objects`] is
//!   the single ownership oracle:
//!   the adapter never re-derives ownership, it only gates ops
//!   against it (CB-STRIP-02).
//! - **Mask-exact edit surface**: `SetStripMute`/`SetStripSolo` are
//!   gated on their declared field (the `Strip` entity is always
//!   scope-owned, so entity confinement alone cannot see the mask);
//!   `InsertProcessor` requires `Inserts` and the scoped strip's
//!   chain; every other accepted op is a control-closure op whose
//!   targets must resolve inside `scope_objects` — an unowned-field
//!   channel target or a non-`Inserts` processor owner fails typed
//!   (CB-STRIP-02/03). All routing/send/sidechain/bus/monitor ops
//!   stay `Unsupported` — external wiring is never candidate-owned.
//! - **Typed routing boundary**: strip ports are *affiliated* with
//!   the scope (the strip entity is scope-owned) but never scope
//!   objects. [`StripFieldsAdapter::boundary_dependencies`] reports
//!   every routing connection touching a scope-affiliated
//!   `SemanticPortId` endpoint that is not wholly scope-owned —
//!   the strip's main route, sends, sidechain feeds and any
//!   strip↔processor crossing. Boundary connections stay shared live
//!   context: adopt never touches them, candidate edits can never
//!   rewire them, and a changed boundary feed moves the
//!   shared-context digest so the comparison stales instead of
//!   producing false equivalence (CB-STRIP-03/RCP-01).
//! - **Honest candidate surface**: [`StripFieldsCapabilities`] is
//!   computed from the declared mask so display truth equals
//!   enforcement truth — every `Supported` verdict has an accepted
//!   edit path, every `Unsupported` verdict is rejected typed
//!   (CB-STRIP-02).
//! - **Adopt gates**: `preflight_adopt` rejects with
//!   [`VariantError::RecordingActive`] while a recording session is
//!   open on a lane the strip feeds (track strip → owning track's
//!   lanes; bus strip → every project lane, fail-closed). Plugin
//!   availability and port rewiring stay with the shared adopt
//!   preflights: a missing plugin fails adopt materialization typed
//!   (`MissingPlugin`) before any mutation, while candidate edits —
//!   project truth — proceed regardless and availability only
//!   withholds Exact claims in state assessment (CB-STRIP-04).

use std::collections::BTreeSet;

use crate::{
    Error, IdGenerator, Operation, Project,
    diff::ObjectKind,
    id::{RoutingConnectionId, SemanticPortId, StripId},
    plugin_state::{CaptureMode, PluginStatePort},
    routing::{ConnectionKind, PortOwner},
    variant::{
        BranchEditOp, DomainAdapter, InstrumentSlotAdapter, InstrumentStateAssessment,
        ProcessingCapability, StripDomainMask, StripField, VariantError, VariantScope,
        adapter::{edit_scoped_project_op, fork_candidate_project, is_strip_fields_op},
        audio::{lane_recording_open, strip_recording_gate_lanes},
        scope::{scope_objects, validate_scope_structure},
    },
};

/// Honest capability report for [`StripFieldsAdapter`] candidates,
/// computed from the declared [`StripDomainMask`] (CB-STRIP-02):
/// display truth equals enforcement truth. Every `Supported` verdict
/// has an accepted edit path; every `Unsupported` verdict is
/// rejected typed — mask-external work is never reported as editable
/// or silently normalized away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripFieldsCapabilities {
    /// `Fader` field — `SetParameterBase`/automation/modulation on
    /// the strip's Fader target (seeded at strip creation).
    pub fader: ProcessingCapability,
    /// `Pan` field — same surface as `fader`; the `Pan` target is
    /// not seeded, it registers through `AddParameterTarget` on
    /// first explicit use (issue #122 contract).
    pub pan: ProcessingCapability,
    /// `Mute` field — `SetStripMute` plus target/automation/modulation
    /// edits on the strip's Mute target.
    pub mute: ProcessingCapability,
    /// `Solo` field — `SetStripSolo` plus the same target surface.
    pub solo: ProcessingCapability,
    /// `Inserts` field — `InsertProcessor` into the scoped strip's
    /// chain plus processor-owned control ops across every chain
    /// processor.
    pub inserts: ProcessingCapability,
    /// Automation lanes/regions on scope-owned targets — `Supported`
    /// for every non-empty mask: at least one field family always
    /// owns a target surface (target *existence* is project state,
    /// not capability — `AddParameterTarget`/`InsertProcessor` are
    /// the creation paths).
    pub automation_edit: ProcessingCapability,
    /// Modulation sources/assignments inside the scope closure —
    /// same rule as `automation_edit`.
    pub modulation_edit: ProcessingCapability,
    /// Scope-entity removal (targets/lanes/sources/assignments/
    /// processors) — unsupported this slice, typed `Unsupported` at
    /// the confinement layer.
    pub entity_remove: ProcessingCapability,
    /// Routing/send/sidechain/bus/monitor ops — external wiring is
    /// shared context and never candidate-owned (CB-STRIP-03).
    pub routing: ProcessingCapability,
    /// Plugin rebind — descriptor/version/`capability_revision`
    /// change. No `ProjectOp` mutates `Processor.binding`; a binding
    /// change is a different authored identity, never a candidate
    /// edit.
    pub plugin_rebind: ProcessingCapability,
    /// Preset selection — plugin-internal opaque state; only the
    /// `PluginStatePort` seam can express it, never a `ProjectOp`.
    pub preset_select: ProcessingCapability,
}

/// A routing connection touching the strip scope's boundary — the
/// typed evidence a comparison or preflight consumer needs to prove
/// there is no silent rewiring and no false audio equivalence
/// (CB-STRIP-03). Boundary connections are never scope objects:
/// they stay live shared context that adopt never touches and
/// candidate edits can never rewire; a changed boundary feed moves
/// the shared-context digest and stales the comparison instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripBoundaryDependency {
    /// The crossing connection.
    pub connection: RoutingConnectionId,
    /// The connection kind (`Main`/`Send`/`Sidechain`).
    pub kind: ConnectionKind,
    /// `true` when the source endpoint port is scope-affiliated —
    /// owned by the scoped strip or by a scope-owned (chain)
    /// processor. Strip ports are scope-affiliated identity ports of
    /// a scope-owned entity but never scope objects themselves.
    pub source_in_scope: bool,
    /// `true` when the destination endpoint is likewise
    /// scope-affiliated.
    pub destination_in_scope: bool,
}

/// Domain adapter for [`VariantScope::StripFields`]: mask-gated
/// channel-field edits, insert-chain edits under `Inserts`, and the
/// shared control-closure surface on full-project candidates. The
/// scoped `Strip`, its declared channel `ParameterTarget`s and their
/// automation/modulation closure — plus, under `Inserts`, the
/// `ProcessorChain` entity (DSP order is scope truth: adopt rejects
/// a live reorder as `StaleScope` rather than positionally merging
/// into an order nobody authored), every chain `Processor`, its
/// `SemanticPort`s, wholly-internal `RoutingConnection`s and
/// processor control closures — transplant through the shared adopt
/// surface; `plugin_instances` stays
/// live-side (the apply write path re-syncs it from the transplanted
/// `Processor` binding).
#[derive(Clone, Debug, Default)]
pub struct StripFieldsAdapter;

impl StripFieldsAdapter {
    /// The adapter singleton.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The capability report for a scope declaring `fields`
    /// (CB-STRIP-02). Every `Supported` verdict has an accepted edit
    /// path; every `Unsupported` verdict is rejected typed by the
    /// confinement layer. The report is mask-relative because the
    /// declared [`StripDomainMask`] *is* the candidate contract.
    #[must_use]
    pub fn capabilities(&self, fields: &StripDomainMask) -> StripFieldsCapabilities {
        let field_cap = |field: StripField| {
            if fields.owns(field) {
                ProcessingCapability::Supported
            } else {
                ProcessingCapability::Unsupported
            }
        };
        StripFieldsCapabilities {
            fader: field_cap(StripField::Fader),
            pan: field_cap(StripField::Pan),
            mute: field_cap(StripField::Mute),
            solo: field_cap(StripField::Solo),
            inserts: field_cap(StripField::Inserts),
            automation_edit: ProcessingCapability::Supported,
            modulation_edit: ProcessingCapability::Supported,
            entity_remove: ProcessingCapability::Unsupported,
            routing: ProcessingCapability::Unsupported,
            plugin_rebind: ProcessingCapability::Unsupported,
            preset_select: ProcessingCapability::Unsupported,
        }
    }

    /// Boundary dependencies of the `StripFields` scope inside
    /// `project`: every routing connection that touches a
    /// scope-affiliated `SemanticPortId` endpoint without being
    /// wholly scope-owned, in stable connection-id order
    /// (CB-STRIP-03). Scope-affiliated ports are the scope-owned
    /// `SemanticPort` objects (chain processor ports under
    /// `Inserts`) plus the scoped strip's own ports — the strip is
    /// scope-owned, so its identity ports delimit the boundary even
    /// though the port entities themselves stay shared. Wholly
    /// internal connections (both endpoints scope objects — chain
    /// wiring under `Inserts`) are not boundary; wholly external
    /// ones are plain shared context. Detection errors propagate
    /// when the scope itself no longer resolves.
    ///
    /// # Errors
    /// [`Error::Validation`] for a non-`StripFields` scope,
    /// [`Error::NotFound`]/[`Error::Validation`] from scope
    /// derivation.
    pub fn boundary_dependencies(
        project: &Project,
        scope: &VariantScope,
    ) -> Result<Vec<StripBoundaryDependency>, Error> {
        let VariantScope::StripFields { strip, .. } = scope else {
            return Err(Error::Validation(format!(
                "strip boundary dependencies require a StripFields scope, got {scope:?}"
            )));
        };
        let objects = scope_objects(project, scope)?;
        let owned_ports: BTreeSet<SemanticPortId> = objects
            .iter()
            .filter(|o| o.kind == ObjectKind::SemanticPort)
            .filter_map(|o| SemanticPortId::from_tagged(&o.stable_id).ok())
            .collect();
        let strip_ports: BTreeSet<SemanticPortId> = project
            .routing
            .ports
            .values()
            .filter(|p| p.owner == PortOwner::Strip(*strip))
            .map(|p| p.id)
            .collect();
        let affiliated = |id: &SemanticPortId| owned_ports.contains(id) || strip_ports.contains(id);
        let mut out = Vec::new();
        for conn in project.routing.connections.values() {
            let internal =
                owned_ports.contains(&conn.source) && owned_ports.contains(&conn.destination);
            let source_in_scope = affiliated(&conn.source);
            let destination_in_scope = affiliated(&conn.destination);
            if internal || (!source_in_scope && !destination_in_scope) {
                continue;
            }
            out.push(StripBoundaryDependency {
                connection: conn.id,
                kind: conn.kind,
                source_in_scope,
                destination_in_scope,
            });
        }
        Ok(out)
    }

    /// Probes `port` for every processor in the scoped strip's insert
    /// chain and records each achieved
    /// [`StateCaptureQuality`](crate::plugin_state::StateCaptureQuality)
    /// or typed
    /// [`PluginStateFailureKind`](crate::plugin_state::PluginStateFailureKind)
    /// verbatim, in chain order — the comparison/adopt honesty seam
    /// for `Inserts`-scoped plugin state (same TIM-04 vocabulary as
    /// the CB-04/CB-05 assessments). The report is mask-independent —
    /// it reads project truth, not the candidate contract — so every
    /// chain processor is assessed in chain order and an empty chain
    /// yields an empty report, never an error. The blobs themselves
    /// are content-digested for identity only; the adapter never
    /// inspects plugin-internal state.
    ///
    /// # Errors
    /// [`Error::NotFound`] when the strip or its chain no longer
    /// resolves.
    pub fn assess_insert_state(
        port: &mut dyn PluginStatePort,
        project: &Project,
        strip: StripId,
        mode: CaptureMode,
    ) -> Result<Vec<InstrumentStateAssessment>, Error> {
        let strip_entity = project
            .routing
            .strips
            .get(&strip)
            .ok_or_else(|| Error::NotFound(format!("strip {strip}")))?;
        let chain = project
            .routing
            .chains
            .get(&strip_entity.chain)
            .ok_or_else(|| Error::NotFound(format!("chain {}", strip_entity.chain)))?;
        Ok(chain
            .processors
            .iter()
            .map(|processor| InstrumentSlotAdapter::assess_state(port, *processor, mode))
            .collect())
    }
}

impl DomainAdapter for StripFieldsAdapter {
    fn name(&self) -> &'static str {
        "strip-fields"
    }

    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError> {
        let VariantScope::StripFields { .. } = scope else {
            return Err(VariantError::InvalidScope {
                detail: format!("strip-fields adapter requires a StripFields scope, got {scope:?}"),
            });
        };
        validate_scope_structure(project, scope).map_err(VariantError::from)?;
        // Full object-set derivation enforces the strip + mask-filtered
        // channel-control + optional insert-chain ownership contract.
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
                detail: format!("strip-fields candidates accept project ops only: {op:?}"),
            });
        };
        // The op's own contract — READONLY rejection, discrete-kind
        // modulation refusal, TimedEvent capability, insert index
        // bounds, strip existence — runs here because the shared
        // confined flow applies without `Operation::validate` (the
        // controller's validate-then-apply order is reproduced
        // explicitly for the accepted surface only).
        if is_strip_fields_op(project_op) {
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
        let VariantScope::StripFields { strip, .. } = scope else {
            return Ok(());
        };
        // The strip resolves to its feeding lanes through the shared
        // mapping — a missing strip is reported by the adopt
        // planner's own scope derivation; the recording gate only
        // applies to a live strip. Plugin availability/port rewiring
        // stay with the shared adopt preflights: they gate adopt
        // materialization (`MissingPlugin`/`InvalidPort` before any
        // mutation), never candidate edits — project truth proceeds
        // and availability only withholds Exact claims (CB-STRIP-04).
        for lane in strip_recording_gate_lanes(live, *strip) {
            if lane_recording_open(live, lane) {
                return Err(VariantError::RecordingActive {
                    detail: format!("recording session open on lane {lane} feeding strip {strip}"),
                });
            }
        }
        Ok(())
    }
}
