//! RCD-04 MIDI Phrase boundary fixture — hanging note / controller
//! leak / stable-ID contract for the plan-level `midi_comp` oracle.
//!
//! Headless contract fixture only: `compile_midi_comp` evaluates a
//! `MidiCompSelection` against persisted `MidiSource` notes and
//! fixture-supplied controller lanes. It does not prove real
//! device/plugin/GUI behavior, and the selection is plan-level data
//! (persisted MIDI comp model is deferred — see the RCD-04 spec).

use synvelis_project::*;

const DIV: u32 = 480;
// project PPQN is fixed at 960 → source delta × 2 = project ticks.

// ---------- helpers ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "rcd-04-midi"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap();
}

fn midi_window() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(8 * 960).unwrap())
            .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn musical_clip(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
    }
}

fn mt(tick: i64) -> MidiTick {
    MidiTick::new(tick).unwrap()
}

/// Track → lane → MIDI source (division 480) → one clip + notes.
/// Returns `(lane, midi_source, clip)`.
fn midi_substrate(c: &mut ProjectController) -> (LaneId, SourceId, ClipId) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(DIV).unwrap(),
        },
    );
    let source = *c.live().midi_sources.keys().next().unwrap();
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: musical_clip(0),
            playback: midi_window(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    (lane, source, clip)
}

fn add_note(c: &mut ProjectController, source: SourceId, key: u8, vel: u8, start: i64, len: i64) {
    apply(
        c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key,
            velocity: vel,
            start: mt(start),
            len: mt(len),
        },
    );
}

/// The canonical two-region selection:
/// - region A: phrase `[480, 2400)` → project `[0, 3840)`
/// - region B: phrase `[2400, 3840)` → project `[3840, 6720)`
fn selection() -> MidiCompSelection {
    MidiCompSelection {
        regions: vec![
            MidiPhraseRegion {
                id: MidiCompRegionId::new(101),
                source_id: SourceId::new(0), // patched by caller
                phrase: MidiRange::try_new(mt(480), mt(2400)).unwrap(),
                project_range: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(3_840))
                    .unwrap(),
            },
            MidiPhraseRegion {
                id: MidiCompRegionId::new(102),
                source_id: SourceId::new(0),
                phrase: MidiRange::try_new(mt(2400), mt(3840)).unwrap(),
                project_range: MusicalRange::try_new(
                    MusicalTick::new(3_840),
                    MusicalTick::new(6_720),
                )
                .unwrap(),
            },
        ],
    }
}

fn selection_for(source: SourceId) -> MidiCompSelection {
    let mut s = selection();
    for r in &mut s.regions {
        r.source_id = source;
    }
    s
}

/// Controller lanes for the fixture source:
/// CC64 sustain, CC1 mod, CC10 (extra kind), pitch bend — some
/// before/inside/after each phrase window.
fn lanes_for(source: SourceId) -> MidiControllerLanes {
    let ev = |tick: i64, kind: MidiControllerKind, value: i32| MidiControllerEvent {
        tick: mt(tick),
        channel: 0,
        kind,
        value,
    };
    MidiControllerLanes {
        lanes: vec![MidiControllerLane {
            source_id: source,
            events: vec![
                ev(100, MidiControllerKind::Cc(64), 127),
                ev(600, MidiControllerKind::Cc(1), 64),
                ev(700, MidiControllerKind::PitchBend, 10_000),
                ev(2_000, MidiControllerKind::Cc(64), 64),
                ev(2_500, MidiControllerKind::Cc(10), 32),
                ev(3_000, MidiControllerKind::Cc(64), 0),
            ],
        }],
    }
}

/// Full fixture: substrate + notes + selection + lanes + plan.
fn fixture() -> (ProjectController, SourceId, ClipId, MidiPlaybackPlan) {
    let mut c = controller();
    let (_lane, source, clip) = midi_substrate(&mut c);
    // Notes (source-native ticks, division 480 → ×2 project ticks):
    // n1 [480,960)    → A: on 0,    off 960
    // n2 [960,2760)   → A: on 960,  virtual off 3840 (natural 4560)
    // n3 [1500,1800)  → A: on 2040, off 2640
    // n5 [100,150)    → excluded (onset before phrase A)
    // n7 [2399,2409)  → A: on 3838, virtual off 3840 (boundary tiny note)
    // n6 [2400,2880)  → B: on 3840, off 4800 (onset exactly at B start)
    // n4 [2500,3000)  → B: on 4040, off 5040
    add_note(&mut c, source, 60, 100, 480, 480);
    add_note(&mut c, source, 64, 90, 960, 1800);
    add_note(&mut c, source, 67, 80, 1500, 300);
    add_note(&mut c, source, 62, 60, 100, 50);
    add_note(&mut c, source, 63, 75, 2399, 10);
    add_note(&mut c, source, 65, 85, 2400, 480);
    add_note(&mut c, source, 72, 70, 2500, 500);
    let lanes = lanes_for(source);
    let sel = selection_for(source);
    let plan = compile_midi_comp(c.live(), &sel, &lanes).unwrap();
    (c, source, clip, plan)
}

