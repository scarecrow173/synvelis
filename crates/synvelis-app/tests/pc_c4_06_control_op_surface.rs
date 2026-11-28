//! PC-C4-06 control op surface acceptance (synvelis#101): the
//! deferred `syn.control.*` APIs are registered on `AppService` and
//! return only genuine typed evidence — `FastControlOutcome`
//! Delivered/Queued/Blocked, `SurfaceReport`/`TickReport`/
//! `ReportDrainReport` counters, and `DispatchStage` observations
//! whose `Reported` rows alone cross into the workspace
//! plugin-report feed. Every drive goes through
//! `UiRuntime::dispatch_op` — the same path the GUI host and
//! headless drivers take.
//!
//! Fixture mirrors `pc_c4_04_app_composition.rs`'s reporting plugin
//! and project shape; the app is wrapped in [`SharedService`] so the
//! fixture can still reach `app_mut()` (engine attach,
//! `note_control_drain`) after registration.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use ruviform_audio_core::ChannelLayout;
use ruviform_media_core::{MidiChannel, MidiEvent, MidiGroup, MidiKind, NoteKey};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, Event,
    EventKind, EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin,
    PluginDescriptor, PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext,
    ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{PluginHandle, PluginHost};
use ruviform_ui::ipc::{ErrorBody, Event as UiEvent};
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::ui_ops::UiOutbox;
use synvelis_app::{AppService, OPS, SessionEngineConfig, SessionRoutingEngine, SynvelisApp};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::PluginHostProcessors;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_project::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, BuildBudget,
    BuildMode, BusKind, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ConflictPolicy,
    ControlOwner, ExplicitConversionPolicy, Interpolation, LaneId, MediaDuration, MediaTimeValue,
    ParamMapping, ParameterRole, ParameterTargetId, PlaybackMapping, PluginRef, PortDirection,
    PortRef, ProcessorBinding, ProcessorId, ProjectController, ProjectOp, ProjectRevision,
    RoutingActivationState, RoutingEdit, RoutingEditIntent, SendGainSpec, SendId, SendTap,
    SourceFrame, SourceId, SourceRange, StripId, StripRef, TargetCapability, TempConnectionId,
    TempSendId, TimeAnchor, TrackId, ValueKind, port_capability_generation, send_level_target_id,
    strip_channel_target_id,
};
use synvelis_workspace::AutomationMode;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-pcc406-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap()
}

fn media_placement(start_frame: u64, end_frame: u64, rate: u32) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(
            MediaTimeValue::new(i64::try_from(start_frame).unwrap(), 1, rate).unwrap(),
        ),
        extent: ClipExtent::Absolute(
            MediaDuration::new(i64::try_from(end_frame - start_frame).unwrap(), 1, rate).unwrap(),
        ),
    }
}

fn natural_playback(offset: i64, frames: i64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(offset).unwrap(),
            SourceFrame::new(offset + frames).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn session_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: synvelis_player::lowering::EngineConfig {
            sample_rate: 48_000,
            channels: 2,
            block_frames: 256,
        },
        prefetch: ruviform_media_reader::PrefetchConfig {
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

fn import_stereo(
    c: &mut ProjectController,
    root: &Path,
    name: &str,
    frames: usize,
) -> synvelis_player::ImportOutcome {
    let wav = root.join(name);
    write_fixture(&wav, 2, 48_000, frames).unwrap();
    import_local(
        c,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap()
}

/// Track + strip + lane — returns `(track, strip, lane)`.
fn add_track_strip(c: &mut ProjectController) -> (TrackId, StripId, LaneId) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    (track, strip, lane)
}

fn add_clip(
    c: &mut ProjectController,
    lane: LaneId,
    source: SourceId,
    start: u64,
    end: u64,
    offset: i64,
) -> ClipId {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(start, end, 48_000),
            playback: natural_playback(offset, i64::try_from(end - start).unwrap()),
        },
    );
    *c.live().clips.keys().next_back().unwrap()
}

fn insert_rec_processor(c: &mut ProjectController, strip: StripId, name: &str) -> ProcessorId {
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: name.into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: REC_DESCRIPTOR.id.into(),
                version: "0.1.0".into(),
                capability_revision: 0,
            }),
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

/// A `ProcessorParam` target on `processor` declaring param 7 plus a
/// linear automation lane — the lane exercises the tick's automation
/// evaluation path (and therefore the per-pump plan-generation
/// advance the observe span must name honestly).
fn add_gain_target(c: &mut ProjectController, processor: ProcessorId) -> ParameterTargetId {
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id: 7,
                flags_bits: ParamFlags::AUTOMATABLE.bits(),
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
            }),
            base: 0.0,
        },
    );
    let target = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Processor(processor))
        .unwrap()
        .id;
    apply(c, ProjectOp::AddAutomationLane { target });
    let lane = *c
        .live()
        .control
        .lanes
        .values()
        .find(|l| l.target == target)
        .map(|l| &l.id)
        .unwrap();
    apply(
        c,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(5, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(0, 1).unwrap(),
                        value: 0.2,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(4, 1).unwrap(),
                        value: 0.6,
                    },
                ],
            },
            policy: ConflictPolicy::Replace,
        },
    );
    target
}

