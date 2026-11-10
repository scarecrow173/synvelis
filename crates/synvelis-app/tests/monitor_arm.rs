//! RCD-03 app Arm/Monitoring UX headless fixture（synvelis#117
//! slice）— `syn.monitoring.*` ops と `syn.monitoring.state` 投影を
//! test-spec E1–E7 で検証する。
//!
//! - E1: Off/Auto/On × Arm × Transport × Punch × Mute/Solo の
//!   route 行列（`synvelis-recording::decide` 消費結果の投影）。
//! - E2: Punch 境界の半開 `[in, out)` segment と live route。
//! - E3: Hardware Direct Unknown/UserReported/DeviceControlled と
//!   double-monitor notice。
//! - E4: `input_output_ready == false` の `input_unavailable`。
//! - E5: view/window/transport 系 op が monitor 状態を変えない。
//! - E6: latency/low-latency 投影（`LatencyReport::resolve`/
//!   `prepare_low_latency`）。
//! - E7: session 未開始の正直な投影（route/segment を捏造しない）。
//!
//! 実 Device/Capture/Software Monitor PCM・実 GUI/multi-window/
//! focus は本 slice の対象外（NotRun — test-spec 参照）。本 fixture
//! は headless の状態機械と投影のみを検証し、可聴結果を主張しない。

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::{BackendReport, DataRegistry, UiRuntime};

use synvelis_app::{AppService, SynvelisApp};
use synvelis_project::{LaneId, ProjectOp, StripId, TrackId};
use synvelis_workspace::ProjectSession;

fn backend() -> BackendReport {
    BackendReport::available("webview2", "test-1.0".to_string())
}

fn apply(session: &mut ProjectSession, op: ProjectOp) {
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op");
}

/// clock epoch 1 件 + track/strip/lane を 2 組持つ app を組み立てる。
/// 返すのは `(app, [(track, strip, lane); 2])`。
fn fixture_app() -> (SynvelisApp, Vec<(TrackId, StripId, LaneId)>) {
    let mut app = SynvelisApp::new(backend());
    let session = app.workspace_mut().session_mut();
    apply(
        session,
        ProjectOp::RegisterClockEpoch {
            description: "monitor-fixture".into(),
        },
    );
    let mut tracks = Vec::new();
    for name in ["a", "b"] {
        apply(session, ProjectOp::AddTrack { name: name.into() });
        let track = *session.project().tracks.keys().next_back().expect("track");
        apply(session, ProjectOp::AddTrackStrip { track_id: track });
        let strip = session
            .project()
            .routing
            .strip_of_track(track)
            .expect("strip")
            .id;
        apply(session, ProjectOp::AddLane { track_id: track });
        let lane = *session.project().tracks[&track]
            .lane_ids
            .last()
            .expect("lane");
        tracks.push((track, strip, lane));
    }
    (app, tracks)
}

fn set_decl(app: &mut SynvelisApp, params: Value) {
    app.op_monitoring_set(&params).expect("syn.monitoring.set");
}

fn punch(app: &mut SynvelisApp, params: Value) {
    app.op_monitoring_punch(&params)
        .expect("syn.monitoring.punch");
}

fn transport(app: &mut SynvelisApp, params: Value) {
    app.op_monitoring_transport(&params)
        .expect("syn.monitoring.transport");
}

/// 既定 session: `transport_start=0`・`stop=960` tick・48 kHz。
/// 既定 tempo 120 BPM / 960 PPQN では 960 tick = 24000 frame。
fn session_start(app: &mut SynvelisApp) {
    app.op_monitoring_session(&json!({
        "action": "start",
        "transport_start": 0,
        "stop": 960,
        "sample_rate": 48000,
    }))
    .expect("syn.monitoring.session start");
}

fn state(app: &SynvelisApp) -> Value {
    app.op_monitoring_state()
}

fn track_row<'a>(state: &'a Value, tagged: &str) -> &'a Value {
    state["tracks"]
        .as_array()
        .expect("tracks array")
        .iter()
        .find(|t| t["track"] == tagged)
        .unwrap_or_else(|| panic!("no track row for {tagged}"))
}

