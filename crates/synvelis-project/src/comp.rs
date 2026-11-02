//! Comping (#12 §13/§15): `TakeGroup` → Comp → `CompRegion`/`CompTransition`
//! schema, the `CompOp` operation set, and the Effective Comp projection.
//!
//! A [`Take`] is a recorded candidate on a lane; a [`TakeGroup`] is the
//! lane-side comping unit collecting comparable candidates; a [`Comp`]
//! is the adopted arrangement — an ordered, non-overlapping set of
//! [`CompRegion`]s plus independent [`CompTransition`]s that own the
//! bounded double-play at region boundaries. Raw sources stay
//! authoritative and immutable: comping state never rewrites a
//! `Source`, `Asset`, or `Take`.
//!
//! [`project_comp`] derives the Effective Comp — the reproducible
//! playback projection — and carries per-region provenance instead of
//! pretending to be a single source.
//!
//! Validation is layered: each [`CompOp`] checks its targets and
//! candidate legality in `validate`, and `ProjectController::apply`
//! re-runs [`Project::validate_references`] — the same oracle used on
//! load — on the post-apply state, so no op can commit a state the
//! loader would reject.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    controller::Operation,
    error::Error,
    id::{
        CompId, CompRegionId, CompTransitionId, IdGenerator, LaneId, SourceId, TakeGroupId, TakeId,
    },
    placement::PlaybackMapping,
    project::{Project, ProjectDomain, ProjectRange},
    range::{MediaSpan, MusicalRange, SourceRange},
    tempo::TickTimeMapping,
    time::{ProjectTime, SourceFrame},
};

// ---------------------------------------------------------------------
// Schema (POD)
// ---------------------------------------------------------------------

/// Lane-side comping unit: the set of comparable candidates over a
/// project span, plus the currently active adopted comp.
///
/// `source_rate` is the single native frame rate used to convert
/// project media lengths into source frames for handle checks — the
/// MD-A approximation until `Source` carries its own native format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeGroup {
    /// Stable ID.
    pub id: TakeGroupId,
    /// Lane this group comps on.
    pub owner_lane_id: LaneId,
    /// Project span the group covers (same domain as its regions).
    pub target_range: ProjectRange,
    /// Comparable candidates (all must live on `owner_lane_id`).
    pub take_ids: Vec<TakeId>,
    /// The adopted comp currently live on the lane.
    pub active_comp_id: Option<CompId>,
    /// Native frame rate shared by the group's candidates (`> 0`).
    pub source_rate: u32,
}

/// A comp: one adopted arrangement inside a take group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comp {
    /// Stable ID.
    pub id: CompId,
    /// Owning take group.
    pub take_group_id: TakeGroupId,
    /// Regions in play order (sorted by range start, non-overlapping).
    pub region_ids: Vec<CompRegionId>,
    /// Boundary transitions (each between adjacent regions).
    pub transition_ids: Vec<CompTransitionId>,
}

/// What a comp region plays. Raw material is referenced, never owned
/// or rewritten.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum CompCandidateRef {
    /// A recorded take. The region start reads the take's source at
    /// `source_offset`; the usable handle is the take's captured
    /// source coverage (union of its segments' source ranges).
    Take {
        /// Candidate take (must belong to the owning take group).
        take_id: TakeId,
        /// Source frame the region start maps to.
        source_offset: SourceFrame,
    },
    /// An explicit window of an existing source. `handle` bounds all
    /// reads (including transition overhang); `play_from` is the frame
    /// the region start maps to and must lie inside `handle`.
    ExistingSource {
        /// Source to read.
        source_id: SourceId,
        /// Readable handle in source frames.
        handle: SourceRange,
        /// Frame mapped to the region start (`handle` must contain it).
        play_from: SourceFrame,
    },
    /// Contributes silence — no handle required.
    Silence,
}

/// One adopted interval: a non-overlapping project span, its source
/// candidate, and the project→source mapping.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompRegion {
    /// Stable ID.
    pub id: CompRegionId,
    /// Owning comp.
    pub comp_id: CompId,
    /// Adopted project span `[start, end)`.
    pub project_range: ProjectRange,
    /// What plays inside the span.
    pub candidate: CompCandidateRef,
    /// How project media time maps onto the candidate's window.
    pub mapping: PlaybackMapping,
}

/// Fade shape selector for a transition side. Numeric parameters and
/// additional curves are deferred to later `PoCs`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum FadeShape {
    /// Linear gain ramp.
    Linear,
    /// Equal-power (cos/sin) ramp — the agreed default candidate.
    EqualPower,
}

/// Bounded double-play at the boundary between two adjacent regions:
/// the left region keeps playing past the boundary for the
/// `boundary → overlap.end` span, the right starts playing before the
/// boundary for `overlap.start → boundary`. Both handles are validated
/// against the candidates' readable windows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompTransition {
    /// Stable ID.
    pub id: CompTransitionId,
    /// Owning comp.
    pub comp_id: CompId,
    /// Region on the left of the boundary (plays the fade-out).
    pub left_region_id: CompRegionId,
    /// Region on the right (plays the fade-in).
    pub right_region_id: CompRegionId,
    /// Overlap zone `[start, end)` — must contain the boundary in its
    /// interior and stay within `[left.start, right.end]`.
    pub overlap_range: ProjectRange,
    /// Fade-out curve applied to the left region.
    pub fade_out: FadeShape,
    /// Fade-in curve applied to the right region.
    pub fade_in: FadeShape,
}

/// Comping entities inside the `Project` aggregate.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompRegistry {
    /// Take groups by ID.
    pub take_groups: BTreeMap<TakeGroupId, TakeGroup>,
    /// Comps by ID.
    pub comps: BTreeMap<CompId, Comp>,
    /// Regions by ID.
    pub regions: BTreeMap<CompRegionId, CompRegion>,
    /// Transitions by ID.
    pub transitions: BTreeMap<CompTransitionId, CompTransition>,
}

// ---------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------

/// Where a candidate reads: `play_from` is the frame the region start
/// maps to, `handle` is the union of readable source intervals.
/// `None` for `Silence`.
fn candidate_handle(
    project: &Project,
    group: &TakeGroup,
    candidate: &CompCandidateRef,
) -> Result<Option<(SourceFrame, Vec<SourceRange>)>, Error> {
    match candidate {
        CompCandidateRef::Silence => Ok(None),
        CompCandidateRef::Take {
            take_id,
            source_offset,
        } => {
            if !group.take_ids.contains(take_id) {
                return Err(Error::Validation(format!(
                    "take {take_id} is not in take group {}",
                    group.id
                )));
            }
            let take = project
                .recording
                .takes
                .get(take_id)
                .ok_or_else(|| Error::NotFound(format!("take {take_id}")))?;
            Ok(Some((*source_offset, take_coverage(project, take)?)))
        }
        CompCandidateRef::ExistingSource {
            source_id,
            handle,
            play_from,
        } => {
            if !project.sources.contains_key(source_id) {
                return Err(Error::NotFound(format!("source {source_id}")));
            }
            if handle.is_empty() {
                return Err(Error::Validation("existing-source handle is empty".into()));
            }
            if !handle.contains(*play_from) {
                return Err(Error::Validation(format!(
                    "play_from outside existing-source handle {source_id}"
                )));
            }
            Ok(Some((*play_from, vec![*handle])))
        }
    }
}

