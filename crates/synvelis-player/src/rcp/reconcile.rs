//! Safe plan reconciliation — edit classification, stale-result
//! rejection, last-good retention, and the automation/parameter
//! delivery gate (Issue #293 contract, RCP-EX-04/05).
//!
//! This module owns the *decision* side of plan lifecycle on top of
//! the existing `RoutingBuildKey`/`RoutingActivationReceipt`
//! vocabulary from `synvelis_project`: it never compiles, publishes,
//! or touches the RT path itself — the caller performs the actual
//! Ruviform non-RT prepare/publish and reports the outcome back via
//! [`PlanReconciler::publish`]. That keeps every stale-activation
//! guarantee testable headlessly.
//!
//! Invariants enforced here:
//!
//! - a prepared candidate publishes only against a live snapshot
//!   whose revision, structure digest, capability/format
//!   generations, clock epoch and runtime instance still match —
//!   anything else is a typed [`RoutingFault`], never a silent
//!   activation;
//! - publish requires a strictly newer generation than the last
//!   published plan (`Superseded` otherwise) — rapid R1→R2→R3
//!   edits cannot let an older prepare displace a newer plan;
//! - every failure retains last-good and records a `Blocked`/
//!   `Failed` receipt; project state is never rolled back;
//! - `reconcile` compares the active plan's pins against the live
//!   snapshot + capability generation and reports drift as
//!   [`ReconcileMismatch`]s — render-index staleness is tracked
//!   separately and never conflated with graph rebuild;
//! - while a mismatch stands, the delivery gate blocks new
//!   automation/parameter traffic addressed to the stale
//!   generation.

use synvelis_project::{
    ProjectRevision, RoutingActivationReceipt, RoutingActivationState, RoutingBuildKey,
    RoutingFault, RoutingReconcileMismatch,
};

use super::model::{RcpNodeId, ResolvedGraphSnapshot};
use super::verify::{PluginVerdict, verify_snapshot};

/// Semantic class of an edit — the RCP-EX-04 classification input.
/// Callers classify *what changed*; this module decides whether the
/// active plan survives it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EditKind {
    /// Rename, UI-only state, view changes — cannot alter the
    /// lowered graph.
    Presentation,
    /// Bound fast-parameter (knob/automation-bound control) edit —
    /// updates control values only; may invalidate render
    /// eligibility but never the plan shape.
    BoundParameter,
    /// Automation curve/event edit — control-domain traffic only.
    AutomationEvent,
    /// Graph structure edit (insert/remove/reconnect) or port
    /// layout change.
    Structural,
    /// Plugin binding change (different plugin/format on a
    /// processor).
    PluginBinding,
    /// Processor port capability change.
    PortLayout,
    /// Render substitution set changed (artifact arrival/removal)
    /// — re-selection needed, not graph recompilation.
    RenderSubstitution,
    /// Source media content change.
    SourceContent,
}

/// Edit classification result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditClass {
    /// Plan pins still hold — no rebuild, no control traffic.
    NoRebuild,
    /// Graph-equivalent edit that only moves control values —
    /// deliver control updates; re-evaluate affected render
    /// eligibility separately (render invalidation is independent
    /// of plan rebuild).
    ControlOnly {
        /// Nodes whose bound parameters/automation may have moved.
        affected: Vec<RcpNodeId>,
    },
    /// Plan pins no longer hold — re-select and re-publish through
    /// the normal prepare/publish path.
    Rebuild,
}

/// Pins + selection identity of one prepared/published plan — the
/// comparison basis for [`classify_edit`] and [`PlanReconciler`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanContext {
    /// Build key pins (revision, fingerprint, generations, epoch).
    pub build_key: RoutingBuildKey,
    /// Render candidate index generation the selection used.
    pub render_index_generation: u64,
    /// Selection decision digest.
    pub selection_digest: u64,
}

