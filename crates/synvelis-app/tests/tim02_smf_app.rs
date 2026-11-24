//! TIM-02 app-layer SMF integration (synvelis#126 app slice) —
//! headless `syn.*` op evidence for `syn.project.import_smf`,
//! `syn.project.export_smf`, the `midi` section of
//! `syn.app.capabilities`, and the clip→MIDI plan/compile evidence
//! the session engine adapter records.
//!
//! Every case drives the same `syn.*` string dispatch the product
//! host uses: `UiRuntime::dispatch_op` → `Service::call` →
//! `AppService` → `SynvelisApp`. Fixture mutation (project ops) goes
//! through `AppService::app_mut()` on the same composed app — there
//! is no second project model and no test-only op surface.
//!
//! Boundary: this is headless evidence only. Instrument-node
//! delivery of compiled MIDI events is wired through the routing
//! seam (synvelis#623 — `tim02_midi_delivery` covers the staged
//! path); this fixture binds no instrument, so every clip honestly
//! records `Unresolved`. Real OS MIDI backends, GUI, and external
//! oracles stay `NotRun` — fake/mock fixtures here never claim
//! those passes.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde_json::{Value, json};

use ruviform_media_reader::PrefetchConfig;
use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::engine::MidiClipDelivery;
use synvelis_app::{AppService, SessionEngineConfig, SessionRoutingEngine, SynvelisApp};
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipPlacement, ClipPlayback, MusicalDuration, MusicalTick,
    PlaybackMapping, ProjectOp, SmfFormat, SourceFrame, SourceRange, TimeAnchor, parse_smf,
};

// ---------- shared headless harness (same pattern as tim01) ----------

/// Shares one [`AppService`] between the headless [`UiRuntime`]
/// (real `syn.*` string dispatch) and the test (fixture mutation via
/// `app_mut`).
struct SharedService(Rc<RefCell<AppService>>);

impl Service for SharedService {
    fn ops(&self) -> &'static [OpInfo] {
        self.0.borrow().ops()
    }

    fn call(
        &mut self,
        op: &str,
        params: &Value,
        ctx: &mut ServiceContext<'_>,
    ) -> Result<CallOutcome, ErrorBody> {
        self.0.borrow_mut().call(op, params, ctx)
    }
}

struct Fixture {
    rt: UiRuntime,
    svc: Rc<RefCell<AppService>>,
    engine: SessionRoutingEngine,
    root: PathBuf,
}

fn session_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: synvelis_player::lowering::EngineConfig {
            sample_rate: 48_000,
            channels: 2,
            block_frames: 256,
        },
        prefetch: PrefetchConfig {
            block_frames: 256,
            block_count: 8,
            channels: 2,
            sample_rate: 48_000,
        },
        budget: BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        mode: BuildMode::Realtime,
        runtime_instance: synvelis_project::RuntimeInstanceId(1),
        device_clock_epoch: synvelis_project::RuntimeClockEpochRef(1),
        engine_config_generation: 1,
    }
}

fn fixture(name: &str) -> Fixture {
    let root = tmp_dir(name);
    let mut app = SynvelisApp::new(BackendReport::available("webview2", "test-1.0".into()));
    let engine = SessionRoutingEngine::new(session_config(root.join("media")));
    app.attach_routing_engine(engine.clone());
    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(svc.clone()))
        .expect("register AppService");
    Fixture {
        rt,
        svc,
        engine,
        root,
    }
}

fn call(rt: &mut UiRuntime, op: &str, params: Value) -> (bool, Option<Value>, Option<ErrorBody>) {
    let events = rt.dispatch_op("test-1", op, params);
    let result = events.iter().find_map(|e| match e {
        Event::Result {
            ok, data, error, ..
        } => Some((*ok, data.clone(), error.clone())),
        _ => None,
    });
    result.expect("every intent yields exactly one result event")
}

fn ok(rt: &mut UiRuntime, op: &str, params: Value) -> Value {
    let (ok, data, error) = call(rt, op, params);
    assert!(ok, "{op} failed: {error:?}");
    data.expect("successful result carries data")
}