// ---------------------------------------------------------------- plugins

const STEREO_BUSES: [BusDescriptor; 2] = [
    BusDescriptor {
        direction: BusDirection::Input,
        layout: ChannelLayout::Stereo,
        name: "Main In",
        role: BusRole::Main,
        enabled: true,
    },
    BusDescriptor {
        direction: BusDirection::Output,
        layout: ChannelLayout::Stereo,
        name: "Main Out",
        role: BusRole::Main,
        enabled: true,
    },
];

const REC_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c4-06-rec",
    name: "PC-C4-06 Reporter",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const REC_PARAMS: [ParamInfo; 1] = [ParamInfo {
    id: ParamId(7),
    name: "Gain",
    unit: "",
    min_value: 0.0,
    max_value: 1.0,
    default_value: 0.0,
    flags: ParamFlags::AUTOMATABLE,
}];

/// A plugin that reports parameter observations back through
/// `drain_output_reports`: on its first `process` call it announces
/// `param 7 = 0.5` (plugin-initiated — distinct from any dispatched
/// value), an unbound `param 99` and one `Midi` report; on every
/// call that receives a `ParamValue` event it echoes the parameter
/// back at half value.
struct ReportPlugin {
    pending: Vec<Event>,
    announce: bool,
}

impl PluginProcessor for ReportPlugin {
    fn prepare(
        &mut self,
        _config: &ProcessConfig,
        _context: &mut ProcessContext,
    ) -> Result<(), PrepareError> {
        Ok(())
    }

    fn process<I, O>(
        &mut self,
        _inputs: &[I],
        _outputs: &mut [O],
        events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        let mut echoes = Vec::new();
        for ev in events.iter() {
            if let EventKind::ParamValue(pe) = ev.kind() {
                echoes.push(Event::new(
                    ev.frame_offset(),
                    EventKind::ParamValue(ParamEvent {
                        param_id: pe.param_id,
                        value: ParamValue::new(pe.value.get() * 0.5).unwrap(),
                    }),
                ));
            }
        }
        if self.announce {
            self.announce = false;
            self.pending.push(Event::new(
                0,
                EventKind::ParamValue(ParamEvent {
                    param_id: ParamId(7),
                    value: ParamValue::new(0.5).unwrap(),
                }),
            ));
            self.pending.push(Event::new(
                0,
                EventKind::ParamValue(ParamEvent {
                    param_id: ParamId(99),
                    value: ParamValue::new(0.25).unwrap(),
                }),
            ));
            self.pending.push(Event::new(
                0,
                EventKind::Midi(MidiEvent {
                    group: MidiGroup::ZERO,
                    channel: MidiChannel::new(0).unwrap(),
                    kind: MidiKind::NoteOn {
                        key: NoteKey::new(60).unwrap(),
                        velocity: 100,
                    },
                }),
            ));
        }
        self.pending.extend(echoes);
        ProcessResult::CONTINUE
    }

    fn drain_output_reports(&mut self, out: &mut Vec<Event>) {
        out.append(&mut self.pending);
    }
}

impl Plugin for ReportPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &REC_PARAMS
    }
    fn buses(&self) -> Option<&[BusDescriptor]> {
        Some(&STEREO_BUSES)
    }
}

/// `instances.len()` after N generations is the per-generation
/// isolation evidence — each publish vends a fresh plugin instance.
type InstanceLogs = Arc<Mutex<Vec<()>>>;

struct ReportFactory {
    instances: InstanceLogs,
}

