//! TIM-02 player-side MIDI lowering (issue #126): project
//! [`MidiPlaybackPlan`] → engine-frame-positioned [`MidiEvent`]s for
//! installation on an attached plugin node's transport plan.
//!
//! This module owns only the *product-side* lowering: time mapping
//! (project `MusicalTick` → engine frame via [`TickTimeMapping`]),
//! plan-action → wire-`MidiKind` translation, note-channel
//! resolution, and the terminal `AllNotesOff` panic expansion.
//! Scheduling, note/voice ledgering, discontinuity release/chase and
//! `EventList` delivery are Ruviform's (`TransportScheduler` /
//! `PluginNode`) — nothing here duplicates them.
//!
//! ## Channel provenance
//!
//! [`MidiPlanAction::NoteOn`]/[`MidiPlanAction::NoteOff`] carry no
//! channel (`MidiNote` has none — a deferred project-model change).
//! When the note's source has a persisted [`SmfDocument`](synvelis_project::SmfDocument), the note's
//! channel is recovered by zipping `source.notes` (`NoteId` order —
//! the deterministic order `ImportMidiSmf` drew ids in
//! `synvelis_project::SmfDocument::paired_notes` order) with `paired_notes()` specs,
//! keeping only pairs whose `(key, velocity, start, len)` content
//! matches. SMF-imported notes are immutable, so the correspondence
//! is stable; detached copies and hand-built sources have no
//! document and fall back to `fallback_channel`.
//!
//! ## `AllNotesOff`
//!
//! The plan's terminal panic maps to MIDI CC123 (All Notes Off,
//! value 0) on every channel that sounded a note — bounded by the
//! channels actually used, never a blind per-key sweep. This is the
//! standard wire panic; the ledger-level guarantee stays with
//! upstream `NoteLedger::release_all` on discontinuities.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_media_core::{
    MidiChannel, MidiController, MidiEvent, MidiGroup, MidiKind, MidiRangeError, MidiValue,
    NoteKey, TimeBase,
};
use synvelis_project::{
    MidiControllerKind, MidiPlanAction, MidiPlaybackPlan, NoteId, Project, SourceId,
    TickTimeMapping,
};

/// Typed failures of [`compile_midi_events`]. Every variant is a
/// boundary refusal — nothing is silently coerced.
#[derive(Debug)]
#[non_exhaustive]
pub enum MidiCompileError {
    /// A project-domain failure: tick→media conversion overflow or
    /// SMF provenance re-derivation (`paired_notes`).
    Project(synvelis_project::Error),
    /// A wire-domain value out of range (channel/key/controller) —
    /// defensive: upstream plan compilation pre-validates these.
    Wire(MidiRangeError),
    /// A plan value outside its wire domain (controller value,
    /// pitch-bend range) — likewise defensive.
    Domain(&'static str),
}

impl std::fmt::Display for MidiCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project(e) => write!(f, "project midi lowering failed: {e}"),
            Self::Wire(e) => write!(f, "midi wire domain violation: {e}"),
            Self::Domain(detail) => write!(f, "midi plan value out of domain: {detail}"),
        }
    }
}

impl std::error::Error for MidiCompileError {}

impl From<synvelis_project::Error> for MidiCompileError {
    fn from(e: synvelis_project::Error) -> Self {
        Self::Project(e)
    }
}

impl From<MidiRangeError> for MidiCompileError {
    fn from(e: MidiRangeError) -> Self {
        Self::Wire(e)
    }
}

/// MIDI CC123 — the standard All Notes Off controller.
const ALL_NOTES_OFF_CONTROLLER: u8 = 123;

/// Per-source `NoteId → channel` provenance recovered from the
/// persisted [`SmfDocument`](synvelis_project::SmfDocument). An absent/empty map means every note of
/// that source falls back to the binding's declared channel.
struct ChannelResolver<'a> {
    project: &'a Project,
    fallback: u8,
    maps: BTreeMap<SourceId, BTreeMap<NoteId, u8>>,
}