fn notice_list(state: &Value, key: &str) -> Vec<String> {
    state["notices"][key]
        .as_array()
        .expect("notice array")
        .iter()
        .map(|v| v.as_str().expect("tagged id").to_owned())
        .collect()
}

fn strip_op(app: &mut SynvelisApp, op: ProjectOp) {
    let session = app.workspace_mut().session_mut();
    apply(session, op);
}

// -- E7: session 未開始の正直な投影 -----------------------------------------

/// session 未開始では `session.active == false`・segments 空・
/// latency は `Unknown`/`Unavailable:NotPrepared`。宣言の静的投影は
/// 返るが route/segment は捏造しない（`decide(ctx, Stopped, false)`
/// の結果のみ）。Hardware Direct `Unknown` は `unknown` のまま
/// `device_controlled` に偽装しない。
#[test]
fn e7_inactive_session_projection_is_honest() {
    let (mut app, tracks) = fixture_app();
    let (track, ..) = tracks[0];
    set_decl(
        &mut app,
        json!({
            "track": track.to_tagged(),
            "arm": "on",
            "monitor": "auto",
            "input_output_ready": true,
        }),
    );

    let s = state(&app);
    assert_eq!(s["session"]["active"], false);
    assert_eq!(s["session"]["transport"], Value::Null);
    assert_eq!(s["segments"], json!([]));
    assert_eq!(s["tracks_omitted"], 0);
    assert_eq!(s["latency"]["resolution"]["quality"], "unknown");
    assert_eq!(s["latency"]["low_latency"]["state"], "unavailable");
    assert_eq!(s["latency"]["low_latency"]["reason"], "not_prepared");

    let row = track_row(&s, &track.to_tagged());
    assert_eq!(row["in_project"], true);
    assert_eq!(row["arm"], "on");
    assert_eq!(row["monitor"], "auto");
    assert_eq!(row["binding"]["kind"], "stereo");
    // Auto + Arm + Stopped は decide の規則上 Input — 捏造ではなく
    // crate 意味論の投影。
    assert_eq!(row["route"], "input");
    // Hardware Direct は既定 Unknown のまま正直に投影される。
    assert_eq!(row["hardware_direct"]["state"], "unknown");
    assert_eq!(row["notices"]["hardware_direct_unknown"], true);
    assert!(notice_list(&s, "hardware_direct_unknown").contains(&track.to_tagged()));
}

// -- E1: Off/Auto/On × Arm × Transport × Punch × Mute/Solo ------------------

