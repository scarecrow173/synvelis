//! MIDI phrase comp playback-plan oracle (RCD-04 fixture semantics,
//! issue #118).
//!
//! Given a persisted [`Project`] ( [`MidiSource`] notes in
//! source-native ticks) and an ordered phrase [`MidiCompSelection`],
//! [`compile_midi_comp`] produces a deterministic [`MidiPlaybackPlan`]
//! that guarantees, by construction:
//!
//! - **No hanging notes** — every adopted note gets a `NoteOff` no
//!   later than its region's project end; a note whose natural end
//!   exceeds the phrase/region boundary is cut by a *virtual* off
//!   recorded in [`MidiRegionReport::virtual_offs`].
//! - **No controller leaks** — every region start emits a
//!   `ControllerChase` snapshot covering the union of controller
//!   kinds declared anywhere in the input lanes; kinds a region's
//!   own source never declared are chased to their default, so
//!   state from a previous region cannot bleed through. Controller
//!   events inside a phrase window play at their mapped project
//!   ticks. The plan ends with `AllNotesOff`.
//! - **No implicit quantization** — source-native ticks map to
//!   project PPQN ticks only when `delta * ppqn / division` is
//!   exact; any remainder is [`Error::Validation`].
//!
//! The selection is **plan-level data**, not persisted project
//! state: persisting a MIDI comp model is deferred until the #126
//! native-tick/chase event model is settled (see
//! `docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md`).
//! Like [`crate::project_comp`]'s [`crate::EffectiveComp`], the
//! plan is a pure, reproducible projection — nothing is mutated.
//!
//! [`MidiSource`]: crate::MidiSource

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    Project,
    error::Error,
    id::{MidiCompRegionId, NoteId, SourceId},
    midi::MidiNote,
    range::{MidiRange, MusicalRange},
    time::{MidiTick, MusicalTick},
};

/// One adopted phrase region of a MIDI comp selection: the half-open
/// source-native tick window `phrase` of `source_id`'s
/// [`crate::MidiSource`] placed on the project musical range
/// `project_range`.
///
/// A note is adopted iff its onset (`start`) lies inside `phrase`
/// (`[start, end)` semantics). A note that starts before `phrase`
/// is not chased in; a note whose end exceeds the mapped region end
/// is cut by a virtual off.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiPhraseRegion {
    /// Stable region identity inside the selection document.
    pub id: MidiCompRegionId,
    /// The MIDI source the phrase reads (must exist in
    /// `project.midi_sources`).
    pub source_id: SourceId,
    /// Source-native tick window selecting note onsets.
    pub phrase: MidiRange,
    /// Project musical placement `[start, end)` (non-empty).
    pub project_range: MusicalRange,
}

/// An ordered MIDI phrase comp selection — the comp's adopted
/// regions. Regions must be sorted by `project_range` and
/// non-overlapping; gaps are legal (they are silence).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiCompSelection {
    /// Adopted regions in project order.
    pub regions: Vec<MidiPhraseRegion>,
}

