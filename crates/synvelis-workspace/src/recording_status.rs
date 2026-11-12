//! Recording status read model (Synvelis #102 PC-C5 slice 4,
//! UI-C5-04) — the recording-transaction status surface behind the
//! Inspector/transport recording indicator.
//!
//! [`recording_status`] maps the [`ProjectSession`]'s recording
//! evidence — the open [`RecordingTransaction`], its fixed vs live
//! revision, the caller-held [`PreparedRecordingCommit`], the last
//! [`RecordDisposition`], the applied registry and the bounded
//! record-notification log — onto the five display states
//! [`RecordingDisplayState::Conflict`] / `Prepare` / `Pending` /
//! `Applied` / `Unknown`, with per-row explanations whose
//! [`RecordingProvenance`] carries the domain values verbatim
//! ([`SealReason`], [`RecordCommitError`], [`RecordNotification`],
//! [`RecordDisposition`], [`AppliedRecord`], [`RecordHealthSummary`],
//! revision comparisons) — never a `StateProjection.provenance`
//! string tag alone.
//!
//! **"Prepare" is not a persisted transaction state.** The domain
//! has no durable `Prepared` variant: [`PreparedRecordingCommit`] is
//! a caller-held snapshot and `prepare_record_commit` runs
//! synchronously inside the commit path, so "prepare in flight" is
//! unobservable. `Prepare` therefore means *awaiting apply*: the
//! caller still holds a prepared payload that
//! [`ControlSession::preflight_record_finish`] verifies against the
//! live session evidence (`Live` verdict) and
//! `control_record_finish` can still consume. `Pending` is every
//! other open transaction — accumulating, sealed, or
//! [`RecordingTxnState::Rejected`] — with the verbatim state carried
//! in provenance.
//!
//! **Conflict is evidence, not a guess.** A transaction displays
//! `Conflict` when `prepare_record_commit`'s own first check would
//! fail — `tx.state == Conflict` (a prepare already reported it) or
//! `fixed_revision != live` — because the revision is monotonic and
//! the next commit is already determined to conflict. Rebase is
//! never automatic: `Conflict` offers Inspect (rebase preview) /
//! `ExplicitRebase` / Abort, and no Retry until the revision is
//! re-pinned.
//!
//! Read-side only — the same contract [`crate::relation_ui`]
//! follows: no project/controller/session mutation, non-RT only.

use std::collections::BTreeMap;

use ruviform_media_core::ClockEpochId;
use synvelis_project::{
    AutomationLaneId, AutomationTimeScope, BeatPoint, ControlOwner, ParameterTargetId, Project,
    ProjectRevision,
};

use crate::control::{
    AppliedRecord, ControlSession, LoopPassId, PlaybackStatus, PreparedRecordingCommit,
    RecordCommitError, RecordDisposition, RecordGap, RecordHealthSummary, RecordNotification,
    RecordingTransaction, RecordingTransactionId, RecordingTxnState, SealReason,
};
use crate::session::ProjectSession;

/// Default number of trailing record notifications
/// [`recording_status_with`] surfaces — the bounded log tail a status
/// view needs; overflow beyond the session's own
/// `RECORD_NOTIFY_CAP` is reported via
/// [`RecordingStatusView::notices_dropped`].
pub const RECORDING_STATUS_NOTICE_TAIL: usize = 8;

/// Bounds for [`recording_status_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordingStatusOptions {
    /// How many of the newest record notifications to surface
    /// (oldest→newest within the tail).
    pub notice_tail: usize,
}

impl Default for RecordingStatusOptions {
    fn default() -> Self {
        Self {
            notice_tail: RECORDING_STATUS_NOTICE_TAIL,
        }
    }
}

/// The five user-visible recording states (UI-C5-04). This is a
/// *display* mapping over session evidence — it never invents a
/// domain state and never collapses the verbatim
/// [`RecordingTxnState`] into it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RecordingDisplayState {
    /// The open transaction's fixed revision drifted from live (or a
    /// prepare already reported `Conflict`) — commit is determined
    /// to fail until an explicit rebase re-pins the revision.
    Conflict,
    /// No drift and the caller-held prepared payload verifies
    /// `Live` — awaiting `control_record_finish`.
    Prepare,
    /// An open transaction that is neither conflicted nor
    /// apply-ready — accumulating, sealed, or `Rejected` (the
    /// verbatim state stays in provenance).
    Pending,
    /// No open transaction; the last disposition is `Committed`.
    Applied,
    /// No open transaction and no committed disposition — idle, or
    /// the last transaction was `Aborted`.
    Unknown,
}

