//! PPQN selection read model — the view-owned select → preview →
//! confirm/cancel lifecycle for the TIM-01 configurable-PPQN contract
//! (synvelis#125 UI slice).
//!
//! This module is a *projection and gesture-state* layer only: it
//! never converts musical ticks itself. The authoritative conversion
//! policy, site enumeration, and feasibility verdicts stay in
//! `synvelis-project`'s [`preview_ppqn`] /
//! `ProjectOp::SetProjectPpqn`; this model consumes their results and
//! decides which `syn.project.*` op a view should mint next. There is
//! no second time model here — beats appear only as display text
//! derived from the project's declared PPQN.
//!
//! Snap/Grid policy decided by this slice: [`SnapUnit::Explicit`]
//! stores a fixed quantum in the gesture domain's declared base
//! units (project ticks) and is **preserved verbatim** across a
//! project PPQN change — only its musical (beat) spacing changes,
//! which [`snap_unit_ppqn_effect`] quantifies for preview surfaces.
//! Rationale is recorded in
//! `docs/designs/tim-01-ppqn-ui-design.md`.
//!
//! Mounted through `state` while `lib.rs` is leased to the SV-107
//! worker — the coordinator relocates the declaration to a top-level
//! `pub mod ppqn_ui;` at integration.

use std::fmt;

use ruviform_media_core::Rounding;
use serde_json::{Value, json};
use synvelis_project::{
    PPQN_MAX, PPQN_MIN, PpqnChangePreview, PpqnConversionPolicy, PpqnPreviewSite, ppqn_in_range,
};

use crate::snap::{DEFAULT_SNAP_QUANTUM_TICKS, SnapUnit, snap_unit_ppqn_effect};

/// Recommended PPQN values offered by the selection surface — the
/// conventional musical grids plus the decimal-friendly 1000. The
/// full selectable range stays `1..=32767` (the SMF 15-bit PPQ
/// bound); these are shortcuts, not a restriction.
pub const RECOMMENDED_PPQN: &[u32] = &[480, 960, 1000, 1920];

/// Informational SMF-export note carried by preview projections —
/// export division remains an explicit option inside the same
/// `1..=32767` SMF PPQ range; this slice changes no export code.
pub const SMF_EXPORT_NOTE: &str =
    "SMF export division stays an explicit option inside the same 1..=32767 SMF PPQ range";

/// Cap on per-site rows a preview projection carries — the report
/// stays bounded on projects with many musical-tick sites; the total
/// counts remain exact.
pub const PREVIEW_SITE_CAP: usize = 32;

/// Wire name of a [`PpqnConversionPolicy`] — the `policy` param of
/// `syn.project.preview_ppqn`/`syn.project.set_ppqn`.
#[must_use]
pub fn ppqn_policy_name(policy: PpqnConversionPolicy) -> &'static str {
    match policy {
        PpqnConversionPolicy::Exact => "exact",
        PpqnConversionPolicy::Round(_) => "round",
    }
}

/// Wire name of a ruviform [`Rounding`] — the `rounding` param,
/// required iff `policy` is `"round"`.
#[must_use]
pub fn ppqn_rounding_name(rounding: Rounding) -> &'static str {
    match rounding {
        Rounding::Floor => "floor",
        Rounding::Ceil => "ceil",
        Rounding::TowardZero => "toward_zero",
        Rounding::AwayFromZero => "away_from_zero",
        Rounding::NearestEven => "nearest_even",
        _ => "unknown",
    }
}

/// `(policy, rounding)` wire pair for `params` — `rounding` is `None`
/// for [`PpqnConversionPolicy::Exact`] and carries the wire name for
/// `Round` (an unrecognised [`Rounding`] serializes as `"unknown"`).
#[must_use]
pub fn ppqn_policy_wire(policy: PpqnConversionPolicy) -> (&'static str, Option<&'static str>) {
    match policy {
        PpqnConversionPolicy::Exact => ("exact", None),
        PpqnConversionPolicy::Round(rounding) => ("round", Some(ppqn_rounding_name(rounding))),
    }
}

/// Lifecycle phase of the PPQN selection gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpqnPhase {
    /// No pending selection — the surface shows the live PPQN only.
    Idle,
    /// A candidate was selected; no `preview_ppqn` result recorded
    /// yet — confirm is not yet possible.
    Selected,
    /// A `preview_ppqn` result is recorded for the pending
    /// candidate — confirm is possible iff `preview.applies`.
    Previewed,
}

