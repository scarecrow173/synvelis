//! Grid display vs snap resolution (ED-A-03).
//!
//! [`GridConfig`] (what the view draws) and [`SnapConfig`] (what
//! gestures snap to) are independent: snapping works with the grid
//! hidden. `SnapUnit::Explicit` never follows zoom; only
//! `SnapUnit::Auto` does (the caller derives `auto_step` from the
//! viewport). Candidates are filtered to the gesture's
//! [`EditTimeDomain`], ranked by an explicit source priority, and
//! tie-broken deterministically. A hysteresis window keeps the
//! current winner stable under sub-threshold pointer drift.
//!
//! Deferred (unverified here): magnetic-mode feel, real hysteresis
//! behaviour under device input, adaptive grid subdivision.

use serde::{Deserialize, Serialize};

use crate::selection::EditTimeDomain;

/// Default snap/grid quantum — one project tick-count of 960, the
/// musical spacing of one quarter note at the default project PPQN
/// of 960. Shared by [`GridConfig`]/[`SnapConfig`] defaults and the
/// PPQN-change effect reporting in `crate::ppqn_ui`.
pub const DEFAULT_SNAP_QUANTUM_TICKS: i64 = 960;

/// A snap/grid quantum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "ticks", rename_all = "kebab-case")]
pub enum SnapUnit {
    /// A fixed quantum in the gesture domain's declared base units
    /// (ticks for musical gestures) — never changes with zoom.
    Explicit(i64),
    /// Zoom-derived quantum; the caller recomputes `auto_step` when
    /// the viewport changes. Only this unit may follow zoom.
    Auto,
}

/// What the view draws as its background grid. Purely cosmetic —
/// it never feeds snap resolution.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridConfig {
    /// Whether the grid lines are painted.
    pub visible: bool,
    /// The grid quantum.
    pub unit: SnapUnit,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            visible: true,
            unit: SnapUnit::Explicit(DEFAULT_SNAP_QUANTUM_TICKS),
        }
    }
}

/// Snapping behaviour for gestures in this view.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapConfig {
    /// Whether snapping is active (independent of `grid.visible`).
    pub enabled: bool,
    /// The snap quantum used to generate grid candidates.
    pub unit: SnapUnit,
    /// Max distance (in the gesture domain's declared base units) a
    /// candidate may sit from the pointer position and still win.
    pub threshold_ticks: i64,
    /// Distance within which the previous winner is kept stable.
    pub hysteresis_ticks: i64,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            unit: SnapUnit::Explicit(DEFAULT_SNAP_QUANTUM_TICKS),
            threshold_ticks: 240,
            hysteresis_ticks: 60,
        }
    }
}

/// Snap candidate sources, in explicit priority order — lower wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapSource {
    /// A clip edge (bounds of other clips on the same lanes).
    ClipBounds,
    /// The transport playhead.
    Playhead,
    /// A grid line of the configured quantum.
    Grid,
}

/// One concrete snap candidate. `position` is a scalar in the
/// gesture domain's declared base units — the caller supplies
/// candidates in the same base the gesture resolves deltas in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapCandidate {
    /// Where the candidate comes from.
    pub source: SnapSource,
    /// Its timeline position, in declared base units.
    pub position: i64,
    /// The time domain the candidate lives in.
    pub domain: EditTimeDomain,
}

/// Everything snap resolution needs for one pointer update — supplied
/// by the caller so the resolver stays a pure function.
#[derive(Debug, Clone, Default)]
pub struct SnapEnvironment {
    /// The quantum `SnapUnit::Auto` resolves to this frame (zoom
    /// derived). Unused by explicit units.
    pub auto_step: i64,
    /// Non-grid candidates (clip bounds, playhead, …).
    pub candidates: Vec<SnapCandidate>,
}

/// The chosen snap position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapWinner {
    /// Winning source.
    pub source: SnapSource,
    /// Snapped position, in declared base units.
    pub position: i64,
}

impl SnapWinner {
    /// Deterministic evidence label — `"clip-bounds@960"`-style; tests
    /// report which candidate actually won.
    #[must_use]
    pub fn label(&self) -> String {
        let src = match self.source {
            SnapSource::ClipBounds => "clip-bounds",
            SnapSource::Playhead => "playhead",
            SnapSource::Grid => "grid",
        };
        format!("{src}@{}", self.position)
    }
}

