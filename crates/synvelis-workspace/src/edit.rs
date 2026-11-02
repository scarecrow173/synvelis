//! Gesture → [`ProjectOp`] translation (ED-A-03/04).
//!
//! The commit path is deliberately thin: [`build_commit`] derives a
//! *single* effect from the fixated gesture — one [`ProjectOp`]
//! (possibly [`ProjectOp::Composite`], which applies atomically as
//! one undo step), a selection update, or nothing. All anchor math
//! is checked and confined to the fixated [`EditTimeDomain`];
//! cross-domain moves are rejected at gesture start, never "converted
//! quietly".
//!
//! Honestly unmodelled: clip content slips, stretches and fades have
//! no commit semantics yet; `SplitClip`'s source-offset is kept
//! caller-supplied because tick→source-frame rate mapping is not
//! modelled — the workspace gesture reports it unsupported rather
//! than faking a conversion. `DrawClip` needs a source binding the
//! probe does not carry, so it is likewise unsupported here.

use synvelis_project::{MusicalRange, MusicalTick, Project, ProjectOp, TimeAnchor};

use crate::error::WorkspaceError;
use crate::gesture::Gesture;
use crate::selection::{EditTimeDomain, LaneScope, SelectionDomain};
use crate::state::SemanticTarget;
use crate::tool::EditOperation;
use crate::tool::HandleKind;

/// What committing a gesture produces.
#[derive(Debug)]
pub enum CommitEffect {
    /// One atomic project mutation (a single op or one `Composite`).
    ProjectOp(ProjectOp),
    /// The view's object selection changes; the project is
    /// untouched.
    Selection {
        /// Domain the new selection belongs to.
        domain: SelectionDomain,
        /// Selected targets (empty clears the domain selection).
        targets: Vec<SemanticTarget>,
    },
    /// Nothing changes.
    NoChange,
}

/// The shared time domain of `targets`, derived from clip anchors.
///
/// Clip targets contribute their anchor's domain; non-clip targets
/// contribute nothing. All contributing domains must agree —
/// mixed musical+media selections reject rather than convert.
///
/// # Errors
/// [`WorkspaceError::MixedTimeDomains`] on disagreement.
pub fn gesture_domain(
    project: &Project,
    targets: &[SemanticTarget],
) -> Result<EditTimeDomain, WorkspaceError> {
    let mut domain: Option<EditTimeDomain> = None;
    for target in targets {
        let next = match target {
            SemanticTarget::Clip(clip_id) => {
                project.clips.get(clip_id).map(|clip| match clip.anchor {
                    TimeAnchor::Musical(_) => EditTimeDomain::Musical,
                    TimeAnchor::Media(_) => EditTimeDomain::Media,
                })
            }
            _ => None,
        };
        if let Some(next) = next {
            match domain {
                None => domain = Some(next),
                Some(d) if d == next => {}
                Some(_) => {
                    return Err(WorkspaceError::MixedTimeDomains(
                        "targets span musical and media anchors".to_string(),
                    ));
                }
            }
        }
    }
    Ok(domain.unwrap_or(EditTimeDomain::Musical))
}