/// The take's captured source coverage: the sorted, merged union of
/// its segments' source ranges.
fn take_coverage(project: &Project, take: &crate::Take) -> Result<Vec<SourceRange>, Error> {
    let mut ranges: Vec<SourceRange> = take
        .segment_ids
        .iter()
        .map(|seg_id| {
            project
                .recording
                .segments
                .get(seg_id)
                .map(|s| s.source_range)
                .ok_or_else(|| Error::NotFound(format!("segment {seg_id}")))
        })
        .collect::<Result<_, _>>()?;
    ranges.sort_by_key(SourceRange::start);
    let mut merged: Vec<SourceRange> = Vec::with_capacity(ranges.len());
    for r in ranges {
        if let Some(last) = merged.last_mut()
            && r.start() <= last.end()
        {
            let end = last.end().max(r.end());
            *last = SourceRange::try_new(last.start(), end)?;
            continue;
        }
        merged.push(r);
    }
    Ok(merged)
}

/// The union interval containing `pos`, if any.
fn union_interval(ranges: &[SourceRange], pos: SourceFrame) -> Option<SourceRange> {
    ranges.iter().copied().find(|r| r.contains(pos))
}

/// Media-frame length of a range at the take group's source rate.
fn range_frames(project: &Project, group: &TakeGroup, range: &ProjectRange) -> Result<i64, Error> {
    let map = TickTimeMapping::new(&project.tempo_map).ok();
    range
        .media_frames(map.as_ref(), group.source_rate)
        .and_then(|v| if v < 0 { Err(Error::Negative) } else { Ok(v) })
}

/// Media frames `value` mapped to consumed source frames under
/// `mapping`, rejecting negative input.
fn consumed(mapping: &PlaybackMapping, media_frames: i64) -> Result<u64, Error> {
    let frames = u64::try_from(media_frames).map_err(|_| Error::Negative)?;
    match mapping {
        PlaybackMapping::Natural | PlaybackMapping::Loop => Ok(frames),
        PlaybackMapping::SimpleStretch(ratio) => ratio.apply(frames),
    }
}

/// Checks a candidate window `[play_from, play_from + consumed)` plus
/// transition overhang `pre`/`post` extra frames against the readable
/// union intervals. `Loop` still requires `play_from` inside a handle
/// interval (its repeat unit is that interval); only the overhang
/// bounds are waived because the wrap stays inside the unit. `Silence`
/// needs nothing.
fn check_window(
    project: &Project,
    group: &TakeGroup,
    candidate: &CompCandidateRef,
    mapping: &PlaybackMapping,
    media_len: i64,
    pre_extra: i64,
    post_extra: i64,
) -> Result<(), Error> {
    let Some((play_from, handle)) = candidate_handle(project, group, candidate)? else {
        return Ok(());
    };
    let base = consumed(mapping, media_len)?;
    if base == 0 {
        return Err(Error::Validation("region consumes no source frames".into()));
    }
    let interval = union_interval(&handle, play_from)
        .ok_or_else(|| Error::Validation("play_from outside candidate handle".into()))?;
    if matches!(mapping, PlaybackMapping::Loop) {
        return Ok(());
    }
    let need_start = play_from
        .checked_sub(i64::try_from(consumed(mapping, pre_extra)?).map_err(|_| Error::Overflow)?)
        .map_err(|_| Error::Negative)?;
    let need_end = play_from
        .checked_add(
            i64::try_from(
                base.checked_add(consumed(mapping, post_extra)?)
                    .ok_or(Error::Overflow)?,
            )
            .map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::Overflow)?;
    if need_start < interval.start() || need_end > interval.end() {
        return Err(Error::Validation(format!(
            "source handle insufficient: need [{}, {}), have [{}, {})",
            need_start.value(),
            need_end.value(),
            interval.start().value(),
            interval.end().value()
        )));
    }
    Ok(())
}

/// Region order inside a comp: `region_ids` sorted by range start is
/// the persisted order; callers also verify it is non-overlapping.
fn sorted_regions<'p>(
    project: &'p Project,
    comp: &crate::Comp,
) -> Result<Vec<&'p CompRegion>, Error> {
    comp.region_ids
        .iter()
        .map(|id| {
            project
                .comp
                .regions
                .get(id)
                .ok_or_else(|| Error::NotFound(format!("region {id}")))
        })
        .collect()
}