fn fail(rt: &mut UiRuntime, op: &str, params: Value) -> Value {
    let (ok, _, error) = call(rt, op, params);
    assert!(!ok, "{op} unexpectedly succeeded");
    serde_json::to_value(error.expect("failure carries an error body"))
        .expect("error body serializes")
}

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("synvelis-tim02app-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn apply_project(f: &Fixture, op: ProjectOp) {
    let mut svc = f.svc.borrow_mut();
    let session = svc.app_mut().workspace_mut().session_mut();
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op applies");
}

fn live_project(f: &Fixture) -> synvelis_project::Project {
    f.svc.borrow().app().workspace().session().project().clone()
}

// ---------- SMF wire bytes (deterministic, division == project PPQN) ----------

fn vlq(mut n: u64) -> Vec<u8> {
    let mut out = vec![(n & 0x7F) as u8];
    n >>= 7;
    while n > 0 {
        out.push(((n & 0x7F) as u8) | 0x80);
        n >>= 7;
    }
    out.reverse();
    out
}

fn ev(delta: u64, bytes: &[u8]) -> Vec<u8> {
    let mut v = vlq(delta);
    v.extend_from_slice(bytes);
    v
}

fn note_on(ch: u8, key: u8, vel: u8) -> Vec<u8> {
    vec![0x90 | ch, key, vel]
}

fn note_off(ch: u8, key: u8, vel: u8) -> Vec<u8> {
    vec![0x80 | ch, key, vel]
}

fn cc(ch: u8, num: u8, val: u8) -> Vec<u8> {
    vec![0xB0 | ch, num, val]
}

fn tempo(us_per_quarter: u32) -> Vec<u8> {
    let mut v = vec![0xFF, 0x51, 0x03];
    v.extend_from_slice(&us_per_quarter.to_be_bytes()[1..]);
    v
}

