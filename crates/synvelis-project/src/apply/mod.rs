//! VCS-05 Safe Apply Selected Change — headless slice (synvelis#96).
//!
//! Applies a caller-selected subset of a canonical [`ChangeSet`]
//! (VCS-02) to the live [`Project`] through the
//! [`ProjectController`] single-writer path. The module is strictly
//! fail-closed: unknown, partial, ambiguous, unsupported, or
//! incomplete inputs never mutate the project, and every failure
//! leaves live state, undo history, Saved HEAD, the Git index, and
//! the Git working tree untouched.
//!
//! ## Contract
//!
//! - [`ApplyChangeIntent`] pins the `ChangeSet`, both endpoint
//!   snapshots, the expected live revision, the selected change IDs,
//!   a target-object fingerprint, and an explicit approval token.
//! - [`apply_selected`] resolves the `ChangeSet` plus both
//!   [`ImmutableSnapshot`] endpoints through [`ChangeSetSource`],
//!   verifies digest bindings and completeness, plans typed entity
//!   transplants (never JSON-path patching), checks the target
//!   fingerprint, runs asset/plugin/port preflight, and commits via
//!   one [`ProjectController::apply`] — exactly one undo step.
//! - [`ApplyJournal`] records `Pending`/`Applied`/`Failed` receipts
//!   so ACK-gap retries and crash recovery return accurate results
//!   instead of double-applying.
//! - [`target_fingerprint`] is the exported canonical fingerprint
//!   callers use to compute `expected_target_fingerprint`.
//!
//! Out of scope (deferred): `Removed` changes (cascade design —
//! every kind, including the monitor declarations), the rest of the
//! recording/comping/SMF/clock-epoch domains (only the authored
//! monitor declarations `TrackMonitorDecl`/`PunchMarks` ride the
//! apply surface, for `Added`/`Modified`), in-place restore,
//! engine-level application, and real OS/provider E2E (synvelis#97).

mod entity;
mod journal;
mod plan;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    Command, Error, ProjectController, ProjectId, ProjectRevision,
    diff::{
        ChangeId, ChangeKind, ChangeSet, ComparisonCompleteness, ComparisonEquality, ObjectKind,
        SemanticObjectRef,
    },
    project::Project,
    routing::PluginRef,
    snapshot::{
        BlobProbe, ContentDigest, ImmutableSnapshot, SnapshotCancel, SnapshotCompleteness,
        SnapshotValidation,
    },
};

pub use journal::{
    ApplyJournal, ApplyJournalError, ApplyJournalRecord, ApplyJournalState, DirectoryApplyJournal,
    InMemoryApplyJournal,
};

// ---------------------------------------------------------------------------
// intent / approval / receipt types
// ---------------------------------------------------------------------------

/// Explicit user approval token carried by an [`ApplyChangeIntent`].
///
/// The engine never interprets *how* consent was obtained — the app
/// layer fills `ticket` after the user approves the exact intent
/// (change set, selection, fingerprint, revision). An empty ticket is
/// rejected as [`ApplyError::Unapproved`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyApproval {
    /// Opaque approval reference (dialog/session/audit token).
    pub ticket: String,
}

/// A request to apply selected changes from one canonical `ChangeSet`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyChangeIntent {
    /// Digest identifying the [`ChangeSet`] to draw changes from.
    pub change_set_id: ContentDigest,
    /// `identity.digest` of the *after* snapshot the `ChangeSet` was
    /// computed against — pins the exact target bytes.
    pub source_snapshot_id: ContentDigest,
    /// The object the user targeted (e.g. the clip/track clicked in
    /// history UI). Included in the fingerprinted object set; a
    /// misidentified or missing target fails unchanged.
    pub target_object: SemanticObjectRef,
    /// Live revision the caller captured the intent at (CAS).
    pub expected_project_revision: ProjectRevision,
    /// The subset of `changeset.changes` to apply, by [`ChangeId`].
    pub selected_change_ids: Vec<ChangeId>,
    /// [`target_fingerprint`] over `{target_object} ∪ selected
    /// objects` computed against the *expected* live state.
    pub expected_target_fingerprint: ContentDigest,
    /// Explicit approval for this exact intent.
    pub approval: ApplyApproval,
}

/// Per-domain effect report — an apply never implies save, VCS, or
/// engine effects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyEffects {
    /// The live project was mutated through the single writer.
    pub live_applied: bool,
    /// Exactly one undo step was pushed (single-writer guarantee).
    pub single_undo_step: bool,
    /// No engine/runtime application was performed or claimed.
    pub engine_applied: bool,
    /// External-state guarantees (Saved HEAD / VCS index / worktree).
    pub untouched: ApplyUntouchedEffects,
}

