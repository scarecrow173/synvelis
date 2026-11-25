//! RCD-05 RT contract evidence: `MonitorRtNode` performs bounded schedule
//! application and PCM mixing inside `RtRuntime::process` without heap
//! allocation or deallocation once warm.
//!
//! This file is a separate test binary because `stats_alloc` observes the
//! process-global allocator. Graph compilation, control-channel filling,
//! input PCM construction, activation and warm-up stay outside the measured
//! window. The window contains real work on every callback: one schedule is
//! consumed, a mid-block Input/Playback boundary is applied, both input
//! planes are mixed through the gain ramp, and monitor output is written.

use std::alloc::System;

use ruviform_audio::typed::{
    Compiler, CompilerConfig, DEFAULT_ALIGNMENT, FactoryError, GraphRuntime, InstalledInfo,
    PrepareOutcome, RtRuntime,
};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, NodeSpec, PortDef,
    PortId, PortRef, Validator,
};
use ruviform_rt::RtContext;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_player::{
    MonitorRoute, MonitorRtControl, MonitorRtNode, MonitorRtNodeConfig, MonitorRtSegment,
};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const FRAMES: usize = 128;
const WARM_BLOCKS: usize = 8;
const MEASURED_BLOCKS: usize = 64;

fn fmt() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn audio_port(direction: Direction) -> PortDef {
    PortDef {
        direction,
        role: ruviform_graph::PortRole::Main,
        format: ruviform_graph::PortFormat::Audio(fmt()),
        multiplicity: ruviform_graph::Multiplicity::unbounded(0),
        capabilities: Vec::new(),
        availability: ruviform_graph::Availability::Always,
        activation: ruviform_graph::Activation::Enabled,
    }
}

fn compiler_config() -> CompilerConfig {
    CompilerConfig {
        max_frames: 256,
        alignment: DEFAULT_ALIGNMENT,
        max_slots: 32,
        max_channels: 4,
        max_delay_lines: 4,
        max_delay_frames: 4096,
        max_processors: 8,
    }
}

struct Drive {
    rt: RtRuntime,
    ctx: RtContext,
    playback_source: PortId,
    monitor_source: PortId,
    monitor_sink: PortId,
    playback: Vec<Vec<f32>>,
    monitor_input: Vec<Vec<f32>>,
}

impl Drive {
    fn process_block(&mut self) -> (f32, f32) {
        {
            let mut slot = self
                .rt
                .input_writer(self.playback_source, FRAMES)
                .expect("playback input");
            for channel in 0..2 {
                slot.data.channel_mut(channel).unwrap()[..FRAMES]
                    .copy_from_slice(&self.playback[channel][..FRAMES]);
            }
        }
        {
            let mut slot = self
                .rt
                .input_writer(self.monitor_source, FRAMES)
                .expect("monitor input");
            for channel in 0..2 {
                slot.data.channel_mut(channel).unwrap()[..FRAMES]
                    .copy_from_slice(&self.monitor_input[channel][..FRAMES]);
            }
        }
        self.rt.process(FRAMES, &self.ctx).expect("process");
        let output = self
            .rt
            .output_reader(self.monitor_sink)
            .expect("monitor output");
        (
            output.data.channel(0).unwrap()[0],
            output.data.channel(0).unwrap()[96],
        )
    }
}

struct MonitorGraph {
    graph: ruviform_graph::ValidatedGraph,
    playback_source: PortId,
    monitor_source: PortId,
    monitor_sink: PortId,
    monitor_node: NodeId,
}

fn build_monitor_graph() -> MonitorGraph {
    let mut builder = GraphBuilder::new();
    let (playback_source, playback_source_ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Source,
        ports: vec![audio_port(Direction::Output)],
    });
    let (monitor_source, monitor_source_ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Source,
        ports: vec![audio_port(Direction::Output)],
    });
    let (monitor_node, monitor_ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: vec![
            audio_port(Direction::Input),
            audio_port(Direction::Input),
            audio_port(Direction::Output),
        ],
    });
    let (monitor_sink, monitor_sink_ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Sink,
        ports: vec![audio_port(Direction::Input)],
    });
    let connect = |from: PortRef, to: PortRef| ConnDef {
        from,
        to,
        delay: EdgeDelay::Zero,
        channel_map: None,
    };
    let port_ref = |node, port| PortRef { node, port };
    builder.connect(&connect(
        port_ref(playback_source, playback_source_ports[0]),
        port_ref(monitor_node, monitor_ports[0]),
    ));
    builder.connect(&connect(
        port_ref(monitor_source, monitor_source_ports[0]),
        port_ref(monitor_node, monitor_ports[1]),
    ));
    builder.connect(&connect(
        port_ref(monitor_node, monitor_ports[2]),
        port_ref(monitor_sink, monitor_sink_ports[0]),
    ));
    MonitorGraph {
        graph: Validator::validate(&builder.build()).expect("graph validates"),
        playback_source: playback_source_ports[0],
        monitor_source: monitor_source_ports[0],
        monitor_sink: monitor_sink_ports[0],
        monitor_node,
    }
}

