//! PC-B parameter dispatch: host-resolved [`TimedParamEvent`]s routed
//! into a live plugin node through the ruviform plugin bridge (issue
//! #43). This module is the **non-RT orchestration surface**: it owns
//! the `TargetId` binding table, the plan builder and install path,
//! and the dispatch ledger that projects plan events to observable
//! stages. The RT delivery itself lives in `PluginNode` (upstream,
//! already alloc-free); a caller wires `attach` + `GraphRuntime` and
//! hands this module the resulting [`PluginNodeControl`].
//!
//! Rejection contract: every failure to write is a typed
//! [`DispatchError`] — upstream [`ParamWriteRejection`] tokens are
//! preserved verbatim so the surface matches
//! `synvelis_project::Error::WriteRejected`; unbound/defunct bindings
//! get explicit tokens (`param_unresolved` / `param_defunct`) and a
//! binding↔event `param_id` mismatch is refused as `param_mismatch`.
//! No rejection is ever converted to a success.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use ruviform_audio_plugin_bridge::{
    DispatchStage, NodeReport, ParamBinding, ParamDispatchLedger, ParamTargetTable,
    ParamValueObservation, PluginNodeControl, TargetId, TargetResolution,
};
use ruviform_graph::NodeId;
use ruviform_media_core::{
    EngineFrame, PlanGeneration, TimeBase, TransportCommand, TransportPlan, TransportPlanBuilder,
    TransportSpan,
};
use ruviform_plugin_core::{EventKind, ParamId, ParamInfo, TimedParamEvent};
use synvelis_project::ParameterTargetId;

/// Typed failures of the dispatch surface.
#[derive(Debug)]
#[non_exhaustive]
pub enum DispatchError {
    /// The write was rejected. `rejection` keeps the upstream
    /// [`ruviform_plugin_core::ParamWriteRejection`] token
    /// (`param_readonly`, `param_not_automatable`, `param_unknown`, …)
    /// or a binding-level token (`param_unresolved`, `param_defunct`,
    /// `param_mismatch`).
    WriteRejected {
        /// The attempted means/stage (`"bind"`, `"resolve"`,
        /// `"dispatch"`).
        means: String,
        /// The rejection token.
        rejection: String,
    },
    /// A staged control/plan message was dropped because the bounded
    /// node queue was full — retry is the caller's policy.
    QueueFull,
    /// An event position could not be represented in the plan's frame
    /// base.
    PositionUnrepresentable,
    /// An empty write batch was submitted — there is no plan to
    /// stage, and staging one would still mark the previous plan's
    /// `Scheduled` entries `PlanReplaced` and clear the node's live
    /// plan. Nothing is staged or requested.
    Empty,
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteRejected { means, rejection } => {
                write!(f, "write via {means} rejected: {rejection}")
            }
            Self::QueueFull => write!(f, "plugin-node control queue full"),
            Self::PositionUnrepresentable => {
                write!(f, "event position not representable in plan frame base")
            }
            Self::Empty => write!(f, "empty write batch — nothing staged"),
        }
    }
}

impl std::error::Error for DispatchError {}

/// Maps a synvelis [`ParameterTargetId`] to a bridge [`TargetId`].
/// Stable ids come from a monotonic generator whose values fit in
/// `u64`; the conversion is injective within one table. Callers must
/// gate on [`in_range`] first — truncating an out-of-range raw id
/// would alias onto another bound target.
fn to_target_id(target: ParameterTargetId) -> TargetId {
    #[allow(clippy::cast_possible_truncation)]
    TargetId(target.raw() as u64)
}

/// Whether `target` fits the bridge's `u64` id space — the gate for
/// every `to_target_id` call (a truncated raw id aliases onto a
/// real bound target).
fn in_range(target: ParameterTargetId) -> bool {
    target.raw() <= u128::from(u64::MAX)
}