/// The external state an apply must never touch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyUntouchedEffects {
    /// Saved HEAD was NOT touched.
    pub saved_head_unchanged: bool,
    /// The VCS index was NOT touched.
    pub vcs_index_untouched: bool,
    /// The Git working tree was NOT touched.
    pub git_worktree_untouched: bool,
}

fn untouched_effects() -> ApplyUntouchedEffects {
    ApplyUntouchedEffects {
        saved_head_unchanged: true,
        vcs_index_untouched: true,
        git_worktree_untouched: true,
    }
}

/// Durable record of one apply attempt — the idempotency/receipt
/// artifact returned to the caller and persisted by [`ApplyJournal`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyReceipt {
    /// Deterministic idempotency key (`apply-<sha256>` of the intent).
    pub apply_key: String,
    /// Digest of the intent that produced this receipt.
    pub intent_digest: ContentDigest,
    /// The `ChangeSet` applied.
    pub change_set_id: ContentDigest,
    /// The after-snapshot identity digest.
    pub source_snapshot_id: ContentDigest,
    /// Selected change IDs actually applied.
    pub applied_change_ids: Vec<ChangeId>,
    /// Live revision before the mutation.
    pub before_revision: ProjectRevision,
    /// Live revision after the mutation.
    pub after_revision: ProjectRevision,
    /// `Applied` = this call committed; `Replayed` = a prior committed
    /// attempt was reconciled (ACK gap / crash recovery).
    pub outcome: ApplyOutcome,
    /// Per-domain effect report.
    pub effects: ApplyEffects,
}

/// Receipt outcome.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplyOutcome {
    /// This invocation performed the mutation.
    Applied,
    /// The mutation was already committed by an earlier attempt;
    /// the recorded receipt is returned without re-applying.
    Replayed,
}

// ---------------------------------------------------------------------------
// errors
// ---------------------------------------------------------------------------

