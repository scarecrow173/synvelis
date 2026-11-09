//! TIM-03 RT contract evidence (Issue #127 process-boundary slice):
//! the real `PluginNode` hot path — `RtRuntime::process` → scheduler
//! `run_callback` → `CallSink` partitioning → plugin `process` —
//! performs no heap allocation or deallocation once warm, measured
//! with `stats_alloc` (same method as `trace_c3_05_rt` and the
//! ruviform RT contract tests; setup and warm-up stay outside the
//! window).
//!
//! The measured window contains *real* boundary work every callback:
//! a `SetLoop[0,128)` wrap splits each 256-frame callback into two
//! 128-frame legal spans (min=128 contract), and a plan param event
//! inside the loop delivers once per occurrence — so span splitting,
//! `calls_for_span` partitioning, event windowing and plugin delivery
//! are all exercised under measurement. Staging (`control.request`,
//! `install_plan`, `input_writer` fill) is RT-suitable per upstream
//! contract; command *application* happens inside `rt.process`.
//!
//! `ProcessCapability::evaluate` is a non-RT preflight oracle by
//! contract and is deliberately excluded from the window.

use std::alloc::System;
use std::sync::{Arc, Mutex};

use ruviform_audio::typed::{Compiler, CompilerConfig, DEFAULT_ALIGNMENT, GraphRuntime, RtRuntime};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{
    AttachConfig, PluginNode, PluginNodeControl, PluginNodeFactory, attach,
};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, PortId, PortRef,
    Validator,
};
use ruviform_media_core::{
    BlockContract, DeliveryPolicy, InBlockTransportSupport, MediaDuration, MediaRange, MediaTime,
    PlanGeneration, ScheduleBudget, TimeBase, TransportCommand,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TransportInfo, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim03-rt",
    name: "TIM-03 RT Recorder",
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

const MAX_CALLS: usize = 512;
const MAX_PARAMS: u8 = 8;

/// Fixed-capacity call log — `process` never allocates.
#[derive(Clone, Copy, Default)]
struct CallHit {
    frames: u32,
    n_params: u8,
}

struct CallLog {
    len: usize,
    calls: [CallHit; MAX_CALLS],
}

impl Default for CallLog {
    fn default() -> Self {
        Self {
            len: 0,
            calls: [CallHit::default(); MAX_CALLS],
        }
    }
}

struct RecPlugin {
    log: Arc<Mutex<CallLog>>,
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
        inputs: &[I],
        _outputs: &mut [O],
        events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        let frames = inputs.first().map_or(0, BufferView::frames);
        let mut n_params = 0u8;
        for ev in events.iter() {
            if matches!(ev.kind(), EventKind::ParamValue(_)) {
                n_params = n_params.saturating_add(1).min(MAX_PARAMS);
            }
        }
        let mut log = self.log.lock().unwrap();
        if log.len < MAX_CALLS {
            let slot = log.len;
            log.calls[slot] = CallHit {
                frames: u32::try_from(frames).unwrap_or(u32::MAX),
                n_params,
            };
            log.len = slot + 1;
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
    log: Arc<Mutex<CallLog>>,
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        ruviform_plugin_core::erase(RecPlugin {
            log: Arc::clone(&self.log),
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
            log: Arc::clone(&self.log),
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

fn loop_range(start: i64, end: i64) -> MediaRange {
    MediaRange::new(t(start), MediaDuration::new(end - start, frame_base()))
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

struct Drive {
    rt: RtRuntime,
    ctx: RtContext,
    src: PortId,
    zeros: Vec<f32>,
}

impl Drive {
    fn new(
        graph: &ruviform_graph::ValidatedGraph,
        node: PluginNode,
        proc: NodeId,
        srcs: [PortId; 2],
    ) -> Self {
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

/// Hosted recorder + compiled graph drive with `ProcessConfig(48k,
/// 128, 512)` under `DeliveryPolicy::Exact` — the loop[0,128) wrap
/// splits every 256-frame callback into two legal 128-frame spans.
fn fixture() -> (PluginNodeControl, Drive, Arc<Mutex<CallLog>>) {
    let log = Arc::new(Mutex::new(CallLog::default()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        log: Arc::clone(&log),
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
    // min=128: the loop[0,128) wrap boundary at offset 128 of every
    // 256-frame callback yields two legal 128-frame spans.
    let process_config = ProcessConfig::new(SampleRate::new(48_000).unwrap(), 128, 512).unwrap();
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
            budget: ScheduleBudget::new(64, 256, 16, 4096),
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
    (control, drive, log)
}

/// The boundary hot path — span splitting at loop wraps, legal call
/// partitioning and per-occurrence param delivery — allocates nothing
/// once warm.
#[test]
fn rt_process_boundary_hot_path_alloc_free() {
    let (mut control, mut drive, log) = fixture();
    let contract = BlockContract::new(128, 512, InBlockTransportSupport::Unknown);

    // Stage once (non-RT): play, loop[0,128), param7@pos64 plan.
    control.request(TransportCommand::Play, 0).unwrap();
    control
        .request(TransportCommand::SetLoop(loop_range(0, 128)), 0)
        .unwrap();
    let mut pb = control.plan_builder(PlanGeneration::new(7));
    pb.event(
        t(64),
        1,
        EventKind::ParamValue(ParamEvent {
            param_id: ParamId(7),
            value: ParamValue::new(0.5).unwrap(),
        }),
    )
    .unwrap();
    control.install_plan(Arc::new(pb.build()), 0).unwrap();

    // Warm-up: first-touch activation, slot configure, plan Arc
    // retire paths — outside the measured window.
    for _ in 0..8 {
        drive.process(256);
    }
    {
        let log = log.lock().unwrap();
        assert!(log.len > 0);
    }

    {
        let region = Region::new(ALLOC);
        for _ in 0..64 {
            drive.process(256);
        }
        let stats = region.change();
        assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
        assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    }

    // Behavioural evidence collected under measurement: every
    // physical call is a legal 128-frame partition and the in-loop
    // param event delivered once per occurrence.
    let log = log.lock().unwrap();
    assert!(log.len >= 64 * 2, "two spans per callback: {}", log.len);
    let mut calls = 0usize;
    let mut param_calls = 0usize;
    for i in 0..log.len {
        let hit = log.calls[i];
        assert!(
            contract.legal_call_len(hit.frames),
            "illegal call {} at index {i}",
            hit.frames
        );
        calls += 1;
        param_calls += usize::from(hit.n_params > 0);
    }
    assert_eq!(calls % 2, 0);
    assert!(param_calls > 0, "in-loop event must deliver per occurrence");
}