/// route 行列: monitor mode × arm × transport 位相 × gate ×
/// mute/solo の組合せで `decide` の投影 route が変わること。
/// Punch gate `[480, 720)` tick（120 BPM では frame [12000,18000)）。
#[test]
fn e1_route_matrix_off_auto_on_x_transport_x_punch_x_mute_solo() {
    let (mut app, tracks) = fixture_app();
    let (ta, sa, _) = tracks[0];
    let (tb, sb, _) = tracks[1];
    punch(
        &mut app,
        json!({"action": "set", "punch_in": 480, "punch_out": 720}),
    );
    for t in [ta, tb] {
        set_decl(
            &mut app,
            json!({
                "track": t.to_tagged(),
                "arm": "on",
                "monitor": "auto",
                "input_output_ready": true,
            }),
        );
    }
    session_start(&mut app);

    // 停止中: phase=stopped。Auto+Arm は Stopped でも Input。
    let s = state(&app);
    assert_eq!(s["session"]["transport"], "stopped");
    assert_eq!(s["session"]["gate"], "punch");
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "input");

    // 再生開始・punch 前: gate 閉 → Playback。
    transport(&mut app, json!({"action": "play"}));
    let s = state(&app);
    assert_eq!(s["session"]["phase"], "punch_wait");
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "playback");

    // punch 内へ seek: gate 開 → Input。
    transport(&mut app, json!({"action": "seek", "position": 600}));
    let s = state(&app);
    assert_eq!(s["session"]["phase"], "punch_open");
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "input");

    // punch out 以降: PostRoll・gate 閉 → Playback。
    transport(&mut app, json!({"action": "seek", "position": 800}));
    let s = state(&app);
    assert_eq!(s["session"]["phase"], "post_roll");
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "playback");

    // Monitor Off + Arm On + gate 開 → Playback 抑制で Muted。
    transport(&mut app, json!({"action": "seek", "position": 600}));
    set_decl(&mut app, json!({"track": ta.to_tagged(), "monitor": "off"}));
    let s = state(&app);
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "muted");

    // Monitor On + ready → gate に依らず Input。
    set_decl(
        &mut app,
        json!({"track": ta.to_tagged(), "monitor": "on", "input_output_ready": true}),
    );
    let s = state(&app);
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "input");

    // Monitor On + !ready → Playback フォールバック + notice（E4 も兼ねる）。
    set_decl(
        &mut app,
        json!({"track": ta.to_tagged(), "input_output_ready": false}),
    );
    let s = state(&app);
    let row = track_row(&s, &ta.to_tagged());
    assert_eq!(row["route"], "playback");
    assert_eq!(row["notices"]["input_unavailable"], true);
    assert!(notice_list(&s, "input_unavailable").contains(&ta.to_tagged()));

    // Arm Off + Auto → Input route は Arm 必須なので Playback。
    set_decl(
        &mut app,
        json!({"track": ta.to_tagged(), "arm": "off", "monitor": "auto", "input_output_ready": true}),
    );
    let s = state(&app);
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "playback");

    // Mute/Solo は project strip 権威。Monitor Off + Arm Off の
    // Playback route にだけ作用する。
    for t in [ta, tb] {
        set_decl(
            &mut app,
            json!({"track": t.to_tagged(), "arm": "off", "monitor": "off"}),
        );
    }
    let s = state(&app);
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "playback");
    assert_eq!(track_row(&s, &tb.to_tagged())["route"], "playback");

    // track_a mute → Muted（track_b は無変化）。
    strip_op(
        &mut app,
        ProjectOp::SetStripMute {
            strip_id: sa,
            muted: true,
        },
    );
    let s = state(&app);
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "muted");
    assert_eq!(track_row(&s, &ta.to_tagged())["muted"], true);
    assert_eq!(track_row(&s, &tb.to_tagged())["route"], "playback");

    // mute 解除 + track_b solo → any_solo で track_a は Muted、
    // soloed の track_b は Playback のまま。
    strip_op(
        &mut app,
        ProjectOp::SetStripMute {
            strip_id: sa,
            muted: false,
        },
    );
    strip_op(
        &mut app,
        ProjectOp::SetStripSolo {
            strip_id: sb,
            soloed: true,
        },
    );
    let s = state(&app);
    assert_eq!(track_row(&s, &ta.to_tagged())["route"], "muted");
    assert_eq!(track_row(&s, &ta.to_tagged())["soloed"], false);
    assert_eq!(track_row(&s, &tb.to_tagged())["route"], "playback");
    assert_eq!(track_row(&s, &tb.to_tagged())["soloed"], true);
}

// -- E2: Punch 境界の半開区間 ----------------------------------------------