/// Fail-closed apply error. Every variant leaves the project
/// unchanged; callers must not treat any of them as partial success.
#[derive(Clone, Debug, PartialEq)]
pub enum ApplyError {
    /// Cooperative cancellation via [`SnapshotCancel`].
    Cancelled,
    /// The intent itself is malformed (empty selection, duplicate
    /// IDs, empty approval ticket is [`Self::Unapproved`]).
    InvalidIntent {
        /// What was wrong.
        detail: String,
    },
    /// `approval.ticket` was empty — no consent was recorded.
    Unapproved,
    /// The `ChangeSet` could not be resolved by the source port.
    ChangeSetNotFound {
        /// The requested `ChangeSet` digest.
        change_set_id: ContentDigest,
    },
    /// The resolved `ChangeSet` does not bind to the declared endpoint
    /// snapshot identities/digests.
    ChangeSetMismatch {
        /// What disagreed.
        detail: String,
    },
    /// The `ChangeSet` is not appliable: `Equal`, `Unknown`, or
    /// `Partial` completeness — unknown/partial changesets never
    /// apply.
    NotApplicable {
        /// Why the changeset was rejected.
        detail: String,
    },
    /// A selected change ID is not in the `ChangeSet`.
    UnknownChange {
        /// The missing change ID.
        change_id: ChangeId,
    },
    /// The object kind or change kind is outside the apply whitelist
    /// (includes every `Removed` change — cascade removal is deferred).
    Unsupported {
        /// Object kind.
        kind: ObjectKind,
        /// Change kind.
        change: ChangeKind,
        /// Machine-readable reason.
        detail: String,
    },
    /// A supported object changed a field outside the per-kind
    /// whitelist (e.g. project `ppqn`/tempo, non-`name` project
    /// fields).
    UnsupportedField {
        /// Object kind.
        kind: ObjectKind,
        /// Dotted field path rejected.
        field: String,
    },
    /// The live project does not contain an entity a non-`Added`
    /// change expects (stale target, already-removed clip/track, or
    /// a misidentified object).
    TargetMissing {
        /// The object reference.
        object: SemanticObjectRef,
    },
    /// An `Added` change's stable ID already exists in the live
    /// project — identity collision, never silently remapped.
    TargetConflict {
        /// The object reference.
        object: SemanticObjectRef,
    },
    /// `expected_target_fingerprint` does not match the recomputed
    /// fingerprint of the live target objects — the target drifted
    /// since the intent was captured. Also covers "same change
    /// applied twice": after the first apply the live state equals
    /// the after-state, so the before-fingerprint no longer matches.
    FingerprintMismatch {
        /// Fingerprint the intent declared.
        expected: ContentDigest,
        /// Fingerprint recomputed over live state.
        actual: ContentDigest,
    },
    /// The `ChangeSet` endpoints belong to a different project than
    /// the live one.
    ProjectMismatch {
        /// Live project ID.
        live: ProjectId,
        /// `ChangeSet` project ID.
        changeset: ProjectId,
    },
    /// `expected_project_revision` is stale.
    StaleRevision {
        /// Revision carried by the intent.
        expected: u64,
        /// Revision actually live.
        actual: u64,
    },
    /// An asset digest introduced by the selection is affirmatively
    /// missing from the blob probe.
    MissingAsset {
        /// The missing digest.
        digest: String,
    },
    /// An asset digest introduced by the selection could not be
    /// verified (`Unknown` probe result — fail closed).
    AssetUnknown {
        /// The unverifiable digest.
        digest: String,
    },
    /// A processor change would leave a plugin binding unresolved or
    /// unavailable.
    MissingPlugin {
        /// Processor stable ID or plugin descriptor.
        detail: String,
    },
    /// A routing connection references ports that do not exist in
    /// the post-apply state.
    InvalidPort {
        /// What was invalid.
        detail: String,
    },
    /// The single-writer rejected the planned mutation (validation
    /// or oracle failure). The project is unchanged.
    Rejected {
        /// The underlying controller error.
        source: Error,
    },
    /// The apply journal failed.
    Journal {
        /// Journal error detail.
        detail: String,
    },
    /// The change-set source port failed internally.
    Source {
        /// Port-reported detail.
        detail: String,
    },
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "apply cancelled"),
            Self::InvalidIntent { detail } => write!(f, "invalid apply intent: {detail}"),
            Self::Unapproved => write!(f, "apply intent carries no approval"),
            Self::ChangeSetNotFound { change_set_id } => {
                write!(f, "change set not found: {change_set_id}")
            }
            Self::ChangeSetMismatch { detail } => write!(f, "change set mismatch: {detail}"),
            Self::NotApplicable { detail } => write!(f, "change set not applicable: {detail}"),
            Self::UnknownChange { change_id } => write!(f, "unknown change id: {}", change_id.0),
            Self::Unsupported {
                kind,
                change,
                detail,
            } => {
                write!(f, "unsupported apply: {kind:?}/{change:?}: {detail}")
            }
            Self::UnsupportedField { kind, field } => {
                write!(f, "unsupported field on {kind:?}: {field}")
            }
            Self::TargetMissing { object } => {
                write!(f, "apply target missing: {}", object.stable_id)
            }
            Self::TargetConflict { object } => {
                write!(f, "apply target conflict: {}", object.stable_id)
            }
            Self::FingerprintMismatch { expected, actual } => write!(
                f,
                "target fingerprint mismatch: expected {expected}, live {actual}"
            ),
            Self::ProjectMismatch { live, changeset } => {
                write!(f, "project mismatch: live {live}, changeset {changeset}")
            }
            Self::StaleRevision { expected, actual } => {
                write!(f, "stale revision: expected {expected}, live {actual}")
            }
            Self::MissingAsset { digest } => write!(f, "missing asset: {digest}"),
            Self::AssetUnknown { digest } => write!(f, "asset availability unknown: {digest}"),
            Self::MissingPlugin { detail } => write!(f, "missing plugin: {detail}"),
            Self::InvalidPort { detail } => write!(f, "invalid port: {detail}"),
            Self::Rejected { source } => write!(f, "apply rejected: {source}"),
            Self::Journal { detail } => write!(f, "apply journal error: {detail}"),
            Self::Source { detail } => write!(f, "change-set source error: {detail}"),
        }
    }
}

impl std::error::Error for ApplyError {}

// ---------------------------------------------------------------------------
// ports
// ---------------------------------------------------------------------------

/// A resolved `ChangeSet` plus both endpoint snapshots — everything the
/// apply engine needs without trusting the `ChangeSet`'s digest-only
/// value references.
#[derive(Clone, Debug)]
pub struct ResolvedChangeSet {
    /// The canonical `ChangeSet`.
    pub changeset: ChangeSet,
    /// The before snapshot (read-only projection).
    pub before: ImmutableSnapshot,
    /// The after snapshot (read-only projection) — the entity source
    /// for transplanted values.
    pub after: ImmutableSnapshot,
}