/// Whether a caller-held [`PreparedRecordingCommit`] can still be
/// consumed — verified against live session evidence, never trusted
/// on the caller's word.
#[derive(Clone, Debug, PartialEq)]
pub enum PreparedVerdict {
    /// Identity checks pass and `expected_revision` is still live —
    /// `control_record_finish` can apply it.
    Live,
    /// Identity checks pass but the revision moved since prepare —
    /// finish's CAS would reject; the payload is stale.
    StaleRevision {
        /// The revision the payload was prepared against.
        expected: ProjectRevision,
        /// The live revision now.
        current: ProjectRevision,
    },
    /// [`ControlSession::preflight_record_finish`] refused the
    /// payload — `AlreadyApplied`/`IntegrityConflict`/
    /// `UnknownTransaction`, verbatim.
    Rejected(RecordCommitError),
}

/// The caller-held prepared payload as display data plus its
/// verified [`PreparedVerdict`].
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedCommitView {
    /// The transaction the payload was prepared from.
    pub id: RecordingTransactionId,
    /// Payload fingerprint — the idempotency/integrity key.
    pub fingerprint: u64,
    /// The revision the apply CAS requires.
    pub expected_revision: ProjectRevision,
    /// Whether the transaction was sealed at prepare time.
    pub was_sealed: bool,
    /// Whether the payload carries a project write (`op.is_some()`).
    pub has_write: bool,
    /// The verified verdict against live session evidence.
    pub verdict: PreparedVerdict,
}

/// One recorded lane of the open transaction — the join of
/// `fixed_lanes` with the segment/gap/delivery evidence, mirroring
/// [`crate::control::RebasePreview`]'s lane reporting plus the
/// session-only overlays a status view needs.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingLaneView {
    /// The lane recorded into.
    pub lane: AutomationLaneId,
    /// The fixed target that recorded into the lane.
    pub target: ParameterTargetId,
    /// Whether the fixed target still exists in the project.
    pub target_exists: bool,
    /// Whether the lane still exists and still feeds `target`.
    pub lane_exists: bool,
    /// Manual points recorded into the lane.
    pub recorded_points: usize,
    /// `(first, last)` recorded position, when non-empty — same
    /// first/last convention [`crate::control::RebasePreview`] uses.
    pub recorded_range: Option<(BeatPoint, BeatPoint)>,
    /// Explicit data-loss runs on the lane (verbatim).
    pub gaps: Vec<RecordGap>,
    /// Delivered write intervals recorded for the lane.
    pub delivery_intervals: usize,
}

/// The open recording transaction projected for display — every
/// field verbatim from [`RecordingTransaction`] plus the live
/// revision it is compared against.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingTxnView {
    /// Session-unique transaction id.
    pub id: RecordingTransactionId,
    /// Lifecycle state — verbatim [`RecordingTxnState`]; `Rejected`
    /// stays distinguishable here even though the display state is
    /// `Pending`.
    pub state: RecordingTxnState,
    /// Why the write gate closed, when sealed — verbatim
    /// [`SealReason`].
    pub sealed: Option<SealReason>,
    /// Session start position (project beats).
    pub started_at: BeatPoint,
    /// Clock epoch captured at start.
    pub epoch: ClockEpochId,
    /// The loop pass this transaction writes into.
    pub pass: LoopPassId,
    /// Live project revision fixed at `record_start`.
    pub fixed_revision: ProjectRevision,
    /// The live revision this view was computed at.
    pub current_revision: ProjectRevision,
    /// `fixed_revision != current_revision` — the same check
    /// `prepare_record_commit` performs first.
    pub revision_stale: bool,
    /// Targets armed+write-capable when `record_start` ran.
    pub fixed_targets: Vec<ParameterTargetId>,
    /// Target → write-lane mapping fixed at start.
    pub fixed_lanes: BTreeMap<ParameterTargetId, AutomationLaneId>,
    /// Target → `(owner, time_scope)` fixed at start.
    pub fixed_scopes: BTreeMap<ParameterTargetId, (ControlOwner, AutomationTimeScope)>,
    /// Recorded lanes with data (segments, gaps or deliveries).
    pub lanes: Vec<RecordingLaneView>,
    /// Total manual points across all lanes.
    pub recorded_points: usize,
    /// Events dropped into this transaction's gap ledger.
    pub record_dropped: u64,
    /// Delivery intervals dropped at the per-lane cap.
    pub deliveries_dropped: u64,
    /// Drops merged into an at-capacity gap ledger.
    pub record_gaps_merged: u64,
}