impl PlanContext {
    /// Derives the context from a selection receipt plus its build
    /// key — the single construction path so pins can't drift.
    #[must_use]
    pub fn from_selection(
        receipt: &super::select::ReconciliationReceipt,
        render_index_generation: u64,
    ) -> Self {
        Self {
            build_key: receipt.build_key.clone(),
            render_index_generation,
            selection_digest: receipt.selection_digest,
        }
    }
}

/// Classifies an edit against the active plan context. Pure
/// function of `(before, after snapshot, live render index
/// generation, edit kind)` — render staleness is *not* an input:
/// artifact invalidation never forces a plan rebuild by itself.
#[must_use]
pub fn classify_edit(
    before: &PlanContext,
    after: &ResolvedGraphSnapshot,
    after_render_index_generation: u64,
    kind: EditKind,
) -> EditClass {
    // Graph/content pins: build a candidate key from `after` with
    // the same build mode and compare graph-relevant fields.
    let after_key = RoutingBuildKey {
        project_id: after.project_id,
        project_revision: after.project_revision,
        routing_fingerprint: synvelis_project::RoutingFingerprint(after.structure_digest),
        capability_generation: after.capability_generation,
        engine_config_generation: after.engine_config_generation,
        source_format_generation: after.source_format_generation,
        lowering_policy_version: before.build_key.lowering_policy_version,
        build_mode: before.build_key.build_mode,
        device_clock_epoch: after.clock_epoch,
        runtime_instance: after.runtime_instance,
    };
    let graph_same = before.build_key.semantic_eq(&after_key)
        && before.render_index_generation == after_render_index_generation;

    if !graph_same {
        return EditClass::Rebuild;
    }
    match kind {
        EditKind::BoundParameter | EditKind::AutomationEvent => EditClass::ControlOnly {
            affected: after
                .nodes
                .values()
                .filter(|n| n.plugin.is_some())
                .map(|n| n.id)
                .collect(),
        },
        // Source media bytes are outside every plan pin — identical
        // pins cannot prove the content is unchanged, so a declared
        // content edit always re-resolves rather than trusting a
        // NoRebuild the evidence cannot support.
        EditKind::SourceContent => EditClass::Rebuild,
        _ => EditClass::NoRebuild,
    }
}

/// A prepared plan candidate awaiting publish — carries its
/// generation and pins; publish is the only way it becomes active.
#[derive(Clone, Debug)]
pub struct PreparedPlan {
    /// Assigned generation (strictly increasing per reconciler).
    pub generation: u64,
    /// Pins the candidate was prepared against.
    pub context: PlanContext,
    /// Selection receipt evidence.
    pub selection_digest: u64,
}

/// Committed (published) plan with its evidence state.
#[derive(Clone, Debug)]
pub struct CommittedPlan {
    /// Plan generation.
    pub generation: u64,
    /// Context pins.
    pub context: PlanContext,
    /// Activation state evidence.
    pub state: RoutingActivationState,
}

/// Current-vs-committed reconciliation view.
#[derive(Clone, Debug, Default)]
pub struct ReconcileOutcome {
    /// Drift aspects; empty means fully current.
    pub mismatches: Vec<RoutingReconcileMismatch>,
    /// Degraded plugin nodes.
    pub plugins_degraded: Vec<(RcpNodeId, PluginVerdict)>,
    /// Whether the render index moved (informational — triggers
    /// re-selection, not rebuild).
    pub render_index_moved: bool,
    /// Current-revision mismatch summary for display: the
    /// committed revision the active plan targets vs. the live
    /// project revision.
    pub revision_mismatch: Option<(ProjectRevision, ProjectRevision)>,
}

/// Generation-tagged plan lifecycle manager — the headless half of
/// "safe plan reconciliation". Non-RT only; RT activation
/// observation arrives through [`Self::observe_active`] or is left
/// to the engine's own receipts.
pub struct PlanReconciler {
    active: Option<CommittedPlan>,
    last_good: Option<CommittedPlan>,
    last_published_generation: u64,
    next_generation: u64,
    outcome: ReconcileOutcome,
}