impl ChannelResolver<'_> {
    /// The wire channel for `(source_id, note_id)` — provenance when
    /// recoverable, else the declared fallback.
    fn channel(&mut self, source_id: SourceId, note_id: NoteId) -> Result<u8, MidiCompileError> {
        if !self.maps.contains_key(&source_id) {
            self.maps
                .insert(source_id, note_channel_map(self.project, source_id)?);
        }
        Ok(self
            .maps
            .get(&source_id)
            .and_then(|map| map.get(&note_id))
            .copied()
            .unwrap_or(self.fallback))
    }
}

/// Builds `NoteId → channel` for `source_id` by aligning
/// `source.notes` (`NoteId` order) with `SmfDocument::paired_notes()`
/// (the deterministic spec order `ImportMidiSmf` consumed). A pair
/// is trusted only when the note's `(key, velocity, start, len)`
/// content matches the spec — any drift leaves the note on the
/// fallback channel rather than guessing a wrong one.
fn note_channel_map(
    project: &Project,
    source_id: SourceId,
) -> Result<BTreeMap<NoteId, u8>, MidiCompileError> {
    let mut map = BTreeMap::new();
    let (Some(source), Some(document)) = (
        project.midi_sources.get(&source_id),
        project.smf_documents.get(&source_id),
    ) else {
        return Ok(map);
    };
    let specs = document.paired_notes()?;
    for (note, spec) in source.notes.values().zip(specs.iter()) {
        if note.key == spec.key
            && note.velocity == spec.velocity
            && note.start == spec.start
            && note.len == spec.len
        {
            map.insert(note.id, spec.channel);
        }
    }
    Ok(map)
}

/// `u8` channel → validated [`MidiChannel`].
fn midi_channel(channel: u8) -> Result<MidiChannel, MidiCompileError> {
    Ok(MidiChannel::new(channel)?)
}

/// `u8` key → validated [`NoteKey`].
fn note_key(key: u8) -> Result<NoteKey, MidiCompileError> {
    Ok(NoteKey::new(key)?)
}

/// One [`MidiPlanAction::ControllerChase`] → its wire [`MidiKind`].
/// Values are re-checked against the wire domain: `from_u7` wraps
/// above 127 and `bend_from_u14` clamps above 16383, so an
/// out-of-domain plan value must be refused here, not coerced.
fn chase_kind(controller: MidiControllerKind, value: i32) -> Result<MidiKind, MidiCompileError> {
    match controller {
        MidiControllerKind::Cc(number) => {
            let value = u8::try_from(value)
                .ok()
                .filter(|value| *value <= 127)
                .ok_or(MidiCompileError::Domain("cc value"))?;
            Ok(MidiKind::ControlChange {
                controller: MidiController::new(number)?,
                value: MidiValue::from_u7(value),
            })
        }
        MidiControllerKind::PitchBend => {
            let value = u16::try_from(value)
                .ok()
                .filter(|value| *value <= 16_383)
                .ok_or(MidiCompileError::Domain("pitch bend value"))?;
            Ok(MidiKind::PitchBend {
                value: MidiValue::bend_from_u14(value),
            })
        }
    }
}

/// Builds the channel voice message on `group 0` / `channel`.
fn voice_event(channel: u8, kind: MidiKind) -> Result<MidiEvent, MidiCompileError> {
    Ok(MidiEvent {
        group: MidiGroup::ZERO,
        channel: midi_channel(channel)?,
        kind,
    })
}