/// The non-RT dispatch orchestrator for one attached plugin node.
///
/// Owns the `TargetId → (node, param)` binding table and the dispatch
/// ledger; turns a batch of lowered [`TimedParamEvent`]s into a
/// generation-tagged [`TransportPlan`] staged on the node.
pub struct ParamDispatch {
    /// The graph node this dispatch drives.
    node: NodeId,
    /// Application-target → binding table.
    table: ParamTargetTable,
    /// Scheduled→Dispatched/Dropped observation ledger.
    ledger: ParamDispatchLedger,
    /// The node's non-RT control handle.
    control: PluginNodeControl,
    /// Plan frame base (engine frames; one tick == one frame).
    frame_base: TimeBase,
    /// Next plan generation to mint.
    next_generation: u64,
    /// The last staged plan — `observe` projects through exactly this,
    /// so a stale/foreign `Arc` can never misattribute stages.
    last_plan: Option<Arc<TransportPlan<EventKind>>>,
    /// The plan generation the node's scheduler last reported live in
    /// an [`ruviform_media_core::AppliedReceipt`] — the node's own
    /// truth. `None` until any receipt arrives. Projection is gated
    /// on it matching `last_plan`'s generation: a `install_plan`
    /// apply rejected inside the scheduler (budget, stale epoch)
    /// leaves the receipt's generation on the previous plan, proving
    /// our staged plan never ran.
    scheduler_gen: Option<PlanGeneration>,
    /// `(node, param)` → owning synvelis target — keeps ledger
    /// attribution honest by refusing a second target on one param.
    param_owners: BTreeMap<(NodeId, ParamId), ParameterTargetId>,
}

impl ParamDispatch {
    /// A dispatcher for `node` behind `control`, building plans in
    /// `frame_base` (the engine frame base — e.g.
    /// `TimeBase::sample_rate(48_000)`).
    ///
    /// `frame_base` **must equal** the node's attach-time
    /// `AttachConfig::frame_base` — the bridge exposes no accessor to
    /// verify this, and a mismatch silently scales event positions,
    /// `Seek` re-anchors, and observation windows.
    #[must_use]
    pub fn new(control: PluginNodeControl, node: NodeId, frame_base: TimeBase) -> Self {
        Self {
            node,
            table: ParamTargetTable::new(),
            ledger: ParamDispatchLedger::new(),
            control,
            frame_base,
            next_generation: 1,
            last_plan: None,
            scheduler_gen: None,
            param_owners: BTreeMap::new(),
        }
    }

    /// The graph node this dispatcher drives.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// The underlying node control handle for transport requests.
    /// Note [`ParamDispatch::observe`] drains the report channel
    /// unconditionally — consumers attached through
    /// `HeadlessDriver` must not expect to drain receipts/faults
    /// themselves once blocks run.
    ///
    /// Out-of-band pushes through this handle (`seek`,
    /// `device_restart`, a direct `install_plan`, …) bypass the
    /// dispatcher: the ledger's last-plan tracking and the driver's
    /// fabricated span/cursor assumptions are not updated, and
    /// `SetClock` re-anchors the cursor `dispatch`/`observe` compute
    /// against. Consumers that need those operations should go
    /// through `HeadlessDriver::seek`/activation, which keeps the
    /// dispatcher consistent.
    pub fn control(&mut self) -> &mut PluginNodeControl {
        &mut self.control
    }

    /// The plan frame base this dispatcher builds positions in.
    #[must_use]
    pub const fn frame_base(&self) -> TimeBase {
        self.frame_base
    }

    /// The generation of the last staged plan, if any — what the
    /// node's scheduler would stamp on span contexts once the
    /// `install_plan` applies.
    #[must_use]
    pub fn last_plan_generation(&self) -> Option<PlanGeneration> {
        self.last_plan.as_ref().map(|plan| plan.generation)
    }

    /// Binds `target` to the plugin-local `param`, validated against
    /// the plugin's live `params` enumeration. A `param` already
    /// owned by a different target is refused — the ledger attributes
    /// plan events by `(node, param)`, so a shared binding would
    /// silently misattribute the second target's writes.
    ///
    /// # Errors
    /// [`DispatchError::WriteRejected`] with the upstream
    /// `param_unknown` token when `params` does not enumerate `param`,
    /// `param_target_out_of_range` when `target` exceeds the `u64`
    /// bridge id space, or `param_in_use` when `param` is bound to a
    /// different target.
    pub fn bind(
        &mut self,
        target: ParameterTargetId,
        param: ParamId,
        params: &[ParamInfo],
    ) -> Result<ParamBinding, DispatchError> {
        if !in_range(target) {
            return Err(DispatchError::WriteRejected {
                means: "bind".into(),
                rejection: "param_target_out_of_range".into(),
            });
        }
        if let Some(owner) = self.param_owners.get(&(self.node, param))
            && *owner != target
        {
            return Err(DispatchError::WriteRejected {
                means: "bind".into(),
                rejection: "param_in_use".into(),
            });
        }
        let binding = self
            .table
            .bind(to_target_id(target), self.node, param, params)
            .map_err(|rej| DispatchError::WriteRejected {
                means: "bind".into(),
                rejection: rej.to_string(),
            })?;
        self.param_owners
            .retain(|key, owner| !(key.0 == self.node && *owner == target));
        self.param_owners.insert((self.node, param), target);
        Ok(binding)
    }

