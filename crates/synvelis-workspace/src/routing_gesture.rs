//! Signal Flow routing gestures — port-drag connect commits
//! (synvelis#108 SF-UX-03).
//!
//! A routing gesture is the *fourth* gesture family, deliberately
//! distinct from [`crate::gesture::Gesture`] (timeline editing,
//! `EditTimeDomain`-bound) and [`crate::control_edge::ControlEdgeGesture`]
//! (control-domain edge): it fixes typed routing endpoints plus the
//! project revision and port-capability generation at pointer-down,
//! and commits exactly one `ProjectOp::Routing` through the session —
//! the same single-writer path the Mixer uses, so undo, revision CAS
//! and engine notification stay shared.
//!
//! Fixed semantics:
//! - **Begin** freezes view, source [`SemanticPortId`], project
//!   revision (CAS) and `port_capability_generation`, plus the owning
//!   window epoch. No later input — selection, focus, dock transfer,
//!   link events, plugin reconfigure — may rewrite them.
//! - **Hover** recomputes a dry-run [`routing_preflight`] assessment
//!   for the candidate target port. The dry run stages on a scratch
//!   [`CounterIdGenerator`] seeded from `id_watermark` — it never
//!   consumes live IDs, never mutates the project, and never reaches
//!   undo or the engine. Pointer movement only ever moves preview
//!   state.
//! - **Drop** fixes the target port. After the drop the candidate is
//!   immutable — a late hover, receipt, or focus event cannot swap
//!   the target.
//! - **Confirm** requires the explicit consent the assessment asked
//!   for (an [`ExplicitChannelMap`] for layout-mismatched ports,
//!   carried with `ExplicitConversionPolicy::Approved`), then applies
//!   one `ProjectOp::Routing` at the fixated revision. Success is one
//!   revision and one undo entry; a moved window epoch, an advanced
//!   project revision, or a moved capability generation drops the
//!   gesture as [`RoutingGestureOutcome::Stale`] instead of
//!   retargeting.
//! - **Cancel** (Escape/pointer-cancel/window close/input-scope
//!   loss) discards the preview; the project is untouched, so no
//!   undo boundary is created.

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::window::WindowId;
use synvelis_project::{
    ConnectionKind, CounterIdGenerator, ExplicitChannelMap, ExplicitConversionPolicy, PortRef,
    PortRole, ProjectRevision, RoutingEdit, RoutingEditError, RoutingEditIntent, SemanticPortId,
    TempConnectionId, routing_preflight,
};

use crate::gesture::CancelReason;

/// Begin-fixed state of a Signal Flow routing gesture. Every field
/// is captured once at Begin; nothing later may rewrite it.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingGestureFixation {
    /// The view that owns the gesture.
    pub view: ViewInstanceId,
    /// The port the drag started from — fixed at Begin.
    pub source: SemanticPortId,
    /// Project revision at Begin; commit's expected revision (CAS).
    pub revision: ProjectRevision,
    /// `port_capability_generation` at Begin — a plugin port
    /// reconfigure mid-gesture moves it and the commit drops stale.
    pub capability_generation: u64,
    /// Owning window at Begin (`None` = detached pool). A different
    /// owner at commit means the view moved windows mid-gesture.
    pub window: Option<WindowId>,
}

/// Compatibility verdict for a hovered candidate target — the
/// display vocabulary behind "compatible / prohibited / needs
/// mapping". Computed by a full dry-run [`routing_preflight`], so it
/// is the same authority the commit will face, never a heuristic.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingPortAssessment {
    /// The connection would preflight cleanly.
    Compatible,
    /// Channel layouts differ — an explicit [`ExplicitChannelMap`]
    /// plus approved conversion policy is required before commit.
    NeedsMapping {
        /// Source port channel count.
        source_channels: u32,
        /// Destination port channel count.
        destination_channels: u32,
    },
    /// Any other typed rejection — direction, domain, unresolved
    /// binding, cycle, multiplicity — surfaced verbatim.
    Prohibited {
        /// The typed rejection detail.
        reason: String,
    },
}

/// Mutable preview state — display/commit-input only. The project is
/// never touched by preview updates.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingGesturePreview {
    /// The port currently hovered (`None` = over empty canvas).
    /// Rewritten freely while dragging; frozen by `drop`.
    pub candidate: Option<SemanticPortId>,
    /// Assessment of `candidate` against the fixated source.
    pub assessment: Option<RoutingPortAssessment>,
    /// The target fixed by drop — `Some` means the gesture awaits
    /// explicit confirm and no later hover may swap it.
    pub dropped_target: Option<SemanticPortId>,
}

