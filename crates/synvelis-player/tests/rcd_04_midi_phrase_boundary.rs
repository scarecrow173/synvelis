//! RCD-04 MIDI phrase boundary — engine-path fixture (synvelis#118).
//!
//! The project-side oracle (`compile_midi_comp`,
//! `synvelis_project::midi_comp`) already pins plan-level boundary
//! safety: virtual offs, chase snapshots, total order. This fixture
//! pins the *same* two-region selection through the only engine path
//! MIDI has — `compile_midi_events` → `ParamDispatch::dispatch_midi`
//! → `HeadlessDriver`/`PluginNode` delivery — so a drift between the
//! plan contract and the wire/driver path cannot hide.
//!
//! There is no semantic/render MIDI path to parity-check against:
//! `lower_semantic`/`RoutingEngine`/`export` carry no MIDI events
//! (audio comp parity lives in `rcd_04_rt_comp_pcm.rs`). What is
//! verified here is the boundary behavior on the real delivery path:
//! at the shared region boundary the wire order is
//! `NoteOff (virtual) → ControllerChase → NoteOn`, a note cut in
//! region A and re-articulated in region B on the same key never
//! double-sounds, every note-on is paired with exactly one note-off,
//! and the terminal panic reaches only channels that sounded.
//!
//! Headless fixtures only: no real OS MIDI backend, device, or plugin
//! acceptance is exercised — those stay `NotRun` (external gate).

