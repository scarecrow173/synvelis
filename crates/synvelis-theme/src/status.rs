//! Automation status token vocabulary (AUTX-07 / synvelis#266) plus
//! the Script Editor diagnostic/analyzer vocabulary (SV-SCRIPT-04 /
//! synvelis#274).
//!
//! The Action Palette, Macro Browser, Workflow Editor, and Run Monitor
//! surfaces render authority states — capability availability, grant
//! verdicts, and run/job phases — that need dedicated product colors
//! distinct from the generic `ok`/`warn`/`err` ramp. The Script Editor
//! adds diagnostic severities (`information`/`warning`/`error`) and
//! analyzer states (`unsupported`/`stale`/`dirty`) whose semantics
//! differ from run status: `unsupported` is a technical capability
//! gap (not "unavailable"), and `stale`/`dirty` mark absent or
//! outdated information — neither is a warning or an error. This
//! module is the single source of truth for the DTCG token *paths*
//! those states map to; the resolved values live in the product
//! theme documents (`assets/themes/*.dtcg.json`, owned outside this
//! crate).
//!
//! Vocabulary contract:
//!
//! - Every automation status has exactly one token path under
//!   `color.status-*` — never reuse `ok`/`warn`/`err` for states whose
//!   semantics differ (e.g. `cancel-unconfirmed` is indeterminate, not
//!   a warning).
//! - Token paths are kebab-case and stable: they are part of the
//!   driver-facing contract, not display text.
//! - The GUI never *evaluates* these states — it only names them;
//!   [`crate::status::StatusTone::of_label`] maps the workspace
//!   projection's stable labels onto tones without importing any
//!   gateway/workspace type (this crate stays a leaf).

/// DTCG token path for one automation status tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatusTone {
    /// Capability/run is available and permitted.
    Ok,
    /// Capability is `Disabled`/`Planned`/`Retired`, or a grant is
    /// missing — visible but never executable.
    Unavailable,
    /// Grant verdict is `Unknown` — absent information is never
    /// rendered as granted.
    Unknown,
    /// Admitted but not yet started (`pending`).
    Pending,
    /// Actively executing (`running`).
    Running,
    /// Provider-confirmed terminal success (`completed`).
    Completed,
    /// Provider-side failure (`failed`).
    Failed,
    /// Cancel requested, confirmation pending — distinct from both
    /// running and cancelled.
    CancelRequested,
    /// Cancel requested but never confirmed — the outcome is
    /// indeterminate.
    CancelUnconfirmed,
    /// Provider-confirmed cancellation (`cancelled`).
    Cancelled,
    /// Outcome cannot be determined; reconciliation required —
    /// never collapsed into success/failure/cancelled.
    Indeterminate,
    /// Analyzer registered but does not advertise the wanted
    /// capability (F-13 `Unsupported`) — a technical gap, distinct
    /// from `Unavailable` (no analyzer at all, F-18).
    Unsupported,
    /// An analyzer result was rejected as stale — revision or
    /// definition-digest drift (F-14). Outdated information, not a
    /// warning and not an error.
    Stale,
    /// Document or definitions changed since the last accepted
    /// analysis — a re-run is needed.
    Dirty,
    /// Diagnostic severity `Information`.
    DiagnosticInformation,
    /// Diagnostic severity `Warning`.
    DiagnosticWarning,
    /// Diagnostic severity `Error`.
    DiagnosticError,
}

impl StatusTone {
    /// All tones, in display order.
    pub const ALL: [Self; 17] = [
        Self::Ok,
        Self::Unavailable,
        Self::Unknown,
        Self::Pending,
        Self::Running,
        Self::Completed,
        Self::Failed,
        Self::CancelRequested,
        Self::CancelUnconfirmed,
        Self::Cancelled,
        Self::Indeterminate,
        Self::Unsupported,
        Self::Stale,
        Self::Dirty,
        Self::DiagnosticInformation,
        Self::DiagnosticWarning,
        Self::DiagnosticError,
    ];

