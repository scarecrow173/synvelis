//! TIM-02 SMF Format 0/1 fixture matrix (issue #126) — parse/export
//! provenance, project import guards, typed errors, deterministic
//! ordering, conversion accounting, and channel-aware chase.
//!
//! Headless contract fixtures only: no real device/plugin/OS/GUI/RT
//! behavior is exercised (those stay `NotRun` — see the test-spec).

use std::collections::BTreeMap;

use synvelis_project::*;

// ---------- SMF byte builders ----------

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

/// One track event: delta varlen + event bytes.
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

fn tempo(us: u32) -> Vec<u8> {
    meta(0x51, &us.to_be_bytes()[1..])
}

fn timesig(numerator: u8, denominator_log2: u8, thirtyseconds: u8, clocks: u8) -> Vec<u8> {
    meta(0x58, &[numerator, denominator_log2, thirtyseconds, clocks])
}

fn trackname(name: &str) -> Vec<u8> {
    meta(0x03, name.as_bytes())
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

fn pitch_bend(channel: u8, value: u16) -> Vec<u8> {
    vec![
        0xE0 | channel,
        u8::try_from(value & 0x7F).unwrap(),
        u8::try_from(value >> 7).unwrap(),
    ]
}

fn program_change(channel: u8, program: u8) -> Vec<u8> {
    vec![0xC0 | channel, program]
}

fn channel_aftertouch(channel: u8, value: u8) -> Vec<u8> {
    vec![0xD0 | channel, value]
}

fn poly_aftertouch(channel: u8, key: u8, value: u8) -> Vec<u8> {
    vec![0xA0 | channel, key, value]
}

fn sysex_f0(data: &[u8]) -> Vec<u8> {
    let mut v = vec![0xF0];
    v.extend(vlq(data.len() as u64));
    v.extend_from_slice(data);
    v
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

// ---------- rich fixtures ----------

/// Format 0, division 480. Event order is already in canonical emit
/// order (meta < sysex < note-off < controller < note-on < eot at
/// equal ticks) so `parse → export → parse` is a `PartialEq` golden.
fn format0_rich() -> Vec<u8> {
    smf_file(
        0,
        480,
        &[mtrk(&[
            ev(0, &tempo(500_000)),
            ev(0, &timesig(4, 2, 24, 8)),
            ev(0, &trackname("Trk0")),
            ev(0, &meta(0x01, b"hello")),
            ev(0, &cc(0, 64, 127)),
            ev(0, &cc(1, 64, 64)),
            ev(120, &note_on(0, 60, 100)),
            ev(240, &note_off(0, 60, 64)),
            // Velocity-0 Note On on the wire is a Note Off.
            ev(0, &note_on(0, 60, 0)),
            ev(0, &note_on(1, 64, 90)),
            // At tick 840: sysex(rank2) → note-off(rank3) →
            // controllers(rank4) → eot(rank6) — canonical order.
            ev(480, &sysex_f0(&[0x7E, 0x00, 0x09])),
            ev(0, &note_off(1, 64, 40)),
            ev(0, &pitch_bend(0, 9_000)),
            ev(0, &program_change(2, 5)),
            ev(0, &channel_aftertouch(3, 100)),
            ev(0, &poly_aftertouch(4, 62, 77)),
            ev(0, &eot()),
        ])],
    )
}

/// Format 1, division 480: conductor track + two event tracks on
/// different channels. Ticks stay even so project-PPQN 960 mapping
/// is exact (×2).
fn format1_two_channels() -> Vec<u8> {
    smf_file(
        1,
        480,
        &[
            mtrk(&[
                ev(0, &tempo(500_000)),
                ev(0, &timesig(4, 2, 24, 8)),
                ev(0, &trackname("Conductor")),
                ev(0, &eot()),
            ]),
            mtrk(&[
                ev(0, &cc(0, 64, 127)),
                ev(240, &note_on(0, 60, 100)),
                ev(480, &note_off(0, 60, 64)),
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

// ---------- project helpers ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "tim-02-smf"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn apply(c: &mut ProjectController, op: ProjectOp) {
    c.apply(cmd(c, op)).unwrap();
}

fn try_apply(c: &mut ProjectController, op: ProjectOp) -> Result<(), Error> {
    c.apply(cmd(c, op)).map(|_| ())
}

fn mt(tick: i64) -> MidiTick {
    MidiTick::new(tick).unwrap()
}

fn import_doc(c: &mut ProjectController, bytes: &[u8]) -> (SmfDocument, SourceId) {
    let doc = parse_smf(bytes).unwrap();
    apply(
        c,
        ProjectOp::ImportMidiSmf {
            document: doc.clone(),
        },
    );
    let source = *c.live().smf_documents.keys().next().unwrap();
    (doc, source)
}

fn musical_clip(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
    }
}

fn midi_window() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(4 * 960).unwrap())
            .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

// ===========================================================================
// Parse: structure, provenance, normalization
// ===========================================================================

#[test]
#[allow(clippy::too_many_lines)]
fn smf_format0_parse_preserves_full_event_stream() {
    let doc = parse_smf(&format0_rich()).unwrap();
    assert_eq!(doc.format, SmfFormat::Format0);
    assert_eq!(doc.division, 480);
    assert_eq!(doc.tracks.len(), 1);
    let events = &doc.tracks[0].events;
    // 17 wire events: the velocity-0 Note On parses as NoteOff.
    assert_eq!(events.len(), 17);

    let kinds: Vec<&SmfEventKind> = events.iter().map(|e| &e.kind).collect();
    assert_eq!(
        kinds[0],
        &SmfEventKind::Tempo {
            us_per_quarter: 500_000
        }
    );
    assert_eq!(
        kinds[1],
        &SmfEventKind::TimeSignature {
            numerator: 4,
            denominator: 2,
            thirtyseconds: 24,
            clocks: 8,
        }
    );
    assert_eq!(
        kinds[2],
        &SmfEventKind::TrackName {
            bytes: b"Trk0".to_vec()
        }
    );
    // Unknown meta type 0x01 stays raw provenance.
    assert_eq!(
        kinds[3],
        &SmfEventKind::MetaOther {
            meta_kind: 0x01,
            data: b"hello".to_vec(),
        }
    );
    assert_eq!(
        kinds[4],
        &SmfEventKind::Cc {
            channel: 0,
            controller: 64,
            value: 127,
        }
    );
    assert_eq!(
        kinds[5],
        &SmfEventKind::Cc {
            channel: 1,
            controller: 64,
            value: 64,
        }
    );
    assert_eq!(
        kinds[6],
        &SmfEventKind::NoteOn {
            channel: 0,
            key: 60,
            velocity: 100,
        }
    );
    assert_eq!(events[6].tick, mt(120));
    assert_eq!(
        kinds[7],
        &SmfEventKind::NoteOff {
            channel: 0,
            key: 60,
            velocity: 64,
        }
    );
    assert_eq!(events[7].tick, mt(360));
    // Wire `90 3C 00` → NoteOff velocity 0, never a zero-velocity NoteOn.
    assert_eq!(
        kinds[8],
        &SmfEventKind::NoteOff {
            channel: 0,
            key: 60,
            velocity: 0,
        }
    );
    assert_eq!(
        kinds[9],
        &SmfEventKind::NoteOn {
            channel: 1,
            key: 64,
            velocity: 90,
        }
    );
    assert_eq!(events[9].tick, mt(360));
    assert_eq!(
        kinds[10],
        &SmfEventKind::Sysex {
            prefix: 0xF0,
            data: vec![0x7E, 0x00, 0x09],
        }
    );
    assert_eq!(events[10].tick, mt(840));
    assert!(matches!(
        kinds[11],
        SmfEventKind::NoteOff { channel: 1, .. }
    ));
    assert_eq!(
        kinds[12],
        &SmfEventKind::PitchBend {
            channel: 0,
            value: 9_000,
        }
    );
    assert_eq!(
        kinds[13],
        &SmfEventKind::ProgramChange {
            channel: 2,
            program: 5,
        }
    );
    assert_eq!(
        kinds[14],
        &SmfEventKind::ChannelAftertouch {
            channel: 3,
            value: 100,
        }
    );
    assert_eq!(
        kinds[15],
        &SmfEventKind::PolyAftertouch {
            channel: 4,
            key: 62,
            value: 77,
        }
    );
    assert_eq!(kinds[16], &SmfEventKind::EndOfTrack);
    assert_eq!(events[16].tick, mt(840));
}

#[test]
fn smf_format1_parse_preserves_tracks_and_channels() {
    let doc = parse_smf(&format1_two_channels()).unwrap();
    assert_eq!(doc.format, SmfFormat::Format1);
    assert_eq!(doc.division, 480);
    assert_eq!(doc.tracks.len(), 3);

    // Conductor track: meta only.
    assert_eq!(doc.tracks[0].events.len(), 4);
    assert!(matches!(
        doc.tracks[0].events[0].kind,
        SmfEventKind::Tempo { .. }
    ));

    // Track 1: channel 0 note + CC64 sustain.
    let t1 = &doc.tracks[1].events;
    assert!(matches!(
        t1[0].kind,
        SmfEventKind::Cc {
            channel: 0,
            controller: 64,
            ..
        }
    ));
    assert!(matches!(
        t1[1].kind,
        SmfEventKind::NoteOn {
            channel: 0,
            key: 60,
            ..
        }
    ));
    assert_eq!(t1[1].tick, mt(240));
    assert!(matches!(
        t1[2].kind,
        SmfEventKind::NoteOff {
            channel: 0,
            key: 60,
            ..
        }
    ));
    assert_eq!(t1[2].tick, mt(720));

    // Track 2: channel 1 note + CC64 sustain — distinct channel state.
    let t2 = &doc.tracks[2].events;
    assert!(matches!(
        t2[0].kind,
        SmfEventKind::Cc {
            channel: 1,
            controller: 64,
            value: 64,
        }
    ));
    assert!(matches!(
        t2[1].kind,
        SmfEventKind::NoteOn {
            channel: 1,
            key: 64,
            ..
        }
    ));
    assert_eq!(t2[1].tick, mt(120));
}

#[test]
fn smf_running_status_is_parsed_but_never_emitted() {
    // Hand-built track: explicit status, then data-only events that
    // reuse running status; a meta event in between does not clear it.
    let mut body = Vec::new();
    body.extend_from_slice(&[0x00, 0x90, 0x3C, 0x64]); // NoteOn ch0 k60 v100 @0
    body.extend_from_slice(&[0x10, 0x80, 0x3C, 0x40]); // NoteOff ch0 k60 @16
    body.extend_from_slice(&[0x00, 0xFF, 0x03, 0x03, b'f', b'o', b'o']); // meta @16
    body.extend_from_slice(&[0x10, 0x90, 0x3C, 0x50]); // explicit NoteOn @32 (running = 0x90)
    body.extend_from_slice(&[0x00, 0x3C, 0x00]); // running 0x90 v0 → NoteOff @32
    body.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]); // EOT @32

    let mut track_chunk = b"MTrk".to_vec();
    track_chunk.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
    track_chunk.extend_from_slice(&body);
    let bytes = smf_file(0, 480, &[track_chunk]);

    let doc = parse_smf(&bytes).unwrap();
    let events = &doc.tracks[0].events;
    assert_eq!(events.len(), 6);
    assert!(matches!(
        events[3].kind,
        SmfEventKind::NoteOn {
            channel: 0,
            key: 60,
            velocity: 80,
        }
    ));
    assert_eq!(events[3].tick, mt(32));
    assert!(matches!(
        events[4].kind,
        SmfEventKind::NoteOff {
            channel: 0,
            key: 60,
            velocity: 0,
        }
    ));
    assert_eq!(events[4].tick, mt(32));

    // Export emits explicit status bytes only — no running status.
    let mut project = Project::new(ProjectId::new(0), "rt");
    let sid = SourceId::new(1);
    project.midi_sources.insert(
        sid,
        MidiSource {
            id: sid,
            division: MidiDivision::ppqn(480).unwrap(),
            notes: BTreeMap::new(),
        },
    );
    project.smf_documents.insert(sid, doc);
    let file = export_smf(
        &project,
        &sid,
        &SmfExportOptions {
            format: SmfFormat::Format0,
            division: 480,
        },
    )
    .unwrap();
    // After every delta varlen, the next byte must be a status byte
    // (>= 0x80) — running status would leave a data byte there.
    let reparsed = parse_smf(&file.bytes).unwrap();
    assert_eq!(reparsed.tracks[0].events.len(), 6);
    // Canonical bytes: every channel event carries its status byte.
    assert!(file.bytes.windows(2).any(|w| w == [0x90, 0x3C]));
}

#[test]
fn smf_paired_notes_fifo_pairs_and_projection_order() {
    // Overlapping same-(channel,key) notes pair FIFO; unmatched
    // NoteOffs stay in the document but not the projection.
    let bytes = smf_file(
        0,
        480,
        &[mtrk(&[
            ev(0, &note_on(0, 60, 100)),
            ev(100, &note_on(0, 60, 90)), // second onset before first off
            ev(100, &note_off(0, 60, 64)), // closes the FIRST onset
            ev(100, &note_off(0, 60, 64)), // closes the second
            ev(0, &note_off(0, 62, 64)),  // unmatched off — ignored
            ev(0, &note_on(1, 60, 80)),   // same key, different channel
            ev(0, &note_off(1, 60, 64)),
            ev(0, &eot()),
        ])],
    );
    let doc = parse_smf(&bytes).unwrap();
    let specs = doc.paired_notes().unwrap();
    // The ch1 note (on@200, off@200) is zero-length → excluded; the
    // unmatched ch0 key-62 off never enters the projection.
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[0].channel, 0);
    assert_eq!(specs[0].key, 60);
    assert_eq!(specs[0].velocity, 100);
    assert_eq!(specs[0].start, mt(0));
    assert_eq!(specs[0].len, mt(200));
    assert_eq!(specs[1].velocity, 90);
    assert_eq!(specs[1].start, mt(100));
    assert_eq!(specs[1].len, mt(200));
}

#[test]
fn smf_unclosed_note_is_typed_validation_error() {
    let bytes = smf_file(
        0,
        480,
        &[mtrk(&[ev(0, &note_on(0, 60, 100)), ev(0, &eot())])],
    );
    let doc = parse_smf(&bytes).unwrap();
    // Parse succeeds (document is well-formed) but the projection is
    // underivable — unclosed notes are never silently auto-closed.
    let err = doc.paired_notes().unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "got {err:?}");

    let mut c = controller();
    let err = try_apply(&mut c, ProjectOp::ImportMidiSmf { document: doc }).unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "got {err:?}");
    assert!(c.live().midi_sources.is_empty());
    assert!(c.live().smf_documents.is_empty());
}

