//! TIM-02 clip→node delivery (synvelis#623) — the app production
//! path: `SessionRoutingEngine` rebuild compiles each MIDI clip to
//! engine-frame events, resolves its lane→track→strip chain-head
//! instrument node, stages the set through the control dispatch,
//! and records the per-clip outcome on `midi_clip_evidence()`.
//!
//! A fake instrument plugin at the strip's chain head is the
//! oracle: after `engine.run` pumps the staged plan, the plugin's
//! `EventList` must carry exactly the compiled events whose engine
//! frames the run window covers. Clips with no instrument report
//! `Unresolved`, clips whose surface cannot stage report `Failed` —
//! evidence, never a silent skip.
//!
//! Boundary: headless evidence only. Real OS plugin hosting and GUI
//! acceptance stay `NotRun` (external gate).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use synvelis_app::engine::{MidiClipDelivery, MidiClipEvidence};
use synvelis_app::{SessionEngineConfig, SessionRoutingEngine, SynvelisApp};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::host::PluginHostProcessors;
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, LaneId,
    MusicalDuration, MusicalTick, PlaybackMapping, PluginRef, ProcessorBinding, ProcessorId,
    ProjectController, ProjectOp, ProjectRevision, SmfFormat, SourceFrame, SourceId, SourceRange,
    StripId, TimeAnchor, TrackId, parse_smf,
};

use ruviform_audio_core::ChannelLayout;
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamFlags, ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor,
    PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase,
    erase_sendable,
};
use ruviform_plugin_host::PluginHost;
use ruviform_ui::BackendReport;

use ruviform_media_core::{MidiEvent, MidiKind};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-tim02del-{}-{}-{}",
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

// ---------- fake instrument plugin (EventKind::Midi recorder) ----------

/// One delivered MIDI event at its position-domain frame — the log
/// proves the staged clip events reached the chain-head node's
/// `EventList` at their compiled engine frames.
#[derive(Debug, Clone, PartialEq)]
struct Rec {
    abs_frame: i64,
    event: MidiEvent,
}

type InstanceLog = Arc<Mutex<Vec<Arc<Mutex<Vec<Rec>>>>>>;

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

const INSTRUMENT_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim02del-instrument",
    name: "TIM-02 Delivery Instrument",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const INSTRUMENT_PARAMS: [ParamInfo; 1] = [ParamInfo {
    id: ruviform_plugin_core::ParamId(3),
    name: "Cut",
    unit: "",
    min_value: 0.0,
    max_value: 1.0,
    default_value: 0.0,
    flags: ParamFlags::AUTOMATABLE,
}];

struct InstrumentPlugin {
    delivered: Arc<Mutex<Vec<Rec>>>,
}

impl PluginProcessor for InstrumentPlugin {
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
        transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        let pos = transport.map_or(0, |t| t.position().value());
        let mut log = self.delivered.lock().unwrap();
        for ev in events.iter() {
            if let EventKind::Midi(event) = ev.kind() {
                log.push(Rec {
                    abs_frame: pos + i64::from(ev.frame_offset()),
                    event,
                });
            }
        }
        ProcessResult::CONTINUE
    }
}

impl Plugin for InstrumentPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        INSTRUMENT_DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &INSTRUMENT_PARAMS
    }
    fn buses(&self) -> Option<&[BusDescriptor]> {
        Some(&STEREO_BUSES)
    }
}

struct InstrumentFactory {
    instances: InstanceLog,
}

impl PluginFactory for InstrumentFactory {
    fn descriptor(&self) -> PluginDescriptor {
        INSTRUMENT_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        erase(InstrumentPlugin { delivered })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        Some(erase_sendable(InstrumentPlugin { delivered }))
    }
}

fn instrument_host() -> (PluginHostProcessors, InstanceLog) {
    let instances = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(InstrumentFactory {
        instances: Arc::clone(&instances),
    }));
    (PluginHostProcessors::new(host), instances)
}

fn delivered(log: &InstanceLog, instance: usize) -> Vec<Rec> {
    log.lock().unwrap()[instance].lock().unwrap().clone()
}

// ---------- SMF wire bytes (division 960 == project PPQN) ----------

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

/// Format 0, division 960, tempo `500_000` µs/q — at 48 kHz one tick
/// maps to exactly 25 engine frames, so `note_off` at tick 96 lands
/// on frame `2_400` and every compiled frame stays a whole number.
fn note_smf(key: u8, note_ticks: u64) -> Vec<u8> {
    let mut bytes = b"MThd".to_vec();
    bytes.extend_from_slice(&6u32.to_be_bytes());
    bytes.extend_from_slice(&0u16.to_be_bytes());
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&960u16.to_be_bytes());
    bytes.extend_from_slice(&mtrk(&[
        ev(0, &tempo(500_000)),
        ev(0, &note_on(0, key, 100)),
        ev(note_ticks, &note_off(0, key, 64)),
        ev(96, &eot()),
    ]));
    bytes
}