/// `left.end == right.start` expressed as a project-time point, in the
/// regions' shared domain. `None` when the regions do not touch or
/// their domains differ.
fn boundary_of(left: &CompRegion, right: &CompRegion) -> Option<ProjectTime> {
    if left.project_range.domain() != right.project_range.domain() {
        return None;
    }
    let left_end = left.project_range.end_time();
    let right_start = right.project_range.start_time();
    match (left_end, right_start) {
        (ProjectTime::Musical(a), ProjectTime::Musical(b)) if a == b => {
            Some(ProjectTime::Musical(a))
        }
        (ProjectTime::Media(a), ProjectTime::Media(b)) if !a.lt(&b) && !b.lt(&a) => {
            Some(ProjectTime::Media(a))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------
// Registry-wide invariant oracle
// ---------------------------------------------------------------------

/// Validates every comping invariant. Used by
/// `Project::validate_references` on load and re-run inside each
/// mutating [`CompOp`] `apply` so a buggy incremental check can never
/// persist an invalid registry.
///
/// # Errors
/// [`Error::NotFound`]/[`Error::WrongOwner`]/[`Error::Validation`].
pub fn validate_registry(project: &Project) -> Result<(), Error> {
    for (key, group) in &project.comp.take_groups {
        key_id(key, group.id)?;
        validate_group(project, group)?;
    }
    for (key, comp) in &project.comp.comps {
        key_id(key, comp.id)?;
        validate_comp(project, comp)?;
    }
    for (key, region) in &project.comp.regions {
        key_id(key, region.id)?;
        let comp = project
            .comp
            .comps
            .get(&region.comp_id)
            .ok_or_else(|| Error::NotFound(format!("comp {}", region.comp_id)))?;
        if !comp.region_ids.contains(&region.id) {
            return Err(Error::WrongOwner(format!(
                "region {} not listed in comp {}",
                region.id, comp.id
            )));
        }
    }
    for (key, transition) in &project.comp.transitions {
        key_id(key, transition.id)?;
        let comp = project
            .comp
            .comps
            .get(&transition.comp_id)
            .ok_or_else(|| Error::NotFound(format!("comp {}", transition.comp_id)))?;
        if !comp.transition_ids.contains(&transition.id) {
            return Err(Error::WrongOwner(format!(
                "transition {} not listed in comp {}",
                transition.id, comp.id
            )));
        }
        if !project
            .comp
            .regions
            .contains_key(&transition.left_region_id)
            || !project
                .comp
                .regions
                .contains_key(&transition.right_region_id)
        {
            return Err(Error::NotFound(format!(
                "transition {} references a missing region",
                transition.id
            )));
        }
    }
    Ok(())
}

/// Key↔`id` coherence for a registry map entry.
fn key_id<K: PartialEq + std::fmt::Display + Copy>(key: &K, id: K) -> Result<(), Error> {
    if *key != id {
        return Err(Error::Validation(format!(
            "registry key {key} does not match entity id {id}"
        )));
    }
    Ok(())
}

/// One take group's invariants: lane ownership, take coverage, and
/// `active_comp_id` back-reference consistency.
fn validate_group(project: &Project, group: &TakeGroup) -> Result<(), Error> {
    if group.source_rate == 0 {
        return Err(Error::Validation(format!(
            "take group {} has zero source_rate",
            group.id
        )));
    }
    if group.target_range.is_empty() {
        return Err(Error::Validation(format!(
            "take group {} has empty target range",
            group.id
        )));
    }
    if !project.lanes.contains_key(&group.owner_lane_id) {
        return Err(Error::NotFound(format!("lane {}", group.owner_lane_id)));
    }
    if group.take_ids.is_empty() {
        return Err(Error::Validation(format!(
            "take group {} has no takes",
            group.id
        )));
    }
    for take_id in &group.take_ids {
        let take = project
            .recording
            .takes
            .get(take_id)
            .ok_or_else(|| Error::NotFound(format!("take {take_id}")))?;
        if take.lane_id != group.owner_lane_id {
            return Err(Error::WrongOwner(format!(
                "take {take_id} lane != group {} lane",
                group.id
            )));
        }
    }
    if let Some(comp_id) = group.active_comp_id {
        let comp = project
            .comp
            .comps
            .get(&comp_id)
            .ok_or_else(|| Error::NotFound(format!("comp {comp_id}")))?;
        if comp.take_group_id != group.id {
            return Err(Error::WrongOwner(format!(
                "active comp {comp_id} belongs to another take group"
            )));
        }
    }
    Ok(())
}

/// One comp's invariants: ordered non-overlapping regions inside the
/// group's target range, source handles per region, and each listed
/// transition.
fn validate_comp(project: &Project, comp: &Comp) -> Result<(), Error> {
    let group = project
        .comp
        .take_groups
        .get(&comp.take_group_id)
        .ok_or_else(|| Error::NotFound(format!("take group {}", comp.take_group_id)))?;
    let regions = sorted_regions(project, comp)?;
    let mut prev_end: Option<ProjectTime> = None;
    for region in &regions {
        if region.comp_id != comp.id {
            return Err(Error::WrongOwner(format!(
                "region {} listed in comp {}",
                region.id, comp.id
            )));
        }
        let range = &region.project_range;
        if range.is_empty() {
            return Err(Error::Validation(format!(
                "region {} has empty range",
                region.id
            )));
        }
        if range.domain() != group.target_range.domain()
            || !group.target_range.contains_range(range)
        {
            return Err(Error::Validation(format!(
                "region {} outside take group {} target range",
                region.id, group.id
            )));
        }
        if let Some(prev) = &prev_end {
            let no_overlap = match (prev, &range.start_time()) {
                (ProjectTime::Musical(e), ProjectTime::Musical(s)) => s >= e,
                (ProjectTime::Media(e), ProjectTime::Media(s)) => !s.lt(e),
                _ => false,
            };
            if !no_overlap {
                return Err(Error::Validation(format!(
                    "region {} overlaps an earlier region or is out of order",
                    region.id
                )));
            }
        }
        prev_end = Some(range.end_time());
        let media_len = range_frames(project, group, range)?;
        check_window(
            project,
            group,
            &region.candidate,
            &region.mapping,
            media_len,
            0,
            0,
        )?;
    }
    for transition_id in &comp.transition_ids {
        let transition = project
            .comp
            .transitions
            .get(transition_id)
            .ok_or_else(|| Error::NotFound(format!("transition {transition_id}")))?;
        if transition.comp_id != comp.id {
            return Err(Error::WrongOwner(format!(
                "transition {} listed in comp {}",
                transition.id, comp.id
            )));
        }
        validate_transition(project, group, comp, &regions, transition)?;
    }
    Ok(())
}

/// One transition's invariants inside its comp: adjacency, shared
/// boundary, overlap containment, no colliding overlap zones, and
/// source handles for the extended reads on both sides.
fn validate_transition(
    project: &Project,
    group: &TakeGroup,
    comp: &Comp,
    regions: &[&CompRegion],
    transition: &CompTransition,
) -> Result<(), Error> {
    let left_idx = regions
        .iter()
        .position(|r| r.id == transition.left_region_id)
        .ok_or_else(|| Error::NotFound(format!("region {}", transition.left_region_id)))?;
    let right_idx = regions
        .iter()
        .position(|r| r.id == transition.right_region_id)
        .ok_or_else(|| Error::NotFound(format!("region {}", transition.right_region_id)))?;
    if left_idx + 1 != right_idx {
        return Err(Error::Validation(format!(
            "transition {} regions are not adjacent",
            transition.id
        )));
    }
    let left = regions[left_idx];
    let right = regions[right_idx];
    let boundary = boundary_of(left, right).ok_or_else(|| {
        Error::Validation(format!(
            "transition {} regions do not share a boundary",
            transition.id
        ))
    })?;
    let overlap = &transition.overlap_range;
    if overlap.is_empty() || overlap.domain() != left.project_range.domain() {
        return Err(Error::Validation(format!(
            "transition {} overlap has wrong domain or is empty",
            transition.id
        )));
    }
    let strictly_inside =
        point_lt(&overlap.start_time(), &boundary) && point_lt(&boundary, &overlap.end_time());
    if !strictly_inside {
        return Err(Error::Validation(format!(
            "transition {} overlap does not strictly contain the boundary",
            transition.id
        )));
    }
    // Overlap must stay within [left.start, right.end].
    let span_ok = {
        let within = |a: &ProjectTime, b: &ProjectTime| !point_lt(a, b);
        within(&overlap.start_time(), &left.project_range.start_time())
            && within(&right.project_range.end_time(), &overlap.end_time())
    };
    if !span_ok {
        return Err(Error::Validation(format!(
            "transition {} overlap escapes the adjacent regions",
            transition.id
        )));
    }
    for other_id in &comp.transition_ids {
        if *other_id == transition.id {
            continue;
        }
        let other = project
            .comp
            .transitions
            .get(other_id)
            .ok_or_else(|| Error::NotFound(format!("transition {other_id}")))?;
        if other.comp_id == comp.id && transition.overlap_range.overlaps(&other.overlap_range) {
            return Err(Error::Validation(format!(
                "transition {} overlap collides with {}",
                transition.id, other.id
            )));
        }
    }
    // Handles: the left region keeps playing past the boundary for
    // `boundary → overlap.end`; the right plays early for
    // `overlap.start → boundary`.
    let post_extra = {
        let post_range = sub_range(&boundary, &overlap.end_time(), overlap.domain())?;
        range_frames(project, group, &post_range)?
    };
    let pre_extra = {
        let pre_range = sub_range(&overlap.start_time(), &boundary, overlap.domain())?;
        range_frames(project, group, &pre_range)?
    };
    let left_len = range_frames(project, group, &left.project_range)?;
    let right_len = range_frames(project, group, &right.project_range)?;
    check_window(
        project,
        group,
        &left.candidate,
        &left.mapping,
        left_len,
        0,
        post_extra,
    )?;
    check_window(
        project,
        group,
        &right.candidate,
        &right.mapping,
        right_len,
        pre_extra,
        0,
    )?;
    Ok(())
}

/// `<` on project-time points inside one domain.
fn point_lt(a: &ProjectTime, b: &ProjectTime) -> bool {
    match (a, b) {
        (ProjectTime::Musical(x), ProjectTime::Musical(y)) => x < y,
        (ProjectTime::Media(x), ProjectTime::Media(y)) => x.lt(y),
        _ => false,
    }
}

/// `[from, to)` as a `ProjectRange` in `domain`. Caller guarantees
/// `from <= to` in that domain.
fn sub_range(
    from: &ProjectTime,
    to: &ProjectTime,
    domain: ProjectDomain,
) -> Result<ProjectRange, Error> {
    match (domain, from, to) {
        (ProjectDomain::Musical, ProjectTime::Musical(a), ProjectTime::Musical(b)) => {
            Ok(ProjectRange::Musical(MusicalRange::try_new(*a, *b)?))
        }
        (ProjectDomain::Media, ProjectTime::Media(a), ProjectTime::Media(b)) => {
            Ok(ProjectRange::Media(MediaSpan::try_new(*a, *b)?))
        }
        _ => Err(Error::Validation("mixed-domain sub range".into())),
    }
}

// ---------------------------------------------------------------------
// CompOp: comping operations under the single writer
// ---------------------------------------------------------------------

/// Comping operations. Every variant is validated against the live
/// project before apply; mutations re-run [`validate_registry`] so the
/// persisted registry always satisfies the invariants.
#[derive(Clone, Debug)]
pub enum CompOp {
    /// Register a take group on a lane.
    RegisterTakeGroup {
        /// Lane the group comps on.
        lane_id: LaneId,
        /// Project span the group covers.
        target_range: ProjectRange,
        /// Comparable candidates (all on `lane_id`).
        take_ids: Vec<TakeId>,
        /// Shared native frame rate for handle conversion (`> 0`).
        source_rate: u32,
    },
    /// Remove a take group — only while it has no comps.
    UnregisterTakeGroup {
        /// Target group.
        take_group_id: TakeGroupId,
    },
    /// Create an empty comp in a group (not auto-activated).
    CreateComp {
        /// Owning group.
        take_group_id: TakeGroupId,
    },
    /// Remove a comp with its regions and transitions; clears the
    /// group's `active_comp_id` when pointed at it.
    RemoveComp {
        /// Target comp.
        comp_id: CompId,
    },
    /// Point a group at a comp it owns (or clear with `None`).
    SetActiveComp {
        /// Target group.
        take_group_id: TakeGroupId,
        /// Comp to activate.
        comp_id: Option<CompId>,
    },
    /// Add an adopted region (non-overlapping, inside target range).
    AddCompRegion {
        /// Owning comp.
        comp_id: CompId,
        /// Adopted span `[start, end)`.
        project_range: ProjectRange,
        /// What plays inside the span.
        candidate: CompCandidateRef,
        /// Project→source mapping.
        mapping: PlaybackMapping,
    },
    /// Re-adopt a region to another candidate.
    SetCompRegionCandidate {
        /// Target region.
        region_id: CompRegionId,
        /// New candidate.
        candidate: CompCandidateRef,
    },
    /// Resize a region. Rejected while transitions reference it —
    /// adjust transitions first so the shared-boundary invariant
    /// cannot be stranded.
    SetCompRegionRange {
        /// Target region.
        region_id: CompRegionId,
        /// New span.
        project_range: ProjectRange,
    },
    /// Remove a region and the transitions that touch it.
    RemoveCompRegion {
        /// Target region.
        region_id: CompRegionId,
    },
    /// Add a boundary transition between two adjacent regions.
    AddCompTransition {
        /// Region left of the boundary.
        left_region_id: CompRegionId,
        /// Region right of the boundary.
        right_region_id: CompRegionId,
        /// Overlap zone (must strictly contain the boundary).
        overlap_range: ProjectRange,
        /// Left fade-out curve.
        fade_out: FadeShape,
        /// Right fade-in curve.
        fade_in: FadeShape,
    },
    /// Adjust a transition's overlap and curves.
    SetCompTransition {
        /// Target transition.
        transition_id: CompTransitionId,
        /// New overlap zone.
        overlap_range: ProjectRange,
        /// New left fade-out curve.
        fade_out: FadeShape,
        /// New right fade-in curve.
        fade_in: FadeShape,
    },
    /// Remove a transition.
    RemoveCompTransition {
        /// Target transition.
        transition_id: CompTransitionId,
    },
}

/// Resolves group + sorted regions for a comp (shared by several ops).
fn comp_parts(
    project: &Project,
    comp_id: CompId,
) -> Result<(&TakeGroup, &Comp, Vec<&CompRegion>), Error> {
    let comp = project
        .comp
        .comps
        .get(&comp_id)
        .ok_or_else(|| Error::NotFound(format!("comp {comp_id}")))?;
    let group = project
        .comp
        .take_groups
        .get(&comp.take_group_id)
        .ok_or_else(|| Error::NotFound(format!("take group {}", comp.take_group_id)))?;
    let regions = sorted_regions(project, comp)?;
    Ok((group, comp, regions))
}

/// Checks a would-be region legality inside a comp: domain, target
/// containment, non-overlap with `existing`, and the candidate window.
fn check_region(
    project: &Project,
    group: &TakeGroup,
    existing: &[&CompRegion],
    range: &ProjectRange,
    candidate: &CompCandidateRef,
    mapping: &PlaybackMapping,
    ignore: Option<CompRegionId>,
) -> Result<(), Error> {
    if range.is_empty() {
        return Err(Error::Validation("region range is empty".into()));
    }
    if range.domain() != group.target_range.domain() || !group.target_range.contains_range(range) {
        return Err(Error::Validation(format!(
            "region range outside take group {} target range",
            group.id
        )));
    }
    for other in existing {
        if Some(other.id) == ignore {
            continue;
        }
        if range.overlaps(&other.project_range) {
            return Err(Error::Validation(format!(
                "region range overlaps region {}",
                other.id
            )));
        }
    }
    let media_len = range_frames(project, group, range)?;
    check_window(project, group, candidate, mapping, media_len, 0, 0)
}

impl Operation for CompOp {
    #[allow(clippy::too_many_lines)] // flat match per op variant
    fn validate(&self, project: &Project) -> Result<(), Error> {
        match self {
            Self::RegisterTakeGroup {
                lane_id,
                target_range,
                take_ids,
                source_rate,
            } => {
                if !project.lanes.contains_key(lane_id) {
                    return Err(Error::NotFound(format!("lane {lane_id}")));
                }
                if *source_rate == 0 {
                    return Err(Error::Validation("source_rate is zero".into()));
                }
                if target_range.is_empty() {
                    return Err(Error::Validation("take group target range is empty".into()));
                }
                if take_ids.is_empty() {
                    return Err(Error::Validation("take group requires takes".into()));
                }
                for take_id in take_ids {
                    let take = project
                        .recording
                        .takes
                        .get(take_id)
                        .ok_or_else(|| Error::NotFound(format!("take {take_id}")))?;
                    if take.lane_id != *lane_id {
                        return Err(Error::WrongOwner(format!(
                            "take {take_id} is on lane {}",
                            take.lane_id
                        )));
                    }
                }
                Ok(())
            }
            Self::UnregisterTakeGroup { take_group_id } => {
                if project
                    .comp
                    .comps
                    .values()
                    .any(|c| c.take_group_id == *take_group_id)
                {
                    return Err(Error::Validation(format!(
                        "take group {take_group_id} still has comps"
                    )));
                }
                project
                    .comp
                    .take_groups
                    .contains_key(take_group_id)
                    .then_some(())
                    .ok_or_else(|| Error::NotFound(format!("take group {take_group_id}")))
            }
            Self::CreateComp { take_group_id } => project
                .comp
                .take_groups
                .contains_key(take_group_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("take group {take_group_id}"))),
            Self::RemoveComp { comp_id } => project
                .comp
                .comps
                .contains_key(comp_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("comp {comp_id}"))),
            Self::SetActiveComp {
                take_group_id,
                comp_id,
            } => {
                if !project.comp.take_groups.contains_key(take_group_id) {
                    return Err(Error::NotFound(format!("take group {take_group_id}")));
                }
                if let Some(id) = comp_id {
                    let comp = project
                        .comp
                        .comps
                        .get(id)
                        .ok_or_else(|| Error::NotFound(format!("comp {id}")))?;
                    if comp.take_group_id != *take_group_id {
                        return Err(Error::WrongOwner(format!(
                            "comp {id} belongs to another take group"
                        )));
                    }
                }
                Ok(())
            }
            Self::AddCompRegion {
                comp_id,
                project_range,
                candidate,
                mapping,
            } => {
                let (group, _comp, regions) = comp_parts(project, *comp_id)?;
                check_region(
                    project,
                    group,
                    &regions,
                    project_range,
                    candidate,
                    mapping,
                    None,
                )
            }
            Self::SetCompRegionCandidate {
                region_id,
                candidate,
            } => {
                let region = project
                    .comp
                    .regions
                    .get(region_id)
                    .ok_or_else(|| Error::NotFound(format!("region {region_id}")))?;
                let (group, _comp, _regions) = comp_parts(project, region.comp_id)?;
                let media_len = range_frames(project, group, &region.project_range)?;
                check_window(project, group, candidate, &region.mapping, media_len, 0, 0)
            }
            Self::SetCompRegionRange {
                region_id,
                project_range,
            } => {
                let region = project
                    .comp
                    .regions
                    .get(region_id)
                    .ok_or_else(|| Error::NotFound(format!("region {region_id}")))?;
                let comp = project
                    .comp
                    .comps
                    .get(&region.comp_id)
                    .ok_or_else(|| Error::NotFound(format!("comp {}", region.comp_id)))?;
                let attached = comp.transition_ids.iter().any(|tid| {
                    project.comp.transitions.get(tid).is_some_and(|t| {
                        t.left_region_id == *region_id || t.right_region_id == *region_id
                    })
                });
                if attached {
                    return Err(Error::Validation(format!(
                        "region {region_id} has transitions; adjust them first"
                    )));
                }
                let (group, _comp, regions) = comp_parts(project, region.comp_id)?;
                check_region(
                    project,
                    group,
                    &regions,
                    project_range,
                    &region.candidate,
                    &region.mapping,
                    Some(*region_id),
                )
            }
            Self::RemoveCompRegion { region_id } => project
                .comp
                .regions
                .contains_key(region_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("region {region_id}"))),
            Self::AddCompTransition {
                left_region_id,
                right_region_id,
                overlap_range,
                ..
            } => {
                let left = project
                    .comp
                    .regions
                    .get(left_region_id)
                    .ok_or_else(|| Error::NotFound(format!("region {left_region_id}")))?;
                let right = project
                    .comp
                    .regions
                    .get(right_region_id)
                    .ok_or_else(|| Error::NotFound(format!("region {right_region_id}")))?;
                if left.comp_id != right.comp_id {
                    return Err(Error::WrongOwner(
                        "transition regions belong to different comps".into(),
                    ));
                }
                let (group, comp, regions) = comp_parts(project, left.comp_id)?;
                let candidate = CompTransition {
                    id: CompTransitionId::new(u128::MAX),
                    comp_id: comp.id,
                    left_region_id: *left_region_id,
                    right_region_id: *right_region_id,
                    overlap_range: *overlap_range,
                    fade_out: FadeShape::Linear,
                    fade_in: FadeShape::Linear,
                };
                validate_transition(project, group, comp, &regions, &candidate)
            }
            Self::SetCompTransition {
                transition_id,
                overlap_range,
                ..
            } => {
                let transition = project
                    .comp
                    .transitions
                    .get(transition_id)
                    .ok_or_else(|| Error::NotFound(format!("transition {transition_id}")))?;
                let (group, comp, regions) = comp_parts(project, transition.comp_id)?;
                let mut candidate = transition.clone();
                candidate.overlap_range = *overlap_range;
                validate_transition(project, group, comp, &regions, &candidate)
            }
            Self::RemoveCompTransition { transition_id } => project
                .comp
                .transitions
                .contains_key(transition_id)
                .then_some(())
                .ok_or_else(|| Error::NotFound(format!("transition {transition_id}"))),
        }
    }

    #[allow(clippy::too_many_lines)] // flat match per op variant
    fn apply(&self, project: &mut Project, ids: &mut dyn IdGenerator) -> Result<(), Error> {
        match self {
            Self::RegisterTakeGroup {
                lane_id,
                target_range,
                take_ids,
                source_rate,
            } => {
                let id = TakeGroupId::new(ids.next());
                project.comp.take_groups.insert(
                    id,
                    TakeGroup {
                        id,
                        owner_lane_id: *lane_id,
                        target_range: *target_range,
                        take_ids: take_ids.clone(),
                        active_comp_id: None,
                        source_rate: *source_rate,
                    },
                );
            }
            Self::UnregisterTakeGroup { take_group_id } => {
                project.comp.take_groups.remove(take_group_id);
            }
            Self::CreateComp { take_group_id } => {
                let id = CompId::new(ids.next());
                project.comp.comps.insert(
                    id,
                    Comp {
                        id,
                        take_group_id: *take_group_id,
                        region_ids: Vec::new(),
                        transition_ids: Vec::new(),
                    },
                );
            }
            Self::RemoveComp { comp_id } => {
                if let Some(comp) = project.comp.comps.remove(comp_id) {
                    for rid in comp.region_ids {
                        project.comp.regions.remove(&rid);
                    }
                    for tid in comp.transition_ids {
                        project.comp.transitions.remove(&tid);
                    }
                    if let Some(group) = project.comp.take_groups.get_mut(&comp.take_group_id)
                        && group.active_comp_id == Some(*comp_id)
                    {
                        group.active_comp_id = None;
                    }
                }
            }
            Self::SetActiveComp {
                take_group_id,
                comp_id,
            } => {
                if let Some(group) = project.comp.take_groups.get_mut(take_group_id) {
                    group.active_comp_id = *comp_id;
                }
            }
            Self::AddCompRegion {
                comp_id,
                project_range,
                candidate,
                mapping,
            } => {
                let id = CompRegionId::new(ids.next());
                project.comp.regions.insert(
                    id,
                    CompRegion {
                        id,
                        comp_id: *comp_id,
                        project_range: *project_range,
                        candidate: candidate.clone(),
                        mapping: *mapping,
                    },
                );
                if let Some(comp) = project.comp.comps.get_mut(comp_id) {
                    comp.region_ids.push(id);
                }
                resort_regions(project, *comp_id);
            }
            Self::SetCompRegionCandidate {
                region_id,
                candidate,
            } => {
                if let Some(region) = project.comp.regions.get_mut(region_id) {
                    region.candidate = candidate.clone();
                }
            }
            Self::SetCompRegionRange {
                region_id,
                project_range,
            } => {
                if let Some(region) = project.comp.regions.get_mut(region_id) {
                    region.project_range = *project_range;
                }
                if let Some(comp_id) = project.comp.regions.get(region_id).map(|r| r.comp_id) {
                    resort_regions(project, comp_id);
                }
            }
            Self::RemoveCompRegion { region_id } => {
                if let Some(region) = project.comp.regions.remove(region_id)
                    && let Some(comp) = project.comp.comps.get_mut(&region.comp_id)
                {
                    comp.region_ids.retain(|id| id != region_id);
                    let orphaned: Vec<_> = comp
                        .transition_ids
                        .iter()
                        .copied()
                        .filter(|tid| {
                            project.comp.transitions.get(tid).is_some_and(|t| {
                                t.left_region_id == *region_id || t.right_region_id == *region_id
                            })
                        })
                        .collect();
                    for tid in orphaned {
                        comp.transition_ids.retain(|id| id != &tid);
                        project.comp.transitions.remove(&tid);
                    }
                }
            }
            Self::AddCompTransition {
                left_region_id,
                right_region_id,
                overlap_range,
                fade_out,
                fade_in,
            } => {
                let left = project.comp.regions.get(left_region_id);
                let Some(comp_id) = left.map(|r| r.comp_id) else {
                    return Err(Error::NotFound(format!("region {left_region_id}")));
                };
                let id = CompTransitionId::new(ids.next());
                project.comp.transitions.insert(
                    id,
                    CompTransition {
                        id,
                        comp_id,
                        left_region_id: *left_region_id,
                        right_region_id: *right_region_id,
                        overlap_range: *overlap_range,
                        fade_out: *fade_out,
                        fade_in: *fade_in,
                    },
                );
                if let Some(comp) = project.comp.comps.get_mut(&comp_id) {
                    comp.transition_ids.push(id);
                }
            }
            Self::SetCompTransition {
                transition_id,
                overlap_range,
                fade_out,
                fade_in,
            } => {
                if let Some(transition) = project.comp.transitions.get_mut(transition_id) {
                    transition.overlap_range = *overlap_range;
                    transition.fade_out = *fade_out;
                    transition.fade_in = *fade_in;
                }
            }
            Self::RemoveCompTransition { transition_id } => {
                if let Some(transition) = project.comp.transitions.remove(transition_id)
                    && let Some(comp) = project.comp.comps.get_mut(&transition.comp_id)
                {
                    comp.transition_ids.retain(|id| id != transition_id);
                }
            }
        }
        Ok(())
    }
}