impl PluginFactory for ReportFactory {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        self.instances.lock().unwrap().push(());
        erase(ReportPlugin {
            pending: Vec::new(),
            announce: true,
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        self.instances.lock().unwrap().push(());
        Some(erase_sendable(ReportPlugin {
            pending: Vec::new(),
            announce: true,
        }))
    }
}

// ---------------------------------------------------------------- CTL-02 endpoint fixture

fn strip_main(strip: StripId, direction: PortDirection) -> PortRef {
    PortRef::StripMain {
        strip: StripRef::Existing(strip),
        direction,
    }
}

fn routing_intent(c: &ProjectController, edits: Vec<RoutingEdit>) -> RoutingEditIntent {
    RoutingEditIntent {
        operation_id: "ctl02-op".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits,
        approved_conversion: ExplicitConversionPolicy::None,
    }
}

fn create_send(c: &mut ProjectController, source: StripId, destination: StripId) -> SendId {
    apply(
        c,
        ProjectOp::Routing {
            intent: routing_intent(
                c,
                vec![RoutingEdit::CreateSend {
                    temp_id: TempSendId(1),
                    temp_connection: TempConnectionId(1),
                    source: strip_main(source, PortDirection::Output),
                    destination: strip_main(destination, PortDirection::Input),
                    tap: SendTap::PostFader,
                    level: SendGainSpec::Create {
                        name: "send level".into(),
                        initial: 1.0,
                    },
                    channel_map: None,
                }],
            ),
        },
    );
    *c.live().routing.sends.keys().next_back().unwrap()
}

struct ControlFixture {
    svc: Rc<RefCell<AppService>>,
    rt: UiRuntime,
    outbox: UiOutbox,
    strip: StripId,
    fader: ParameterTargetId,
    send: SendId,
    send_level: ParameterTargetId,
    pan: ParameterTargetId,
    mute: ParameterTargetId,
    solo: ParameterTargetId,
}

/// strip/send topology を production `ProjectOp` で構築した永続化
/// project bundle fixture。automation mode の選択だけは session 直接
/// setup とする。この slice には mode 選択 endpoint が存在しないが、
/// 測定対象の gesture 自体はすべて `syn.control.subscribe` /
/// `dispatch` を通す。
fn control_fixture(root: &Path) -> ControlFixture {
    let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let outbox = app.event_outbox();
    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(Rc::clone(&svc)))
        .expect("register AppService");
    let bundle = root.join("bundle");
    ok(
        &mut rt,
        "syn.project.new",
        json!({ "path": bundle.to_string_lossy(), "name": "ctl02" }),
    );

    let (strip, fader, send, send_level, pan, mute, solo) = {
        let mut svc = svc.borrow_mut();
        let session = svc.app_mut().workspace_mut().session_mut();
        let c = session.controller_mut();
        let (_track, strip, _lane) = add_track_strip(c);
        apply(
            c,
            ProjectOp::AddBus {
                kind: BusKind::Return,
                name: "verb".into(),
            },
        );
        let bus = *c.live().routing.buses.keys().next_back().unwrap();
        let ret_strip = c.live().routing.buses[&bus].strip;
        let send = create_send(c, strip, ret_strip);
        let fader = strip_channel_target_id(c.live(), strip, ParameterRole::Fader).unwrap();
        let mute = strip_channel_target_id(c.live(), strip, ParameterRole::Mute).unwrap();
        let solo = strip_channel_target_id(c.live(), strip, ParameterRole::Solo).unwrap();
        apply(
            c,
            ProjectOp::AddParameterTarget {
                name: "Pan".into(),
                owner: ControlOwner::Strip(strip),
                role: ParameterRole::Pan,
                time_scope: AutomationTimeScope::ProjectMusical,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.5,
            },
        );
        let pan = strip_channel_target_id(c.live(), strip, ParameterRole::Pan).unwrap();
        let send_level = send_level_target_id(c.live(), send).unwrap();
        let project = c.live().clone();
        for target in [fader, send_level] {
            session
                .control_mut()
                .set_mode(&project, target, AutomationMode::Off)
                .unwrap();
        }
        (strip, fader, send, send_level, pan, mute, solo)
    };

    ControlFixture {
        svc,
        rt,
        outbox,
        strip,
        fader,
        send,
        send_level,
        pan,
        mute,
        solo,
    }
}

fn ctl_notifies(outbox: &UiOutbox, channel: &str) -> Vec<Value> {
    let mut matched = Vec::new();
    let mut rest = Vec::new();
    {
        let mut q = outbox.borrow_mut();
        while let Some(e) = q.pop_front() {
            match &e {
                UiEvent::Notify { data, .. } if data["channel"] == channel => {
                    matched.push(data["data"].clone());
                }
                _ => rest.push(e),
            }
        }
        for e in rest.into_iter().rev() {
            q.push_front(e);
        }
    }
    matched
}

fn ctl_drain(outbox: &UiOutbox) {
    outbox.borrow_mut().clear();
}

fn ctl_attach(rt: &mut UiRuntime, view: &str, window: &str) {
    ok(
        rt,
        "syn.view.attach",
        json!({ "view_id": view, "window_id": window }),
    );
}

fn ctl_subscribe(rt: &mut UiRuntime, view: &str, target: ParameterTargetId) {
    ok(
        rt,
        "syn.control.subscribe",
        json!({ "view_id": view, "target": target.to_tagged() }),
    );
}

fn ctl_dispatch(rt: &mut UiRuntime, view: &str, cmd: &str, intent: &Value, generation: u32) {
    let ack = ok(
        rt,
        "syn.control.dispatch",
        json!({
            "intent": intent.clone(),
            "ctx": {
                "commandId": cmd,
                "viewId": view,
                "generation": u64::from(generation),
                "issuedAt": 0u64,
            },
        }),
    );
    assert_eq!(ack["accepted"], true);
}

fn param_gesture(
    kind: &str,
    target: ParameterTargetId,
    gesture: &str,
    sequence: u64,
    value: f64,
) -> Value {
    json!({
        "kind": kind,
        "target": target.to_tagged(),
        "controlId": "ctl02-control",
        "sequence": sequence,
        "gestureId": gesture,
        "originView": "mixer",
        "value": { "plain": value, "normalized": value },
    })
}

