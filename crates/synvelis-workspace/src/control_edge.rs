//! Control Layer edge gestures — modulation connect drags
//! (Synvelis #102 UI-C5-02).
//!
//! A Control Layer edge connects a [`ModulationSource`] to a
//! [`ParameterTarget`]. It is deliberately *not* an Audio Routing
//! gesture: routing lives in [`crate::gesture`]/[`crate::edit`] over
//! `SemanticTarget`s, while this module fixes typed control-domain
//! endpoints at Begin and commits exactly one
//! `ProjectOp::AddModulationAssignment` through the session.
//!
//! Fixed semantics mirror the routing gesture contract:
//! - **Begin** freezes view, both endpoint ids, the project revision
//!   (CAS) and the owning window epoch.
//! - **Preview** mutates only the in-memory preview values — the
//!   project is never touched.
//! - **Commit** applies the single op at the fixated revision; a
//!   moved window epoch or an advanced project revision drops the
//!   gesture as [`ControlEdgeOutcome::Stale`] instead of retargeting.
//! - **Cancel** discards the preview; the project is untouched, so
//!   no undo boundary is created.
//!
//! Undo correctness falls out of the session contract: only a
//! successful `ProjectSession::apply` advances the revision and
//! history, so previews and cancels can never dirty undo.

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::window::WindowId;
use synvelis_project::{ModulationSourceId, ParameterTargetId, Polarity, ProjectRevision};

use crate::gesture::CancelReason;

/// Begin-fixed state of a Control Layer edge gesture. Every field is
/// captured once at Begin; no later input — selection change, focus
/// change, dock transfer, another view's activity — may rewrite it.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlEdgeFixation {
    /// The view that owns the gesture.
    pub view: ViewInstanceId,
    /// Drag source — a modulation source id (typed, not a UI anchor).
    pub source: ModulationSourceId,
    /// Drop target — a parameter target id.
    pub target: ParameterTargetId,
    /// Project revision at Begin; commit's expected revision (CAS).
    pub revision: ProjectRevision,
    /// Owning window at Begin (`None` = detached pool). A different
    /// owner at commit means the view moved windows mid-gesture.
    pub window: Option<WindowId>,
}

/// Mutable preview values — display/commit-input state only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlEdgePreview {
    /// Assignment depth (normalized increment scale).
    pub depth: f64,
    /// Bipolar/unipolar application.
    pub polarity: Polarity,
}

impl Default for ControlEdgePreview {
    fn default() -> Self {
        Self {
            depth: 1.0,
            polarity: Polarity::Bipolar,
        }
    }
}

/// A running Control Layer edge gesture — fixation + preview.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlEdgeGesture {
    /// Fixed-at-Begin data.
    pub fixation: ControlEdgeFixation,
    /// Mutable preview state.
    pub preview: ControlEdgePreview,
}

/// The result of `control_edge_commit`/`control_edge_cancel`.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlEdgeOutcome {
    /// The gesture committed; carries the new project revision.
    Applied {
        /// Revision after the atomic commit.
        revision: ProjectRevision,
    },
    /// The gesture was cancelled; the project is untouched.
    Cancelled {
        /// Why the gesture ended.
        reason: CancelReason,
    },
    /// The gesture was dropped without applying: the view's window
    /// epoch moved (dock transfer/detach) or the fixated project
    /// revision went stale. Never retargets to current state.
    Stale,
}
