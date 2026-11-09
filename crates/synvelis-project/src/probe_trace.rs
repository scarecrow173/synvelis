//! Session-facing probe→trace port contract (Issue #107, SF-04
//! slice 2).
//!
//! [`ProbeTracePort`] is the boundary between a session layer
//! (`synvelis-workspace`'s `ControlSession`) and a probe/trace
//! provider (`synvelis-player`'s `ProbeTraceBridge` over
//! `RoutingEngine` + per-node `DispatchTraceAdapter`s). It carries
//! only contract vocabulary — semantic probe keys, project types and
//! [`ControlTrace`] projections. Runtime identities (`ProbeId`,
//! ruviform `ProbeEvent`/`MetricRecord`/`StatusRecord`, graph-local
//! `NodeId`/`PortId`) never cross this boundary: the provider
//! translates them into [`ProbeObservationUpdate`]s inside its own
//! layer.
//!
//! Ownership rules the contract encodes:
//!
//! - `probe_drain` is the **single** drain path for probe events —
//!   the same single-owner principle as the dispatch report drain.
//!   A second caller must never drain the same underlying queue.
//! - `record_probe` writes only the `observed` stage slot of the
//!   target's [`ControlTrace`]; every other stage stays independent.
//!   It returns `false` when the target has no trace route — the
//!   caller observes the drop instead of it happening silently.
//! - Missing evidence stays missing: the port never fabricates a
//!   measurement for a point that produced none.

use std::collections::BTreeMap;
use std::fmt;

use crate::id::ParameterTargetId;
use crate::probe::{ProbeCaptureSegment, ProbeMode, ProbeSubscriptionKey, SemanticProbeBinding};
use crate::project::Project;
use crate::trace::{ControlTrace, ProbeObservation};

/// One contract-level probe observation update produced by
/// [`ProbeTracePort::probe_drain`] — the semantic key the update
/// belongs to plus the [`ProbeObservation`] to record. At most one
/// update per drained runtime event; events that cannot be honestly
/// translated (unknown probe, stale generation, unrepresentable
/// stamp) produce *no* update rather than a guessed one.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ProbeObservationUpdate {
    /// Subscription the observation belongs to.
    pub key: ProbeSubscriptionKey,
    /// Observation channel state to record.
    pub observation: ProbeObservation,
}

/// Failures of the probe control plane — defined, never a panic or a
/// silent no-op. Mirrors the provider's own error taxonomy; runtime
/// broker errors collapse into [`ProbePortError::Failed`] because
/// the contract layer cannot name ruviform error types.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProbePortError {
    /// The subscription key is not installed.
    UnknownKey,
    /// The binding has no runtime probe (unresolved/unavailable/
    /// unsupported resolution or failed install).
    NotBound,
    /// A capture segment is already open for this probe.
    AlreadyCapturing,
    /// The live project does not match the published plan's project.
    ProjectMismatch,
    /// No published plan/source map exists to resolve against.
    NoActivePlan,
    /// Provider-side failure the contract cannot represent (broker
    /// channel full, capacity, budget, transport).
    Failed(String),
}

impl fmt::Display for ProbePortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey => f.write_str("probe subscription not installed"),
            Self::NotBound => f.write_str("probe binding has no runtime probe"),
            Self::AlreadyCapturing => f.write_str("probe already has an open capture segment"),
            Self::ProjectMismatch => f.write_str("project does not match the published plan"),
            Self::NoActivePlan => f.write_str("no published plan to resolve against"),
            Self::Failed(reason) => write!(f, "probe provider error: {reason}"),
        }
    }
}

impl std::error::Error for ProbePortError {}

/// The session-facing probe observation + trace boundary. See the
/// module docs for the ownership and honesty rules.
///
/// All methods are non-RT orchestration calls — implementations may
/// allocate; none may be invoked from an RT callback.
pub trait ProbeTracePort {
    /// Requests observation of `key`'s semantic point in `mode`
    /// against the committed `project`. Idempotent per live key;
    /// resolution failure is recorded on the binding ledger, never
    /// faked.
    ///
    /// # Errors
    /// [`ProbePortError::NoActivePlan`]/`ProjectMismatch`/`Broker`-class
    /// failures mapped to [`ProbePortError::Failed`].
    fn probe_request(
        &mut self,
        project: &Project,
        key: ProbeSubscriptionKey,
        mode: ProbeMode,
    ) -> Result<(), ProbePortError>;

    /// Requests a mode change on an installed subscription.
    ///
    /// # Errors
    /// [`ProbePortError::UnknownKey`]/`NotBound`/`Failed`.
    fn probe_set_mode(
        &mut self,
        key: ProbeSubscriptionKey,
        mode: ProbeMode,
    ) -> Result<(), ProbePortError>;

    /// Starts a bounded capture — an explicit new capture session.
    ///
    /// # Errors
    /// [`ProbePortError::UnknownKey`]/`NotBound`/`AlreadyCapturing`/
    /// `Failed`.
    fn probe_capture(
        &mut self,
        key: ProbeSubscriptionKey,
        max_frames: u64,
    ) -> Result<(), ProbePortError>;

    /// Stops an in-flight capture; the segment closes on the RT stop
    /// confirmation.
    ///
    /// # Errors
    /// [`ProbePortError::UnknownKey`]/`NotBound`/`Failed`.
    fn probe_stop_capture(&mut self, key: ProbeSubscriptionKey) -> Result<(), ProbePortError>;

    /// Removes a subscription — the binding ledger keeps a `Stopped`
    /// record; RT `Removed` evidence remains the confirmation.
    ///
    /// # Errors
    /// [`ProbePortError::UnknownKey`]/`Failed`.
    fn probe_remove(&mut self, key: ProbeSubscriptionKey) -> Result<(), ProbePortError>;

    /// Drains pending probe events into contract-level updates —
    /// **the only drain path**. The provider applies its own
    /// bookkeeping (binding states, capture segments) before
    /// translating; events that cannot be honestly represented are
    /// dropped inside the provider, never guessed.
    #[must_use]
    fn probe_drain(&mut self) -> Vec<ProbeObservationUpdate>;

    /// The subscription binding ledger — requested/queued state stays
    /// distinct from RT-confirmed state per binding.
    #[must_use]
    fn probe_bindings(&self) -> &BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>;

    /// Closed capture segments in close order.
    #[must_use]
    fn probe_segments(&self) -> &[ProbeCaptureSegment];

    /// Records probe-channel evidence into `target`'s `observed`
    /// stage slot. Other stage slots are untouched. Returns `false`
    /// when `target` has no trace route — a visible drop, not a
    /// silent one.
    fn record_probe(&mut self, target: ParameterTargetId, observation: ProbeObservation) -> bool;

    /// The [`ControlTrace`] for `target`, if the provider tracks one.
    #[must_use]
    fn trace(&self, target: ParameterTargetId) -> Option<&ControlTrace>;

    /// All tracked traces — non-RT (allocates).
    #[must_use]
    fn traces(&self) -> Vec<(ParameterTargetId, ControlTrace)>;
}
