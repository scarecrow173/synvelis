//! TIM-02 player MIDI RT/Offline integration tests — issue #126
//! (headless slice; spec `docs/specs/tim-02-player-midi-rt-offline.md`,
//! test spec `docs/test-specs/tim-02-player-midi-rt-fixtures.md`).
//!
//! Covers the player-side binding/playback slice:
//! `compile_midi_events` lowers a project `MidiPlaybackPlan` to
//! engine-frame `MidiEvent`s, `ParamDispatch::dispatch_midi` installs
//! them as a generation-tagged transport plan, and `HeadlessDriver`'s
//! transport surface (`set_loop`/`clear_loop`/`seek`/`panic`/
//! `resume_attached_transport`) drives Loop/Seek/Stop/Panic/Chase
//! with zero hanging voices — all through the real `PluginNode`
//! scheduler path, never a duplicated one.
//!
//! Headless fixtures only: no real OS MIDI backend, device, or plugin
//! acceptance is exercised — those stay `NotRun` (external gate).

use std::sync::{Arc, Mutex};

use ruviform_audio::render::{
    PreparedRender, RenderOutput, RenderSpec, RenderTransport, SilentFeeder, render,
};
use ruviform_audio::typed::{
    Compiler, CompilerConfig, DEFAULT_ALIGNMENT, FactoryError, GraphProcessor, GraphRuntime,
    InstalledInfo, NodeEvent, NodeIo, OfflineRenderSupport, PrepareOutcome, RtRuntime,
};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, Attached, PluginNode, PluginNodeFactory, attach};
use ruviform_exec::{CancellationToken, Progress};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, NodeSpec, PortId,
    PortRef, ValidatedGraph, Validator,
};
use ruviform_io_file::{Durability, PublishMode, PublishOptions, SealedFile, StagedFile};
use ruviform_io_wav::WavOutputSpec;
use ruviform_media_core::{
    BlockContract, ClockEpochId, DeliveryPolicy, EngineFrame, InBlockTransportSupport, MidiChannel,
    MidiController, MidiEvent, MidiGroup, MidiKind, MidiValue, NoteKey, ScheduleBudget,
    ScheduledEvent, TimeBase, TransportCommand, TransportScheduler,
};
use ruviform_media_io::Muxer;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError,
    ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::lowering::{EngineConfig, lower_with_inserts};
use synvelis_player::{
    AttachedNode, HeadlessDriver, ParamDispatch, PumpPolicy, compile_midi_events,
};
use synvelis_project::*;

// ---------------------------------------------------------------------
// Fixture plugin: records every delivered `EventKind::Midi` at its
// position-domain frame (`transport.position` at call start + the
// event's in-call offset — the same convention as `pc_b`'s recorder).
// The log Vec is pre-sized so `process` never allocates on the RT
// path once warm.
// ---------------------------------------------------------------------

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim02-midi",
    name: "TIM-02 MIDI Recorder",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const BUSES: [BusDescriptor; 2] = [
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

/// Pre-sized event log — far beyond any fixture's delivery count, so
/// `push` never reallocates inside a measured window.
const LOG_CAPACITY: usize = 8192;

/// One delivered MIDI event at its position-domain frame.
#[derive(Debug, Clone, PartialEq)]
struct Mid {
    abs_frame: i64,
    event: MidiEvent,
}

struct MidiRecPlugin {
    delivered: Arc<Mutex<Vec<Mid>>>,
}

impl PluginProcessor for MidiRecPlugin {
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
            if let EventKind::Midi(midi) = ev.kind() {
                log.push(Mid {
                    abs_frame: pos + i64::from(ev.frame_offset()),
                    event: midi,
                });
            }
        }
        ProcessResult::CONTINUE
    }
}

impl Plugin for MidiRecPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &[]
    }
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
    }
}

struct MidiFactory {
    delivered: Arc<Mutex<Vec<Mid>>>,
}

impl PluginFactory for MidiFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(MidiRecPlugin {
            delivered: Arc::clone(&self.delivered),
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(MidiRecPlugin {
            delivered: Arc::clone(&self.delivered),
        }))
    }
}

fn fmt() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

/// A hosted fixture plugin + its `attached` parts — the caller picks
/// the runtime (driver `RtRuntime`, raw playback `RtRuntime`, or
/// offline `PreparedRender`).
struct Hosted {
    host: PluginHost,
    attached: Attached,
    delivered: Arc<Mutex<Vec<Mid>>>,
}

