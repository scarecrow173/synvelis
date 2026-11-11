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

use ruviform_ui::dock::ViewInstanceId;
use serde::{Deserialize, Serialize};
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
    /// Subscription key → the principals holding a live lease on it.
    /// A key with any holder is treated as live: `probe_subscribe`
    /// joins it without another port request, and only the last
    /// lease release reaches `port.probe_remove`. Keys are removed
    /// outright — an empty holder set never stays in the map.
    leases: BTreeMap<ProbeSubscriptionKey, BTreeSet<ProbePrincipal>>,
    /// View → the keys it leases — the reverse index of `leases`'
    /// `View` members, kept in sync on every lease change so
    /// [`ControlSession::probe_release_subscriber`] (window close)
    /// can enumerate a view's holds without scanning all keys.
    by_view: BTreeMap<ViewInstanceId, BTreeSet<ProbeSubscriptionKey>>,
    /// Subscription key → the principal that explicitly started the
    /// in-flight capture. Attribution only: a capture that ends
    /// implicitly port-side (budget, generation cut) leaves the
    /// entry until an explicit stop/remove/release clears it — the
    /// binding ledger, not this map, is the job-state truth.
    capture_owners: BTreeMap<ProbeSubscriptionKey, ProbePrincipal>,
}

impl ProbeTraceSession {
    pub(crate) fn new() -> Self {
        static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
        Self {
            session_id: ProjectSessionId(NEXT_SESSION.fetch_add(1, Ordering::Relaxed)),
            targets: BTreeMap::new(),
            leases: BTreeMap::new(),
            by_view: BTreeMap::new(),
            capture_owners: BTreeMap::new(),
        }
    }

    /// Whether `key` has at least one live lease — the session's
    /// "the port subscription is live" view. `leases ⊆ targets`:
    /// every leased key is correlated, while a removed key keeps its
    /// correlation (revival) with no lease.
    fn holds(&self, key: &ProbeSubscriptionKey) -> bool {
        self.leases
            .get(key)
            .is_some_and(|holders| !holders.is_empty())
    }

    /// Records `principal` as a holder of `key`.
    fn add_lease(&mut self, key: ProbeSubscriptionKey, principal: ProbePrincipal) {
        self.leases.entry(key).or_default().insert(principal);
        if let ProbePrincipal::View(view) = principal {
            self.by_view.entry(view).or_default().insert(key);
        }
    }

    /// Drops `View(view)`'s lease on `key`. The caller guarantees at
    /// least one other holder remains — a sole-holder release goes
    /// through `remove_key` instead. A capture owned by the released
    /// view retreats to `Session`: the job outlives its owner's
    /// window — a close must not stop other subscribers' work or
    /// in-flight jobs.
    fn release_lease(&mut self, key: ProbeSubscriptionKey, view: ViewInstanceId) {
        let empty = if let Some(holders) = self.leases.get_mut(&key) {
            holders.remove(&ProbePrincipal::View(view));
            holders.is_empty()
        } else {
            false
        };
        if empty {
            self.leases.remove(&key);
        }
        Self::unindex_view(&mut self.by_view, view, key);
        if self.capture_owners.get(&key) == Some(&ProbePrincipal::View(view)) {
            self.capture_owners.insert(key, ProbePrincipal::Session);
        }
    }

    /// Removes `key` from `view`'s index, dropping the entry when
    /// its set empties — `by_view` never keeps empty sets.
    fn unindex_view(
        by_view: &mut BTreeMap<ViewInstanceId, BTreeSet<ProbeSubscriptionKey>>,
        view: ViewInstanceId,
        key: ProbeSubscriptionKey,
    ) {
        let empty = if let Some(keys) = by_view.get_mut(&view) {
            keys.remove(&key);
            keys.is_empty()
        } else {
            false
        };
        if empty {
            by_view.remove(&view);
        }
    }

    /// Revokes every lease on `key` and its capture owner — the
    /// session-authorized removal path (`probe_remove`, last lease
    /// release, `probe_release_all`). Call only after the port
    /// confirmed the remove; `targets` correlation stays for revival.
    fn revoke_leases(&mut self, key: ProbeSubscriptionKey) {
        if let Some(holders) = self.leases.remove(&key) {
            for holder in holders {
                if let ProbePrincipal::View(view) = holder {
                    Self::unindex_view(&mut self.by_view, view, key);
                }
            }
        }
        self.capture_owners.remove(&key);
    }
}

/// The principal holding a probe subscription lease or owning an
/// in-flight capture — session authority or a logical view
/// subscriber. `View` wraps ruviform-ui's [`ViewInstanceId`], which
/// is minted by the consumer and survives window move,
/// detach/reattach, and layout restore, so a lease follows the
/// logical inspector rather than an OS window.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProbePrincipal {
    /// Session authority — the hold recorded by `probe_request`,
    /// the owner recorded by session-level `probe_capture`, and
    /// the retreat target when a capture owner's view lease is
    /// released while the key stays held.
    Session,
    /// A logical view (window/inspector) subscriber.
    View(ViewInstanceId),
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

