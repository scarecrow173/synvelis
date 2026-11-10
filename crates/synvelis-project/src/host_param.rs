//! Host Parameter authority lookups and the stage-1 semantic plugin
//! binding (issue #122, CTL-02).
//!
//! `Project::control` is the single persisted authority for every Host
//! DSP parameter: strip channel params (`Fader`/`Pan`/`Mute`/`Solo`),
//! send levels (`SendLevel`) and plugin params (`ProcessorParam` with
//! a [`TargetCapability`]) all live there as [`ParameterTarget`]s
//! addressed by [`ParameterTargetId`]. This module provides the
//! canonical *lookups* over that authority and the first half of the
//! two-stage binding the issue requires:
//!
//! - **Stage 1 (semantic, persisted):** [`semantic_plugin_binding`]
//!   resolves a plugin parameter target to its
//!   `(ProcessorId, PluginRef, ParamId)` — the stable processor
//!   identity, the authored plugin identity (descriptor/version/
//!   capability revision) and the plugin-local parameter id. A
//!   `ProcessorId` is never reused: a different processor is a
//!   different binding even when the plugin descriptor matches.
//! - **Stage 2 (runtime, `synvelis-player`):** the verified
//!   [`PluginParameterBinding`](crate) record pins the semantic
//!   binding to `(StablePluginInstanceId, NodeId, capability
//!   generation, graph generation, attach frame base)` — consumed by
//!   `RoutingControlDispatch`, never re-derived from raw ids here.
//!
//! Internal channel targets (strip/send, `capability: None`) have no
//! stage-1 plugin binding and no stage-2 runtime binding — their
//! delivery path is the lowering-time `ParameterTarget::base`
//! projection, a deliberately separate binding from plugin params.

use ruviform_plugin_core::ParamId;

use crate::{
    control::{ControlOwner, ParameterRole, STRIP_CHANNEL_ROLES},
    error::Error,
    id::{ParameterTargetId, ProcessorId, SendId, StripId},
    project::Project,
    routing::{PluginRef, ProcessorBinding},
};

/// The registered [`ParameterTargetId`] of `strip`'s singleton
/// channel `role` ([`STRIP_CHANNEL_ROLES`]), if present. Absent means
/// the strip predates registration or was crafted without it —
/// lowering treats absence as the canonical default, never as an
/// error.
#[must_use]
pub fn strip_channel_target_id(
    project: &Project,
    strip: StripId,
    role: ParameterRole,
) -> Option<ParameterTargetId> {
    debug_assert!(STRIP_CHANNEL_ROLES.contains(&role));
    project
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == role)
        .map(|t| t.id)
}

/// The send's persisted level authority: its `gain_target` resolved
/// back to the `Send`-owned `SendLevel` target it must reference
/// (issue #122 — `SendConfig` stores no second gain value). `None`
/// when the send is unconfigured (the explicit legacy/unconfigured
/// representation).
#[must_use]
pub fn send_level_target_id(project: &Project, send: SendId) -> Option<ParameterTargetId> {
    let target = project.routing.sends.get(&send)?.gain_target?;
    project
        .control
        .targets
        .get(&target)
        .filter(|t| t.owner == ControlOwner::Send(send) && t.role == ParameterRole::SendLevel)
        .map(|t| t.id)
}

/// The stage-1 plugin instance binding of a processor:
/// `ProcessorId` → authored [`PluginRef`].
///
/// This is the processor-level counterpart of
/// [`semantic_plugin_binding`]: where that resolves a *target* to
/// `(ProcessorId, PluginRef, ParamId)`, this resolves the processor
/// itself to the plugin identity its
/// [`ProcessorBinding::Resolved`] pins — the canonical lookup a
/// composition root uses to vouch for which plugin an instance was
/// authored as. Fail-closed like its sibling: an absent processor is
/// [`Error::NotFound`], an [`ProcessorBinding::Unresolved`] one is
/// [`Error::Validation`] — never a guess at a numerically matching
/// plugin.
///
/// # Errors
/// [`Error::NotFound`] when the processor is absent from
/// `project.routing.processors`; [`Error::Validation`] when its
/// binding is [`ProcessorBinding::Unresolved`].
pub fn plugin_instance_binding(
    project: &Project,
    processor: ProcessorId,
) -> Result<&PluginRef, Error> {
    let processor_ref = project
        .routing
        .processors
        .get(&processor)
        .ok_or_else(|| Error::NotFound(format!("processor {processor}")))?;
    match &processor_ref.binding {
        ProcessorBinding::Resolved(plugin) => Ok(plugin),
        ProcessorBinding::Unresolved { .. } => Err(Error::Validation(format!(
            "processor {processor} plugin binding is unresolved"
        ))),
    }
}

/// The stage-1 semantic binding of a plugin parameter target:
/// `ParameterTargetId` → `(ProcessorId, PluginRef, ParamId)`.
///
/// Resolution is fail-closed: the target must exist, be owned by a
/// [`ControlOwner::Processor`], that processor must carry a
/// [`ProcessorBinding::Resolved`] plugin identity, and the target
/// must declare a [`crate::TargetCapability`] (the plugin-local
/// `param_id` plus the write contract the runtime binding re-checks).
/// An [`ProcessorBinding::Unresolved`] processor, a non-processor
/// owner, or a capability-less target is a typed [`Error`], never a
/// guess — numeric `ParamId` coincidence across plugins or
/// generations binds nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticPluginBinding {
    /// The project control target this binding serves.
    pub target: ParameterTargetId,
    /// The owning processor — a stable identity, never reused.
    pub processor: ProcessorId,
    /// The authored plugin identity (descriptor, version, capability
    /// revision) pinned at bind time.
    pub plugin: PluginRef,
    /// The plugin-local parameter id (`ParamId` raw value).
    pub param_id: ParamId,
}

/// Resolves the stage-1 semantic binding of `target`.
///
/// # Errors
/// [`Error::NotFound`] when the target or its processor is absent;
/// [`Error::Validation`] when the target is not processor-owned, the
/// processor binding is unresolved, or the target carries no plugin
/// capability.
pub fn semantic_plugin_binding(
    project: &Project,
    target: ParameterTargetId,
) -> Result<SemanticPluginBinding, Error> {
    let param = project
        .control
        .targets
        .get(&target)
        .ok_or_else(|| Error::NotFound(format!("parameter target {target}")))?;
    let ControlOwner::Processor(processor) = param.owner else {
        return Err(Error::Validation(format!(
            "target {target} is not processor-owned ({:?})",
            param.owner
        )));
    };
    let processor_ref = project
        .routing
        .processors
        .get(&processor)
        .ok_or_else(|| Error::NotFound(format!("processor {processor}")))?;
    let ProcessorBinding::Resolved(plugin) = &processor_ref.binding else {
        return Err(Error::Validation(format!(
            "processor {processor} plugin binding is unresolved"
        )));
    };
    let capability = param.capability.ok_or_else(|| {
        Error::Validation(format!("target {target} carries no plugin capability"))
    })?;
    Ok(SemanticPluginBinding {
        target,
        processor,
        plugin: plugin.clone(),
        param_id: ParamId(capability.param_id),
    })
}