fn host_plugin() -> Hosted {
    let delivered = Arc::new(Mutex::new(Vec::with_capacity(LOG_CAPACITY)));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(MidiFactory {
        delivered: Arc::clone(&delivered),
    }));
    let HostReply::Instantiated { handle } = host
        .execute(HostCommand::Instantiate {
            plugin_id: DESCRIPTOR.id.to_owned(),
        })
        .unwrap()
    else {
        panic!("Instantiate")
    };
    host.execute(HostCommand::Initialize { handle }).unwrap();
    let process_config = ProcessConfig::new(SampleRate::new(48_000).unwrap(), 1, 512).unwrap();
    host.execute(HostCommand::Activate {
        handle,
        config: process_config,
    })
    .unwrap();
    host.execute(HostCommand::StartProcessing { handle })
        .unwrap();
    let attached = attach(
        &mut host,
        handle,
        &AttachConfig {
            buses: BUSES.to_vec(),
            port_format: fmt(),
            bounds: EndpointBounds {
                max_frames: 512,
                channels: 2,
            },
            process_config,
            frame_base: frame_base(),
            policy: DeliveryPolicy::Exact,
            budget: ScheduleBudget::default(),
            input: None,
        },
    )
    .expect("attach");
    Hosted {
        host,
        attached,
        delivered,
    }
}

/// `Source → plugin → Sink` — shared graph shape for the raw
/// `RtRuntime` and offline `render()` drives.
fn build_graph(attached: &Attached) -> (ValidatedGraph, NodeId, [PortId; 2], [PortId; 2]) {
    let audio = |dir| ruviform_graph::PortDef {
        direction: dir,
        role: ruviform_graph::PortRole::Main,
        format: ruviform_graph::PortFormat::Audio(fmt()),
        multiplicity: ruviform_graph::Multiplicity::unbounded(0),
        capabilities: Vec::new(),
        availability: ruviform_graph::Availability::Always,
        activation: ruviform_graph::Activation::Enabled,
    };
    let mut b = GraphBuilder::new();
    let (src, sp) = b.add_node(&NodeDef {
        kind: NodeKind::Source,
        ports: vec![audio(Direction::Output)],
    });
    let (proc, pp) = b.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: attached.ports.clone(),
    });
    let (sink, kp) = b.add_node(&NodeDef {
        kind: NodeKind::Sink,
        ports: vec![audio(Direction::Input)],
    });
    let conn = |from, to| ConnDef {
        from,
        to,
        delay: EdgeDelay::Zero,
        channel_map: None,
    };
    let pref = |node, port| PortRef { node, port };
    b.connect(&conn(pref(src, sp[0]), pref(proc, pp[0])));
    b.connect(&conn(pref(proc, pp[1]), pref(sink, kp[0])));
    let g = Validator::validate(&b.build()).expect("graph validates");
    (g, proc, [sp[0], sp[0]], [kp[0], kp[0]])
}

fn compiler_config() -> CompilerConfig {
    CompilerConfig {
        max_frames: 512,
        alignment: DEFAULT_ALIGNMENT,
        max_slots: 64,
        max_channels: 8,
        max_delay_lines: 16,
        max_delay_frames: 4096,
        max_processors: 16,
    }
}

/// Test-scoped `OfflineRenderSupport::Verified` declaration for the
/// fixture node — same pattern as `pc_b_04`: the recorder is
/// stateless apart from the `delivered` log and transport-
/// deterministic, so sequential-offline equivalence is verified by
/// the parity assertion itself.
struct VerifiedOfflineNode(PluginNode);

impl GraphProcessor for VerifiedOfflineNode {
    fn latency(&self) -> u32 {
        self.0.latency()
    }

    fn process(&mut self, io: &mut NodeIo<'_>, ctx: &RtContext) {
        self.0.process(io, ctx);
    }

    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }
}

type DynMuxerCtor = Box<
    dyn for<'a> FnOnce(&'a mut StagedFile) -> Result<Box<dyn Muxer + 'a>, ruviform_media_io::Error>
        + Send,
>;

/// Playback-style block drive over a compiled graph — feeds silence
/// into the source and steps `rt.process`.
struct Drive {
    rt: RtRuntime,
    ctx: RtContext,
    src: PortId,
    zeros: Vec<f32>,
}

impl Drive {
    fn new(graph: &ValidatedGraph, node: PluginNode, proc: NodeId, srcs: [PortId; 2]) -> Self {
        let mut factory = PluginNodeFactory::new();
        factory.insert(proc, node);
        let (mut runtime, rt) = GraphRuntime::new(&compiler_config(), 8, 8);
        let compiled = Compiler::new(compiler_config())
            .compile(
                graph,
                &mut factory,
                runtime.registry(),
                runtime.allocator(),
                runtime.generation() + 1,
            )
            .expect("compiles");
        runtime.publish(compiled).expect("publish");
        Self {
            rt,
            ctx: RtContext::mint(),
            src: srcs[0],
            zeros: vec![0.0; 512],
        }
    }

    fn process(&mut self, frames: usize) {
        let mut slot = self.rt.input_writer(self.src, frames).expect("input bound");
        for ch in 0..2 {
            slot.data.channel_mut(ch).unwrap()[..frames].copy_from_slice(&self.zeros[..frames]);
        }
        drop(slot);
        self.rt.process(frames, &self.ctx).expect("process");
    }
}

// ---------------------------------------------------------------------
// Expected-event builders (wire domain).
// ---------------------------------------------------------------------

fn midi(ch: u8, kind: MidiKind) -> MidiEvent {
    MidiEvent {
        group: MidiGroup::ZERO,
        channel: MidiChannel::new(ch).unwrap(),
        kind,
    }
}