/// The last lifecycle outcome — the verbatim
/// [`RecordDisposition`] plus the applied-registry receipt when the
/// commit produced a revision.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingOutcomeView {
    /// `Committed`/`Aborted` — verbatim.
    pub disposition: RecordDisposition,
    /// The session-scoped exactly-once receipt for a committed id
    /// that produced a revision (`None` for empty commits and
    /// aborts).
    pub receipt: Option<AppliedRecord>,
}

/// Typed provenance for one [`RecordingExplanation`] row — the
/// domain evidence behind the text, kept as values (never reduced
/// to a string tag).
#[derive(Clone, Debug, PartialEq)]
pub enum RecordingProvenance {
    /// The open transaction's own fields.
    Transaction {
        /// Transaction id.
        id: RecordingTransactionId,
        /// Verbatim lifecycle state.
        state: RecordingTxnState,
        /// Verbatim seal reason, when sealed.
        sealed: Option<SealReason>,
    },
    /// The fixed-vs-live revision comparison — the same evidence
    /// `prepare_record_commit`'s first check uses.
    RevisionDrift {
        /// Revision fixed at `record_start`/last rebase.
        fixed: ProjectRevision,
        /// Live revision now.
        current: ProjectRevision,
    },
    /// The caller-held prepared payload and its verified verdict.
    Prepared {
        /// Payload transaction id.
        id: RecordingTransactionId,
        /// Revision the apply CAS requires.
        expected_revision: ProjectRevision,
        /// Whether the transaction was sealed at prepare time.
        was_sealed: bool,
        /// The verified verdict.
        verdict: PreparedVerdict,
    },
    /// The most recent lifecycle disposition.
    Disposition(RecordDisposition),
    /// The applied-registry receipt of a committed transaction.
    AppliedReceipt {
        /// Committed transaction id.
        id: RecordingTransactionId,
        /// Fingerprint + produced revision.
        receipt: AppliedRecord,
    },
    /// One entry of the bounded record-notification log.
    Notification(RecordNotification),
    /// One fixed target's record-health counters.
    Health(RecordHealthSummary),
    /// Session gates: record-enabled flag + transport status.
    Gates {
        /// Whether record is enabled.
        record_enabled: bool,
        /// Rolling/paused/stopped.
        status: PlaybackStatus,
    },
}

/// One explanation row: display text plus the typed evidence that
/// produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingExplanation {
    /// English display text (same convention as `gateway_ui` notes).
    pub text: String,
    /// The typed provenance behind the text.
    pub provenance: RecordingProvenance,
}

/// A legal user action for the current display state — each maps
/// onto an existing session op; the view never performs them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RecordingAction {
    /// `control_record_rebase_preview` — inspect what an explicit
    /// rebase would write. Never merges.
    Inspect,
    /// The idempotent commit path — `control_record_commit` without
    /// a held payload, `control_record_finish` with a `Live` one.
    /// `AlreadyApplied`/`Conflict`/`Rejected` are typed responses,
    /// never silent double-writes.
    Retry,
    /// `control_record_approve_rebase` — explicit approval after
    /// Inspect; re-pins the fixed revision, keeps the seal.
    ExplicitRebase,
    /// `control_record_abort` — discard the open transaction.
    Abort,
}