fn note_id(c: &ProjectController, source: SourceId, key: u8) -> NoteId {
    c.live().midi_sources[&source]
        .notes
        .values()
        .find(|n| n.key == key)
        .unwrap()
        .id
}

// ---------------------------------------------------------------------
// F5: boundary safety — hanging notes / controller leaks
// ---------------------------------------------------------------------

#[test]
fn midi_phrase_plan_has_no_hanging_notes_or_leaks() {
    let (c, source, _clip, plan) = fixture();

    // Every NoteOn has exactly one NoteOff for the same note identity
    // at a tick >= the on; the active set is empty at plan end.
    let mut active: Vec<(SourceId, NoteId)> = Vec::new();
    let mut ons = 0usize;
    let mut offs = 0usize;
    for ev in &plan.events {
        match &ev.action {
            MidiPlanAction::NoteOn { .. } => {
                let id = ev.note.unwrap();
                assert!(
                    !active.contains(&id),
                    "note {id:?} double-triggered while active"
                );
                active.push(id);
                ons += 1;
            }
            MidiPlanAction::NoteOff { .. } => {
                let id = ev.note.unwrap();
                let pos = active
                    .iter()
                    .position(|a| *a == id)
                    .unwrap_or_else(|| panic!("note-off without note-on: {id:?}"));
                active.remove(pos);
                offs += 1;
            }
            _ => {}
        }
    }
    assert_eq!(ons, offs, "every on must be paired with an off");
    assert!(active.is_empty(), "hanging notes at plan end: {active:?}");

    // Terminal AllNotesOff at the last region's end.
    let last = plan.events.last().unwrap();
    assert_eq!(last.action, MidiPlanAction::AllNotesOff);
    assert_eq!(last.at, MusicalTick::new(6_720));

    // Region A virtual offs: n2 (key64) and n7 (key63) are cut at 3840.
    let report_a = &plan.regions[0];
    let n2 = note_id(&c, source, 64);
    let n7 = note_id(&c, source, 63);
    assert!(report_a.virtual_offs.contains(&(source, n2)));
    assert!(report_a.virtual_offs.contains(&(source, n7)));
    // Region B has no virtual offs.
    assert_eq!(plan.regions[1].virtual_offs.len(), 0);

    // Adopted sets: A = {60,64,67,63} (n5 key62 excluded — onset 100
    // < phrase start 480), B = {65,72}.
    let mut keys_a: Vec<u8> = c.live().midi_sources[&source]
        .notes
        .values()
        .filter(|n| report_a.notes.contains(&(source, n.id)))
        .map(|n| n.key)
        .collect();
    keys_a.sort_unstable();
    assert_eq!(keys_a, vec![60, 63, 64, 67]);
    let mut keys_b: Vec<u8> = c.live().midi_sources[&source]
        .notes
        .values()
        .filter(|n| plan.regions[1].notes.contains(&(source, n.id)))
        .map(|n| n.key)
        .collect();
    keys_b.sort_unstable();
    assert_eq!(keys_b, vec![65, 72]);
}

#[test]
fn midi_phrase_events_are_deterministic_and_ordered() {
    let (c, source, _clip, plan) = fixture();
    let lanes = lanes_for(source);
    let sel = selection_for(source);
    let again = compile_midi_comp(c.live(), &sel, &lanes).unwrap();
    assert_eq!(plan, again, "compile must be deterministic");

    // Total order: non-decreasing `at`; at equal ticks offs < chases
    // < ons (rank order), so adjacent regions release before retrigger.
    let mut prev: Option<(i64, u8)> = None;
    for ev in &plan.events {
        let rank = match &ev.action {
            MidiPlanAction::NoteOff { .. } => 0,
            MidiPlanAction::AllNotesOff => 1,
            MidiPlanAction::ControllerChase { .. } => 2,
            MidiPlanAction::NoteOn { .. } => 3,
        };
        let key = (ev.at.value(), rank);
        if let Some(p) = prev {
            assert!(key >= p, "events out of order at {key:?} after {p:?}");
        }
        prev = Some(key);
    }

    // At the shared boundary tick 3840: A's two virtual offs precede
    // B's chase snapshot which precedes B's first note-on (n6 key65).
    let boundary: Vec<&MidiPlanEvent> = plan
        .events
        .iter()
        .filter(|e| e.at == MusicalTick::new(3_840))
        .collect();
    let kinds: Vec<&MidiPlanAction> = boundary.iter().map(|e| &e.action).collect();
    // Virtual offs are emitted in adopted (start) order: n2 (key64,
    // start 960) before n7 (key63, start 2399).
    assert_eq!(kinds[0], &MidiPlanAction::NoteOff { key: 64 });
    assert_eq!(kinds[1], &MidiPlanAction::NoteOff { key: 63 });
    assert!(
        kinds[2..6]
            .iter()
            .all(|a| matches!(a, MidiPlanAction::ControllerChase { .. }))
    );
    assert_eq!(
        **kinds.last().unwrap(),
        MidiPlanAction::NoteOn {
            key: 65,
            velocity: 85
        }
    );
}