fn note_on(ch: u8, key: u8, vel: u8) -> MidiEvent {
    midi(
        ch,
        MidiKind::NoteOn {
            key: NoteKey::new(key).unwrap(),
            velocity: MidiValue::from_u7(vel),
        },
    )
}

fn note_off(ch: u8, key: u8) -> MidiEvent {
    midi(
        ch,
        MidiKind::NoteOff {
            key: NoteKey::new(key).unwrap(),
            velocity: 0,
        },
    )
}

fn cc(ch: u8, controller: u8, value: u8) -> MidiEvent {
    midi(
        ch,
        MidiKind::ControlChange {
            controller: MidiController::new(controller).unwrap(),
            value: MidiValue::from_u7(value),
        },
    )
}

fn bend(ch: u8, value: u16) -> MidiEvent {
    midi(
        ch,
        MidiKind::PitchBend {
            value: MidiValue::bend_from_u14(value),
        },
    )
}

fn mid(frame: i64, event: MidiEvent) -> Mid {
    Mid {
        abs_frame: frame,
        event,
    }
}

// ---------------------------------------------------------------------
// Project fixture helpers.
// ---------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "tim-02-player-midi"),
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

fn musical(tick: i64) -> MusicalTick {
    MusicalTick::new(tick)
}

/// Hand-built (non-SMF) MIDI source — no `smf_documents` entry, so
/// note channels resolve to the binding's `fallback_channel`.
fn add_source(c: &mut ProjectController, division: u32) -> SourceId {
    apply(
        c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(division).unwrap(),
        },
    );
    *c.live().midi_sources.keys().next().unwrap()
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

fn region(id: u128, source: SourceId, phrase: (i64, i64), proj: (i64, i64)) -> MidiPhraseRegion {
    MidiPhraseRegion {
        id: MidiCompRegionId::new(id),
        source_id: source,
        phrase: MidiRange::try_new(mt(phrase.0), mt(phrase.1)).unwrap(),
        project_range: MusicalRange::try_new(musical(proj.0), musical(proj.1)).unwrap(),
    }
}

fn lanes_for(source: SourceId, events: Vec<MidiControllerEvent>) -> MidiControllerLanes {
    MidiControllerLanes {
        lanes: vec![MidiControllerLane {
            source_id: source,
            events,
        }],
    }
}

fn ctrl(tick: i64, channel: u8, kind: MidiControllerKind, value: i32) -> MidiControllerEvent {
    MidiControllerEvent {
        tick: mt(tick),
        channel,
        kind,
        value,
    }
}

/// `compile_midi_comp` + `compile_midi_events` in one step — the same
/// chain a player binding performs (plan oracle is project-owned;
/// wire lowering is player-owned).
fn compile_events(
    c: &ProjectController,
    selection: &MidiCompSelection,
    lanes: &MidiControllerLanes,
    fallback_channel: u8,
) -> Vec<(i64, MidiEvent)> {
    let plan = compile_midi_comp(c.live(), selection, lanes).unwrap();
    let mapping = TickTimeMapping::new(&c.live().tempo_map).unwrap();
    compile_midi_events(c.live(), &plan, &mapping, frame_base(), fallback_channel).unwrap()
}

// ---------------------------------------------------------------------
// SMF wire builders (format-1 provenance fixture).
// ---------------------------------------------------------------------

fn smf_vlq(mut v: u64) -> Vec<u8> {
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

fn smf_ev(delta: u64, body: &[u8]) -> Vec<u8> {
    let mut v = smf_vlq(delta);
    v.extend_from_slice(body);
    v
}

fn smf_meta(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0xFF, kind];
    v.extend(smf_vlq(payload.len() as u64));
    v.extend_from_slice(payload);
    v
}

fn smf_tempo(us: u32) -> Vec<u8> {
    smf_meta(0x51, &us.to_be_bytes()[1..])
}

fn smf_eot() -> Vec<u8> {
    vec![0xFF, 0x2F, 0x00]
}

fn smf_note_on(ch: u8, key: u8, vel: u8) -> Vec<u8> {
    vec![0x90 | ch, key, vel]
}

fn smf_note_off(ch: u8, key: u8, vel: u8) -> Vec<u8> {
    vec![0x80 | ch, key, vel]
}

fn smf_cc(ch: u8, controller: u8, value: u8) -> Vec<u8> {
    vec![0xB0 | ch, controller, value]
}

