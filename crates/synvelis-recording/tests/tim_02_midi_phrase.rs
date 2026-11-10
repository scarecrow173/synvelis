//! TIM-02 SMF → MIDI Phrase consumption fixture (issue #126).
//!
//! `synvelis-recording` owns no MIDI state of its own — the Take/
//! Phrase consumption path runs entirely on `synvelis-project`'s
//! persisted model: `ProjectOp::ImportMidiSmf` stores the provenance
//! document, `SmfDocument::controller_events` feeds the plan-level
//! `MidiControllerLanes`, and `compile_midi_comp` lowers a
//! `MidiCompSelection` (phrase) into a `MidiPlaybackPlan`.
//!
//! This fixture proves the imported SMF survives that path with
//! native ticks, per-channel controller state, and zero hanging
//! notes at the comp end (every `NoteOn` closed, terminal
//! `AllNotesOff`). It is a headless contract check — no real
//! device/plugin/OS backend, GUI, or RT timing is exercised.

use synvelis_project::*;

// ---------- SMF byte builders (same wire helpers as the project
// fixture; kept local so the recording crate's contract test does
// not reach into another crate's test module) ----------

fn vlq(mut v: u64) -> Vec<u8> {
    let mut buf = [0u8; 8];
    let mut n = 0;
    loop {
        buf[n] = (v & 0x7F) as u8;
        v >>= 7;
        n += 1;
        if v == 0 {
            break;
        }
    }
    let mut out = Vec::with_capacity(n);
    for i in (0..n).rev() {
        let mut b = buf[i];
        if i != 0 {
            b |= 0x80;
        }
        out.push(b);
    }
    out
}

fn ev(delta: u64, body: &[u8]) -> Vec<u8> {
    let mut v = vlq(delta);
    v.extend_from_slice(body);
    v
}

fn meta(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0xFF, kind];
    v.extend(vlq(payload.len() as u64));
    v.extend_from_slice(payload);
    v
}

fn eot() -> Vec<u8> {
    vec![0xFF, 0x2F, 0x00]
}

fn note_on(channel: u8, key: u8, velocity: u8) -> Vec<u8> {
    vec![0x90 | channel, key, velocity]
}

fn note_off(channel: u8, key: u8, velocity: u8) -> Vec<u8> {
    vec![0x80 | channel, key, velocity]
}

fn cc(channel: u8, controller: u8, value: u8) -> Vec<u8> {
    vec![0xB0 | channel, controller, value]
}

fn mtrk(events: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for e in events {
        body.extend_from_slice(e);
    }
    let mut v = b"MTrk".to_vec();
    v.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
    v.extend_from_slice(&body);
    v
}

fn smf_file(format: u16, division: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let mut v = b"MThd".to_vec();
    v.extend_from_slice(&6u32.to_be_bytes());
    v.extend_from_slice(&format.to_be_bytes());
    v.extend_from_slice(&u16::try_from(tracks.len()).unwrap().to_be_bytes());
    v.extend_from_slice(&division.to_be_bytes());
    for t in tracks {
        v.extend_from_slice(t);
    }
    v
}

// ---------- project helpers ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "tim-02-midi-phrase"),
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

fn mt(tick: i64) -> MidiTick {
    MidiTick::new(tick).unwrap()
}

/// Format 1, division 480: conductor + two event tracks. CC64
/// sustain differs per channel (ch0=127, ch1=64); a mid-phrase ch0
/// sustain release sits inside the adopted window. All ticks even →
/// exact ×2 mapping onto the project's 960 PPQN.
fn fixture_smf() -> Vec<u8> {
    smf_file(
        1,
        480,
        &[
            mtrk(&[
                ev(0, &meta(0x51, &[0x07, 0xA1, 0x20])), // tempo 500000
                ev(0, &meta(0x58, &[4, 2, 24, 8])),      // 4/4
                ev(0, &eot()),
            ]),
            mtrk(&[
                ev(0, &cc(0, 64, 127)),
                ev(240, &note_on(0, 60, 100)),
                ev(240, &cc(0, 64, 0)), // sustain release mid-phrase
                ev(240, &note_off(0, 60, 64)),
                ev(0, &eot()),
            ]),
            mtrk(&[
                ev(0, &cc(1, 64, 64)),
                ev(120, &note_on(1, 64, 90)),
                ev(360, &note_off(1, 64, 40)),
                ev(0, &eot()),
            ]),
        ],
    )
}

