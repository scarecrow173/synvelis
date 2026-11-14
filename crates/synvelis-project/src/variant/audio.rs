//! CB-02 domain adapters for [`VariantScope::AudioClip`] and
//! [`VariantScope::TakeUsage`] (synvelis#279).
//!
//! Both adapters keep the CB-01 contract — candidate payloads are
//! full project clones minus variant state, edits are confined to the
//! declared scope, and adopt materializes only scoped entities through
//! the shared plan/preflight/CAS path. What differs is the accepted
//! edit surface and the adopt-time domain guard:
//!
//! - [`AudioClipAdapter`] accepts the clip-local `ProjectOp` surface
//!   (trim/extent, placement, mute, clip-local notes) through the
//!   shared confined-edit flow, and exposes [`AudioClipCapabilities`]
//!   so candidate surfaces display the same verdict edits enforce
//!   (CB-CLIP-02).
//! - [`TakeUsageAdapter`] accepts comping [`CompOp`]s through the
//!   additive [`BranchEditOp`] channel — comping ops are deliberately
//!   not `ProjectOp` variants so downstream exhaustive matches stay
//!   stable — confined to the scoped take group and its comp closure.
//!   Comping removals fail closed `Unsupported` in this slice.
//!
//! Raw-material invariants: `Source`/`Asset`/`Take` and the whole
//! recording-session registry are shared, never scoped and never
//! transplanted — adopt writes only `TakeGroup`/`Comp`/`CompRegion`/
//! `CompTransition` entities that reference them. Both adapters reject
//! adopt while a recording session is open on the affected lane
//! ([`VariantError::RecordingActive`], CB-TAKE-03) and while the
//! candidate references shared material the live project lost.

use crate::{
    Error, IdGenerator, Operation, Project, ProjectOp,
    comp::{CompCandidateRef, CompOp},
    id::{CompId, CompRegionId, CompTransitionId, LaneId, TakeGroupId},
    placement::{PlaybackMapping, StretchRatio},
    recording_data::SessionState,
    variant::{
        BranchEditOp, DomainAdapter, VariantError, VariantScope,
        adapter::edit_scoped_project_op,
        scope::{scope_objects, validate_scope_structure},
    },
};

/// Whether the pipeline can actually process a candidate edit — the
/// same verdict `edit_candidate` enforces, so a capability report can
/// never advertise what edits reject (CB-CLIP-02).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProcessingCapability {
    /// Candidates may express it and consumers lower it.
    Supported,
    /// No consumer lowers it — edits expressing it fail
    /// [`VariantError::Unsupported`].
    Unsupported,
}

/// Honest processing-capability report for [`AudioClipAdapter`]
/// candidates (CB-CLIP-02): display truth equals enforcement truth.
/// Every `Supported` verdict has an accepted edit path; every
/// `Unsupported` verdict is rejected typed — unsupported work is
/// never reported as playable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioClipCapabilities {
    /// `SetClipExtent` — trim inside the source window.
    pub trim: ProcessingCapability,
    /// `MoveClip` — timeline anchor/placement changes.
    pub placement: ProcessingCapability,
    /// `SetClipMuted`.
    pub mute: ProcessingCapability,
    /// Clip-local notes (`AddNote`/`RemoveNote` on the scoped clip).
    pub clip_notes: ProcessingCapability,
    /// `Loop` playback mapping — representable in the model, refused
    /// `UnsupportedMapping` at player/render lowering.
    pub loop_playback: ProcessingCapability,
    /// `SimpleStretch` time stretch — representable, refused at
    /// lowering the same way.
    pub stretch: ProcessingCapability,
    /// Pitch shift — no project representation exists at all.
    pub pitch: ProcessingCapability,
    /// Clip-level fades — no project representation exists.
    pub fade: ProcessingCapability,
    /// External/offline processing recipes — no project representation
    /// exists.
    pub external_recipe: ProcessingCapability,
}

/// The lowering verdict for a playback mapping. `Natural` is the only
/// mapping `synvelis-player`/`synvelis-render` consume; `Loop` and
/// `SimpleStretch` are representable but refused `UnsupportedMapping`
/// at lowering, so the candidate edit surface rejects them
/// `Unsupported` instead of storing silently dead state.
pub(crate) fn playback_mapping_capability(mapping: &PlaybackMapping) -> ProcessingCapability {
    match mapping {
        PlaybackMapping::Natural => ProcessingCapability::Supported,
        PlaybackMapping::Loop | PlaybackMapping::SimpleStretch(_) => {
            ProcessingCapability::Unsupported
        }
    }
}