fn smf_mtrk(events: &[Vec<u8>]) -> Vec<u8> {
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

/// Format 1, division 480: conductor + ch0 note/CC track + ch1
/// note/CC track — the channel-provenance fixture.
fn smf_two_channel_file() -> Vec<u8> {
    smf_file(
        1,
        480,
        &[
            smf_mtrk(&[smf_ev(0, &smf_tempo(500_000)), smf_ev(0, &smf_eot())]),
            smf_mtrk(&[
                smf_ev(0, &smf_cc(0, 64, 127)),
                smf_ev(0, &smf_note_on(0, 60, 100)),
                smf_ev(480, &smf_note_off(0, 60, 64)),
                smf_ev(0, &smf_eot()),
            ]),
            smf_mtrk(&[
                smf_ev(0, &smf_cc(1, 64, 64)),
                smf_ev(120, &smf_note_on(1, 64, 90)),
                smf_ev(480, &smf_note_off(1, 64, 40)),
                smf_ev(0, &smf_eot()),
            ]),
        ],
    )
}

// ---------------------------------------------------------------------
// Driver fixture: HeadlessDriver + one attached MIDI node on the
// lowered insert slot (the instrument position).
// ---------------------------------------------------------------------

fn engine() -> EngineConfig {
    EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 128,
    }
}

/// Activates a driver whose single insert slot carries the fixture
/// plugin. Returns `(driver, insert node id, delivered log, host
/// guard)` — the host must stay alive for the node's endpoint.
fn driver_fixture(
    c: &ProjectController,
) -> (HeadlessDriver, NodeId, Arc<Mutex<Vec<Mid>>>, PluginHost) {
    let hosted = host_plugin();
    let delivered = Arc::clone(&hosted.delivered);
    let lowered = lower_with_inserts(
        c.live(),
        &|_: SourceId| -> Option<AudioFormat> { None },
        &engine(),
        1,
    );
    let insert = lowered.inserts[0].node_id;
    let mut driver = HeadlessDriver::new(engine());
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, insert, frame_base())
                    .unwrap(),
                boundary: None,
            }],
        )
        .expect("activate");
    (driver, insert, delivered, hosted.host)
}

/// Dispatches compiled MIDI events onto the driver's attached node.
fn dispatch_midi(driver: &mut HeadlessDriver, insert: NodeId, events: &[(i64, MidiEvent)]) {
    driver
        .attached_dispatch(insert)
        .expect("dispatch bound")
        .dispatch_midi(events)
        .expect("dispatch_midi");
}

/// Project tick → engine frame under the default map (120 BPM,
/// PPQN 960, 48 kHz frame base): 960 ticks/beat × 0.5 s/beat →
/// 25 frames per project tick.
const FRAMES_PER_PROJECT_TICK: i64 = 25;

fn ptick(tick: i64) -> i64 {
    tick * FRAMES_PER_PROJECT_TICK
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// TIM-02 lowering: the plan's total order becomes engine-frame
/// `MidiEvent`s — note on/off pairs, terminal `AllNotesOff` → CC123
/// on every channel that sounded, `group 0`, fallback channel for a
/// non-SMF source.
#[test]
fn tim02_compile_lowers_plan_to_wire_events() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    // n1: src [120,360) → project [240,720) → frames [6000,18000)
    add_note(&mut c, source, 60, 100, 120, 240);
    // n2: src [400,480) → project [800,960) → frames [20000,24000)
    add_note(&mut c, source, 64, 90, 400, 80);

    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 480), (0, 960))],
    };
    let events = compile_events(&c, &selection, &MidiControllerLanes::default(), 0);

    // 2 ons + 2 offs + terminal CC123 — no chase events with empty lanes.
    assert_eq!(events.len(), 5, "{events:?}");
    let frames: Vec<i64> = events.iter().map(|(f, _)| *f).collect();
    assert_eq!(frames, vec![6_000, 18_000, 20_000, 24_000, 24_000]);
    assert!(frames.windows(2).all(|w| w[0] <= w[1]), "frames monotone");

    assert_eq!(events[0].1, note_on(0, 60, 100));
    assert_eq!(events[1].1, note_off(0, 60));
    assert_eq!(events[2].1, note_on(0, 64, 90));
    // Same frame: the note-off (rank 0) precedes the terminal
    // AllNotesOff expansion (rank 1).
    assert_eq!(events[3].1, note_off(0, 64));
    assert_eq!(events[4].1, cc(0, 123, 0));
}

/// TIM-02 basic playback: events compiled from the plan land on the
/// endpoint at their absolute engine frames through the driver.
#[test]
fn tim02_driver_delivers_midi_at_absolute_frames() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    add_note(&mut c, source, 60, 100, 120, 240);
    add_note(&mut c, source, 64, 90, 400, 80);
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 480), (0, 960))],
    };
    let events = compile_events(&c, &selection, &MidiControllerLanes::default(), 0);

    let (mut driver, insert, delivered, _host) = driver_fixture(&c);
    dispatch_midi(&mut driver, insert, &events);
    driver.run(210, PumpPolicy::Every(1)).expect("run");

    let expected = vec![
        mid(6_000, note_on(0, 60, 100)),
        mid(18_000, note_off(0, 60)),
        mid(20_000, note_on(0, 64, 90)),
        mid(24_000, note_off(0, 64)),
        mid(24_000, cc(0, 123, 0)),
    ];
    assert_eq!(*delivered.lock().unwrap(), expected);
}