/// A running Signal Flow routing gesture — fixation + preview.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingGesture {
    /// Fixed-at-Begin data.
    pub fixation: RoutingGestureFixation,
    /// Mutable preview state.
    pub preview: RoutingGesturePreview,
}

/// The result of `routing_gesture_drop`/`routing_gesture_confirm`/
/// `routing_gesture_cancel`.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingGestureOutcome {
    /// The gesture committed; carries the new project revision.
    /// Exactly one undo boundary was created.
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
    /// epoch moved, the fixated project revision went stale, or the
    /// port capability generation moved (plugin reconfigure). Never
    /// retargets to current state.
    Stale,
}

/// The `ConnectionKind` a dropped port pair commits as — derived
/// from the *destination* port's role: a sidechain input port makes
/// a `Sidechain` connection, everything else `Main` (`Send` routes
/// are authored through their own edit path, never a bare port drag).
#[must_use]
pub fn routing_connection_kind(destination_role: PortRole) -> ConnectionKind {
    match destination_role {
        PortRole::Sidechain => ConnectionKind::Sidechain,
        PortRole::Main => ConnectionKind::Main,
    }
}

/// Builds the single-edit [`RoutingEditIntent`] a confirmed gesture
/// commits — `CreateConnection` between the fixated source and the
/// dropped target. `channel_map` is the explicit consent: `Some`
/// carries `ExplicitConversionPolicy::Approved`, `None` carries the
/// default `None` policy (a layout-mismatched pair then rejects as
/// `RequiresUserDecision`, never a silent up/downmix).
#[must_use]
pub fn routing_gesture_intent(
    operation_id: String,
    fixation: &RoutingGestureFixation,
    target: SemanticPortId,
    destination_role: PortRole,
    channel_map: Option<&ExplicitChannelMap>,
) -> RoutingEditIntent {
    RoutingEditIntent {
        operation_id,
        expected_port_capability_generation: fixation.capability_generation,
        edits: vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: PortRef::Existing(fixation.source),
            destination: PortRef::Existing(target),
            kind: routing_connection_kind(destination_role),
            channel_map: channel_map.cloned(),
        }],
        approved_conversion: if channel_map.is_some() {
            ExplicitConversionPolicy::Approved
        } else {
            ExplicitConversionPolicy::None
        },
    }
}

/// Classifies a dry-run [`routing_preflight`] result into the
/// display vocabulary. `RequiresUserDecision` becomes
/// [`RoutingPortAssessment::NeedsMapping`] only when the cause is a
/// channel-layout decision — identified by re-checking the endpoint
/// channel counts, since other user decisions do not exist for a
/// single `CreateConnection` today; any other typed error is
/// [`RoutingPortAssessment::Prohibited`].
pub(crate) fn classify_preflight(
    result: Result<synvelis_project::RoutingPlan, RoutingEditError>,
    project: &synvelis_project::Project,
    source: SemanticPortId,
    target: SemanticPortId,
) -> RoutingPortAssessment {
    match result {
        Ok(_) => RoutingPortAssessment::Compatible,
        Err(RoutingEditError::RequiresUserDecision { .. }) => {
            let channels =
                |port: SemanticPortId| project.routing.ports.get(&port).map_or(0, |p| p.channels);
            let (source_channels, destination_channels) = (channels(source), channels(target));
            if source_channels == destination_channels {
                RoutingPortAssessment::Prohibited {
                    reason: "routing-requires-decision".to_string(),
                }
            } else {
                RoutingPortAssessment::NeedsMapping {
                    source_channels,
                    destination_channels,
                }
            }
        }
        Err(error) => RoutingPortAssessment::Prohibited {
            reason: error.to_string(),
        },
    }
}

/// Runs the hover assessment: one dry-run preflight of the
/// single-`CreateConnection` intent against the *live* project —
/// capability generation comes from the fixation so a plugin
/// reconfigure surfaces as [`RoutingEditError::StalePortCapability`]
/// (prohibited), matching what commit would face. The scratch
/// generator is seeded from `id_watermark` exactly like
/// `ProjectController::preview_routing`; live IDs are never consumed.
pub(crate) fn assess_connection(
    project: &synvelis_project::Project,
    fixation: &RoutingGestureFixation,
    target: SemanticPortId,
    destination_role: PortRole,
) -> RoutingPortAssessment {
    let intent = routing_gesture_intent(
        format!("sf-gesture-preview-{}", fixation.view.get()),
        fixation,
        target,
        destination_role,
        None,
    );
    let mut dry = CounterIdGenerator::new(project.id_watermark);
    let result = routing_preflight(project, &intent, &mut dry);
    classify_preflight(result, project, fixation.source, target)
}