/// CB-CLIP-02 enforcement half of [`AudioClipCapabilities`]: a
/// `SetClipPlayback` carrying a mapping with no lowering consumer is
/// rejected `Unsupported` before the op touches the candidate.
pub(crate) fn clip_edit_guard(op: &ProjectOp) -> Result<(), VariantError> {
    if let ProjectOp::SetClipPlayback { playback, .. } = op
        && playback_mapping_capability(&playback.mapping) == ProcessingCapability::Unsupported
    {
        return Err(VariantError::Unsupported {
            detail: format!(
                "clip playback mapping {:?} has no lowering consumer",
                playback.mapping
            ),
        });
    }
    Ok(())
}

/// `true` while a recording session is `Open` on `lane` — the
/// CB-ADP-03 write-gate conflict: scoped live mutation on a lane
/// mid-recording is rejected typed, never silently applied.
pub(crate) fn lane_recording_open(project: &Project, lane: LaneId) -> bool {
    project.recording.streams.values().any(|stream| {
        stream.lane_id == lane
            && project
                .recording
                .sessions
                .get(&stream.session_id)
                .is_some_and(|session| session.state == SessionState::Open)
    })
}

/// Domain adapter for [`VariantScope::AudioClip`]: clip-local edits
/// (trim/extent, placement, mute, clip notes) on full-project
/// candidates, with raw `Source`/`Asset` shared and untouched.
#[derive(Clone, Debug, Default)]
pub struct AudioClipAdapter;

impl AudioClipAdapter {
    /// The adapter singleton.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The capability report candidate surfaces display (CB-CLIP-02).
    /// `loop_playback`/`stretch` share [`playback_mapping_capability`]
    /// with the edit guard, so the displayed verdict is the enforced
    /// verdict.
    ///
    /// # Panics
    /// Never in practice: `StretchRatio::new(2, 1)` is statically
    /// valid; the `expect` exists only because the constructor
    /// returns `Result`.
    #[must_use]
    pub fn capabilities(&self) -> AudioClipCapabilities {
        AudioClipCapabilities {
            trim: ProcessingCapability::Supported,
            placement: ProcessingCapability::Supported,
            mute: ProcessingCapability::Supported,
            clip_notes: ProcessingCapability::Supported,
            loop_playback: playback_mapping_capability(&PlaybackMapping::Loop),
            stretch: playback_mapping_capability(&PlaybackMapping::SimpleStretch(
                StretchRatio::new(2, 1).expect("2:1 is a valid stretch ratio"),
            )),
            pitch: ProcessingCapability::Unsupported,
            fade: ProcessingCapability::Unsupported,
            external_recipe: ProcessingCapability::Unsupported,
        }
    }
}

impl DomainAdapter for AudioClipAdapter {
    fn name(&self) -> &'static str {
        "audio-clip"
    }

    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError> {
        let VariantScope::AudioClip { clip } = scope else {
            return Err(VariantError::InvalidScope {
                detail: format!("audio-clip adapter requires an AudioClip scope, got {scope:?}"),
            });
        };
        validate_scope_structure(project, scope).map_err(VariantError::from)?;
        let clip_entity = project
            .clips
            .get(clip)
            .ok_or_else(|| Error::NotFound(format!("clip {clip}")))?;
        if project.midi_sources.contains_key(&clip_entity.source_id) {
            return Err(VariantError::InvalidScope {
                detail: format!(
                    "clip {clip} reads a MIDI source — MIDI clips use VariantScope::MidiClip"
                ),
            });
        }
        scope_objects(project, scope).map_err(VariantError::from)?;
        Ok(())
    }

    fn fork_candidate(&self, project: &Project) -> Result<Project, VariantError> {
        super::adapter::fork_candidate_project(project)
    }

    fn edit_candidate(
        &self,
        candidate: &mut Project,
        scope: &VariantScope,
        op: &BranchEditOp,
        ids: &mut dyn IdGenerator,
    ) -> Result<(), VariantError> {
        let BranchEditOp::Project(project_op) = op else {
            return Err(VariantError::Unsupported {
                detail: format!("audio-clip candidates accept project ops only: {op:?}"),
            });
        };
        edit_scoped_project_op(candidate, scope, project_op, ids, Some(&clip_edit_guard))
    }

    fn preflight_adopt(
        &self,
        live: &Project,
        _candidate: &Project,
        scope: &VariantScope,
    ) -> Result<(), VariantError> {
        let VariantScope::AudioClip { clip } = scope else {
            return Ok(());
        };
        // A missing clip is reported by the adopt planner's own scope
        // derivation; the recording gate only applies to a live clip.
        if let Some(clip_entity) = live.clips.get(clip)
            && lane_recording_open(live, clip_entity.lane_id)
        {
            return Err(VariantError::RecordingActive {
                detail: format!(
                    "recording session open on lane {} of clip {clip}",
                    clip_entity.lane_id
                ),
            });
        }
        Ok(())
    }
}

