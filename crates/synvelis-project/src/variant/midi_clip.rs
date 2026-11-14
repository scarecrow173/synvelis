//! CB-03 domain adapter for [`VariantScope::MidiClip`] (synvelis#280).
//!
//! The adapter keeps the CB-01 contract — candidate payloads are full
//! project clones minus variant state, edits are confined to the
//! declared scope, and adopt materializes only scoped entities through
//! the shared plan/preflight/CAS path. What is MIDI-specific:
//!
//! - **Native event fidelity**: `MidiSource` (`id`/`division`/`notes`)
//!   and `SmfDocument` stay native — no audio flattening, no project
//!   PPQN requantize, no silent quantize/rounding. Note positions
//!   remain source-native [`MidiTick`](crate::time::MidiTick) values;
//!   `beat = start / division` stays the explicit rational mapping.
//!   `SmfDocument` is deliberately *not* a scope object: it is shared
//!   provenance context, so adopt never rewrites the imported stream
//!   (CB-MIDI-03/06).
//! - **SMF-backed immutability**: note ops on a source carrying SMF
//!   provenance are rejected typed — the document is authoritative and
//!   `MidiSource.notes` is its projection. The editable copy is
//!   `ProjectOp::DetachClipSource` on live *before* fork; candidate-
//!   internal detach is `Unsupported` (it would rebind the scope's
//!   owned set mid-branch, which the engine's no-removal invariant
//!   rejects fail-closed).
//! - **Honest capabilities**: [`MidiClipCapabilities`] reports the
//!   same verdict `edit_candidate` enforces — note add/move/velocity
//!   are `Supported`; note removal (scope-entity removal is
//!   unsupported in this slice), key/len edits, CC/PitchBend/CC64,
//!   MIDI2 and unknown-event editing are `Unsupported` because no
//!   project op surface exists for them (CB-MIDI-06).
//!
//! Scope confinement for the MIDI op surface lives in the shared
//! helpers ([`check_op_targets`](super::adapter) /
//! `check_created_within_scope`): note ops must target the scope's
//! `MidiSource` and, where they name one, a scoped `MidiNote`.
//! `AddMidiSource`/`RemoveMidiSource`/`ImportMidiSmf`/
//! `DetachClipSource` stay `Unsupported` — they create entities
//! outside the scope or rebind the clip's source, and the post-edit
//! scope derivation would diverge from the declared scope.

use crate::{
    Error, IdGenerator, Operation, Project, ProjectOp,
    variant::{
        BranchEditOp, DomainAdapter, VariantError, VariantScope,
        adapter::edit_scoped_project_op,
        audio::{
            ProcessingCapability, clip_edit_guard, lane_recording_open, playback_mapping_capability,
        },
        scope::{scope_objects, validate_scope_structure},
    },
};

/// Honest processing-capability report for [`MidiClipAdapter`]
/// candidates (CB-MIDI-06): display truth equals enforcement truth.
/// Every `Supported` verdict has an accepted edit path; every
/// `Unsupported` verdict is rejected typed — unsupported MIDI work is
/// never reported as editable or silently normalized away.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiClipCapabilities {
    /// `AddMidiNote` — new note in the scoped source (branch-minted
    /// `NoteId` reserved in live space).
    pub note_add: ProcessingCapability,
    /// `MoveMidiNote` — start-tick move; `len` is preserved.
    pub note_move: ProcessingCapability,
    /// `SetMidiNoteVelocity` — velocity 1–127.
    pub note_velocity: ProcessingCapability,
    /// `RemoveMidiNote`/clip-local `RemoveNote` — scope-entity removal
    /// is unsupported in this slice (typed `Unsupported` at the
    /// confinement layer).
    pub note_remove: ProcessingCapability,
    /// Note key edits — no `ProjectOp` surface exists.
    pub note_key: ProcessingCapability,
    /// Note length/resize edits — no `ProjectOp` surface exists.
    pub note_len: ProcessingCapability,
    /// CC edits (incl. CC64 sustain) — no project representation for
    /// candidate edits; the native stream lives only in
    /// `SmfDocument`/`midi_comp` playback-plan projection.
    pub cc: ProcessingCapability,
    /// Pitch bend edits — no project representation.
    pub pitch_bend: ProcessingCapability,
    /// Note ops on an SMF-provenance source — immutable by contract;
    /// detach on live before fork for an editable copy.
    pub smf_backed_notes: ProcessingCapability,
    /// `DetachClipSource` inside a candidate — unsupported (scope
    /// rebind); detach on live before fork instead.
    pub detach_in_candidate: ProcessingCapability,
    /// `Loop` playback mapping — representable, refused at lowering.
    pub loop_playback: ProcessingCapability,
    /// `SimpleStretch` time stretch — representable, refused at
    /// lowering the same way.
    pub stretch: ProcessingCapability,
    /// MIDI2 / unknown event kinds — no project representation;
    /// preserved as `Unsupported`, never normalized.
    pub midi2_unknown_events: ProcessingCapability,
}