/// The recording-status view: display state, verbatim evidence,
/// explanation rows and the legal actions for that state.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingStatusView {
    /// The mapped display state.
    pub state: RecordingDisplayState,
    /// Live [`ProjectRevision`] the view was computed at — compare
    /// with [`ProjectSession::revision`] via [`Self::is_current`].
    pub revision: ProjectRevision,
    /// Session record-enabled gate.
    pub record_enabled: bool,
    /// Transport status at view time.
    pub transport: PlaybackStatus,
    /// The open transaction's projection, when one exists.
    pub transaction: Option<RecordingTxnView>,
    /// The caller-held prepared payload's verified projection.
    pub prepared: Option<PreparedCommitView>,
    /// The last lifecycle outcome, when any commit/abort ran.
    pub outcome: Option<RecordingOutcomeView>,
    /// Bounded tail (newest last) of the record-notification log.
    pub notices: Vec<RecordNotification>,
    /// Notifications the session dropped because its bounded log was
    /// full — counted, never silent.
    pub notices_dropped: u64,
    /// State/evidence/action explanation rows, each with typed
    /// provenance.
    pub explanations: Vec<RecordingExplanation>,
    /// Legal actions for `state`, in deterministic order.
    pub actions: Vec<RecordingAction>,
}

impl RecordingStatusView {
    /// Whether `revision` still matches `session`'s live revision —
    /// `false` means the view describes a stale project state.
    #[must_use]
    pub fn is_current(&self, session: &ProjectSession) -> bool {
        self.revision == session.revision()
    }
}

/// The recording status of `session` with the default
/// [`RecordingStatusOptions`] and no caller-held prepared payload —
/// `Prepare` is unreachable through this entry point by design (the
/// session cannot observe a payload it does not hold).
#[must_use]
pub fn recording_status(session: &ProjectSession) -> RecordingStatusView {
    recording_status_with(session, None, RecordingStatusOptions::default())
}

/// [`recording_status`] with the caller-held prepared payload and
/// explicit bounds. `prepared` is evidence the session cannot see —
/// it is verified through
/// [`ControlSession::preflight_record_finish`] and the
/// `expected_revision == live` check, never trusted verbatim.
#[must_use]
pub fn recording_status_with(
    session: &ProjectSession,
    prepared: Option<&PreparedRecordingCommit>,
    options: RecordingStatusOptions,
) -> RecordingStatusView {
    let live = session.revision();
    let control = session.control();
    let record_enabled = control.record_enabled();
    let transport = control.playback().status;

    // -- caller-held prepared payload: verify, never trust ---------
    let prepared_view = prepared.map(|p| verify_prepared(control, p, live));
    let prepared_live = matches!(
        prepared_view.as_ref().map(|v| &v.verdict),
        Some(PreparedVerdict::Live)
    );

    // -- open transaction projection --------------------------------
    let tx = control.recording();
    let transaction = tx.map(|tx| txn_view(tx, live, session.project()));
    let disposition = control.last_disposition();
    let state = display_state(tx, live, prepared_live, disposition);
    let outcome = outcome_view(control, disposition);
    let notices = notice_tail(control, options.notice_tail);
    let notices_dropped = control.record_notifications_dropped();
    let actions = legal_actions(state);
    let explanations = build_explanations(&StatusEvidence {
        state,
        live,
        transaction: transaction.as_ref(),
        prepared: prepared_view.as_ref(),
        outcome: outcome.as_ref(),
        notices: &notices,
        control,
        record_enabled,
        transport,
    });

    RecordingStatusView {
        state,
        revision: live,
        record_enabled,
        transport,
        transaction,
        prepared: prepared_view,
        outcome,
        notices,
        notices_dropped,
        explanations,
        actions,
    }
}