/// Resolves every take group a comping op touches. Group references
/// that do not resolve contribute nothing — `CompOp::validate` then
/// reports the typed `NotFound`, and confinement must not mask it.
/// `RegisterTakeGroup` contributes nothing by construction: it
/// creates a group rather than editing one, so it can never land
/// inside an existing [`VariantScope::TakeUsage`].
fn comp_op_groups(project: &Project, op: &CompOp) -> Vec<TakeGroupId> {
    let comp_group = |comp_id: CompId| {
        project
            .comp
            .comps
            .get(&comp_id)
            .map(|comp| comp.take_group_id)
    };
    let group_exists = |group_id: &TakeGroupId| {
        project
            .comp
            .take_groups
            .contains_key(group_id)
            .then_some(*group_id)
    };
    let region_group = |region_id: CompRegionId| {
        project
            .comp
            .regions
            .get(&region_id)
            .and_then(|region| comp_group(region.comp_id))
    };
    let transition_group = |transition_id: CompTransitionId| {
        project
            .comp
            .transitions
            .get(&transition_id)
            .and_then(|transition| comp_group(transition.comp_id))
    };
    match op {
        CompOp::RegisterTakeGroup { .. } => Vec::new(),
        CompOp::UnregisterTakeGroup { take_group_id } | CompOp::CreateComp { take_group_id } => {
            group_exists(take_group_id).into_iter().collect()
        }
        CompOp::RemoveComp { comp_id } | CompOp::AddCompRegion { comp_id, .. } => {
            comp_group(*comp_id).into_iter().collect()
        }
        CompOp::SetActiveComp {
            take_group_id,
            comp_id,
        } => {
            let mut groups: Vec<TakeGroupId> = group_exists(take_group_id).into_iter().collect();
            if let Some(id) = comp_id
                && let Some(group) = comp_group(*id)
            {
                groups.push(group);
            }
            groups
        }
        CompOp::SetCompRegionCandidate { region_id, .. }
        | CompOp::SetCompRegionRange { region_id, .. }
        | CompOp::RemoveCompRegion { region_id } => region_group(*region_id).into_iter().collect(),
        CompOp::AddCompTransition {
            left_region_id,
            right_region_id,
            ..
        } => [
            region_group(*left_region_id),
            region_group(*right_region_id),
        ]
        .into_iter()
        .flatten()
        .collect(),
        CompOp::SetCompTransition { transition_id, .. }
        | CompOp::RemoveCompTransition { transition_id } => {
            transition_group(*transition_id).into_iter().collect()
        }
    }
}

/// Capability + confinement gate for a comping candidate edit: the
/// op must stay inside the scoped take group, express only mappings
/// the pipeline lowers, and — in this slice — never remove scoped
/// entities. Unresolved references fall through so `CompOp::validate`
/// reports the typed `NotFound`.
fn check_comp_op(project: &Project, scope: &VariantScope, op: &CompOp) -> Result<(), VariantError> {
    if let CompOp::AddCompRegion { mapping, .. } = op
        && playback_mapping_capability(mapping) == ProcessingCapability::Unsupported
    {
        return Err(VariantError::Unsupported {
            detail: format!(
                "comp region playback mapping {mapping:?} is representable but no consumer lowers it"
            ),
        });
    }
    let VariantScope::TakeUsage { target } = scope else {
        return Err(VariantError::InvalidScope {
            detail: format!("take-usage adapter requires a TakeUsage scope, got {scope:?}"),
        });
    };
    if matches!(op, CompOp::RegisterTakeGroup { .. }) {
        return Err(VariantError::ScopeViolation {
            detail: "take-group registration creates an entity outside the scoped group".into(),
        });
    }
    let groups = comp_op_groups(project, op);
    for group in &groups {
        if *group != target.take_group {
            return Err(VariantError::ScopeViolation {
                detail: format!(
                    "comping op targets take group {group}, outside scope group {}",
                    target.take_group
                ),
            });
        }
    }
    if !groups.is_empty()
        && matches!(
            op,
            CompOp::UnregisterTakeGroup { .. }
                | CompOp::RemoveComp { .. }
                | CompOp::RemoveCompRegion { .. }
                | CompOp::RemoveCompTransition { .. }
        )
    {
        return Err(VariantError::Unsupported {
            detail: format!("comping removal is not supported in this slice: {op:?}"),
        });
    }
    Ok(())
}