impl MidiCompSelection {
    /// Structural validation against the project: regions sorted and
    /// non-overlapping with unique ids, non-empty phrase/project
    /// ranges, per-source phrase windows non-overlapping (so a note
    /// onset can be adopted by at most one region — no duplicate
    /// `(source_id, note_id)` playback), and every `source_id`
    /// resolving to a [`crate::MidiSource`].
    ///
    /// Length-mapping checks run in [`compile_midi_comp`], which has
    /// the division context.
    ///
    /// # Errors
    /// [`Error::Validation`] on any violated rule.
    pub fn validate(&self, project: &Project) -> Result<(), Error> {
        let mut prev_end: Option<MusicalTick> = None;
        let mut seen_ids: BTreeSet<MidiCompRegionId> = BTreeSet::new();
        let mut phrases_by_source: BTreeMap<SourceId, Vec<MidiRange>> = BTreeMap::new();
        for region in &self.regions {
            if !seen_ids.insert(region.id) {
                return Err(Error::Validation(format!(
                    "midi comp region {}: duplicate region id",
                    region.id
                )));
            }
            if region.phrase.is_empty() {
                return Err(Error::Validation(format!(
                    "midi comp region {}: empty phrase range",
                    region.id
                )));
            }
            if region.project_range.is_empty() {
                return Err(Error::Validation(format!(
                    "midi comp region {}: empty project range",
                    region.id
                )));
            }
            if let Some(end) = prev_end
                && region.project_range.start() < end
            {
                return Err(Error::Validation(format!(
                    "midi comp region {}: project ranges overlap or are unsorted",
                    region.id
                )));
            }
            prev_end = Some(region.project_range.end());
            if !project.midi_sources.contains_key(&region.source_id) {
                return Err(Error::Validation(format!(
                    "midi comp region {}: source {} is not a MIDI source",
                    region.id, region.source_id
                )));
            }
            phrases_by_source
                .entry(region.source_id)
                .or_default()
                .push(region.phrase);
        }
        // A note onset inside two regions' phrases of the same source
        // would be adopted twice; per-source phrase windows must not
        // overlap (adjacent windows are fine — `[start, end)`).
        for (source_id, mut phrases) in phrases_by_source {
            phrases.sort_by_key(|r| (r.start().value(), r.end().value()));
            for pair in phrases.windows(2) {
                if pair[0].overlaps(&pair[1]) {
                    return Err(Error::Validation(format!(
                        "midi comp: overlapping phrase windows on source {source_id}"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// A controller kind the boundary oracle understands.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MidiControllerKind {
    /// CC controller number (0–127).
    Cc(u8),
    /// Pitch bend (14-bit, 0–16383; 8192 is center).
    PitchBend,
}

/// One controller event in source-native ticks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiControllerEvent {
    /// Source-native tick of the event.
    pub tick: MidiTick,
    /// MIDI channel the controller addresses (0–15). Two channels
    /// are distinct controller states — chase never mixes them.
    pub channel: u8,
    /// Controller addressed.
    pub kind: MidiControllerKind,
    /// Controller value (CC 0–127, pitch bend 0–16383).
    pub value: i32,
}

impl MidiControllerEvent {
    /// Enforces the wire-domain value contract.
    ///
    /// # Errors
    /// [`Error::Validation`] on out-of-domain channel/kind/value
    /// combinations.
    pub fn validate(&self) -> Result<(), Error> {
        if self.channel > 15 {
            return Err(Error::Validation(format!(
                "midi controller event at {}: channel {} > 15",
                self.tick.value(),
                self.channel
            )));
        }
        let ok = match self.kind {
            MidiControllerKind::Cc(n) => n <= 127 && (0..=127).contains(&self.value),
            MidiControllerKind::PitchBend => (0..=16_383).contains(&self.value),
        };
        if !ok {
            return Err(Error::Validation(format!(
                "midi controller event at {}: kind {:?} value {} out of domain",
                self.tick.value(),
                self.kind,
                self.value
            )));
        }
        Ok(())
    }
}

/// Controller events belonging to one MIDI source — plan-level input
/// standing in for the event data a future persisted model will own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiControllerLane {
    /// Source the events belong to.
    pub source_id: SourceId,
    /// Events in non-decreasing tick order (ties keep vector order —
    /// last write wins).
    pub events: Vec<MidiControllerEvent>,
}

/// Controller lanes accompanying a [`MidiCompSelection`]. Lanes may
/// name sources not present in the selection; their events are still
/// counted toward the chased kind union.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiControllerLanes {
    /// Per-source controller event lanes.
    pub lanes: Vec<MidiControllerLane>,
}

/// What a [`MidiPlanEvent`] does at its project tick.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MidiPlanAction {
    /// Note-on for `key` at `velocity`.
    NoteOn {
        /// Musical key (0–127).
        key: u8,
        /// Note-on velocity (1–127).
        velocity: u8,
    },
    /// Note-off for `key` (release velocity is not modeled).
    NoteOff {
        /// Musical key (0–127).
        key: u8,
    },
    /// Chase the given controller value at a region boundary —
    /// emitted at every region start so no state leaks across
    /// regions.
    ControllerChase {
        /// MIDI channel the controller addresses (0–15).
        channel: u8,
        /// Controller addressed.
        controller: MidiControllerKind,
        /// Chased value.
        value: i32,
    },
    /// All-notes-off at the end of the compiled plan.
    AllNotesOff,
}

/// One scheduled plan event in project musical ticks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiPlanEvent {
    /// Project musical tick.
    pub at: MusicalTick,
    /// The action.
    pub action: MidiPlanAction,
    /// Region that emitted the event.
    pub region: MidiCompRegionId,
    /// Source note identity for note events (`None` for controller
    /// and `AllNotesOff` actions).
    pub note: Option<(SourceId, NoteId)>,
    /// Source-native tick provenance (`note.start` for on,
    /// `note.end` for off; `None` otherwise).
    pub source_tick: Option<MidiTick>,
}

/// Boundary evidence for one adopted region.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiRegionReport {
    /// The region.
    pub region: MidiCompRegionId,
    /// Adopted notes `(source_id, note_id)`, sorted.
    pub notes: Vec<(SourceId, NoteId)>,
    /// Adopted notes cut at the region's project end (virtual
    /// offs) — the hanging-note guard's audit trail.
    pub virtual_offs: Vec<(SourceId, NoteId)>,
    /// Controller chase snapshot emitted at the region start:
    /// `((channel, kind), value)` sorted by `(channel, kind)`.
    /// Covers the union of `(channel, kind)` pairs declared in the
    /// input lanes; undeclared pairs chase their default
    /// (CC → 0, pitch bend → 8192).
    pub chase: Vec<((u8, MidiControllerKind), i32)>,
}

/// The deterministic playback plan compiled from a
/// [`MidiCompSelection`]. `events` are totally ordered:
/// `(at, action rank, region, note, source_tick)` where note-offs
/// precede controller chases which precede note-ons at the same
/// tick — adjacent regions therefore release before they retrigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiPlaybackPlan {
    /// Scheduled events in deterministic order.
    pub events: Vec<MidiPlanEvent>,
    /// Per-region boundary evidence in selection order.
    pub regions: Vec<MidiRegionReport>,
}