/// Verifies a caller-held prepared payload against live session
/// evidence — [`ControlSession::preflight_record_finish`] for
/// identity (applied registry, open-transaction id + fingerprint)
/// and `expected_revision == live` for the apply CAS `finish` will
/// run. The caller's claim is never trusted verbatim.
fn verify_prepared(
    control: &ControlSession,
    prepared: &PreparedRecordingCommit,
    live: ProjectRevision,
) -> PreparedCommitView {
    let verdict = match control.preflight_record_finish(prepared) {
        Err(e) => PreparedVerdict::Rejected(e),
        Ok(()) if prepared.expected_revision != live => PreparedVerdict::StaleRevision {
            expected: prepared.expected_revision,
            current: live,
        },
        Ok(()) => PreparedVerdict::Live,
    };
    PreparedCommitView {
        id: prepared.id,
        fingerprint: prepared.fingerprint,
        expected_revision: prepared.expected_revision,
        was_sealed: prepared.was_sealed,
        has_write: prepared.op.is_some(),
        verdict,
    }
}

/// The display-state mapping (module docs): no open transaction →
/// `Applied` on a committed disposition else `Unknown`; an open
/// transaction → `Conflict` on reported state or fixed/live drift,
/// `Prepare` on a `Live` caller-held payload, `Pending` otherwise.
fn display_state(
    tx: Option<&RecordingTransaction>,
    live: ProjectRevision,
    prepared_live: bool,
    disposition: Option<RecordDisposition>,
) -> RecordingDisplayState {
    match tx {
        None => match disposition {
            Some(RecordDisposition::Committed { .. }) => RecordingDisplayState::Applied,
            _ => RecordingDisplayState::Unknown,
        },
        Some(tx) => {
            // The same evidence `prepare_record_commit` checks first:
            // a reported Conflict state or a fixed/live drift means
            // the next commit is already determined to conflict.
            if tx.state == RecordingTxnState::Conflict || tx.fixed_revision != live {
                RecordingDisplayState::Conflict
            } else if prepared_live {
                RecordingDisplayState::Prepare
            } else {
                RecordingDisplayState::Pending
            }
        }
    }
}

/// The legal actions per display state — each maps onto an existing
/// session op and the view never performs them. `Conflict` offers no
/// `Retry`: the commit is determined to re-conflict until the
/// revision is re-pinned, so Inspect → `ExplicitRebase` → (Retry via
/// the commit path) is the honest sequence.
fn legal_actions(state: RecordingDisplayState) -> Vec<RecordingAction> {
    match state {
        RecordingDisplayState::Conflict => vec![
            RecordingAction::Inspect,
            RecordingAction::ExplicitRebase,
            RecordingAction::Abort,
        ],
        RecordingDisplayState::Prepare | RecordingDisplayState::Pending => vec![
            RecordingAction::Inspect,
            RecordingAction::Retry,
            RecordingAction::Abort,
        ],
        // No open transaction — every record op would answer
        // `NoTransaction`; nothing is legal.
        RecordingDisplayState::Applied | RecordingDisplayState::Unknown => Vec::new(),
    }
}

/// The last lifecycle outcome with its applied-registry receipt
/// when the commit produced a revision.
fn outcome_view(
    control: &ControlSession,
    disposition: Option<RecordDisposition>,
) -> Option<RecordingOutcomeView> {
    disposition.map(|disposition| {
        let receipt = match disposition {
            RecordDisposition::Committed { id, .. } => control.applied_record(id),
            RecordDisposition::Aborted { .. } => None,
        };
        RecordingOutcomeView {
            disposition,
            receipt,
        }
    })
}

/// The bounded tail of the record-notification log (oldest→newest
/// within the tail).
fn notice_tail(control: &ControlSession, tail: usize) -> Vec<RecordNotification> {
    let len = control.record_notifications().len();
    control
        .record_notifications()
        .iter()
        .skip(len.saturating_sub(tail))
        .cloned()
        .collect()
}

/// The assembled session evidence one explanation pass reads —
/// bundles the inputs so [`build_explanations`] stays a flat row
/// builder.
struct StatusEvidence<'a> {
    state: RecordingDisplayState,
    live: ProjectRevision,
    transaction: Option<&'a RecordingTxnView>,
    prepared: Option<&'a PreparedCommitView>,
    outcome: Option<&'a RecordingOutcomeView>,
    notices: &'a [RecordNotification],
    control: &'a ControlSession,
    record_enabled: bool,
    transport: PlaybackStatus,
}

