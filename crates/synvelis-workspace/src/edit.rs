//! Gesture → [`ProjectOp`] translation (ED-A-03/04, ED-B).
//!
//! The commit path is deliberately thin: [`build_commit`] derives a
//! *single* effect from the fixated gesture — one [`ProjectOp`]
//! (possibly [`ProjectOp::Composite`], which applies atomically as
//! one undo step), a selection update, or nothing. All anchor math
//! is checked and confined to the fixated [`EditTimeDomain`];
//! cross-domain moves are rejected at gesture start, never "converted
//! quietly". Media-domain deltas stay in the gesture origin's declared
//! time base until each target rescales them to its own base — exact
//! rational math floored once at the final boundary.
//!
//! ED-B (issue #65) adds: `Absolute`-anchored move/trim commits,
//! `SlipClip` commits as [`ProjectOp::SetClipPlayback`] (the source
//! window translates by the mapping-adjusted delta; anchor/extent
//! untouched), and media-domain marquee. Clips whose anchor and extent
//! disagree in domain have explicit per-operation outcomes — trim
//! rejects (no declared conversion), move works in the anchor's
//! domain, and slip works for a musically anchored clip with an
//! absolute extent via the declared tempo-map conversion.
//!
//! Honestly unmodelled: clip content stretches and fades have no
//! commit semantics yet; `SplitClip`'s source-offset is kept
//! caller-supplied because tick→source-frame rate mapping is not
//! modelled — the workspace gesture reports it unsupported rather
//! than faking a conversion. `DrawClip` needs a source binding the
//! probe does not carry, so it is likewise unsupported here. Slip on
//! a clip without a declared media rate (musical extent) is a defined
//! rejection, not a guess.

use synvelis_project::{
    ClipExtent, ClipPlayback, MediaDuration, MediaTimeValue, MusicalDuration, MusicalRange,
    MusicalTick, Project, ProjectOp, SourceRange, TickTimeMapping, TimeAnchor,
};

