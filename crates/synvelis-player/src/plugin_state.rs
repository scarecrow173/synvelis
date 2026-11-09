//! TIM-04 (issue #128) — non-RT plugin-state adapter: the
//! `synvelis-player` implementation of
//! [`synvelis_project::PluginStatePort`] over a shared
//! [`PluginHost`].
//!
//! The adapter consumes Ruviform's generic state contract
//! (`HostCommand::SaveState`/`RestoreState`, ruviform#196) — it does
//! not reimplement host lifecycle or state machinery. Every method
//! is **non-RT**: capture/restore may allocate and call plugin
//! hooks, so they must never run inside an RT callback.
//!
//! Live instances are bound per published routing generation via
//! [`PluginStateEndpoint`]s vended by
//! [`crate::routing_engine::RoutingEngine::plugin_state_handles`]
//! and [`PluginStateHost::bind`]. A stale handle (its generation
//! retired and the lease destroyed the instance) fails closed as
//! `Unavailable`/`Crashed` through the host's own generation
//! counter — a state operation can never reach another
//! generation's plugin.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ruviform_plugin_core::LifecycleState;
use ruviform_plugin_host::{HostCommand, HostError, HostReply, PluginHandle, PluginHost};

use synvelis_project::{
    CaptureMode, CaptureObservation, PluginRef, PluginStateBinding, PluginStateCapture,
    PluginStateCaptureError, PluginStateFailureKind, PluginStatePort, ProcessorId,
    StateCaptureQuality,
};

/// One live plugin instance vended for plugin-state operations —
/// the non-RT half of a published generation's chain connection.
#[derive(Clone, Copy, Debug)]
pub struct PluginStateEndpoint {
    /// Semantic processor the instance serves.
    pub processor: ProcessorId,
    /// Host handle of the live instance (generation-checked by the
    /// host — stale handles are rejected, never redirected).
    pub handle: PluginHandle,
}

/// A bound live instance: host handle plus the identity recorded
/// at bind time (used for [`PluginStatePort::describe`]).
#[derive(Clone)]
struct BoundInstance {
    handle: PluginHandle,
    plugin: PluginRef,
    graph_generation: u64,
}

/// `PluginHost` → [`PluginStatePort`] adapter. `!Send` like the host
/// it wraps (`Rc<RefCell>`); single-owner non-RT object.
pub struct PluginStateHost {
    host: Rc<RefCell<PluginHost>>,
    /// Opaque identity of this host runtime — recorded into every
    /// [`PluginStateBinding`] so a binding from a different host
    /// instance is visibly distinct.
    host_instance: String,
    instances: BTreeMap<ProcessorId, BoundInstance>,
}

impl PluginStateHost {
    /// Wraps a shared `PluginHost` — typically the same `Rc` a
    /// [`crate::host::PluginHostProcessors`] was built from (via
    /// [`crate::host::PluginHostProcessors::host`]).
    #[must_use]
    pub fn new(host: Rc<RefCell<PluginHost>>) -> Self {
        let host_instance = format!("plugin-host-{:p}", Rc::as_ptr(&host));
        Self {
            host,
            host_instance,
            instances: BTreeMap::new(),
        }
    }

    /// Records the live instance serving `processor` for
    /// `graph_generation`. Called once per published generation's
    /// chain connection; re-binding the same processor replaces the
    /// stale entry (the old handle is generation-rejected by the
    /// host anyway).
    pub fn bind(
        &mut self,
        processor: ProcessorId,
        handle: PluginHandle,
        plugin: &PluginRef,
        graph_generation: u64,
    ) {
        self.instances.insert(
            processor,
            BoundInstance {
                handle,
                plugin: plugin.clone(),
                graph_generation,
            },
        );
    }

    /// Drops the binding for `processor` (e.g. its generation
    /// retired). The host-side instance is torn down separately by
    /// the connection lease — this only forgets the mapping.
    pub fn unbind(&mut self, processor: ProcessorId) {
        self.instances.remove(&processor);
    }
}