// ===========================================================================
// Project integration: import, persistence, immutability, detach
// ===========================================================================

#[test]
fn smf_import_creates_source_with_native_division_and_projection() {
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &format1_two_channels());

    let src = &c.live().midi_sources[&source];
    assert_eq!(src.division.ticks_per_quarter(), 480);
    assert_eq!(src.notes.len(), 2);

    // Provenance document is stored verbatim.
    assert_eq!(c.live().smf_documents[&source], doc);

    // Note projection carries no channel (document owns it) but keeps
    // native ticks: ch0 note [240,720) len 480, ch1 note [120,480)
    // len 360.
    let mut notes: Vec<_> = src.notes.values().collect();
    notes.sort_by_key(|n| (n.start.value(), n.key));
    assert_eq!(notes[0].key, 64);
    assert_eq!(notes[0].start, mt(120));
    assert_eq!(notes[0].len, mt(360));
    assert_eq!(notes[1].key, 60);
    assert_eq!(notes[1].start, mt(240));
    assert_eq!(notes[1].len, mt(480));

    // NoteIds were drawn deterministically from the projection order
    // (start, key, channel, velocity, len) — ch1 note (start 120)
    // precedes ch0 note (start 240).
    let ids: Vec<NoteId> = src.notes.keys().copied().collect();
    assert!(ids.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn smf_document_survives_save_load_and_schema_is_v10() {
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &format0_rich());

    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let bytes = store
        .read_snapshot(&store.head().unwrap().unwrap())
        .unwrap();
    let env: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(env["schema_version"], serde_json::json!(SCHEMA_VERSION));
    assert_eq!(SCHEMA_VERSION, 11);
    // The document is persisted inside the project snapshot.
    assert!(env["project"]["smf_documents"].is_object());

    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live().smf_documents[&source], doc);
    assert_eq!(
        c2.live().midi_sources[&source].notes.len(),
        c.live().midi_sources[&source].notes.len()
    );
}