/// A read-model failure — range rejection, missing pending state, or
/// a preview that does not match the pending candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PpqnUiError {
    /// `select` was called with a value outside `1..=32767`.
    OutOfRange {
        /// The rejected value.
        value: u32,
    },
    /// The gesture requires a pending selection that does not exist.
    NoPending,
    /// The recorded preview belongs to a different candidate or a
    /// different live PPQN than the pending selection expects.
    PreviewMismatch {
        /// The pending candidate's target PPQN.
        pending: u32,
        /// The PPQN the preview was computed for.
        previewed: u32,
    },
}

impl fmt::Display for PpqnUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange { value } => {
                write!(
                    f,
                    "ppqn {value} outside selectable range {PPQN_MIN}..={PPQN_MAX}"
                )
            }
            Self::NoPending => write!(f, "no pending ppqn selection"),
            Self::PreviewMismatch { pending, previewed } => write!(
                f,
                "preview is for ppqn {previewed} but pending selection targets {pending}"
            ),
        }
    }
}

impl std::error::Error for PpqnUiError {}

/// Display projection of one [`PpqnPreviewSite`] — carried verbatim
/// (kind/instance/part/tick/projected) so the surface reports exactly
/// what the project layer flagged, capped at [`PREVIEW_SITE_CAP`].
pub type PpqnPreviewSiteView = PpqnPreviewSite;

/// The Snap/Grid effect a PPQN change has on the shipped default
/// quantum — explicit units keep their tick count; only the musical
/// spacing moves. Per-view explicit quanta follow the identical
/// preservation rule via [`snap_unit_ppqn_effect`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapGridEffect {
    /// The default quantum's fixed tick count.
    pub quantum_ticks: i64,
    /// Musical spacing before the change, in beats.
    pub beats_before: f64,
    /// Musical spacing after the change, in beats.
    pub beats_after: f64,
}

impl SnapGridEffect {
    /// One-line display text, e.g. `"960 ticks: 1.000 → 2.000 beats"`.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "{} ticks: {:.3} → {:.3} beats",
            self.quantum_ticks, self.beats_before, self.beats_after
        )
    }
}

/// View-facing projection of a [`PpqnChangePreview`] — the
/// "変更前 Preview" the Issue contract requires before any mutation.
/// Carries feasibility verdicts, bounded site evidence, the
/// project revision the preview was computed against (staleness
/// guard), and the informational Snap/Grid + SMF-export effects.
#[derive(Debug, Clone, PartialEq)]
pub struct PpqnPreviewView {
    /// Live PPQN the preview ran against.
    pub old_ppqn: u32,
    /// Requested target PPQN.
    pub new_ppqn: u32,
    /// Wire `policy` name (`"exact"`/`"round"`).
    pub policy: &'static str,
    /// Wire `rounding` name — `None` for `exact`.
    pub rounding: Option<&'static str>,
    /// Persisted musical-tick sites the change re-projects.
    pub site_count: usize,
    /// Total inexact sites (list below is capped).
    pub inexact_count: usize,
    /// Inexact sites, capped at [`PREVIEW_SITE_CAP`].
    pub inexact_sites: Vec<PpqnPreviewSiteView>,
    /// Total overflow sites (any overflow fails every policy).
    pub overflow_count: usize,
    /// Overflow sites, capped at [`PREVIEW_SITE_CAP`].
    pub overflow_sites: Vec<PpqnPreviewSiteView>,
    /// `SetProjectPpqn` under `Exact` would succeed.
    pub exact_ok: bool,
    /// A `Round` policy has no overflow to reject.
    pub round_ok: bool,
    /// The requested policy's dry-run succeeded — the authoritative
    /// "this confirm would apply" flag.
    pub applies: bool,
    /// The rejection the requested-policy dry-run produced, if any.
    pub rejection: Option<String>,
    /// Project revision the preview was computed against — a
    /// confirm minted from a stale preview fails the
    /// `expected_revision` CAS honestly.
    pub project_revision: u64,
    /// Snap/Grid effect on the default quantum.
    pub snap_grid: SnapGridEffect,
    /// SMF-export informational note.
    pub smf_export: &'static str,
}

