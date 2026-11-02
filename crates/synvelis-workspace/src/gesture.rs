//! Editing gesture state (ED-A-02 fixation, ED-A-04 preview).
//!
//! A [`Gesture`] captures, at `pointer_down`, *everything* that may
//! not float later: the resolved [`EditOperation`], the fixed target
//! set, the project revision it was started against, and the shared
//! [`EditTimeDomain`]. Pointer updates only ever move the
//! [`GesturePreview`] — view/session state that never touches the
//! project, its revision, undo history or the engine stub. Commit
//! validates against the fixated revision, so an in-between project
//! change rejects rather than silently retargeting.

use serde::{Deserialize, Serialize};
use synvelis_project::{MusicalTick, ProjectRevision};

use crate::selection::EditTimeDomain;
use crate::snap::SnapWinner;
use crate::state::SemanticTarget;
use crate::tool::EditOperation;

/// Everything captured at gesture start — immutable for the
/// gesture's lifetime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GestureFixation {
    /// The resolved operation.
    pub operation: EditOperation,
    /// The fixed target set (primary = last).
    pub targets: Vec<SemanticTarget>,
    /// Project revision the gesture started against.
    pub revision: ProjectRevision,
    /// The shared time domain of the targets (or the probe domain for
    /// target-free operations like marquee).
    pub domain: EditTimeDomain,
    /// Pointer position at gesture start.
    pub origin: MusicalTick,
}

impl GestureFixation {
    /// The primary target — the gesture anchor and the only object
    /// that snaps during a multi-target move.
    #[must_use]
    pub fn primary(&self) -> Option<SemanticTarget> {
        self.targets.last().copied()
    }
}

/// The live, mutable half of a gesture. This is *the* drag preview:
/// it exists only inside view state and is dropped on commit/cancel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GesturePreview {
    /// Current pointer position.
    pub position: MusicalTick,
    /// Tick delta from the fixation origin, after snapping.
    pub delta: i64,
    /// The active snap winner, when the last update snapped.
    pub snap: Option<SnapWinner>,
}

/// A running gesture — fixation + preview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gesture {
    /// Fixed-at-start data.
    pub fixation: GestureFixation,
    /// Mutable preview state.
    pub preview: GesturePreview,
}

/// Why a gesture ended without committing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CancelReason {
    /// Escape key.
    Escape,
    /// Pointer cancellation (device-level abort).
    PointerCancel,
    /// The window hosting the gesture was destroyed.
    WindowClosed,
    /// A fixed target disappeared from the project.
    TargetDeleted,
    /// The input context left `View` mid-gesture.
    InputScopeLost,
}

/// The result of a `commit_gesture`/`cancel_gesture` call.
#[derive(Debug, Clone, PartialEq)]
pub enum GestureOutcome {
    /// The gesture committed; carries the new project revision.
    Applied {
        /// Revision after the atomic commit.
        revision: ProjectRevision,
    },
    /// The gesture updated the view's object selection (select /
    /// marquee commit) — no project mutation happened.
    SelectionChanged,
    /// The gesture committed to nothing (e.g. erase with no
    /// surviving targets).
    NoChange,
    /// The gesture was cancelled — project untouched.
    Cancelled {
        /// Why it was cancelled.
        reason: CancelReason,
    },
}