/// Port resolving a `ChangeSet` digest to its `ChangeSet` and both
/// endpoint snapshots. Implementations must return snapshots whose
/// `identity.digest` values match `changeset.key` — the engine
/// re-verifies the binding itself.
pub trait ChangeSetSource {
    /// Resolves `change_set_id`; `None` = unknown.
    fn resolve(&self, change_set_id: &ContentDigest) -> Option<ResolvedChangeSet>;
}

/// Optional plugin-availability probe. When absent, only *explicitly
/// unresolved* bindings fail preflight; a `Resolved` binding is
/// accepted on the snapshot's own evidence (VCS-01 records opaque
/// state as `Unknown`, never `Missing`).
pub trait PluginAvailability {
    /// `true` when the plugin identified by `plugin` is installed
    /// and bindable on this host.
    fn plugin_available(&self, plugin: &PluginRef) -> bool;
}

/// Everything the engine needs beyond the controller.
pub struct ApplySources<'a> {
    /// `ChangeSet` + endpoint snapshot resolution.
    pub changesets: &'a dyn ChangeSetSource,
    /// Optional asset-availability probe for newly introduced
    /// digests. `None` → every probed digest is `Unknown` → fail
    /// closed when a probe is required.
    pub assets: Option<&'a dyn BlobProbe>,
    /// Optional plugin-availability probe.
    pub plugins: Option<&'a dyn PluginAvailability>,
}

// ---------------------------------------------------------------------------
// fingerprint
// ---------------------------------------------------------------------------

/// Canonical fingerprint over a set of [`SemanticObjectRef`]s against
/// `project` — the value callers place in
/// [`ApplyChangeIntent::expected_target_fingerprint`].
///
/// Covers each object's full entity serialization (singleton views
/// for `Project`/`RoutingModel`, the `MidiNote` value for MIDI notes)
/// in deterministic `(kind, stable_id)` order; absent entities encode
/// as `null` so `Added` targets fingerprint consistently on both
/// sides of an apply. Returns [`ApplyError::Unsupported`] for kinds
/// outside the apply surface — fingerprinting an unsupported target
/// fails before any mutation.
///
/// # Errors
/// [`ApplyError::Unsupported`]/[`ApplyError::Source`] on
/// serialization failure.
pub fn target_fingerprint(
    project: &Project,
    objects: &[SemanticObjectRef],
) -> Result<ContentDigest, ApplyError> {
    let set: BTreeSet<SemanticObjectRef> = objects.iter().cloned().collect();
    entity::fingerprint(project, &set)
}

// ---------------------------------------------------------------------------
// engine
// ---------------------------------------------------------------------------

fn intent_digest(intent: &ApplyChangeIntent) -> Result<ContentDigest, ApplyError> {
    let bytes = serde_json::to_vec(intent).map_err(|e| ApplyError::InvalidIntent {
        detail: e.to_string(),
    })?;
    Ok(ContentDigest::sha256(&bytes))
}

fn apply_key(digest: &ContentDigest) -> String {
    format!("apply-{}", digest.to_string().replace(':', ""))
}