fn compile_monitor_runtime(
    graph: &ruviform_graph::ValidatedGraph,
    monitor_node_id: NodeId,
    node: MonitorRtNode,
) -> (GraphRuntime, RtRuntime) {
    let mut processor = Some(node);
    let mut factory = move |spec: &NodeSpec,
                            _existing: Option<&InstalledInfo>|
          -> Result<PrepareOutcome, FactoryError> {
        if spec.kind() == NodeKind::Processor {
            assert_eq!(spec.id(), monitor_node_id);
            Ok(PrepareOutcome::New(Box::new(
                processor.take().expect("one monitor processor"),
            )))
        } else {
            Ok(PrepareOutcome::Boundary)
        }
    };
    let (mut runtime, rt) = GraphRuntime::new(&compiler_config(), 8, 8);
    let compiled = Compiler::new(compiler_config())
        .compile(
            graph,
            &mut factory,
            runtime.registry(),
            runtime.allocator(),
            runtime.generation() + 1,
        )
        .expect("monitor graph compiles");
    runtime.publish(compiled).expect("publish");
    (runtime, rt)
}

fn plane(base: f32) -> Vec<Vec<f32>> {
    (0..2)
        .map(|channel| {
            (0..FRAMES)
                .map(|frame| {
                    base + f32::from(u8::try_from(channel).unwrap()) * 0.1
                        + f32::from(u8::try_from(frame).unwrap()) * 0.0001
                })
                .collect()
        })
        .collect()
}

fn fixture() -> (GraphRuntime, Drive, MonitorRtControl) {
    let (node, mut control) = MonitorRtNode::new(MonitorRtNodeConfig {
        format: fmt(),
        max_block_frames: 256,
        ramp_frames: 16,
        control_capacity: WARM_BLOCKS + MEASURED_BLOCKS + 8,
    })
    .expect("monitor node config");
    let graph = build_monitor_graph();
    let (runtime, rt) = compile_monitor_runtime(&graph.graph, graph.monitor_node, node);

    let segments = [
        MonitorRtSegment::new(0, 64, MonitorRoute::Input),
        MonitorRtSegment::new(64, u32::try_from(FRAMES).unwrap(), MonitorRoute::Playback),
    ];
    for sequence in 0..(WARM_BLOCKS + MEASURED_BLOCKS) {
        control
            .push_tile(
                u64::try_from(sequence).unwrap(),
                u32::try_from(FRAMES).unwrap(),
                &segments,
            )
            .expect("schedule fits control capacity");
    }

    let drive = Drive {
        rt,
        ctx: RtContext::mint(),
        playback_source: graph.playback_source,
        monitor_source: graph.monitor_source,
        monitor_sink: graph.monitor_sink,
        playback: plane(0.5),
        monitor_input: plane(-0.25),
    };
    (runtime, drive, control)
}

#[test]
fn rt_monitor_node_process_hot_path_alloc_free() {
    let (_runtime, mut drive, control) = fixture();

    // First-touch activation, schedule ordering, slot configuration and
    // immutable-borrow paths are warm before measurement.
    for _ in 0..WARM_BLOCKS {
        let _ = drive.process_block();
    }
    assert_eq!(control.status().applied_blocks(), WARM_BLOCKS as u64);

    let mut observed_head = 0.0_f32;
    let mut observed_crossfade = 0.0_f32;
    {
        let region = Region::new(ALLOC);
        for _ in 0..MEASURED_BLOCKS {
            let (head, crossfade) = drive.process_block();
            observed_head += head;
            observed_crossfade += crossfade;
        }
        let stats = region.change();
        assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
        assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    }

    // Behavioural evidence collected while measured: schedules were consumed
    // and monitor PCM was actually generated. Frame 0 is in the Input segment;
    // frame 96 is after the Input→Playback boundary, inside the ramped
    // playback region. Both accumulate finite non-zero observations.
    let status = control.status();
    assert_eq!(
        status.applied_blocks(),
        (WARM_BLOCKS + MEASURED_BLOCKS) as u64
    );
    assert_eq!(
        status.applied_segments(),
        ((WARM_BLOCKS + MEASURED_BLOCKS) * 2) as u64
    );
    assert_eq!(
        status.output_frames(),
        ((WARM_BLOCKS + MEASURED_BLOCKS) * FRAMES) as u64
    );
    assert!(observed_head.is_finite() && observed_head != 0.0);
    assert!(observed_crossfade.is_finite() && observed_crossfade != 0.0);
    assert_eq!(status.input_missing_blocks(), 0);
    assert_eq!(status.schedule_contract_violations(), 0);
}
