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
    DispatchStage, FrameBaseMismatch, NodeReport, ParamBinding, ParamDispatchLedger,
    ParamTargetTable, ParamValueObservation, PluginNodeControl, TargetId, TargetResolution,
};
use ruviform_graph::NodeId;
use ruviform_media_core::{
    EngineFrame, MediaTime, MidiEvent, PlanGeneration, TimeBase, TransportCommand,
    TransportGeneration, TransportPlan, TransportPlanBuilder, TransportSpan,
};
use ruviform_plugin_core::{EventKind, ParamId, ParamInfo, TimedParamEvent};
use synvelis_project::ParameterTargetId;

use crate::musical_transport::PlanMusicalTransport;

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
    /// `frame_base` differs from the node's attach-time frame base —
    /// rejected before any plan is built or staged, so the node's
    /// live plan and queue are untouched (Ruviform #307 verified
    /// context). `expected`/`actual` keep the upstream
    /// [`FrameBaseMismatch`] values.
    FrameBaseMismatch(FrameBaseMismatch),
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
            Self::FrameBaseMismatch(e) => write!(f, "{e}"),
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
pub(crate) fn in_range(target: ParameterTargetId) -> bool {
    target.raw() <= u128::from(u64::MAX)
}

/// The drained result of one [`ParamDispatch::observe_cycle`]:
/// latest per-target ledger observations plus every scheduler
/// receipt drained this cycle. Receipts are `Copy` projections —
/// the `CommandId` correlation token stays inside the scheduler.
#[derive(Debug)]
pub struct DispatchCycle {
    /// Latest observation per synvelis target after report
    /// application and call-window projection.
    pub observations: Vec<(ParameterTargetId, ParamValueObservation)>,
    /// Every [`ruviform_media_core::AppliedReceipt`] drained this
    /// cycle, in drain order.
    pub receipts: Vec<ruviform_media_core::AppliedReceipt>,
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
    /// The node's staged clip MIDI content — `(engine frame,
    /// MidiEvent)` pairs merged into every plan this dispatcher
    /// mints (TIM-02 node delivery, synvelis#623). The node's live
    /// plan is single (`install_plan` replaces wholesale), so clip
    /// content and param batches can only coexist inside one
    /// merged plan; keeping the set here makes "a plan carries the
    /// node's full event picture" a dispatcher-level invariant —
    /// every mint path (`dispatch`, `dispatch_midi`,
    /// `clear_midi_plan`) produces it, never the callers.
    staged_midi: Vec<(i64, MidiEvent)>,
    /// Complete project-owned musical source applied to every plan this
    /// dispatcher mints. `None` is the honest unavailable state — never
    /// a partial tempo-map/PPQN pair or a fixed fallback.
    musical_transport: Option<PlanMusicalTransport>,
}

