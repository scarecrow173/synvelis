//! CB-04 domain adapter for [`VariantScope::InstrumentSlot`]
//! (synvelis#281).
//!
//! The adapter keeps the CB-01 contract — candidate payloads are full
//! project clones minus variant state, edits are confined to the
//! declared scope, and adopt materializes only scoped entities through
//! the shared plan/preflight/CAS path. What is instrument-specific:
//!
//! - **Shared MIDI input, isolated slot state**: the scope owns the
//!   processor, its semantic ports and its processor-owned control
//!   closure (parameter targets, automation lanes, modulation
//!   sources/assignments) via [`scope_objects`]. The MIDI
//!   source/clip feeding the slot, the upstream strip/chain structure
//!   and all downstream wiring stay *shared context* — candidate edits
//!   can never touch them (the instrument op surface in
//!   [`check_op_targets`](super::adapter) fails them `Unsupported`
//!   fail-closed, CB-INST-01/03).
//! - **Honest candidate surface**: parameter base/target, automation
//!   lane/region and modulation ops are confined to the scoped
//!   processor's closure. Plugin rebind, preset selection, processor
//!   insert/remove/reorder and every routing op have no in-scope
//!   `ProjectOp` representation and are `Unsupported` — never silently
//!   normalized (CB-INST-02, [`InstrumentSlotCapabilities`]).
//! - **Independent plugin state**: candidates are separate project
//!   clones — `plugin_instances`, control targets and automation are
//!   per-candidate values, so A/B state cannot alias at the project
//!   layer. Opaque plugin state lives outside the project in
//!   content-addressed [`PluginStateStore`] manifests; the adapter
//!   never deep-inspects blobs — [`InstrumentSlotAdapter::assess_state`]
//!   records the port's achieved [`StateCaptureQuality`] or typed
//!   [`PluginStateFailureKind`] verbatim for comparison/adopt honesty
//!   (CB-INST-01/02, CB-ADP-04, CB-INST-RCP-01).
//! - **Adopt gates**: `preflight_adopt` rejects with
//!   [`VariantError::RecordingActive`] while a recording session is
//!   open on a lane the slot feeds (owning track's lanes; every lane
//!   for bus-owned strips — fail closed). Plugin availability and
//!   port rewiring stay with the shared `preflight_plugins`/
//!   `preflight_ports` adopt path; a missing or degraded plugin never
//!   blocks project-truth edits/adopts — it only withholds Exact
//!   claims (CB-INST-05).

use crate::{
    IdGenerator, Operation, Project,
    id::{LaneId, ProcessorId},
    plugin_state::{
        CaptureMode, PluginStateBinding, PluginStateCaptureError, PluginStateOutcome,
        PluginStatePort,
    },
    routing::StripOwner,
    snapshot::ContentDigest,
    variant::{
        BranchEditOp, DomainAdapter, VariantError, VariantScope,
        adapter::{edit_scoped_project_op, fork_candidate_project, is_instrument_slot_op},
        audio::{ProcessingCapability, lane_recording_open},
        scope::{scope_objects, validate_scope_structure},
    },
};