// ---------- fixture ----------

struct Fixture {
    app: SynvelisApp,
    engine: SessionRoutingEngine,
    instances: Option<InstanceLog>,
}

fn fixture(name: &str) -> Fixture {
    let root = scratch(name);
    let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".into()));
    let engine = SessionRoutingEngine::new(session_config(root.join("media")));
    Fixture {
        app,
        engine,
        instances: None,
    }
}

fn controller(f: &mut Fixture) -> &mut ProjectController {
    f.app.workspace_mut().session_mut().controller_mut()
}

/// Import `bytes` as a project MIDI source via the same op the
/// `syn.project.import_smf` service applies.
fn import_midi(f: &mut Fixture, bytes: &[u8]) -> SourceId {
    let document = parse_smf(bytes).unwrap();
    assert_eq!(document.format, SmfFormat::Format0);
    assert_eq!(document.division, 960);
    let c = controller(f);
    apply(c, ProjectOp::ImportMidiSmf { document });
    *c.live().midi_sources.keys().next_back().unwrap()
}

/// Track + strip + lane — the chain-head resolution path needs the
/// strip's chain bound to the clip's track.
fn add_track_strip(f: &mut Fixture) -> (TrackId, StripId, LaneId) {
    let c = controller(f);
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    (track, strip, lane)
}

fn add_lane(f: &mut Fixture, track: TrackId) -> LaneId {
    let c = controller(f);
    apply(c, ProjectOp::AddLane { track_id: track });
    *c.live().tracks[&track].lane_ids.last().unwrap()
}

/// One MIDI clip on `lane` at musical tick `start` spanning `ticks`
/// — the same musical placement `tim02_smf_app` uses (division ==
/// project PPQN keeps the 1:1 comp contract exact).
fn add_midi_clip(
    f: &mut Fixture,
    lane: LaneId,
    source: SourceId,
    start: u64,
    ticks: u64,
) -> ClipId {
    let c = controller(f);
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(i64::try_from(start).unwrap())),
                extent: ClipExtent::Musical(
                    MusicalDuration::new(i64::try_from(ticks).unwrap()).unwrap(),
                ),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(i64::try_from(ticks).unwrap()).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    *c.live().clips.keys().next_back().unwrap()
}

/// Insert the fake instrument at the strip's chain head — index 0
/// is the only position `instrument_node_for_clip` resolves.
fn insert_instrument(f: &mut Fixture, strip: StripId) -> ProcessorId {
    let c = controller(f);
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "instrument".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: INSTRUMENT_DESCRIPTOR.id.into(),
                version: "0.1.0".into(),
                capability_revision: 0,
            }),
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

/// Bind the instrument host + shared catalog cell before attach —
/// the same composition order `pc_c4_04` uses.
fn bind_instrument_host(f: &mut Fixture) {
    let (mut processors, instances) = instrument_host();
    processors.register_params(INSTRUMENT_DESCRIPTOR.id, INSTRUMENT_PARAMS.to_vec());
    let cell = processors.param_catalog_cell();
    f.engine.set_processor_host(Box::new(processors));
    f.engine.set_param_catalog(cell);
    f.instances = Some(instances);
}

/// Attach the engine and push a no-op revision so the notification
/// drives plan → prepare → publish → attach/sync → MIDI staging.
fn attach_and_rebuild(f: &mut Fixture) {
    f.app.attach_routing_engine(f.engine.clone());
    let rev = f.app.workspace().session().revision();
    f.app
        .workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "tim02del".into(),
            },
            rev,
        )
        .unwrap();
}

fn evidence(f: &Fixture) -> Vec<MidiClipEvidence> {
    f.engine.midi_clip_evidence()
}

// ===========================================================================
// clip→node delivery evidence (AC1)
// ===========================================================================