use crate::error::WorkspaceError;
use crate::gesture::Gesture;
use crate::selection::{
    EditTimeDomain, EditTimePoint, LaneScope, SelectionDomain, floor_div_i128, media_rescale,
};
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
        let next =
            match target {
                SemanticTarget::Clip(clip_id) => project.clips.get(clip_id).map(|clip| match clip
                    .placement
                    .anchor
                {
                    TimeAnchor::Musical(_) => EditTimeDomain::Musical,
                    TimeAnchor::Absolute(_) => EditTimeDomain::Media,
                }),
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
#[allow(clippy::too_many_lines)] // flat match per edit operation
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
            let targets: Vec<SemanticTarget> = match (fix.origin, gesture.preview.position) {
                (EditTimePoint::Musical(lo), EditTimePoint::Musical(hi)) => {
                    let (lo, hi) = ordered(lo, hi);
                    let range = MusicalRange::try_new(lo, hi)?;
                    lane_scope
                        .lanes
                        .iter()
                        .filter_map(|lane| project.lanes.get(lane))
                        .flat_map(|lane| lane.clip_ids.iter().copied())
                        .filter(|clip_id| {
                            project.clips.get(clip_id).is_some_and(|c| {
                                matches!(
                                    (c.placement.anchor, c.placement.extent),
                                    (TimeAnchor::Musical(start), ClipExtent::Musical(dur))
                                        if MusicalRange::try_new(
                                            start,
                                            start
                                                .checked_add(dur.value())
                                                .unwrap_or(MusicalTick::new(i64::MAX)),
                                        )
                                        .is_ok_and(|r| r.overlaps(&range))
                                )
                            })
                        })
                        .map(SemanticTarget::Clip)
                        .collect()
                }
                (EditTimePoint::Media(lo), EditTimePoint::Media(hi)) => {
                    let (lo, hi) = if lo.lt(&hi) { (lo, hi) } else { (hi, lo) };
                    let mut targets = Vec::new();
                    for lane in lane_scope
                        .lanes
                        .iter()
                        .filter_map(|lane| project.lanes.get(lane))
                    {
                        for clip_id in lane.clip_ids.iter().copied() {
                            let Some(clip) = project.clips.get(&clip_id) else {
                                continue;
                            };
                            let (TimeAnchor::Absolute(anchor), ClipExtent::Absolute(extent)) =
                                (clip.placement.anchor, clip.placement.extent)
                            else {
                                continue;
                            };
                            if media_clip_overlaps(lo, hi, anchor, extent)? {
                                targets.push(SemanticTarget::Clip(clip_id));
                            }
                        }
                    }
                    targets
                }
                _ => {
                    return Err(WorkspaceError::MixedTimeDomains(
                        "marquee endpoints disagree on the time domain".to_string(),
                    ));
                }
            };
            Ok(CommitEffect::Selection {
                domain: SelectionDomain::Clips,
                targets,
            })
        }
        EditOperation::MoveClips => {
            let ops = move_ops(
                project,
                fix.targets.as_slice(),
                gesture.preview.delta,
                fix.origin,
            )?;
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
            let delta = gesture.preview.delta;
            let op = match (clip.placement.anchor, clip.placement.extent) {
                (TimeAnchor::Musical(start), ClipExtent::Musical(dur)) => {
                    let end = start.checked_add(dur.value())?;
                    match edge {
                        HandleKind::StartTrim => {
                            let new_start = start.checked_add(delta)?;
                            ProjectOp::Composite {
                                ops: vec![
                                    ProjectOp::MoveClip {
                                        clip_id,
                                        anchor: TimeAnchor::Musical(new_start),
                                    },
                                    ProjectOp::SetClipExtent {
                                        clip_id,
                                        extent: ClipExtent::Musical(MusicalDuration::new(
                                            end.ticks_since(new_start)?,
                                        )?),
                                    },
                                ],
                            }
                        }
                        HandleKind::EndTrim => ProjectOp::SetClipExtent {
                            clip_id,
                            extent: ClipExtent::Musical(MusicalDuration::new(
                                end.checked_add(delta)?.ticks_since(start)?,
                            )?),
                        },
                        _ => {
                            return Err(WorkspaceError::UnsupportedOperation(
                                "trim edge is not a trim handle",
                            ));
                        }
                    }
                }
                (TimeAnchor::Absolute(anchor), ClipExtent::Absolute(extent)) => {
                    media_trim_op(clip_id, anchor, extent, edge, delta, fix.origin)?
                }
                _ => {
                    // Mixed anchor/extent domains: a trim edge would
                    // need an undeclared domain conversion — defined
                    // rejection, never a quiet rewrite (ED-B-04).
                    return Err(WorkspaceError::UnsupportedOperation(
                        "trim on a mixed anchor/extent domain clip",
                    ));
                }
            };
            Ok(CommitEffect::ProjectOp(op))
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
        EditOperation::SlipClip => {
            let Some(SemanticTarget::Clip(clip_id)) = fix.primary() else {
                return Ok(CommitEffect::NoChange);
            };
            let clip = project
                .clips
                .get(&clip_id)
                .ok_or_else(|| WorkspaceError::InvalidGesture(format!("clip {clip_id} gone")))?;
            let op = slip_op(
                project,
                clip_id,
                clip.placement,
                clip.playback,
                fix.origin,
                gesture.preview.delta,
            )?;
            Ok(CommitEffect::ProjectOp(op))
        }
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
/// clip keeps its relative offset by sharing it. Media-anchored
/// clips rescale `delta` (in `origin`'s declared base) into their own
/// anchor base; a gesture/domain mismatch is an explicit error.
fn move_ops(
    project: &Project,
    targets: &[SemanticTarget],
    delta: i64,
    origin: EditTimePoint,
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
        let anchor = match (clip.placement.anchor, origin) {
            (TimeAnchor::Musical(start), EditTimePoint::Musical(_)) => {
                TimeAnchor::Musical(start.checked_add(delta)?)
            }
            (TimeAnchor::Absolute(anchor), EditTimePoint::Media(o)) => {
                let units = media_rescale(
                    delta,
                    (o.numerator(), o.denominator()),
                    (anchor.numerator(), anchor.denominator()),
                )
                .ok_or(synvelis_project::Error::Overflow)?;
                TimeAnchor::Absolute(MediaTimeValue::new(
                    anchor
                        .value()
                        .checked_add(units)
                        .ok_or(synvelis_project::Error::Overflow)?,
                    anchor.numerator(),
                    anchor.denominator(),
                )?)
            }
            _ => {
                return Err(WorkspaceError::MixedTimeDomains(
                    "clip anchor domain disagrees with the gesture domain".to_string(),
                ));
            }
        };
        ops.push(ProjectOp::MoveClip {
            clip_id: *clip_id,
            anchor,
        });
    }
    Ok(ops)
}