/// TIM-02 native tick fidelity: source-native ticks map through the
/// project PPQN grid exactly — division 480 → PPQN 960 is ×2, and
/// the delivered frame equals `tick_to_media` for every event.
#[test]
fn tim02_native_tick_maps_to_engine_frames() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    // Odd source ticks: on@61 len119 → end 180. project ticks are
    // exactly ×2 (122 / 360) → frames 3050 / 9000 at 25 f/tick.
    add_note(&mut c, source, 60, 100, 61, 119);
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 480), (0, 960))],
    };
    let events = compile_events(&c, &selection, &MidiControllerLanes::default(), 3);
    assert_eq!(events.len(), 3, "{events:?}"); // on, off, CC123 — fallback ch 3
    assert_eq!(events[0].0, ptick(122));
    assert_eq!(events[1].0, ptick(360));
    assert_eq!(events[2].0, ptick(960));
    for (_, event) in &events {
        assert_eq!(event.channel.get(), 3, "fallback channel");
        assert_eq!(event.group, MidiGroup::ZERO);
    }

    // Cross-check the frames against the authoritative mapping —
    // the same call `compile_midi_events` performs internally.
    let mapping = TickTimeMapping::new(&c.live().tempo_map).unwrap();
    let plan = compile_midi_comp(c.live(), &selection, &MidiControllerLanes::default()).unwrap();
    for ((frame, _), plan_event) in events.iter().zip(plan.events.iter()) {
        // AllNotesOff expands to one wire event per sounded channel —
        // one channel here, so the zip stays aligned.
        let expected = mapping
            .tick_to_media(plan_event.at, 1, 48_000)
            .unwrap()
            .value();
        assert_eq!(*frame, expected, "frame must equal tick_to_media");
    }
}

/// TIM-02 deterministic same-position ordering: a note-off and two
/// note-ons sharing one project tick deliver off → on(n2) → on(n3)
/// in plan order (action rank + `NoteId` order).
#[test]
fn tim02_same_tick_off_precedes_ons() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    // n1 ends at src 200; n2/n3 start at src 200 — same project tick.
    add_note(&mut c, source, 60, 100, 100, 100);
    add_note(&mut c, source, 62, 90, 200, 100);
    add_note(&mut c, source, 64, 80, 200, 50);
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 400), (0, 800))],
    };
    let events = compile_events(&c, &selection, &MidiControllerLanes::default(), 0);

    let (mut driver, insert, delivered, _host) = driver_fixture(&c);
    dispatch_midi(&mut driver, insert, &events);
    driver.run(170, PumpPolicy::Every(1)).expect("run"); // 21760 > 20000

    let got = delivered.lock().unwrap().clone();
    // Boundary tick: src 200 → project 400 → frame 10_000.
    let at_boundary: Vec<MidiEvent> = got
        .iter()
        .filter(|m| m.abs_frame == 10_000)
        .map(|m| m.event)
        .collect();
    assert_eq!(
        at_boundary,
        vec![note_off(0, 60), note_on(0, 62, 90), note_on(0, 64, 80)],
        "same-tick order must be off → ons in plan order"
    );
    // Every on has a later off — zero hanging voices at plan end.
    assert!(got.iter().any(|m| m.event == cc(0, 123, 0)));
}

/// TIM-02 controller chase: the region-start chase snapshot covers
/// the declared `(channel, kind)` union channel-aware — ch0 CC64=127
/// and ch1 CC64=64 are distinct states — and in-phrase controller
/// events fire at their mapped frames.
#[test]
fn tim02_controller_chase_is_channel_aware() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    add_note(&mut c, source, 60, 100, 100, 200);
    let lanes = lanes_for(
        source,
        vec![
            ctrl(0, 0, MidiControllerKind::Cc(64), 127),
            ctrl(0, 1, MidiControllerKind::Cc(64), 64),
            ctrl(0, 0, MidiControllerKind::PitchBend, 8_000),
            ctrl(200, 0, MidiControllerKind::Cc(64), 20),
            ctrl(200, 1, MidiControllerKind::Cc(10), 99),
        ],
    );
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 400), (0, 800))],
    };

    let (mut driver, insert, delivered, _host) = driver_fixture(&c);
    let events = compile_events(&c, &selection, &lanes, 0);
    dispatch_midi(&mut driver, insert, &events);
    driver.run(170, PumpPolicy::Every(1)).expect("run"); // 21760 > 20000

    let expected = vec![
        // Region start (proj 0 → frame 0): chase covers the union
        // {(0,CC64),(0,PB),(1,CC10),(1,CC64)} in (channel, kind) order;
        // (1,CC10) undeclared at tick ≤ 0 → default 0.
        mid(0, cc(0, 64, 127)),
        mid(0, bend(0, 8_000)),
        mid(0, cc(1, 10, 0)),
        mid(0, cc(1, 64, 64)),
        // Note: src [100,300) → proj [200,600) → frames [5000,15000).
        mid(5_000, note_on(0, 60, 100)),
        // In-phrase controller events at src 200 → proj 400 → f10000.
        mid(10_000, cc(0, 64, 20)),
        mid(10_000, cc(1, 10, 99)),
        mid(15_000, note_off(0, 60)),
        // Terminal AllNotesOff — only ch0 sounded a note.
        mid(20_000, cc(0, 123, 0)),
    ];
    assert_eq!(*delivered.lock().unwrap(), expected);
}

