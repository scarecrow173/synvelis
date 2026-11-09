//! `ControlSession` probe→trace wiring (Issue #107, SF-04 slice 2).
//!
//! Connects the session-facing [`ProbeTracePort`] — implemented by
//! `synvelis-player`'s `ProbeTraceBridge` — to session state:
//!
//! - **Correlation**: [`ProbeTraceSession`] mints this session's
//!   [`ProjectSessionId`] and records which `ParameterTargetId`s each
//!   [`ProbeSubscriptionKey`] feeds. A drain update is recorded only
//!   for correlated targets — an event the session never subscribed
//!   is counted `uncorrelated`, never written.
//! - **Single drain owner**: [`ControlSession::probe_drain`] is the
//!   only session path that calls `port.probe_drain()` — the same
//!   single-owner principle as the dispatch report drain.
//! - **Honest states**: `probe_remove` records `Missing` into
//!   correlated targets synchronously (the provider clears the
//!   runtime probe id at remove time, so later RT `Removed` evidence
//!   can no longer be attributed); every other state flows through
//!   drain updates verbatim.
//! - **Stage separation**: `record_probe` writes only the `observed`
//!   slot of each target's [`ControlTrace`]; freshness is read back
//!   through [`ControlSession::probe_observed_freshness`].
//!
//! The port is passed **per call** (`&mut dyn ProbeTracePort`)
//! because the probe source (engine) and the trace adapters are
//! shared composition-root resources the session must not own —
//! the same pattern as `ProjectSession::control_with` passing the
//! project context per call.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_project::probe::{
    ProbeCaptureSegment, ProbeMode, ProbeSubscriptionKey, ProjectSessionId, SemanticProbeBinding,
    SemanticProbePoint, SubscriptionKind,
};
use synvelis_project::trace::{
    ControlTrace, FreshnessContext, ObservationFreshness, ProbeObservation, UnavailableReason,
};
use synvelis_project::{ParameterTargetId, ProbePortError, ProbeTracePort, Project};

use crate::control::ControlSession;

/// Session-scoped probe-trace state: the minted session id (the
/// subscription-key namespace) and the subscription→target
/// correlation. Owns no runtime or trace state — those live behind
/// the port.
#[derive(Debug)]
pub(crate) struct ProbeTraceSession {
    /// This session's id — minted once, namespaced so subscription
    /// keys never collide across sessions or engines.
    session_id: ProjectSessionId,
    /// Subscription key → the parameter targets this session feeds
    /// probe observations into. One key may fan out to several
    /// targets; one target may hold several keys.
    targets: BTreeMap<ProbeSubscriptionKey, BTreeSet<ParameterTargetId>>,
}

impl ProbeTraceSession {
    pub(crate) fn new() -> Self {
        static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
        Self {
            session_id: ProjectSessionId(NEXT_SESSION.fetch_add(1, Ordering::Relaxed)),
            targets: BTreeMap::new(),
        }
    }
}

/// Outcome counters of one [`ControlSession::probe_drain`] — every
/// port update and every `record_probe` call is accounted for, so a
/// dropped observation is never silent.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProbeDrainReport {
    /// Contract updates the port returned this drain.
    pub updates: usize,
    /// Updates with no session correlation — not recorded anywhere.
    pub uncorrelated: usize,
    /// `record_probe` calls the port accepted.
    pub recorded: usize,
    /// `record_probe` calls the port rejected (no trace route).
    pub unrouted: usize,
}

/// Failures of the session probe-trace surface.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeTraceError {
    /// `target` is not a parameter target of the live project —
    /// fail-closed before any port call, so no correlation is
    /// registered for a target that cannot exist.
    UnknownTarget(ParameterTargetId),
    /// The port rejected the operation.
    Port(ProbePortError),
}

impl fmt::Display for ProbeTraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget(target) => write!(f, "unknown parameter target {target}"),
            Self::Port(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProbeTraceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnknownTarget(_) => None,
            Self::Port(error) => Some(error),
        }
    }
}

impl ControlSession {
    /// Requests probe observation of `point` feeding `target` in
    /// `mode`. Mints the subscription key under this session's id and
    /// — only on port success — correlates `target` to it, so a
    /// failed request never claims observations it cannot receive.
    /// Idempotent: re-requesting the same `(point, kind)` for the
    /// same target is a set insert, not a duplicate subscription.
    ///
    /// # Errors
    /// [`ProbeTraceError::UnknownTarget`] when `target` is absent from
    /// the live project; [`ProbeTraceError::Port`] on port failure.
    pub fn probe_request(
        &mut self,
        project: &Project,
        port: &mut dyn ProbeTracePort,
        target: ParameterTargetId,
        point: SemanticProbePoint,
        kind: SubscriptionKind,
        mode: ProbeMode,
    ) -> Result<ProbeSubscriptionKey, ProbeTraceError> {
        if !project.control.targets.contains_key(&target) {
            return Err(ProbeTraceError::UnknownTarget(target));
        }
        let key = ProbeSubscriptionKey {
            session_id: self.probe_session.session_id,
            point,
            kind,
        };
        port.probe_request(project, key, mode)
            .map_err(ProbeTraceError::Port)?;
        self.probe_session
            .targets
            .entry(key)
            .or_default()
            .insert(target);
        Ok(key)
    }

