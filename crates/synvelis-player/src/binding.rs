//! PC-C4 plugin parameter binding — verified frame base, capability
//! gate and generation-consistent routing of project control targets
//! onto a live plugin node's parameters (issue #101, fake plugin port
//! slice).
//!
//! [`ParamDispatch`](crate::ParamDispatch) already owns the
//! `TargetId → (node, param)` table and plan staging; this module adds
//! the layer the issue requires on top of it:
//!
//! - [`AttachClockContext`] — the attach-time frame base, minted only
//!   from [`PluginNodeControl::frame_base`] (Ruviform #307 verified
//!   context). No constructor accepts an externally claimed
//!   [`TimeBase`], so an unverified base can never enter a binding.
//! - [`PluginParameterBinding`] — the runtime record pinning a project
//!   [`ParameterTargetId`] to a stable plugin instance, `NodeId`,
//!   plugin-local [`ParamId`], the instance's authored [`PluginRef`]
//!   identity, capability generation, graph generation and the
//!   verified clock context.
//! - [`PluginBindingSession`] — one attached instance's binding table
//!   plus `bind`/`revalidate`/`invalidate`/`dispatch`/`eval_event`.
//!   `dispatch` validates **every** `(binding, event)` pair before any
//!   staging — one bad pin rejects the whole batch atomically, and a
//!   foreign instance/node/generation/clock record can never reach a
//!   different plugin's parameter (wrong-target delivery is 0 by
//!   construction: the node's control channel is dedicated to this
//!   instance and the param id is checked against the pinned record).
//!
//! All operations are non-RT orchestration: plan construction,
//! validation and staging happen off the audio path. `eval_rt` /
//! `lower_event` / `check_param_write` upstream allocation-free
//! contracts are consumed unchanged.
//!
//! Rejection contract: every failure is a typed [`BindingError`] (or
//! the wrapped [`DispatchError`]/project [`Error`]); upstream
//! `ParamWriteRejection` tokens are preserved verbatim and no
//! rejection is ever converted to a success.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use ruviform_audio_plugin_bridge::{FrameBaseMismatch, PluginNodeControl};
use ruviform_graph::NodeId;
use ruviform_media_core::{MediaTime, TimeBase, TransportPlan};
use ruviform_plugin_core::{
    EventKind, ParamId, ParamInfo, ParamWriteMeans, TimedParamEvent, check_param_write,
};
use synvelis_project::{
    BeatPoint, ControlOwner, EvalPlan, EvalScratch, ParameterTargetId, PluginRef, ProcessorId,
    Project, dispatch_check, lower_event, modulation_allowed, plugin_parameter_instance_binding,
};

use crate::dispatch::{DispatchError, ParamDispatch};

/// Runtime identity of one plugin instance inside a binding session.
///
/// Issued by the project's persisted identity store
/// (`Project::plugin_instances`, schema v13 — PC-C4-05): the same
/// authored processor/plugin pair carries the same id across
/// save→reload, engine restart and re-attach. A rebind to a different
/// `PluginRef` (descriptor, version or `capability_revision` drift)
/// is a different authored identity and mints a new id, so a binding
/// record pinned to an old plugin identity is structurally stale.
/// Staleness across generations is still detected by the
/// capability/graph generation pins — the instance id alone no longer
/// rotates per attach.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StablePluginInstanceId(u64);

impl StablePluginInstanceId {
    /// Wraps a caller-issued raw identity.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw identity value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The attach-time clock context of one plugin node — the verified
/// frame base from Ruviform #307.
///
/// The only mint path is [`Self::of`], which reads
/// [`PluginNodeControl::frame_base`] — the same
/// `AttachConfig::frame_base` the node's scheduler runs on. There is
/// deliberately no constructor taking an arbitrary [`TimeBase`]: a
/// value obtained anywhere else must pass [`Self::check_frame_base`]
/// before use. Clock epoch, transport generation and plan generation
/// are independent axes and do not mutate this context.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AttachClockContext {
    frame_base: TimeBase,
}

impl AttachClockContext {
    /// Mints the verified context from the node's attach-time frame
    /// base.
    #[must_use]
    pub fn of(control: &PluginNodeControl) -> Self {
        Self {
            frame_base: control.frame_base(),
        }
    }