/// One [`ProjectOp`] for an `Absolute`×`Absolute` trim (ED-B-02).
///
/// `delta` is in `origin`'s declared base; the anchor and extent each
/// rescale it to their own declared base. `StartTrim` keeps the
/// clip's media end fixed — the anchor moves and the extent shrinks
/// (or grows on a left drag) atomically in one `Composite`.
fn media_trim_op(
    clip_id: synvelis_project::ClipId,
    anchor: MediaTimeValue,
    extent: MediaDuration,
    edge: HandleKind,
    delta: i64,
    origin: EditTimePoint,
) -> Result<ProjectOp, WorkspaceError> {
    let Some(obase) = origin.media_base() else {
        return Err(WorkspaceError::MixedTimeDomains(
            "media trim on a musical-domain gesture".to_string(),
        ));
    };
    let anchor_base = (anchor.numerator(), anchor.denominator());
    let extent_base = (extent.numerator(), extent.denominator());
    if edge == HandleKind::StartTrim {
        let anchor_delta =
            media_rescale(delta, obase, anchor_base).ok_or(synvelis_project::Error::Overflow)?;
        let new_anchor_v = anchor
            .value()
            .checked_add(anchor_delta)
            .ok_or(synvelis_project::Error::Overflow)?;
        // The media end stays fixed: the new extent is
        // `floor(extent − applied_anchor_delta)` evaluated as one
        // rational expression in the extent's declared base — a
        // single floor at the final boundary, so the committed end
        // never lands past the original one (it may sit up to one
        // extent unit short, the closest representable value).
        let numer = mul3_i128(extent.value(), anchor.denominator(), extent.numerator())
            .and_then(|l| {
                mul3_i128(anchor_delta, anchor.numerator(), extent.denominator())
                    .and_then(|r| l.checked_sub(r))
            })
            .ok_or(synvelis_project::Error::Overflow)?;
        let denom = i128::from(anchor.denominator()) * i128::from(extent.numerator());
        let new_extent_v = floor_div_i128(numer, denom).ok_or(synvelis_project::Error::Overflow)?;
        return Ok(ProjectOp::Composite {
            ops: vec![
                ProjectOp::MoveClip {
                    clip_id,
                    anchor: TimeAnchor::Absolute(MediaTimeValue::new(
                        new_anchor_v,
                        anchor.numerator(),
                        anchor.denominator(),
                    )?),
                },
                ProjectOp::SetClipExtent {
                    clip_id,
                    extent: ClipExtent::Absolute(MediaDuration::new(
                        new_extent_v,
                        extent.numerator(),
                        extent.denominator(),
                    )?),
                },
            ],
        });
    }
    if edge != HandleKind::EndTrim {
        return Err(WorkspaceError::UnsupportedOperation(
            "trim edge is not a trim handle",
        ));
    }
    let extent_delta =
        media_rescale(delta, obase, extent_base).ok_or(synvelis_project::Error::Overflow)?;
    let new_extent_v = extent
        .value()
        .checked_add(extent_delta)
        .ok_or(synvelis_project::Error::Overflow)?;
    Ok(ProjectOp::SetClipExtent {
        clip_id,
        extent: ClipExtent::Absolute(MediaDuration::new(
            new_extent_v,
            extent.numerator(),
            extent.denominator(),
        )?),
    })
}

/// `value · a · b` in `i128` — checked so extreme but legal inputs
/// surface as `None` instead of wrapping.
fn mul3_i128(value: i64, a: u32, b: u32) -> Option<i128> {
    i128::from(value)
        .checked_mul(i128::from(a))
        .and_then(|n| n.checked_mul(i128::from(b)))
}