    /// Revalidates `target`'s binding against the plugin's live
    /// `params` enumeration — a param dropped by the plugin flips the
    /// binding defunct so later `resolve_write` returns
    /// `param_defunct`. Returns `false` when the binding went defunct
    /// (or was absent), or when `target` exceeds the bridge id space.
    pub fn revalidate(&mut self, target: ParameterTargetId, params: &[ParamInfo]) -> bool {
        if !in_range(target) {
            return false;
        }
        let ok = self
            .table
            .revalidate(to_target_id(target), self.node, params);
        if !ok {
            self.param_owners.retain(|_, owner| *owner != target);
        }
        ok
    }

    /// Drops `target`'s binding and its ownership record.
    pub fn unbind(&mut self, target: ParameterTargetId) -> bool {
        if !in_range(target) {
            return false;
        }
        self.param_owners.retain(|_, owner| *owner != target);
        self.table.unbind(to_target_id(target))
    }

    /// Marks every binding on this dispatcher's node defunct — e.g.
    /// after the plugin re-enumerated its parameters.
    pub fn invalidate_node(&mut self) {
        self.table.invalidate_node(self.node);
        self.param_owners.clear();
    }

    /// Resolves `target` to a live binding, refusing writes the table
    /// cannot honor.
    ///
    /// # Errors
    /// [`DispatchError::WriteRejected`] — `param_unresolved` when no
    /// binding exists, `param_defunct` when the binding failed
    /// revalidation against the plugin's enumeration.
    pub fn resolve_write(&self, target: ParameterTargetId) -> Result<ParamBinding, DispatchError> {
        if !in_range(target) {
            return Err(DispatchError::WriteRejected {
                means: "resolve".into(),
                rejection: "param_target_out_of_range".into(),
            });
        }
        match self.table.resolve(to_target_id(target)) {
            TargetResolution::Resolved(binding) => Ok(binding),
            TargetResolution::Unresolved => Err(DispatchError::WriteRejected {
                means: "resolve".into(),
                rejection: "param_unresolved".into(),
            }),
            TargetResolution::Defunct => Err(DispatchError::WriteRejected {
                means: "resolve".into(),
                rejection: "param_defunct".into(),
            }),
            other => Err(DispatchError::WriteRejected {
                means: "resolve".into(),
                rejection: format!("param_resolution_{other:?}"),
            }),
        }
    }