fn commit_param(
    rt: &mut UiRuntime,
    outbox: &UiOutbox,
    target: ParameterTargetId,
    gesture: &str,
    value: f64,
) {
    for (kind, sequence) in [("begin", 1), ("preview", 2), ("commit", 3)] {
        ctl_dispatch(
            rt,
            "mixer",
            &format!("{gesture}-{kind}"),
            &param_gesture(kind, target, gesture, sequence, value),
            1,
        );
    }
    let receipts = ctl_notifies(outbox, "syn.control.receipt");
    let commit_id = format!("{gesture}-commit");
    let commit = receipts
        .iter()
        .find(|r| r["receipt"]["commandId"] == commit_id)
        .unwrap_or_else(|| panic!("missing commit receipt: {receipts:?}"));
    assert_eq!(commit["receipt"]["status"], "applied");
    assert_eq!(commit["receipt"]["detail"], "base edit");
}

fn target_base(fx: &ControlFixture, target: ParameterTargetId) -> f64 {
    fx.svc
        .borrow()
        .app()
        .workspace()
        .session()
        .project()
        .control
        .targets[&target]
        .base
}

fn latest_projection(fx: &ControlFixture, target: ParameterTargetId) -> Value {
    let rows = ctl_notifies(&fx.outbox, "syn.control.projection");
    rows.iter()
        .rfind(|row| row["target"] == target.to_tagged())
        .unwrap_or_else(|| panic!("missing projection for {}: {rows:?}", target.to_tagged()))
        .clone()
}