#[test]
fn smf_document_deserialize_revalidates_domain() {
    // A crafted snapshot cannot smuggle an out-of-domain document:
    // SmfDocument's serde gate re-runs validate() on load.
    let doc = parse_smf(&format0_rich()).unwrap();
    let mut json = serde_json::to_value(&doc).unwrap();
    json["division"] = serde_json::json!(0);
    assert!(serde_json::from_value::<SmfDocument>(json).is_err());

    let mut json = serde_json::to_value(&doc).unwrap();
    json["tracks"][0]["events"][1]["tick"] = serde_json::json!(-5);
    assert!(serde_json::from_value::<SmfDocument>(json).is_err());
}

#[test]
fn smf_backed_source_rejects_all_note_edits() {
    let mut c = controller();
    let (_doc, source) = import_doc(&mut c, &format1_two_channels());
    let note_id = *c.live().midi_sources[&source].notes.keys().next().unwrap();

    for op in [
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: mt(0),
            len: mt(480),
        },
        ProjectOp::RemoveMidiNote {
            source_id: source,
            note_id,
        },
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id,
            velocity: 90,
        },
        ProjectOp::MoveMidiNote {
            source_id: source,
            note_id,
            start: mt(960),
        },
    ] {
        let err = try_apply(&mut c, op).unwrap_err();
        assert!(
            matches!(err, Error::Validation(_)),
            "expected validation rejection, got {err:?}"
        );
    }
    // Nothing changed.
    assert_eq!(c.live().midi_sources[&source].notes.len(), 2);
}