/// Exact media-overlap test for the marquee (ED-B-05): does the clip
/// `[anchor, anchor + extent)` overlap `(lo, hi)`?
///
/// `anchor < hi` compares declared instants directly. `lo <
/// anchor + extent` compares rationals *without* materializing a
/// floored end: both sides are expressed in extent-base units and
/// compared by quotient/remainder — every intermediate stays within
/// the same `i128` bound `MediaTimeValue::lt` already relies on, and
/// the shared `extent.numerator` factor cancels out of the remainder
/// cross-product, so no declared base is ever silently converted.
///
/// # Errors
/// [`synvelis_project::Error::Overflow`] only when an intermediate
/// exceeds `i128` (declared values near the `i64`/`u32` extremes).
fn media_clip_overlaps(
    lo: MediaTimeValue,
    hi: MediaTimeValue,
    anchor: MediaTimeValue,
    extent: MediaDuration,
) -> Result<bool, WorkspaceError> {
    if !anchor.lt(&hi) {
        return Ok(false);
    }
    if !anchor.lt(&lo) {
        // lo <= anchor < anchor + extent (extents are non-empty).
        return Ok(true);
    }
    // lo > anchor: compare `lo < anchor + extent` in extent-base
    // units. `ln/ld` is `lo` in extent units; `en/ed` is the exact
    // end in extent units.
    let overflow = || WorkspaceError::Project(synvelis_project::Error::Overflow);
    let ln = mul3_i128(lo.value(), lo.numerator(), extent.denominator()).ok_or_else(overflow)?;
    let ld = i128::from(lo.denominator()) * i128::from(extent.numerator());
    let en = mul3_i128(anchor.value(), anchor.numerator(), extent.denominator())
        .and_then(|l| {
            mul3_i128(extent.value(), extent.numerator(), anchor.denominator())
                .and_then(|r| l.checked_add(r))
        })
        .ok_or_else(overflow)?;
    let ed = i128::from(anchor.denominator()) * i128::from(extent.numerator());
    let (f_l, r_l) = (ln / ld, ln % ld);
    let (f_e, r_e) = (en / ed, en % ed);
    if f_l != f_e {
        return Ok(f_l < f_e);
    }
    // Same whole part: `r_l/ld < r_e/ed` ⟺ `r_l·a.den < r_e·lo.den`
    // (the shared `extent.numerator` cancels).
    let l = r_l
        .checked_mul(i128::from(anchor.denominator()))
        .ok_or_else(overflow)?;
    let r = r_e
        .checked_mul(i128::from(lo.denominator()))
        .ok_or_else(overflow)?;
    Ok(l < r)
}

/// One [`ProjectOp::SetClipPlayback`] for a slip gesture (ED-B-03).
///
/// The extent's declared denominator is the clip's declared source
/// rate (the `validate_clip_contract` convention); the gesture delta
/// is first expressed in media frames at that rate — through the
/// tempo map for a musically anchored clip — then the mapping turns
/// it into the source-window shift (`consumed_len`: `Natural`/`Loop`
/// 1:1, `SimpleStretch` × ratio). Anchor, extent and mapping stay
/// untouched; a musically extended clip declares no rate and is a
/// defined rejection.
fn slip_op(
    project: &Project,
    clip_id: synvelis_project::ClipId,
    placement: synvelis_project::ClipPlacement,
    playback: ClipPlayback,
    origin: EditTimePoint,
    delta: i64,
) -> Result<ProjectOp, WorkspaceError> {
    let ClipExtent::Absolute(extent) = placement.extent else {
        return Err(WorkspaceError::UnsupportedOperation(
            "slip needs a declared media rate (absolute extent)",
        ));
    };
    let rate = extent.denominator();
    let delta = delta_in_media_frames(project, placement, origin, delta, rate)?;
    let consumed = playback.consumed_len(delta.unsigned_abs())?;
    let consumed = i64::try_from(consumed).map_err(|_| synvelis_project::Error::Overflow)?;
    let shift = if delta < 0 { -consumed } else { consumed };
    let region = SourceRange::try_new(
        playback.source_region.start().checked_add(shift)?,
        playback.source_region.end().checked_add(shift)?,
    )?;
    Ok(ProjectOp::SetClipPlayback {
        clip_id,
        playback: ClipPlayback {
            source_region: region,
            mapping: playback.mapping,
        },
    })
}

/// The gesture's preview `delta` expressed in media frames at `rate`.
///
/// A media gesture rescales its origin-base delta; a musical gesture
/// (musically anchored clip with an absolute extent — the only mixed
/// clip a slip can reach) converts the tick delta through the
/// project's tempo map, the declared conversion of ED-B-04.
fn delta_in_media_frames(
    project: &Project,
    placement: synvelis_project::ClipPlacement,
    origin: EditTimePoint,
    delta: i64,
    rate: u32,
) -> Result<i64, WorkspaceError> {
    match (origin, placement.anchor) {
        (EditTimePoint::Media(o), TimeAnchor::Absolute(_)) => {
            media_rescale(delta, (o.numerator(), o.denominator()), (1, rate))
                .ok_or(WorkspaceError::Project(synvelis_project::Error::Overflow))
        }
        (EditTimePoint::Musical(_), TimeAnchor::Musical(start)) => {
            let map = TickTimeMapping::new(&project.tempo_map)?;
            let before = map.tick_to_media(start, 1, rate)?.value();
            let after = map
                .tick_to_media(start.checked_add(delta)?, 1, rate)?
                .value();
            after
                .checked_sub(before)
                .ok_or(WorkspaceError::Project(synvelis_project::Error::Overflow))
        }
        _ => Err(WorkspaceError::MixedTimeDomains(
            "slip anchor domain disagrees with the gesture domain".to_string(),
        )),
    }
}