impl ParamDispatch {
    /// A dispatcher for `node` behind `control`, building plans in
    /// `frame_base` (the engine frame base — e.g.
    /// `TimeBase::sample_rate(48_000)`).
    ///
    /// `frame_base` **must equal** the node's attach-time
    /// `AttachConfig::frame_base` — verified through
    /// [`PluginNodeControl::check_frame_base`] (Ruviform #307) before
    /// anything is staged. A mismatch is a typed
    /// [`DispatchError::FrameBaseMismatch`] carrying the upstream
    /// expected/actual values; without the check a foreign base would
    /// silently scale event positions, `Seek` re-anchors, and
    /// observation windows.
    ///
    /// # Errors
    /// [`DispatchError::FrameBaseMismatch`] when `frame_base` differs
    /// from the attach-time base.
    pub fn new(
        control: PluginNodeControl,
        node: NodeId,
        frame_base: TimeBase,
    ) -> Result<Self, DispatchError> {
        control
            .check_frame_base(frame_base)
            .map_err(DispatchError::FrameBaseMismatch)?;
        Ok(Self {
            node,
            table: ParamTargetTable::new(),
            ledger: ParamDispatchLedger::new(),
            control,
            frame_base,
            next_generation: 1,
            last_plan: None,
            scheduler_gen: None,
            param_owners: BTreeMap::new(),
            staged_midi: Vec::new(),
            musical_transport: None,
        })
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

    /// The plan generation the node scheduler last reported live in a
    /// drained receipt. `None` means there is no scheduler evidence yet;
    /// it is never treated as proof that a staged install succeeded.
    pub(crate) const fn scheduler_plan_generation(&self) -> Option<PlanGeneration> {
        self.scheduler_gen
    }

    /// The node's staged clip MIDI content — merged into every plan
    /// this dispatcher mints. Empty on attach and after
    /// [`Self::clear_midi_plan`].
    #[must_use]
    pub fn staged_midi(&self) -> &[(i64, MidiEvent)] {
        &self.staged_midi
    }

    /// The last plan this dispatcher staged, if any. Test and evidence
    /// callers use this to inspect the exact generation, event payload,
    /// tempo revision, tempo map, and PPQN the node was given.
    #[must_use]
    pub fn last_plan(&self) -> Option<&Arc<TransportPlan<EventKind>>> {
        self.last_plan.as_ref()
    }

    /// Replaces the complete musical source future plans carry. Returns
    /// `true` when the persisted project basis changed. This method does
    /// not install a plan by itself; initial attachment must not mint an
    /// empty discontinuity, while existing plans are refreshed through
    /// [`Self::refresh_musical_plan`].
    pub fn set_musical_transport(&mut self, source: Option<PlanMusicalTransport>) -> bool {
        let changed = match (&self.musical_transport, &source) {
            (None, None) => false,
            (Some(current), Some(next)) => !current.same_project_basis(next),
            _ => true,
        };
        if changed {
            self.musical_transport = source;
        }
        changed
    }

    /// Whether the last staged plan already carries the currently desired
    /// musical source. No plan is vacuously current — absence of content
    /// must not trigger an empty install.
    #[must_use]
    pub fn last_plan_musical_is_current(&self) -> bool {
        let Some(plan) = &self.last_plan else {
            return true;
        };
        match &self.musical_transport {
            Some(source) => source.plan_carries(plan),
            None => plan.tempo_map.is_none() && plan.musical_ppqn.is_none(),
        }
    }

    /// Rebuilds the last staged plan with the current musical source,
    /// preserving every event position, source sequence, payload, and
    /// optional transport-generation binding. No `Play` is requested: a
    /// musical refresh must not re-anchor transport. Returns `None` when
    /// no plan has been staged yet.
    ///
    /// # Errors
    /// [`DispatchError::PositionUnrepresentable`] when a previously
    /// representable event cannot be rebuilt, or [`DispatchError::QueueFull`]
    /// when the node control queue refuses the install.
    pub fn refresh_musical_plan(
        &mut self,
    ) -> Result<Option<Arc<TransportPlan<EventKind>>>, DispatchError> {
        let Some(previous) = self.last_plan.clone() else {
            return Ok(None);
        };
        let mut builder = self.next_plan_builder();
        for event in &previous.events {
            let result = if let Some(generation) = event.transport_generation {
                builder.bound_event(
                    event.position,
                    event.source_sequence,
                    generation,
                    event.payload,
                )
            } else {
                builder.event(event.position, event.source_sequence, event.payload)
            };
            result.map_err(|_| DispatchError::PositionUnrepresentable)?;
        }
        let plan = Arc::new(builder.build());
        self.control
            .install_plan(Arc::clone(&plan), 0)
            .map_err(|_| DispatchError::QueueFull)?;
        self.ledger.mark_plan_replaced();
        self.ledger.record_plan(&plan, &self.table, self.node);
        self.last_plan = Some(Arc::clone(&plan));
        Ok(Some(plan))
    }

    fn next_plan_builder(&mut self) -> TransportPlanBuilder<EventKind> {
        let generation = PlanGeneration::new(self.next_generation);
        self.next_generation = self.next_generation.wrapping_add(1);
        let builder = self.control.plan_builder(generation);
        match &self.musical_transport {
            Some(source) => source.apply(builder),
            None => builder,
        }
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
        let writes: Vec<(
            ParameterTargetId,
            TimedParamEvent,
            Option<TransportGeneration>,
        )> = writes
            .iter()
            .map(|(target, event)| (*target, *event, None))
            .collect();
        self.dispatch_with_transport_generations(&writes)
    }

    /// [`Self::dispatch`] with an explicit optional transport-generation
    /// binding per parameter event. `None` uses the ordinary unbound
    /// builder path exactly as [`Self::dispatch`]; `Some(generation)` uses
    /// the upstream bound-event contract, so a transport discontinuity
    /// invalidates that one event instead of silently delivering it in a
    /// later position epoch.
    ///
    /// # Errors
    /// Same as [`Self::dispatch`].
    pub fn dispatch_with_transport_generations(
        &mut self,
        writes: &[(
            ParameterTargetId,
            TimedParamEvent,
            Option<TransportGeneration>,
        )],
    ) -> Result<Arc<TransportPlan<EventKind>>, DispatchError> {
        if writes.is_empty() {
            return Err(DispatchError::Empty);
        }
        for (target, event, _) in writes {
            let binding = self.resolve_write(*target)?;
            if binding.param != event.event.param_id {
                return Err(DispatchError::WriteRejected {
                    means: "dispatch".into(),
                    rejection: "param_mismatch".into(),
                });
            }
        }
        let mut builder = self.next_plan_builder();
        let mut seq = 0_u64;
        for (_, event, transport_generation) in writes {
            let result = if let Some(generation) = transport_generation {
                builder.bound_event(
                    event.position,
                    seq,
                    *generation,
                    EventKind::ParamValue(event.event),
                )
            } else {
                builder.event(event.position, seq, EventKind::ParamValue(event.event))
            };
            result.map_err(|_| DispatchError::PositionUnrepresentable)?;
            seq += 1;
        }
        // Merged plan (synvelis#623): the node's staged clip MIDI
        // rides in the same plan — the single-live-plan wholesale
        // replace would otherwise drop it on every param batch.
        // Params take the lower `source_sequence` values, so a
        // same-position param/MIDI pair orders param first.
        for (frame, midi) in &self.staged_midi {
            builder
                .event(
                    MediaTime::new(*frame, self.frame_base),
                    seq,
                    EventKind::Midi(*midi),
                )
                .map_err(|_| DispatchError::PositionUnrepresentable)?;
            seq += 1;
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

    /// Stages `events` — `(engine frame, MidiEvent)` pairs — as the
    /// node's staged clip MIDI content and installs one
    /// generation-tagged [`TransportPlan`] of `EventKind::Midi`
    /// payloads on the node (TIM-02, issues #126/#623).
    ///
    /// The install path mirrors [`Self::dispatch`]: the plan is built
    /// through the node's verified `plan_builder` (attach-time frame
    /// base), minted under `next_generation`, staged with
    /// `install_plan(due = 0)`, previously scheduled ledger entries
    /// are marked `PlanReplaced`, and `last_plan` tracks the staged
    /// plan. `record_plan` registers only `ParamValue` events, so a
    /// MIDI plan records zero ledger entries — param observation is
    /// unaffected.
    ///
    /// On success `events` become [`Self::staged_midi`]: every later
    /// [`Self::dispatch`] merges them into the param batch's plan so
    /// the single live plan keeps carrying the clip content instead
    /// of being wiped by the next wholesale replace.
    ///
    /// Unlike [`Self::dispatch`] no `Play` is requested: `Play`
    /// re-anchors the node's transport position, and a bare `Play`
    /// after `Stop` would restart position at 0, diverging from the
    /// engine cursor the driver owns. Transport Play/Seek anchoring
    /// stays with `HeadlessDriver` (activation, `seek`,
    /// `resume_attached_transport`) or the consumer's own seam
    /// ([`crate::RoutingControlDispatch::stage_midi_plan`]);
    /// standalone consumers start transport through
    /// [`Self::control`] themselves.
    ///
    /// The node's live plan is single — `install_plan` replaces it
    /// wholesale. The plan generation change is itself a transport
    /// discontinuity: the node releases sounding notes and chases
    /// the new plan's prefix before delivery resumes.
    ///
    /// # Errors
    /// [`DispatchError::Empty`] when `events` is empty (use
    /// [`Self::clear_midi_plan`] to drop staged content);
    /// [`DispatchError::PositionUnrepresentable`] when a frame
    /// cannot convert to the plan frame base;
    /// [`DispatchError::QueueFull`] when the node's bounded queue
    /// drops the staged install.
    pub fn dispatch_midi(
        &mut self,
        events: &[(i64, MidiEvent)],
    ) -> Result<Arc<TransportPlan<EventKind>>, DispatchError> {
        if events.is_empty() {
            return Err(DispatchError::Empty);
        }
        let preserved: Vec<_> = self
            .last_plan
            .iter()
            .flat_map(|plan| plan.events.iter().copied())
            .filter(|event| !matches!(event.payload, EventKind::Midi(_)))
            .collect();
        let mut builder = self.next_plan_builder();
        for event in &preserved {
            let result = if let Some(generation) = event.transport_generation {
                builder.bound_event(
                    event.position,
                    event.source_sequence,
                    generation,
                    event.payload,
                )
            } else {
                builder.event(event.position, event.source_sequence, event.payload)
            };
            result.map_err(|_| DispatchError::PositionUnrepresentable)?;
        }
        let mut seq = preserved
            .iter()
            .map(|event| event.source_sequence)
            .max()
            .map_or(0, |seq| seq.saturating_add(1));
        for (frame, midi) in events {
            builder
                .event(
                    MediaTime::new(*frame, self.frame_base),
                    seq,
                    EventKind::Midi(*midi),
                )
                .map_err(|_| DispatchError::PositionUnrepresentable)?;
            seq = seq.saturating_add(1);
        }
        let plan = Arc::new(builder.build());
        self.control
            .install_plan(Arc::clone(&plan), 0)
            .map_err(|_| DispatchError::QueueFull)?;
        self.ledger.mark_plan_replaced();
        self.ledger.record_plan(&plan, &self.table, self.node);
        self.last_plan = Some(Arc::clone(&plan));
        self.staged_midi = events.to_vec();
        Ok(plan)
    }

    /// Drops the node's staged clip MIDI content and installs a plan
    /// without MIDI events — the deliberate wholesale replace that
    /// releases sounding notes through the generation-change
    /// discontinuity (synvelis#623). Non-MIDI events already owned by
    /// the prior plan are preserved; only clip MIDI is cleared. The
    /// MIDI-free install is
    /// exactly the wipe [`Self::dispatch`]/[`Self::dispatch_midi`]
    /// guard against with [`DispatchError::Empty`], routed through
    /// an explicit API so it can never happen by accident.
    ///
    /// No `Play` is requested — same anchoring contract as
    /// [`Self::dispatch_midi`]. `staged_midi` clears only on a
    /// successful install: a `QueueFull` leaves both the node's
    /// live plan and the staged set untouched, so a retry — or the
    /// next [`Self::dispatch`] — still sees the real content.
    ///
    /// # Errors
    /// [`DispatchError::QueueFull`] when the node's bounded queue
    /// drops the staged install.
    pub fn clear_midi_plan(&mut self) -> Result<Arc<TransportPlan<EventKind>>, DispatchError> {
        let preserved: Vec<_> = self
            .last_plan
            .iter()
            .flat_map(|plan| plan.events.iter().copied())
            .filter(|event| !matches!(event.payload, EventKind::Midi(_)))
            .collect();
        let mut builder = self.next_plan_builder();
        for event in &preserved {
            let result = if let Some(generation) = event.transport_generation {
                builder.bound_event(
                    event.position,
                    event.source_sequence,
                    generation,
                    event.payload,
                )
            } else {
                builder.event(event.position, event.source_sequence, event.payload)
            };
            result.map_err(|_| DispatchError::PositionUnrepresentable)?;
        }
        let plan = Arc::new(builder.build());
        self.control
            .install_plan(Arc::clone(&plan), 0)
            .map_err(|_| DispatchError::QueueFull)?;
        self.ledger.mark_plan_replaced();
        self.ledger.record_plan(&plan, &self.table, self.node);
        self.last_plan = Some(Arc::clone(&plan));
        self.staged_midi.clear();
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
        self.observe_cycle(span, call_start, call_frames)
            .observations
    }

    /// [`Self::observe`] plus the drained scheduler receipts — the
    /// PC-C3-05 trace adapter consumes both halves: observations for
    /// the `dispatched` stage and receipts for `scheduled`/`applied`.
    /// Drain order, the scheduler-generation projection gate and the
    /// report-application order are identical to [`Self::observe`].
    pub fn observe_cycle(
        &mut self,
        span: &TransportSpan,
        call_start: EngineFrame,
        call_frames: u32,
    ) -> DispatchCycle {
        let reports = self.control.drain_reports();
        let mut receipts = Vec::new();
        for report in &reports {
            match report {
                NodeReport::Receipt(receipt) => receipts.push(*receipt),
                NodeReport::UndeliverableSpan { .. }
                | NodeReport::EventScratchOverflow
                | NodeReport::EndpointError { .. } => {}
                _ => {
                    self.ledger.apply_report(report);
                }
            }
        }
        // Receipts carry the scheduler's live plan generation — the
        // last one in the drain is the newest truth. An install
        // rejected at apply time leaves it on the previous plan, so a
        // mismatch here proves `last_plan` never ran and projecting
        // through it would mint `Dispatched` for undelivered events.
        // Before the first receipt there is no mismatch evidence yet;
        // standalone/fabricated-span consumers retain the pre-existing
        // projection contract, while any drained scheduler truth gates it.
        if let Some(receipt) = receipts.last() {
            self.scheduler_gen = Some(receipt.plan_generation);
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
        DispatchCycle {
            observations: self
                .ledger
                .observations()
                .into_iter()
                .map(|(tid, obs)| (ParameterTargetId::new(u128::from(tid.0)), obs))
                .collect(),
            receipts,
        }
    }

    /// Records a plugin-reported `value` for `target` in the ledger —
    /// the latest eligible `Scheduled`/`Dispatched` entry becomes
    /// [`DispatchStage::Reported`]. Returns `false` when no eligible
    /// entry exists or `target` is out of the bridge id range.
    /// Non-RT; visible via [`Self::observe`]/[`Self::observe_cycle`].
    pub fn record_reported(
        &mut self,
        target: ParameterTargetId,
        value: ruviform_plugin_core::ParamValue,
    ) -> bool {
        if !in_range(target) {
            return false;
        }
        self.ledger.record_reported(to_target_id(target), value)
    }

    /// Declares that no plugin→host reporting channel exists: every
    /// still-pending ledger entry moves to
    /// [`DispatchStage::Unavailable`] — the honest terminal stage.
    /// Non-RT; visible via [`Self::observe`]/[`Self::observe_cycle`].
    pub fn mark_reporting_unavailable(&mut self) {
        self.ledger.mark_reporting_unavailable();
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