#[test]
fn smf_plain_midi_source_still_accepts_note_edits() {
    // The immutability guard is scoped to SMF-backed sources only.
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let source = *c.live().midi_sources.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: mt(0),
            len: mt(480),
        },
    );
    let note_id = *c.live().midi_sources[&source].notes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id,
            velocity: 90,
        },
    );
    assert_eq!(c.live().midi_sources[&source].notes[&note_id].velocity, 90);
}

#[test]
fn smf_detach_clip_source_creates_editable_copy_without_provenance() {
    let mut c = controller();
    let (_doc, source) = import_doc(&mut c, &format1_two_channels());

    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: musical_clip(0),
            playback: midi_window(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();

    apply(&mut c, ProjectOp::DetachClipSource { clip_id: clip });
    // The clip now reads a fresh source with the same notes but no
    // SMF provenance — and that copy accepts note edits.
    let detached = c.live().clips[&clip].source_id;
    assert_ne!(detached, source);
    assert!(!c.live().smf_documents.contains_key(&detached));
    assert!(c.live().smf_documents.contains_key(&source));
    assert_eq!(
        c.live().midi_sources[&detached].notes.len(),
        c.live().midi_sources[&source].notes.len()
    );
    let note_id = *c.live().midi_sources[&detached]
        .notes
        .keys()
        .next()
        .unwrap();
    apply(
        &mut c,
        ProjectOp::SetMidiNoteVelocity {
            source_id: detached,
            note_id,
            velocity: 90,
        },
    );
    assert_eq!(
        c.live().midi_sources[&detached].notes[&note_id].velocity,
        90
    );
}

#[test]
fn smf_remove_source_removes_provenance_document() {
    let mut c = controller();
    let (_doc, source) = import_doc(&mut c, &format1_two_channels());
    assert!(c.live().smf_documents.contains_key(&source));
    apply(&mut c, ProjectOp::RemoveMidiSource { source_id: source });
    assert!(!c.live().midi_sources.contains_key(&source));
    assert!(!c.live().smf_documents.contains_key(&source));
}

#[test]
fn smf_validate_references_rejects_divergence_and_orphans() {
    let mut c = controller();
    let (_doc, source) = import_doc(&mut c, &format1_two_channels());

    // (a) Document without a source → NotFound.
    let mut p = c.live().clone();
    p.midi_sources.remove(&source);
    let err = p.validate_references().unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "got {err:?}");

    // (b) Division disagreement → Validation.
    let mut p = c.live().clone();
    p.midi_sources.get_mut(&source).unwrap().division = MidiDivision::ppqn(960).unwrap();
    let err = p.validate_references().unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "got {err:?}");

    // (c) Note projection divergence → Validation. Channel is not
    // part of the note multiset, but key/velocity/start/len are.
    let mut p = c.live().clone();
    let note_id = *p.midi_sources[&source].notes.keys().next().unwrap();
    p.midi_sources
        .get_mut(&source)
        .unwrap()
        .notes
        .get_mut(&note_id)
        .unwrap()
        .velocity = 42;
    let err = p.validate_references().unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "got {err:?}");
}

#[test]
fn smf_v8_snapshot_without_documents_loads_with_empty_map() {
    // v8 → v9 is an identity migration: `smf_documents` is
    // `#[serde(default)]`, so a pre-v9 body loads with an empty map.
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let mut project_json = serde_json::to_value(c.live()).unwrap();
    project_json
        .as_object_mut()
        .unwrap()
        .remove("smf_documents");
    let env = serde_json::json!({
        "schema_version": 8,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": project_json,
    });
    let mut store = InMemoryStore::new();
    store
        .save("p", c.live_revision(), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert!(c2.live().smf_documents.is_empty());
    assert_eq!(c2.live().midi_sources.len(), 1);
}

// ===========================================================================
// Export: provenance path, synthesized path, conversion accounting
// ===========================================================================

fn options(format: SmfFormat, division: u32) -> SmfExportOptions {
    SmfExportOptions { format, division }
}

#[test]
fn smf_export_same_format_division_is_golden() {
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &format0_rich());
    let project = c.live().clone();

    let file = export_smf(&project, &source, &options(SmfFormat::Format0, 480)).unwrap();
    // Deterministic: a second export produces identical bytes.
    let file2 = export_smf(&project, &source, &options(SmfFormat::Format0, 480)).unwrap();
    assert_eq!(file.bytes, file2.bytes);

    // Same division → every event conversion exact.
    assert_eq!(file.report.exact_count, file.report.event_count);
    assert_eq!(file.report.rounded_count, 0);
    assert_eq!(file.report.max_error, SmfRational::ZERO);
    assert_eq!(file.report.unsupported_events, []);

    // Golden roundtrip: parse(export(parse(bytes))) == original doc.
    let reparsed = parse_smf(&file.bytes).unwrap();
    assert_eq!(reparsed, doc);
}

