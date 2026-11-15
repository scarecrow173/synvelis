//! CB-07 domain adapter for [`VariantScope::Arrangement`]
//! (synvelis#284).
//!
//! The adapter keeps the CB-01 contract — candidate payloads are full
//! project clones minus variant state, edits are confined to the
//! declared scope, and adopt materializes only scope-owned entities
//! through the shared plan/preflight/CAS path. What is arrangement
//! specific:
//!
//! - **Range-derived ownership**: the scope owns every musical clip
//!   whose placement overlaps `time` on the selected tracks' lanes,
//!   plus (under `fields.notes`) their clip-local notes and each
//!   clip's control closure — [`scope_objects`] stays the single
//!   ownership authority for confinement, fingerprints and adopt
//!   planning. `ArrangementDomainMask` gates the *edit surface*:
//!   clip-structure ops require `fields.clips`, clip-local note ops
//!   require `fields.notes` (`notes` implies clip ownership, so a
//!   notes-only scope still confines clips).
//! - **In-range deletion is expressible**: `RemoveClip`,
//!   `RemoveClipsInRange` and `SplitClip` are accepted candidate
//!   edits — a range selection that cannot delete clips cannot
//!   express the CB-ARR-01 A/B surface. The shared confined flow's
//!   entity-rooted "no scope removal" invariant is therefore
//!   parameterized: [`check_arrangement_scope_removals`] allows a
//!   scope object to leave the set *only* when the entity itself is
//!   gone from the candidate (genuine in-range deletion or note
//!   cascade). An entity that merely leaves the range — `MoveClip`/
//!   `SetClipExtent` carrying a scoped clip outside `time`, or a
//!   `SplitClip` shrinking the kept half out of range — still fails
//!   `Unsupported`: the clip was never deleted, so its departure
//!   from scope ownership is an untracked live mutation, not a
//!   candidate edit. Control-closure and every other kind removal
//!   stays fail-closed (a clip owning control entities cannot be
//!   deleted at all — `validate_references` rejects the dangling
//!   `ControlOwner::Clip` first). `SplitClip` additionally
//!   repartitions clip-local notes onto the new clip: under a
//!   `clips`-only mask those notes are not scope-owned, so a split
//!   that would migrate them past the split point rejects
//!   `Unsupported` rather than expressing a divergence adopt cannot
//!   materialize.
//! - **Adopt deferral, stated plainly**: the shared adopt planner
//!   (`plan_adopt_writes`) still rejects *any* candidate-side scope
//!   removal with typed `Unsupported` — that invariant is
//!   slice-wide, not arrangement-specific, and `engine.rs`/`apply`
//!   are frozen for this slice. A candidate expressing clip deletion
//!   is therefore fork/edit/diff/save-reload capable (CB-ARR-01/02)
//!   but its adopt and render-resolution reject typed until the
//!   apply surface grows `Removed` materialization. Non-removal
//!   candidates adopt atomically through the shared CAS path
//!   (CB-ARR-03).
//! - **Absolute anchors are honest exclusions**: clips anchored or
//!   extented in absolute time have no stable musical span without
//!   a tempo-map conversion policy, so [`scope_objects`] never owns
//!   them. [`ArrangementAdapter::boundary_dependencies`] reports
//!   them (and out-of-range musical clips on the same lanes) as
//!   typed boundary evidence instead of silently ignoring them
//!   (CB-ARR-05); `absolute_anchor` in the capability report is
//!   `Unsupported` for the same reason.
//! - **Adopt gates**: `preflight_adopt` rejects with
//!   [`VariantError::RecordingActive`] while a recording session is
//!   open on any lane of a scoped track. Plugin availability, port
//!   rewiring and stale-scope checks stay with the shared adopt
//!   path — the adapter adds only the recording gate.

use std::collections::BTreeSet;

use crate::{
    Error, IdGenerator, Operation, Project,
    diff::{ObjectKind, SemanticObjectRef},
    id::{ClipId, LaneId, NoteId, TrackId},
    variant::{
        ArrangementDomainMask, BranchEditOp, DomainAdapter, VariantError, VariantScope,
        adapter::{
            ScopeRemovalCheck, edit_scoped_project_op_with_removal, fork_candidate_project,
            is_arrangement_op,
        },
        audio::{ProcessingCapability, clip_edit_guard, lane_recording_open},
        scope::{scope_objects, validate_scope_structure},
    },
};

