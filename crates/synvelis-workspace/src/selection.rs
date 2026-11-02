//! Editing-model selection types (ED-A-01, issue #11/#38).
//!
//! One view carries three independent selection concepts, none
//! derived from another:
//!
//! - [`ObjectSelections`] — the object selection, *per editing
//!   domain* (tracks/lanes/clips/sources), with an explicit primary
//!   target used as the gesture anchor.
//! - [`TimeSelection`] — a time range, meaningful on its own and
//!   never implicitly destructive.
//! - [`LaneScope`] — the lanes an editing command may reach into.
//!
//! [`EditBinding`] (an open editor's displayed clip/source) is a
//! display binding, not a command target.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use synvelis_project::{
    ClipId, LaneId, MediaSpan, MediaTimeValue, MusicalRange, MusicalTick, SourceId,
};

use crate::error::WorkspaceError;
use crate::state::{EditingSelection, SemanticTarget};

/// The editing domain a selection or command belongs to.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum SelectionDomain {
    /// Track objects (mixer strip selection, …).
    Tracks,
    /// Lane objects.
    Lanes,
    /// Clip objects — the default domain a fresh view starts in.
    #[default]
    Clips,
    /// Media source objects.
    Sources,
}

impl SelectionDomain {
    /// The domain `target` belongs to.
    #[must_use]
    pub fn of(target: SemanticTarget) -> Self {
        match target {
            SemanticTarget::Track(_) => Self::Tracks,
            SemanticTarget::Lane(_) => Self::Lanes,
            SemanticTarget::Clip(_) => Self::Clips,
            SemanticTarget::Source(_) => Self::Sources,
        }
    }
}

/// Per-domain object selection. Selections in different domains
/// coexist: switching the active domain never erases another
/// domain's selection, and the *primary* target is the last target
/// of the active domain's selection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectSelections {
    /// The domain new selections/commands act in.
    pub active: SelectionDomain,
    /// One [`EditingSelection`] per domain (absent = empty).
    #[serde(default)]
    pub selections: BTreeMap<SelectionDomain, EditingSelection>,
}

impl ObjectSelections {
    /// The active domain's selection (empty when unset).
    #[must_use]
    pub fn active_selection(&self) -> &EditingSelection {
        static EMPTY: EditingSelection = EditingSelection {
            targets: Vec::new(),
        };
        self.selections.get(&self.active).unwrap_or(&EMPTY)
    }

    /// The selection stored for `domain`.
    #[must_use]
    pub fn selection(&self, domain: SelectionDomain) -> Option<&EditingSelection> {
        self.selections.get(&domain)
    }

    /// The primary (gesture-anchor) target of the active domain.
    #[must_use]
    pub fn primary(&self) -> Option<SemanticTarget> {
        self.active_selection().primary()
    }

    /// Whether every domain's selection is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selections.values().all(EditingSelection::is_empty)
    }

    /// Stores `selection` under `domain` and makes it active.
    ///
    /// # Errors
    /// [`WorkspaceError::MixedDomains`] when `selection` mixes
    /// targets from more than one domain or disagrees with `domain`.
    pub fn set(
        &mut self,
        domain: SelectionDomain,
        selection: EditingSelection,
    ) -> Result<(), WorkspaceError> {
        for target in &selection.targets {
            if SelectionDomain::of(*target) != domain {
                return Err(WorkspaceError::MixedDomains(format!(
                    "target {target:?} is not in domain {domain:?}"
                )));
            }
        }
        if selection.is_empty() {
            self.selections.remove(&domain);
        } else {
            self.selections.insert(domain, selection);
        }
        self.active = domain;
        Ok(())
    }

    /// Makes `domain` active without touching its stored selection —
    /// switching domains never erases another domain's selection.
    pub fn activate(&mut self, domain: SelectionDomain) {
        self.active = domain;
    }

    /// Stores `selection` in the domain its targets imply.
    ///
    /// # Errors
    /// [`WorkspaceError::MixedDomains`] when `selection` mixes
    /// targets from more than one domain.
    pub fn assign(&mut self, selection: EditingSelection) -> Result<(), WorkspaceError> {
        let Some(first) = selection.targets.first() else {
            // An empty selection clears the active domain only.
            self.selections.remove(&self.active);
            return Ok(());
        };
        self.set(SelectionDomain::of(*first), selection)
    }
}

/// Time-domain an edit operates in. Snap candidates and anchors are
/// only comparable inside the same domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EditTimeDomain {
    /// Musical ticks (project tempo map domain).
    Musical,
    /// Media time (source/session frames).
    Media,
}