#[test]
fn smf_export_format1_preserves_track_structure() {
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &format1_two_channels());
    let project = c.live().clone();

    let file = export_smf(&project, &source, &options(SmfFormat::Format1, 480)).unwrap();
    let reparsed = parse_smf(&file.bytes).unwrap();
    assert_eq!(reparsed.format, SmfFormat::Format1);
    assert_eq!(reparsed.tracks.len(), 3);
    // Track contents are identical — same events, same order.
    assert_eq!(reparsed.tracks, doc.tracks);
}

#[test]
fn smf_export_format0_merges_tracks_deterministically() {
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &format1_two_channels());
    let project = c.live().clone();

    let file = export_smf(&project, &source, &options(SmfFormat::Format0, 480)).unwrap();
    let reparsed = parse_smf(&file.bytes).unwrap();
    assert_eq!(reparsed.format, SmfFormat::Format0);
    assert_eq!(reparsed.tracks.len(), 1);

    // The merged event multiset equals the union of source tracks
    // (EOT markers collapse to the single writer-appended one).
    let mut expected: Vec<SmfEvent> = doc
        .tracks
        .iter()
        .flat_map(|t| t.events.iter().cloned())
        .filter(|e| !matches!(e.kind, SmfEventKind::EndOfTrack))
        .collect();
    expected.sort_by_key(|e| (e.tick.value(), 0u8, 0u32));
    let mut actual: Vec<SmfEvent> = reparsed.tracks[0]
        .events
        .iter()
        .filter(|e| !matches!(e.kind, SmfEventKind::EndOfTrack))
        .cloned()
        .collect();
    // Compare as multisets — merged order is (tick, rank, track, seq).
    let key = |e: &SmfEvent| format!("{:?}", e.kind);
    expected.sort_by_key(|e| (e.tick.value(), key(e)));
    actual.sort_by_key(|e| (e.tick.value(), key(e)));
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected.iter()) {
        assert_eq!(a.tick, e.tick);
        assert_eq!(a.kind, e.kind);
    }
}

#[test]
fn smf_export_ppqn_up_conversion_is_exact() {
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &format1_two_channels());
    let project = c.live().clone();

    // 480 → 960 doubles every tick exactly.
    let file = export_smf(&project, &source, &options(SmfFormat::Format1, 960)).unwrap();
    assert_eq!(file.report.exact_count, file.report.event_count);
    assert_eq!(file.report.rounded_count, 0);
    assert_eq!(file.report.max_error, SmfRational::ZERO);

    let reparsed = parse_smf(&file.bytes).unwrap();
    assert_eq!(reparsed.division, 960);
    for (src_track, out_track) in doc.tracks.iter().zip(reparsed.tracks.iter()) {
        for (s, o) in src_track.events.iter().zip(out_track.events.iter()) {
            assert_eq!(o.tick.value(), s.tick.value() * 2);
        }
    }
}

#[test]
fn smf_export_ppqn_down_reports_rounding_and_max_error() {
    // Division 960 source with odd ticks → 480 target rounds half-up
    // and reports the worst error as an exact rational.
    let bytes = smf_file(
        0,
        960,
        &[mtrk(&[
            ev(0, &note_on(0, 60, 100)),
            ev(1, &note_off(0, 60, 64)), // tick 1 → 0.5 → 1, err 1/2
            ev(2, &note_on(0, 62, 90)),  // tick 3 → 1.5 → 2, err 1/2
            ev(1, &note_off(0, 62, 64)), // tick 4 → 2 exact
            ev(0, &eot()),
        ])],
    );
    let doc = parse_smf(&bytes).unwrap();
    assert_eq!(doc.paired_notes().unwrap().len(), 2);

    let mut project = Project::new(ProjectId::new(0), "ppqn");
    let sid = SourceId::new(7);
    project.midi_sources.insert(
        sid,
        MidiSource {
            id: sid,
            division: MidiDivision::ppqn(960).unwrap(),
            notes: BTreeMap::new(),
        },
    );
    project.smf_documents.insert(sid, doc);

    let file = export_smf(&project, &sid, &options(SmfFormat::Format0, 480)).unwrap();
    let r = &file.report;
    // 5 source events (note on/off ×2 + eot... eot is a source event).
    assert_eq!(r.event_count, 5);
    assert_eq!(r.exact_count, 3); // tick0, tick4, eot@4
    assert_eq!(r.rounded_count, 2); // ticks 1 and 3
    assert_eq!(r.max_error, SmfRational::new(1, 2));

    // Reimported ticks: 0→0, 1→1 (half-up), 3→2, 4→2.
    let reparsed = parse_smf(&file.bytes).unwrap();
    let ticks: Vec<i64> = reparsed.tracks[0]
        .events
        .iter()
        .filter(|e| !matches!(e.kind, SmfEventKind::EndOfTrack))
        .map(|e| e.tick.value())
        .collect();
    // Canonical order: note-offs before note-ons at equal ticks.
    assert_eq!(ticks, vec![0, 1, 2, 2]);
}

#[test]
fn smf_export_same_tick_noteoff_precedes_noteon() {
    // A retrigger: note off and note on for the same key at the same
    // tick must serialize off-first — the off closes the previous
    // sounding instance before the on retriggers.
    let bytes = smf_file(
        0,
        480,
        &[mtrk(&[
            ev(0, &note_on(0, 60, 100)),
            ev(480, &note_off(0, 60, 64)),
            ev(0, &note_on(0, 60, 90)), // retrigger at the same tick
            ev(480, &note_off(0, 60, 64)),
            ev(0, &eot()),
        ])],
    );
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &bytes);
    let project = c.live().clone();

    // Both notes pair cleanly: [0,480) and [480,960).
    assert_eq!(doc.paired_notes().unwrap().len(), 2);

    for format in [SmfFormat::Format0, SmfFormat::Format1] {
        let file = export_smf(&project, &source, &options(format, 480)).unwrap();
        let reparsed = parse_smf(&file.bytes).unwrap();
        let track = &reparsed.tracks[reparsed.tracks.len() - 1];
        let at_480: Vec<&SmfEventKind> = track
            .events
            .iter()
            .filter(|e| e.tick == mt(480))
            .map(|e| &e.kind)
            .collect();
        assert_eq!(at_480.len(), 2, "format {format:?}");
        assert!(
            matches!(at_480[0], SmfEventKind::NoteOff { .. }),
            "format {format:?}: off must precede on, got {at_480:?}"
        );
        assert!(
            matches!(at_480[1], SmfEventKind::NoteOn { .. }),
            "format {format:?}: expected on second, got {at_480:?}"
        );
    }
}