/// Total order on project-time points: domain first (musical before
/// media — regions in one comp always share a domain, so the
/// cross-domain arms only make the order total), then exact position
/// inside the domain via `lt`.
fn point_cmp(a: &ProjectTime, b: &ProjectTime) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (ProjectTime::Musical(x), ProjectTime::Musical(y)) => x.cmp(y),
        (ProjectTime::Media(x), ProjectTime::Media(y)) => {
            if x.lt(y) {
                Ordering::Less
            } else if y.lt(x) {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }
        (ProjectTime::Musical(_), ProjectTime::Media(_)) => Ordering::Less,
        (ProjectTime::Media(_), ProjectTime::Musical(_)) => Ordering::Greater,
    }
}

/// Re-sorts one comp's `region_ids` by `project_range` start. The
/// list is taken out first so the comparator can borrow `project`
/// immutably while no mutable borrow is live.
fn resort_regions(project: &mut Project, comp_id: CompId) {
    let Some(comp) = project.comp.comps.get_mut(&comp_id) else {
        return;
    };
    let mut ids = std::mem::take(&mut comp.region_ids);
    ids.sort_by(|a, b| region_start_cmp(project, *a, *b));
    if let Some(comp) = project.comp.comps.get_mut(&comp_id) {
        comp.region_ids = ids;
    }
}