    /// The verified attach-time frame base.
    #[must_use]
    pub const fn frame_base(self) -> TimeBase {
        self.frame_base
    }

    /// Verifies `base` against the attach-time frame base, returning
    /// the upstream [`FrameBaseMismatch`] carrying expected/actual.
    ///
    /// # Errors
    /// [`FrameBaseMismatch`] when `base` differs.
    pub fn check_frame_base(&self, base: TimeBase) -> Result<(), FrameBaseMismatch> {
        if base == self.frame_base {
            Ok(())
        } else {
            Err(FrameBaseMismatch {
                expected: self.frame_base,
                actual: base,
            })
        }
    }
}

/// A bound project target → plugin parameter record with every
/// generation pin the dispatch path must re-check.
///
/// `PartialEq` compares the whole record — a record minted by one
/// session never equals one minted by another (instance id, node and
/// generations differ), which is exactly the stale/foreign-binding
/// detection [`PluginBindingSession::dispatch`] performs.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginParameterBinding {
    /// The project control target this binding serves.
    pub synvelis_target: ParameterTargetId,
    /// The authored plugin identity the bound instance was connected
    /// as — the connect-time `ProcessorBinding::Resolved` `PluginRef`
    /// (PC-C4-02). A record minted against one plugin identity never
    /// equals one minted against another.
    pub plugin: PluginRef,
    /// The plugin instance this binding was minted for.
    pub instance_id: StablePluginInstanceId,
    /// The generation-local graph node owning the parameter.
    pub node: NodeId,
    /// The plugin-local parameter id (raw `ParamId`).
    pub plugin_param: ParamId,
    /// Capability generation pinned at bind time — bumped by the
    /// composition root when the plugin's parameter surface changes.
    pub capability_generation: u64,
    /// Graph/control generation pinned at bind time — bumped when the
    /// control graph plan is swapped.
    pub graph_generation: u64,
    /// The verified attach-time clock context.
    pub attach_clock_context: AttachClockContext,
}