/// `MonitorProbe` が生成する segment は session 全域を連続・非重複・
/// 半開 `[in, out)` で覆い、punch 境界で正確に分割される。live route
/// は境界 tick で half-open 規則（in は開・out は閉）に従う。
#[test]
fn e2_punch_segments_are_contiguous_half_open() {
    let (mut app, tracks) = fixture_app();
    let (track, ..) = tracks[0];
    punch(
        &mut app,
        json!({"action": "set", "punch_in": 480, "punch_out": 720}),
    );
    set_decl(
        &mut app,
        json!({
            "track": track.to_tagged(),
            "arm": "on",
            "monitor": "auto",
            "input_output_ready": true,
        }),
    );
    session_start(&mut app);
    transport(&mut app, json!({"action": "play"}));

    let s = state(&app);
    let session_end = s["session"]["session_end_frames"].as_i64().expect("end");
    assert!(session_end > 0);
    let track_segments = s["segments"]
        .as_array()
        .expect("segments")
        .iter()
        .find(|t| t["track"] == track.to_tagged())
        .expect("track segments");
    assert_eq!(track_segments["truncated"], false);
    let segs = track_segments["segments"].as_array().expect("segment list");
    assert!(segs.len() >= 3, "punch gate must split into >= 3 regions");

    // 連続・非重複・半開: 各区間の start == 前区間の end、
    // 全体で [0, session_end) を覆う。
    assert_eq!(segs[0]["start"], 0);
    assert_eq!(segs.last().expect("last")["end"], session_end);
    for w in segs.windows(2) {
        assert_eq!(w[0]["end"], w[1]["start"], "segments must be contiguous");
    }
    for seg in segs {
        assert!(seg["start"].as_i64().unwrap() < seg["end"].as_i64().unwrap());
    }

    // gate_open の true 区間は連続した 1 塊（punch 区間）。
    let flags: Vec<bool> = segs
        .iter()
        .map(|g| g["gate_open"].as_bool().expect("gate_open"))
        .collect();
    let mut open_runs = 0usize;
    let mut prev = false;
    for &f in &flags {
        if f && !prev {
            open_runs += 1;
        }
        prev = f;
    }
    assert_eq!(open_runs, 1, "gate_open must be one contiguous run");
    assert!(flags.contains(&true) && flags.contains(&false));

    // 半開境界: punch_in tick は開、punch_out tick は閉。
    transport(&mut app, json!({"action": "seek", "position": 479}));
    assert_eq!(
        track_row(&state(&app), &track.to_tagged())["route"],
        "playback"
    );
    transport(&mut app, json!({"action": "seek", "position": 480}));
    assert_eq!(
        track_row(&state(&app), &track.to_tagged())["route"],
        "input"
    );
    transport(&mut app, json!({"action": "seek", "position": 720}));
    assert_eq!(
        track_row(&state(&app), &track.to_tagged())["route"],
        "playback"
    );

    // punch clear → Normal gate（transport_start から連続 open）。
    // session active 中は変更不可なので stop → clear → 再 start。
    app.op_monitoring_session(&json!({"action": "stop"}))
        .expect("session stop");
    punch(&mut app, json!({"action": "clear"}));
    session_start(&mut app);
    transport(&mut app, json!({"action": "play"}));
    let s = state(&app);
    assert_eq!(s["session"]["gate"], "normal");
    let segs = s["segments"][0]["segments"].as_array().expect("segments");
    assert!(
        segs.iter().all(|g| g["gate_open"] == true),
        "normal gate must be open for the whole session"
    );
}

// -- E3: Hardware Direct 状態と二重監視 notice --------------------------------

/// Hardware Direct は外部観測の verbatim 投影: `Unknown` は
/// `unknown` のまま notice を立て、`device_controlled` に偽装
/// しない。Input route + 外部観測 active で `double_monitoring`。
#[test]
fn e3_hardware_direct_states_and_double_monitoring() {
    let (mut app, tracks) = fixture_app();
    let (track, ..) = tracks[0];
    set_decl(
        &mut app,
        json!({
            "track": track.to_tagged(),
            "monitor": "on",
            "input_output_ready": true,
        }),
    );

    // 既定 Unknown → 正直な notice、route は Input。
    let s = state(&app);
    let row = track_row(&s, &track.to_tagged());
    assert_eq!(row["hardware_direct"]["state"], "unknown");
    assert_eq!(row["notices"]["hardware_direct_unknown"], true);
    assert_eq!(row["route"], "input");

    // UserReported + observed active → Input route で二重監視警告。
    set_decl(
        &mut app,
        json!({
            "track": track.to_tagged(),
            "hardware_direct": {"state": "user_reported", "observed_active": true},
        }),
    );
    let s = state(&app);
    let row = track_row(&s, &track.to_tagged());
    assert_eq!(row["hardware_direct"]["state"], "user_reported");
    assert_eq!(row["hardware_direct"]["observed_active"], true);
    assert_eq!(row["notices"]["double_monitoring"], true);
    assert!(notice_list(&s, "double_monitoring").contains(&track.to_tagged()));
    assert_eq!(row["notices"]["hardware_direct_unknown"], false);

    // DeviceControlled + 未観測 → どちらの notice も立たない。
    set_decl(
        &mut app,
        json!({
            "track": track.to_tagged(),
            "hardware_direct": {"state": "device_controlled"},
        }),
    );
    let s = state(&app);
    let row = track_row(&s, &track.to_tagged());
    assert_eq!(row["hardware_direct"]["state"], "device_controlled");
    assert_eq!(row["hardware_direct"]["observed_active"], Value::Null);
    assert_eq!(row["notices"]["double_monitoring"], false);
    assert_eq!(row["notices"]["hardware_direct_unknown"], false);
    assert!(notice_list(&s, "hardware_direct_unknown").is_empty());
}