#[test]
fn midi_phrase_chase_covers_kind_union_and_mid_phrase_events_play() {
    let (_c, _source, _clip, plan) = fixture();

    // Region A chase at phrase.start=480: only events <= 480 count —
    // CC64 sustain 127 (@100); CC1/PB/CC10 are later → defaults.
    let chase_a = &plan.regions[0].chase;
    assert_eq!(chase_a.len(), 4, "kind union must be chased");
    let get = |k: MidiControllerKind| {
        chase_a
            .iter()
            .find(|((ch, kind), _)| *ch == 0 && *kind == k)
            .map(|(_, v)| *v)
    };
    assert_eq!(get(MidiControllerKind::Cc(64)), Some(127));
    assert_eq!(get(MidiControllerKind::Cc(1)), Some(0));
    assert_eq!(get(MidiControllerKind::Cc(10)), Some(0));
    assert_eq!(get(MidiControllerKind::PitchBend), Some(8_192));

    // Region B chase at phrase.start=2400 picks up CC64=64 (@2000)
    // and the earlier CC1/PB values; CC10 @2500 is inside B, not at
    // its start → default at chase, live event mid-phrase.
    let chase_b = &plan.regions[1].chase;
    let get_b = |k: MidiControllerKind| {
        chase_b
            .iter()
            .find(|((ch, kind), _)| *ch == 0 && *kind == k)
            .map(|(_, v)| *v)
    };
    assert_eq!(get_b(MidiControllerKind::Cc(64)), Some(64));
    assert_eq!(get_b(MidiControllerKind::Cc(1)), Some(64));
    assert_eq!(get_b(MidiControllerKind::PitchBend), Some(10_000));
    assert_eq!(get_b(MidiControllerKind::Cc(10)), Some(0));

    // Mid-phrase controller events play at mapped ticks:
    // A: CC1=64 @600→240, PB=10000 @700→440, CC64=64 @2000→3040.
    // B: CC10=32 @2500→4040, CC64=0 @3000→5040.
    let chase_at = |tick: i64, kind: MidiControllerKind, value: i32| {
        plan.events.iter().any(|e| {
            e.at == MusicalTick::new(tick)
                && matches!(
                    e.action,
                    MidiPlanAction::ControllerChase {
                        channel: 0,
                        controller,
                        value: v,
                    } if controller == kind && v == value
                )
        })
    };
    assert!(chase_at(240, MidiControllerKind::Cc(1), 64));
    assert!(chase_at(440, MidiControllerKind::PitchBend, 10_000));
    assert!(chase_at(3_040, MidiControllerKind::Cc(64), 64));
    assert!(chase_at(4_040, MidiControllerKind::Cc(10), 32));
    assert!(chase_at(5_040, MidiControllerKind::Cc(64), 0));
}

#[test]
fn midi_phrase_preserves_native_ticks_without_requantize() {
    let (c, source, _clip, plan) = fixture();
    // n7 onset at source tick 2399 (odd for div-480) maps exactly to
    // project tick 3838 (1919 * 960/480) — provenance keeps 2399.
    let n7 = note_id(&c, source, 63);
    let on = plan
        .events
        .iter()
        .find(|e| {
            e.note == Some((source, n7))
                && matches!(e.action, MidiPlanAction::NoteOn { key: 63, .. })
        })
        .unwrap();
    assert_eq!(on.at, MusicalTick::new(3_838));
    assert_eq!(on.source_tick, Some(mt(2_399)));

    // n1 onset at source tick 480 → project tick 0 (phrase-relative).
    let n1 = note_id(&c, source, 60);
    let on1 = plan
        .events
        .iter()
        .find(|e| {
            e.note == Some((source, n1))
                && matches!(e.action, MidiPlanAction::NoteOn { key: 60, .. })
        })
        .unwrap();
    assert_eq!(on1.at, MusicalTick::ZERO);
    assert_eq!(on1.source_tick, Some(mt(480)));
}