/// Typed failures of the binding/session surface. Upstream rejection
/// tokens are preserved verbatim; nothing is converted to success.
#[non_exhaustive]
#[derive(Debug)]
pub enum BindingError {
    /// A supplied frame base differs from the verified attach-time
    /// base (upstream [`FrameBaseMismatch`] values preserved).
    FrameBaseMismatch(FrameBaseMismatch),
    /// The binding record belongs to a different plugin instance.
    InstanceMismatch,
    /// The binding record or the supplied dispatcher names a
    /// different graph node than this session's.
    NodeMismatch,
    /// The record's capability generation is stale relative to the
    /// session's pinned generation.
    CapabilityGeneration {
        /// The session's pinned generation.
        expected: u64,
        /// The generation the record carries.
        actual: u64,
    },
    /// The record's graph generation is stale relative to the
    /// session's pinned generation.
    GraphGeneration {
        /// The session's pinned generation.
        expected: u64,
        /// The generation the record carries.
        actual: u64,
    },
    /// The supplied [`EvalPlan`] no longer matches the project
    /// ([`EvalPlan::is_stale`]) — a graph-plan swap went undetected.
    PlanStale,
    /// No binding exists for the target in this session.
    TargetNotBound,
    /// The supplied record differs from the session's recorded
    /// binding for the target (stale or re-bound record).
    BindingDefunct,
    /// The plugin's live `params` enumeration no longer contains the
    /// bound parameter id.
    ParamRemoved,
    /// The project target's owner is not this session's processor —
    /// Strip/Send/Clip/etc. targets can never bind to a plugin.
    OwnerMismatch,
    /// The stage-1 semantic binding resolves a different plugin
    /// identity than this session's pinned instance identity — the
    /// project rebound the processor (or the session was opened
    /// against another plugin) and the target's contract no longer
    /// describes this instance (PC-C4-02).
    PluginMismatch {
        /// The plugin identity the session pinned at attach.
        expected: PluginRef,
        /// The plugin identity the live semantic binding resolves.
        actual: PluginRef,
    },
    /// The target carries no [`synvelis_project::TargetCapability`] —
    /// there is no plugin param contract to bind against.
    CapabilityMissing,
    /// The declared [`synvelis_project::TargetCapability`] drifted
    /// from the plugin's live [`ParamInfo`] (flags or plain range).
    CapabilityMismatch {
        /// The parameter whose declaration drifted.
        param: ParamId,
    },
    /// An enabled modulation assignment targets a discrete
    /// [`synvelis_project::ValueKind`] — continuous modulation is not
    /// meaningful there and is refused rather than silently applied.
    UnsupportedModulation {
        /// The target carrying the unsupported assignment.
        target: ParameterTargetId,
    },
    /// The project has no control target with this id.
    UnknownTarget(ParameterTargetId),
    /// A write was rejected. `rejection` keeps the upstream
    /// `param_*` token (`param_readonly`, `param_not_automatable`,
    /// `param_unknown`, `param_unsupported_means`, …) or a
    /// binding-level token.
    WriteRejected {
        /// The attempted means/stage.
        means: String,
        /// The rejection token.
        rejection: String,
    },
    /// The underlying [`ParamDispatch`] rejected the operation.
    Dispatch(DispatchError),
    /// A project-layer operation failed ([`synvelis_project::Error`]).
    Project(synvelis_project::Error),
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameBaseMismatch(e) => write!(f, "{e}"),
            Self::InstanceMismatch => write!(f, "binding belongs to another plugin instance"),
            Self::NodeMismatch => write!(f, "binding/dispatcher node differs from session node"),
            Self::CapabilityGeneration { expected, actual } => {
                write!(
                    f,
                    "capability generation stale: expected {expected}, got {actual}"
                )
            }
            Self::GraphGeneration { expected, actual } => {
                write!(
                    f,
                    "graph generation stale: expected {expected}, got {actual}"
                )
            }
            Self::PlanStale => write!(f, "eval plan is stale for the current project"),
            Self::TargetNotBound => write!(f, "target has no binding in this session"),
            Self::BindingDefunct => {
                write!(
                    f,
                    "binding record differs from the session's recorded binding"
                )
            }
            Self::ParamRemoved => write!(f, "plugin no longer enumerates the bound param"),
            Self::OwnerMismatch => {
                write!(f, "target owner is not this session's processor")
            }
            Self::PluginMismatch { expected, actual } => {
                write!(
                    f,
                    "plugin identity mismatch: pinned {expected:?}, resolved {actual:?}"
                )
            }
            Self::CapabilityMissing => {
                write!(f, "target carries no plugin capability contract")
            }
            Self::CapabilityMismatch { param } => {
                write!(f, "declared capability drifted from live param {param:?}")
            }
            Self::UnsupportedModulation { target } => {
                write!(
                    f,
                    "continuous modulation unsupported for discrete target {target}"
                )
            }
            Self::UnknownTarget(target) => write!(f, "unknown control target {target}"),
            Self::WriteRejected { means, rejection } => {
                write!(f, "write via {means} rejected: {rejection}")
            }
            Self::Dispatch(e) => write!(f, "{e}"),
            Self::Project(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BindingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dispatch(e) => Some(e),
            Self::Project(e) => Some(e),
            _ => None,
        }
    }
}

impl From<DispatchError> for BindingError {
    fn from(e: DispatchError) -> Self {
        Self::Dispatch(e)
    }
}

impl From<synvelis_project::Error> for BindingError {
    fn from(e: synvelis_project::Error) -> Self {
        Self::Project(e)
    }
}

fn write_rejected(error: synvelis_project::Error) -> BindingError {
    match error {
        synvelis_project::Error::WriteRejected { means, rejection } => {
            BindingError::WriteRejected { means, rejection }
        }
        other => BindingError::Project(other),
    }
}