/// Honest processing-capability report for [`ArrangementAdapter`]
/// candidates (CB-ARR-02): display truth equals enforcement truth.
/// Every `Supported` verdict has an accepted edit path; every
/// `Unsupported` verdict is rejected typed — unsupported arrangement
/// work is never reported as editable or silently normalized away.
/// The report is mask-relative because the declared
/// [`ArrangementDomainMask`] *is* the candidate contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangementCapabilities {
    /// `AddClip` on a scoped track's lane — `fields.clips`. The
    /// placement's range fit is verified post-edit: an in-range clip
    /// becomes scope-owned, an out-of-range one fails
    /// `ScopeViolation`.
    pub clip_add: ProcessingCapability,
    /// `RemoveClip`/`RemoveClipsInRange` in-range deletion —
    /// `fields.clips`. The candidate edit surface accepts it; adopt
    /// materialization of scope removals is deferred slice-wide (the
    /// shared planner rejects typed `Unsupported` — see module
    /// docs), so `Supported` here means *expressible*, never
    /// *adoptable*.
    pub clip_remove: ProcessingCapability,
    /// `SplitClip` — `fields.clips`. Both halves must stay inside
    /// the scope's ownership post-edit or the edit fails typed. A
    /// split repartitions clip-local notes onto the new clip, so
    /// under a `clips`-only mask a split that would migrate notes
    /// past the split point rejects `Unsupported` — those notes are
    /// not scope-owned and adopt cannot materialize their
    /// re-parenting.
    pub clip_split: ProcessingCapability,
    /// `MoveClip`/`SetClipExtent` — `fields.clips`. A move that
    /// carries the clip out of `time` is a scope-removal rule
    /// violation (`Unsupported`), never a silent re-scope.
    pub clip_move: ProcessingCapability,
    /// `SetClipPlayback` — `fields.clips`. Only the `Natural`
    /// mapping lowers; `Loop`/`SimpleStretch` reject through the
    /// shared clip guard.
    pub clip_playback: ProcessingCapability,
    /// `SetClipMuted` — `fields.clips`.
    pub clip_mute: ProcessingCapability,
    /// `RemoveClipsInRange` lane/range sweep — `fields.clips`. Every
    /// named lane must belong to a scoped track and `range` must be
    /// contained in `time` (containment, not mere overlap — a wider
    /// sweep would delete out-of-scope clips the object-set diff
    /// cannot see). `Supported` means *expressible*: adopt
    /// materialization of scope removals is deferred slice-wide (see
    /// module docs).
    pub range_clear: ProcessingCapability,
    /// `AddNote` on a scope-owned clip — `fields.notes`.
    pub note_add: ProcessingCapability,
    /// `RemoveNote` on a scope-owned note — `fields.notes`.
    /// `Supported` means *expressible*: adopt materialization of
    /// scope removals is deferred slice-wide (see module docs).
    pub note_remove: ProcessingCapability,
    /// Absolute-anchored or absolute-extent placement — never
    /// scope-owned: no tempo-map conversion policy exists in this
    /// slice, so such clips are boundary-reported, not editable
    /// (CB-ARR-05).
    pub absolute_anchor: ProcessingCapability,
    /// MIDI source/note ops — `MidiSource`/`MidiNote` entities are
    /// not arrangement scope objects; source-level editing is the
    /// `MidiClip` scope's exclusive contract.
    pub midi_source_edit: ProcessingCapability,
    /// Clip-owned control closure (targets/automation lanes/
    /// modulation sources/assignments) — scope-owned for confinement
    /// and adopt transplant, but no candidate op surface edits it.
    pub control_edit: ProcessingCapability,
}

/// Why a clip on a scoped lane is not scope-owned — the typed
/// boundary evidence a comparison or preflight consumer needs to
/// prove there is no silent reach outside the declared selection
/// (CB-ARR-04/05).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArrangementBoundaryReason {
    /// Absolute-anchored or absolute-extent placement: no stable
    /// musical span exists to compare against the scope's
    /// `MusicalRange` — tempo-map conversion is shared-context
    /// policy this slice never assumes.
    AbsoluteAnchor,
    /// Fully musical placement that does not overlap the scope's
    /// `time` range — adjacent shared context on the same lane.
    OutsideRange,
}

/// A clip sharing a scoped lane without being scope-owned — the
/// arrangement analog of the strip/routing boundary dependency: the
/// clip is shared context the candidate can never edit, and its
/// presence is reported instead of silently ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrangementBoundaryDependency {
    /// The non-owned clip.
    pub clip: ClipId,
    /// The lane hosting it (always a scoped track's lane).
    pub lane: LaneId,
    /// Why the clip is outside the scope-owned set.
    pub reason: ArrangementBoundaryReason,
}

/// Domain adapter for [`VariantScope::Arrangement`]: mask-gated
/// clip/note edits on full-project candidates across a track set ×
/// musical range. Scoped clips, their clip-local notes (under
/// `fields.notes`) and clip-owned control closures transplant
/// through the shared adopt surface; lane/track structure, sources,
/// assets and every out-of-range clip stay shared context the
/// candidate can never touch.
#[derive(Clone, Debug, Default)]
pub struct ArrangementAdapter;