/// Honest processing-capability report for [`InstrumentSlotAdapter`]
/// candidates (CB-INST-02): display truth equals enforcement truth.
/// Every `Supported` verdict has an accepted edit path; every
/// `Unsupported` verdict is rejected typed — unsupported instrument
/// work is never reported as editable or silently normalized away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstrumentSlotCapabilities {
    /// `SetParameterBase` — normalized base edit on a processor-owned
    /// target (rejected by the op itself on `READONLY` capabilities).
    pub param_base: ProcessingCapability,
    /// `AddParameterTarget` owned by the scoped processor.
    pub param_target_add: ProcessingCapability,
    /// `RemoveParameterTarget` — scope-entity removal is unsupported in
    /// this slice (typed `Unsupported` at the confinement layer).
    pub param_target_remove: ProcessingCapability,
    /// `AddAutomationLane`/`WriteAutomationRegion`/
    /// `EraseAutomationRange` on lanes of scoped targets.
    pub automation_edit: ProcessingCapability,
    /// `RemoveAutomationLane` — scope-entity removal unsupported.
    pub automation_remove: ProcessingCapability,
    /// Processor-owned `AddModulationSource`,
    /// `AddModulationAssignment` and `SetModulationAssignment` whose
    /// source/target/depth-source all stay inside the scoped closure.
    pub modulation_edit: ProcessingCapability,
    /// `RemoveModulationSource`/`RemoveModulationAssignment` —
    /// scope-entity removal unsupported.
    pub modulation_remove: ProcessingCapability,
    /// Plugin rebind — descriptor/version/`capability_revision`
    /// change. No `ProjectOp` mutates `Processor.binding`; a binding
    /// change is a different authored identity, never a candidate edit.
    pub plugin_rebind: ProcessingCapability,
    /// Preset selection — plugin-internal opaque state; only the
    /// `PluginStatePort` seam can express it, never a `ProjectOp`.
    pub preset_select: ProcessingCapability,
    /// Processor insert/remove/reorder — shared chain structure is
    /// never candidate-owned.
    pub chain_structure: ProcessingCapability,
    /// Routing/connection/send/bus edits — shared wiring stays outside
    /// the scope (existing external connections are never rewired).
    pub routing: ProcessingCapability,
    /// MIDI source/clip edits — the shared MIDI input feeding the slot
    /// is context, not scope content.
    pub midi_input: ProcessingCapability,
}

/// The honest per-processor plugin-state verdict a comparison or
/// adopt gate consumes (CB-INST-02 / CB-ADP-04): the port's achieved
/// [`StateCaptureQuality`](crate::plugin_state::StateCaptureQuality)
/// or typed [`PluginStateFailureKind`], recorded verbatim — never
/// upgraded, never silently substituted.
#[derive(Clone, Debug, PartialEq)]
pub struct InstrumentStateAssessment {
    /// The processor probed.
    pub processor: ProcessorId,
    /// The live binding identity the port reported (`None` = no live
    /// instance: unresolved binding, stale handle, unknown processor).
    pub binding: Option<PluginStateBinding>,
    /// The capture outcome in the existing TIM-04 vocabulary.
    pub outcome: PluginStateOutcome,
}

impl InstrumentStateAssessment {
    /// `true` only when the outcome is a `Captured` record whose
    /// achieved quality is exact-eligible (`Quiescent`/`Coordinated`,
    /// TIM-04). `BestEffort` is never promoted — a bit-exact
    /// regenerate claim requires exact-eligible quality, and
    /// `Unsupported`/`Failed` carry no state at all (CB-INST-RCP-01).
    #[must_use]
    pub fn exact_eligible(&self) -> bool {
        match &self.outcome {
            PluginStateOutcome::Captured { quality, .. } => quality.exact_eligible(),
            PluginStateOutcome::Unsupported | PluginStateOutcome::Failed { .. } => false,
        }
    }
}

/// Domain adapter for [`VariantScope::InstrumentSlot`]: processor-owned
/// control edits on full-project candidates. The scoped `Processor`,
/// its `SemanticPort`s and its control closure transplant through the
/// shared adopt surface; `plugin_instances` stays live-side — the
/// apply write path re-syncs it from the transplanted `Processor`
/// binding (`sync_processor`: unchanged plugin keeps its id, rebind
/// mints a new one, unresolved drops it).
#[derive(Clone, Debug, Default)]
pub struct InstrumentSlotAdapter;

impl InstrumentSlotAdapter {
    /// The adapter singleton.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The capability report candidate surfaces display (CB-INST-02).
    /// Every `Supported` verdict has an accepted edit path; every
    /// `Unsupported` verdict is rejected typed by the confinement
    /// layer.
    #[must_use]
    pub fn capabilities(&self) -> InstrumentSlotCapabilities {
        InstrumentSlotCapabilities {
            param_base: ProcessingCapability::Supported,
            param_target_add: ProcessingCapability::Supported,
            param_target_remove: ProcessingCapability::Unsupported,
            automation_edit: ProcessingCapability::Supported,
            automation_remove: ProcessingCapability::Unsupported,
            modulation_edit: ProcessingCapability::Supported,
            modulation_remove: ProcessingCapability::Unsupported,
            plugin_rebind: ProcessingCapability::Unsupported,
            preset_select: ProcessingCapability::Unsupported,
            chain_structure: ProcessingCapability::Unsupported,
            routing: ProcessingCapability::Unsupported,
            midi_input: ProcessingCapability::Unsupported,
        }
    }