// -- E4: input unavailable ---------------------------------------------------

/// Monitor On + `input_output_ready == false` は Playback
/// フォールバック + `input_unavailable` notice。Monitor Auto/Off
/// では立たない（notice は Monitor On 専用の decide 規則）。
#[test]
fn e4_input_unavailable_is_explicit() {
    let (mut app, tracks) = fixture_app();
    let (track, ..) = tracks[0];
    set_decl(
        &mut app,
        json!({
            "track": track.to_tagged(),
            "monitor": "on",
            "input_output_ready": false,
        }),
    );
    let s = state(&app);
    let row = track_row(&s, &track.to_tagged());
    assert_eq!(row["route"], "playback");
    assert_eq!(row["notices"]["input_unavailable"], true);
    assert!(notice_list(&s, "input_unavailable").contains(&track.to_tagged()));

    // Auto + !ready → input_unavailable は立たない。
    set_decl(
        &mut app,
        json!({"track": track.to_tagged(), "monitor": "auto"}),
    );
    let s = state(&app);
    assert_eq!(
        track_row(&s, &track.to_tagged())["notices"]["input_unavailable"],
        false
    );
    assert!(notice_list(&s, "input_unavailable").is_empty());
}

// -- E5: view/window/transport 系 op の不変性 ---------------------------------

/// view/window 系 read op と project transport 変更は monitor
/// session 状態を一切変えない（multi-window/focus の実 GUI 行は
/// NotRun — headless では op 面の不変性のみ検証）。
#[test]
fn e5_view_window_and_project_transport_do_not_change_monitor_state() {
    let (mut app, tracks) = fixture_app();
    let (track, ..) = tracks[0];
    set_decl(
        &mut app,
        json!({
            "track": track.to_tagged(),
            "arm": "on",
            "monitor": "auto",
            "input_output_ready": true,
        }),
    );
    punch(
        &mut app,
        json!({"action": "set", "punch_in": 480, "punch_out": 720}),
    );
    session_start(&mut app);
    transport(&mut app, json!({"action": "play"}));
    transport(&mut app, json!({"action": "seek", "position": 600}));

    let before = state(&app);
    // view/window 系 read op + project transport（monitor transport
    // とは別系統）を実行しても monitor 投影は不変。
    let _ = app.op_view_list();
    let _ = app.op_window_list();
    let _ = app.workspace_info();
    app.transport_set(true);
    let after = state(&app);
    assert_eq!(before, after);
}

// -- E6: latency / low-latency 投影 -------------------------------------------