/// Orders two regions by `project_range` start (exact media ordering,
/// not raw `value` — bases may differ).
fn region_start_cmp(project: &Project, a: CompRegionId, b: CompRegionId) -> std::cmp::Ordering {
    let start = |id| {
        project
            .comp
            .regions
            .get(&id)
            .map(|r| r.project_range.start_time())
    };
    match (start(a), start(b)) {
        (Some(x), Some(y)) => point_cmp(&x, &y),
        _ => std::cmp::Ordering::Equal,
    }
}

// ---------------------------------------------------------------------
// Effective Comp projection (#12 §15)
// ---------------------------------------------------------------------

/// A resolved read of one source window under one mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceBinding {
    /// The source read.
    pub source_id: SourceId,
    /// Source frames read (`start` = region-start frame).
    pub window: SourceRange,
    /// Mapping applied across the window.
    pub mapping: PlaybackMapping,
}

/// Where a projected segment's material comes from — the provenance
/// an editor can display; never flattened into a single source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompProvenance {
    /// Adopted from a recorded take.
    Take(TakeId),
    /// Adopted from an existing source window.
    ExistingSource(SourceId),
    /// Contributes silence.
    Silence,
}

/// One adopted base segment of the Effective Comp.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveSegment {
    /// The comp region it was derived from.
    pub region_id: CompRegionId,
    /// Adopted project span `[start, end)`.
    pub project_range: ProjectRange,
    /// Source read for the span (`None` for silence).
    pub binding: Option<SourceBinding>,
    /// Material provenance.
    pub provenance: CompProvenance,
}

