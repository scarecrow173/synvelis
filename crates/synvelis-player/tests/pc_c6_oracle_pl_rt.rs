//! PC-C6 oracle row PL05 RT half (issue #103, PC-C4 plugin gate):
//! the real `PluginNode` param-delivery hot path — staged
//! `TimedParamEvent` plan → scheduler → `EventList` → plugin
//! `process` — performs no heap allocation or deallocation once warm,
//! measured with `stats_alloc` (same method as `tim02_midi_rt` and
//! `tim03_process_boundary_rt`; setup and warm-up stay outside the
//! window). Events sit at non-block-aligned frames so the measured
//! window also exercises min-process-frame-offset delivery.
//!
//! This file is a separate test binary on purpose: `stats_alloc`'s
//! `Region` observes the process-global allocator, so the measured
//! window must not share a process with other tests. Evidence is
//! **headless fixture** only — real-plugin RT behaviour stays `NotRun`.

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
use ruviform_media_core::{DeliveryPolicy, MediaTime, ScheduleBudget, TimeBase};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TimedParamEvent, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_player::ParamDispatch;
use synvelis_project::ParameterTargetId;

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c6-pl-rt",
    name: "PC-C6 PL RT Recorder",
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

const PARAMS: [ParamInfo; 1] = [ParamInfo {
    id: ParamId(7),
    name: "Gain",
    unit: "",
    min_value: 0.0,
    max_value: 1.0,
    default_value: 0.0,
    flags: ParamFlags::AUTOMATABLE,
}];

/// Pre-sized event log — far beyond any fixture's delivery count, so
/// `push` never reallocates inside a measured window.
const LOG_CAPACITY: usize = 8192;

/// One delivered parameter event at its absolute engine frame.
#[derive(Debug, Clone, PartialEq)]
struct Delivered {
    abs_frame: u64,
    param_id: u32,
    value: f64,
}

struct RecPlugin {
    delivered: Arc<Mutex<Vec<Delivered>>>,
}

impl PluginProcessor for RecPlugin {
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
            if let EventKind::ParamValue(pe) = ev.kind() {
                #[allow(clippy::cast_sign_loss)]
                log.push(Delivered {
                    abs_frame: (pos + i64::from(ev.frame_offset())) as u64,
                    param_id: pe.param_id.0,
                    value: pe.value.get(),
                });
            }
        }
        ProcessResult::CONTINUE
    }
}

impl Plugin for RecPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &PARAMS
    }
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
    }
}

struct RecFactory {
    delivered: Arc<Mutex<Vec<Delivered>>>,
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(RecPlugin {
            delivered: Arc::clone(&self.delivered),
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
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

/// The compiled-graph RT drive: input writer + process, mirroring the
/// `tim02_midi_rt` measurement shape.
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

/// Hosted recorder + compiled graph drive. Returns control, drive,
/// delivery log and processor node id.
type FixtureParts = (
    PluginNodeControl,
    Drive,
    Arc<Mutex<Vec<Delivered>>>,
    NodeId,
    [PortId; 2],
);

fn fixture() -> FixtureParts {
    let delivered = Arc::new(Mutex::new(Vec::with_capacity(LOG_CAPACITY)));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
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
    (control, drive, delivered, proc, [sp[0], sp[0]])
}

fn param_event(frame: i64, value: f64) -> TimedParamEvent {
    TimedParamEvent {
        position: MediaTime::new(frame, frame_base()),
        event: ParamEvent {
            param_id: ParamId(7),
            value: ParamValue::new(value).unwrap(),
        },
    }
}

/// PL05 RT oracle: a staged param plan whose events sit at
/// non-block-aligned frames (10, 130, 300 inside 256-frame callbacks)
/// delivers at exactly those absolute frames with zero allocations in
/// the measured window — min-process-frame-offset + RT noalloc in one
/// evidence row.
#[test]
fn rt_pl05_param_delivery_alloc_free() {
    let (control, mut drive, delivered, proc, _srcs) = fixture();
    let mut dispatch = ParamDispatch::new(control, proc, frame_base()).unwrap();
    let target = ParameterTargetId::new(1);
    dispatch.bind(target, ParamId(7), &PARAMS).unwrap();

    // Stage once (non-RT): three events off every block boundary.
    let events = [
        param_event(10, 0.25),
        param_event(130, 0.5),
        param_event(300, 0.75),
    ];
    let writes: Vec<(ParameterTargetId, TimedParamEvent)> =
        events.iter().map(|e| (target, *e)).collect();
    dispatch.dispatch(&writes).unwrap();

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

    // Behavioural evidence: every declared event landed at its exact
    // absolute frame — min process frame offset preserved under
    // measurement (oracle: expected = declared (frame, param, value)).
    let log = delivered.lock().unwrap();
    let mut got: Vec<(u64, u32, u64)> = log
        .iter()
        .map(|d| (d.abs_frame, d.param_id, d.value.to_bits()))
        .collect();
    got.sort_unstable();
    let mut want: Vec<(u64, u32, u64)> = events
        .iter()
        .map(|e| {
            (
                u64::try_from(e.position.value()).unwrap(),
                e.event.param_id.0,
                e.event.value.get().to_bits(),
            )
        })
        .collect();
    want.sort_unstable();
    assert_eq!(got, want, "delivered stream must equal the declared batch");
}