impl ArrangementAdapter {
    /// The adapter singleton.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The capability report for a scope declaring `fields`
    /// (CB-ARR-02). Every `Supported` verdict has an accepted edit
    /// path; every `Unsupported` verdict is rejected typed by the
    /// confinement layer. The report is mask-relative because the
    /// declared [`ArrangementDomainMask`] *is* the candidate
    /// contract.
    #[must_use]
    pub fn capabilities(&self, fields: &ArrangementDomainMask) -> ArrangementCapabilities {
        let clip = |on: bool| {
            if on {
                ProcessingCapability::Supported
            } else {
                ProcessingCapability::Unsupported
            }
        };
        ArrangementCapabilities {
            clip_add: clip(fields.clips),
            clip_remove: clip(fields.clips),
            clip_split: clip(fields.clips),
            clip_move: clip(fields.clips),
            clip_playback: clip(fields.clips),
            clip_mute: clip(fields.clips),
            range_clear: clip(fields.clips),
            note_add: clip(fields.notes),
            note_remove: clip(fields.notes),
            absolute_anchor: ProcessingCapability::Unsupported,
            midi_source_edit: ProcessingCapability::Unsupported,
            control_edit: ProcessingCapability::Unsupported,
        }
    }

    /// Boundary dependencies of the `Arrangement` scope inside
    /// `project`: every clip on a scoped track's lane that is *not*
    /// scope-owned, in clip-id order (CB-ARR-04/05). Ownership is
    /// derived through [`scope_objects`] — the single authority — so
    /// the report can never drift from confinement. Non-owned clips
    /// on scoped lanes are the seam the selection shares with live:
    /// absolute-anchored placements (`AbsoluteAnchor` — no stable
    /// musical span exists) and musical clips outside `time`
    /// (`OutsideRange` — adjacent context a wider sweep could have
    /// reached). Clips on non-scoped tracks are wholly external
    /// shared context, not boundary.
    ///
    /// # Errors
    /// [`Error::Validation`] for a non-`Arrangement` scope,
    /// [`Error::NotFound`]/[`Error::Validation`] from scope
    /// derivation.
    pub fn boundary_dependencies(
        project: &Project,
        scope: &VariantScope,
    ) -> Result<Vec<ArrangementBoundaryDependency>, Error> {
        let VariantScope::Arrangement { tracks, .. } = scope else {
            return Err(Error::Validation(format!(
                "arrangement boundary dependencies require an Arrangement scope, got {scope:?}"
            )));
        };
        // Same structural contract `validate_scope` enforces — a
        // malformed scope must fail typed here, not degrade to a
        // silently partial dependency report.
        validate_scope_structure(project, scope)?;
        let scoped: BTreeSet<TrackId> = tracks.iter().copied().collect();
        let owned_clips: BTreeSet<ClipId> = scope_objects(project, scope)?
            .iter()
            .filter(|o| o.kind == ObjectKind::Clip)
            .filter_map(|o| ClipId::from_tagged(&o.stable_id).ok())
            .collect();
        let mut out = Vec::new();
        for clip in project.clips.values() {
            let Some(lane) = project.lanes.get(&clip.lane_id) else {
                continue;
            };
            if !scoped.contains(&lane.track_id) || owned_clips.contains(&clip.id) {
                continue;
            }
            let reason = arrangement_boundary_reason(clip);
            out.push(ArrangementBoundaryDependency {
                clip: clip.id,
                lane: lane.id,
                reason,
            });
        }
        Ok(out)
    }
}

/// Classifies a non-owned clip on a scoped lane. Ownership was already
/// derived through [`scope_objects`], so a clip reaching this point
/// failed exactly one `arrangement_clips` predicate: any non-musical
/// anchor or extent is `AbsoluteAnchor` (the same exclusion the scope
/// derivation applies — no tempo-map conversion policy exists); a
/// fully musical clip is `OutsideRange` — on a scoped lane the only
/// remaining way to miss ownership is failing the half-open overlap
/// test against `time`.
fn arrangement_boundary_reason(clip: &crate::Clip) -> ArrangementBoundaryReason {
    match (clip.placement.anchor, clip.placement.extent) {
        (crate::TimeAnchor::Musical(_), crate::ClipExtent::Musical(_)) => {
            ArrangementBoundaryReason::OutsideRange
        }
        _ => ArrangementBoundaryReason::AbsoluteAnchor,
    }
}