/// CTL-02 endpoint 統合: Fader と `SendLevel` を
/// `syn.control.subscribe` + `syn.control.dispatch` gesture だけで
/// 書き込み、同一 target id/value が projection/receipt、project
/// Undo/Redo、Save/Reload を通って残ることを確認する。第二の永続化
/// gain field は作らない。
#[test]
fn strip_and_send_endpoint_authority_survives_history_and_reload() {
    let root = scratch("endpoint-authority");
    let mut fx = control_fixture(&root);
    ctl_attach(&mut fx.rt, "mixer", "win-1");
    for target in [fx.fader, fx.send_level] {
        ctl_subscribe(&mut fx.rt, "mixer", target);
    }
    // subscription 自体が projection 証拠である。gesture が値を変える
    // 前に、両方の internal target が同じ row channel に入る。
    let initial_rows = ctl_notifies(&fx.outbox, "syn.control.projection");
    for target in [fx.fader, fx.send_level] {
        assert!(
            initial_rows
                .iter()
                .any(|row| row["target"] == target.to_tagged()),
            "missing initial projection for {}",
            target.to_tagged()
        );
    }
    ctl_drain(&fx.outbox);

    commit_param(&mut fx.rt, &fx.outbox, fx.fader, "g-fader", 0.625);
    commit_param(&mut fx.rt, &fx.outbox, fx.send_level, "g-send", 0.375);
    assert_eq!(target_base(&fx, fx.fader), 0.625);
    assert_eq!(target_base(&fx, fx.send_level), 0.375);

    assert_eq!(
        ok(&mut fx.rt, "syn.project.undo", json!({}))["undone"],
        true
    );
    assert_eq!(target_base(&fx, fx.send_level), 1.0);
    assert_eq!(target_base(&fx, fx.fader), 0.625);
    assert_eq!(
        ok(&mut fx.rt, "syn.project.undo", json!({}))["undone"],
        true
    );
    assert_eq!(target_base(&fx, fx.fader), 1.0);
    assert_eq!(
        ok(&mut fx.rt, "syn.project.redo", json!({}))["redone"],
        true
    );
    assert_eq!(target_base(&fx, fx.fader), 0.625);
    assert_eq!(
        ok(&mut fx.rt, "syn.project.redo", json!({}))["redone"],
        true
    );
    assert_eq!(target_base(&fx, fx.send_level), 0.375);

    ok(&mut fx.rt, "syn.project.save", json!({}));
    // reload は workspace を置き換える。先に reload 前 view を閉じ、
    // reload 後の観測を stale UI cache の再利用ではなく unmount/remount
    // 後の新しい binding に合わせる。
    ok(&mut fx.rt, "syn.view.close", json!({ "view_id": "mixer" }));
    ok(&mut fx.rt, "syn.project.reload", json!({}));
    assert_eq!(target_base(&fx, fx.fader), 0.625);
    assert_eq!(target_base(&fx, fx.send_level), 0.375);
    {
        let svc = fx.svc.borrow();
        let project = svc.app().workspace().session().project();
        assert_eq!(
            project.routing.sends[&fx.send].gain_target,
            Some(fx.send_level)
        );
        assert_eq!(project.control.targets[&fx.fader].capability, None);
        assert_eq!(project.control.targets[&fx.send_level].capability, None);
    }

    ctl_attach(&mut fx.rt, "mixer-reload", "win-1");
    ctl_subscribe(&mut fx.rt, "mixer-reload", fx.fader);
    ctl_subscribe(&mut fx.rt, "mixer-reload", fx.send_level);
    let rows = ctl_notifies(&fx.outbox, "syn.control.projection");
    for (target, value) in [(fx.fader, 0.625), (fx.send_level, 0.375)] {
        let row = rows
            .iter()
            .rev()
            .find(|row| row["target"] == target.to_tagged())
            .unwrap_or_else(|| panic!("missing initial row for {}: {rows:?}", target.to_tagged()));
        assert_eq!(row["row"]["observed"]["value"]["normalized"], value);
    }
    let listed = ok(&mut fx.rt, "syn.control.targets", json!({}));
    for target in [fx.fader, fx.send_level] {
        assert!(
            listed["targets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["target"] == target.to_tagged()),
            "{} missing from control targets",
            target.to_tagged()
        );
    }
}

/// CTL-02 endpoint authority 観測: 残りの strip role である
/// Pan/Mute/Solo が project parameter target として解決され、同じ
/// subscribe/projection endpoint 経路で永続化 base を公開することを
/// 確認する。
#[test]
fn pan_mute_solo_endpoint_authority_observation() {
    let root = scratch("endpoint-strip-roles");
    let mut fx = control_fixture(&root);
    ctl_attach(&mut fx.rt, "mixer", "win-1");
    ctl_drain(&fx.outbox);
    for (target, expected) in [(fx.pan, 0.5), (fx.mute, 0.0), (fx.solo, 0.0)] {
        ctl_subscribe(&mut fx.rt, "mixer", target);
        let row = latest_projection(&fx, target);
        assert_eq!(row["row"]["kind"], "param");
        assert_eq!(row["row"]["observed"]["value"]["normalized"], expected);
        assert_eq!(row["row"]["observed"]["value"]["read_only"], false);
    }
    {
        let svc = fx.svc.borrow();
        let project = svc.app().workspace().session().project();
        for target in [fx.pan, fx.mute, fx.solo] {
            assert_eq!(
                project.control.targets[&target].owner,
                ControlOwner::Strip(fx.strip)
            );
        }
    }
    let listed = ok(&mut fx.rt, "syn.control.targets", json!({}));
    for target in [fx.pan, fx.mute, fx.solo] {
        assert!(
            listed["targets"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["target"] == target.to_tagged() && row["kind"] == "param"),
            "{} missing from control targets",
            target.to_tagged()
        );
    }
}

// ---------------------------------------------------------------- service fixture

/// `AppService` behind a shared handle — the fixture reaches
/// `app_mut()` (project mutation, `note_control_drain`) through the
/// same object `UiRuntime` dispatches into.
struct SharedService(Rc<RefCell<AppService>>);

impl Service for SharedService {
    fn ops(&self) -> &'static [OpInfo] {
        OPS
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
    svc: Rc<RefCell<AppService>>,
    rt: UiRuntime,
    engine: SessionRoutingEngine,
    processor: ProcessorId,
    target: ParameterTargetId,
    host: Rc<RefCell<PluginHost>>,
}

/// App + session + engine + plugin host behind `AppService`, with
/// the project already holding one imported clip, one rec-bound
/// chain processor and one param-7 target — identical composition
/// to `pc_c4_04_app_composition.rs`, but every behavioral drive in
/// the tests below goes through `syn.*` ops.
fn fixture(root: &Path) -> Fixture {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let (processor, _strip, target) = {
        let c = app.workspace_mut().session_mut().controller_mut();
        let out = import_stereo(c, root, "a.wav", 4_800);
        let (_track, strip, lane) = add_track_strip(c);
        add_clip(c, lane, out.commit.source_id, 0, 480, 0);
        let processor = insert_rec_processor(c, strip, "rec");
        let target = add_gain_target(c, processor);
        (processor, strip, target)
    };

    let mut host = PluginHost::new();
    let instances = Arc::new(Mutex::new(Vec::new()));
    host.register_factory(Box::new(ReportFactory {
        instances: Arc::clone(&instances),
    }));
    let mut processors = PluginHostProcessors::new(host);
    processors.register_params(REC_DESCRIPTOR.id, REC_PARAMS.to_vec());
    let cell = processors.param_catalog_cell();
    let host_rc = processors.host();

    let engine = SessionRoutingEngine::new(session_config(root.join("media")));
    engine.set_processor_host(Box::new(processors));
    engine.set_param_catalog(cell);
    app.attach_routing_engine(engine.clone());

    // First notification → prepare + publish → attach/sync.
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "pcc406".into(),
            },
            rev,
        )
        .unwrap();

    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(Rc::clone(&svc)))
        .expect("register AppService");

    Fixture {
        svc,
        rt,
        engine,
        processor,
        target,
        host: host_rc,
    }
}