/// Applies `intent`'s selected changes to `controller`'s live project.
///
/// Ordering: journal replay check → resolve/verify `ChangeSet` → plan
/// typed writes → fingerprint CAS → asset/plugin/port preflight →
/// single [`ProjectController::apply`] → journal completion. Every
/// error path leaves the project unchanged and records a `Failed`
/// journal entry (when a journal is supplied and the attempt reached
/// the mutation stage).
///
/// # Errors
/// Any [`ApplyError`]; the project is guaranteed unchanged.
pub fn apply_selected(
    controller: &mut ProjectController,
    intent: &ApplyChangeIntent,
    sources: &ApplySources<'_>,
    journal: &mut dyn ApplyJournal,
    cancel: &SnapshotCancel,
) -> Result<ApplyReceipt, ApplyError> {
    if cancel.is_cancelled() {
        return Err(ApplyError::Cancelled);
    }
    validate_intent(intent)?;

    let digest = intent_digest(intent)?;
    let key = apply_key(&digest);

    // Inspect the existing record BEFORE re-arming Pending: `Applied`
    // replays the recorded receipt; a pre-existing `Pending` marks a
    // possible committed-but-unreceipted earlier attempt (ACK gap /
    // crash) that `apply_inner` reconciles against live state.
    let prior_state = journal
        .load(&key)
        .map_err(|e| ApplyError::Journal {
            detail: e.to_string(),
        })?
        .map(|r| r.state);
    if let Some(ApplyJournalState::Applied(receipt)) = prior_state {
        let mut replayed = receipt;
        replayed.outcome = ApplyOutcome::Replayed;
        return Ok(replayed);
    }
    let prior_pending = matches!(prior_state, Some(ApplyJournalState::Pending));

    journal
        .store(ApplyJournalRecord {
            key: key.clone(),
            intent_digest: digest,
            state: ApplyJournalState::Pending,
        })
        .map_err(|e| ApplyError::Journal {
            detail: e.to_string(),
        })?;

    match apply_inner(controller, intent, sources, cancel, prior_pending) {
        Ok((receipt, applied_ids)) => {
            let mut receipt = receipt;
            receipt.applied_change_ids = applied_ids;
            receipt.apply_key.clone_from(&key);
            receipt.intent_digest = digest;
            journal
                .store(ApplyJournalRecord {
                    key,
                    intent_digest: digest,
                    state: ApplyJournalState::Applied(receipt.clone()),
                })
                .map_err(|e| ApplyError::Journal {
                    detail: e.to_string(),
                })?;
            Ok(receipt)
        }
        Err(error) => {
            let _ = journal.store(ApplyJournalRecord {
                key,
                intent_digest: digest,
                state: ApplyJournalState::Failed {
                    detail: error.to_string(),
                },
            });
            Err(error)
        }
    }
}

fn validate_intent(intent: &ApplyChangeIntent) -> Result<(), ApplyError> {
    if intent.approval.ticket.trim().is_empty() {
        return Err(ApplyError::Unapproved);
    }
    if intent.selected_change_ids.is_empty() {
        return Err(ApplyError::InvalidIntent {
            detail: "empty change selection".into(),
        });
    }
    let mut seen = BTreeSet::new();
    for id in &intent.selected_change_ids {
        if !seen.insert(id) {
            return Err(ApplyError::InvalidIntent {
                detail: format!("duplicate change id {}", id.0),
            });
        }
    }
    Ok(())
}

fn apply_inner(
    controller: &mut ProjectController,
    intent: &ApplyChangeIntent,
    sources: &ApplySources<'_>,
    cancel: &SnapshotCancel,
    prior_pending: bool,
) -> Result<(ApplyReceipt, Vec<ChangeId>), ApplyError> {
    // -- resolve + verify -------------------------------------------------
    let resolved = resolve_verified(intent, sources, controller.live().project_id)?;
    let changeset = &resolved.changeset;
    let after = &resolved.after;

    // -- fingerprint CAS ---------------------------------------------------
    // The fingerprinted set is derived from the *declared selection*
    // (not the plan) so the crash/ACK-gap reconcile below works even
    // when the live state already equals the after-state — in which
    // case planning would fail on `Added`-now-present conflicts before
    // the reconcile could run.
    let live = controller.live();
    let mut objects: BTreeSet<SemanticObjectRef> = BTreeSet::new();
    for change in &changeset.changes {
        if intent.selected_change_ids.contains(&change.id) {
            objects.insert(change.object.clone());
        }
    }
    objects.insert(intent.target_object.clone());
    let live_fp = entity::fingerprint(live, &objects)?;
    let after_fp = entity::fingerprint(after.projection.project(), &objects)?;

    // Crash/ACK-gap reconcile (Pending journal only): live already at
    // the after-state means the earlier attempt committed but its
    // receipt was lost — report Replayed, never re-apply.
    if prior_pending && live_fp == after_fp {
        let applied_ids: Vec<ChangeId> = changeset
            .changes
            .iter()
            .filter(|c| intent.selected_change_ids.contains(&c.id))
            .map(|c| c.id.clone())
            .collect();
        let revision = controller.live_revision();
        let receipt = apply_receipt(
            intent,
            applied_ids.clone(),
            revision,
            revision,
            ApplyOutcome::Replayed,
        );
        return Ok((receipt, applied_ids));
    }

    if live_fp != intent.expected_target_fingerprint {
        return Err(ApplyError::FingerprintMismatch {
            expected: intent.expected_target_fingerprint,
            actual: live_fp,
        });
    }

    // -- selection → plan --------------------------------------------------
    let mut planned = plan::plan_changes(
        live,
        after.projection.project(),
        changeset,
        &intent.selected_change_ids,
        cancel,
    )?;

    // -- preflight: assets / plugins / ports -------------------------------
    plan::preflight_assets(live, &planned, sources.assets, after.projection.project())?;
    plan::preflight_plugins(live, &planned, sources.plugins)?;
    plan::preflight_ports(live, after.projection.project(), &planned)?;

    if cancel.is_cancelled() {
        return Err(ApplyError::Cancelled);
    }

    // -- single-writer commit ----------------------------------------------
    let before_revision = controller.live_revision();
    let applied_ids: Vec<ChangeId> = planned.iter().map(|w| w.change_id.clone()).collect();
    let op = plan::ApplySelectedOp {
        writes: std::mem::take(&mut planned),
        after: after.projection.project().clone(),
    };
    let command = Command {
        expected_revision: intent.expected_project_revision,
        op,
    };
    let after_revision = controller.apply(command).map_err(|e| match e {
        Error::StaleRevision { expected, actual } => ApplyError::StaleRevision { expected, actual },
        other => ApplyError::Rejected { source: other },
    })?;

    let receipt = apply_receipt(
        intent,
        applied_ids.clone(),
        before_revision,
        after_revision,
        ApplyOutcome::Applied,
    );
    Ok((receipt, applied_ids))
}