/// A position on the edit timeline, tagged with its time domain
/// (ED-B, issue #65).
///
/// Probes, gesture origins and preview positions carry the domain in
/// the value itself, so a musical tick can never be silently
/// reinterpreted as a media instant (or vice versa). Media positions
/// keep their declared time base; converting an instant or a delta
/// between bases is exact rational math floored once at the final
/// boundary — the same convention [`MediaTimeValue::frames_at`]
/// established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
pub enum EditTimePoint {
    /// Musical position in project PPQN ticks.
    Musical(MusicalTick),
    /// Media position in its declared time base.
    Media(MediaTimeValue),
}

impl EditTimePoint {
    /// The domain this position lives in.
    #[must_use]
    pub fn domain(&self) -> EditTimeDomain {
        match self {
            Self::Musical(_) => EditTimeDomain::Musical,
            Self::Media(_) => EditTimeDomain::Media,
        }
    }

    /// The declared `(numerator, denominator)` time base a media
    /// position carries — `None` for musical positions (ticks have no
    /// declared base).
    #[must_use]
    pub fn media_base(&self) -> Option<(u32, u32)> {
        match self {
            Self::Musical(_) => None,
            Self::Media(m) => Some((m.numerator(), m.denominator())),
        }
    }

    /// Signed delta `self - origin`, expressed in `origin`'s declared
    /// base units (ticks for musical). Media instants with different
    /// bases rescale through exact rational math, floored once.
    ///
    /// # Errors
    /// [`WorkspaceError::MixedTimeDomains`] when the domains differ;
    /// [`WorkspaceError::Project`] on arithmetic overflow.
    pub fn delta_since(&self, origin: &Self) -> Result<i64, WorkspaceError> {
        match (*self, *origin) {
            (Self::Musical(pos), Self::Musical(orig)) => {
                Ok(pos.ticks_since(orig).map_err(WorkspaceError::Project)?)
            }
            (Self::Media(pos), Self::Media(orig)) => {
                let pos_units = media_rescale(
                    pos.value(),
                    (pos.numerator(), pos.denominator()),
                    (orig.numerator(), orig.denominator()),
                )
                .ok_or(WorkspaceError::Project(synvelis_project::Error::Overflow))?;
                pos_units
                    .checked_sub(orig.value())
                    .ok_or(WorkspaceError::Project(synvelis_project::Error::Overflow))
            }
            _ => Err(WorkspaceError::MixedTimeDomains(
                "position domain differs from the gesture origin".to_string(),
            )),
        }
    }
}

/// `floor(numer / denom)` as `i64` — floors toward negative infinity
/// for negative numerators, `None` when the quotient does not fit
/// `i64`. `denom` must be positive.
pub(crate) fn floor_div_i128(numer: i128, denom: i128) -> Option<i64> {
    let q = numer / denom;
    let floored = if numer % denom < 0 { q - 1 } else { q };
    i64::try_from(floored).ok()
}

/// Re-expresses `value` (in units of base `from`) in units of base
/// `to` — exact `i128` rational math floored once at the final
/// boundary. `None` when the rescaled value does not fit `i64`. A
/// time base `(n, d)` means one unit spans `n / d` seconds.
pub(crate) fn media_rescale(value: i64, from: (u32, u32), to: (u32, u32)) -> Option<i64> {
    let numer = i128::from(value) * i128::from(from.0) * i128::from(to.1);
    let denom = i128::from(from.1) * i128::from(to.0);
    floor_div_i128(numer, denom)
}

/// A view-local time selection. Independent of the object selection:
/// both may be present at once, and a time selection alone never
/// triggers destructive object commands.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "range", deny_unknown_fields)]
pub enum TimeSelection {
    /// Musical tick range.
    Musical(MusicalRange),
    /// Media-time span.
    Media(MediaSpan),
}

impl TimeSelection {
    /// The domain this selection lives in.
    #[must_use]
    pub fn domain(&self) -> EditTimeDomain {
        match self {
            Self::Musical(_) => EditTimeDomain::Musical,
            Self::Media(_) => EditTimeDomain::Media,
        }
    }
}

/// The lanes an editing command may reach into — e.g. which lanes a
/// range delete sweeps. Ordered, duplicates removed on construction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaneScope {
    /// Scoped lanes, in insertion order.
    pub lanes: Vec<LaneId>,
}

impl LaneScope {
    /// A scope containing `lanes` (deduplicated, order preserved).
    #[must_use]
    pub fn new(lanes: Vec<LaneId>) -> Self {
        let mut dedup = Vec::new();
        for lane in lanes {
            if !dedup.contains(&lane) {
                dedup.push(lane);
            }
        }
        Self { lanes: dedup }
    }

    /// Whether the scope is empty (a range command then has nothing
    /// to sweep and resolves `Unresolved`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
    }
}

/// Read-only editor display/edit binding — the clip+source an open
/// editor shows. It is *not* an editing selection and never resolves
/// as a command target (ED-A-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditBinding {
    /// Clip displayed by the editor.
    pub clip: ClipId,
    /// Source the clip refers to.
    pub source: SourceId,
}