/// Post-edit scope-removal rule for arrangement candidates (CB-07):
/// a scope object may leave the owned set *only* by entity deletion —
/// `Clip`/`Note` ids absent from the candidate afterwards. An entity
/// that still exists but fell out of the range-owned set (move-out,
/// shrink-out, split-out) is an untracked live mutation and fails
/// `Unsupported`; every non-clip/note kind fails closed the same way
/// (control-closure entities can only leave scope through their
/// owning clip, which the clip rule already polices).
fn check_arrangement_scope_removals(
    candidate: &Project,
    removed: &BTreeSet<SemanticObjectRef>,
) -> Result<(), VariantError> {
    for object in removed {
        match object.kind {
            ObjectKind::Clip => {
                let clip = ClipId::from_tagged(&object.stable_id).map_err(VariantError::from)?;
                if candidate.clips.contains_key(&clip) {
                    return Err(VariantError::Unsupported {
                        detail: format!(
                            "clip {clip} left the declared arrangement range — \
                             moving scoped content out of range is not supported"
                        ),
                    });
                }
            }
            ObjectKind::Note => {
                let note = NoteId::from_tagged(&object.stable_id).map_err(VariantError::from)?;
                if candidate.notes.contains_key(&note) {
                    return Err(VariantError::Unsupported {
                        detail: format!(
                            "note {note} left the declared arrangement scope — \
                             moving scoped content out of range is not supported"
                        ),
                    });
                }
            }
            _ => {
                return Err(VariantError::Unsupported {
                    detail: format!(
                        "scope entity removal is not supported in this slice: {}",
                        object.stable_id
                    ),
                });
            }
        }
    }
    Ok(())
}

/// The removal policy handed to the shared confined-edit flow —
/// a plain `fn` so the seam stays monomorphization-free.
const ARRANGEMENT_REMOVAL: ScopeRemovalCheck = check_arrangement_scope_removals;

impl DomainAdapter for ArrangementAdapter {
    fn name(&self) -> &'static str {
        "arrangement"
    }

    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError> {
        let VariantScope::Arrangement { .. } = scope else {
            return Err(VariantError::InvalidScope {
                detail: format!("arrangement adapter requires an Arrangement scope, got {scope:?}"),
            });
        };
        validate_scope_structure(project, scope).map_err(VariantError::from)?;
        // Full object-set derivation enforces the track/time/mask
        // ownership contract (non-empty selection, resolvable tracks,
        // musical overlap).
        scope_objects(project, scope).map_err(VariantError::from)?;
        Ok(())
    }

    fn fork_candidate(&self, project: &Project) -> Result<Project, VariantError> {
        fork_candidate_project(project)
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
                detail: format!("arrangement candidates accept project ops only: {op:?}"),
            });
        };
        // The engine pairs adapter and scope independently — the
        // arrangement removal policy below must never run against a
        // scope this adapter does not own.
        if !matches!(scope, VariantScope::Arrangement { .. }) {
            return Err(VariantError::InvalidScope {
                detail: format!(
                    "arrangement adapter cannot edit a non-Arrangement scope: {scope:?}"
                ),
            });
        }
        // The op's own contract — clip contract validation, split
        // position/offset checks, note bounds, sweep lane/range
        // existence — runs here because the shared confined flow
        // applies without `Operation::validate` (the controller's
        // validate-then-apply order is reproduced explicitly for the
        // accepted surface only).
        if is_arrangement_op(project_op) {
            project_op.validate(candidate)?;
        }
        // `clip_edit_guard` keeps the CB-CLIP-02 lowering verdict:
        // `Loop`/`SimpleStretch` playback mappings have no consumer
        // and reject `Unsupported` before the op touches the
        // candidate. The removal policy replaces the entity-rooted
        // no-removal invariant with the arrangement rule — genuine
        // in-range deletions are expressible, move-outs are not.
        edit_scoped_project_op_with_removal(
            candidate,
            scope,
            project_op,
            ids,
            Some(&clip_edit_guard),
            Some(ARRANGEMENT_REMOVAL),
        )
    }

    fn preflight_adopt(
        &self,
        live: &Project,
        _candidate: &Project,
        scope: &VariantScope,
    ) -> Result<(), VariantError> {
        let VariantScope::Arrangement { tracks, .. } = scope else {
            return Ok(());
        };
        // A missing track is reported by the adopt planner's own
        // scope derivation; the recording gate only applies to live
        // tracks. Every lane of every scoped track gates — clips
        // under edit sit on those lanes, so an open session anywhere
        // in the selection is a write-gate conflict (CB-ADP-03).
        for track in tracks {
            let Some(track_entity) = live.tracks.get(track) else {
                continue;
            };
            for lane in &track_entity.lane_ids {
                if lane_recording_open(live, *lane) {
                    return Err(VariantError::RecordingActive {
                        detail: format!(
                            "recording session open on lane {lane} of scoped track {track}"
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}
