//! Editing tools and semantic hit resolution (ED-A-02).
//!
//! The headless `PoC` uses a *semantic* [`HitProbe`]: the caller
//! supplies position + zone candidates, never pixel coordinates.
//! The [`Tool::Smart`] resolver applies the fixed priority
//! `Handle > Body > LaneEmpty > Background`; a dedicated tool skips
//! that ladder and resolves its own operation. Two equally-ranked
//! candidates resolve [`ToolResolution::Ambiguous`] with suggested
//! dedicated tools — the ambiguity is surfaced, never guessed away.
//!
//! Deferred (unverified here): real hit-test geometry, drag
//! thresholds, hover, pen/touch behaviour.

use serde::{Deserialize, Serialize};
use synvelis_project::MusicalTick;

use crate::state::SemanticTarget;

/// The editing tool active on a view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    /// Context-sensitive tool — resolves by hit-zone priority.
    #[default]
    Smart,
    /// Explicit single-object selection.
    Select,
    /// Range selection over time.
    Marquee,
    /// Create a clip on an empty lane span.
    Draw,
    /// Split a clip at the hit position.
    Split,
    /// Trim a clip edge (requires a handle hit).
    Trim,
    /// Slide a clip's content under a fixed window.
    Slip,
    /// Time-stretch a clip boundary.
    Stretch,
    /// Adjust a clip fade handle.
    Fade,
    /// Delete the hit objects.
    Erase,
}

impl Tool {
    /// Parses a command-argument tool name (`"smart"`, `"trim"`, …).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "smart" => Self::Smart,
            "select" => Self::Select,
            "marquee" => Self::Marquee,
            "draw" => Self::Draw,
            "split" => Self::Split,
            "trim" => Self::Trim,
            "slip" => Self::Slip,
            "stretch" => Self::Stretch,
            "fade" => Self::Fade,
            "erase" => Self::Erase,
            _ => return None,
        })
    }
}

/// Clip handle kinds a probe can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HandleKind {
    /// Leading-edge trim handle.
    StartTrim,
    /// Trailing-edge trim handle.
    EndTrim,
    /// Fade-in curve handle.
    FadeIn,
    /// Fade-out curve handle.
    FadeOut,
    /// Stretch boundary handle.
    Stretch,
}

/// The semantic zone a hit candidate occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "zone", content = "handle", rename_all = "kebab-case")]
pub enum HitZone {
    /// On a clip handle (trim/fade/stretch).
    Handle(HandleKind),
    /// On a clip body.
    Body,
    /// On a lane's empty span.
    LaneEmpty,
    /// On view background (no lane).
    Background,
}

impl HitZone {
    /// Smart-tool priority — lower wins.
    fn priority(self) -> u8 {
        match self {
            Self::Handle(_) => 0,
            Self::Body => 1,
            Self::LaneEmpty => 2,
            Self::Background => 3,
        }
    }
}

/// One reported hit: the zone touched plus the entity under it, when
/// any (lane-empty reports its lane, background reports nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitCandidate {
    /// The zone hit.
    pub zone: HitZone,
    /// The semantic entity under the hit, if any.
    pub target: Option<SemanticTarget>,
}

/// A semantic pointer probe — position plus every zone candidate the
/// (hypothetical) hit test reports. Deterministic by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitProbe {
    /// Pointer position in musical ticks.
    pub position: MusicalTick,
    /// Zone candidates at that position, in report order.
    pub candidates: Vec<HitCandidate>,
}

/// The operation a gesture fixes at start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum EditOperation {
    /// Select the hit object (updates selection on commit).
    Select,
    /// Range-select clips on the scoped lanes over the dragged
    /// tick range (updates selection on commit).
    Marquee,
    /// Move the fixed clip set; the primary target snaps.
    MoveClips,
    /// Resize the clip's leading/trailing anchor edge.
    TrimClip {
        /// Which edge is dragged.
        edge: HandleKind,
    },
    /// Split the clip at the probe position.
    SplitClip,
    /// Create a clip on the hit lane.
    DrawClip,
    /// Delete the fixed targets.
    EraseTargets,
    /// Slip content (resolves; commit unsupported in this `PoC`).
    SlipClip,
    /// Stretch boundary (resolves; commit unsupported).
    StretchClip,
    /// Adjust fade handle (resolves; commit unsupported).
    FadeClip,
}

/// What a `pointer_down` resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResolution {
    /// A gesture may start with this fixed operation/target set.
    Resolved {
        /// The fixed operation.
        operation: EditOperation,
        /// The fixed target set.
        targets: Vec<SemanticTarget>,
    },
    /// Equally-ranked candidates — exposed with the dedicated tools
    /// that would disambiguate, instead of guessing.
    Ambiguous {
        /// The competing operations.
        candidates: Vec<EditOperation>,
        /// Dedicated tools that resolve the ambiguity.
        suggested: Vec<Tool>,
    },
    /// Nothing to do — no gesture starts.
    NoGesture,
}

/// Maps a handle kind to its operation.
fn handle_op(kind: HandleKind) -> (EditOperation, Tool) {
    match kind {
        HandleKind::StartTrim | HandleKind::EndTrim => {
            (EditOperation::TrimClip { edge: kind }, Tool::Trim)
        }
        HandleKind::FadeIn | HandleKind::FadeOut => (EditOperation::FadeClip, Tool::Fade),
        HandleKind::Stretch => (EditOperation::StretchClip, Tool::Stretch),
    }
}

