//! Capability registry — registration and discovery of versioned
//! [`CapabilityDescriptor`]s.
//!
//! Discovery returns metadata only. Being listed here grants no
//! permission (GT-003); the invoke path re-checks authorization on every
//! request through the gateway's policy evaluator.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use crate::capability::{CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityVersion};
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::request::OperationRequest;

/// The executor behind a capability.
///
/// Implementations receive the *validated* [`OperationRequest`];
/// fixture/mock handlers satisfy this trait so the contract is testable
/// without any production operation (Issue #53 scope).
#[allow(clippy::result_large_err)] // typed envelope is the contract
pub trait CapabilityOperation: Send + Sync {
    /// Executes the operation.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — the typed failure the gateway returns
    /// unchanged.
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope>;
}

#[allow(clippy::result_large_err)] // typed envelope is the contract
impl<F> CapabilityOperation for F
where
    F: Fn(&OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> + Send + Sync,
{
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        self(req)
    }
}

/// One registered capability version: descriptor plus optional executor.
///
/// `handler` is crate-internal: the executor is only reachable through
/// [`crate::Gateway::invoke`], which runs policy evaluation first.
/// Exposing it would let any holder of `&CapabilityRegistry` execute a
/// capability with a forged request — discovery grants metadata only.
pub struct Registration {
    /// The registered contract.
    pub descriptor: CapabilityDescriptor,
    /// The executor, when installed.
    pub(crate) handler: Option<Arc<dyn CapabilityOperation>>,
}

/// Versioned capability registry.
///
/// Registration is append-only per `(id, version)`: a registered
/// contract never mutates in place, so a previously advertised
/// capability cannot silently change meaning.
#[derive(Default)]
pub struct CapabilityRegistry {
    registrations: BTreeMap<(CapabilityId, CapabilityVersion), Registration>,
}

impl CapabilityRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `descriptor` (with an optional executor).
    ///
    /// # Errors
    /// [`GatewayError::CapabilityConflict`] — the same id+version is
    /// already registered.
    pub fn register(
        &mut self,
        descriptor: CapabilityDescriptor,
        handler: Option<Arc<dyn CapabilityOperation>>,
    ) -> Result<(), GatewayError> {
        let key = (descriptor.id.clone(), descriptor.version);
        if self.registrations.contains_key(&key) {
            return Err(GatewayError::CapabilityConflict {
                id: descriptor.id.to_string(),
                version: descriptor.version,
            });
        }
        self.registrations.insert(
            key,
            Registration {
                descriptor,
                handler,
            },
        );
        Ok(())
    }

    /// Resolves an exact capability ref to its registration.
    ///
    /// Crate-internal: this is the only path that reaches `handler`, and
    /// it must stay behind `bind`/`invoke` (see [`Registration`]).
    ///
    /// # Errors
    /// - [`GatewayError::UnknownCapability`] — id not registered.
    /// - [`GatewayError::UnsupportedCapabilityVersion`] — version not
    ///   registered for that id.
    pub(crate) fn resolve(
        &self,
        capability: &CapabilityRef,
    ) -> Result<&Registration, GatewayError> {
        if !self
            .registrations
            .keys()
            .any(|(id, _)| *id == capability.id)
        {
            return Err(GatewayError::UnknownCapability {
                id: capability.id.to_string(),
            });
        }
        self.registrations
            .get(&(capability.id.clone(), capability.version))
            .ok_or_else(|| GatewayError::UnsupportedCapabilityVersion {
                id: capability.id.to_string(),
                requested: capability.version,
                supported: self.versions(&capability.id),
            })
    }

    /// The descriptor for an exact capability ref (discovery metadata —
    /// grants nothing).
    ///
    /// # Errors
    /// Same as [`resolve`][Self::resolve].
    pub fn describe(
        &self,
        capability: &CapabilityRef,
    ) -> Result<&CapabilityDescriptor, GatewayError> {
        Ok(&self.resolve(capability)?.descriptor)
    }

    /// Whether an executor is registered for the exact capability ref
    /// (discovery metadata — grants nothing).
    ///
    /// `false` covers every "no executor reachable" case alike:
    /// unknown id, unregistered version, and a registration without a
    /// handler. [`crate::Gateway::invoke`] remains the sole authority
    /// that reports [`crate::GatewayError::NoExecutor`] at execution
    /// time; this predicate only lets read-only projections (e.g. the
    /// AUTX-01 action catalog) distinguish a No-Handler registration
    /// from an unobservable one.
    #[must_use]
    pub fn has_executor(&self, capability: &CapabilityRef) -> bool {
        self.resolve(capability)
            .is_ok_and(|registration| registration.handler.is_some())
    }

    /// Every registered capability ref (discovery metadata — grants
    /// nothing).
    #[must_use]
    pub fn list(&self) -> Vec<CapabilityRef> {
        self.registrations
            .values()
            .map(|r| r.descriptor.capability_ref())
            .collect()
    }

    /// Versions registered for `id` (empty when unknown).
    #[must_use]
    pub fn versions(&self, id: &CapabilityId) -> Vec<CapabilityVersion> {
        self.registrations
            .keys()
            .filter(|(k, _)| k == id)
            .map(|(_, v)| *v)
            .collect()
    }
}