#[test]
fn smf_export_synthesizes_notes_only_source() {
    // A plain AddMidiSource (no provenance) exports minimal
    // channel-0 note events — no invented meta/controller data.
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let source = *c.live().midi_sources.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: mt(120),
            len: mt(240),
        },
    );
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 64,
            velocity: 80,
            start: mt(0),
            len: mt(96),
        },
    );
    let project = c.live().clone();

    let file = export_smf(&project, &source, &options(SmfFormat::Format0, 480)).unwrap();
    let reparsed = parse_smf(&file.bytes).unwrap();
    assert_eq!(reparsed.format, SmfFormat::Format0);
    assert_eq!(reparsed.division, 480);
    assert_eq!(reparsed.tracks.len(), 1);
    // 2 notes × (on+off) + eot = 5 events, all channel 0.
    let events = &reparsed.tracks[0].events;
    assert_eq!(events.len(), 5);
    for e in events.iter().take(4) {
        let ch = match &e.kind {
            SmfEventKind::NoteOn { channel, .. } | SmfEventKind::NoteOff { channel, .. } => {
                *channel
            }
            other => panic!("unexpected synthesized event {other:?}"),
        };
        assert_eq!(ch, 0);
    }
    // Note ordering at tick 0: note64 on; at 96: note64 off;
    // at 120: note60 on; at 360: note60 off.
    assert!(matches!(
        events[0].kind,
        SmfEventKind::NoteOn {
            key: 64,
            velocity: 80,
            ..
        }
    ));
    assert_eq!(events[0].tick, mt(0));
    assert!(matches!(
        events[1].kind,
        SmfEventKind::NoteOff { key: 64, .. }
    ));
    assert_eq!(events[1].tick, mt(96));
    assert!(matches!(
        events[2].kind,
        SmfEventKind::NoteOn {
            key: 60,
            velocity: 100,
            ..
        }
    ));
    assert_eq!(events[2].tick, mt(120));
    assert!(matches!(
        events[3].kind,
        SmfEventKind::NoteOff { key: 60, .. }
    ));
    assert_eq!(events[3].tick, mt(360));

    // Format 1 synthesis: empty conductor + one event track.
    let file1 = export_smf(&project, &source, &options(SmfFormat::Format1, 480)).unwrap();
    let reparsed1 = parse_smf(&file1.bytes).unwrap();
    assert_eq!(reparsed1.tracks.len(), 2);
    assert_eq!(reparsed1.tracks[0].events.len(), 1);
    assert!(matches!(
        reparsed1.tracks[0].events[0].kind,
        SmfEventKind::EndOfTrack
    ));
    assert_eq!(reparsed1.tracks[1].events.len(), 5);
}

#[test]
fn smf_export_rejects_invalid_targets() {
    let mut c = controller();
    let (_doc, source) = import_doc(&mut c, &format1_two_channels());
    let project = c.live().clone();

    // Unknown source.
    let err = export_smf(
        &project,
        &SourceId::new(999),
        &options(SmfFormat::Format0, 480),
    )
    .unwrap_err();
    assert!(matches!(err, SmfError::NotFound(_)), "got {err:?}");

    // Out-of-domain division.
    for bad in [0u32, 32_768, u32::MAX] {
        let err = export_smf(&project, &source, &options(SmfFormat::Format0, bad)).unwrap_err();
        assert!(
            matches!(err, SmfError::Unsupported { .. }),
            "division {bad}: got {err:?}"
        );
    }
}

#[test]
fn smf_export_overflow_is_typed() {
    // A document event at i64::MAX ticks cannot convert to a larger
    // division — checked arithmetic reports Overflow, not a panic.
    let doc = SmfDocument {
        format: SmfFormat::Format0,
        division: 480,
        tracks: vec![SmfTrack {
            events: vec![SmfEvent {
                tick: mt(i64::MAX),
                kind: SmfEventKind::NoteOff {
                    channel: 0,
                    key: 60,
                    velocity: 0,
                },
            }],
        }],
    };
    doc.validate().unwrap();

    let mut project = Project::new(ProjectId::new(0), "ovf");
    let sid = SourceId::new(3);
    project.midi_sources.insert(
        sid,
        MidiSource {
            id: sid,
            division: MidiDivision::ppqn(480).unwrap(),
            notes: BTreeMap::new(),
        },
    );
    project.smf_documents.insert(sid, doc);

    let err = export_smf(&project, &sid, &options(SmfFormat::Format0, 960)).unwrap_err();
    assert!(matches!(err, SmfError::Overflow { .. }), "got {err:?}");
}

// ===========================================================================
// Typed parse errors
// ===========================================================================