/// Accounting of one subscriber-release pass
/// ([`ControlSession::probe_unsubscribe`],
/// [`ControlSession::probe_release_subscriber`],
/// [`ControlSession::probe_release_all`]) — which keys lost a lease
/// and which subscriptions the port actually removed. Nothing drops
/// silently: `removed ⊆ released` always holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProbeReleaseReport {
    /// Keys that lost at least one lease in this pass.
    pub released: Vec<ProbeSubscriptionKey>,
    /// Keys whose last lease left, so the port subscription was
    /// removed (`probe_remove` semantics: `Missing` recorded into
    /// correlated targets).
    pub removed: Vec<ProbeSubscriptionKey>,
}

/// Failures of the session probe-trace surface.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeTraceError {
    /// `target` is not a parameter target of the live project —
    /// fail-closed before any port call, so no correlation is
    /// registered for a target that cannot exist.
    UnknownTarget(ParameterTargetId),
    /// `subscriber` holds no lease on `key` — unsubscribe,
    /// subscriber capture, and owner abort are fail-closed on a
    /// lease the session never granted.
    NotSubscriber {
        /// The subscription key the operation targeted.
        key: ProbeSubscriptionKey,
        /// The view that holds no lease on it.
        subscriber: ViewInstanceId,
    },
    /// `subscriber` holds a lease on `key` but is not the recorded
    /// owner of its in-flight capture — abort is owner-only.
    NotCaptureOwner {
        /// The subscription key the operation targeted.
        key: ProbeSubscriptionKey,
        /// The view that is not the capture owner.
        subscriber: ViewInstanceId,
    },
    /// The port rejected the operation.
    Port(ProbePortError),
}