/// TIM-02 Loop: `set_loop` wraps the node's position domain — events
/// inside the loop re-fire once per occurrence, the wrap discontinuity
/// releases every sounding note and chases the still-open prefix, and
/// events outside the loop fire exactly once after `clear_loop`.
#[test]
fn tim02_loop_replay_releases_and_refires() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    // Source ticks → frames = src × 50 (480→960 ×2, ×25 f/tick).
    // N1 key60 src[200,400) → on@10000 off@20000 — inside loop.
    add_note(&mut c, source, 60, 100, 200, 200);
    // N2 key62 src[100,700) → on@5000 (pre-loop) off@35000 (post-loop)
    // — spans the whole loop region, chased at every wrap.
    add_note(&mut c, source, 62, 90, 100, 600);
    // N3 key64 src[550,600) → on@27500 off@30000 — beyond loop end.
    add_note(&mut c, source, 64, 80, 550, 50);
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 960), (0, 1920))],
    };
    let events = compile_events(&c, &selection, &MidiControllerLanes::default(), 0);

    let (mut driver, insert, delivered, _host) = driver_fixture(&c);
    driver.set_loop(8_192, 24_576).expect("set_loop");
    dispatch_midi(&mut driver, insert, &events);

    // Phase A: first linear pass 0→24576 then wrap1 at cursor 24576
    // (block 192). run(200) → cursor 25600, position ≈ 9216.
    driver.run(200, PumpPolicy::Every(1)).expect("run a");
    // Phase B: lap1 completes and wrap2 lands at cursor 40960
    // (block 320). run(125) → cursor 41600, position ≈ 8832.
    driver.run(125, PumpPolicy::Every(1)).expect("run b");
    // Phase C: clear the loop — position continues linearly from
    // ≈8832 through the former loop region and beyond.
    driver.clear_loop().expect("clear_loop");
    driver.run(310, PumpPolicy::Every(1)).expect("run c"); // → pos ≈48512

    let expected = vec![
        // First pass (position 0→24576): pre-loop on, in-loop pair.
        mid(5_000, note_on(0, 62, 90)),
        mid(10_000, note_on(0, 60, 100)),
        mid(20_000, note_off(0, 60)),
        // Wrap1 @pos 24576→8192: release sounding {62}, chase {62}.
        mid(8_192, note_off(0, 62)),
        mid(8_192, note_on(0, 62, 90)),
        // Lap1: in-loop events re-fire at their in-lap positions.
        mid(10_000, note_on(0, 60, 100)),
        mid(20_000, note_off(0, 60)),
        // Wrap2: same release+chase.
        mid(8_192, note_off(0, 62)),
        mid(8_192, note_on(0, 62, 90)),
        // Post-clear linear pass from ≈8832: remaining events once.
        mid(10_000, note_on(0, 60, 100)),
        mid(20_000, note_off(0, 60)),
        mid(27_500, note_on(0, 64, 80)),
        mid(30_000, note_off(0, 64)),
        mid(35_000, note_off(0, 62)),
        mid(48_000, cc(0, 123, 0)),
    ];
    assert_eq!(*delivered.lock().unwrap(), expected);
}

/// TIM-02 Seek: landing mid-note releases the ledger, chases the key
/// still sounding at the target, skips events in the jumped interval,
/// and delivers the remaining plan normally.
#[test]
fn tim02_seek_releases_and_chases() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    // N1 key60 src[100,500) → on@5000 off@25000.
    add_note(&mut c, source, 60, 100, 100, 400);
    // N2 key62 src[600,700) → on@30000 off@35000.
    add_note(&mut c, source, 62, 90, 600, 100);
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 960), (0, 1920))],
    };
    let events = compile_events(&c, &selection, &MidiControllerLanes::default(), 0);

    let (mut driver, insert, delivered, _host) = driver_fixture(&c);
    dispatch_midi(&mut driver, insert, &events);
    driver.run(60, PumpPolicy::Every(1)).expect("run pre"); // cursor 7680

    // Seek to engine frame 20000 — inside N1's span, before N2.
    driver.seek(20_000).expect("seek");
    driver.run(230, PumpPolicy::Every(1)).expect("run post"); // → 49440

    let expected = vec![
        mid(5_000, note_on(0, 60, 100)),
        // Seek discontinuity at position 20000: release then chase.
        mid(20_000, note_off(0, 60)),
        mid(20_000, note_on(0, 60, 100)),
        mid(25_000, note_off(0, 60)),
        mid(30_000, note_on(0, 62, 90)),
        mid(35_000, note_off(0, 62)),
        mid(48_000, cc(0, 123, 0)),
    ];
    assert_eq!(*delivered.lock().unwrap(), expected);
}