/// Builds the explanation rows in deterministic order: state
/// headline → session gates → transaction summary/seal/drift/lane
/// rows → prepared verdict → outcome + receipt → notification tail →
/// per-target health. Every row carries typed provenance.
fn build_explanations(e: &StatusEvidence<'_>) -> Vec<RecordingExplanation> {
    let mut explanations = Vec::new();
    let disposition = e.outcome.map(|o| o.disposition);
    explain(
        &mut explanations,
        state_headline(e.state, e.transaction, e.live, e.prepared, disposition),
        state_provenance(
            e.state,
            e.transaction,
            e.live,
            e.prepared,
            disposition,
            e.record_enabled,
            e.transport,
        ),
    );
    explain(
        &mut explanations,
        format!(
            "Record gate: {}recording enabled, transport {:?}.",
            if e.record_enabled { "" } else { "not " },
            e.transport
        ),
        RecordingProvenance::Gates {
            record_enabled: e.record_enabled,
            status: e.transport,
        },
    );
    if let Some(txv) = e.transaction {
        transaction_rows(txv, &mut explanations);
    }
    if let Some(pv) = e.prepared {
        prepared_row(pv, &mut explanations);
    }
    if let Some(outcome) = e.outcome {
        outcome_rows(outcome, &mut explanations);
    }
    for notice in e.notices {
        explain(
            &mut explanations,
            notice_text(notice),
            RecordingProvenance::Notification(notice.clone()),
        );
    }
    if let Some(txv) = e.transaction {
        health_rows(e.control, &txv.fixed_targets, &mut explanations);
    }
    explanations
}

/// Transaction summary, seal, revision-drift and per-lane rows —
/// all carrying the transaction's verbatim provenance.
fn transaction_rows(txv: &RecordingTxnView, explanations: &mut Vec<RecordingExplanation>) {
    let prov = || RecordingProvenance::Transaction {
        id: txv.id,
        state: txv.state,
        sealed: txv.sealed,
    };
    explain(
        explanations,
        format!(
            "Transaction {}: state {:?}, fixed revision {:?}, {} fixed target(s), {} recorded point(s).",
            txv.id,
            txv.state,
            txv.fixed_revision,
            txv.fixed_targets.len(),
            txv.recorded_points
        ),
        prov(),
    );
    if let Some(reason) = txv.sealed {
        explain(
            explanations,
            format!(
                "Write gate sealed ({reason:?}) — new points are rejected; commit, rebase or abort stay legal."
            ),
            prov(),
        );
    }
    if txv.revision_stale {
        explain(
            explanations,
            format!(
                "Fixed revision {:?} differs from live {:?} — the next commit conflicts until an explicit rebase re-pins it.",
                txv.fixed_revision, txv.current_revision
            ),
            RecordingProvenance::RevisionDrift {
                fixed: txv.fixed_revision,
                current: txv.current_revision,
            },
        );
    }
    for lane in &txv.lanes {
        explain(
            explanations,
            format!(
                "Lane {} (target {}): {} point(s), range {:?}, {} gap(s), {} delivery interval(s).",
                lane.lane,
                lane.target,
                lane.recorded_points,
                lane.recorded_range,
                lane.gaps.len(),
                lane.delivery_intervals
            ),
            prov(),
        );
    }
}

/// The caller-held prepared payload's verified-verdict row.
fn prepared_row(pv: &PreparedCommitView, explanations: &mut Vec<RecordingExplanation>) {
    let text = match &pv.verdict {
        PreparedVerdict::Live => format!(
            "Prepared commit {} verified — finish applies it as one undo step.",
            pv.id
        ),
        PreparedVerdict::StaleRevision { expected, current } => format!(
            "Prepared commit {} is stale — expected revision {expected:?}, live {current:?}; finish would be rejected by CAS.",
            pv.id
        ),
        PreparedVerdict::Rejected(err) => format!("Prepared commit {} rejected: {err}.", pv.id),
    };
    explain(
        explanations,
        text,
        RecordingProvenance::Prepared {
            id: pv.id,
            expected_revision: pv.expected_revision,
            was_sealed: pv.was_sealed,
            verdict: pv.verdict.clone(),
        },
    );
}