impl fmt::Display for ProbeTraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget(target) => write!(f, "unknown parameter target {target}"),
            Self::NotSubscriber { key, subscriber } => {
                write!(f, "view {subscriber} holds no lease on probe key {key:?}")
            }
            Self::NotCaptureOwner { key, subscriber } => {
                write!(
                    f,
                    "view {subscriber} does not own the capture on probe key {key:?}"
                )
            }
            Self::Port(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProbeTraceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnknownTarget(_) | Self::NotSubscriber { .. } | Self::NotCaptureOwner { .. } => {
                None
            }
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
        // The session's own hold: view subscribers joining and
        // leaving this key later must not revoke a subscription the
        // session itself asked for.
        self.probe_session.add_lease(key, ProbePrincipal::Session);
        Ok(key)
    }

    /// Subscribes logical view `subscriber` (a window/inspector —
    /// identified by its stable [`ViewInstanceId`]) to probe
    /// observation of `point` feeding `target` in `mode`.
    ///
    /// Dedup is session-side: when `key` already has a live lease
    /// the port is **not** called again — the shared binding's mode
    /// is unchanged (mode changes go through [`Self::probe_set_mode`];
    /// there is no per-subscriber mode) — and only the lease and
    /// target correlation are added. When no live hold exists
    /// (first subscriber, or revival of a `Stopped` binding) the
    /// port request goes through with `mode`.
    ///
    /// # Errors
    /// [`ProbeTraceError::UnknownTarget`] when `target` is absent
    /// from the live project; [`ProbeTraceError::Port`] on port
    /// failure — a failed request mutates no lease or correlation.
    #[allow(clippy::too_many_arguments)] // flat request pack mirroring `probe_request` + subscriber
    pub fn probe_subscribe(
        &mut self,
        project: &Project,
        port: &mut dyn ProbeTracePort,
        subscriber: ViewInstanceId,
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
        if !self.probe_session.holds(&key) {
            port.probe_request(project, key, mode)
                .map_err(ProbeTraceError::Port)?;
        }
        self.probe_session
            .targets
            .entry(key)
            .or_default()
            .insert(target);
        self.probe_session
            .add_lease(key, ProbePrincipal::View(subscriber));
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

    /// Starts a bounded capture on subscription `key`. Session
    /// authority: the capture owner is recorded as
    /// [`ProbePrincipal::Session`], so a later view-level
    /// [`Self::probe_abort_capture`] correctly refuses it — session
    /// aborts go through [`Self::probe_stop_capture`].
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
            .map_err(ProbeTraceError::Port)?;
        self.probe_session
            .capture_owners
            .insert(key, ProbePrincipal::Session);
        Ok(())
    }

    /// Starts a bounded capture on `key` on behalf of view
    /// `subscriber` — the subscriber-authorized counterpart of
    /// [`Self::probe_capture`]. Requires `subscriber` to hold a
    /// lease on `key`; the owner is recorded as
    /// [`ProbePrincipal::View`] so only that view may later abort
    /// via [`Self::probe_abort_capture`]. An in-flight capture is
    /// rejected by the port (`AlreadyCapturing`), so the owner is
    /// never silently overwritten.
    ///
    /// # Errors
    /// [`ProbeTraceError::NotSubscriber`] when `subscriber` holds no
    /// lease on `key`; [`ProbeTraceError::Port`] on port failure —
    /// a failed capture records no owner.
    pub fn probe_capture_for(
        &mut self,
        port: &mut dyn ProbeTracePort,
        subscriber: ViewInstanceId,
        key: ProbeSubscriptionKey,
        max_frames: u64,
    ) -> Result<(), ProbeTraceError> {
        let view = ProbePrincipal::View(subscriber);
        if !self
            .probe_session
            .leases
            .get(&key)
            .is_some_and(|holders| holders.contains(&view))
        {
            return Err(ProbeTraceError::NotSubscriber { key, subscriber });
        }
        port.probe_capture(key, max_frames)
            .map_err(ProbeTraceError::Port)?;
        self.probe_session.capture_owners.insert(key, view);
        Ok(())
    }

    /// Stops an in-flight capture on subscription `key` —
    /// unconditional session authority regardless of the recorded
    /// owner.
    ///
    /// # Errors
    /// [`ProbeTraceError::Port`].
    pub fn probe_stop_capture(
        &mut self,
        port: &mut dyn ProbeTracePort,
        key: ProbeSubscriptionKey,
    ) -> Result<(), ProbeTraceError> {
        port.probe_stop_capture(key)
            .map_err(ProbeTraceError::Port)?;
        self.probe_session.capture_owners.remove(&key);
        Ok(())
    }

    /// Aborts the in-flight capture on `key` — **owner only**. The
    /// recorded [`ProbePrincipal`] must be
    /// `View(subscriber)`; a non-owner subscriber (including one
    /// whose capture was started under session authority) is
    /// rejected with [`ProbeTraceError::NotCaptureOwner`], and a
    /// view with no lease at all with
    /// [`ProbeTraceError::NotSubscriber`]. Session-authorized abort
    /// remains [`Self::probe_stop_capture`].
    ///
    /// # Errors
    /// [`ProbeTraceError::NotSubscriber`],
    /// [`ProbeTraceError::NotCaptureOwner`], [`ProbeTraceError::Port`].
    pub fn probe_abort_capture(
        &mut self,
        port: &mut dyn ProbeTracePort,
        subscriber: ViewInstanceId,
        key: ProbeSubscriptionKey,
    ) -> Result<(), ProbeTraceError> {
        let view = ProbePrincipal::View(subscriber);
        if !self
            .probe_session
            .leases
            .get(&key)
            .is_some_and(|holders| holders.contains(&view))
        {
            return Err(ProbeTraceError::NotSubscriber { key, subscriber });
        }
        if self.probe_session.capture_owners.get(&key) != Some(&view) {
            return Err(ProbeTraceError::NotCaptureOwner { key, subscriber });
        }
        port.probe_stop_capture(key)
            .map_err(ProbeTraceError::Port)?;
        self.probe_session.capture_owners.remove(&key);
        Ok(())
    }

    /// The recorded owner of `key`'s in-flight capture, if any.
    /// Attribution, not liveness: a capture that ended implicitly
    /// port-side keeps its entry until an explicit
    /// stop/remove/release clears it — consult
    /// [`Self::probe_bindings`] for the binding ledger's truth.
    #[must_use]
    pub fn probe_capture_owner(&self, key: ProbeSubscriptionKey) -> Option<ProbePrincipal> {
        self.probe_session.capture_owners.get(&key).copied()
    }

    /// Removes subscription `key` — unconditional session authority:
    /// every lease on the key is revoked regardless of holder. On
    /// success the correlated targets' `observed` slots are set to
    /// `Missing` immediately — the provider clears the runtime probe
    /// id at remove time, so RT `Removed` evidence arriving later
    /// can no longer be attributed to this key. The correlation
    /// itself is kept: a `Stopped` binding may be re-requested under
    /// the same key.
    ///
    /// # Errors
    /// [`ProbeTraceError::Port`] — on failure no lease, owner, or
    /// correlation is touched.
    pub fn probe_remove(
        &mut self,
        port: &mut dyn ProbeTracePort,
        key: ProbeSubscriptionKey,
    ) -> Result<(), ProbeTraceError> {
        self.remove_key(port, key)
    }

    /// Releases view `subscriber`'s lease on `key`. When other
    /// holders remain the port subscription — and any in-flight
    /// capture — continues untouched; when this was the last lease
    /// the key is removed with [`Self::probe_remove`] semantics.
    ///
    /// # Errors
    /// [`ProbeTraceError::NotSubscriber`] when `subscriber` holds no
    /// lease on `key`; [`ProbeTraceError::Port`] on port failure —
    /// the lease is retained and the release may be retried.
    pub fn probe_unsubscribe(
        &mut self,
        port: &mut dyn ProbeTracePort,
        subscriber: ViewInstanceId,
        key: ProbeSubscriptionKey,
    ) -> Result<ProbeReleaseReport, ProbeTraceError> {
        let mut report = ProbeReleaseReport::default();
        self.release_subscriber_key(port, subscriber, key, &mut report)?;
        Ok(report)
    }

    /// Releases every lease view `subscriber` holds — the window
    /// close path. Each key follows [`Self::probe_unsubscribe`]
    /// semantics; a port failure aborts the pass but leaves already
    /// processed keys consistent, so the close may be retried.
    /// A view holding no leases yields an empty report, not an
    /// error — closing an unsubscribed window is not a failure.
    ///
    /// # Errors
    /// [`ProbeTraceError::Port`] on the first failing key.
    pub fn probe_release_subscriber(
        &mut self,
        port: &mut dyn ProbeTracePort,
        subscriber: ViewInstanceId,
    ) -> Result<ProbeReleaseReport, ProbeTraceError> {
        let keys: Vec<ProbeSubscriptionKey> = self
            .probe_session
            .by_view
            .get(&subscriber)
            .map(|keys| keys.iter().copied().collect())
            .unwrap_or_default();
        let mut report = ProbeReleaseReport::default();
        for key in keys {
            self.release_subscriber_key(port, subscriber, key, &mut report)?;
        }
        Ok(report)
    }

    /// Releases **all** leases — session stop. Every held key is
    /// removed with [`Self::probe_remove`] semantics regardless of
    /// which principal holds it.
    ///
    /// # Errors
    /// [`ProbeTraceError::Port`] on the first failing key; keys
    /// already processed stay removed.
    pub fn probe_release_all(
        &mut self,
        port: &mut dyn ProbeTracePort,
    ) -> Result<ProbeReleaseReport, ProbeTraceError> {
        let keys: Vec<ProbeSubscriptionKey> = self.probe_session.leases.keys().copied().collect();
        let mut report = ProbeReleaseReport::default();
        for key in keys {
            self.remove_key(port, key)?;
            report.released.push(key);
            report.removed.push(key);
        }
        Ok(report)
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

    /// The principals currently holding a lease on `key` — empty
    /// when the session holds none (never requested, or removed).
    #[must_use]
    pub fn probe_lease_holders(&self, key: ProbeSubscriptionKey) -> Vec<ProbePrincipal> {
        self.probe_session
            .leases
            .get(&key)
            .map(|holders| holders.iter().copied().collect())
            .unwrap_or_default()
    }

    /// The keys view `subscriber` currently leases — the read-side
    /// counterpart of [`Self::probe_release_subscriber`].
    #[must_use]
    pub fn probe_subscriber_keys(&self, subscriber: ViewInstanceId) -> Vec<ProbeSubscriptionKey> {
        self.probe_session
            .by_view
            .get(&subscriber)
            .map(|keys| keys.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Shared per-key release for [`Self::probe_unsubscribe`] and
    /// [`Self::probe_release_subscriber`]. Sole-holder releases go
    /// through [`Self::remove_key`] so the port remove is confirmed
    /// before any local state moves; shared releases are local-only
    /// and cannot fail.
    fn release_subscriber_key(
        &mut self,
        port: &mut dyn ProbeTracePort,
        subscriber: ViewInstanceId,
        key: ProbeSubscriptionKey,
        report: &mut ProbeReleaseReport,
    ) -> Result<(), ProbeTraceError> {
        let view = ProbePrincipal::View(subscriber);
        let leased = self
            .probe_session
            .leases
            .get(&key)
            .is_some_and(|holders| holders.contains(&view));
        if !leased {
            return Err(ProbeTraceError::NotSubscriber { key, subscriber });
        }
        let sole = self
            .probe_session
            .leases
            .get(&key)
            .is_some_and(|holders| holders.len() == 1);
        if sole {
            self.remove_key(port, key)?;
            report.removed.push(key);
        } else {
            self.probe_session.release_lease(key, subscriber);
        }
        report.released.push(key);
        Ok(())
    }

    /// Removes `key` at the port, records `Missing` into correlated
    /// targets, then revokes leases and the capture owner — in that
    /// order, so a port failure leaves every local structure
    /// untouched and the removal retryable.
    fn remove_key(
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
        self.probe_session.revoke_leases(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use synvelis_project::probe::{SemanticSignalEntityRef, SignalBoundary};
    use synvelis_project::{
        AutomationTimeScope, ControlOwner, CounterIdGenerator, ParamMapping, ParameterRole,
        ProbeObservationUpdate, ProjectController, ProjectId, ProjectOp, StripId, ValueKind,
    };

    use super::*;
    use crate::session::ProjectSession;

    // -----------------------------------------------------------------
    // fixture — mirrors tests/sf04_s2_probe_trace.rs's fake-port
    // pattern, trimmed to what ownership accounting observes.
    // -----------------------------------------------------------------

    fn session() -> ProjectSession {
        let project = Project::new(ProjectId::new(0x5107), "sf04-s5");
        let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
        ProjectSession::new(controller)
    }

    fn add_target(s: &mut ProjectSession) -> ParameterTargetId {
        let rev = s.revision();
        s.apply(
            ProjectOp::AddParameterTarget {
                name: "p".into(),
                owner: ControlOwner::Project,
                role: ParameterRole::Generic,
                time_scope: AutomationTimeScope::ProjectMusical,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.0,
            },
            rev,
        )
        .expect("fixture op");
        *s.project()
            .control
            .targets
            .keys()
            .rfind(|id| synvelis_project::is_issuable_raw(id.raw()))
            .unwrap()
    }

    fn point() -> SemanticProbePoint {
        SemanticProbePoint {
            entity: SemanticSignalEntityRef::Strip(StripId::new(7)),
            boundary: SignalBoundary::PostFader,
            port: None,
        }
    }

    fn view(raw: u64) -> ViewInstanceId {
        ViewInstanceId::new(raw).expect("nonzero view id")
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum FailOp {
        Request,
        Remove,
        Capture,
        Stop,
    }

    #[derive(Default)]
    struct FakePort {
        pending: VecDeque<ProbeObservationUpdate>,
        requests: Vec<(ProbeSubscriptionKey, ProbeMode)>,
        modes: Vec<(ProbeSubscriptionKey, ProbeMode)>,
        captures: Vec<(ProbeSubscriptionKey, u64)>,
        stops: Vec<ProbeSubscriptionKey>,
        removes: Vec<ProbeSubscriptionKey>,
        recorded: Vec<(ParameterTargetId, ProbeObservation)>,
        bindings: BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>,
        segments: Vec<ProbeCaptureSegment>,
        fails: BTreeSet<FailOp>,
    }

    impl FakePort {
        fn queue(&mut self, key: ProbeSubscriptionKey) {
            self.pending.push_back(ProbeObservationUpdate {
                key,
                observation: ProbeObservation::Missing,
            });
        }
    }

    impl ProbeTracePort for FakePort {
        fn probe_request(
            &mut self,
            _project: &Project,
            key: ProbeSubscriptionKey,
            mode: ProbeMode,
        ) -> Result<(), ProbePortError> {
            if self.fails.contains(&FailOp::Request) {
                return Err(ProbePortError::Failed("injected".into()));
            }
            self.requests.push((key, mode));
            Ok(())
        }

        fn probe_set_mode(
            &mut self,
            key: ProbeSubscriptionKey,
            mode: ProbeMode,
        ) -> Result<(), ProbePortError> {
            self.modes.push((key, mode));
            Ok(())
        }

        fn probe_capture(
            &mut self,
            key: ProbeSubscriptionKey,
            max_frames: u64,
        ) -> Result<(), ProbePortError> {
            if self.fails.contains(&FailOp::Capture) {
                return Err(ProbePortError::Failed("injected".into()));
            }
            self.captures.push((key, max_frames));
            Ok(())
        }

        fn probe_stop_capture(&mut self, key: ProbeSubscriptionKey) -> Result<(), ProbePortError> {
            if self.fails.contains(&FailOp::Stop) {
                return Err(ProbePortError::Failed("injected".into()));
            }
            self.stops.push(key);
            Ok(())
        }

        fn probe_remove(&mut self, key: ProbeSubscriptionKey) -> Result<(), ProbePortError> {
            if self.fails.contains(&FailOp::Remove) {
                return Err(ProbePortError::Failed("injected".into()));
            }
            self.removes.push(key);
            Ok(())
        }

        fn probe_drain(&mut self) -> Vec<ProbeObservationUpdate> {
            self.pending.drain(..).collect()
        }

        fn probe_bindings(&self) -> &BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding> {
            &self.bindings
        }

        fn probe_segments(&self) -> &[ProbeCaptureSegment] {
            &self.segments
        }

        fn record_probe(
            &mut self,
            target: ParameterTargetId,
            observation: ProbeObservation,
        ) -> bool {
            self.recorded.push((target, observation));
            true
        }

        fn trace(&self, _target: ParameterTargetId) -> Option<&ControlTrace> {
            None
        }

        fn traces(&self) -> Vec<(ParameterTargetId, ControlTrace)> {
            Vec::new()
        }
    }

    fn subscribe(
        s: &mut ProjectSession,
        port: &mut FakePort,
        subscriber: ViewInstanceId,
        target: ParameterTargetId,
        kind: SubscriptionKind,
        mode: ProbeMode,
    ) -> Result<ProbeSubscriptionKey, ProbeTraceError> {
        s.control_with(|c, p| c.probe_subscribe(p, port, subscriber, target, point(), kind, mode))
    }

    fn missing_records(port: &FakePort) -> usize {
        port.recorded
            .iter()
            .filter(|(_, o)| matches!(o, ProbeObservation::Missing))
            .count()
    }

    // -----------------------------------------------------------------
    // SF-PROBE-06: shared subscription dedup + lease accounting
    // -----------------------------------------------------------------

    #[test]
    fn sf04_s5_two_views_share_one_port_request_and_fan_out() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let t2 = add_target(&mut s);
        let mut port = FakePort::default();
        let (v1, v2) = (view(1), view(2));

        let k1 = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1");
        let k2 = subscribe(
            &mut s,
            &mut port,
            v2,
            t2,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v2");
        assert_eq!(k1, k2);
        // The second subscriber joined the live hold — the port saw
        // exactly one request, so no broker capture can duplicate.
        assert_eq!(port.requests, vec![(k1, ProbeMode::Monitor)]);

        assert_eq!(
            s.control().probe_lease_holders(k1),
            vec![ProbePrincipal::View(v1), ProbePrincipal::View(v2)]
        );
        assert_eq!(s.control().probe_subscriber_keys(v1), vec![k1]);
        assert_eq!(s.control().probe_subscriber_keys(v2), vec![k1]);
        assert_eq!(s.control().probe_targets(k1).len(), 2);

        // One drain update still fans out to every correlated target.
        port.queue(k1);
        let report = s.control_mut().probe_drain(&mut port);
        assert_eq!(report.recorded, 2);
        assert!(port.recorded.iter().any(|(t, _)| *t == t1));
        assert!(port.recorded.iter().any(|(t, _)| *t == t2));
    }

    #[test]
    fn sf04_s5_later_subscriber_mode_does_not_change_shared_binding() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let (v1, v2) = (view(1), view(2));

        let key = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1");
        subscribe(
            &mut s,
            &mut port,
            v2,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Capture,
        )
        .expect("subscribe v2");
        // `mode` applies only when no live hold exists — the shared
        // binding kept Monitor and no set_mode was issued.
        assert_eq!(port.requests, vec![(key, ProbeMode::Monitor)]);
        assert_eq!(port.modes, Vec::new());

        // Mode changes remain a session-authority operation.
        s.control_mut()
            .probe_set_mode(&mut port, key, ProbeMode::Off)
            .expect("set_mode");
        assert_eq!(port.modes, vec![(key, ProbeMode::Off)]);
    }

    #[test]
    fn sf04_s5_partial_release_keeps_hold_and_last_release_removes() {
        let mut s = session();
        let (t1, t2) = (add_target(&mut s), add_target(&mut s));
        let mut port = FakePort::default();
        let (v1, v2) = (view(1), view(2));

        let key = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1");
        subscribe(
            &mut s,
            &mut port,
            v2,
            t2,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v2");

        // Window close of v1: lease drops, port subscription and the
        // shared binding survive for v2 — no remove, no Missing.
        let report = s
            .control_mut()
            .probe_unsubscribe(&mut port, v1, key)
            .expect("unsubscribe v1");
        assert_eq!(report.released, vec![key]);
        assert_eq!(report.removed, Vec::new());
        assert_eq!(port.removes, Vec::new());
        assert_eq!(
            s.control().probe_lease_holders(key),
            vec![ProbePrincipal::View(v2)]
        );
        assert_eq!(s.control().probe_subscriber_keys(v1), Vec::new());
        assert_eq!(s.control().probe_subscriber_keys(v2), vec![key]);

        // Last lease leaves → probe_remove semantics: port remove +
        // Missing into every correlated target, correlation kept.
        let report = s
            .control_mut()
            .probe_unsubscribe(&mut port, v2, key)
            .expect("unsubscribe v2");
        assert_eq!(report.released, vec![key]);
        assert_eq!(report.removed, vec![key]);
        assert_eq!(port.removes, vec![key]);
        assert_eq!(missing_records(&port), 2);
        assert_eq!(s.control().probe_lease_holders(key), Vec::new());
        assert_eq!(s.control().probe_targets(key).len(), 2);
    }

    #[test]
    fn sf04_s5_release_subscriber_is_window_close_scoped_to_that_view() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let (v1, v2, v3) = (view(1), view(2), view(3));

        let shared = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1 metrics");
        let solo = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Pcm,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1 pcm");
        subscribe(
            &mut s,
            &mut port,
            v2,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v2");

        // A view that never subscribed closes cleanly — empty report,
        // zero port traffic (window close is idempotent).
        let report = s
            .control_mut()
            .probe_release_subscriber(&mut port, v3)
            .expect("release v3");
        assert_eq!(report.released, Vec::new());
        assert_eq!(report.removed, Vec::new());
        assert_eq!(port.removes, Vec::new());

        // v1 closes: its sole-held PCM key is removed, the shared
        // metrics key survives on v2's lease alone.
        let report = s
            .control_mut()
            .probe_release_subscriber(&mut port, v1)
            .expect("release v1");
        assert_eq!(report.released, vec![shared, solo]);
        assert_eq!(report.removed, vec![solo]);
        assert_eq!(port.removes, vec![solo]);
        assert_eq!(missing_records(&port), 1);
        assert_eq!(
            s.control().probe_lease_holders(shared),
            vec![ProbePrincipal::View(v2)]
        );
        assert_eq!(s.control().probe_lease_holders(solo), Vec::new());
        assert_eq!(s.control().probe_subscriber_keys(v1), Vec::new());
        assert_eq!(s.control().probe_subscriber_keys(v2), vec![shared]);
    }

    // -----------------------------------------------------------------
    // SF-PROBE-06: capture ownership
    // -----------------------------------------------------------------

    #[test]
    fn sf04_s5_capture_owner_gates_view_abort() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let (v1, v2, v3) = (view(1), view(2), view(3));

        let key = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1");
        subscribe(
            &mut s,
            &mut port,
            v2,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v2");

        s.control_mut()
            .probe_capture_for(&mut port, v2, key, 4_096)
            .expect("capture_for v2");
        assert_eq!(port.captures, vec![(key, 4_096)]);
        assert_eq!(
            s.control().probe_capture_owner(key),
            Some(ProbePrincipal::View(v2))
        );

        // Non-owner with a lease → NotCaptureOwner; no lease at all →
        // NotSubscriber. Neither reaches the port.
        let err = s
            .control_mut()
            .probe_abort_capture(&mut port, v1, key)
            .expect_err("v1 is not owner");
        assert!(matches!(err, ProbeTraceError::NotCaptureOwner { .. }));
        let err = s
            .control_mut()
            .probe_abort_capture(&mut port, v3, key)
            .expect_err("v3 holds no lease");
        assert!(matches!(err, ProbeTraceError::NotSubscriber { .. }));
        assert_eq!(port.stops, Vec::new());

        // A port failure on the owner's abort keeps the owner
        // recorded — the abort may be retried.
        port.fails.insert(FailOp::Stop);
        let err = s
            .control_mut()
            .probe_abort_capture(&mut port, v2, key)
            .expect_err("stop failure");
        assert!(matches!(err, ProbeTraceError::Port(_)));
        port.fails.remove(&FailOp::Stop);
        assert_eq!(
            s.control().probe_capture_owner(key),
            Some(ProbePrincipal::View(v2))
        );

        // The owner aborts: port stop goes through, owner clears.
        s.control_mut()
            .probe_abort_capture(&mut port, v2, key)
            .expect("owner abort");
        assert_eq!(port.stops, vec![key]);
        assert_eq!(s.control().probe_capture_owner(key), None);

        // Session-authority capture records Session as owner — view
        // abort stays refused, session stop_capture clears it.
        s.control_mut()
            .probe_capture(&mut port, key, 8)
            .expect("session capture");
        assert_eq!(
            s.control().probe_capture_owner(key),
            Some(ProbePrincipal::Session)
        );
        let err = s
            .control_mut()
            .probe_abort_capture(&mut port, v2, key)
            .expect_err("session-owned capture");
        assert!(matches!(err, ProbeTraceError::NotCaptureOwner { .. }));
        s.control_mut()
            .probe_stop_capture(&mut port, key)
            .expect("session stop");
        assert_eq!(s.control().probe_capture_owner(key), None);
    }

    #[test]
    fn sf04_s5_owner_lease_loss_retreats_capture_to_session() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let (v1, v2) = (view(1), view(2));

        let key = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1");
        subscribe(
            &mut s,
            &mut port,
            v2,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v2");
        s.control_mut()
            .probe_capture_for(&mut port, v2, key, 4_096)
            .expect("capture_for v2");

        // v2's window closes while v1 still holds the key: the
        // capture must NOT stop — ownership retreats to Session.
        s.control_mut()
            .probe_unsubscribe(&mut port, v2, key)
            .expect("unsubscribe v2");
        assert_eq!(port.stops, Vec::new());
        assert_eq!(port.removes, Vec::new());
        assert_eq!(
            s.control().probe_capture_owner(key),
            Some(ProbePrincipal::Session)
        );
        assert_eq!(
            s.control().probe_lease_holders(key),
            vec![ProbePrincipal::View(v1)]
        );

        // Session authority can still stop the surviving capture.
        s.control_mut()
            .probe_stop_capture(&mut port, key)
            .expect("session stop_capture");
        assert_eq!(port.stops, vec![key]);
        assert_eq!(s.control().probe_capture_owner(key), None);
    }

    #[test]
    fn sf04_s5_capture_for_requires_lease_and_failed_capture_keeps_owner_empty() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let (v1, v2) = (view(1), view(2));

        let key = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1");

        // A view without a lease cannot start a capture.
        let err = s
            .control_mut()
            .probe_capture_for(&mut port, v2, key, 4_096)
            .expect_err("v2 holds no lease");
        assert!(matches!(err, ProbeTraceError::NotSubscriber { .. }));
        assert_eq!(port.captures, Vec::new());

        // A port failure propagates and records no owner.
        port.fails.insert(FailOp::Capture);
        let err = s
            .control_mut()
            .probe_capture_for(&mut port, v1, key, 4_096)
            .expect_err("port failure");
        assert!(matches!(err, ProbeTraceError::Port(_)));
        port.fails.remove(&FailOp::Capture);
        assert_eq!(s.control().probe_capture_owner(key), None);
    }

    // -----------------------------------------------------------------
    // SF-PROBE-06: session stop, legacy interop, fail-closed atomicity
    // -----------------------------------------------------------------

    #[test]
    fn sf04_s5_release_all_stops_every_held_subscription() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let v1 = view(1);

        // Legacy session request + a view subscription on a second
        // key + a view-owned capture — release_all revokes all.
        let legacy = s
            .control_with(|c, p| {
                c.probe_request(
                    p,
                    &mut port,
                    t1,
                    point(),
                    SubscriptionKind::Metrics,
                    ProbeMode::Monitor,
                )
            })
            .expect("legacy request");
        let view_key = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Pcm,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1");
        s.control_mut()
            .probe_capture_for(&mut port, v1, view_key, 4_096)
            .expect("capture_for");

        let report = s
            .control_mut()
            .probe_release_all(&mut port)
            .expect("release_all");
        let mut expected = vec![legacy, view_key];
        expected.sort();
        assert_eq!(report.released, expected);
        assert_eq!(report.removed, expected);
        assert_eq!(port.removes.len(), 2);
        for key in [legacy, view_key] {
            assert_eq!(s.control().probe_lease_holders(key), Vec::new());
            assert_eq!(s.control().probe_capture_owner(key), None);
        }
        assert_eq!(s.control().probe_subscriber_keys(v1), Vec::new());
    }

    #[test]
    fn sf04_s5_legacy_request_lease_survives_view_release() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let v1 = view(1);

        let key = s
            .control_with(|c, p| {
                c.probe_request(
                    p,
                    &mut port,
                    t1,
                    point(),
                    SubscriptionKind::Metrics,
                    ProbeMode::Monitor,
                )
            })
            .expect("legacy request");
        // A view joining the session-held key dedups against the
        // Session lease — still one port request total.
        subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe v1");
        assert_eq!(port.requests.len(), 1);
        assert_eq!(
            s.control().probe_lease_holders(key),
            vec![ProbePrincipal::Session, ProbePrincipal::View(v1)]
        );

        // The view leaving must not take the session's own hold down.
        s.control_mut()
            .probe_release_subscriber(&mut port, v1)
            .expect("release v1");
        assert_eq!(port.removes, Vec::new());
        assert_eq!(
            s.control().probe_lease_holders(key),
            vec![ProbePrincipal::Session]
        );

        // Session remove still works unconditionally afterwards.
        s.control_mut()
            .probe_remove(&mut port, key)
            .expect("session remove");
        assert_eq!(port.removes, vec![key]);
        assert_eq!(s.control().probe_lease_holders(key), Vec::new());
    }

    #[test]
    fn sf04_s5_port_failures_leave_local_state_untouched() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let v1 = view(1);

        // Failed first-hold request: no lease, no correlation claim.
        port.fails.insert(FailOp::Request);
        let err = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect_err("request failure");
        assert!(matches!(err, ProbeTraceError::Port(_)));
        port.fails.remove(&FailOp::Request);
        let key = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("retry subscribe");

        // Failed last-lease release: lease retained, retry succeeds.
        port.fails.insert(FailOp::Remove);
        let err = s
            .control_mut()
            .probe_unsubscribe(&mut port, v1, key)
            .expect_err("remove failure");
        assert!(matches!(err, ProbeTraceError::Port(_)));
        port.fails.remove(&FailOp::Remove);
        assert_eq!(
            s.control().probe_lease_holders(key),
            vec![ProbePrincipal::View(v1)]
        );
        assert_eq!(s.control().probe_subscriber_keys(v1), vec![key]);
        s.control_mut()
            .probe_unsubscribe(&mut port, v1, key)
            .expect("retry unsubscribe");
        assert_eq!(port.removes, vec![key]);
        assert_eq!(s.control().probe_lease_holders(key), Vec::new());
    }

    #[test]
    fn sf04_s5_resubscribe_after_remove_is_a_fresh_port_request() {
        let mut s = session();
        let t1 = add_target(&mut s);
        let mut port = FakePort::default();
        let v1 = view(1);

        let key = subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Monitor,
        )
        .expect("subscribe");
        s.control_mut()
            .probe_unsubscribe(&mut port, v1, key)
            .expect("unsubscribe");
        assert_eq!(port.removes, vec![key]);

        // Stopped binding revival: no live lease → port request again.
        subscribe(
            &mut s,
            &mut port,
            v1,
            t1,
            SubscriptionKind::Metrics,
            ProbeMode::Capture,
        )
        .expect("resubscribe");
        assert_eq!(port.requests.len(), 2);
        assert_eq!(port.requests[1], (key, ProbeMode::Capture));
        assert_eq!(
            s.control().probe_lease_holders(key),
            vec![ProbePrincipal::View(v1)]
        );
    }
}