    /// The DTCG token path (relative to the theme's `color` group,
    /// e.g. `status-ok` resolves as `color.status-ok`).
    #[must_use]
    pub const fn token_path(self) -> &'static str {
        match self {
            Self::Ok => "status-ok",
            Self::Unavailable => "status-unavailable",
            Self::Unknown => "status-unknown",
            Self::Pending => "status-pending",
            Self::Running => "status-running",
            Self::Completed => "status-completed",
            Self::Failed => "status-failed",
            Self::CancelRequested => "status-cancel-requested",
            Self::CancelUnconfirmed => "status-cancel-unconfirmed",
            Self::Cancelled => "status-cancelled",
            Self::Indeterminate => "status-indeterminate",
            Self::Unsupported => "status-unsupported",
            Self::Stale => "status-stale",
            Self::Dirty => "status-dirty",
            Self::DiagnosticInformation => "status-diagnostic-information",
            Self::DiagnosticWarning => "status-diagnostic-warning",
            Self::DiagnosticError => "status-diagnostic-error",
        }
    }

    /// Maps a stable status *label* (the vocabulary used by the
    /// workspace projections — `MonitorStatus::label`,
    /// `Availability::name`, `GrantState::label` heads, and the RCP-06
    /// five-axis render labels) onto a tone. Unknown labels map to
    /// [`Self::Unknown`] — fail-closed, never guessed.
    ///
    /// RCP-06 note: the five presentation axes carry meaning in their
    /// *labels* (`edit-possible`, `render-stale`, `quality-preview`,
    /// …) — color is only a secondary cue. Labels map onto the
    /// existing tone ramp; no render-specific token paths are added
    /// because meaning must survive without color (RCP-UX-01).
    #[must_use]
    pub fn of_label(label: &str) -> Self {
        match label {
            // Permitted/available — automation grants plus the RCP-06
            // editability (`edit-possible`/`live`) and exact-quality
            // labels.
            "available"
            | "granted"
            | "satisfied"
            | "edit-possible"
            | "live"
            | "ready"
            | "quality-exact-artifact"
            | "quality-verified-tolerance" => Self::Ok,
            // Confirmed terminal success — `completed`, exact render
            // validity, published runtime/job states.
            "completed" | "render-valid" | "runtime-published" | "published" => Self::Completed,
            // Disabled/unavailable — capability gaps, blocked axes,
            // missing/unsupported render coverage, preview-unavailable
            // quality.
            "disabled"
            | "planned"
            | "retired"
            | "unavailable"
            | "missing"
            | "edit-blocked"
            | "blocked"
            | "render-missing"
            | "render-unsupported"
            | "render-denied"
            | "quality-unavailable" => Self::Unavailable,
            // Admitted/pending — queued states, preview-class render
            // validity and quality, configured/prepared runtime.
            "pending" | "render-preview" | "render-unverified" | "quality-preview"
            | "runtime-configured" | "runtime-prepared" | "resolve" | "queue" | "preparing" => {
                Self::Pending
            }
            // Actively executing — running automation/render/runtime.
            "running" | "runtime-active" | "rendering" | "verifying" => Self::Running,
            "failed" | "render-corrupt" | "rejected" => Self::Failed,
            "cancel-requested" => Self::CancelRequested,
            "cancel-unconfirmed" => Self::CancelUnconfirmed,
            "cancelled" => Self::Cancelled,
            // Indeterminate — stale render, last-good mismatch, partial
            // publication; never collapsed into success/failure.
            "indeterminate" | "render-stale" | "runtime-last-good-mismatch" | "partial" => {
                Self::Indeterminate
            }
            // Script Editor analyzer vocabulary (SV-SCRIPT-04):
            // `unsupported` is a capability gap, `stale`/`dirty` mark
            // outdated analysis — none reuse ok/warn/err semantics.
            "unsupported" => Self::Unsupported,
            "stale" => Self::Stale,
            "dirty" => Self::Dirty,
            // Diagnostic severities — dedicated tones, never the
            // generic ok/warn/err ramp.
            "information" | "diagnostic-information" | "severity-information" => {
                Self::DiagnosticInformation
            }
            "warning" | "diagnostic-warning" | "severity-warning" => Self::DiagnosticWarning,
            "error" | "diagnostic-error" | "severity-error" => Self::DiagnosticError,
            // `render-unknown`, `idle`, and every unrecognized label
            // fail closed to `Unknown` — never guessed.
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StatusTone;

    #[test]
    fn token_paths_are_unique_kebab_case() {
        let mut paths: Vec<&str> = StatusTone::ALL.iter().map(|t| t.token_path()).collect();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), StatusTone::ALL.len());
        for path in paths {
            assert!(path.starts_with("status-"), "{path}");
            assert!(
                path.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{path}"
            );
        }
    }

    #[test]
    fn every_monitor_label_maps_to_a_distinct_tone() {
        // The Run Monitor vocabulary must never collapse: pending,
        // cancel-requested, cancel-unconfirmed, cancelled, and
        // indeterminate are five different tones.
        let cases = [
            ("pending", StatusTone::Pending),
            ("running", StatusTone::Running),
            ("completed", StatusTone::Completed),
            ("failed", StatusTone::Failed),
            ("cancel-requested", StatusTone::CancelRequested),
            ("cancel-unconfirmed", StatusTone::CancelUnconfirmed),
            ("cancelled", StatusTone::Cancelled),
            ("indeterminate", StatusTone::Indeterminate),
        ];
        for (label, tone) in cases {
            assert_eq!(StatusTone::of_label(label), tone, "{label}");
        }
    }

    #[test]
    fn rcp06_axis_labels_map_to_distinct_tones() {
        // RCP-UX-01: every five-axis label resolves to a tone; the
        // labels themselves carry the meaning (color is secondary).
        let cases = [
            ("edit-possible", StatusTone::Ok),
            ("edit-blocked", StatusTone::Unavailable),
            ("live", StatusTone::Ok),
            ("blocked", StatusTone::Unavailable),
            ("render-valid", StatusTone::Completed),
            ("render-preview", StatusTone::Pending),
            ("render-stale", StatusTone::Indeterminate),
            ("render-missing", StatusTone::Unavailable),
            ("render-corrupt", StatusTone::Failed),
            ("render-unknown", StatusTone::Unknown),
            ("quality-exact-artifact", StatusTone::Ok),
            ("quality-preview", StatusTone::Pending),
            ("quality-unavailable", StatusTone::Unavailable),
            ("runtime-active", StatusTone::Running),
            ("runtime-last-good-mismatch", StatusTone::Indeterminate),
            ("partial", StatusTone::Indeterminate),
        ];
        for (label, tone) in cases {
            assert_eq!(StatusTone::of_label(label), tone, "{label}");
        }
    }

    #[test]
    fn svscript04_analyzer_and_severity_labels_map_to_dedicated_tones() {
        // SV-SCRIPT-04: analyzer availability/capability/staleness and
        // diagnostic severities get their own tones — `unsupported`
        // is not `unavailable`, `stale`/`dirty` are not warnings.
        let cases = [
            ("unavailable", StatusTone::Unavailable),
            ("unsupported", StatusTone::Unsupported),
            ("stale", StatusTone::Stale),
            ("dirty", StatusTone::Dirty),
            ("ready", StatusTone::Ok),
            ("idle", StatusTone::Unknown),
            ("rejected", StatusTone::Failed),
            ("information", StatusTone::DiagnosticInformation),
            ("warning", StatusTone::DiagnosticWarning),
            ("error", StatusTone::DiagnosticError),
            ("diagnostic-warning", StatusTone::DiagnosticWarning),
            ("severity-error", StatusTone::DiagnosticError),
        ];
        for (label, tone) in cases {
            assert_eq!(StatusTone::of_label(label), tone, "{label}");
        }
    }

    #[test]
    fn unrecognized_labels_fail_closed_to_unknown() {
        assert_eq!(StatusTone::of_label(""), StatusTone::Unknown);
        assert_eq!(StatusTone::of_label("sort-of-fine"), StatusTone::Unknown);
        assert_eq!(StatusTone::of_label("OK"), StatusTone::Unknown);
    }
}
