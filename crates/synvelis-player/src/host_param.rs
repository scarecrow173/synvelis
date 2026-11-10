//! Host Parameter generation-correlation evidence (issue #122,
//! CTL-02).
//!
//! [`binding_evidence`] correlates one persisted
//! [`ParameterTargetId`] across the binding stages the issue requires
//! and classifies its delivery state:
//!
//! - **Stage 1 (semantic):** [`synvelis_project::semantic_plugin_binding`]
//!   resolves `ParameterTargetId` → `(ProcessorId, PluginRef,
//!   ParamId)` — the stable processor identity and authored plugin
//!   identity. `None` for internal channel targets and unresolvable
//!   bindings.
//! - **Stage 2 (runtime):** the verified
//!   [`PluginParameterBinding`] on the attached
//!   [`RoutingControlDispatch`] surface — instance/node/capability/
//!   graph-generation/frame-base pins included.
//! - **Generation axes:** the surface generation, the published plan
//!   generation (`RoutingSourceMap::generation`, caller-supplied) and
//!   the RT-observed `active_generation` must all agree before a
//!   plugin param is `Observed`; disagreement is `Stale` (reject,
//!   never retarget) and a not-yet-active plan is `Pending` (gated,
//!   never delivered early).
//!
//! Internal channel targets (strip fader/pan/mute/solo, send level)
//! always report [`HostParamDelivery::Unbound`] here — their delivery
//! path is the lowering-time `ParameterTarget::base` projection, a
//! deliberately separate binding that never touches plugin `ParamId`s.
//! This is the "host internal vs plugin param are different bindings"
//! contract made observable.

use synvelis_project::host_param::{SemanticPluginBinding, semantic_plugin_binding};
use synvelis_project::{ControlOwner, Error, ParameterRole, ParameterTargetId, Project};

use crate::binding::PluginParameterBinding;
use crate::routing_control::RoutingControlDispatch;

/// Generation-correlated evidence for one host parameter target.
#[derive(Clone, Debug)]
pub struct HostParamEvidence {
    /// The persisted control target.
    pub target: ParameterTargetId,
    /// Persisted owner (`Strip`/`Send`/`Processor`/…).
    pub owner: ControlOwner,
    /// Persisted semantic role.
    pub role: ParameterRole,
    /// Stage-1 semantic binding — `Some` only for processor plugin
    /// params that resolve through [`semantic_plugin_binding`].
    pub semantic: Option<SemanticPluginBinding>,
    /// Stage-2 verified runtime binding on the attached surface —
    /// `Some` only for bound plugin params.
    pub runtime: Option<PluginParameterBinding>,
    /// The attached control surface's pinned routing plan generation.
    pub surface_generation: Option<u64>,
    /// The published plan generation the engine's source map names —
    /// caller-supplied from `RoutingEngine::source_map()`.
    pub source_map_generation: Option<u64>,
    /// The RT-observed active generation — caller-supplied from
    /// `RoutingEngine::rt_status().active_generation` (`0` → `None`).
    pub active_generation: Option<u64>,
}

/// The delivery classification of a [`HostParamEvidence`] — `Pending`,
/// gated and observed states stay distinct; a stale record is never
/// retargeted into a delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostParamDelivery {
    /// No stage-2 runtime binding exists — internal channel targets
    /// (strip/send) land here by design (their authority is the
    /// lowering-time base projection), as do unbound or defunct
    /// plugin bindings.
    Unbound,
    /// The binding's pinned graph generation disagrees with the
    /// current published/surface generation — a stale or foreign
    /// record that must be rejected, never delivered.
    Stale {
        /// The generation the binding record pins.
        pinned: u64,
        /// The current published plan generation.
        current: u64,
    },
    /// Every generation pin agrees but the RT-observed active
    /// generation has not reached the surface generation — delivery
    /// is gated (queued), not lost and not delivered early.
    Pending {
        /// The generation awaiting RT observation.
        generation: u64,
        /// The last RT-observed active generation, if any.
        active: Option<u64>,
    },
    /// Every generation pin agrees and the plan is RT-active —
    /// deliverable now.
    Observed {
        /// The agreed generation.
        generation: u64,
    },
}

impl HostParamEvidence {
    /// Classifies the target's delivery state across the generation
    /// axes. The published plan generation (source map) is the
    /// authority; the control surface must agree with it — a surface
    /// synced to a different generation is stale evidence, not a
    /// delivery path.
    #[must_use]
    pub fn delivery(&self) -> HostParamDelivery {
        let Some(binding) = &self.runtime else {
            return HostParamDelivery::Unbound;
        };
        let Some(current) = self.source_map_generation.or(self.surface_generation) else {
            return HostParamDelivery::Unbound;
        };
        if binding.graph_generation != current
            || self.surface_generation.is_some_and(|s| s != current)
        {
            return HostParamDelivery::Stale {
                pinned: binding.graph_generation,
                current,
            };
        }
        if self.active_generation != Some(current) {
            return HostParamDelivery::Pending {
                generation: current,
                active: self.active_generation,
            };
        }
        HostParamDelivery::Observed {
            generation: current,
        }
    }
}

/// Gathers generation-correlated evidence for `target`.
///
/// `source_map_generation` is `engine.source_map().map(|m| m.generation)`
/// and `active_generation` the RT-observed value
/// (`engine.rt_status().active_generation`, mapping `0` to `None`) —
/// both caller-supplied so the evidence names *which* generation each
/// pin carries. `None` map means nothing published yet.
///
/// # Errors
/// [`Error::NotFound`] when `target` is not a persisted target.
pub fn binding_evidence(
    project: &Project,
    control: &RoutingControlDispatch,
    source_map_generation: Option<u64>,
    active_generation: Option<u64>,
    target: ParameterTargetId,
) -> Result<HostParamEvidence, Error> {
    let param = project
        .control
        .targets
        .get(&target)
        .ok_or_else(|| Error::NotFound(format!("parameter target {target}")))?;
    Ok(HostParamEvidence {
        target,
        owner: param.owner,
        role: param.role,
        semantic: semantic_plugin_binding(project, target).ok(),
        runtime: control.binding(target),
        surface_generation: control.generation(),
        source_map_generation,
        active_generation,
    })
}