/// A compiled MIDI clip resolves to its track strip's chain-head
/// instrument and stages there — the plugin's `EventList` then
/// carries exactly the compiled events the run window reaches.
#[test]
fn compiled_clip_stages_on_chain_head_instrument() {
    let mut f = fixture("staged");
    bind_instrument_host(&mut f);
    let source = import_midi(&mut f, &note_smf(60, 96));
    let (_track, strip, lane) = add_track_strip(&mut f);
    let processor = insert_instrument(&mut f, strip);
    add_midi_clip(&mut f, lane, source, 0, 1_920);
    attach_and_rebuild(&mut f);

    let evidence = evidence(&f);
    assert_eq!(evidence.len(), 1, "{evidence:?}");
    let entry = &evidence[0];
    assert_eq!(entry.error, None);
    assert_eq!(
        entry.delivery,
        MidiClipDelivery::Staged(processor),
        "compiled clip must stage on the chain-head instrument"
    );

    // 12 blocks = 3_072 frames — covers the note_off at frame 2_400.
    f.engine.run(12, PumpPolicy::Every(1)).unwrap();
    let got = delivered(f.instances.as_ref().unwrap(), 0);
    let expected: Vec<Rec> = entry
        .events
        .iter()
        .filter(|(frame, _)| *frame < 3_072)
        .map(|(frame, event)| Rec {
            abs_frame: *frame,
            event: *event,
        })
        .collect();
    assert_eq!(
        got, expected,
        "the instrument's EventList must carry the staged events at their engine frames"
    );
    assert!(
        got.iter().any(|rec| matches!(
            rec.event.kind,
            MidiKind::NoteOn { key, .. } if key.get() == 60
        )),
        "the staged note must deliver: {got:?}"
    );
}

/// Two clips on two lanes of the same track resolve to the same
/// chain-head node — one `sync_midi_plans` call merges their staged
/// sets so neither clip's content is last-writer-wins'd away.
#[test]
fn sibling_clips_merge_into_one_staged_set() {
    let mut f = fixture("merge");
    bind_instrument_host(&mut f);
    let source_a = import_midi(&mut f, &note_smf(60, 96));
    let source_b = import_midi(&mut f, &note_smf(64, 96));
    let (track, strip, lane_a) = add_track_strip(&mut f);
    let processor = insert_instrument(&mut f, strip);
    let lane_b = add_lane(&mut f, track);
    add_midi_clip(&mut f, lane_a, source_a, 0, 1_920);
    add_midi_clip(&mut f, lane_b, source_b, 0, 1_920);
    attach_and_rebuild(&mut f);

    let evidence = evidence(&f);
    assert_eq!(evidence.len(), 2, "{evidence:?}");
    for entry in &evidence {
        assert_eq!(
            entry.delivery,
            MidiClipDelivery::Staged(processor),
            "both clips must stage on the shared chain-head node: {entry:?}"
        );
    }

    f.engine.run(12, PumpPolicy::Every(1)).unwrap();
    let got = delivered(f.instances.as_ref().unwrap(), 0);
    for key in [60u8, 64] {
        assert!(
            got.iter().any(|rec| matches!(
                rec.event.kind,
                MidiKind::NoteOn { key: k, .. } if k.get() == key
            )),
            "key {key} must deliver from the merged stage: {got:?}"
        );
    }
}

/// A MIDI clip whose lane's track has no strip/chain resolves to
/// `None` — the honest `Unresolved` record, never a silent skip.
#[test]
fn clip_without_instrument_reports_unresolved() {
    let mut f = fixture("unresolved");
    let source = import_midi(&mut f, &note_smf(60, 96));
    // Track + lane, deliberately no strip — `strip_of_track` is
    // `None` so the chain-head rule cannot resolve a node.
    let lane = {
        let c = controller(&mut f);
        apply(c, ProjectOp::AddTrack { name: "t".into() });
        let track = *c.live().tracks.keys().next_back().unwrap();
        apply(c, ProjectOp::AddLane { track_id: track });
        *c.live().tracks[&track].lane_ids.last().unwrap()
    };
    add_midi_clip(&mut f, lane, source, 0, 1_920);
    attach_and_rebuild(&mut f);

    let evidence = evidence(&f);
    assert_eq!(evidence.len(), 1, "{evidence:?}");
    assert_eq!(evidence[0].delivery, MidiClipDelivery::Unresolved);
}

/// Resolution succeeds but nothing can stage — no processor host
/// means no vended control node — and the refusal lands as typed
/// `Failed` evidence naming the processor.
#[test]
fn staging_without_control_surface_reports_failed() {
    let mut f = fixture("failed");
    let source = import_midi(&mut f, &note_smf(60, 96));
    let (_track, strip, lane) = add_track_strip(&mut f);
    let processor = insert_instrument(&mut f, strip);
    add_midi_clip(&mut f, lane, source, 0, 1_920);
    // Deliberately no `bind_instrument_host` — the engine vends no
    // control node for the resolved processor.
    attach_and_rebuild(&mut f);

    let evidence = evidence(&f);
    assert_eq!(evidence.len(), 1, "{evidence:?}");
    match &evidence[0].delivery {
        MidiClipDelivery::Failed {
            processor: failed, ..
        } => assert_eq!(*failed, processor),
        delivery => panic!("expected Failed delivery evidence, got {delivery:?}"),
    }
}