// ---------------------------------------------------------------------
// F5: stable IDs — undo/redo, save/reload, serialized diff
// ---------------------------------------------------------------------

#[test]
fn midi_phrase_stable_ids_survive_undo_redo_save_reload() {
    let (mut c, source, clip, _plan) = fixture();

    // Undo/redo a note edit: the NoteId and restored fields are
    // identical — edits never mint new identities.
    let n1 = note_id(&c, source, 60);
    let before = c.live().midi_sources[&source].notes[&n1].clone();
    apply(
        &mut c,
        ProjectOp::MoveMidiNote {
            source_id: source,
            note_id: n1,
            start: mt(500),
        },
    );
    assert_eq!(c.live().midi_sources[&source].notes[&n1].start, mt(500));
    assert!(c.undo().unwrap());
    assert_eq!(c.live().midi_sources[&source].notes[&n1], before);
    assert!(c.redo().unwrap());
    assert_eq!(c.live().midi_sources[&source].notes[&n1].start, mt(500));
    assert!(c.undo().unwrap());

    // Save → load into a fresh controller: every stable ID and the
    // serialized project are identical (VCS-diff-equivalent check —
    // the canonical JSON is the diff surface).
    let json_before = serde_json::to_string(c.live()).unwrap();
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    let json_after = serde_json::to_string(c2.live()).unwrap();
    assert_eq!(json_before, json_after, "save/reload must be identity");

    // Source/clip/note identities are the tagged stable forms.
    assert!(source.to_tagged().starts_with("source-"));
    assert!(clip.to_tagged().starts_with("clip-"));
    assert!(n1.to_tagged().starts_with("note-"));
    for (id, note) in &c2.live().midi_sources[&source].notes {
        assert_eq!(*id, note.id, "map key must equal note.id");
    }
}

#[test]
fn midi_selection_serializes_with_stable_region_ids() {
    let (_c, source, _clip, _plan) = fixture();
    let sel = selection_for(source);
    let json = serde_json::to_string(&sel).unwrap();
    let back: MidiCompSelection = serde_json::from_str(&json).unwrap();
    assert_eq!(sel, back, "selection round-trip must preserve IDs");
    assert!(json.contains("midi-comp-region-"));
}

// ---------------------------------------------------------------------
// Rejection matrix — malformed selections fail closed
// ---------------------------------------------------------------------

fn expect_compile_err(project: &Project, sel: &MidiCompSelection, lanes: &MidiControllerLanes) {
    assert!(
        compile_midi_comp(project, sel, lanes).is_err(),
        "selection must be rejected: {sel:?}"
    );
}

#[test]
fn midi_phrase_rejects_overlapping_unsorted_and_empty_regions() {
    let mut c = controller();
    let (_l, source, _cl) = midi_substrate(&mut c);
    add_note(&mut c, source, 60, 100, 480, 480);
    let lanes = lanes_for(source);

    // Overlapping project ranges.
    let mut sel = selection_for(source);
    sel.regions[1].project_range =
        MusicalRange::try_new(MusicalTick::new(1_000), MusicalTick::new(5_000)).unwrap();
    expect_compile_err(c.live(), &sel, &lanes);

    // Unsorted regions (B placed before A).
    let mut sel = selection_for(source);
    sel.regions.swap(0, 1);
    expect_compile_err(c.live(), &sel, &lanes);

    // Empty phrase range.
    let mut sel = selection_for(source);
    sel.regions[0].phrase = MidiRange::try_new(mt(480), mt(480)).unwrap();
    expect_compile_err(c.live(), &sel, &lanes);

    // Empty project range.
    let mut sel = selection_for(source);
    sel.regions[0].project_range =
        MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(0)).unwrap();
    expect_compile_err(c.live(), &sel, &lanes);
}