/// The binding table + verified context of **one** attached plugin
/// instance. Non-RT only — every method runs on the orchestration
/// side; the RT path never sees this type.
///
/// A session does not own its [`ParamDispatch`] — the driver/adapter
/// keeps that ownership — so every method takes `&mut ParamDispatch`
/// (or `&ParamDispatch` for the read-only checks) explicitly. The
/// session verifies the dispatcher's node and frame base against its
/// pinned context on every call, so a dispatcher built for another
/// node or a foreign frame base fails closed.
pub struct PluginBindingSession {
    /// Stable runtime identity of the attached plugin instance.
    instance_id: StablePluginInstanceId,
    /// The generation-local graph node this session binds.
    node: NodeId,
    /// The semantic processor that owns bindable targets.
    processor: ProcessorId,
    /// The authored plugin identity the attached instance was
    /// connected as — connect-time evidence vended through
    /// [`crate::ProcessorControl::plugin`], never the live project's
    /// current claim (PC-C4-02).
    plugin: PluginRef,
    /// Verified attach-time clock context.
    clock: AttachClockContext,
    /// Capability generation pinned at attach.
    capability_generation: u64,
    /// Graph/control generation pinned at attach.
    graph_generation: u64,
    /// `synvelis target → recorded binding`.
    bindings: BTreeMap<ParameterTargetId, PluginParameterBinding>,
}

impl PluginBindingSession {
    /// Opens a session for the plugin instance behind `control`.
    ///
    /// The verified clock context is minted from
    /// [`PluginNodeControl::frame_base`] — the attach-time
    /// `AttachConfig::frame_base` — so no unverified `TimeBase` can
    /// enter the session. `plugin` is the instance's **own** authored
    /// identity — the `ProcessorBinding::Resolved` `PluginRef`
    /// captured when this generation's instance was connected (vended
    /// via [`crate::ProcessorControl::plugin`]), *not* the live
    /// project's current binding: a later project rebind does not
    /// change what this instance is. `graph_generation`/
    /// `capability_generation` are the caller's pins for the
    /// control-graph plan and the plugin's parameter surface;
    /// re-attach, plugin reload and graph-plan swaps mint a new
    /// session rather than mutating these. `instance_id` is the
    /// persisted identity hydrated from `Project::plugin_instances`
    /// (PC-C4-05) — stable per authored processor/plugin pair, new
    /// only when the authored `PluginRef` itself changes.
    ///
    /// Infallible by construction: the context is read, never claimed.
    #[must_use]
    pub fn attach(
        control: &PluginNodeControl,
        node: NodeId,
        processor: ProcessorId,
        plugin: PluginRef,
        graph_generation: u64,
        capability_generation: u64,
        instance_id: StablePluginInstanceId,
    ) -> Self {
        Self {
            instance_id,
            node,
            processor,
            plugin,
            clock: AttachClockContext::of(control),
            capability_generation,
            graph_generation,
            bindings: BTreeMap::new(),
        }
    }

    /// The stable instance identity this session pins.
    #[must_use]
    pub const fn instance_id(&self) -> StablePluginInstanceId {
        self.instance_id
    }

    /// The graph node this session binds.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// The semantic processor owning this session's targets.
    #[must_use]
    pub const fn processor(&self) -> ProcessorId {
        self.processor
    }

    /// The authored plugin identity this session's instance was
    /// connected as (PC-C4-02).
    #[must_use]
    pub const fn plugin(&self) -> &PluginRef {
        &self.plugin
    }

    /// The verified attach-time clock context.
    #[must_use]
    pub const fn clock_context(&self) -> AttachClockContext {
        self.clock
    }

    /// The verified attach-time frame base (convenience for
    /// [`AttachClockContext::frame_base`]).
    #[must_use]
    pub const fn frame_base(&self) -> TimeBase {
        self.clock.frame_base()
    }

    /// The pinned capability generation.
    #[must_use]
    pub const fn capability_generation(&self) -> u64 {
        self.capability_generation
    }

    /// The pinned graph generation.
    #[must_use]
    pub const fn graph_generation(&self) -> u64 {
        self.graph_generation
    }

    /// Whether `capability_generation`/`graph_generation` still match
    /// this session's pins — `false` means the session is stale and a
    /// new attach is required.
    #[must_use]
    pub const fn is_current(&self, capability_generation: u64, graph_generation: u64) -> bool {
        self.capability_generation == capability_generation
            && self.graph_generation == graph_generation
    }

    /// The recorded binding for `target`, if any.
    #[must_use]
    pub fn binding(&self, target: ParameterTargetId) -> Option<&PluginParameterBinding> {
        self.bindings.get(&target)
    }