/// Default chase value for a controller kind never declared on a
/// region's own lane (CC → 0, pitch bend → center 8192).
fn default_value(kind: MidiControllerKind) -> i32 {
    match kind {
        MidiControllerKind::Cc(_) => 0,
        MidiControllerKind::PitchBend => 8_192,
    }
}

/// Maps a source-native tick delta to project PPQN ticks. Exact
/// rational only — a nonzero remainder is an error (no implicit
/// quantization of native ticks).
fn map_delta(delta: i64, division: u32, ppqn: u32) -> Result<i64, Error> {
    let num = i128::from(delta) * i128::from(ppqn);
    let den = i128::from(division);
    if num % den != 0 {
        return Err(Error::Validation(format!(
            "midi tick delta {delta} not representable in project ticks \
             (division {division}, ppqn {ppqn})"
        )));
    }
    i64::try_from(num / den).map_err(|_| Error::Overflow)
}

/// Action ordering rank at equal ticks: offs release before chases
/// establish state before ons retrigger.
fn action_rank(action: MidiPlanAction) -> u8 {
    match action {
        MidiPlanAction::NoteOff { .. } => 0,
        MidiPlanAction::AllNotesOff => 1,
        MidiPlanAction::ControllerChase { .. } => 2,
        MidiPlanAction::NoteOn { .. } => 3,
    }
}

