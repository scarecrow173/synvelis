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
use synvelis_project::{ClipId, LaneId, MediaSpan, MusicalRange, SourceId};

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