    /// Iterates every recorded binding.
    pub fn bindings(&self) -> impl Iterator<Item = &PluginParameterBinding> {
        self.bindings.values()
    }

    /// Verifies `dispatch` drives this session's node in the verified
    /// frame base — the gate every method applies before touching the
    /// binding table or staging anything.
    fn check_dispatch(&self, dispatch: &ParamDispatch) -> Result<(), BindingError> {
        if dispatch.node() != self.node {
            return Err(BindingError::NodeMismatch);
        }
        self.clock
            .check_frame_base(dispatch.frame_base())
            .map_err(BindingError::FrameBaseMismatch)
    }

    /// Binds `target` to its declared plugin parameter, gated by the
    /// project capability contract **and** the plugin's live
    /// `params` enumeration.
    ///
    /// Checks, in order: dispatcher node/frame-base pin → target
    /// exists → `owner == ControlOwner::Processor(this.processor)` →
    /// capability present → **canonical
    /// `plugin_parameter_instance_binding` resolution** (processor
    /// registered + `Resolved` + plugin identity matches this
    /// session's pin — PC-C4-02 — then persisted instance identity
    /// matches the attach-time pin — PC-C4 plugin-binding) → live
    /// enumeration contains the param → declared capability matches
    /// live `ParamInfo` → `check_param_write`/`dispatch_check` accept
    /// `TimedEvent` → no enabled modulation on a discrete `ValueKind`
    /// → [`ParamDispatch::bind`]. Any failure is typed and leaves
    /// both tables unchanged.
    ///
    /// `params` must be the plugin's **current** enumeration — after a
    /// plugin re-enumerates parameters the caller runs
    /// [`Self::revalidate`] before trusting existing bindings.
    ///
    /// # Errors
    /// [`BindingError`] per the check order above, or
    /// [`BindingError::Dispatch`] from the underlying bind.
    pub fn bind(
        &mut self,
        dispatch: &mut ParamDispatch,
        project: &Project,
        target: ParameterTargetId,
        params: &[ParamInfo],
    ) -> Result<PluginParameterBinding, BindingError> {
        self.check_dispatch(dispatch)?;
        let project_target = project
            .control
            .targets
            .get(&target)
            .ok_or(BindingError::UnknownTarget(target))?;
        if project_target.owner != ControlOwner::Processor(self.processor) {
            return Err(BindingError::OwnerMismatch);
        }
        let capability = project_target
            .capability
            .ok_or(BindingError::CapabilityMissing)?;
        // PC-C4-02: the stage-1 semantic binding is the single
        // authority for `(processor, plugin, param_id)` — resolve it
        // before minting any runtime pin. A processor missing from
        // `routing.processors` or `Unresolved` fails closed as a
        // project-layer error; a semantic plugin identity different
        // from this instance's connect-time pin is `PluginMismatch`.
        // PC-C4 plugin-binding: the canonical
        // `ParameterTarget → PluginInstance` mapping additionally
        // resolves the persisted instance identity — the store must
        // vouch for this session's attach-time pin, so a re-issued
        // id (plugin rebind A→B→A), a missing entry, or plugin drift
        // fails closed as `InstanceMismatch` after the plugin check.
        let resolved = plugin_parameter_instance_binding(project, target)?;
        let semantic = resolved.semantic;
        if semantic.plugin != self.plugin {
            return Err(BindingError::PluginMismatch {
                expected: self.plugin.clone(),
                actual: semantic.plugin,
            });
        }
        let instance = resolved.instance.ok_or(BindingError::InstanceMismatch)?;
        if instance.get() != self.instance_id.get() {
            return Err(BindingError::InstanceMismatch);
        }
        let param = semantic.param_id;
        let info = params
            .iter()
            .find(|info| info.id == param)
            .ok_or(BindingError::ParamRemoved)?;
        let declared_flags = capability.flags()?;
        // Bit-exact comparison: the declared contract and the live
        // enumeration must agree on the same f64 values — any drift,
        // however small, is a capability mismatch, not an epsilon.
        if declared_flags != info.flags
            || capability.min_value.to_bits() != info.min_value.to_bits()
            || capability.max_value.to_bits() != info.max_value.to_bits()
            || capability.default_value.to_bits() != info.default_value.to_bits()
        {
            return Err(BindingError::CapabilityMismatch { param });
        }
        check_param_write(info, ParamWriteMeans::TimedEvent).map_err(|rejection| {
            BindingError::WriteRejected {
                means: "bind".into(),
                rejection: rejection.to_string(),
            }
        })?;
        dispatch_check(project_target, ParamWriteMeans::TimedEvent).map_err(write_rejected)?;
        if !modulation_allowed(project_target.kind)
            && project
                .control
                .assignments
                .values()
                .any(|a| a.target == target && a.enabled)
        {
            return Err(BindingError::UnsupportedModulation { target });
        }
        dispatch.bind(target, param, params)?;
        let binding = PluginParameterBinding {
            synvelis_target: target,
            plugin: self.plugin.clone(),
            instance_id: self.instance_id,
            node: self.node,
            plugin_param: param,
            capability_generation: self.capability_generation,
            graph_generation: self.graph_generation,
            attach_clock_context: self.clock,
        };
        self.bindings.insert(target, binding.clone());
        Ok(binding)
    }