/// The last disposition row plus the applied-registry receipt row
/// when the commit produced a revision.
fn outcome_rows(outcome: &RecordingOutcomeView, explanations: &mut Vec<RecordingExplanation>) {
    let text = match outcome.disposition {
        RecordDisposition::Committed { id, revision } => match revision {
            Some(rev) => format!("Transaction {id} committed at revision {rev:?}."),
            None => format!("Transaction {id} committed — empty session, nothing applied."),
        },
        RecordDisposition::Aborted { id } => {
            format!("Transaction {id} was aborted — nothing applied.")
        }
    };
    explain(
        explanations,
        text,
        RecordingProvenance::Disposition(outcome.disposition),
    );
    if let (RecordDisposition::Committed { id, .. }, Some(receipt)) =
        (outcome.disposition, outcome.receipt)
    {
        explain(
            explanations,
            format!(
                "Applied receipt for {id}: fingerprint {}, revision {:?}.",
                receipt.fingerprint, receipt.revision
            ),
            RecordingProvenance::AppliedReceipt { id, receipt },
        );
    }
}

/// Per-target record-health rows. Health is a per-target lookup —
/// the session exposes no table enumeration, so rows cover the
/// caller-supplied target set only (the open transaction's fixed
/// targets; spec Deferred Design Concerns).
fn health_rows(
    control: &ControlSession,
    targets: &[ParameterTargetId],
    explanations: &mut Vec<RecordingExplanation>,
) {
    for target in targets {
        if let Some(health) = control.record_health(*target) {
            explain(
                explanations,
                format!(
                    "Record health for {target}: dropped {}, queue_fulls {}, overflowed {}, rejected {}.",
                    health.dropped, health.queue_fulls, health.overflowed, health.rejected
                ),
                RecordingProvenance::Health(health),
            );
        }
    }
}

/// Projects the open transaction into its display view — verbatim
/// fields plus the lane join.
fn txn_view(
    tx: &RecordingTransaction,
    live: ProjectRevision,
    project: &Project,
) -> RecordingTxnView {
    let lanes = tx
        .fixed_lanes
        .iter()
        .filter_map(|(target, lane)| {
            let points = tx.segments.get(lane);
            let recorded_points = points.map_or(0, Vec::len);
            let recorded_range = points.and_then(|ps| {
                ps.first()
                    .zip(ps.last())
                    .map(|(first, last)| (first.pos, last.pos))
            });
            let gaps = tx.record_gaps.get(lane).cloned().unwrap_or_default();
            let delivery_intervals = tx.deliveries.get(lane).map_or(0, Vec::len);
            if recorded_points == 0 && gaps.is_empty() && delivery_intervals == 0 {
                return None;
            }
            Some(RecordingLaneView {
                lane: *lane,
                target: *target,
                target_exists: project.control.targets.contains_key(target),
                lane_exists: project
                    .control
                    .lanes
                    .get(lane)
                    .is_some_and(|l| l.target == *target),
                recorded_points,
                recorded_range,
                gaps,
                delivery_intervals,
            })
        })
        .collect();
    RecordingTxnView {
        id: tx.id,
        state: tx.state,
        sealed: tx.sealed,
        started_at: tx.started_at,
        epoch: tx.epoch,
        pass: tx.pass,
        fixed_revision: tx.fixed_revision,
        current_revision: live,
        revision_stale: tx.fixed_revision != live,
        fixed_targets: tx.fixed_targets.clone(),
        fixed_lanes: tx.fixed_lanes.clone(),
        fixed_scopes: tx.fixed_scopes.clone(),
        lanes,
        recorded_points: tx.segments.values().map(Vec::len).sum(),
        record_dropped: tx.record_dropped,
        deliveries_dropped: tx.deliveries_dropped,
        record_gaps_merged: tx.record_gaps_merged,
    }
}

fn explain(
    explanations: &mut Vec<RecordingExplanation>,
    text: impl Into<String>,
    provenance: RecordingProvenance,
) {
    explanations.push(RecordingExplanation {
        text: text.into(),
        provenance,
    });
}