    /// Lowers `writes` into one generation-tagged plan and stages it
    /// on the node: every `(target, event)` pair must resolve to a
    /// live binding **and** address the bound `param_id` — a write
    /// lowered for one target but addressed at another is refused as
    /// `param_mismatch` before anything is staged.
    ///
    /// On success transport `Play` is requested and the plan is
    /// installed for immediate delivery (`due_frames = 0`),
    /// previously scheduled ledger entries are marked `PlanReplaced`,
    /// and the new plan's addressable events enter the ledger as
    /// `Scheduled`. The `Play` request goes first so a `QueueFull` on
    /// `install_plan` can never leave an orphaned plan staged on the
    /// node — a queued `Play` is idempotent transport state, not a
    /// staged write.
    ///
    /// # Errors
    /// [`DispatchError::WriteRejected`] on binding resolution or
    /// param mismatch; [`DispatchError::Empty`] when `writes` is
    /// empty (staging an empty plan would still replace the live
    /// plan and `PlanReplaced`-drop its pending entries);
    /// [`DispatchError::PositionUnrepresentable`] when an event
    /// position cannot convert to the frame base;
    /// [`DispatchError::QueueFull`] when the node's bounded queue
    /// drops the staged message.
    pub fn dispatch(
        &mut self,
        writes: &[(ParameterTargetId, TimedParamEvent)],
    ) -> Result<Arc<TransportPlan<EventKind>>, DispatchError> {
        if writes.is_empty() {
            return Err(DispatchError::Empty);
        }
        for (target, event) in writes {
            let binding = self.resolve_write(*target)?;
            if binding.param != event.event.param_id {
                return Err(DispatchError::WriteRejected {
                    means: "dispatch".into(),
                    rejection: "param_mismatch".into(),
                });
            }
        }
        let mut builder = TransportPlanBuilder::<EventKind>::new(
            self.frame_base,
            PlanGeneration::new(self.next_generation),
        );
        self.next_generation = self.next_generation.wrapping_add(1);
        for (seq, (_, event)) in writes.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            builder
                .event(
                    event.position,
                    seq as u64,
                    EventKind::ParamValue(event.event),
                )
                .map_err(|_| DispatchError::PositionUnrepresentable)?;
        }
        let plan = Arc::new(builder.build());
        self.control
            .request(TransportCommand::Play, 0)
            .map_err(|_| DispatchError::QueueFull)?;
        self.control
            .install_plan(Arc::clone(&plan), 0)
            .map_err(|_| DispatchError::QueueFull)?;
        self.ledger.mark_plan_replaced();
        self.ledger.record_plan(&plan, &self.table, self.node);
        self.last_plan = Some(Arc::clone(&plan));
        Ok(plan)
    }

    /// Advances the observation ledger: projects `Scheduled` entries
    /// onto the physical call `[call_start, call_start + call_frames)`
    /// of `span` (`Dispatched`), applies drained node reports
    /// (faults/undeliverable spans → `Dropped`), and returns the
    /// latest observation per synvelis target. Projection always uses
    /// the plan staged by the last `dispatch` — there is no way to
    /// observe through a stale or foreign plan.
    ///
    /// The caller supplies the span/call window because the physical
    /// call partition lives inside the RT runtime — this method is the
    /// non-RT side observing what the node reported. Reports drain on
    /// every call (plan or not) — receipts/faults of transport-only
    /// controls must not fill the bounded channel.
    ///
    /// Report application order matters: install/control-time reports
    /// (`VaultExceeded`, `Fault`, stale-epoch and receipt traffic)
    /// apply **before** the projection so a plan the node never
    /// installed or a faulted scheduler cannot mint `Dispatched`
    /// entries — `VaultExceeded` drops only still-`Scheduled` entries,
    /// which an earlier projection would have wrongly promoted.
    /// Call-window reports (`UndeliverableSpan`, `EventScratchOverflow`,
    /// `EndpointError`) attribute to the call just projected — the
    /// latter two name "the most recently projected call" — so they
    /// apply after it.
    ///
    /// Projection is additionally gated on the scheduler's own
    /// receipts: the last [`NodeReport::Receipt`] in the drain reports
    /// the plan generation actually live on the node, and a mismatch
    /// with `last_plan` (a `install_plan` apply rejected inside the
    /// scheduler — e.g. `BudgetExceeded`) skips the projection
    /// instead of minting `Dispatched` for events the plugin never
    /// received.
    pub fn observe(
        &mut self,
        span: &TransportSpan,
        call_start: EngineFrame,
        call_frames: u32,
    ) -> Vec<(ParameterTargetId, ParamValueObservation)> {
        let reports = self.control.drain_reports();
        for report in &reports {
            if !matches!(
                report,
                NodeReport::UndeliverableSpan { .. }
                    | NodeReport::EventScratchOverflow
                    | NodeReport::EndpointError { .. }
            ) {
                self.ledger.apply_report(report);
            }
        }
        // Receipts carry the scheduler's live plan generation — the
        // last one in the drain is the newest truth. An install
        // rejected at apply time leaves it on the previous plan, so a
        // mismatch here proves `last_plan` never ran and projecting
        // through it would mint `Dispatched` for undelivered events.
        for report in reports.iter().rev() {
            if let NodeReport::Receipt(receipt) = report {
                self.scheduler_gen = Some(receipt.plan_generation);
                break;
            }
        }
        if let Some(plan) = self.last_plan.clone()
            && self
                .scheduler_gen
                .is_none_or(|live| live == plan.generation)
        {
            self.ledger
                .project_call(&plan, span, call_start, call_frames);
        }
        for report in &reports {
            if matches!(
                report,
                NodeReport::UndeliverableSpan { .. }
                    | NodeReport::EventScratchOverflow
                    | NodeReport::EndpointError { .. }
            ) {
                self.ledger.apply_report(report);
            }
        }
        self.ledger
            .observations()
            .into_iter()
            .map(|(tid, obs)| (ParameterTargetId::new(u128::from(tid.0)), obs))
            .collect()
    }

    /// The latest stage a target reached in the ledger, if tracked.
    ///
    /// `Dispatched` is a plan×span projection — "the call window
    /// covered the event" — not an endpoint acknowledgement. For
    /// plugin classes with no report channel, callers may finalize
    /// honesty via the ledger's `mark_reporting_unavailable`
    /// terminal (`Unavailable`) instead of leaving entries
    /// `Dispatched` forever.
    #[must_use]
    pub fn stage(&self, target: ParameterTargetId) -> Option<DispatchStage> {
        if !in_range(target) {
            return None;
        }
        self.ledger.query(to_target_id(target)).map(|obs| obs.stage)
    }
}