/// Domain adapter for [`VariantScope::TakeUsage`]: comping edits on
/// the scoped take group's comp closure. Raw takes and sources stay
/// shared references — candidate payloads and adopt writes never
/// touch them.
#[derive(Clone, Debug, Default)]
pub struct TakeUsageAdapter;

impl TakeUsageAdapter {
    /// The adapter singleton.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl DomainAdapter for TakeUsageAdapter {
    fn name(&self) -> &'static str {
        "take-usage"
    }

    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError> {
        if !matches!(scope, VariantScope::TakeUsage { .. }) {
            return Err(VariantError::InvalidScope {
                detail: format!("take-usage adapter requires a TakeUsage scope, got {scope:?}"),
            });
        }
        validate_scope_structure(project, scope).map_err(VariantError::from)?;
        scope_objects(project, scope).map_err(VariantError::from)?;
        Ok(())
    }

    fn fork_candidate(&self, project: &Project) -> Result<Project, VariantError> {
        super::adapter::fork_candidate_project(project)
    }

    fn edit_candidate(
        &self,
        candidate: &mut Project,
        scope: &VariantScope,
        op: &BranchEditOp,
        ids: &mut dyn IdGenerator,
    ) -> Result<(), VariantError> {
        let BranchEditOp::Comp(comp_op) = op else {
            return Err(VariantError::Unsupported {
                detail: "take-usage candidates accept comping ops only".into(),
            });
        };
        check_comp_op(candidate, scope, comp_op)?;
        let before_objects = scope_objects(candidate, scope)?;
        // `CompOp::apply` does not self-validate — the controller's
        // validate-then-apply contract is reproduced here explicitly.
        Operation::validate(comp_op, candidate)?;
        Operation::apply(comp_op, candidate, ids)?;
        candidate.validate_references()?;

        let after_objects = scope_objects(candidate, scope)?;
        if let Some(removed) = before_objects.difference(&after_objects).next() {
            return Err(VariantError::Unsupported {
                detail: format!(
                    "scope entity removal is not supported in this slice: {}",
                    removed.stable_id
                ),
            });
        }
        Ok(())
    }

    fn preflight_adopt(
        &self,
        live: &Project,
        candidate: &Project,
        scope: &VariantScope,
    ) -> Result<(), VariantError> {
        let VariantScope::TakeUsage { target } = scope else {
            return Ok(());
        };
        let group = live
            .comp
            .take_groups
            .get(&target.take_group)
            .ok_or_else(|| {
                VariantError::Project(Error::NotFound(format!("take group {}", target.take_group)))
            })?;
        if lane_recording_open(live, group.owner_lane_id) {
            return Err(VariantError::RecordingActive {
                detail: format!(
                    "recording session open on lane {} of take group {}",
                    group.owner_lane_id, target.take_group
                ),
            });
        }
        // The candidate's comp closure must still resolve against live
        // shared raw material — takes and sources are never
        // transplanted, so a candidate referencing material live lost
        // fails closed here before any write is planned (CB-TAKE-03).
        let Some(candidate_group) = candidate.comp.take_groups.get(&target.take_group) else {
            return Ok(());
        };
        for take_id in &candidate_group.take_ids {
            if !live.recording.takes.contains_key(take_id) {
                return Err(VariantError::Project(Error::NotFound(format!(
                    "take {take_id}"
                ))));
            }
        }
        for comp in candidate
            .comp
            .comps
            .values()
            .filter(|comp| comp.take_group_id == target.take_group)
        {
            for region_id in &comp.region_ids {
                let Some(region) = candidate.comp.regions.get(region_id) else {
                    continue;
                };
                match &region.candidate {
                    CompCandidateRef::Take { take_id, .. } => {
                        if !live.recording.takes.contains_key(take_id) {
                            return Err(VariantError::Project(Error::NotFound(format!(
                                "take {take_id}"
                            ))));
                        }
                    }
                    CompCandidateRef::ExistingSource { source_id, .. } => {
                        if !live.sources.contains_key(source_id) {
                            return Err(VariantError::Project(Error::NotFound(format!(
                                "source {source_id}"
                            ))));
                        }
                    }
                    CompCandidateRef::Silence => {}
                }
            }
        }
        Ok(())
    }
}