    /// Requests a mode change on subscription `key`.
    ///
    /// # Errors
    /// [`ProbeTraceError::Port`].
    pub fn probe_set_mode(
        &mut self,
        port: &mut dyn ProbeTracePort,
        key: ProbeSubscriptionKey,
        mode: ProbeMode,
    ) -> Result<(), ProbeTraceError> {
        port.probe_set_mode(key, mode)
            .map_err(ProbeTraceError::Port)
    }

    /// Starts a bounded capture on subscription `key`.
    ///
    /// # Errors
    /// [`ProbeTraceError::Port`].
    pub fn probe_capture(
        &mut self,
        port: &mut dyn ProbeTracePort,
        key: ProbeSubscriptionKey,
        max_frames: u64,
    ) -> Result<(), ProbeTraceError> {
        port.probe_capture(key, max_frames)
            .map_err(ProbeTraceError::Port)
    }

    /// Stops an in-flight capture on subscription `key`.
    ///
    /// # Errors
    /// [`ProbeTraceError::Port`].
    pub fn probe_stop_capture(
        &mut self,
        port: &mut dyn ProbeTracePort,
        key: ProbeSubscriptionKey,
    ) -> Result<(), ProbeTraceError> {
        port.probe_stop_capture(key).map_err(ProbeTraceError::Port)
    }

    /// Removes subscription `key`. On success the correlated targets'
    /// `observed` slots are set to `Missing` immediately — the
    /// provider clears the runtime probe id at remove time, so RT
    /// `Removed` evidence arriving later can no longer be attributed
    /// to this key. The correlation itself is kept: a `Stopped`
    /// binding may be re-requested under the same key.
    ///
    /// # Errors
    /// [`ProbeTraceError::Port`].
    pub fn probe_remove(
        &mut self,
        port: &mut dyn ProbeTracePort,
        key: ProbeSubscriptionKey,
    ) -> Result<(), ProbeTraceError> {
        port.probe_remove(key).map_err(ProbeTraceError::Port)?;
        if let Some(targets) = self.probe_session.targets.get(&key) {
            for &target in targets {
                port.record_probe(target, ProbeObservation::Missing);
            }
        }
        Ok(())
    }

    /// **The single probe drain owner**: pulls contract updates from
    /// the port and records each into every correlated target's
    /// `observed` slot. Updates the session never correlated are
    /// counted `uncorrelated`; `record_probe` rejections (no trace
    /// route) are counted `unrouted` — nothing drops silently.
    #[must_use]
    pub fn probe_drain(&mut self, port: &mut dyn ProbeTracePort) -> ProbeDrainReport {
        let updates = port.probe_drain();
        let mut report = ProbeDrainReport {
            updates: updates.len(),
            ..ProbeDrainReport::default()
        };
        for update in updates {
            let Some(targets) = self.probe_session.targets.get(&update.key) else {
                report.uncorrelated += 1;
                continue;
            };
            for &target in targets {
                if port.record_probe(target, update.observation) {
                    report.recorded += 1;
                } else {
                    report.unrouted += 1;
                }
            }
        }
        report
    }

    /// The port's subscription binding ledger, verbatim.
    #[must_use]
    pub fn probe_bindings<'a>(
        &self,
        port: &'a dyn ProbeTracePort,
    ) -> &'a BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding> {
        port.probe_bindings()
    }

    /// The port's closed capture segments, verbatim.
    #[must_use]
    pub fn probe_segments<'a>(&self, port: &'a dyn ProbeTracePort) -> &'a [ProbeCaptureSegment] {
        port.probe_segments()
    }

    /// The [`ControlTrace`] the port tracks for `target`, if any.
    #[must_use]
    pub fn probe_trace<'a>(
        &self,
        port: &'a dyn ProbeTracePort,
        target: ParameterTargetId,
    ) -> Option<&'a ControlTrace> {
        port.trace(target)
    }

    /// Every trace the port tracks — non-RT (allocates).
    #[must_use]
    pub fn probe_traces(
        &self,
        port: &dyn ProbeTracePort,
    ) -> Vec<(ParameterTargetId, ControlTrace)> {
        port.traces()
    }

    /// Freshness of `target`'s probe-observed channel. A target with
    /// no trace at all reports `Unavailable{NoEvidence}` — missing
    /// evidence is never upgraded.
    #[must_use]
    pub fn probe_observed_freshness(
        &self,
        port: &dyn ProbeTracePort,
        target: ParameterTargetId,
        ctx: &FreshnessContext,
    ) -> ObservationFreshness {
        port.trace(target).map_or(
            ObservationFreshness::Unavailable {
                reason: UnavailableReason::NoEvidence,
            },
            |trace| trace.observed_freshness(ctx),
        )
    }

    /// The subscription keys this session correlated to `target`.
    #[must_use]
    pub fn probe_subscriptions(&self, target: ParameterTargetId) -> Vec<ProbeSubscriptionKey> {
        self.probe_session
            .targets
            .iter()
            .filter(|(_, targets)| targets.contains(&target))
            .map(|(key, _)| *key)
            .collect()
    }

    /// The targets `key` feeds — empty when the session holds no
    /// correlation for it.
    #[must_use]
    pub fn probe_targets(&self, key: ProbeSubscriptionKey) -> Vec<ParameterTargetId> {
        self.probe_session
            .targets
            .get(&key)
            .map(|targets| targets.iter().copied().collect())
            .unwrap_or_default()
    }
}