#[test]
#[allow(clippy::too_many_lines)] // table-driven malformed-input coverage
fn smf_malformed_inputs_are_typed() {
    // Not an SMF at all.
    let err = parse_smf(b"NOTMIDI!").unwrap_err();
    assert!(matches!(err, SmfError::Malformed { .. }), "got {err:?}");

    // Empty / truncated header.
    assert!(matches!(
        parse_smf(b"").unwrap_err(),
        SmfError::Malformed { .. }
    ));
    assert!(matches!(
        parse_smf(b"MThd\x00\x00\x00\x06\x00").unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // Header length < 6.
    let mut bad = b"MThd".to_vec();
    bad.extend_from_slice(&5u32.to_be_bytes());
    bad.extend_from_slice(&[0; 5]);
    assert!(matches!(
        parse_smf(&bad).unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // Declared track count mismatch (0 MTrk chunks, ntracks=1).
    let bytes = smf_file(0, 480, &[]);
    // smf_file with no tracks writes ntracks=0 → "no tracks" malformed.
    assert!(matches!(
        parse_smf(&bytes).unwrap_err(),
        SmfError::Malformed { .. }
    ));
    // Hand-craft ntracks=2 with one MTrk.
    let mut two = b"MThd".to_vec();
    two.extend_from_slice(&6u32.to_be_bytes());
    two.extend_from_slice(&0u16.to_be_bytes());
    two.extend_from_slice(&2u16.to_be_bytes());
    two.extend_from_slice(&480u16.to_be_bytes());
    two.extend_from_slice(&mtrk(&[ev(0, &eot())]));
    assert!(matches!(
        parse_smf(&two).unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // Track chunk length overruns the file.
    let mut over = b"MThd".to_vec();
    over.extend_from_slice(&6u32.to_be_bytes());
    over.extend_from_slice(&0u16.to_be_bytes());
    over.extend_from_slice(&1u16.to_be_bytes());
    over.extend_from_slice(&480u16.to_be_bytes());
    over.extend_from_slice(b"MTrk");
    over.extend_from_slice(&999u32.to_be_bytes());
    over.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
    assert!(matches!(
        parse_smf(&over).unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // Trailing bytes that are not a full chunk.
    let mut trail = smf_file(0, 480, &[mtrk(&[ev(0, &eot())])]);
    trail.extend_from_slice(&[0xAA]);
    assert!(matches!(
        parse_smf(&trail).unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // Delta varlen cut off mid-quantity.
    let cut = smf_file(
        0,
        480,
        &[{
            let mut body = vec![0x80]; // continuation bit, then EOF
            body.extend_from_slice(b"MTrk"); // will be overwritten below
            let mut t = b"MTrk".to_vec();
            t.extend_from_slice(&4u32.to_be_bytes());
            t.extend_from_slice(&body[..1]);
            t
        }],
    );
    assert!(matches!(
        parse_smf(&cut).unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // Data byte without running status.
    let no_status = smf_file(
        0,
        480,
        &[mtrk(&[{
            let mut v = vec![0x00, 0x3C, 0x64]; // delta 0 + two data bytes
            v.extend_from_slice(&eot()[1..]); // wrong — build properly below
            v
        }])],
    );
    let _ = no_status;
    let body = vec![0x00u8, 0x3C, 0x64, 0x00, 0xFF, 0x2F, 0x00];
    let mut t = b"MTrk".to_vec();
    t.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
    t.extend_from_slice(&body);
    let bytes = smf_file(0, 480, &[t]);
    assert!(matches!(
        parse_smf(&bytes).unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // System status byte inside track data.
    let mut body = vec![0x00, 0xF1, 0x00];
    body.extend_from_slice(&eot());
    let mut t = b"MTrk".to_vec();
    t.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
    t.extend_from_slice(&body);
    let bytes = smf_file(0, 480, &[t]);
    assert!(matches!(
        parse_smf(&bytes).unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // Delta with no following event.
    let mut t = b"MTrk".to_vec();
    t.extend_from_slice(&1u32.to_be_bytes());
    t.push(0x05);
    let bytes = smf_file(0, 480, &[t]);
    assert!(matches!(
        parse_smf(&bytes).unwrap_err(),
        SmfError::Malformed { .. }
    ));

    // Division zero.
    let bytes = smf_file(0, 0, &[mtrk(&[ev(0, &eot())])]);
    assert!(matches!(
        parse_smf(&bytes).unwrap_err(),
        SmfError::Malformed { .. }
    ));
}

#[test]
fn smf_unsupported_inputs_are_typed_never_success() {
    // Format 2.
    let bytes = smf_file(2, 480, &[mtrk(&[ev(0, &eot())])]);
    let err = parse_smf(&bytes).unwrap_err();
    assert!(matches!(err, SmfError::Unsupported { .. }), "got {err:?}");

    // Format 3 (undefined).
    let bytes = smf_file(3, 480, &[mtrk(&[ev(0, &eot())])]);
    let err = parse_smf(&bytes).unwrap_err();
    assert!(matches!(err, SmfError::Unsupported { .. }), "got {err:?}");

    // SMPTE division: high bit set (e.g. 0xE728 = 25 fps).
    let bytes = smf_file(0, 0xE728, &[mtrk(&[ev(0, &eot())])]);
    let err = parse_smf(&bytes).unwrap_err();
    assert!(matches!(err, SmfError::Unsupported { .. }), "got {err:?}");
}

#[test]
fn smf_tick_accumulation_overflow_is_typed() {
    // ~130 maximal 8-byte VLQ deltas exceed i64::MAX tick accumulation.
    let max_delta = vlq((1u64 << 56) - 1);
    let mut body = Vec::new();
    for _ in 0..130 {
        body.extend_from_slice(&max_delta);
        body.extend_from_slice(&note_on(0, 60, 100));
    }
    body.extend_from_slice(&eot());
    let mut t = b"MTrk".to_vec();
    t.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
    t.extend_from_slice(&body);
    let bytes = smf_file(0, 480, &[t]);
    let err = parse_smf(&bytes).unwrap_err();
    assert!(matches!(err, SmfError::Overflow { .. }), "got {err:?}");
}

// ===========================================================================
// Controller projection → channel-aware chase → no hanging notes
// ===========================================================================

/// Every `NoteOn` in the plan is matched by a `NoteOff` for the same
/// `(source, note)` and the plan terminates in `AllNotesOff` — the
/// "no hanging notes / 発音残り 0" contract.
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
    assert!(
        matches!(
            plan.events.last().map(|e| &e.action),
            Some(MidiPlanAction::AllNotesOff)
        ),
        "plan must terminate in AllNotesOff"
    );
}

#[test]
fn smf_controller_events_feed_channel_aware_chase() {
    // Purpose-built Format 0: two channels, CC64 sustain state before
    // and inside the phrase window, notes on both channels.
    let bytes = smf_file(
        0,
        480,
        &[mtrk(&[
            ev(0, &cc(0, 64, 127)),
            ev(0, &cc(1, 64, 64)),
            ev(120, &note_on(1, 64, 90)),
            ev(120, &cc(0, 64, 64)), // mid-phrase sustain change, ch0
            ev(120, &note_off(1, 64, 40)),
            ev(120, &note_on(0, 60, 100)),
            ev(480, &note_off(0, 60, 64)),
            ev(0, &eot()),
        ])],
    );
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &bytes);

    // The provenance projection carries channel identity into the
    // lane: (tick, channel, kind, value) in deterministic order.
    let lane_events = doc.controller_events();
    assert_eq!(lane_events.len(), 3);
    assert_eq!(lane_events[0].channel, 0);
    assert_eq!(lane_events[0].kind, MidiControllerKind::Cc(64));
    assert_eq!(lane_events[0].value, 127);
    assert_eq!(lane_events[0].tick, mt(0));
    assert_eq!(lane_events[1].channel, 1);
    assert_eq!(lane_events[1].value, 64);
    assert_eq!(lane_events[2].channel, 0);
    assert_eq!(lane_events[2].value, 64);
    assert_eq!(lane_events[2].tick, mt(240));

    // Phrase [0,960) source ticks → project [0,1920) at PPQN 960
    // (480 division → exact ×2 mapping, no requantization).
    let selection = MidiCompSelection {
        regions: vec![MidiPhraseRegion {
            id: MidiCompRegionId::new(201),
            source_id: source,
            phrase: MidiRange::try_new(mt(0), mt(960)).unwrap(),
            project_range: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(1_920))
                .unwrap(),
        }],
    };
    let lanes = MidiControllerLanes {
        lanes: vec![MidiControllerLane {
            source_id: source,
            events: lane_events,
        }],
    };
    let plan = compile_midi_comp(c.live(), &selection, &lanes).unwrap();

    // Channel-aware chase: ch0 sustain 127 and ch1 sustain 64 are
    // distinct states — the snapshot carries both, keyed by channel.
    let chase = &plan.regions[0].chase;
    assert_eq!(
        chase,
        &vec![
            ((0, MidiControllerKind::Cc(64)), 127),
            ((1, MidiControllerKind::Cc(64)), 64),
        ]
    );
    // Both chases emit at the region start (project tick 0) with
    // their channel preserved.
    for (channel, value) in [(0u8, 127i32), (1u8, 64i32)] {
        assert!(plan.events.iter().any(|e| {
            e.at == MusicalTick::new(0)
                && matches!(
                    e.action,
                    MidiPlanAction::ControllerChase {
                        channel: ch,
                        controller: MidiControllerKind::Cc(64),
                        value: v,
                    } if ch == channel && v == value
                )
        }));
    }
    // The mid-phrase ch0 CC64=64 (@240 source → project 480) plays
    // live at its mapped tick.
    assert!(plan.events.iter().any(|e| {
        e.at == MusicalTick::new(480)
            && matches!(
                e.action,
                MidiPlanAction::ControllerChase {
                    channel: 0,
                    controller: MidiControllerKind::Cc(64),
                    value: 64,
                }
            )
    }));

    // Both notes adopted; every note closes; plan ends in panic.
    assert_eq!(plan.regions[0].notes.len(), 2);
    assert_eq!(plan.regions[0].virtual_offs, []);
    assert_no_hanging_notes(&plan);
}

#[test]
fn smf_phrase_boundary_virtual_off_and_panic() {
    // A note whose natural end crosses the phrase boundary is cut by
    // a virtual off — no hanging note at the region edge.
    let bytes = smf_file(
        0,
        480,
        &[mtrk(&[
            ev(0, &note_on(0, 60, 100)),
            ev(0, &cc(0, 64, 127)),
            ev(1_440, &note_off(0, 60, 64)), // end @1440 > phrase end 960
            ev(0, &eot()),
        ])],
    );
    let mut c = controller();
    let (doc, source) = import_doc(&mut c, &bytes);
    let lanes = MidiControllerLanes {
        lanes: vec![MidiControllerLane {
            source_id: source,
            events: doc.controller_events(),
        }],
    };
    let selection = MidiCompSelection {
        regions: vec![MidiPhraseRegion {
            id: MidiCompRegionId::new(202),
            source_id: source,
            phrase: MidiRange::try_new(mt(0), mt(960)).unwrap(),
            project_range: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(1_920))
                .unwrap(),
        }],
    };
    let plan = compile_midi_comp(c.live(), &selection, &lanes).unwrap();

    assert_eq!(plan.regions[0].notes.len(), 1);
    assert_eq!(plan.regions[0].virtual_offs.len(), 1);
    // The virtual off lands exactly at the region end (project 1920),
    // not at the note's natural end.
    let note = plan.regions[0].notes[0];
    let off = plan
        .events
        .iter()
        .find(|e| e.note == Some(note) && matches!(e.action, MidiPlanAction::NoteOff { .. }))
        .unwrap();
    assert_eq!(off.at, MusicalTick::new(1_920));
    assert_eq!(off.source_tick, Some(mt(1_440)));
    assert_no_hanging_notes(&plan);
}