#[test]
fn midi_phrase_rejects_non_midi_and_missing_sources() {
    let mut c = controller();
    let (_l, midi_src, _cl) = midi_substrate(&mut c);
    add_note(&mut c, midi_src, 60, 100, 480, 480);

    // A media SourceId is not a MIDI source.
    apply(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: "aa".into(),
        },
    );
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "aa".into(),
        },
    );
    let media_src = *c.live().sources.keys().next().unwrap();
    let lanes = lanes_for(midi_src);
    let mut sel = selection_for(media_src);
    sel.regions[0].phrase = MidiRange::try_new(mt(0), mt(960)).unwrap();
    sel.regions[0].project_range =
        MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(1_920)).unwrap();
    sel.regions.remove(1);
    expect_compile_err(c.live(), &sel, &lanes);

    // A SourceId that exists nowhere.
    let mut sel = selection_for(SourceId::new(9_999));
    sel.regions[0].phrase = MidiRange::try_new(mt(0), mt(960)).unwrap();
    sel.regions[0].project_range =
        MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(1_920)).unwrap();
    sel.regions.remove(1);
    expect_compile_err(c.live(), &sel, &lanes);
}

#[test]
fn midi_phrase_rejects_length_mismatch_and_inexact_mapping() {
    let mut c = controller();
    let (_l, source, _cl) = midi_substrate(&mut c);
    add_note(&mut c, source, 60, 100, 480, 480);
    let lanes = lanes_for(source);

    // Phrase maps to 3840 project ticks but the region claims 4000.
    let mut sel = selection_for(source);
    sel.regions.remove(1);
    sel.regions[0].project_range =
        MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(4_000)).unwrap();
    expect_compile_err(c.live(), &sel, &lanes);

    // Division 1000: phrase [0,1000) maps exactly to 960 project
    // ticks, but a note onset at delta 1 does not (960 % 1000 != 0)
    // — native ticks are never implicitly quantized.
    let mut c2 = controller();
    apply(&mut c2, ProjectOp::AddTrack { name: "t".into() });
    let track = *c2.live().tracks.keys().next().unwrap();
    apply(&mut c2, ProjectOp::AddLane { track_id: track });
    let _lane = *c2.live().lanes.keys().next().unwrap();
    apply(
        &mut c2,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(1_000).unwrap(),
        },
    );
    let src2 = *c2.live().midi_sources.keys().next().unwrap();
    add_note(&mut c2, src2, 60, 100, 1, 500);
    let sel2 = MidiCompSelection {
        regions: vec![MidiPhraseRegion {
            id: MidiCompRegionId::new(7),
            source_id: src2,
            phrase: MidiRange::try_new(mt(0), mt(1_000)).unwrap(),
            project_range: MusicalRange::try_new(MusicalTick::ZERO, MusicalTick::new(960)).unwrap(),
        }],
    };
    expect_compile_err(c2.live(), &sel2, &MidiControllerLanes::default());
}

#[test]
fn midi_phrase_rejects_duplicate_ids_overlapping_phrases_and_zero_division() {
    let mut c = controller();
    let (_l, source, _cl) = midi_substrate(&mut c);
    add_note(&mut c, source, 60, 100, 480, 480);
    let lanes = lanes_for(source);

    // Region ids are the stable identity — duplicates are rejected.
    let mut sel = selection_for(source);
    sel.regions[1].id = sel.regions[0].id;
    expect_compile_err(c.live(), &sel, &lanes);

    // Overlapping phrase windows on the same source would adopt the
    // same note onset twice; per-source phrases must not overlap so
    // `(source_id, note_id)` playback stays unique. (validate()
    // rejects before the length-mapping check runs.)
    let mut sel = selection_for(source);
    sel.regions[1].phrase = MidiRange::try_new(mt(2_000), mt(3_000)).unwrap();
    expect_compile_err(c.live(), &sel, &lanes);

    // The canonical adjacent-window selection still compiles.
    let sel_ok = selection_for(source);
    assert!(compile_midi_comp(c.live(), &sel_ok, &lanes).is_ok());

    // `MidiDivision::Ppqn(0)` bypasses `MidiDivision::ppqn`; an
    // in-memory source carrying it must fail closed (Error), not
    // panic on `% 0`.
    let mut p = c.live().clone();
    let mut src = p.midi_sources[&source].clone();
    src.division = MidiDivision::Ppqn(0);
    p.midi_sources.insert(source, src);
    let sel = selection_for(source);
    assert!(compile_midi_comp(&p, &sel, &lanes).is_err());
}

#[test]
fn midi_phrase_rejects_out_of_domain_controller_events() {
    let mut c = controller();
    let (_l, source, _cl) = midi_substrate(&mut c);
    add_note(&mut c, source, 60, 100, 480, 480);
    let sel = selection_for(source);

    let bad = MidiControllerLanes {
        lanes: vec![MidiControllerLane {
            source_id: source,
            events: vec![MidiControllerEvent {
                tick: mt(100),
                channel: 0,
                kind: MidiControllerKind::Cc(64),
                value: 200, // CC domain is 0–127
            }],
        }],
    };
    assert!(compile_midi_comp(c.live(), &sel, &bad).is_err());
}