/// Resolves `probe` under `tool` into a fixed operation + targets.
///
/// `selection` is the view's *current* object-selection targets —
/// used only to keep a body hit inside an existing clip selection a
/// multi-target move; later selection changes never retarget.
#[must_use]
pub fn resolve_tool(tool: Tool, probe: &HitProbe, selection: &[SemanticTarget]) -> ToolResolution {
    if tool != Tool::Smart {
        return resolve_dedicated(tool, probe);
    }
    let Some(best) = probe.candidates.iter().map(|c| c.zone.priority()).min() else {
        return ToolResolution::NoGesture;
    };
    let top: Vec<&HitCandidate> = probe
        .candidates
        .iter()
        .filter(|c| c.zone.priority() == best)
        .collect();
    if top.len() > 1 {
        // Same-priority competition — surface every option.
        let mut candidates = Vec::new();
        let mut suggested = Vec::new();
        for hit in &top {
            let (op, dedicated) = smart_op(hit);
            candidates.push(op);
            suggested.push(dedicated);
        }
        return ToolResolution::Ambiguous {
            candidates,
            suggested,
        };
    }
    let (operation, _) = smart_op(top[0]);
    ToolResolution::Resolved {
        operation,
        targets: smart_targets(operation, top[0], selection),
    }
}

/// Smart-tool operation + the dedicated tool that pins it.
fn smart_op(hit: &HitCandidate) -> (EditOperation, Tool) {
    match hit.zone {
        HitZone::Handle(kind) => handle_op(kind),
        HitZone::Body => (EditOperation::MoveClips, Tool::Select),
        HitZone::LaneEmpty => (EditOperation::DrawClip, Tool::Draw),
        HitZone::Background => (EditOperation::Marquee, Tool::Marquee),
    }
}

/// Fixed targets for a smart-resolved operation.
fn smart_targets(
    operation: EditOperation,
    hit: &HitCandidate,
    selection: &[SemanticTarget],
) -> Vec<SemanticTarget> {
    match operation {
        // A body hit on a selected clip moves the whole selection;
        // otherwise just the hit clip (it becomes the new selection).
        EditOperation::MoveClips => {
            let Some(target) = hit.target else {
                return Vec::new();
            };
            if selection.contains(&target) {
                selection.to_vec()
            } else {
                vec![target]
            }
        }
        EditOperation::Marquee => Vec::new(),
        _ => hit.target.into_iter().collect(),
    }
}

/// Dedicated-tool resolution — explicit, never through the Smart
/// ladder. A tool with no compatible candidate reports `NoGesture`.
fn resolve_dedicated(tool: Tool, probe: &HitProbe) -> ToolResolution {
    let entity = |z: fn(&HitCandidate) -> bool| -> Option<SemanticTarget> {
        probe
            .candidates
            .iter()
            .find(|c| z(c))
            .and_then(|c| c.target)
    };
    let resolved = |operation, target: Option<SemanticTarget>| ToolResolution::Resolved {
        operation,
        targets: target.into_iter().collect(),
    };
    match tool {
        Tool::Smart => unreachable!("smart handled by caller"),
        Tool::Select => {
            let target = entity(|c| c.target.is_some());
            resolved(EditOperation::Select, target)
        }
        Tool::Marquee => resolved(EditOperation::Marquee, None),
        Tool::Draw => entity(|c| c.zone == HitZone::LaneEmpty)
            .map_or(ToolResolution::NoGesture, |t| {
                resolved(EditOperation::DrawClip, Some(t))
            }),
        Tool::Split => entity(|c| c.zone == HitZone::Body).map_or(ToolResolution::NoGesture, |t| {
            resolved(EditOperation::SplitClip, Some(t))
        }),
        Tool::Trim => probe
            .candidates
            .iter()
            .find(|c| {
                matches!(
                    c.zone,
                    HitZone::Handle(HandleKind::StartTrim | HandleKind::EndTrim)
                )
            })
            .map_or(ToolResolution::NoGesture, |c| {
                let HitZone::Handle(edge) = c.zone else {
                    unreachable!()
                };
                resolved(EditOperation::TrimClip { edge }, c.target)
            }),
        Tool::Slip => entity(|c| c.zone == HitZone::Body).map_or(ToolResolution::NoGesture, |t| {
            resolved(EditOperation::SlipClip, Some(t))
        }),
        Tool::Stretch => {
            entity(|c| matches!(c.zone, HitZone::Handle(HandleKind::Stretch) | HitZone::Body))
                .map_or(ToolResolution::NoGesture, |t| {
                    resolved(EditOperation::StretchClip, Some(t))
                })
        }
        Tool::Fade => entity(|c| {
            matches!(
                c.zone,
                HitZone::Handle(HandleKind::FadeIn | HandleKind::FadeOut)
            )
        })
        .map_or(ToolResolution::NoGesture, |t| {
            resolved(EditOperation::FadeClip, Some(t))
        }),
        Tool::Erase => entity(|c| c.target.is_some()).map_or(ToolResolution::NoGesture, |t| {
            resolved(EditOperation::EraseTargets, Some(t))
        }),
    }
}