/// Domain adapter for [`VariantScope::MidiClip`]: clip-local and
/// scoped-source note edits on full-project candidates. The scoped
/// `MidiSource`/`MidiNote` entities transplant through the shared
/// adopt surface; the clip's `SmfDocument` provenance stays shared
/// context and is never rewritten by adopt.
#[derive(Clone, Debug, Default)]
pub struct MidiClipAdapter;

impl MidiClipAdapter {
    /// The adapter singleton.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The capability report candidate surfaces display (CB-MIDI-06).
    /// `loop_playback`/`stretch` share
    /// [`playback_mapping_capability`] with the edit guard, so the
    /// displayed verdict is the enforced verdict.
    ///
    /// # Panics
    /// Never in practice: `StretchRatio::new(2, 1)` is statically
    /// valid; the `expect` exists only because the constructor
    /// returns `Result`.
    #[must_use]
    pub fn capabilities(&self) -> MidiClipCapabilities {
        MidiClipCapabilities {
            note_add: ProcessingCapability::Supported,
            note_move: ProcessingCapability::Supported,
            note_velocity: ProcessingCapability::Supported,
            note_remove: ProcessingCapability::Unsupported,
            note_key: ProcessingCapability::Unsupported,
            note_len: ProcessingCapability::Unsupported,
            cc: ProcessingCapability::Unsupported,
            pitch_bend: ProcessingCapability::Unsupported,
            smf_backed_notes: ProcessingCapability::Unsupported,
            detach_in_candidate: ProcessingCapability::Unsupported,
            loop_playback: playback_mapping_capability(&crate::placement::PlaybackMapping::Loop),
            stretch: playback_mapping_capability(
                &crate::placement::PlaybackMapping::SimpleStretch(
                    crate::placement::StretchRatio::new(2, 1)
                        .expect("2:1 is a valid stretch ratio"),
                ),
            ),
            midi2_unknown_events: ProcessingCapability::Unsupported,
        }
    }
}

/// `true` for the `ProjectOp`s whose contract includes the SMF-backed
/// immutability rule — the shared confined-edit flow applies ops
/// without running `Operation::validate`, so the adapter re-runs the
/// op's own validation for exactly this surface before confinement.
fn is_midi_note_op(op: &ProjectOp) -> bool {
    matches!(
        op,
        ProjectOp::AddMidiNote { .. }
            | ProjectOp::RemoveMidiNote { .. }
            | ProjectOp::SetMidiNoteVelocity { .. }
            | ProjectOp::MoveMidiNote { .. }
    )
}

impl DomainAdapter for MidiClipAdapter {
    fn name(&self) -> &'static str {
        "midi-clip"
    }

    fn validate_scope(&self, project: &Project, scope: &VariantScope) -> Result<(), VariantError> {
        let VariantScope::MidiClip { clip } = scope else {
            return Err(VariantError::InvalidScope {
                detail: format!("midi-clip adapter requires a MidiClip scope, got {scope:?}"),
            });
        };
        validate_scope_structure(project, scope).map_err(VariantError::from)?;
        let clip_entity = project
            .clips
            .get(clip)
            .ok_or_else(|| Error::NotFound(format!("clip {clip}")))?;
        if !project.midi_sources.contains_key(&clip_entity.source_id) {
            return Err(VariantError::InvalidScope {
                detail: format!(
                    "clip {clip} does not read a MIDI source — media clips use VariantScope::AudioClip"
                ),
            });
        }
        // Full object-set derivation enforces MIDI source exclusivity:
        // a source shared with another clip makes ownership ambiguous.
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
                detail: format!("midi-clip candidates accept project ops only: {op:?}"),
            });
        };
        // The op's own contract — including SMF-backed immutability —
        // runs here because the shared confined flow applies without
        // `Operation::validate` (the controller's validate-then-apply
        // order is reproduced explicitly for this surface only).
        if is_midi_note_op(project_op) {
            project_op.validate(candidate)?;
        }
        edit_scoped_project_op(candidate, scope, project_op, ids, Some(&clip_edit_guard))
    }

    fn preflight_adopt(
        &self,
        live: &Project,
        candidate: &Project,
        scope: &VariantScope,
    ) -> Result<(), VariantError> {
        let VariantScope::MidiClip { clip } = scope else {
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
        // Source-identity gate: the candidate clip must still read the
        // source live reads. Detach/re-point inside a candidate is
        // unsupported, so any divergence here is foreign state — fail
        // closed before transplant planning rather than guessing.
        if let (Some(live_clip), Some(candidate_clip)) =
            (live.clips.get(clip), candidate.clips.get(clip))
            && live_clip.source_id != candidate_clip.source_id
        {
            return Err(VariantError::Unsupported {
                detail: format!(
                    "candidate clip {clip} reads source {}, live reads {}",
                    candidate_clip.source_id, live_clip.source_id
                ),
            });
        }
        Ok(())
    }
}