/// A bare app with no routing engine — the honest empty-surface
/// fixture.
fn bare_runtime() -> UiRuntime {
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(AppService::new(SynvelisApp::new(BackendReport::available(
        "gpui",
        "test-0.1".to_string(),
    ))))
    .expect("register AppService");
    rt
}

/// Dispatches `op` and extracts the single `result` event payload.
fn call(rt: &mut UiRuntime, op: &str, params: Value) -> (bool, Option<Value>, Option<ErrorBody>) {
    let events = rt.dispatch_op("test-1", op, params);
    let result = events.iter().find_map(|e| match e {
        UiEvent::Result {
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

/// RT-observed activation generation — the same value the dispatch
/// gates consume via `rt_status().active_generation`.
fn active_generation(engine: &SessionRoutingEngine) -> Option<u64> {
    let receipt = engine.receipt().expect("receipt");
    match receipt.state {
        RoutingActivationState::Active { generation, .. } => Some(generation),
        _ => None,
    }
}

fn activate(engine: &SessionRoutingEngine) -> u64 {
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    active_generation(engine).expect("Active receipt after run")
}

fn revision(fx: &Fixture) -> u64 {
    fx.svc.borrow().app().workspace().session().revision().0
}

/// `syn.control.fast_control` params for `target` at `position_frames`
/// engine frames in the surface's verified frame base.
fn fast_params(target: ParameterTargetId, value: f64, means: &str, position_frames: i64) -> Value {
    json!({
        "target": target.to_tagged(),
        "value": value,
        "means": means,
        "position_frames": position_frames,
        "project_revision": 0,
    })
}

/// `syn.control.observe` params — the caller-supplied physical call
/// window `[0, 256)` and a deliverable Playing span `[0, 512)` whose
/// `plan_generation` the caller names honestly (learned from prior
/// receipts; each op's trailing `ui_pump` stages one automation plan).
fn observe_params(processor: ProcessorId, generation: u64, plan_generation: u64) -> Value {
    json!({
        "processor": processor.to_tagged(),
        "generation": generation,
        "call": { "epoch": 0, "frame": 0, "frames": 256 },
        "span": {
            "start": { "epoch": 0, "frame": 0 },
            "end": { "epoch": 0, "frame": 512 },
            "status": "playing",
            "position_ticks": 0,
            "position_base": { "numerator": 1, "denominator": 48_000 },
            "loop_ticks": null,
            "occurrence": 0,
            "clock_epoch": 0,
            "transport_generation": 0,
            "plan_generation": plan_generation,
            "tempo_revision": 0,
            "deliverable": true
        }
    })
}

/// Drains `generation`'s vended endpoints through the real
/// `PluginHost::drain_output_reports` — the composition-root path the
/// app deliberately does not own — then records the typed report on
/// the app for `syn.control.status`.
fn drain_and_record(fx: &mut Fixture, generation: u64) -> synvelis_player::ReportDrainReport {
    let endpoints = fx.engine.plugin_state_handles(generation);
    let active = active_generation(&fx.engine);
    let report = fx
        .engine
        .with_control_dispatch(|dispatch| {
            dispatch.drain_output_reports(
                &endpoints,
                generation,
                active,
                &mut |handle: PluginHandle, out: &mut Vec<Event>| {
                    fx.host.borrow_mut().drain_output_reports(handle, out)
                },
            )
        })
        .expect("attached surface");
    fx.svc
        .borrow_mut()
        .app_mut()
        .note_control_drain(report.clone());
    report
}

// ---------------------------------------------------------------- tests

/// Registration + honest empty state: the three ops exist on the
/// surface, and with no routing engine attached every evidence field
/// is `null`/`false` — nothing is fabricated.
#[test]
fn control_ops_fail_closed_without_surface() {
    let mut rt = bare_runtime();

    let status = ok(&mut rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], false);
    assert!(status["generation"].is_null());
    assert!(status["active_generation"].is_null());
    assert!(status["frame_base"].is_null());
    assert!(status["last_error"].is_null());
    assert!(status["surface"].is_null());
    assert!(status["tick"].is_null());
    assert_eq!(status["drain"]["attached"], false);
    assert_eq!(status["drain"]["drained"], 0);

    // A well-formed write still fails closed — no surface, no queue.
    let err = fail(
        &mut rt,
        "syn.control.fast_control",
        fast_params(ParameterTargetId::new(1), 0.5, "timed_event", 0),
    );
    assert_eq!(err["code"], "service_error");

    let err = fail(
        &mut rt,
        "syn.control.observe",
        observe_params(ProcessorId::new(1), 1, 1),
    );
    assert_eq!(err["code"], "service_error");
}

/// `syn.control.status` projects the attached surface's typed
/// evidence verbatim; `syn.control.fast_control` returns the player's
/// `FastControlOutcome` — Delivered while active, `blocked` with a
/// typed kind (plus the recorded block detail) on typed refusals.
#[test]
fn fast_control_and_status_return_typed_evidence() {
    let root = scratch("typed");
    let mut fx = fixture(&root);
    let generation = fx
        .engine
        .control_generation()
        .expect("surface attached at publish");
    assert_eq!(generation, 1, "first published generation");

    // Status before any RT activation: surface evidence present,
    // active_generation honestly null (never observed → gated).
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], true);
    assert_eq!(status["generation"], 1);
    assert!(status["active_generation"].is_null());
    assert_eq!(status["frame_base"]["numerator"], 1);
    assert_eq!(status["frame_base"]["denominator"], 48_000);
    assert_eq!(status["surface"]["generation"], 1);
    assert_eq!(status["surface"]["bound_targets"], 1);
    assert_eq!(
        status["surface"]["controlled_processors"],
        json!([fx.processor.to_tagged()])
    );
    assert!(status["surface"]["unbound"].as_object().unwrap().is_empty());
    assert!(status["tick"].is_null(), "no pump-driven tick yet");
    assert!(status["last_error"].is_null());

    activate(&fx.engine);

    // Delivered while the surface's generation is RT-active. The op
    // stamps `position` in the surface's verified frame base — the
    // same base `status.frame_base` reports.
    let mut params = fast_params(fx.target, 0.7, "timed_event", 100);
    params["project_revision"] = json!(revision(&fx));
    let outcome = ok(&mut fx.rt, "syn.control.fast_control", params);
    assert_eq!(outcome["outcome"], "delivered");

    // The trailing pump ran one control tick — status now carries it.
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["active_generation"], 1);
    let tick = &status["tick"];
    assert_eq!(tick["attached"], true);
    assert_eq!(tick["generation"], 1);
    assert_eq!(tick["gated"], false);
    assert!(tick["evaluated"].as_u64().unwrap() >= 1);

    // Typed refusals stay typed: unsupported means lowers to
    // `Blocked(Lowering)` — never silently downgraded to TimedEvent.
    let mut params = fast_params(fx.target, 0.7, "immediate", 100);
    params["project_revision"] = json!(revision(&fx));
    let outcome = ok(&mut fx.rt, "syn.control.fast_control", params);
    assert_eq!(outcome["outcome"], "blocked");
    assert_eq!(outcome["block"], "lowering");
    assert!(outcome["block_detail"].is_string());

    // A well-formed but unbound target id blocks `unbound_target`.
    let mut params = fast_params(ParameterTargetId::new(0xdead), 0.5, "timed_event", 0);
    params["project_revision"] = json!(revision(&fx));
    let outcome = ok(&mut fx.rt, "syn.control.fast_control", params);
    assert_eq!(outcome["outcome"], "blocked");
    assert_eq!(outcome["block"], "unbound_target");

    // Malformed params fail `invalid_params`, not a fabricated write.
    let err = fail(
        &mut fx.rt,
        "syn.control.fast_control",
        fast_params(fx.target, 0.5, "bogus_means", 0),
    );
    assert_eq!(err["code"], "invalid_params");
    let err = fail(
        &mut fx.rt,
        "syn.control.fast_control",
        json!({ "target": "not-a-tagged-id" }),
    );
    assert_eq!(err["code"], "invalid_params");
}