/// Builds the commit effect for `gesture`.
///
/// `lane_scope` feeds marquee range selection. `delta` is the
/// already-snapped preview delta.
///
/// # Errors
/// [`WorkspaceError::UnsupportedOperation`] for operations with no
/// modelled commit semantics; [`WorkspaceError::Project`] on anchor
/// math failure.
pub fn build_commit(
    project: &Project,
    gesture: &Gesture,
    lane_scope: &LaneScope,
) -> Result<CommitEffect, WorkspaceError> {
    let fix = &gesture.fixation;
    match fix.operation {
        EditOperation::Select => {
            let domain = fix
                .targets
                .first()
                .map_or(SelectionDomain::Clips, |t| SelectionDomain::of(*t));
            Ok(CommitEffect::Selection {
                domain,
                targets: fix.targets.clone(),
            })
        }
        EditOperation::Marquee => {
            let (lo, hi) = ordered(fix.origin, gesture.preview.position);
            let range = MusicalRange::try_new(lo, hi)?;
            let targets: Vec<SemanticTarget> = lane_scope
                .lanes
                .iter()
                .filter_map(|lane| project.lanes.get(lane))
                .flat_map(|lane| lane.clip_ids.iter().copied())
                .filter(|clip_id| {
                    matches!(
                        project.clips.get(clip_id).map(|c| c.anchor),
                        Some(TimeAnchor::Musical(r)) if r.overlaps(&range)
                    )
                })
                .map(SemanticTarget::Clip)
                .collect();
            Ok(CommitEffect::Selection {
                domain: SelectionDomain::Clips,
                targets,
            })
        }
        EditOperation::MoveClips => {
            let ops = move_ops(project, fix.targets.as_slice(), gesture.preview.delta)?;
            Ok(one_op(ops))
        }
        EditOperation::TrimClip { edge } => {
            let Some(SemanticTarget::Clip(clip_id)) = fix.primary() else {
                return Ok(CommitEffect::NoChange);
            };
            let clip = project
                .clips
                .get(&clip_id)
                .ok_or_else(|| WorkspaceError::InvalidGesture(format!("clip {clip_id} gone")))?;
            let TimeAnchor::Musical(range) = clip.anchor else {
                return Err(WorkspaceError::UnsupportedOperation(
                    "trim on a media-anchored clip",
                ));
            };
            let (start, end) = (range.start(), range.end());
            let delta = gesture.preview.delta;
            let anchor = match edge {
                HandleKind::StartTrim => MusicalRange::try_new(start.checked_add(delta)?, end)?,
                _ => MusicalRange::try_new(start, end.checked_add(delta)?)?,
            };
            Ok(CommitEffect::ProjectOp(ProjectOp::MoveClip {
                clip_id,
                anchor: TimeAnchor::Musical(anchor),
            }))
        }
        EditOperation::EraseTargets => {
            let ops: Vec<ProjectOp> = fix
                .targets
                .iter()
                .filter_map(|t| match t {
                    SemanticTarget::Clip(clip_id) => {
                        Some(ProjectOp::RemoveClip { clip_id: *clip_id })
                    }
                    _ => None,
                })
                .collect();
            Ok(one_op(ops))
        }
        EditOperation::SplitClip => Err(WorkspaceError::UnsupportedOperation(
            "split commit needs a tick→source-frame offset mapping (unmodelled)",
        )),
        EditOperation::DrawClip => Err(WorkspaceError::UnsupportedOperation(
            "draw commit needs a source binding the probe does not carry",
        )),
        EditOperation::SlipClip => Err(WorkspaceError::UnsupportedOperation("slip")),
        EditOperation::StretchClip => Err(WorkspaceError::UnsupportedOperation("stretch")),
        EditOperation::FadeClip => Err(WorkspaceError::UnsupportedOperation("fade")),
    }
}

/// Wraps 0/1/N ops into a single commit effect — N becomes one
/// `Composite` (one atomic apply = one undo step).
fn one_op(ops: Vec<ProjectOp>) -> CommitEffect {
    match ops.len() {
        0 => CommitEffect::NoChange,
        1 => CommitEffect::ProjectOp(ops.into_iter().next().unwrap_or_else(|| unreachable!())),
        _ => CommitEffect::ProjectOp(ProjectOp::Composite { ops }),
    }
}

/// `[lo, hi)` ordering of two ticks.
fn ordered(a: MusicalTick, b: MusicalTick) -> (MusicalTick, MusicalTick) {
    if a <= b { (a, b) } else { (b, a) }
}

/// One `MoveClip` per clip target, shifted by the (snapped) `delta`
/// — the primary's snap already lives in `delta`, and every other
/// clip keeps its relative offset by sharing it.
fn move_ops(
    project: &Project,
    targets: &[SemanticTarget],
    delta: i64,
) -> Result<Vec<ProjectOp>, WorkspaceError> {
    let mut ops = Vec::new();
    for target in targets {
        let SemanticTarget::Clip(clip_id) = target else {
            continue;
        };
        let clip = project
            .clips
            .get(clip_id)
            .ok_or_else(|| WorkspaceError::InvalidGesture(format!("clip {clip_id} gone")))?;
        let TimeAnchor::Musical(range) = clip.anchor else {
            return Err(WorkspaceError::UnsupportedOperation(
                "moving a media-anchored clip",
            ));
        };
        let anchor = TimeAnchor::Musical(MusicalRange::try_new(
            range.start().checked_add(delta)?,
            range.end().checked_add(delta)?,
        )?);
        ops.push(ProjectOp::MoveClip {
            clip_id: *clip_id,
            anchor,
        });
    }
    Ok(ops)
}