fn eot() -> Vec<u8> {
    vec![0xFF, 0x2F, 0x00]
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

/// Format 0, division 960 (the default project PPQN): two channels'
/// notes plus a CC64 so the export report and the clip→MIDI compile
/// both exercise controller chase.
fn format0_notes() -> Vec<u8> {
    smf_file(
        0,
        960,
        &[mtrk(&[
            ev(0, &tempo(500_000)),
            ev(0, &cc(0, 64, 127)),
            ev(0, &note_on(0, 60, 100)),
            ev(480, &note_off(0, 60, 64)),
            ev(0, &note_on(1, 64, 90)),
            ev(480, &note_off(1, 64, 40)),
            ev(960, &eot()),
        ])],
    )
}

fn write_smf(root: &Path, name: &str, bytes: &[u8]) -> String {
    let path = root.join(name);
    fs::write(&path, bytes).unwrap();
    path.to_string_lossy().to_string()
}

fn import(f: &mut Fixture, path: &str) -> String {
    let res = ok(&mut f.rt, "syn.project.import_smf", json!({ "path": path }));
    res["source_id"].as_str().unwrap().to_string()
}

/// Track + lane holding one MIDI clip on `source` — musical extent
/// and source window both 1920 ticks so the natural 1:1 comp length
/// contract holds exactly (division == project PPQN).
fn add_midi_clip(f: &Fixture, source: &str) {
    apply_project(f, ProjectOp::AddTrack { name: "t".into() });
    let track = *live_project(f).tracks.keys().next_back().unwrap();
    apply_project(f, ProjectOp::AddLane { track_id: track });
    let lane = *live_project(f).tracks[&track].lane_ids.last().unwrap();
    let source_id = synvelis_project::SourceId::from_tagged(source).unwrap();
    apply_project(
        f,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(1_920).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_920).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
}

// ===========================================================================
// syn.project.import_smf
// ===========================================================================

/// Import mints one MIDI source through `ProjectOp::ImportMidiSmf`
/// and the reply carries the source id plus the import summary.
#[test]
fn import_smf_mints_source_and_reports_summary() {
    let mut f = fixture("import");
    let path = write_smf(&f.root, "in.mid", &format0_notes());

    let res = ok(&mut f.rt, "syn.project.import_smf", json!({ "path": path }));
    let source_id = res["source_id"].as_str().unwrap();
    assert!(source_id.starts_with("source-"), "{res}");
    assert_eq!(res["import"]["format"], "format0");
    assert_eq!(res["import"]["division"], 960);
    assert_eq!(res["import"]["tracks"], 1);
    assert_eq!(res["import"]["controllers"], 1);
    assert_eq!(res["notes"], 2);

    let project = live_project(&f);
    assert_eq!(project.midi_sources.len(), 1);
    assert_eq!(project.smf_documents.len(), 1);
}

/// Missing file → `service_error` with the io kind in `details`;
/// malformed bytes → `invalid_params` with `smf_error.kind` —
/// callers branch on structure, never message text.
#[test]
fn import_smf_typed_errors() {
    let mut f = fixture("import-err");
    let missing = f.root.join("nope.mid").to_string_lossy().to_string();
    let err = fail(
        &mut f.rt,
        "syn.project.import_smf",
        json!({ "path": missing }),
    );
    assert_eq!(err["code"], "service_error");
    assert_eq!(err["details"]["io_error"]["action"], "read");
    assert_eq!(err["details"]["io_error"]["kind"], "NotFound");

    let bad = write_smf(&f.root, "bad.mid", b"not a midi file");
    let err = fail(&mut f.rt, "syn.project.import_smf", json!({ "path": bad }));
    assert_eq!(err["code"], "invalid_params");
    assert_eq!(err["details"]["smf_error"]["kind"], "malformed");
    assert!(err["details"]["smf_error"]["offset"].is_number());

    let err = fail(&mut f.rt, "syn.project.import_smf", json!({}));
    assert_eq!(err["code"], "invalid_params");
    assert!(live_project(&f).midi_sources.is_empty());
}

/// The shared mutation gate: import rejects while transport is
/// playing and succeeds after stop.
#[test]
fn import_smf_rejects_while_transport_playing() {
    let mut f = fixture("import-gate");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    ok(&mut f.rt, "syn.transport.set", json!({ "playing": true }));

    let err = fail(&mut f.rt, "syn.project.import_smf", json!({ "path": path }));
    assert_eq!(err["code"], "invalid_params");
    assert!(live_project(&f).midi_sources.is_empty());

    ok(&mut f.rt, "syn.transport.set", json!({ "playing": false }));
    let res = ok(&mut f.rt, "syn.project.import_smf", json!({ "path": path }));
    assert!(res["source_id"].is_string());
}

/// `expected_revision` is a controller CAS — a stale value rejects
/// before any mutation.
#[test]
fn import_smf_rejects_stale_expected_revision() {
    let mut f = fixture("import-cas");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    let stale = u64::MAX;
    let err = fail(
        &mut f.rt,
        "syn.project.import_smf",
        json!({ "path": path, "expected_revision": stale }),
    );
    assert_eq!(err["code"], "invalid_params");
    assert!(live_project(&f).midi_sources.is_empty());
}

// ===========================================================================
// syn.project.export_smf
// ===========================================================================

/// Export writes the SMF bytes and returns the full conversion
/// report — nothing silently dropped.
#[test]
fn export_smf_writes_file_and_reports_conversion() {
    let mut f = fixture("export");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    let source_id = import(&mut f, &path);
    let out = f.root.join("out.mid").to_string_lossy().to_string();

    let res = ok(
        &mut f.rt,
        "syn.project.export_smf",
        json!({ "source_id": source_id, "path": out }),
    );
    assert_eq!(res["format"], "format0");
    assert_eq!(res["division"], 960);
    assert!(res["bytes"].as_u64().unwrap() > 0);
    let report = &res["report"];
    assert!(report["event_count"].as_u64().unwrap() > 0);
    assert_eq!(report["exact_count"], report["event_count"]);
    assert_eq!(report["rounded_count"], 0);
    assert_eq!(report["max_error"]["num"], 0);
    assert_eq!(report["unsupported_events"], json!([]));

    // The written bytes re-parse to the same format/division.
    let doc = parse_smf(&fs::read(res["path"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(doc.format, SmfFormat::Format0);
    assert_eq!(doc.division, 960);
}

/// Export is read-only on the project — no revision bump, no
/// mutation gate.
#[test]
fn export_smf_is_read_only() {
    let mut f = fixture("export-ro");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    let source_id = import(&mut f, &path);
    let rev_before = ok(&mut f.rt, "syn.project.info", json!({}))["revision"]
        .as_u64()
        .unwrap();
    let out = f.root.join("out.mid").to_string_lossy().to_string();
    ok(
        &mut f.rt,
        "syn.project.export_smf",
        json!({ "source_id": source_id, "path": out }),
    );
    let rev_after = ok(&mut f.rt, "syn.project.info", json!({}))["revision"]
        .as_u64()
        .unwrap();
    assert_eq!(rev_before, rev_after);
}

/// Typed errors: unknown source → `not_found`; bad `format` →
/// `invalid_params`; missing params → `invalid_params`.
#[test]
fn export_smf_typed_errors() {
    let mut f = fixture("export-err");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    let source_id = import(&mut f, &path);
    let out = f.root.join("out.mid").to_string_lossy().to_string();

    let err = fail(
        &mut f.rt,
        "syn.project.export_smf",
        json!({ "source_id": "source-00000000000000000000000000000000", "path": out }),
    );
    assert_eq!(err["code"], "invalid_params");
    assert_eq!(err["details"]["smf_error"]["kind"], "not_found");

    let err = fail(
        &mut f.rt,
        "syn.project.export_smf",
        json!({ "source_id": source_id, "path": out, "format": "format2" }),
    );
    assert_eq!(err["code"], "invalid_params");

    for params in [
        json!({ "path": out }),
        json!({ "source_id": source_id }),
        json!({ "source_id": 7, "path": out }),
    ] {
        let err = fail(&mut f.rt, "syn.project.export_smf", params.clone());
        assert_eq!(err["code"], "invalid_params", "{params}");
    }
    assert!(!f.root.join("out.mid").exists());
}

// ===========================================================================
// round trip: import → undo/redo → save/reload → export → re-import
// ===========================================================================

/// `syn.project.undo`/`redo` round-trip the import through the
/// existing controller snapshot path.
#[test]
fn import_smf_undo_redo_round_trip() {
    let mut f = fixture("undo");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    import(&mut f, &path);
    assert_eq!(live_project(&f).midi_sources.len(), 1);

    ok(&mut f.rt, "syn.project.undo", json!({}));
    assert_eq!(live_project(&f).midi_sources.len(), 0);
    assert_eq!(live_project(&f).smf_documents.len(), 0);

    ok(&mut f.rt, "syn.project.redo", json!({}));
    assert_eq!(live_project(&f).midi_sources.len(), 1);
    assert_eq!(live_project(&f).smf_documents.len(), 1);
}

/// The imported source + its provenance document survive
/// `syn.project.save`/`reload` through the bundle store.
#[test]
fn import_smf_survives_save_reload() {
    let mut f = fixture("save-reload");
    let bundle = f.root.join("proj").to_string_lossy().to_string();
    ok(
        &mut f.rt,
        "syn.project.new",
        json!({ "path": bundle, "name": "smf" }),
    );
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    import(&mut f, &path);
    ok(&mut f.rt, "syn.project.save", json!({}));
    ok(&mut f.rt, "syn.project.reload", json!({}));

    let project = live_project(&f);
    assert_eq!(project.midi_sources.len(), 1);
    assert_eq!(project.smf_documents.len(), 1);
    let source = project.midi_sources.values().next().unwrap();
    assert_eq!(source.notes.len(), 2);
    assert_eq!(source.division.ticks_per_quarter(), 960);
}

/// Full op-path round trip: import → export at a different division
/// → re-import — the second source is distinct and carries the
/// re-derived native ticks.
#[test]
fn import_export_reimport_round_trip() {
    let mut f = fixture("roundtrip");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    let first = import(&mut f, &path);
    let out = f.root.join("out.mid").to_string_lossy().to_string();

    ok(
        &mut f.rt,
        "syn.project.export_smf",
        json!({ "source_id": first, "path": out, "division": 480 }),
    );
    let second = import(&mut f, &out);
    assert_ne!(first, second);

    let project = live_project(&f);
    assert_eq!(project.midi_sources.len(), 2);
    let second_id = synvelis_project::SourceId::from_tagged(&second).unwrap();
    let source = &project.midi_sources[&second_id];
    assert_eq!(source.division.ticks_per_quarter(), 480);
    assert_eq!(source.notes.len(), 2);
}

// ===========================================================================
// syn.app.capabilities — midi section
// ===========================================================================

/// The `midi` section reports the op surface `available`, the OS
/// backend split honestly (Windows implemented / real-port `NotRun`,
/// macOS+Linux unsupported), and the deferred formats listed.
#[test]
fn capabilities_reports_midi_section() {
    let mut f = fixture("caps");
    let caps = ok(&mut f.rt, "syn.app.capabilities", json!({}));
    let midi = &caps["midi"];
    assert_eq!(midi["smf"]["state"], "available");
    assert_eq!(
        midi["smf"]["ops"],
        json!(["syn.project.import_smf", "syn.project.export_smf"])
    );
    assert_eq!(midi["backend"]["windows"]["state"], "implemented");
    assert_eq!(midi["backend"]["windows"]["backend"], "winmm");
    assert_eq!(midi["backend"]["windows"]["real_port"], "not_run");
    assert_eq!(midi["backend"]["macos"]["state"], "unsupported");
    assert_eq!(midi["backend"]["linux"]["state"], "unsupported");
    let unsupported = midi["unsupported"].as_array().unwrap();
    for feature in ["smpte_division", "smf_format2", "midi2"] {
        assert!(
            unsupported
                .iter()
                .any(|u| u["feature"] == feature && u["state"] == "unsupported"),
            "missing unsupported entry for {feature}"
        );
    }
}

// ===========================================================================
// clip→MIDI plan/compile/delivery evidence
// ===========================================================================

/// A MIDI-source clip plans through `plan_midi_clips` and compiles
/// to engine-frame wire events — recorded as adapter evidence on
/// every revision build. This fixture binds no instrument (no
/// processor, no host), so the delivery record stays honestly
/// `Unresolved`; staged delivery is `tim02_midi_delivery`'s gate.
#[test]
fn midi_clip_compiles_to_engine_frame_evidence() {
    let mut f = fixture("midi-clip");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    let source_id = import(&mut f, &path);
    add_midi_clip(&f, &source_id);

    let evidence = f.engine.midi_clip_evidence();
    assert_eq!(evidence.len(), 1, "{evidence:?}");
    let entry = &evidence[0];
    assert_eq!(entry.error, None);
    assert_ne!(entry.events.len(), 0);
    assert_eq!(entry.delivery, MidiClipDelivery::Unresolved);
    // Note on/off pairs land in engine frames in non-decreasing order.
    let frames: Vec<i64> = entry.events.iter().map(|(frame, _)| *frame).collect();
    let mut sorted = frames.clone();
    sorted.sort_unstable();
    assert_eq!(frames, sorted);
}

/// A muted MIDI clip produces no evidence entry — the plan pass
/// reports it as an explicit skip, never a silent play.
#[test]
fn muted_midi_clip_produces_no_evidence() {
    let mut f = fixture("midi-muted");
    let path = write_smf(&f.root, "in.mid", &format0_notes());
    let source_id = import(&mut f, &path);
    add_midi_clip(&f, &source_id);
    let clip_id = *live_project(&f).clips.keys().next().unwrap();
    apply_project(
        &f,
        ProjectOp::SetClipMuted {
            clip_id,
            muted: true,
        },
    );

    let evidence = f.engine.midi_clip_evidence();
    assert!(evidence.is_empty(), "{evidence:?}");
}

/// `sys.capabilities` lists both SMF ops — the op surface is
/// discoverable, matching the service's registered `OpInfo`s.
#[test]
fn sys_capabilities_lists_smf_ops() {
    let mut f = fixture("ops");
    let caps = ok(&mut f.rt, "sys.capabilities", json!({}));
    let ops = serde_json::to_string(&caps).expect("caps serializes");
    assert!(ops.contains("syn.project.import_smf"), "{ops}");
    assert!(ops.contains("syn.project.export_smf"), "{ops}");
}