/// `syn.control.observe` advances the node's ledger through the
/// caller-supplied call window: host-resolved stages
/// (`scheduled`/`dispatched`) never feed the plugin-report channel —
/// only the `Reported` row the composition-root drain produced does.
/// Scheduler receipts are drain-once evidence: the engine-driven
/// per-block `observe_block` (observe-engine-wiring) consumes them
/// during `run`, so the explicit op reports only what its own call
/// drained — the applied-generation evidence is asserted on the
/// drained observation cycles instead.
#[test]
fn observe_reports_stages_and_feeds_only_reported() {
    let root = scratch("observe");
    let mut fx = fixture(&root);
    let generation = activate(&fx.engine);

    // Delivered fast write (plan gen 1) + the op's trailing pump
    // automation dispatch (plan gen 2) — last_plan is gen 2.
    let mut params = fast_params(fx.target, 0.7, "timed_event", 100);
    params["project_revision"] = json!(revision(&fx));
    let outcome = ok(&mut fx.rt, "syn.control.fast_control", params);
    assert_eq!(outcome["outcome"], "delivered");

    // RT applies both staged plans. The engine-driven per-block
    // observation drains the scheduler receipts into the buffered
    // cycles — drain-once evidence the explicit observe below can no
    // longer see. The drained cycles carry the applied-generation
    // receipts (the same evidence a driver uses to name the live
    // `plan_generation`).
    fx.engine.run(2, PumpPolicy::Every(1)).unwrap();
    let observed = fx.engine.control_drain_observations();
    assert!(
        observed.iter().any(|cycle| {
            cycle.receipts.iter().any(|receipt| {
                receipt.plan_generation == ruviform_media_core::PlanGeneration::new(2)
                    && receipt.status == ruviform_media_core::ApplyStatus::Applied
            })
        }),
        "drained observation cycles confirm applied plan generation 2"
    );

    // First observe: span names the live plan generation (2). The
    // automation entry projects `dispatched` — host-resolved, so
    // `fed` must be 0. Its own drain finds the report channel empty —
    // the engine observation above already consumed the receipts.
    let result = ok(
        &mut fx.rt,
        "syn.control.observe",
        observe_params(fx.processor, generation, 2),
    );
    let observations = result["observations"].as_array().unwrap();
    assert!(
        observations.iter().any(|row| {
            row["target"] == json!(fx.target.to_tagged())
                && row["stage"] == "dispatched"
                && row["value"].is_number()
        }),
        "dispatched observation projected: {observations:?}"
    );
    assert_eq!(result["fed"], 0, "host-resolved stages never feed");
    assert!(
        result["receipts"].as_array().unwrap().is_empty(),
        "scheduler receipts are drain-once; engine observation consumed them: {:?}",
        result["receipts"]
    );

    // Composition-root drain: the plugin's announce/echo reports
    // advance the ledger entry to `Reported`; the typed
    // ReportDrainReport is recorded for status.
    let drain = drain_and_record(&mut fx, generation);
    assert_eq!(drain.drained, 1, "one endpoint drained");
    assert!(drain.param_reports >= 1);
    assert!(drain.reported >= 1, "a report advanced the ledger");
    assert!(!drain.generation_gated && !drain.inactive_gated);

    // The observe op's pump staged plan gen 3; run applies it so the
    // scheduler receipt confirms the live generation before the next
    // observe projects.
    fx.engine.run(2, PumpPolicy::Every(1)).unwrap();

    // Second observe: the same entry is now `reported` — plugin
    // evidence — and exactly one row crosses into the feed.
    let result = ok(
        &mut fx.rt,
        "syn.control.observe",
        observe_params(fx.processor, generation, 3),
    );
    let reported = result["observations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["stage"] == "reported")
        .cloned()
        .expect("reported observation after drain");
    assert_eq!(reported["target"], json!(fx.target.to_tagged()));
    let reported_value = reported["value"].as_f64().expect("reported value");
    assert!(
        (0.0..=1.0).contains(&reported_value),
        "plugin-reported normalized value"
    );
    assert_ne!(
        reported_value, 0.7,
        "the fed value is plugin-reported (announce/echo), never the \
         host-resolved dispatched value relabeled"
    );
    assert_eq!(result["fed"], 1, "one Reported observation fed");

    // Workspace evidence: the plugin-report channel holds the value
    // under an `Estimated` stamp → `Stale`, never `Live`/`Captured`.
    let observation = fx
        .svc
        .borrow()
        .app()
        .workspace()
        .session()
        .control()
        .observation(fx.target);
    let data = observation
        .plugin_reported
        .expect("plugin report reached ControlSession");
    assert_eq!(data.value, reported_value);
    assert_eq!(
        data.stamp.provenance,
        ruviform_media_core::TimestampProvenance::Estimated,
        "app-fed reports are backend-estimated evidence"
    );

    // Status now carries the recorded drain report verbatim.
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    let drain_json = &status["drain"];
    assert_eq!(drain_json["attached"], true);
    assert_eq!(drain_json["generation"], 1);
    assert_eq!(drain_json["drained"], 1);
    assert!(drain_json["param_reports"].as_u64().unwrap() >= 1);
    assert!(drain_json["reported"].as_u64().unwrap() >= 1);
    assert_eq!(drain_json["generation_gated"], false);
    assert_eq!(drain_json["inactive_gated"], false);
}

/// Generation gates fail closed on the wire: a stale routing
/// generation or an unknown processor errors before any observation
/// runs — a foreign generation's reports can never be attributed.
#[test]
fn observe_fails_closed_on_stale_generation() {
    let root = scratch("gate");
    let mut fx = fixture(&root);
    let generation = activate(&fx.engine);

    // Stale routing generation — rejected before observe_cycle.
    let err = fail(
        &mut fx.rt,
        "syn.control.observe",
        observe_params(fx.processor, generation + 1, 1),
    );
    assert_eq!(err["code"], "service_error");

    // Well-formed but unknown processor — no control node on the
    // surface.
    let err = fail(
        &mut fx.rt,
        "syn.control.observe",
        observe_params(ProcessorId::new(0xdead), generation, 1),
    );
    assert_eq!(err["code"], "service_error");

    // Malformed span — invalid_params, never a fabricated context.
    let mut params = observe_params(fx.processor, generation, 1);
    params["span"]["status"] = json!("bogus");
    let err = fail(&mut fx.rt, "syn.control.observe", params);
    assert_eq!(err["code"], "invalid_params");

    let err = fail(
        &mut fx.rt,
        "syn.control.observe",
        json!({ "processor": "x" }),
    );
    assert_eq!(err["code"], "invalid_params");
}