/// One boundary transition of the Effective Comp: the bounded
/// double-play zone with each side's extended source read.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveTransition {
    /// The comp transition it was derived from.
    pub transition_id: CompTransitionId,
    /// Switch point (`left.end == right.start`).
    pub boundary: ProjectTime,
    /// Bounded overlap zone `[start, end)`.
    pub overlap_range: ProjectRange,
    /// Left region's read extended through `boundary → overlap.end`
    /// (`None` for silence).
    pub left: Option<SourceBinding>,
    /// Right region's read extended through `overlap.start → boundary`.
    pub right: Option<SourceBinding>,
    /// Left fade-out curve.
    pub fade_out: FadeShape,
    /// Right fade-in curve.
    pub fade_in: FadeShape,
}

/// The Effective Comp: the reproducible playback projection derived
/// from a comp's regions and transitions. It is **not** a single
/// source — every segment keeps its provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveComp {
    /// The comp projected.
    pub comp_id: CompId,
    /// Ordered, non-overlapping adopted segments.
    pub segments: Vec<EffectiveSegment>,
    /// Bounded double-play zones at region boundaries.
    pub transitions: Vec<EffectiveTransition>,
}

impl EffectiveComp {
    /// The unique adopted segment containing `pos` (the non-overlap
    /// invariant makes this `Some` at most once). Boundary positions
    /// belong to the right-hand segment by `[start, end)` semantics.
    #[must_use]
    pub fn segment_at(&self, pos: &ProjectTime) -> Option<&EffectiveSegment> {
        self.segments
            .iter()
            .find(|s| s.project_range.contains_time(pos))
    }

