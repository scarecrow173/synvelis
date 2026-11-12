//! TIM-02 × TIM-01 cross-golden fixture (issues #126 / #125) —
//! `SetProjectPpqn` over an SMF-backed source, end to end.
//!
//! Headless contract fixture only: a Format 1 division-480 document
//! is imported into a default-PPQN-960 project (the "PPQN 異種"
//! heterogeneity the TIM-02 acceptance line requires), a clip is
//! anchored to the imported source, `SetProjectPpqn` runs under
//! both `Exact` and `Round` policies, the project is saved/loaded,
//! and `export_smf` at division 480 must produce byte-identical
//! golden output whose re-imported document equals the original —
//! proving `ppqn.rs`'s source-native skip invariant holds across
//! the whole chain. No real device/plugin/OS/GUI/RT behavior is
//! exercised (those stay `NotRun`).

use ruviform_media_core::Rounding;
use synvelis_project::*;

// ---------- SMF byte builders (same wire vocabulary as tim_02_smf) ----------

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

// ---------- fixture ----------

/// Format 1, division 480 — deliberately *not* the project PPQN
/// (960). The conductor track carries two tempo events (a mid-phrase
/// change at tick 960) and the event tracks carry channel-aware
/// CC64 sustain state plus closed notes, so tempo/CC64 provenance
/// is observable on both sides of the PPQN chain.
fn format1_division480() -> Vec<u8> {
    smf_file(
        1,
        480,
        &[
            mtrk(&[
                ev(0, &tempo(500_000)),
                ev(0, &timesig(4, 2, 24, 8)),
                ev(0, &trackname("Conductor")),
                ev(960, &tempo(400_000)),
                ev(0, &eot()),
            ]),
            mtrk(&[
                ev(0, &cc(0, 64, 127)),
                ev(240, &note_on(0, 60, 100)),
                ev(240, &cc(0, 64, 64)),
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

// ---------- project helpers ----------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "tim-02-ppqn-smf"),
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

fn try_apply(c: &mut ProjectController, op: ProjectOp) -> Result<(), Error> {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .map(|_| ())
}

/// Asserts rejection and that neither the live revision nor the
/// live project content moved.
fn rejected(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    let live = c.live().clone();
    assert!(try_apply(c, op).is_err(), "op must be rejected");
    assert_eq!(
        c.live_revision(),
        rev,
        "rejected op must not advance revision"
    );
    assert_eq!(c.live(), &live, "rejected op must not mutate live state");
}

fn ppqn_op(old: u32, new: u32, policy: PpqnConversionPolicy) -> ProjectOp {
    ProjectOp::SetProjectPpqn {
        old_ppqn: old,
        new_ppqn: new,
        policy,
    }
}

fn import_smf(c: &mut ProjectController, bytes: &[u8]) -> (SmfDocument, SourceId) {
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

/// Musical clip placement: `anchor_tick`/`extent_ticks` on the
/// *project* PPQN grid (960 by default) — the only values
/// `SetProjectPpqn` may re-project in this fixture.
fn musical_clip(anchor_tick: i64, extent_ticks: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(extent_ticks).unwrap()),
    }
}

fn midi_window() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(4 * 960).unwrap())
            .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

/// Adds one track/lane and a musical clip reading `source`.
/// Returns the clip id.
fn add_clip(c: &mut ProjectController, source: SourceId, anchor_tick: i64) -> ClipId {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: musical_clip(anchor_tick, 1_920),
            playback: midi_window(),
        },
    );
    *c.live().clips.keys().next().unwrap()
}

/// The export golden: Format 1 at division 480 — the source's own
/// division, so every tick conversion is exact.
fn export_golden(project: &Project, source: &SourceId) -> SmfFile {
    export_smf(
        project,
        source,
        &SmfExportOptions {
            format: SmfFormat::Format1,
            division: 480,
        },
    )
    .unwrap()
}

/// Note-projection content as `(key, velocity, start, len)` tuples —
/// `NoteId`s are controller-drawn and not part of the contract.
fn note_tuples(source: &MidiSource) -> Vec<(u8, u8, i64, i64)> {
    let mut notes: Vec<_> = source
        .notes
        .values()
        .map(|n| (n.key, n.velocity, n.start.value(), n.len.value()))
        .collect();
    notes.sort_unstable();
    notes
}

/// Counts document events matching `pred` across all tracks.
fn count_events(doc: &SmfDocument, pred: impl Fn(&SmfEventKind) -> bool) -> usize {
    doc.tracks
        .iter()
        .flat_map(|t| t.events.iter())
        .filter(|e| pred(&e.kind))
        .count()
}

/// Tempo/meta and CC64 provenance must be present in the fixture
/// document — asserted on both the original and every re-import.
fn assert_provenance_present(doc: &SmfDocument) {
    assert_eq!(
        count_events(doc, |k| matches!(k, SmfEventKind::Tempo { .. })),
        2,
        "two conductor tempo events"
    );
    assert_eq!(
        count_events(doc, |k| matches!(
            k,
            SmfEventKind::Cc { controller: 64, .. }
        )),
        3,
        "CC64 sustain on ch0 (x2) and ch1 (x1)"
    );
}

/// The tail of the chain shared by every scenario: export bytes
/// reparse to the original document (full event/tempo/CC64
/// equality) and re-importing them stores an identical document
/// with an identical note projection.
fn assert_export_reimport_golden(
    file: &SmfFile,
    doc: &SmfDocument,
    note_projection: &[(u8, u8, i64, i64)],
) {
    assert_eq!(
        file.report.exact_count, file.report.event_count,
        "division-480 export must be all-exact"
    );
    assert_eq!(file.report.rounded_count, 0);
    assert_eq!(file.report.max_error, SmfRational::ZERO);
    assert_eq!(file.report.unsupported_events, []);

    let reparsed = parse_smf(&file.bytes).unwrap();
    assert_eq!(&reparsed, doc, "export→parse must reproduce the document");
    assert_provenance_present(&reparsed);

    let mut c = controller();
    apply(&mut c, ProjectOp::ImportMidiSmf { document: reparsed });
    let source = *c.live().smf_documents.keys().next().unwrap();
    assert_eq!(&c.live().smf_documents[&source], doc);
    assert_eq!(
        note_tuples(&c.live().midi_sources[&source]),
        note_projection,
        "re-imported note projection must match"
    );
}

// ===========================================================================
// The full chain under Exact
// ===========================================================================

/// TIM-02 acceptance line "PPQN 異種の Import / Project Change /
/// Save / Export / 再 Import": import@480 into a PPQN-960 project,
/// clip anchor, `SetProjectPpqn` Exact 960→480, save/load, export@480
/// byte-golden, re-import doc equality — with tempo/CC64 provenance
/// intact at every step.
#[test]
fn ppqn_exact_change_preserves_smf_chain_end_to_end() {
    let mut c = controller();
    assert_eq!(c.live().ppqn, ProjectPpqn(960), "default project PPQN");

    // --- Import @480 (source-native division ≠ project PPQN) ---
    let (doc, source) = import_smf(&mut c, &format1_division480());
    assert_eq!(
        c.live().midi_sources[&source].division.ticks_per_quarter(),
        480
    );
    assert_provenance_present(&doc);

    // --- Clip anchor on the SMF-backed source ---
    // Anchor 3840 @960 = 4 beats, extent 1920 = 2 beats — whole-beat
    // positions exact on every PPQN grid.
    let clip = add_clip(&mut c, source, 3_840);

    // Provenance snapshot before the PPQN change.
    let smf_before = c.live().smf_documents[&source].clone();
    let midi_before = c.live().midi_sources[&source].clone();
    let clip_before = c.live().clips[&clip].clone();
    let golden_before = export_golden(c.live(), &source);
    let projection_before = note_tuples(&midi_before);

    // --- Project Change: Exact 960 → 480 ---
    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    let p = c.live();
    assert_eq!(p.ppqn, ProjectPpqn(480));
    assert_eq!(p.tempo_map.ppqn, 480);

    // The clip's musical placement re-projects exactly (halved);
    // its source playback window is media-domain and untouched.
    let clip_after = p.clips[&clip].clone();
    assert_eq!(
        clip_after.placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(1_920))
    );
    assert_eq!(
        clip_after.placement.extent,
        ClipExtent::Musical(MusicalDuration::new(960).unwrap())
    );
    assert_eq!(clip_after.playback, clip_before.playback);

    // The core invariant: source-native SMF data is never
    // re-projected — document, division, and note ticks identical.
    assert_eq!(&p.smf_documents[&source], &smf_before);
    assert_eq!(&p.midi_sources[&source], &midi_before);

    // Export is a pure function of the source-native slice: the
    // division-480 golden is byte-identical across the PPQN change.
    let golden_after = export_golden(p, &source);
    assert_eq!(golden_after.bytes, golden_before.bytes);

    // --- Save / Load ---
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live().ppqn, ProjectPpqn(480));
    assert_eq!(c2.live().smf_documents[&source], doc);
    assert_eq!(c2.live().midi_sources[&source], midi_before);
    assert_eq!(c2.live().clips[&clip], clip_after);

    // --- Export @480 byte-golden + re-import doc equality ---
    let file = export_golden(c2.live(), &source);
    assert_eq!(file.bytes, golden_before.bytes, "golden survives save/load");
    assert_export_reimport_golden(&file, &doc, &projection_before);
}