    /// Revalidates `target`'s binding against the plugin's live
    /// `params` enumeration (e.g. after the plugin re-enumerated or a
    /// GUI-side parameter disappeared). Returns `false` — and drops
    /// the session's record so later dispatch is `BindingDefunct` —
    /// when the binding went defunct.
    pub fn revalidate(
        &mut self,
        dispatch: &mut ParamDispatch,
        target: ParameterTargetId,
        params: &[ParamInfo],
    ) -> bool {
        // A dispatcher that fails the node/frame-base pin cannot vouch
        // for this session — treat the binding as unverifiable and drop
        // it rather than trusting a foreign table's verdict.
        if self.check_dispatch(dispatch).is_err() {
            self.bindings.remove(&target);
            return false;
        }
        if dispatch.revalidate(target, params) {
            true
        } else {
            self.bindings.remove(&target);
            false
        }
    }

    /// Invalidates every binding — pair with
    /// [`ParamDispatch::invalidate_node`] when the node is retired or
    /// the plugin re-attached. All recorded bindings become defunct;
    /// a fresh [`Self::attach`] + [`Self::bind`] is required.
    pub fn invalidate(&mut self, dispatch: &mut ParamDispatch) {
        dispatch.invalidate_node();
        self.bindings.clear();
    }

    /// Validates every `(binding, event)` pair against the session's
    /// pins, then stages the whole batch through
    /// [`ParamDispatch::dispatch`] in one plan.
    ///
    /// Validation is **atomic**: all records are checked before any
    /// event is staged, so one foreign/stale pin rejects the entire
    /// batch and nothing reaches the plugin. Per record: the binding
    /// must be the session's recorded binding for its target
    /// ([`BindingError::BindingDefunct`] on drift), its instance/node/
    /// capability/graph/clock pins must match the session
    /// ([`BindingError::InstanceMismatch`], [`BindingError::NodeMismatch`],
    /// [`BindingError::CapabilityGeneration`],
    /// [`BindingError::GraphGeneration`], [`BindingError::FrameBaseMismatch`]),
    /// and `event.event.param_id` must equal the pinned
    /// `plugin_param` (`param_mismatch` — never a silent redirect to
    /// another parameter).
    ///
    /// # Errors
    /// [`BindingError`] on the first failed pin; the underlying
    /// [`DispatchError`] (`Empty`, `QueueFull`,
    /// `PositionUnrepresentable`, `WriteRejected`) is preserved via
    /// [`BindingError::Dispatch`].
    pub fn dispatch(
        &self,
        dispatch: &mut ParamDispatch,
        writes: &[(PluginParameterBinding, TimedParamEvent)],
    ) -> Result<Arc<TransportPlan<EventKind>>, BindingError> {
        if writes.is_empty() {
            return Err(DispatchError::Empty.into());
        }
        self.check_dispatch(dispatch)?;
        let mut batch = Vec::with_capacity(writes.len());
        for (binding, event) in writes {
            let recorded = self
                .bindings
                .get(&binding.synvelis_target)
                .ok_or(BindingError::TargetNotBound)?;
            if binding.instance_id != self.instance_id {
                return Err(BindingError::InstanceMismatch);
            }
            if binding.node != self.node {
                return Err(BindingError::NodeMismatch);
            }
            if binding.capability_generation != self.capability_generation {
                return Err(BindingError::CapabilityGeneration {
                    expected: self.capability_generation,
                    actual: binding.capability_generation,
                });
            }
            if binding.graph_generation != self.graph_generation {
                return Err(BindingError::GraphGeneration {
                    expected: self.graph_generation,
                    actual: binding.graph_generation,
                });
            }
            if binding.plugin != self.plugin {
                return Err(BindingError::PluginMismatch {
                    expected: self.plugin.clone(),
                    actual: binding.plugin.clone(),
                });
            }
            self.clock
                .check_frame_base(binding.attach_clock_context.frame_base())
                .map_err(BindingError::FrameBaseMismatch)?;
            if binding != recorded {
                return Err(BindingError::BindingDefunct);
            }
            if binding.plugin_param != event.event.param_id {
                return Err(DispatchError::WriteRejected {
                    means: "dispatch".into(),
                    rejection: "param_mismatch".into(),
                }
                .into());
            }
            batch.push((binding.synvelis_target, *event));
        }
        dispatch.dispatch(&batch).map_err(BindingError::Dispatch)
    }

