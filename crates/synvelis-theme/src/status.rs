//! Automation status token vocabulary (AUTX-07 / synvelis#266).
//!
//! The Action Palette, Macro Browser, Workflow Editor, and Run Monitor
//! surfaces render authority states — capability availability, grant
//! verdicts, and run/job phases — that need dedicated product colors
//! distinct from the generic `ok`/`warn`/`err` ramp. This module is
//! the single source of truth for the DTCG token *paths* those
//! states map to; the resolved values live in the product theme
//! documents (`assets/themes/*.dtcg.json`, owned outside this crate).
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
}

impl StatusTone {
    /// All tones, in display order.
    pub const ALL: [Self; 11] = [
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
        }
    }

    /// Maps a stable status *label* (the vocabulary used by the
    /// workspace projections — `MonitorStatus::label`,
    /// `Availability::name`, `GrantState::label` heads) onto a tone.
    /// Unknown labels map to [`Self::Unknown`] — fail-closed, never
    /// guessed.
    #[must_use]
    pub fn of_label(label: &str) -> Self {
        match label {
            "available" | "granted" | "satisfied" => Self::Ok,
            "completed" => Self::Completed,
            "disabled" | "planned" | "retired" | "unavailable" | "missing" => Self::Unavailable,
            "pending" => Self::Pending,
            "running" => Self::Running,
            "failed" => Self::Failed,
            "cancel-requested" => Self::CancelRequested,
            "cancel-unconfirmed" => Self::CancelUnconfirmed,
            "cancelled" => Self::Cancelled,
            "indeterminate" => Self::Indeterminate,
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
    fn unrecognized_labels_fail_closed_to_unknown() {
        assert_eq!(StatusTone::of_label(""), StatusTone::Unknown);
        assert_eq!(StatusTone::of_label("sort-of-fine"), StatusTone::Unknown);
        assert_eq!(StatusTone::of_label("OK"), StatusTone::Unknown);
    }
}