impl PlanReconciler {
    /// Empty reconciler — no active plan, generation counter at 1.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: None,
            last_good: None,
            last_published_generation: 0,
            next_generation: 1,
            outcome: ReconcileOutcome::default(),
        }
    }

    /// The currently published plan (what would be activated).
    #[must_use]
    pub fn active(&self) -> Option<&CommittedPlan> {
        self.active.as_ref()
    }

    /// Last-good plan — retained on every failure; what playback
    /// falls back to.
    #[must_use]
    pub fn last_good(&self) -> Option<&CommittedPlan> {
        self.last_good.as_ref()
    }

    /// Latest reconcile outcome (revision mismatch display data).
    #[must_use]
    pub fn outcome(&self) -> &ReconcileOutcome {
        &self.outcome
    }

    /// Whether new automation/parameter traffic addressed to
    /// `generation` may be delivered. Blocked whenever the active
    /// plan is stale relative to committed project state — old-
    /// generation delivery during mismatch is exactly the misroute
    /// the spec forbids.
    #[must_use]
    pub fn delivery_gate(&self, generation: u64) -> DeliveryDecision {
        let Some(active) = &self.active else {
            return DeliveryDecision::BlockStaleGeneration {
                active_generation: None,
            };
        };
        // While any mismatch stands the active plan is stale
        // relative to committed state — block all delivery to it.
        if !self.outcome.mismatches.is_empty() {
            return DeliveryDecision::BlockStaleGeneration {
                active_generation: Some(active.generation),
            };
        }
        if generation == active.generation {
            DeliveryDecision::Allow
        } else {
            DeliveryDecision::BlockStaleGeneration {
                active_generation: Some(active.generation),
            }
        }
    }

    /// Prepares a candidate from a selection receipt — assigns the
    /// next generation. Does not publish; the caller runs the
    /// actual Ruviform compile/prepare and then calls
    /// [`Self::publish`].
    #[must_use]
    pub fn prepare(&mut self, receipt: &super::select::ReconciliationReceipt) -> PreparedPlan {
        let generation = self.next_generation;
        self.next_generation += 1;
        PreparedPlan {
            generation,
            context: PlanContext {
                build_key: receipt.build_key.clone(),
                render_index_generation: receipt.render_index_generation,
                selection_digest: receipt.selection_digest,
            },
            selection_digest: receipt.selection_digest,
        }
    }

    /// Publishes a prepared candidate against the live snapshot.
    /// Revalidates every pin first — a stale candidate is rejected
    /// before the transport call. On transport failure the receipt
    /// records `Failed`, last-good is retained, and the active plan
    /// is untouched.
    pub fn publish(
        &mut self,
        prepared: &PreparedPlan,
        live: &ResolvedGraphSnapshot,
        capabilities: &super::model::PluginCapabilitySnapshot,
        publish_result: Result<(), RoutingFault>,
    ) -> RoutingActivationReceipt {
        // --- Pin revalidation (pre-publish stale rejection) ---
        let pin_fault = Self::check_pins(&prepared.context, live, capabilities);
        if let Some(fault) = pin_fault {
            return self.record_blocked(&prepared.context.build_key, fault);
        }
        if prepared.generation <= self.last_published_generation {
            return self.record_blocked(&prepared.context.build_key, RoutingFault::Superseded);
        }

        match publish_result {
            Ok(()) => {
                self.last_published_generation = prepared.generation;
                let old = self.active.take();
                let plan = CommittedPlan {
                    generation: prepared.generation,
                    context: prepared.context.clone(),
                    state: RoutingActivationState::Published {
                        project_revision: live.project_revision,
                        generation: prepared.generation,
                    },
                };
                self.active = Some(plan);
                if let Some(prev) = old {
                    self.last_good = Some(prev);
                }
                // Successful publish clears any standing mismatch —
                // the new plan is current by construction.
                self.outcome = ReconcileOutcome::default();
                self.outcome.render_index_moved = false;
                RoutingActivationReceipt {
                    request_id: None,
                    build_key: Some(prepared.context.build_key.clone()),
                    state: RoutingActivationState::Published {
                        project_revision: live.project_revision,
                        generation: prepared.generation,
                    },
                    planned_generation: Some(prepared.generation),
                    published_generation: Some(prepared.generation),
                    observed_active_generation: None,
                    last_good_generation: self.last_good.as_ref().map(|p| p.generation),
                    frames_observed: 0,
                    fault: None,
                }
            }
            Err(fault) => {
                // QueueFull / RetireOverflow / any transport
                // refusal: last-good retained, active untouched,
                // receipt Failed — never a silent success.
                self.record_failed(&prepared.context.build_key, fault)
            }
        }
    }

    /// Reconciles the active plan against the live snapshot and
    /// capability generation. Detects revision/fingerprint/
    /// capability/format/epoch drift and plugin degradation;
    /// render-index movement is reported separately (it triggers
    /// re-selection, not rebuild). On any drift the outcome carries
    /// the mismatch and the delivery gate closes.
    pub fn reconcile(
        &mut self,
        live: &ResolvedGraphSnapshot,
        capabilities: &super::model::PluginCapabilitySnapshot,
        live_render_index_generation: u64,
    ) -> ReconcileOutcome {
        let Some(active) = &self.active else {
            self.outcome = ReconcileOutcome::default();
            return ReconcileOutcome::default();
        };
        let mut outcome = ReconcileOutcome::default();
        let key = &active.context.build_key;

        if key.project_id != live.project_id {
            outcome
                .mismatches
                .push(RoutingReconcileMismatch::SourceMapIdentity);
        }
        if key.project_revision != live.project_revision {
            outcome
                .mismatches
                .push(RoutingReconcileMismatch::ProjectRevision);
            outcome.revision_mismatch = Some((key.project_revision, live.project_revision));
        }
        if key.routing_fingerprint.0 != live.structure_digest {
            outcome
                .mismatches
                .push(RoutingReconcileMismatch::RoutingFingerprint);
        }
        // Capability drift on either side: plan pin vs. snapshot
        // pin, or the capability snapshot vs. the snapshot's pinned
        // generation — verdicts computed against a different
        // generation are evidence for a different world.
        if key.capability_generation != live.capability_generation
            || capabilities.generation != live.capability_generation
        {
            outcome
                .mismatches
                .push(RoutingReconcileMismatch::CapabilityGeneration);
        }
        if key.engine_config_generation != live.engine_config_generation {
            outcome
                .mismatches
                .push(RoutingReconcileMismatch::EngineConfigGeneration);
        }
        if key.source_format_generation != live.source_format_generation {
            outcome
                .mismatches
                .push(RoutingReconcileMismatch::SourceFormatGeneration);
        }
        if key.device_clock_epoch != live.clock_epoch
            || key.runtime_instance != live.runtime_instance
        {
            outcome
                .mismatches
                .push(RoutingReconcileMismatch::DeviceEpoch);
        }
        if active.context.render_index_generation != live_render_index_generation {
            outcome.render_index_moved = true;
        }

        // Plugin degradation: any plugin node whose verdict left
        // Live since publish (restore failure, capability drift).
        let verdicts = verify_snapshot(&live.nodes, capabilities);
        for (node_id, verdict) in &verdicts {
            if *verdict != PluginVerdict::Live {
                outcome.plugins_degraded.push((*node_id, verdict.clone()));
            }
        }
        if !outcome.plugins_degraded.is_empty() {
            outcome
                .mismatches
                .push(RoutingReconcileMismatch::CapabilityGeneration);
        }

        // A pure revision move with identical graph content is the
        // `NoRebuild` case — update the active pin so subsequent
        // reconciles don't re-report it, but keep evidence that a
        // re-selection may still be needed for render pins.
        if outcome.mismatches == [RoutingReconcileMismatch::ProjectRevision]
            && key.routing_fingerprint.0 == live.structure_digest
        {
            if let Some(active) = &mut self.active {
                active.context.build_key.project_revision = live.project_revision;
            }
            outcome
                .mismatches
                .retain(|m| *m != RoutingReconcileMismatch::ProjectRevision);
            outcome.revision_mismatch = None;
        }

        self.outcome = outcome.clone();
        outcome
    }

    /// Records RT-observed activation of a generation — the
    /// `Active` ack is separate from `Published` by contract.
    pub fn observe_active(
        &mut self,
        generation: u64,
        epoch: synvelis_project::RuntimeClockEpochRef,
    ) {
        // Activation evidence must match both the plan generation
        // and the clock epoch the plan was pinned to — an RT
        // observation from a different epoch is not activation of
        // this plan.
        if let Some(active) = &mut self.active
            && active.generation == generation
            && epoch == active.context.build_key.device_clock_epoch
        {
            active.state = RoutingActivationState::Active {
                project_revision: active.context.build_key.project_revision,
                generation,
                epoch,
            };
        }
    }

    /// Pin revalidation shared by publish — returns the first fault
    /// found. Associated function: it compares two explicit inputs
    /// and holds no reconciler state.
    fn check_pins(
        context: &PlanContext,
        live: &ResolvedGraphSnapshot,
        capabilities: &super::model::PluginCapabilitySnapshot,
    ) -> Option<RoutingFault> {
        let key = &context.build_key;
        if key.project_id != live.project_id || key.project_revision != live.project_revision {
            return Some(RoutingFault::StaleProjectSnapshot);
        }
        if key.routing_fingerprint.0 != live.structure_digest {
            return Some(RoutingFault::SpeculativeMismatch);
        }
        if key.capability_generation != live.capability_generation
            || capabilities.generation != live.capability_generation
        {
            return Some(RoutingFault::PluginCapabilityChanged);
        }
        if key.engine_config_generation != live.engine_config_generation
            || key.source_format_generation != live.source_format_generation
        {
            return Some(RoutingFault::UnsupportedFormatOrTimeMap);
        }
        if key.device_clock_epoch != live.clock_epoch
            || key.runtime_instance != live.runtime_instance
        {
            return Some(RoutingFault::DeviceEpochChanged);
        }
        None
    }

    fn record_blocked(
        &mut self,
        key: &RoutingBuildKey,
        fault: RoutingFault,
    ) -> RoutingActivationReceipt {
        RoutingActivationReceipt {
            request_id: None,
            build_key: Some(key.clone()),
            state: RoutingActivationState::Blocked {
                fault,
                last_good_generation: self.last_good.as_ref().map(|p| p.generation),
            },
            planned_generation: None,
            published_generation: None,
            observed_active_generation: None,
            last_good_generation: self.last_good.as_ref().map(|p| p.generation),
            frames_observed: 0,
            fault: Some(fault),
        }
    }

    fn record_failed(
        &mut self,
        key: &RoutingBuildKey,
        fault: RoutingFault,
    ) -> RoutingActivationReceipt {
        RoutingActivationReceipt {
            request_id: None,
            build_key: Some(key.clone()),
            state: RoutingActivationState::Failed {
                fault,
                last_good_generation: self.last_good.as_ref().map(|p| p.generation),
            },
            planned_generation: None,
            published_generation: None,
            observed_active_generation: None,
            last_good_generation: self.last_good.as_ref().map(|p| p.generation),
            frames_observed: 0,
            fault: Some(fault),
        }
    }
}

impl Default for PlanReconciler {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of the automation/parameter delivery gate.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DeliveryDecision {
    /// Delivery permitted — generation matches the current active
    /// plan and no revision mismatch stands.
    Allow,
    /// Delivery blocked: the addressed generation is stale relative
    /// to committed project state. Carries the currently active
    /// generation for diagnostics.
    BlockStaleGeneration {
        /// Generation currently active (last-good), if any.
        active_generation: Option<u64>,
    },
}