    /// The unique transition whose overlap zone contains `pos`
    /// (overlap zones are non-overlapping by validation).
    #[must_use]
    pub fn transition_at(&self, pos: &ProjectTime) -> Option<&EffectiveTransition> {
        self.transitions
            .iter()
            .find(|t| t.overlap_range.contains_time(pos))
    }
}

/// One candidate's resolved base window. `Natural`/`SimpleStretch`
/// read `[play_from, play_from + consumed)`; `Loop` repeats the union
/// interval containing `play_from` — the repeat unit is bounded by the
/// candidate's readable handle, never by the project extent.
fn base_window(
    project: &Project,
    group: &TakeGroup,
    candidate: &CompCandidateRef,
    mapping: &PlaybackMapping,
    media_len: i64,
) -> Result<Option<(SourceId, SourceRange)>, Error> {
    let Some((play_from, handle)) = candidate_handle(project, group, candidate)? else {
        return Ok(None);
    };
    let source_id = match candidate {
        CompCandidateRef::Take { take_id, .. } => {
            project
                .recording
                .takes
                .get(take_id)
                .ok_or_else(|| Error::NotFound(format!("take {take_id}")))?
                .source_id
        }
        CompCandidateRef::ExistingSource { source_id, .. } => *source_id,
        CompCandidateRef::Silence => unreachable!("silence has no handle"),
    };
    let window = if matches!(mapping, PlaybackMapping::Loop) {
        union_interval(&handle, play_from)
            .ok_or_else(|| Error::Validation("play_from outside candidate handle".into()))?
    } else {
        let len = consumed(mapping, media_len)?;
        bound_window(play_from, len)?
    };
    Ok(Some((source_id, window)))
}

/// Derives the Effective Comp — deterministic and non-destructive:
/// the same project always yields the same projection and nothing is
/// mutated.
///
/// # Errors
/// Any [`Error`] when the comp or its references are invalid.
pub fn project_comp(project: &Project, comp_id: CompId) -> Result<EffectiveComp, Error> {
    let (group, comp, regions) = comp_parts(project, comp_id)?;
    let mut segments = Vec::with_capacity(regions.len());
    for region in &regions {
        segments.push(project_segment(project, group, region)?);
    }
    let mut transitions = Vec::with_capacity(comp.transition_ids.len());
    for tid in &comp.transition_ids {
        let transition = project
            .comp
            .transitions
            .get(tid)
            .ok_or_else(|| Error::NotFound(format!("transition {tid}")))?;
        transitions.push(project_transition(project, group, &regions, transition)?);
    }
    Ok(EffectiveComp {
        comp_id,
        segments,
        transitions,
    })
}