impl PpqnPreviewView {
    /// Projects a [`PpqnChangePreview`] into display state.
    #[must_use]
    pub fn from_preview(preview: &PpqnChangePreview, project_revision: u64) -> Self {
        let (policy, rounding) = ppqn_policy_wire(preview.policy);
        let effect = snap_unit_ppqn_effect(
            SnapUnit::Explicit(DEFAULT_SNAP_QUANTUM_TICKS),
            preview.old_ppqn,
            preview.new_ppqn,
        );
        Self {
            old_ppqn: preview.old_ppqn,
            new_ppqn: preview.new_ppqn,
            policy,
            rounding,
            site_count: preview.site_count,
            inexact_count: preview.inexact.len(),
            inexact_sites: preview
                .inexact
                .iter()
                .take(PREVIEW_SITE_CAP)
                .cloned()
                .collect(),
            overflow_count: preview.overflow_sites.len(),
            overflow_sites: preview
                .overflow_sites
                .iter()
                .take(PREVIEW_SITE_CAP)
                .cloned()
                .collect(),
            exact_ok: preview.exact_ok,
            round_ok: preview.round_ok,
            applies: preview.applies,
            rejection: preview.rejection.clone(),
            project_revision,
            snap_grid: SnapGridEffect {
                quantum_ticks: effect.quantum_ticks.unwrap_or(DEFAULT_SNAP_QUANTUM_TICKS),
                beats_before: effect.beats_before.unwrap_or(0.0),
                beats_after: effect.beats_after.unwrap_or(0.0),
            },
            smf_export: SMF_EXPORT_NOTE,
        }
    }

    /// One-line feasibility summary for compact display.
    #[must_use]
    pub fn summary(&self) -> String {
        let verdict = if self.applies { "applies" } else { "rejected" };
        format!(
            "{} → {} ppqn ({}/{}) — {verdict} — {} sites, {} inexact, {} overflow",
            self.old_ppqn,
            self.new_ppqn,
            self.policy,
            self.rounding.unwrap_or("—"),
            self.site_count,
            self.inexact_count,
            self.overflow_count,
        )
    }
}

/// The pending selection — candidate PPQN + conversion policy, with
/// the recorded preview once [`PpqnUi::record_preview`] lands.
#[derive(Debug, Clone, PartialEq)]
pub struct PpqnPending {
    /// Candidate target PPQN (already range-validated by `select`).
    pub new_ppqn: u32,
    /// Requested conversion policy.
    pub policy: PpqnConversionPolicy,
    /// Project revision the recorded preview ran against — `None`
    /// until a preview lands.
    pub project_revision: Option<u64>,
    /// Recorded preview projection — `None` in [`PpqnPhase::Selected`].
    pub preview: Option<PpqnPreviewView>,
}

/// Wire-ready parameters for `syn.project.set_ppqn` — produced by
/// [`PpqnUi::confirm_request`] only when the pending preview
/// applies, so a confirm can never silently quantize or skip the
/// preview step.
#[derive(Debug, Clone, PartialEq)]
pub struct PpqnConfirm {
    /// PPQN the preview ran against (CAS input).
    pub old_ppqn: u32,
    /// Candidate target PPQN.
    pub new_ppqn: u32,
    /// Wire `policy` name.
    pub policy: &'static str,
    /// Wire `rounding` name — `None` for `exact`.
    pub rounding: Option<&'static str>,
    /// Revision the preview was computed against — the
    /// `expected_revision` CAS that rejects a stale confirm.
    pub expected_revision: Option<u64>,
}

impl PpqnConfirm {
    /// The `params` object for `syn.project.set_ppqn`.
    #[must_use]
    pub fn params(&self) -> Value {
        json!({
            "old_ppqn": self.old_ppqn,
            "new_ppqn": self.new_ppqn,
            "policy": self.policy,
            "rounding": self.rounding,
            "expected_revision": self.expected_revision,
        })
    }
}

/// The view-owned PPQN selection gesture — select → preview →
/// confirm/cancel over the live project PPQN. Pending state is
/// session-scoped view state: it is never persisted and never
/// mutates the project; only the minted `syn.project.*` ops do.
#[derive(Debug, Clone, PartialEq)]
pub struct PpqnUi {
    current_ppqn: u32,
    pending: Option<PpqnPending>,
}

impl PpqnUi {
    /// A read model tracking `current_ppqn` as the live project PPQN.
    #[must_use]
    pub fn new(current_ppqn: u32) -> Self {
        Self {
            current_ppqn,
            pending: None,
        }
    }

    /// The live project PPQN last observed.
    #[must_use]
    pub const fn current_ppqn(&self) -> u32 {
        self.current_ppqn
    }

    /// The pending selection, if any.
    #[must_use]
    pub fn pending(&self) -> Option<&PpqnPending> {
        self.pending.as_ref()
    }

    /// Current lifecycle phase.
    #[must_use]
    pub fn phase(&self) -> PpqnPhase {
        match &self.pending {
            None => PpqnPhase::Idle,
            Some(p) if p.preview.is_some() => PpqnPhase::Previewed,
            Some(_) => PpqnPhase::Selected,
        }
    }