/// TIM-02 Panic: `panic()` forwards `Stop` — the node releases every
/// ledger-tracked note at the next block and suppresses plan/chase
/// delivery while stopped (zero hanging voices). `resume_attached_
/// transport` re-anchors to the driver cursor and `Play` reconstructs
/// sounding notes from the plan prefix.
#[test]
fn tim02_panic_releases_hanging_voice_and_resume() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    // Long note: on@5000, off@45000 — still sounding at panic time.
    add_note(&mut c, source, 60, 100, 100, 800);
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 960), (0, 1920))],
    };
    let events = compile_events(&c, &selection, &MidiControllerLanes::default(), 0);

    let (mut driver, insert, delivered, _host) = driver_fixture(&c);
    dispatch_midi(&mut driver, insert, &events);
    driver.run(70, PumpPolicy::Every(1)).expect("run pre"); // cursor 8960

    driver.panic().expect("panic");
    driver.run(4, PumpPolicy::Every(1)).expect("run panic"); // cursor → 9472

    {
        let got = delivered.lock().unwrap();
        // The Stop discontinuity reports position 0 (Stopped anchors
        // at 0) — the release lands at offset 0 of that call.
        assert_eq!(
            got.as_slice(),
            &[mid(5_000, note_on(0, 60, 100)), mid(0, note_off(0, 60))],
            "panic must release the sounding voice — and nothing else"
        );
    }

    // Resume re-anchors at the driver cursor (9472): the plan prefix
    // still has key60 open → chase reconstructs it at the cursor,
    // then the scheduled off@45000 and terminal CC123 land.
    driver.resume_attached_transport().expect("resume");
    driver.run(320, PumpPolicy::Every(1)).expect("run post"); // → 50944

    let expected = vec![
        mid(5_000, note_on(0, 60, 100)),
        mid(0, note_off(0, 60)), // panic release (stopped position domain)
        mid(9_472, note_on(0, 60, 100)), // resume chase at cursor
        mid(45_000, note_off(0, 60)),
        mid(48_000, cc(0, 123, 0)),
    ];
    assert_eq!(*delivered.lock().unwrap(), expected);
}

/// TIM-02 SMF channel provenance: notes imported via `ImportMidiSmf`
/// recover their wire channel from `SmfDocument::paired_notes()` —
/// ch0 and ch1 notes stay distinct under a non-matching fallback.
#[test]
fn tim02_smf_channel_provenance() {
    let mut c = controller();
    let doc = parse_smf(&smf_two_channel_file()).unwrap();
    apply(
        &mut c,
        ProjectOp::ImportMidiSmf {
            document: doc.clone(),
        },
    );
    let source = *c.live().smf_documents.keys().next().unwrap();

    // Phrase [0,960) src ticks covers both onsets (0 and 120);
    // project range [0,1920) → frames [0,48000).
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 960), (0, 1920))],
    };
    // Controller lanes come from the document's own CC/PB events.
    let lanes = lanes_for(source, doc.controller_events());
    // fallback 7 must NOT appear — provenance supplies 0 and 1.
    let events = compile_events(&c, &selection, &lanes, 7);

    let expected = vec![
        // Chase at proj 0 (frame 0): CC64 per channel, (channel,kind) order.
        (0, cc(0, 64, 127)),
        (0, cc(1, 64, 64)),
        // ch0 note on@0 → proj 0 → frame 0 (rank 3, after chases).
        (0, note_on(0, 60, 100)),
        // ch1 note on@120 → proj 240 → frame 6000.
        (6_000, note_on(1, 64, 90)),
        // ch0 off: end 480 → proj 960 → frame 24000.
        (24_000, note_off(0, 60)),
        // ch1 off: end 600 → proj 1200 → frame 30000.
        (30_000, note_off(1, 64)),
        // Terminal AllNotesOff on both sounded channels.
        (48_000, cc(0, 123, 0)),
        (48_000, cc(1, 123, 0)),
    ];
    let got: Vec<(i64, MidiEvent)> = events.clone();
    assert_eq!(got, expected, "provenance channels must be 0 and 1");

    // End-to-end: the same stream delivers through the driver.
    let (mut driver, insert, delivered, _host) = driver_fixture(&c);
    dispatch_midi(&mut driver, insert, &events);
    driver.run(390, PumpPolicy::Every(1)).expect("run"); // → 49920

    let delivered_events: Vec<(i64, MidiEvent)> = delivered
        .lock()
        .unwrap()
        .iter()
        .map(|m| (m.abs_frame, m.event))
        .collect();
    assert_eq!(delivered_events, expected);
}

/// `Play` through the dispatch's own control handle — the documented
/// standalone-consumer path (`dispatch_midi` deliberately does not
/// request Play; transport anchoring stays with the driver).
fn control_play(dispatch: &mut ParamDispatch) {
    dispatch
        .control()
        .request(TransportCommand::Play, 0)
        .unwrap();
}

