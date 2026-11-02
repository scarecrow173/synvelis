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
            unit: SnapUnit::Explicit(960),
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
            unit: SnapUnit::Explicit(960),
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