/// Resolves the snap position for `position`.
///
/// Order: domain filter → hysteresis keep of `previous` → distance
/// threshold → nearest candidate → source priority on ties. `None`
/// means "no snap; keep the raw position".
#[must_use]
pub fn resolve_snap(
    config: &SnapConfig,
    env: &SnapEnvironment,
    position: i64,
    domain: EditTimeDomain,
    previous: Option<SnapWinner>,
) -> Option<SnapWinner> {
    if !config.enabled {
        return None;
    }
    let mut candidates: Vec<SnapCandidate> = env
        .candidates
        .iter()
        .copied()
        .filter(|c| c.domain == domain)
        .collect();
    // Grid candidates are generated in the gesture's own domain.
    let step = match config.unit {
        SnapUnit::Explicit(step) => Some(step),
        SnapUnit::Auto => (env.auto_step > 0).then_some(env.auto_step),
    };
    if let Some(step) = step
        && step > 0
    {
        for base in [position.div_euclid(step), position.div_euclid(step) + 1] {
            candidates.push(SnapCandidate {
                source: SnapSource::Grid,
                position: base * step,
                domain,
            });
        }
    }
    // Hysteresis: the previous winner survives while the pointer
    // stays within the keep window of its position.
    let hysteresis = u64::try_from(config.hysteresis_ticks.max(0)).unwrap_or(0);
    let threshold = u64::try_from(config.threshold_ticks.max(0)).unwrap_or(0);
    if let Some(prev) = previous {
        let drift = position.abs_diff(prev.position);
        if drift <= hysteresis {
            return Some(prev);
        }
    }
    candidates
        .iter()
        .filter(|c| position.abs_diff(c.position) <= threshold)
        .min_by_key(|c| (position.abs_diff(c.position), c.source))
        .map(|c| SnapWinner {
            source: c.source,
            position: c.position,
        })
}

/// How a snap/grid quantum's *musical* spacing moves across a project
/// PPQN change — the report half of the TIM-01 UI-slice policy
/// (synvelis#125).
///
/// Policy: [`SnapUnit::Explicit`] is a fixed quantum in the gesture
/// domain's declared base units — project ticks — and is **preserved
/// verbatim** across a PPQN change; the stored per-view/preset value
/// is never rewritten. Only its beat spacing scales, which is what
/// this type quantifies for preview surfaces. [`SnapUnit::Auto`] is
/// zoom-derived and carries no fixed tick quantum to report.
///
/// Rebinding was rejected: a tick→beat→tick round-trip under a new
/// PPQN is lossy for arbitrary quanta (e.g. 100 ticks at 960 → 1000
/// is 104.1̄6 ticks), persisted presets cannot be rewritten from a
/// project command, and `SnapUnit::Explicit`'s contract is
/// tick-fixed by definition. See
/// `docs/designs/tim-01-ppqn-ui-design.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapPpqnEffect {
    /// The quantum's fixed tick count — `None` for
    /// [`SnapUnit::Auto`] (zoom-derived, no fixed quantum).
    pub quantum_ticks: Option<i64>,
    /// Musical spacing before the change, in beats
    /// (`quantum_ticks / old_ppqn`) — `None` for `Auto`.
    pub beats_before: Option<f64>,
    /// Musical spacing after the change, in beats — `None` for
    /// `Auto`.
    pub beats_after: Option<f64>,
}

impl SnapPpqnEffect {
    /// One-line display text — `"960 ticks: 1.000 → 2.000 beats"` for
    /// explicit units, `"zoom-derived (auto)"` for `Auto`.
    #[must_use]
    pub fn describe(&self) -> String {
        match (self.quantum_ticks, self.beats_before, self.beats_after) {
            (Some(ticks), Some(before), Some(after)) => {
                format!("{ticks} ticks: {before:.3} → {after:.3} beats")
            }
            _ => "zoom-derived (auto)".to_string(),
        }
    }
}

/// Evaluates the Snap/Grid PPQN-change effect of `unit` — the
/// preservation policy made observable: explicit quanta keep their
/// tick count and their beat spacing is recomputed against the new
/// PPQN for reporting only. `old_ppqn`/`new_ppqn` are the project
/// PPQN values before/after the change (both `>= 1`).
#[must_use]
#[allow(clippy::cast_precision_loss)] // display-only beat spacing, 3 decimals
pub fn snap_unit_ppqn_effect(unit: SnapUnit, old_ppqn: u32, new_ppqn: u32) -> SnapPpqnEffect {
    let SnapUnit::Explicit(ticks) = unit else {
        return SnapPpqnEffect {
            quantum_ticks: None,
            beats_before: None,
            beats_after: None,
        };
    };
    let old = f64::from(old_ppqn.max(1));
    let new = f64::from(new_ppqn.max(1));
    SnapPpqnEffect {
        quantum_ticks: Some(ticks),
        beats_before: Some(ticks as f64 / old),
        beats_after: Some(ticks as f64 / new),
    }
}