/// Compiles `selection` into a deterministic [`MidiPlaybackPlan`].
///
/// Time mapping: a source-native delta maps to project ticks via
/// `delta * project_ppqn / division` and must be exact — the
/// stored native ticks are never requantized. Each region requires
/// its mapped phrase length to equal its project range length
/// (MIDI phrase comp is Musical+Musical natural 1:1; stretch/loop
/// mappings are out of scope).
///
/// # Errors
/// [`Error::Validation`] on structural violations, inexact tick
/// mapping, or length mismatch; [`Error::Overflow`] on range
/// arithmetic overflow.
#[allow(clippy::too_many_lines)] // region ごとの emit を直列に読める形に保つ
pub fn compile_midi_comp(
    project: &Project,
    selection: &MidiCompSelection,
    controllers: &MidiControllerLanes,
) -> Result<MidiPlaybackPlan, Error> {
    selection.validate(project)?;
    let ppqn = project.tempo_map.ppqn;

    // Validate controller input and collect the chased
    // (channel, kind) union — channel 0 CC64 and channel 1 CC64
    // are distinct controller states and never share a chase.
    let mut all_pairs: BTreeSet<(u8, MidiControllerKind)> = BTreeSet::new();
    for lane in &controllers.lanes {
        for event in &lane.events {
            event.validate()?;
            all_pairs.insert((event.channel, event.kind));
        }
    }

    let mut events: Vec<MidiPlanEvent> = Vec::new();
    let mut reports: Vec<MidiRegionReport> = Vec::with_capacity(selection.regions.len());

    for region in &selection.regions {
        let source = &project.midi_sources[&region.source_id];
        // `MidiDivision::Ppqn(0)` is constructible without going
        // through `MidiDivision::ppqn`; guard the divisor here so a
        // malformed in-memory source fails closed instead of
        // panicking on `% 0`.
        source.division.validate()?;
        let division = source.division.ticks_per_quarter();
        let phrase_start = region.phrase.start().value();
        let phrase_len = region
            .phrase
            .end()
            .value()
            .checked_sub(phrase_start)
            .ok_or(Error::Overflow)?;
        let proj_start = region.project_range.start().value();
        let proj_end = region.project_range.end().value();
        let proj_len = proj_end.checked_sub(proj_start).ok_or(Error::Overflow)?;

        // Natural 1:1 mapping: the phrase's mapped length must equal
        // the project range length exactly.
        let mapped_phrase_len = map_delta(phrase_len, division, ppqn)?;
        if mapped_phrase_len != proj_len {
            return Err(Error::Validation(format!(
                "midi comp region {}: mapped phrase length {mapped_phrase_len} \
                 != project range length {proj_len}",
                region.id
            )));
        }

        // Adopted notes: onset inside the phrase, deterministic order.
        let mut adopted: Vec<&MidiNote> = source
            .notes
            .values()
            .filter(|n| region.phrase.contains(n.start))
            .collect();
        adopted.sort_by_key(|n| (n.start.value(), n.id.raw()));

        let mut note_ids: Vec<(SourceId, NoteId)> = Vec::with_capacity(adopted.len());
        let mut virtual_offs: Vec<(SourceId, NoteId)> = Vec::new();

        for note in adopted {
            let on_delta = note
                .start
                .value()
                .checked_sub(phrase_start)
                .ok_or(Error::Overflow)?;
            let on_offset = map_delta(on_delta, division, ppqn)?;
            let on_at = MusicalTick::new(proj_start.checked_add(on_offset).ok_or(Error::Overflow)?);
            let end = note.end()?;
            let off_delta = end
                .value()
                .checked_sub(phrase_start)
                .ok_or(Error::Overflow)?;
            let off_offset = map_delta(off_delta, division, ppqn)?;
            let natural_off = proj_start.checked_add(off_offset).ok_or(Error::Overflow)?;
            // The note is cut at the region end when its natural off
            // lands beyond it — the virtual off prevents hanging.
            let (off_at, is_virtual) = if natural_off > proj_end {
                (proj_end, true)
            } else {
                (natural_off, false)
            };
            if is_virtual {
                virtual_offs.push((region.source_id, note.id));
            }
            note_ids.push((region.source_id, note.id));
            let provenance = Some((region.source_id, note.id));
            events.push(MidiPlanEvent {
                at: on_at,
                action: MidiPlanAction::NoteOn {
                    key: note.key,
                    velocity: note.velocity,
                },
                region: region.id,
                note: provenance,
                source_tick: Some(note.start),
            });
            events.push(MidiPlanEvent {
                at: MusicalTick::new(off_at),
                action: MidiPlanAction::NoteOff { key: note.key },
                region: region.id,
                note: provenance,
                source_tick: Some(end),
            });
        }

        // Controller chase at the region start: the effective state
        // of this source's lane at `phrase.start`, covering the union
        // of all declared (channel, kind) pairs so nothing leaks from
        // a previous region's source.
        let mut chase_map: BTreeMap<(u8, MidiControllerKind), i32> = BTreeMap::new();
        let lane_events: &[MidiControllerEvent] = controllers
            .lanes
            .iter()
            .find(|l| l.source_id == region.source_id)
            .map_or(&[], |l| l.events.as_slice());
        // Stable sort by tick keeps vector order for ties (last write
        // wins), matching the lane's documented chronological order.
        let mut ordered: Vec<&MidiControllerEvent> = lane_events.iter().collect();
        ordered.sort_by_key(|e| e.tick.value());
        for (channel, kind) in &all_pairs {
            let value = ordered
                .iter()
                .rev()
                .find(|e| {
                    e.channel == *channel && e.kind == *kind && e.tick.value() <= phrase_start
                })
                .map_or_else(|| default_value(*kind), |e| e.value);
            chase_map.insert((*channel, *kind), value);
        }
        for ((channel, kind), value) in &chase_map {
            events.push(MidiPlanEvent {
                at: MusicalTick::new(proj_start),
                action: MidiPlanAction::ControllerChase {
                    channel: *channel,
                    controller: *kind,
                    value: *value,
                },
                region: region.id,
                note: None,
                source_tick: None,
            });
        }

        // Controller events *inside* the phrase window play at their
        // mapped project ticks — the phrase carries its own controller
        // data, not just its boundary state.
        let phrase_end = region.phrase.end().value();
        for event in ordered
            .iter()
            .filter(|e| e.tick.value() > phrase_start && e.tick.value() < phrase_end)
        {
            let delta = event
                .tick
                .value()
                .checked_sub(phrase_start)
                .ok_or(Error::Overflow)?;
            let offset = map_delta(delta, division, ppqn)?;
            let at = proj_start.checked_add(offset).ok_or(Error::Overflow)?;
            events.push(MidiPlanEvent {
                at: MusicalTick::new(at),
                action: MidiPlanAction::ControllerChase {
                    channel: event.channel,
                    controller: event.kind,
                    value: event.value,
                },
                region: region.id,
                note: None,
                source_tick: Some(event.tick),
            });
        }

        reports.push(MidiRegionReport {
            region: region.id,
            notes: note_ids,
            virtual_offs,
            chase: chase_map.into_iter().collect(),
        });
    }

    // Comp end: release everything still conceptually held.
    let comp_end = selection.regions.last().map_or(MusicalTick::ZERO, |r| {
        // `MidiCompRegionId` has no natural "comp" id; reuse the
        // last region's id as the emitting region for the terminal
        // action — it is bookkeeping, not a note/voice identity.
        r.project_range.end()
    });
    let terminal_region = selection
        .regions
        .last()
        .map_or_else(|| MidiCompRegionId::new(0), |r| r.id);
    events.push(MidiPlanEvent {
        at: comp_end,
        action: MidiPlanAction::AllNotesOff,
        region: terminal_region,
        note: None,
        source_tick: None,
    });

    // Deterministic total order: (at, action rank, region, note,
    // source_tick). Note-less events sort before noted ones at the
    // same rank via the `Option` order (None < Some).
    events.sort_by(|a, b| {
        (
            a.at.value(),
            action_rank(a.action),
            a.region.raw(),
            a.note.map(|(_, n)| n.raw()),
            a.source_tick.map(MidiTick::value),
        )
            .cmp(&(
                b.at.value(),
                action_rank(b.action),
                b.region.raw(),
                b.note.map(|(_, n)| n.raw()),
                b.source_tick.map(MidiTick::value),
            ))
    });

    Ok(MidiPlaybackPlan {
        events,
        regions: reports,
    })
}