/// One region's segment: the base source window plus candidate
/// provenance.
fn project_segment(
    project: &Project,
    group: &TakeGroup,
    region: &CompRegion,
) -> Result<EffectiveSegment, Error> {
    let media_len = range_frames(project, group, &region.project_range)?;
    let base = base_window(
        project,
        group,
        &region.candidate,
        &region.mapping,
        media_len,
    )?;
    let (binding, provenance) = match (&region.candidate, base) {
        (CompCandidateRef::Silence, None) => (None, CompProvenance::Silence),
        (CompCandidateRef::Take { take_id, .. }, Some((src, window))) => (
            Some(SourceBinding {
                source_id: src,
                window,
                mapping: region.mapping,
            }),
            CompProvenance::Take(*take_id),
        ),
        (CompCandidateRef::ExistingSource { source_id, .. }, Some((src, window))) => (
            Some(SourceBinding {
                source_id: src,
                window,
                mapping: region.mapping,
            }),
            CompProvenance::ExistingSource(*source_id),
        ),
        (candidate, base) => {
            return Err(Error::Validation(format!(
                "inconsistent candidate/base for region {}: {candidate:?}/{base:?}",
                region.id
            )));
        }
    };
    Ok(EffectiveSegment {
        region_id: region.id,
        project_range: region.project_range,
        binding,
        provenance,
    })
}

/// One transition's effective form: the boundary, the overlap, and both
/// sides' extended source windows.
fn project_transition(
    project: &Project,
    group: &TakeGroup,
    regions: &[&CompRegion],
    transition: &CompTransition,
) -> Result<EffectiveTransition, Error> {
    let left = regions
        .iter()
        .find(|r| r.id == transition.left_region_id)
        .ok_or_else(|| Error::NotFound(format!("region {}", transition.left_region_id)))?;
    let right = regions
        .iter()
        .find(|r| r.id == transition.right_region_id)
        .ok_or_else(|| Error::NotFound(format!("region {}", transition.right_region_id)))?;
    let boundary = boundary_of(left, right)
        .ok_or_else(|| Error::Validation("transition regions do not touch".into()))?;
    let left_len = range_frames(project, group, &left.project_range)?;
    let right_len = range_frames(project, group, &right.project_range)?;
    let domain = left.project_range.domain();
    let post_media = range_frames(
        project,
        group,
        &sub_range(&boundary, &transition.overlap_range.end_time(), domain)?,
    )?;
    let pre_media = range_frames(
        project,
        group,
        &sub_range(&transition.overlap_range.start_time(), &boundary, domain)?,
    )?;
    Ok(EffectiveTransition {
        transition_id: transition.id,
        boundary,
        overlap_range: transition.overlap_range,
        left: extended_binding(project, group, left, left_len, 0, post_media)?,
        right: extended_binding(project, group, right, right_len, pre_media, 0)?,
        fade_out: transition.fade_out,
        fade_in: transition.fade_in,
    })
}

/// `[from, from + len)` as a `SourceRange`.
fn bound_window(from: SourceFrame, len: u64) -> Result<SourceRange, Error> {
    let end = from.checked_add(i64::try_from(len).map_err(|_| Error::Overflow)?)?;
    SourceRange::try_new(from, end)
}

/// A transition side's extended read: the base window widened by the
/// overhang (pre/post extra media frames under the side's mapping).
/// `Loop` keeps its window — the wrap already stays inside it.
fn extended_binding(
    project: &Project,
    group: &TakeGroup,
    region: &CompRegion,
    media_len: i64,
    pre_media: i64,
    post_media: i64,
) -> Result<Option<SourceBinding>, Error> {
    let Some((source_id, base)) = base_window(
        project,
        group,
        &region.candidate,
        &region.mapping,
        media_len,
    )?
    else {
        return Ok(None);
    };
    let window = if matches!(region.mapping, PlaybackMapping::Loop) {
        base
    } else {
        let pre =
            i64::try_from(consumed(&region.mapping, pre_media)?).map_err(|_| Error::Overflow)?;
        let post =
            i64::try_from(consumed(&region.mapping, post_media)?).map_err(|_| Error::Overflow)?;
        SourceRange::try_new(
            base.start().checked_sub(pre)?,
            base.end().checked_add(post)?,
        )?
    };
    Ok(Some(SourceBinding {
        source_id,
        window,
        mapping: region.mapping,
    }))
}

// ---------------------------------------------------------------------
// Editor targets (#12 §15.2)
// ---------------------------------------------------------------------

/// What an editor displays. Switching display targets is view state —
/// never a project mutation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "target", deny_unknown_fields)]
pub enum EditorDisplayTarget {
    /// The comp's Effective Comp projection.
    EffectiveComp(CompId),
    /// One explicit take's raw source.
    TakeSource(TakeId),
    /// Compare/ghost view over a comp.
    Compare(CompId),
}

/// An editable, derived version of a source — the only form of source
/// an editing target may bind to. The raw capture (version `0`) is not
/// addressable by construction: the fields are private so `new`'s
/// `version >= 1` check cannot be bypassed by a literal.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "SourceVersionRefRepr", into = "SourceVersionRefRepr")]
pub struct SourceVersionRef {
    /// Source this version derives from.
    source_id: SourceId,
    /// Derived version index (`>= 1`; `0` is the raw capture).
    version: u64,
}

/// Wire form for [`SourceVersionRef`]; validates on conversion.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceVersionRefRepr {
    source_id: SourceId,
    version: u64,
}

impl SourceVersionRef {
    /// Creates a reference to a derived version, rejecting the raw
    /// capture (`version == 0`).
    ///
    /// # Errors
    /// [`Error::Validation`] when `version` is `0`.
    pub fn new(source_id: SourceId, version: u64) -> Result<Self, Error> {
        if version == 0 {
            return Err(Error::Validation(
                "raw capture is not an editable source version".into(),
            ));
        }
        Ok(Self { source_id, version })
    }

    /// The source this version derives from.
    #[must_use]
    pub const fn source_id(&self) -> SourceId {
        self.source_id
    }

    /// The derived version index (`>= 1`).
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }
}

impl From<SourceVersionRef> for SourceVersionRefRepr {
    fn from(v: SourceVersionRef) -> Self {
        Self {
            source_id: v.source_id,
            version: v.version,
        }
    }
}

impl TryFrom<SourceVersionRefRepr> for SourceVersionRef {
    type Error = Error;
    fn try_from(repr: SourceVersionRefRepr) -> Result<Self, Self::Error> {
        Self::new(repr.source_id, repr.version)
    }
}

/// Which authority an editor edits — independent of what it displays
/// (`EditorDisplayTarget`). Raw source content is never directly
/// editable: content edits bind to a derived version.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "target", deny_unknown_fields)]
pub enum EditorEditingTarget {
    /// The comp arrangement itself (regions/transitions/candidates).
    Comp(CompId),
    /// An editable derived version of a source.
    EditableSource(SourceVersionRef),
}