    /// Evaluates `target`'s control value at `pos` through the
    /// compiled [`EvalPlan`] (automation + manual + modulation +
    /// depth drivers, DAG-cycle-safe by construction) and lowers it
    /// to a [`TimedParamEvent`] at `position`.
    ///
    /// Fails closed: the target must be bound in this session
    /// ([`BindingError::TargetNotBound`]), exist in the project
    /// ([`BindingError::UnknownTarget`]), and pass
    /// [`lower_event`]'s capability/write gate — including rejecting
    /// means the contract does not support (`Immediate`, `PerNote`).
    /// Plan freshness is **not** re-checked per event — the caller
    /// gates the control surface with [`Self::check_plan`] at
    /// orchestration boundaries (a graph-plan swap requires
    /// recompile/re-attach anyway). `manual` is the pre-modulation
    /// reference for `target` only,
    /// matching [`EvalPlan::eval_rt`]'s semantics; automation+manual
    /// composition is the plan's existing behavior, not re-implemented
    /// here.
    ///
    /// # Errors
    /// [`BindingError::TargetNotBound`], [`BindingError::UnknownTarget`],
    /// [`BindingError::Project`] from `eval_rt`/`lower_event`,
    /// [`BindingError::WriteRejected`] when the declared capability
    /// rejects `means`.
    #[expect(clippy::too_many_arguments)]
    pub fn eval_event(
        &self,
        project: &Project,
        plan: &EvalPlan,
        scratch: &mut EvalScratch,
        target: ParameterTargetId,
        pos: BeatPoint,
        manual: Option<f64>,
        position: MediaTime,
        means: ParamWriteMeans,
    ) -> Result<TimedParamEvent, BindingError> {
        if !self.bindings.contains_key(&target) {
            return Err(BindingError::TargetNotBound);
        }
        let project_target = project
            .control
            .targets
            .get(&target)
            .ok_or(BindingError::UnknownTarget(target))?;
        let host_normalized = plan
            .eval_rt(project, target, pos, manual, scratch)
            .map_err(BindingError::Project)?;
        lower_event(project_target, host_normalized, position, means).map_err(write_rejected)
    }

    /// Verifies `plan` still matches `project` — the graph-plan-swap
    /// gate. [`EvalPlan::is_stale`] recompiles and compares exactly
    /// what `ControlGraph::compile` froze; a stale plan means the
    /// control surface moved and the caller must recompile/re-attach
    /// before dispatching further events.
    ///
    /// # Errors
    /// [`BindingError::PlanStale`] when the plan diverges.
    pub fn check_plan(&self, plan: &EvalPlan, project: &Project) -> Result<(), BindingError> {
        if plan.is_stale(project) {
            Err(BindingError::PlanStale)
        } else {
            Ok(())
        }
    }
}