/// Lowers `plan` into `(engine frame, MidiEvent)` pairs, preserving
/// the plan's total order: `at` is non-decreasing (the plan sorts by
/// `(at, action rank, …)` and `tick_to_media` is monotonic), and
/// same-frame order is the plan's — note-offs precede chases which
/// precede note-ons, matching upstream's cross-source note-off
/// priority.
///
/// `frame_base` is the engine frame base the events will be
/// scheduled in — pass the dispatch's verified attach-time base
/// (`ParamDispatch::frame_base`), typically
/// `TimeBase::sample_rate(rate)`. One engine frame is one tick of
/// that base, so the returned `i64` positions drop straight into
/// [`crate::ParamDispatch::dispatch_midi`].
/// `fallback_channel` is the declared channel for notes whose source
/// has no recoverable SMF provenance (and for note actions lacking
/// `note` identity — a defensive case upstream never emits).
///
/// # Errors
/// [`MidiCompileError::Project`] on tick-mapping overflow or SMF
/// provenance failure; [`MidiCompileError::Wire`]/[`Domain`] on a
/// defensive domain violation.
pub fn compile_midi_events(
    project: &Project,
    plan: &MidiPlaybackPlan,
    mapping: &TickTimeMapping,
    frame_base: TimeBase,
    fallback_channel: u8,
) -> Result<Vec<(i64, MidiEvent)>, MidiCompileError> {
    // Pass 1: resolve every note event's channel (building per-source
    // provenance maps lazily) and collect the channels that sounded
    // notes — the `AllNotesOff` expansion needs the complete set, so
    // it cannot be decided inline even though the action is
    // contractually terminal.
    let mut resolver = ChannelResolver {
        project,
        fallback: fallback_channel,
        maps: BTreeMap::new(),
    };
    let mut note_channels: Vec<u8> = Vec::with_capacity(plan.events.len());
    let mut sounded_channels: BTreeSet<u8> = BTreeSet::new();
    for event in &plan.events {
        let channel = match event.action {
            MidiPlanAction::NoteOn { .. } | MidiPlanAction::NoteOff { .. } => {
                let channel = match event.note {
                    Some((source_id, note_id)) => resolver.channel(source_id, note_id)?,
                    None => fallback_channel,
                };
                sounded_channels.insert(channel);
                channel
            }
            _ => fallback_channel,
        };
        note_channels.push(channel);
    }

    // Pass 2: emit wire events in plan order.
    let mut out: Vec<(i64, MidiEvent)> = Vec::with_capacity(plan.events.len());
    for (index, event) in plan.events.iter().enumerate() {
        let frame = mapping
            .tick_to_media(event.at, frame_base.numerator(), frame_base.denominator())
            .map_err(MidiCompileError::Project)?
            .value();
        match event.action {
            MidiPlanAction::NoteOn { key, velocity } => {
                // `from_u7` wraps above 127 — refuse instead of
                // emitting a wrapped velocity.
                if velocity > 0x7F {
                    return Err(MidiCompileError::Domain("note-on velocity"));
                }
                out.push((
                    frame,
                    voice_event(
                        note_channels[index],
                        MidiKind::NoteOn {
                            key: note_key(key)?,
                            velocity: MidiValue::from_u7(velocity),
                        },
                    )?,
                ));
            }
            MidiPlanAction::NoteOff { key } => {
                out.push((
                    frame,
                    voice_event(
                        note_channels[index],
                        MidiKind::NoteOff {
                            key: note_key(key)?,
                            velocity: MidiValue::from_u7(0),
                        },
                    )?,
                ));
            }
            MidiPlanAction::ControllerChase {
                channel,
                controller,
                value,
            } => {
                out.push((frame, voice_event(channel, chase_kind(controller, value)?)?));
            }
            MidiPlanAction::AllNotesOff => {
                let controller = MidiController::new(ALL_NOTES_OFF_CONTROLLER)?;
                for channel in &sounded_channels {
                    out.push((
                        frame,
                        voice_event(
                            *channel,
                            MidiKind::ControlChange {
                                controller,
                                value: 0,
                            },
                        )?,
                    ));
                }
            }
        }
    }
    Ok(out)
}