/// Every `(source, note)` `NoteOn` is matched by a `NoteOff` and the
/// plan terminates in `AllNotesOff` — 発音残り 0.
fn assert_no_hanging_notes(plan: &MidiPlaybackPlan) {
    let mut open: std::collections::BTreeMap<(SourceId, NoteId), i64> =
        std::collections::BTreeMap::new();
    for e in &plan.events {
        if let Some(note) = e.note {
            match e.action {
                MidiPlanAction::NoteOn { .. } => *open.entry(note).or_default() += 1,
                MidiPlanAction::NoteOff { .. } => *open.entry(note).or_default() -= 1,
                _ => {}
            }
        }
    }
    assert!(
        open.values().all(|&v| v == 0),
        "hanging notes remain: {open:?}"
    );
    assert!(matches!(
        plan.events.last().map(|e| &e.action),
        Some(MidiPlanAction::AllNotesOff)
    ));
}

#[test]
fn smf_import_feeds_phrase_plan_with_zero_hanging_notes() {
    let mut c = controller();
    let doc = parse_smf(&fixture_smf()).unwrap();
    apply(
        &mut c,
        ProjectOp::ImportMidiSmf {
            document: doc.clone(),
        },
    );
    let source = *c.live().smf_documents.keys().next().unwrap();

    // The persisted projection: two notes on the native 480 division.
    assert_eq!(c.live().midi_sources[&source].notes.len(), 2);
    assert_eq!(
        c.live().midi_sources[&source].division.ticks_per_quarter(),
        480
    );

    // Take/Phrase consumption: the document's controller projection
    // becomes the plan lane verbatim (channel included).
    let lanes = MidiControllerLanes {
        lanes: vec![MidiControllerLane {
            source_id: source,
            events: doc.controller_events(),
        }],
    };
    assert_eq!(lanes.lanes[0].events.len(), 3);
    assert!(lanes.lanes[0].events.iter().all(|e| e.channel <= 15));

    // Phrase window covering both note onsets: [0, 960) source ticks
    // → [0, 1920) project ticks (exact ×2 at PPQN 960).
    let selection = MidiCompSelection {
        regions: vec![MidiPhraseRegion {
            id: MidiCompRegionId::new(301),
            source_id: source,
            phrase: MidiRange::try_new(mt(0), mt(960)).unwrap(),
            project_range: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(1_920))
                .unwrap(),
        }],
    };
    let plan = compile_midi_comp(c.live(), &selection, &lanes).unwrap();

    // Both notes adopted at their mapped project ticks — native
    // timing preserved, no requantization (480 → 960 PPQN is ×2).
    assert_eq!(plan.regions[0].notes.len(), 2);
    let ons: Vec<(u8, i64)> = plan
        .events
        .iter()
        .filter_map(|e| match e.action {
            MidiPlanAction::NoteOn { key, .. } => Some((key, e.at.value())),
            _ => None,
        })
        .collect();
    // ch1 key64 @120→240, ch0 key60 @240→480.
    assert!(ons.contains(&(64, 240)));
    assert!(ons.contains(&(60, 480)));

    // Channel-aware chase: ch0 sustain 127 and ch1 sustain 64 are
    // distinct states; the mid-phrase ch0 release (source @480 →
    // project 960) plays live inside the window.
    let chase = &plan.regions[0].chase;
    assert!(chase.contains(&((0, MidiControllerKind::Cc(64)), 127)));
    assert!(chase.contains(&((1, MidiControllerKind::Cc(64)), 64)));
    assert!(plan.events.iter().any(|e| {
        e.at == MusicalTick::new(960)
            && matches!(
                e.action,
                MidiPlanAction::ControllerChase {
                    channel: 0,
                    controller: MidiControllerKind::Cc(64),
                    value: 0,
                }
            )
    }));

    // 発音残り 0: every adopted note closes and the plan panics.
    assert_no_hanging_notes(&plan);
}

#[test]
fn smf_import_then_export_reimport_roundtrips_through_project() {
    // Export → Reimport on the same project keeps the document
    // byte-semantics: the re-imported document equals the original.
    let mut c = controller();
    let doc = parse_smf(&fixture_smf()).unwrap();
    apply(
        &mut c,
        ProjectOp::ImportMidiSmf {
            document: doc.clone(),
        },
    );
    let source = *c.live().smf_documents.keys().next().unwrap();

    let file = export_smf(
        c.live(),
        &source,
        &SmfExportOptions {
            format: SmfFormat::Format1,
            division: 480,
        },
    )
    .unwrap();
    assert_eq!(file.report.rounded_count, 0);

    let reparsed = parse_smf(&file.bytes).unwrap();
    assert_eq!(reparsed, doc);

    // A second import produces an independent source + document.
    apply(&mut c, ProjectOp::ImportMidiSmf { document: reparsed });
    assert_eq!(c.live().smf_documents.len(), 2);
    assert_eq!(c.live().midi_sources.len(), 2);
}