/// Resolve the pinned `ChangeSet` and verify every read-only binding
/// before any live-state or plan computation.
fn resolve_verified(
    intent: &ApplyChangeIntent,
    sources: &ApplySources<'_>,
    live_project_id: ProjectId,
) -> Result<ResolvedChangeSet, ApplyError> {
    let resolved =
        sources
            .changesets
            .resolve(&intent.change_set_id)
            .ok_or(ApplyError::ChangeSetNotFound {
                change_set_id: intent.change_set_id,
            })?;
    let changeset = &resolved.changeset;
    let before = &resolved.before;
    let after = &resolved.after;

    if before.identity.digest != changeset.key.before_digest
        || after.identity.digest != changeset.key.after_digest
        || changeset.comparison.before.digest != before.identity.digest
        || changeset.comparison.after.digest != after.identity.digest
    {
        return Err(ApplyError::ChangeSetMismatch {
            detail: "endpoint digests do not bind to the resolved snapshots".into(),
        });
    }
    if intent.source_snapshot_id != after.identity.digest {
        return Err(ApplyError::ChangeSetMismatch {
            detail: "source_snapshot_id is not the after snapshot identity".into(),
        });
    }
    let project_id = before.identity.project_id;
    if after.identity.project_id != project_id {
        return Err(ApplyError::ProjectMismatch {
            live: project_id,
            changeset: after.identity.project_id,
        });
    }
    if live_project_id != project_id {
        return Err(ApplyError::ProjectMismatch {
            live: live_project_id,
            changeset: project_id,
        });
    }
    if changeset.completeness != ComparisonCompleteness::Complete
        || !matches!(changeset.overall, ComparisonEquality::Different)
    {
        return Err(ApplyError::NotApplicable {
            detail: format!(
                "completeness={:?} overall={:?}",
                changeset.completeness,
                match &changeset.overall {
                    ComparisonEquality::Equal => "equal",
                    ComparisonEquality::Different => "different",
                    ComparisonEquality::Unknown(_) => "unknown",
                }
            ),
        });
    }
    if before.validation != SnapshotValidation::Valid
        || after.validation != SnapshotValidation::Valid
        || before.completeness != SnapshotCompleteness::Complete
        || after.completeness != SnapshotCompleteness::Complete
    {
        return Err(ApplyError::NotApplicable {
            detail: format!(
                "endpoint validation/completeness not fully verified: before={:?}/{:?} after={:?}/{:?}",
                before.validation, before.completeness, after.validation, after.completeness
            ),
        });
    }
    Ok(resolved)
}

/// Build the receipt — `apply_key`/`intent_digest` stay empty here;
/// `apply_selected` stamps them after journaling.
fn apply_receipt(
    intent: &ApplyChangeIntent,
    applied_change_ids: Vec<ChangeId>,
    before_revision: ProjectRevision,
    after_revision: ProjectRevision,
    outcome: ApplyOutcome,
) -> ApplyReceipt {
    ApplyReceipt {
        apply_key: String::new(),
        intent_digest: ContentDigest::sha256(&[]),
        change_set_id: intent.change_set_id,
        source_snapshot_id: intent.source_snapshot_id,
        applied_change_ids,
        before_revision,
        after_revision,
        outcome,
        effects: ApplyEffects {
            live_applied: true,
            single_undo_step: true,
            engine_applied: false,
            untouched: untouched_effects(),
        },
    }
}