// ===========================================================================
// The full chain under Round (declared, possibly lossy)
// ===========================================================================

/// `SetProjectPpqn` under `Round` may quantize a musical clip
/// anchor — but the SMF-backed source is outside the re-projection
/// walk by construction, so provenance, export bytes, and the
/// re-imported document are unaffected. `Exact` on the same
/// inexact anchor rejects atomically first.
#[test]
fn ppqn_round_change_still_preserves_smf_source_native_data() {
    let mut c = controller();
    let (doc, source) = import_smf(&mut c, &format1_division480());

    // Anchor tick 1 @960 = 1/960 beat — inexact on the 1000 grid
    // (1000/960 = 25/24 ticks). Extent stays whole-beat.
    let clip = add_clip(&mut c, source, 1);

    let smf_before = c.live().smf_documents[&source].clone();
    let midi_before = c.live().midi_sources[&source].clone();
    let golden_before = export_golden(c.live(), &source);
    let projection_before = note_tuples(&midi_before);

    // Exact cannot represent the anchor — atomic rejection.
    rejected(&mut c, ppqn_op(960, 1_000, PpqnConversionPolicy::Exact));
    assert_eq!(c.live().ppqn, ProjectPpqn(960));

    // Round(Floor) applies the declared rounding: 25/24 → 1 tick.
    apply(
        &mut c,
        ppqn_op(960, 1_000, PpqnConversionPolicy::Round(Rounding::Floor)),
    );
    assert_eq!(c.live().ppqn, ProjectPpqn(1_000));
    let clip_after = &c.live().clips[&clip];
    assert_eq!(
        clip_after.placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(1))
    );

    // Lossy project-side conversion still never touches the
    // source-native slice — and export@480 ignores project PPQN
    // entirely (960 → 1000 changed nothing on the wire).
    let p = c.live();
    assert_eq!(&p.smf_documents[&source], &smf_before);
    assert_eq!(&p.midi_sources[&source], &midi_before);
    let golden_after = export_golden(p, &source);
    assert_eq!(golden_after.bytes, golden_before.bytes);

    // Save/load keeps the whole chain intact.
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live().ppqn, ProjectPpqn(1_000));
    assert_eq!(c2.live().smf_documents[&source], doc);

    let file = export_golden(c2.live(), &source);
    assert_eq!(file.bytes, golden_before.bytes);
    assert_export_reimport_golden(&file, &doc, &projection_before);
}