    /// Probes `port` for `processor`'s opaque plugin state and records
    /// the achieved quality or typed failure verbatim — the
    /// comparison/adopt honesty seam for CB-INST-02 and CB-INST-RCP-01.
    /// The blob itself is content-digested for identity only; the
    /// adapter never inspects plugin-internal state.
    ///
    /// `mode` is the caller's requested [`CaptureMode`] guarantee; the
    /// returned [`PluginStateCapture::achieved`](crate::plugin_state::PluginStateCapture)
    /// quality is what the port actually achieved.
    #[must_use]
    pub fn assess_state(
        port: &mut dyn PluginStatePort,
        processor: ProcessorId,
        mode: CaptureMode,
    ) -> InstrumentStateAssessment {
        let binding = port.describe(processor);
        let outcome = match port.capture(processor, mode) {
            Ok(capture) => PluginStateOutcome::Captured {
                quality: capture.achieved,
                blob_digest: ContentDigest::sha256(&capture.blob),
                blob_len: u64::try_from(capture.blob.len()).unwrap_or(u64::MAX),
            },
            Err(PluginStateCaptureError::Unsupported) => PluginStateOutcome::Unsupported,
            Err(PluginStateCaptureError::Failed(kind)) => PluginStateOutcome::Failed { kind },
        };
        InstrumentStateAssessment {
            processor,
            binding,
            outcome,
        }
    }
}

/// Lanes whose open recording session gates adoption of `processor`'s
/// slot (CB-ADP-03 / CB-INST-05 write gate): the owning track's lanes
/// for a track strip; *every* project lane for a bus strip — a bus
/// processor sits on every routed lane's path, so ownership cannot be
/// proven narrower and the gate fails closed.
fn recording_gate_lanes(project: &Project, processor: ProcessorId) -> Vec<LaneId> {
    for strip in project.routing.strips.values() {
        let on_chain = project
            .routing
            .chains
            .get(&strip.chain)
            .is_some_and(|chain| chain.processors.contains(&processor));
        if !on_chain {
            continue;
        }
        return match strip.owner {
            StripOwner::Track(track) => project
                .tracks
                .get(&track)
                .map(|t| t.lane_ids.clone())
                .unwrap_or_default(),
            StripOwner::Bus(_) => project.lanes.keys().copied().collect(),
        };
    }
    Vec::new()
}

impl DomainAdapter for InstrumentSlotAdapter {
    fn name(&self) -> &'static str {
        "instrument-slot"
    }

    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError> {
        if !matches!(scope, VariantScope::InstrumentSlot { .. }) {
            return Err(VariantError::InvalidScope {
                detail: format!(
                    "instrument-slot adapter requires an InstrumentSlot scope, got {scope:?}"
                ),
            });
        }
        validate_scope_structure(project, scope).map_err(VariantError::from)?;
        // Full object-set derivation enforces the processor + port +
        // control-closure ownership contract.
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
                detail: format!("instrument-slot candidates accept project ops only: {op:?}"),
            });
        };
        // The op's own contract — READONLY rejection, discrete-kind
        // modulation refusal, TimedEvent capability — runs here because
        // the shared confined flow applies without `Operation::validate`
        // (the controller's validate-then-apply order is reproduced
        // explicitly for the accepted surface only).
        if is_instrument_slot_op(project_op) {
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
        let VariantScope::InstrumentSlot { processor } = scope else {
            return Ok(());
        };
        // A missing processor is reported by the adopt planner's own
        // scope derivation; the recording gate only applies to a live
        // slot. Plugin availability/port rewiring stay with the shared
        // adopt preflights — a missing plugin never blocks project
        // truth, it only withholds Exact claims (CB-ADP-04).
        for lane in recording_gate_lanes(live, *processor) {
            if lane_recording_open(live, lane) {
                return Err(VariantError::RecordingActive {
                    detail: format!(
                        "recording session open on lane {lane} feeding instrument slot {processor}"
                    ),
                });
            }
        }
        Ok(())
    }
}