/// `syn.monitoring.latency` の注入値が `LatencyReport::resolve` /
/// `prepare_low_latency` の結果として投影される。round_trip は
/// authoritative で reported は除外計上、`Measured` は Clock Epoch
/// 確定時のみ維持される。readiness 欠落は `Unavailable` + reason
/// を正直に返し、latency 自体は隠さない。
#[test]
fn e6_latency_and_low_latency_projection() {
    let (mut app, _) = fixture_app();

    app.op_monitoring_latency(&json!({
        "report": {
            "round_trip": {
                "frames": 640,
                "quality": "measured",
                "basis": "round_trip_measured",
                "clock_epoch": 7,
            },
            "reported": [
                {"frames": 10, "quality": "reported", "basis": "callback_reported", "clock_epoch": null},
            ],
            "reported_verified": false,
        },
        "readiness": {"prepared": true, "input_ready": true, "output_ready": true},
        "tone_difference": false,
    }))
    .expect("latency inject");

    let s = state(&app);
    let latency = &s["latency"];
    assert_eq!(latency["resolution"]["total_frames"], 640);
    assert_eq!(latency["resolution"]["quality"], "measured");
    assert_eq!(latency["resolution"]["basis"], "round_trip_measured");
    assert_eq!(latency["resolution"]["clock_epoch"], 7);
    // round_trip authoritative → reported は合計へ再加算されず除外計上。
    assert_eq!(latency["resolution"]["excluded_reported"], 1);
    assert_eq!(latency["low_latency"]["state"], "ready");
    assert_eq!(latency["low_latency"]["reason"], Value::Null);
    assert_eq!(latency["low_latency"]["tone_difference"], false);

    // tone_difference 既知 → Degraded（利用可能だが音質差あり）。
    app.op_monitoring_latency(&json!({"tone_difference": true}))
        .expect("tone_difference");
    assert_eq!(state(&app)["latency"]["low_latency"]["state"], "degraded");

    // readiness 欠落 → Unavailable:NotPrepared。latency は保持。
    app.op_monitoring_latency(&json!({
        "readiness": {"prepared": false, "input_ready": true, "output_ready": true},
    }))
    .expect("readiness");
    let s = state(&app);
    assert_eq!(s["latency"]["low_latency"]["state"], "unavailable");
    assert_eq!(s["latency"]["low_latency"]["reason"], "not_prepared");
    assert_eq!(s["latency"]["resolution"]["total_frames"], 640);

    // input 未準備 → InputNotReady 優先順位。
    app.op_monitoring_latency(&json!({
        "readiness": {"prepared": true, "input_ready": false, "output_ready": false},
    }))
    .expect("readiness");
    assert_eq!(
        state(&app)["latency"]["low_latency"]["reason"],
        "input_not_ready"
    );
}

// -- 付帯: project 非破壊・service 配線 ----------------------------------------

/// monitor ops は project revision・永続化・Undo を一切変更しない
/// （宣言は session-only）。
#[test]
fn monitoring_ops_do_not_mutate_project() {
    let (mut app, tracks) = fixture_app();
    let (track, ..) = tracks[0];
    let revision_before = app.project_info()["revision"].clone();

    set_decl(
        &mut app,
        json!({
            "track": track.to_tagged(),
            "arm": "on",
            "monitor": "auto",
            "input_output_ready": true,
        }),
    );
    punch(
        &mut app,
        json!({"action": "set", "punch_in": 480, "punch_out": 720}),
    );
    session_start(&mut app);
    transport(&mut app, json!({"action": "play"}));
    app.op_monitoring_latency(&json!({
        "readiness": {"prepared": true, "input_ready": true, "output_ready": true},
    }))
    .expect("latency");
    let _ = state(&app);

    assert_eq!(app.project_info()["revision"], revision_before);
    // session stop 後も宣言は保持され revision は不変。
    app.op_monitoring_session(&json!({"action": "stop"}))
        .expect("session stop");
    assert_eq!(state(&app)["session"]["active"], false);
    assert_eq!(app.project_info()["revision"], revision_before);
    let s = state(&app);
    let row = track_row(&s, &track.to_tagged());
    assert_eq!(row["arm"], "on");
    assert_eq!(row["monitor"], "auto");
}

/// `syn.monitoring.*` が OPS 一覧と dispatch 経路に登録され、
/// `syn.monitoring.state` が wire 経由で正直な空投影を返す。
#[test]
fn service_registers_monitoring_ops() {
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(AppService::new(SynvelisApp::new(backend())))
        .expect("register AppService");
    for op in [
        "syn.monitoring.set",
        "syn.monitoring.punch",
        "syn.monitoring.transport",
        "syn.monitoring.session",
        "syn.monitoring.latency",
        "syn.monitoring.state",
    ] {
        assert!(
            synvelis_app::OPS.iter().any(|o| o.name == op),
            "{op} missing from OPS"
        );
    }
    let events = rt.dispatch_op("test-1", "syn.monitoring.state", json!({}));
    let (ok, data, error) = events
        .iter()
        .find_map(|e| match e {
            Event::Result {
                ok, data, error, ..
            } => Some((*ok, data.clone(), error.clone())),
            _ => None,
        })
        .expect("result event");
    assert!(ok, "op failed: {error:?}");
    let data: Value = data.expect("result data");
    assert_eq!(data["session"]["active"], false);
    assert_eq!(data["segments"], json!([]));
    // error 型は dispatch 経路の型を明示するため残す。
    let _: Option<ErrorBody> = error;
}