/// The state headline text — the first explanation row.
fn state_headline(
    state: RecordingDisplayState,
    tx: Option<&RecordingTxnView>,
    live: ProjectRevision,
    prepared: Option<&PreparedCommitView>,
    disposition: Option<RecordDisposition>,
) -> String {
    match state {
        RecordingDisplayState::Conflict => format!(
            "Recording transaction conflicts with the live project revision (fixed {:?}, live {live:?}) — inspect the rebase preview, approve an explicit rebase, or abort.",
            tx.map(|t| t.fixed_revision)
        ),
        RecordingDisplayState::Prepare => format!(
            "Prepared commit {} awaits apply — finish writes it as one undo step.",
            prepared.map_or_else(|| "?".to_string(), |p| p.id.to_string())
        ),
        RecordingDisplayState::Pending => match tx {
            Some(t) if t.sealed.is_some() => format!(
                "Recording transaction {} is sealed ({:?}) — commit or abort to finish; the write gate is closed.",
                t.id, t.sealed
            ),
            Some(t) => format!(
                "Recording transaction {} is open — accumulating recorded points.",
                t.id
            ),
            None => "No recording transaction is open.".to_string(),
        },
        RecordingDisplayState::Applied => match disposition {
            Some(RecordDisposition::Committed { id, .. }) => {
                format!("Transaction {id} committed — the recording was applied.")
            }
            _ => "The last recording transaction committed.".to_string(),
        },
        RecordingDisplayState::Unknown => match disposition {
            Some(RecordDisposition::Aborted { id }) => {
                format!("Transaction {id} was aborted — nothing is applied and nothing is pending.")
            }
            _ => "No recording transaction is open and no commit is pending.".to_string(),
        },
    }
}

/// The headline row's typed provenance.
fn state_provenance(
    state: RecordingDisplayState,
    tx: Option<&RecordingTxnView>,
    live: ProjectRevision,
    prepared: Option<&PreparedCommitView>,
    disposition: Option<RecordDisposition>,
    record_enabled: bool,
    transport: PlaybackStatus,
) -> RecordingProvenance {
    match state {
        RecordingDisplayState::Conflict => match tx {
            Some(t) if t.fixed_revision != live => RecordingProvenance::RevisionDrift {
                fixed: t.fixed_revision,
                current: live,
            },
            Some(t) => RecordingProvenance::Transaction {
                id: t.id,
                state: t.state,
                sealed: t.sealed,
            },
            None => RecordingProvenance::Gates {
                record_enabled,
                status: transport,
            },
        },
        RecordingDisplayState::Prepare => prepared.map_or(
            RecordingProvenance::Gates {
                record_enabled,
                status: transport,
            },
            |p| RecordingProvenance::Prepared {
                id: p.id,
                expected_revision: p.expected_revision,
                was_sealed: p.was_sealed,
                verdict: p.verdict.clone(),
            },
        ),
        RecordingDisplayState::Pending => tx.map_or(
            RecordingProvenance::Gates {
                record_enabled,
                status: transport,
            },
            |t| RecordingProvenance::Transaction {
                id: t.id,
                state: t.state,
                sealed: t.sealed,
            },
        ),
        RecordingDisplayState::Applied | RecordingDisplayState::Unknown => disposition
            .map(RecordingProvenance::Disposition)
            .unwrap_or(RecordingProvenance::Gates {
                record_enabled,
                status: transport,
            }),
    }
}

/// Display text for one record notification — the variant's fields
/// rendered; the typed value itself rides in
/// [`RecordingProvenance::Notification`].
fn notice_text(notice: &RecordNotification) -> String {
    match notice {
        RecordNotification::QueueFull {
            target,
            dropped_total,
        } => format!("Record queue full for {target}: {dropped_total} event(s) dropped."),
        RecordNotification::DataGap {
            lane,
            start,
            end,
            dropped,
        } => format!("Data gap on lane {lane} [{start}..{end}): {dropped} event(s) lost."),
        RecordNotification::Rejected {
            target,
            kind,
            count,
        } => format!("{count} record event(s) rejected for {target}: {kind:?}."),
        RecordNotification::Partial { accepted } => {
            format!("Record drain partial: {accepted} event(s) accepted, more queued.")
        }
    }
}