/// TIM-02 RT/Offline parity: the same compiled+dispatched MIDI
/// stream delivers identically under a playback-style `RtRuntime`
/// drive and the offline `render()` pipeline — absolute-frame
/// comparison, call partitioning may differ (same convention as
/// `pc_b_04`).
#[test]
#[expect(clippy::too_many_lines)]
fn tim02_rt_offline_parity() {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    add_note(&mut c, source, 60, 100, 120, 240);
    add_note(&mut c, source, 64, 90, 400, 80);
    let selection = MidiCompSelection {
        regions: vec![region(1, source, (0, 480), (0, 960))],
    };
    let events = compile_events(&c, &selection, &MidiControllerLanes::default(), 0);
    let range_end: u64 = 24_576; // past the terminal CC123 @24000

    // ---- playback-style drive --------------------------------------
    let hosted_a = host_plugin();
    let (graph_a, proc_a, srcs_a, _s) = build_graph(&hosted_a.attached);
    let node_a = hosted_a.attached.node;
    let control_a = hosted_a.attached.control;
    let mut drive = Drive::new(&graph_a, node_a, proc_a, srcs_a);
    let mut dispatch_a = ParamDispatch::new(control_a, proc_a, frame_base()).unwrap();
    control_play(&mut dispatch_a);
    dispatch_a.dispatch_midi(&events).unwrap();
    // 96 × 256 = 24576 = range_end — covers the terminal CC123 @24000.
    for _ in 0..96 {
        drive.process(256);
    }
    let playback_events = hosted_a.delivered.lock().unwrap().clone();

    // ---- offline render drive --------------------------------------
    let hosted_b = host_plugin();
    assert_eq!(
        hosted_b.attached.node.offline_support(),
        OfflineRenderSupport::Unknown
    );
    let (graph_b, proc_b, _sb, sinks_b) = build_graph(&hosted_b.attached);
    let node_b = hosted_b.attached.node;
    let control_b = hosted_b.attached.control;
    let mut wrapped = Some(VerifiedOfflineNode(node_b));
    let mut factory_b = move |spec: &NodeSpec,
                              _existing: Option<&InstalledInfo>|
          -> Result<PrepareOutcome, FactoryError> {
        if spec.kind() == NodeKind::Processor {
            assert_eq!(spec.id(), proc_b);
            Ok(PrepareOutcome::New(Box::new(
                wrapped.take().expect("one processor node"),
            )))
        } else {
            Ok(PrepareOutcome::Boundary)
        }
    };
    let mut prepared = PreparedRender::compile(&graph_b, &mut factory_b, compiler_config(), 8, 8)
        .expect("prepared render");
    let mut dispatch_b = ParamDispatch::new(control_b, proc_b, frame_base()).unwrap();
    control_play(&mut dispatch_b);
    dispatch_b.dispatch_midi(&events).unwrap();

    let base = frame_base();
    let mut transport = RenderTransport::<u32> {
        scheduler: TransportScheduler::new(
            base,
            BlockContract::new(1, 512, InBlockTransportSupport::Unknown),
            DeliveryPolicy::Exact,
            ScheduleBudget::default(),
        ),
        commands: vec![ruviform_audio::render::StagedCommand {
            command: TransportCommand::Play,
            due: EngineFrame::new(ClockEpochId::new(0), 0),
        }],
        plans: Vec::new(),
    };
    let spec = RenderSpec {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        output_port: sinks_b[0],
        range_start: 0,
        range_end,
        tail: ruviform_audio::render::TailSpec::None,
        job_start: 0,
        job_end: range_end,
        callback_chunk: 64,
    };
    let dest =
        std::env::temp_dir().join(format!("synvelis-tim02-parity-{}.wav", std::process::id()));
    let staged = StagedFile::create(&dest).expect("staged output");
    let info = WavOutputSpec {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
    .stream_info();
    let encoder = ruviform_io_wav::make_encoder(&info).expect("encoder");
    let muxer_info = info.clone();
    let make_muxer: DynMuxerCtor = Box::new(move |staged| {
        ruviform_io_wav::open_muxer(staged, std::slice::from_ref(&muxer_info))
            .map(|m| Box::new(m) as Box<dyn Muxer>)
    });
    let output = RenderOutput {
        staged,
        make_muxer,
        encoder: Box::new(encoder),
        publish_options: PublishOptions::new(PublishMode::Replace, Durability::BestEffort),
        verify: None::<
            Box<
                dyn FnOnce(
                    &mut SealedFile,
                    &ruviform_audio::render::OutputFacts,
                ) -> Result<(), ruviform_media_io::Error>,
            >,
        >,
        publish: Box::new(SealedFile::publish),
    };
    let mut feeder = SilentFeeder;
    let progress = Progress::new();
    let cancel = CancellationToken::new();
    let mut no_events = |_: &ScheduledEvent<u32>, _: u64| -> Option<NodeEvent> { None };
    render(
        &mut prepared,
        &mut transport,
        &spec,
        &mut feeder,
        output,
        &progress,
        &cancel,
        &mut no_events,
    )
    .expect("render");
    let render_events = hosted_b.delivered.lock().unwrap().clone();

    assert_eq!(
        playback_events, render_events,
        "playback drive and offline render deliver the same MIDI stream"
    );
    let _ = std::fs::remove_file(&dest);
}