    /// Selects `new_ppqn` as the pending candidate under `policy` —
    /// the gesture's first step. Re-selecting replaces the pending
    /// candidate (the previous preview, if any, is discarded with
    /// it). Out-of-range values are rejected before any state
    /// changes.
    ///
    /// # Errors
    /// [`PpqnUiError::OutOfRange`] when `new_ppqn` is outside
    /// `1..=32767`.
    pub fn select(
        &mut self,
        new_ppqn: u32,
        policy: PpqnConversionPolicy,
    ) -> Result<(), PpqnUiError> {
        if !ppqn_in_range(new_ppqn) {
            return Err(PpqnUiError::OutOfRange { value: new_ppqn });
        }
        self.pending = Some(PpqnPending {
            new_ppqn,
            policy,
            project_revision: None,
            preview: None,
        });
        Ok(())
    }

    /// The `params` for `syn.project.preview_ppqn` on the pending
    /// candidate — `None` when nothing is selected. The view mints
    /// the op through its binding and feeds the result back via
    /// [`Self::record_preview`].
    #[must_use]
    pub fn preview_params(&self) -> Option<Value> {
        let pending = self.pending.as_ref()?;
        let (policy, rounding) = ppqn_policy_wire(pending.policy);
        Some(json!({
            "new_ppqn": pending.new_ppqn,
            "policy": policy,
            "rounding": rounding,
        }))
    }

    /// Records the `preview_ppqn` result for the pending candidate —
    /// the gesture's second step. The preview must target the same
    /// `new_ppqn` the pending selection carries and must have run
    /// against the current live PPQN, so a stale or mismatched
    /// report can never arm confirm.
    ///
    /// # Errors
    /// [`PpqnUiError::NoPending`] without a selection;
    /// [`PpqnUiError::PreviewMismatch`] on a foreign preview.
    pub fn record_preview(
        &mut self,
        preview: &PpqnChangePreview,
        project_revision: u64,
    ) -> Result<(), PpqnUiError> {
        let pending = self.pending.as_mut().ok_or(PpqnUiError::NoPending)?;
        if preview.new_ppqn != pending.new_ppqn || preview.old_ppqn != self.current_ppqn {
            return Err(PpqnUiError::PreviewMismatch {
                pending: pending.new_ppqn,
                previewed: preview.new_ppqn,
            });
        }
        pending.project_revision = Some(project_revision);
        pending.preview = Some(PpqnPreviewView::from_preview(preview, project_revision));
        Ok(())
    }

    /// Wire-ready `syn.project.set_ppqn` params for the pending
    /// gesture — `Some` only in [`PpqnPhase::Previewed`] with
    /// `preview.applies`, so confirm always carries the exact
    /// candidate, policy, and the preview-time revision CAS. A
    /// preview that reports rejection yields `None`: the surface
    /// shows the rejection instead of offering a doomed confirm.
    #[must_use]
    pub fn confirm_request(&self) -> Option<PpqnConfirm> {
        let pending = self.pending.as_ref()?;
        let preview = pending.preview.as_ref()?;
        if !preview.applies {
            return None;
        }
        let (policy, rounding) = ppqn_policy_wire(pending.policy);
        Some(PpqnConfirm {
            old_ppqn: self.current_ppqn,
            new_ppqn: pending.new_ppqn,
            policy,
            rounding,
            expected_revision: pending.project_revision,
        })
    }

    /// Cancels the pending gesture — non-destructive by construction:
    /// nothing was ever applied, so cancel only drops view-local
    /// state. Returns `true` when a pending selection was discarded.
    pub fn cancel(&mut self) -> bool {
        self.pending.take().is_some()
    }

    /// Completes the gesture after `syn.project.set_ppqn` succeeded —
    /// clears the pending selection and adopts `new_ppqn` as the live
    /// value. Pair with [`Self::observe_current`] when the change
    /// came from outside this gesture (undo, open, another view).
    pub fn committed(&mut self, new_ppqn: u32) {
        self.pending = None;
        self.current_ppqn = new_ppqn;
    }

    /// Re-syncs the live PPQN from the project (undo/redo, project
    /// open/new, another view's confirm). A changed value invalidates
    /// any pending selection — its preview ran against the old grid.
    /// Returns `true` when the observed value differed.
    pub fn observe_current(&mut self, ppqn: u32) -> bool {
        if ppqn == self.current_ppqn {
            return false;
        }
        self.current_ppqn = ppqn;
        self.pending = None;
        true
    }
}