use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, Attached, attach};
use ruviform_graph::NodeId;
use ruviform_media_core::{
    DeliveryPolicy, MidiChannel, MidiController, MidiEvent, MidiGroup, MidiKind, MidiValue,
    NoteKey, ScheduleBudget, TimeBase,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError,
    ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use synvelis_player::lowering::{EngineConfig, lower_with_inserts};
use synvelis_player::{
    AttachedNode, HeadlessDriver, ParamDispatch, PumpPolicy, compile_midi_events,
};
use synvelis_project::*;

// ---------------------------------------------------------------------
// Fixture plugin: records every delivered `EventKind::Midi` at its
// position-domain frame (same convention as `tim02_midi_playback`).
// ---------------------------------------------------------------------

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.rcd04-midi-boundary",
    name: "RCD-04 MIDI Boundary Recorder",
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
// Project fixture helpers (tim02 conventions).
// ---------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "rcd-04-midi-boundary"),
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

fn ctrl(tick: i64, channel: u8, kind: MidiControllerKind, value: i32) -> MidiControllerEvent {
    MidiControllerEvent {
        tick: mt(tick),
        channel,
        kind,
        value,
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

/// `compile_midi_comp` + `compile_midi_events` — the same chain a
/// player binding performs (plan oracle is project-owned; wire
/// lowering is player-owned).
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
// The two-region boundary fixture — mirrors
// `synvelis_project::midi_comp`'s canonical selection:
//   A: phrase [480,2400) → project [0,3840)
//   B: phrase [2400,3840) → project [3840,6720)
// division 480 → project PPQN 960 is an exact ×2; every fixture tick
// below maps without remainder.
// ---------------------------------------------------------------------

/// Builds the boundary project: one div-480 MIDI source, the note
/// set below, the two-region selection, and the controller lanes.
/// Returns `(controller, selection, lanes)`.
fn boundary_fixture() -> (ProjectController, MidiCompSelection, MidiControllerLanes) {
    let mut c = controller();
    let source = add_source(&mut c, 480);
    // n1 key60 [480,960)   → A: on 0,    off 960 (natural, inside A)
    // n2 key64 [960,2760)  → A: on 960,  virtual off 3840 (cut at A end)
    // n3 key67 [1500,1800) → A: on 2040, off 2640 (natural, inside A)
    // n4 key62 [100,150)   → excluded (onset before phrase A)
    // n5 key63 [2399,2409) → A: on 3838, virtual off 3840 (boundary tiny)
    // n6 key64 [2400,2880) → B: on 3840, off 4800 — SAME KEY as n2:
    //                        the cut note re-articulates at the
    //                        boundary; off must precede the re-on.
    // n7 key65 [2500,3000) → B: on 4040, off 5040 (natural, inside B)
    // n8 key67 [3500,4000) → B: on 6040, virtual off 6720 (cut at end)
    add_note(&mut c, source, 60, 100, 480, 480);
    add_note(&mut c, source, 64, 90, 960, 1800);
    add_note(&mut c, source, 67, 80, 1500, 300);
    add_note(&mut c, source, 62, 60, 100, 50);
    add_note(&mut c, source, 63, 75, 2399, 10);
    add_note(&mut c, source, 64, 85, 2400, 480);
    add_note(&mut c, source, 65, 70, 2500, 500);
    add_note(&mut c, source, 67, 72, 3500, 500);

    let selection = MidiCompSelection {
        regions: vec![
            region(101, source, (480, 2400), (0, 3_840)),
            region(102, source, (2400, 3840), (3_840, 6_720)),
        ],
    };
    // Controller lanes — the chased (channel, kind) union is
    // {(0,Cc1),(0,Cc64),(0,PitchBend),(1,Cc10),(1,Cc64)}; ch1 events
    // prove chase never mixes channels.
    let lanes = lanes_for(
        source,
        vec![
            ctrl(100, 0, MidiControllerKind::Cc(64), 127), // pre-A → A chase
            ctrl(600, 0, MidiControllerKind::Cc(1), 64),   // inside A
            ctrl(700, 0, MidiControllerKind::PitchBend, 10_000), // inside A
            ctrl(1500, 1, MidiControllerKind::Cc(64), 90), // inside A, ch1
            ctrl(2000, 0, MidiControllerKind::Cc(64), 64), // inside A → B chase
            ctrl(2500, 1, MidiControllerKind::Cc(10), 32), // inside B, ch1
            ctrl(3000, 0, MidiControllerKind::Cc(64), 0),  // inside B
        ],
    );
    (c, selection, lanes)
}

/// The complete expected wire stream for `boundary_fixture` —
/// `compile_midi_events` output in plan order (frames = project
/// ticks × 25). This is the contract the driver must deliver
/// verbatim; deriving it by hand keeps the test honest about the
/// boundary semantics rather than echoing the implementation.
fn expected_wire() -> Vec<(i64, MidiEvent)> {
    vec![
        // ── Region A start (proj 0 → frame 0): chase snapshot over
        // the 5-pair union. ch0: Cc1=0(default), Cc64=127(@100),
        // PB=8192(default); ch1: Cc10=0, Cc64=0 — all undeclared at
        // phrase start 480 chase their defaults.
        (0, cc(0, 1, 0)),
        (0, cc(0, 64, 127)),
        (0, bend(0, 8_192)),
        (0, cc(1, 10, 0)),
        (0, cc(1, 64, 0)),
        // n1 onset sits exactly on phrase start → on at proj 0,
        // after the chase snapshot (chase rank < note-on rank).
        (0, note_on(0, 60, 100)),
        // A mid-phrase controller events at mapped ticks:
        // CC1=64 @600→proj240, PB=10000 @700→proj440,
        // ch1 CC64=90 @1500→proj2040 (shares frame with n3's on —
        // chase rank precedes note-on).
        (ptick(240), cc(0, 1, 64)),
        (ptick(440), bend(0, 10_000)),
        // n1 natural off @960 precedes n2 on @960 (off < on rank).
        (ptick(960), note_off(0, 60)),
        (ptick(960), note_on(0, 64, 90)),
        (ptick(2_040), cc(1, 64, 90)),
        (ptick(2_040), note_on(0, 67, 80)),
        (ptick(2_640), note_off(0, 67)),
        // CC64=64 @2000→proj3040.
        (ptick(3_040), cc(0, 64, 64)),
        // n5 boundary tiny note on @3838.
        (ptick(3_838), note_on(0, 63, 75)),
        // ── Shared boundary proj 3840 → frame 96000: A's two virtual
        // offs (adopted order n2@960 < n5@2399), then B's 5-pair
        // chase (ch0 Cc1=64, Cc64=64(@2000), PB=10000; ch1 Cc10=0
        // default, Cc64=90(@1500)), then n6's re-articulation on the
        // SAME key 64 — off strictly precedes re-on, no double voice.
        (ptick(3_840), note_off(0, 64)),
        (ptick(3_840), note_off(0, 63)),
        (ptick(3_840), cc(0, 1, 64)),
        (ptick(3_840), cc(0, 64, 64)),
        (ptick(3_840), bend(0, 10_000)),
        (ptick(3_840), cc(1, 10, 0)),
        (ptick(3_840), cc(1, 64, 90)),
        (ptick(3_840), note_on(0, 64, 85)),
        // B mid-phrase: ch1 CC10=32 @2500→proj4040 (chase precedes
        // n7's on at the same tick), n7 off @5040 precedes the
        // CC64=0 @3000→proj5040 event (off rank < chase rank).
        (ptick(4_040), cc(1, 10, 32)),
        (ptick(4_040), note_on(0, 65, 70)),
        (ptick(4_800), note_off(0, 64)),
        (ptick(5_040), note_off(0, 65)),
        (ptick(5_040), cc(0, 64, 0)),
        // n8 on @6040, virtual off @6720 = comp end; the terminal
        // AllNotesOff expands to CC123 on ch0 only — the sole
        // channel that sounded a note (ch1 carried controllers only).
        (ptick(6_040), note_on(0, 67, 72)),
        (ptick(6_720), note_off(0, 67)),
        (ptick(6_720), cc(0, 123, 0)),
    ]
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// RCD-04 boundary lowering: the two-region plan compiles to the
/// exact wire stream — boundary order off → chase → on, same-key
/// re-articulation, channel-aware chase values, terminal CC123 on
/// sounded channels only.
#[test]
fn rcd_04_boundary_lowers_to_exact_wire_stream() {
    let (c, selection, lanes) = boundary_fixture();
    let events = compile_events(&c, &selection, &lanes, 0);
    let expected: Vec<(i64, MidiEvent)> = expected_wire();
    assert_eq!(
        events, expected,
        "wire stream must match the boundary contract"
    );

    // Monotone non-decreasing frames (plan total order survives
    // tick→media mapping).
    assert!(
        events.windows(2).all(|w| w[0].0 <= w[1].0),
        "frames must be monotone"
    );
}

/// RCD-04 boundary delivery: the HeadlessDriver/PluginNode path
/// delivers the compiled stream verbatim — no hanging voices, no
/// duplicate (channel,key) sounding, boundary off-before-re-on.
#[test]
fn rcd_04_boundary_driver_delivers_without_hanging_or_duplicate_voices() {
    let (c, selection, lanes) = boundary_fixture();
    let events = compile_events(&c, &selection, &lanes, 0);

    let (mut driver, insert, delivered, _host) = driver_fixture(&c);
    dispatch_midi(&mut driver, insert, &events);
    // Comp end is proj 6720 → frame 168000; run past it.
    driver.run(1_400, PumpPolicy::Every(1)).expect("run");

    let got = delivered.lock().unwrap().clone();
    let expected: Vec<Mid> = expected_wire()
        .into_iter()
        .map(|(f, e)| mid(f, e))
        .collect();
    assert_eq!(
        got, expected,
        "driver must deliver the wire stream verbatim"
    );

    // Ledger-level invariants over the delivered stream (independent
    // of the exact expected vector): every NoteOn on a (channel,key)
    // is preceded by its pair's NoteOff and no key sounds twice;
    // the active set is empty once the terminal panic lands.
    let mut active: Vec<(u8, u8)> = Vec::new();
    for m in &got {
        let ch = m.event.channel.get();
        match m.event.kind {
            MidiKind::NoteOn { key, .. } => {
                let k = key.get();
                assert!(
                    !active.contains(&(ch, k)),
                    "duplicate sounding voice ch{ch} key{k} at frame {}",
                    m.abs_frame
                );
                active.push((ch, k));
            }
            MidiKind::NoteOff { key, .. } => {
                let k = key.get();
                let pos = active
                    .iter()
                    .position(|a| *a == (ch, k))
                    .unwrap_or_else(|| panic!("note-off without note-on ch{ch} key{k}"));
                active.remove(pos);
            }
            _ => {}
        }
    }
    assert!(
        active.is_empty(),
        "hanging voices at stream end: {active:?}"
    );
}