impl PluginStatePort for PluginStateHost {
    fn describe(&self, processor: ProcessorId) -> Option<PluginStateBinding> {
        let instance = self.instances.get(&processor)?;
        Some(PluginStateBinding {
            descriptor: instance.plugin.descriptor.clone(),
            version: instance.plugin.version.clone(),
            capability_revision: instance.plugin.capability_revision,
            host_runtime_instance: self.host_instance.clone(),
            graph_generation: instance.graph_generation,
        })
    }

    fn capture(
        &mut self,
        processor: ProcessorId,
        mode: CaptureMode,
    ) -> Result<PluginStateCapture, PluginStateCaptureError> {
        let Some(instance) = self.instances.get(&processor).cloned() else {
            return Err(PluginStateCaptureError::Failed(
                PluginStateFailureKind::Unavailable,
            ));
        };
        let mut host = self.host.borrow_mut();
        // Observe the lifecycle on both sides of the state call: a
        // processing instance yields at best `BestEffort` — the host
        // cannot attest a point-in-time agreement while the RT stream
        // is live.
        let before = host.lifecycle_state(instance.handle);
        let result = host.execute(HostCommand::SaveState {
            handle: instance.handle,
        });
        let after = host.lifecycle_state(instance.handle);
        drop(host);

        let processing = matches!(before, Some(LifecycleState::Processing))
            || matches!(after, Some(LifecycleState::Processing));
        let stable_boundary = before.is_some() && before == after && !processing;
        let achieved = if !stable_boundary {
            StateCaptureQuality::BestEffort
        } else if mode == CaptureMode::Coordinated {
            StateCaptureQuality::Coordinated
        } else {
            StateCaptureQuality::Quiescent
        };
        let observation = CaptureObservation {
            processing,
            stable_boundary,
        };

        match result {
            Ok(HostReply::State { bytes }) => Ok(PluginStateCapture {
                blob: bytes,
                achieved,
                observation,
                detail: None,
            }),
            Ok(_) => Err(PluginStateCaptureError::Failed(
                PluginStateFailureKind::CaptureFailed,
            )),
            Err(error) => Err(map_host_error(error)),
        }
    }

    fn restore(
        &mut self,
        processor: ProcessorId,
        bytes: &[u8],
    ) -> Result<(), PluginStateCaptureError> {
        let Some(instance) = self.instances.get(&processor).cloned() else {
            return Err(PluginStateCaptureError::Failed(
                PluginStateFailureKind::Unavailable,
            ));
        };
        let mut host = self.host.borrow_mut();
        match host.execute(HostCommand::RestoreState {
            handle: instance.handle,
            bytes: bytes.to_vec(),
        }) {
            Ok(HostReply::Ok) => Ok(()),
            Ok(_) => Err(PluginStateCaptureError::Failed(
                PluginStateFailureKind::CaptureFailed,
            )),
            Err(error) => Err(map_host_error(error)),
        }
    }
}

/// `HostError` → typed state failure. `Unsupported` stays distinct
/// (the plugin has no state hooks); `InvalidHandle`/`UnknownPluginId`
/// mean no live instance; `Failed`/`Ipc` mean the execution context
/// is gone or unreachable (crash/transport); `PluginFailed` is a
/// hook-level refusal. `HostError` is `#[non_exhaustive]` — unknown
/// future variants fail closed as `CaptureFailed`.
fn map_host_error(error: HostError) -> PluginStateCaptureError {
    match error {
        HostError::Unsupported => PluginStateCaptureError::Unsupported,
        HostError::InvalidHandle | HostError::UnknownPluginId => {
            PluginStateCaptureError::Failed(PluginStateFailureKind::Unavailable)
        }
        HostError::Failed | HostError::Ipc => {
            PluginStateCaptureError::Failed(PluginStateFailureKind::Crashed)
        }
        _ => PluginStateCaptureError::Failed(PluginStateFailureKind::CaptureFailed),
    }
}
