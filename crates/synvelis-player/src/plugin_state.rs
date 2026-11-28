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
use std::thread;
use std::time::{Duration, Instant};

use ruviform_plugin_core::LifecycleState;
use ruviform_plugin_host::{
    FailureCause, HostCommand, HostError, HostNotification, HostReply, PluginHandle, PluginHost,
};

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
    failure_notifications: Vec<HostNotification>,
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
            failure_notifications: Vec::new(),
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

    /// Returns the cached failure notification for `handle`, if the
    /// adapter observed one while attributing a failed state command.
    #[must_use]
    pub fn failure_notification(&self, handle: PluginHandle) -> Option<HostNotification> {
        self.failure_notifications
            .iter()
            .copied()
            .find(|notification| match notification {
                HostNotification::InstanceFailed { handle: failed, .. } => *failed == handle,
                _ => false,
            })
    }

    /// Drains host failure notifications only while attributing a
    /// failed state command. Notifications for every handle are
    /// retained in the adapter cache rather than dropped, so the
    /// adapter — not each test or caller — owns this consumption.
    fn failure_notification_for(&mut self, handle: PluginHandle) -> Option<HostNotification> {
        if let Some(notification) = self.failure_notification(handle) {
            return Some(notification);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let notification = self.host.borrow_mut().try_recv_notification();
            if let Some(notification) = notification {
                self.failure_notifications.push(notification);
                if self
                    .failure_notification(handle)
                    .is_some_and(|cached| cached == notification)
                {
                    return Some(notification);
                }
            }
            if Instant::now() >= deadline {
                return self.failure_notification(handle);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn failure_cause(&mut self, handle: PluginHandle) -> Option<FailureCause> {
        if let Some(HostNotification::InstanceFailed { cause, .. }) =
            self.failure_notification_for(handle)
        {
            return Some(cause);
        }
        self.host
            .borrow()
            .diagnostics_for_handle(handle)
            .and_then(|record| record.cause)
    }

    /// `HostError` → typed state failure, using the host's typed
    /// failure attribution when the execution context is gone or
    /// unreachable. A missing attribution stays `Crashed`: the
    /// operation did not complete and must not be upgraded into a
    /// timeout or success.
    fn map_host_error(
        &mut self,
        handle: PluginHandle,
        error: HostError,
    ) -> PluginStateCaptureError {
        match error {
            HostError::Unsupported => {
                if self.host.borrow().lifecycle_state(handle) == Some(LifecycleState::Failed) {
                    let kind = match self.failure_cause(handle) {
                        Some(FailureCause::Timeout) => PluginStateFailureKind::Timeout,
                        _ => PluginStateFailureKind::Crashed,
                    };
                    PluginStateCaptureError::Failed(kind)
                } else {
                    PluginStateCaptureError::Unsupported
                }
            }
            HostError::InvalidHandle | HostError::UnknownPluginId => {
                PluginStateCaptureError::Failed(PluginStateFailureKind::Unavailable)
            }
            HostError::Failed | HostError::Ipc => {
                let kind = match self.failure_cause(handle) {
                    Some(FailureCause::Timeout) => PluginStateFailureKind::Timeout,
                    _ => PluginStateFailureKind::Crashed,
                };
                PluginStateCaptureError::Failed(kind)
            }
            _ => PluginStateCaptureError::Failed(PluginStateFailureKind::CaptureFailed),
        }
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
        _mode: CaptureMode,
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
        let achieved = if stable_boundary {
            StateCaptureQuality::Quiescent
        } else {
            StateCaptureQuality::BestEffort
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
            Err(error) => Err(self.map_host_error(instance.handle, error)),
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
        let result = {
            let mut host = self.host.borrow_mut();
            host.execute(HostCommand::RestoreState {
                handle: instance.handle,
                bytes: bytes.to_vec(),
            })
        };
        match result {
            Ok(HostReply::Ok) => Ok(()),
            Ok(_) => Err(PluginStateCaptureError::Failed(
                PluginStateFailureKind::CaptureFailed,
            )),
            Err(error) => Err(self.map_host_error(instance.handle, error)),
        }
    }
}
