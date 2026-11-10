//! TIM-02 RT contract evidence (issue #126 player MIDI slice): the
//! real `PluginNode` hot path — `RtRuntime::process` → scheduler
//! `run_callback` → `CallSink` partitioning → plugin `process` —
//! performs no heap allocation or deallocation once warm, measured
//! with `stats_alloc` (same method as `tim03_process_boundary_rt`
//! and `trace_c3_05_rt`; setup and warm-up stay outside the window).
//!
//! This file is a separate test binary on purpose: `stats_alloc`'s
//! `Region` observes the process-global allocator, so the measured
//! window must not share a process with other tests.
//!
//! The measured window contains *real* boundary work every callback:
//! a `SetLoop[64,320)` wrap splits each 256-frame callback into
//! legal spans, the discontinuity pre-pass releases the spanning
//! key62 and chases it back, and the in-loop key60 pair re-fires —
//! span splitting, `calls_for_span` partitioning, event windowing,
//! ledger release/chase and plugin delivery are all exercised under
//! measurement. Plan content here is hand-built `(frame, MidiEvent)`
//! pairs — the project-plan lowering is covered by
//! `tim02_midi_playback.rs`; the RT property under test is the
//! delivery path, not the lowering.

use std::alloc::System;
use std::sync::{Arc, Mutex};

use ruviform_audio::typed::{Compiler, CompilerConfig, DEFAULT_ALIGNMENT, GraphRuntime, RtRuntime};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{
    AttachConfig, PluginNode, PluginNodeControl, PluginNodeFactory, attach,
};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, PortId, PortRef,
    ValidatedGraph, Validator,
};
use ruviform_media_core::{
    DeliveryPolicy, MediaDuration, MediaRange, MediaTime, MidiChannel, MidiEvent, MidiGroup,
    MidiKind, MidiValue, NoteKey, ScheduleBudget, TimeBase, TransportCommand,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError,
    ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_player::ParamDispatch;

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim02-rt",
    name: "TIM-02 RT MIDI Recorder",
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

fn t(value: i64) -> MediaTime {
    MediaTime::new(value, frame_base())
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

/// `Source → plugin → Sink` compiled graph + block drive.
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

/// Hosted recorder + compiled graph drive under `DeliveryPolicy::
/// Exact` — the loop[64,320) wrap splits every 256-frame callback.
/// Returns `(control, drive, delivered log, processor node id)`.
fn fixture() -> (PluginNodeControl, Drive, Arc<Mutex<Vec<Mid>>>, NodeId) {
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
    std::mem::forget(host);

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
    let graph = Validator::validate(&b.build()).expect("graph validates");

    let node = attached.node;
    let control = attached.control;
    let drive = Drive::new(&graph, node, proc, [sp[0], sp[0]]);
    (control, drive, delivered, proc)
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

/// The MIDI boundary hot path — span splitting at loop wraps,
/// discontinuity release/chase, per-occurrence plan delivery and
/// `EventList` construction — allocates nothing once warm.
#[test]
fn rt_midi_delivery_hot_path_alloc_free() {
    let (control, mut drive, delivered, proc) = fixture();
    let mut dispatch = ParamDispatch::new(control, proc, frame_base()).unwrap();

    // Stage once (non-RT): play, loop [64,320), MIDI plan whose
    // key62 spans the loop (on@50 off@400) so every wrap runs the
    // release+chase pre-pass, and key60 cycles inside the loop.
    control_play(&mut dispatch);
    dispatch
        .control()
        .request(
            TransportCommand::SetLoop(MediaRange::new(
                t(64),
                MediaDuration::new(256, frame_base()),
            )),
            0,
        )
        .unwrap();
    let events: Vec<(i64, MidiEvent)> = vec![
        (50, note_on(0, 62, 90)),
        (100, note_on(0, 60, 100)),
        (200, note_off(0, 60)),
        (400, note_off(0, 62)),
    ];
    dispatch.dispatch_midi(&events).unwrap();

    // Warm-up: first-touch activation, slot configure, plan Arc
    // retire paths — outside the measured window.
    for _ in 0..8 {
        drive.process(256);
    }
    assert!(!delivered.lock().unwrap().is_empty());

    {
        let region = Region::new(ALLOC);
        for _ in 0..64 {
            drive.process(256);
        }
        let stats = region.change();
        assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
        assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    }

    // Behavioural evidence collected under measurement: loop events
    // kept re-firing per occurrence — the in-loop on/off pair count
    // grows with processed laps, and the spanning key62 was released
    // and chased at every wrap.
    let log = delivered.lock().unwrap();
    let ons60 = log
        .iter()
        .filter(|m| m.event == note_on(0, 60, 100))
        .count();
    let offs60 = log.iter().filter(|m| m.event == note_off(0, 60)).count();
    assert!(ons60 >= 32, "in-loop events must re-fire per lap: {ons60}");
    assert_eq!(ons60, offs60, "no hanging in-loop voice");
    let ons62 = log.iter().filter(|m| m.event == note_on(0, 62, 90)).count();
    let offs62 = log.iter().filter(|m| m.event == note_off(0, 62)).count();
    // key62's scheduled off@400 sits outside the loop — while
    // looping it is only ever released by wraps, never re-off'd by
    // the plan; each wrap releases it once and the chase re-fires
    // the on, so offs62 == wraps and ons62 == 1 + wraps.
    assert!(offs62 > 0, "wraps must release the spanning voice");
    assert_eq!(ons62, offs62 + 1, "chase reconstructs each release");
}
