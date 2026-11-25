//! RCD-05 typed-graph behavioural fixture for `MonitorRtNode`.
//!
//! The fixture drives the real `ruviform_audio::typed::RtRuntime::process`
//! path. Its oracle is independent of the node implementation and of
//! `MonitorAdapter::process_block`: it derives each output frame directly
//! from the schedule, the two input PCM planes, and the documented linear
//! gain-ramp arithmetic.

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
use synvelis_player::{
    MONITOR_RT_FAULT_CONTROL_OVERFLOW, MONITOR_RT_FAULT_IO_CONTRACT,
    MONITOR_RT_FAULT_SCHEDULE_MALFORMED, MONITOR_RT_FAULT_SCHEDULE_MISSING,
    MONITOR_RT_FAULT_SEQUENCE, MONITOR_RT_STATUS_INPUT_MISSING, MonitorRoute, MonitorRtControl,
    MonitorRtNode, MonitorRtNodeConfig, MonitorRtNodeError, MonitorRtSchedule, MonitorRtSegment,
};

const FRAMES: usize = 128;

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

struct Fixture {
    runtime: GraphRuntime,
    rt: RtRuntime,
    ctx: RtContext,
    control: MonitorRtControl,
    playback_source: PortId,
    monitor_source: PortId,
    monitor_sink: PortId,
    nonmonitor_sink: PortId,
    monitor_node: NodeId,
}

impl Fixture {
    fn new(ramp_frames: u32, control_capacity: usize) -> Self {
        Self::new_with_max_block(ramp_frames, control_capacity, 256)
    }

    fn new_with_max_block(
        ramp_frames: u32,
        control_capacity: usize,
        max_block_frames: usize,
    ) -> Self {
        let (node, control) = MonitorRtNode::new(MonitorRtNodeConfig {
            format: fmt(),
            max_block_frames,
            ramp_frames,
            control_capacity,
        })
        .expect("monitor node config");

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
        let (nonmonitor_sink, nonmonitor_sink_ports) = builder.add_node(&NodeDef {
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
        builder.connect(&connect(
            port_ref(playback_source, playback_source_ports[0]),
            port_ref(nonmonitor_sink, nonmonitor_sink_ports[0]),
        ));
        let graph = Validator::validate(&builder.build()).expect("graph validates");

        let mut processor = Some(node);
        let mut factory = move |spec: &NodeSpec,
                                _existing: Option<&InstalledInfo>|
              -> Result<PrepareOutcome, FactoryError> {
            if spec.kind() == NodeKind::Processor {
                assert_eq!(spec.id(), monitor_node);
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
                &graph,
                &mut factory,
                runtime.registry(),
                runtime.allocator(),
                runtime.generation() + 1,
            )
            .expect("monitor graph compiles");
        runtime.publish(compiled).expect("publish");
        Self {
            runtime,
            rt,
            ctx: RtContext::mint(),
            control,
            playback_source: playback_source_ports[0],
            monitor_source: monitor_source_ports[0],
            monitor_sink: monitor_sink_ports[0],
            nonmonitor_sink: nonmonitor_sink_ports[0],
            monitor_node,
        }
    }

    fn write_source(&mut self, port: PortId, pcm: &[Vec<f32>], frames: usize) {
        let mut slot = self.rt.input_writer(port, frames).expect("source input");
        for (channel, plane) in pcm.iter().enumerate() {
            slot.data.channel_mut(channel).unwrap()[..frames].copy_from_slice(&plane[..frames]);
        }
    }

    fn process(&mut self, frames: usize) {
        self.rt.process(frames, &self.ctx).expect("rt process");
    }

    fn read_output(&self, port: PortId, frames: usize) -> Vec<Vec<f32>> {
        let slot = self.rt.output_reader(port).expect("output reader");
        (0..2)
            .map(|channel| slot.data.channel(channel).unwrap()[..frames].to_vec())
            .collect()
    }

    fn monitor_pcm(&self, frames: usize) -> Vec<Vec<f32>> {
        self.read_output(self.monitor_sink, frames)
    }

    fn nonmonitor_pcm(&self, frames: usize) -> Vec<Vec<f32>> {
        self.read_output(self.nonmonitor_sink, frames)
    }
}

fn pcm(frames: usize, f: impl Fn(usize, usize) -> f32) -> Vec<Vec<f32>> {
    (0..2)
        .map(|channel| (0..frames).map(|frame| f(channel, frame)).collect())
        .collect()
}

fn playback_pcm(frames: usize) -> Vec<Vec<f32>> {
    pcm(frames, |channel, frame| {
        0.125
            + f32::from(u8::try_from(channel).unwrap()) * 0.25
            + f32::from(u8::try_from(frame).unwrap()) * 0.001
    })
}

fn input_pcm(frames: usize) -> Vec<Vec<f32>> {
    pcm(frames, |channel, frame| {
        -0.75
            + f32::from(u8::try_from(channel).unwrap()) * 0.5
            + f32::from(u8::try_from(frame).unwrap()) * 0.002
    })
}

fn write_inputs(
    fixture: &mut Fixture,
    playback: &[Vec<f32>],
    monitor: Option<&[Vec<f32>]>,
    frames: usize,
) {
    fixture.write_source(fixture.playback_source, playback, frames);
    if let Some(monitor) = monitor {
        fixture.write_source(fixture.monitor_source, monitor, frames);
    }
}

fn push_single(fixture: &mut Fixture, sequence: u64, frames: usize, route: MonitorRoute) {
    fixture
        .control
        .push_tile(
            sequence,
            u32::try_from(frames).unwrap(),
            &[MonitorRtSegment::new(
                0,
                u32::try_from(frames).unwrap(),
                route,
            )],
        )
        .expect("push schedule");
}

fn assert_bit_eq(got: &[Vec<f32>], expected: &[Vec<f32>]) {
    assert_eq!(got.len(), expected.len(), "channel count");
    for (got_channel, expected_channel) in got.iter().zip(expected) {
        assert_eq!(got_channel.len(), expected_channel.len(), "frame count");
        for (index, (got, expected)) in got_channel.iter().zip(expected_channel).enumerate() {
            assert_eq!(
                got.to_bits(),
                expected.to_bits(),
                "sample {index}: got {got:?}, expected {expected:?}"
            );
        }
    }
}

fn assert_silence(pcm: &[Vec<f32>]) {
    for channel in pcm {
        for sample in channel {
            assert_eq!(*sample, 0.0);
        }
    }
}

fn exactly_one(value: f32) -> bool {
    value.to_bits() == 1.0_f32.to_bits()
}

fn exactly_zero(value: f32) -> bool {
    value.to_bits() == 0.0_f32.to_bits()
}

/// Independent oracle: no production helper or adapter code is called.
/// It reproduces the documented arithmetic from first principles for one
/// block whose initial gains are both zero.
fn independent_oracle(
    frames: usize,
    segments: &[MonitorRtSegment],
    playback: &[Vec<f32>],
    monitor: Option<&[Vec<f32>]>,
    ramp_frames: u32,
) -> Vec<Vec<f32>> {
    let step = if ramp_frames == 0 {
        f32::INFINITY
    } else {
        1.0 / f32::from(u16::try_from(ramp_frames).expect("test ramp fits u16"))
    };
    let mut input_gain = 0.0_f32;
    let mut playback_gain = 0.0_f32;
    let mut gains = Vec::with_capacity(frames);
    let mut segment_index = 0_usize;
    for frame in 0..frames {
        while segment_index + 1 < segments.len()
            && frame >= segments[segment_index].end_frame as usize
        {
            segment_index += 1;
        }
        let route = segments[segment_index].route;
        let input_target = f32::from(u8::from(route == MonitorRoute::Input && monitor.is_some()));
        let playback_target = f32::from(u8::from(route == MonitorRoute::Playback));
        if input_gain < input_target {
            input_gain = (input_gain + step).min(input_target);
        } else if input_gain > input_target {
            input_gain = (input_gain - step).max(input_target);
        }
        if playback_gain < playback_target {
            playback_gain = (playback_gain + step).min(playback_target);
        } else if playback_gain > playback_target {
            playback_gain = (playback_gain - step).max(playback_target);
        }
        gains.push((input_gain, playback_gain));
    }
    (0..2)
        .map(|channel| {
            (0..frames)
                .map(|frame| {
                    let (input_gain, playback_gain) = gains[frame];
                    let playback_sample = playback[channel][frame];
                    let input_sample = monitor.map_or(0.0, |pcm| pcm[channel][frame]);
                    let playback_term = if exactly_one(playback_gain) {
                        playback_sample
                    } else if exactly_zero(playback_gain) {
                        0.0
                    } else {
                        playback_sample * playback_gain
                    };
                    let input_term = if exactly_one(input_gain) {
                        input_sample
                    } else if exactly_zero(input_gain) {
                        0.0
                    } else {
                        input_sample * input_gain
                    };
                    if exactly_one(playback_gain) && exactly_zero(input_gain) {
                        playback_sample
                    } else if exactly_one(input_gain) && exactly_zero(playback_gain) {
                        input_sample
                    } else {
                        playback_term + input_term
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn monitor_rt_node_constructor_rejects_invalid_config() {
    let base = MonitorRtNodeConfig {
        format: fmt(),
        max_block_frames: 256,
        ramp_frames: 8,
        control_capacity: 4,
    };
    assert!(matches!(
        MonitorRtNode::new(MonitorRtNodeConfig {
            max_block_frames: 0,
            ..base
        }),
        Err(MonitorRtNodeError::ZeroBlockFrames)
    ));
    assert!(matches!(
        MonitorRtNode::new(MonitorRtNodeConfig {
            control_capacity: 0,
            ..base
        }),
        Err(MonitorRtNodeError::ZeroControlCapacity)
    ));
    assert!(matches!(
        MonitorRtNode::new(MonitorRtNodeConfig {
            ramp_frames: u32::from(u16::MAX) + 1,
            ..base
        }),
        Err(MonitorRtNodeError::RampFramesTooLarge { .. })
    ));
    let mut format = fmt();
    format.sample_format = SampleFormat::F64;
    assert!(matches!(
        MonitorRtNode::new(MonitorRtNodeConfig { format, ..base }),
        Err(MonitorRtNodeError::UnsupportedSampleFormat)
    ));
}

#[test]
fn monitor_rt_node_playback_passthrough_preserves_bits_and_nonmonitor_path() {
    let mut fixture = Fixture::new(0, 4);
    let playback = pcm(FRAMES, |channel, frame| {
        if frame == 3 && channel == 0 {
            -0.0
        } else {
            playback_pcm(FRAMES)[channel][frame]
        }
    });
    let input = input_pcm(FRAMES);
    push_single(&mut fixture, 0, FRAMES, MonitorRoute::Playback);
    write_inputs(&mut fixture, &playback, Some(&input), FRAMES);
    fixture.process(FRAMES);

    assert_bit_eq(&fixture.monitor_pcm(FRAMES), &playback);
    assert_bit_eq(&fixture.nonmonitor_pcm(FRAMES), &playback);
    let status = fixture.control.status();
    assert_eq!(status.input_missing_blocks(), 0);
    assert_eq!(status.applied_blocks(), 1);
    assert_eq!(status.output_frames(), u64::try_from(FRAMES).unwrap());
}

#[test]
fn monitor_rt_node_distinguishes_supplied_silence_from_missing_input() {
    let mut fixture = Fixture::new(0, 4);
    let playback = playback_pcm(FRAMES);
    let supplied_silence = pcm(FRAMES, |_channel, _frame| 0.0);

    push_single(&mut fixture, 0, FRAMES, MonitorRoute::Input);
    write_inputs(&mut fixture, &playback, Some(&supplied_silence), FRAMES);
    fixture.process(FRAMES);
    assert_silence(&fixture.monitor_pcm(FRAMES));
    assert_eq!(fixture.control.status().input_missing_blocks(), 0);
    assert_eq!(fixture.control.status().applied_blocks(), 1);

    push_single(&mut fixture, 1, FRAMES, MonitorRoute::Input);
    write_inputs(&mut fixture, &playback, None, FRAMES);
    fixture.process(FRAMES);
    assert_silence(&fixture.monitor_pcm(FRAMES));
    let status = fixture.control.status();
    assert_eq!(status.input_missing_blocks(), 1);
    assert_ne!(status.fault_flags() & MONITOR_RT_STATUS_INPUT_MISSING, 0);
    assert_eq!(status.applied_blocks(), 2);
}

#[test]
fn monitor_rt_node_unused_missing_monitor_input_is_not_input_missing() {
    let mut fixture = Fixture::new(0, 4);
    let playback = playback_pcm(FRAMES);
    push_single(&mut fixture, 0, FRAMES, MonitorRoute::Playback);
    write_inputs(&mut fixture, &playback, None, FRAMES);
    fixture.process(FRAMES);

    assert_bit_eq(&fixture.monitor_pcm(FRAMES), &playback);
    let status = fixture.control.status();
    assert_eq!(status.input_missing_blocks(), 0);
    assert_eq!(status.fault_flags() & MONITOR_RT_STATUS_INPUT_MISSING, 0);
    assert_eq!(status.playback_missing_blocks(), 0);
    assert_eq!(status.applied_blocks(), 1);
}

#[test]
fn monitor_rt_node_mid_block_boundaries_match_independent_oracle() {
    let mut fixture = Fixture::new(8, 4);
    let playback = playback_pcm(FRAMES);
    let input = input_pcm(FRAMES);
    let segments = [
        MonitorRtSegment::new(0, 32, MonitorRoute::Input),
        MonitorRtSegment::new(32, 80, MonitorRoute::Playback),
        MonitorRtSegment::new(80, u32::try_from(FRAMES).unwrap(), MonitorRoute::Muted),
    ];
    fixture
        .control
        .push_tile(0, u32::try_from(FRAMES).unwrap(), &segments)
        .expect("push segmented schedule");
    write_inputs(&mut fixture, &playback, Some(&input), FRAMES);
    fixture.process(FRAMES);

    let expected = independent_oracle(FRAMES, &segments, &playback, Some(&input), 8);
    let got = fixture.monitor_pcm(FRAMES);
    assert_bit_eq(&got, &expected);
    let status = fixture.control.status();
    assert_eq!(status.applied_blocks(), 1);
    assert_eq!(status.applied_segments(), 3);
    assert_eq!(status.output_frames(), u64::try_from(FRAMES).unwrap());
}

#[test]
fn monitor_rt_node_frames_above_configured_max_write_zero_output() {
    let mut fixture = Fixture::new_with_max_block(0, 4, 64);
    let playback = playback_pcm(FRAMES);
    let input = input_pcm(FRAMES);
    write_inputs(&mut fixture, &playback, Some(&input), FRAMES);
    fixture.process(FRAMES);

    assert_silence(&fixture.monitor_pcm(FRAMES));
    let status = fixture.control.status();
    assert_ne!(status.fault_flags() & MONITOR_RT_FAULT_IO_CONTRACT, 0);
    assert_eq!(status.output_frames(), u64::try_from(FRAMES).unwrap());
    assert_eq!(status.applied_blocks(), 0);
}

#[test]
fn monitor_rt_node_compiled_monitor_input_is_a_direct_edge() {
    let fixture = Fixture::new(0, 4);
    let plan = fixture.runtime.published_plan();
    let scheduled = plan
        .schedule
        .iter()
        .find(|node| node.node == fixture.monitor_node)
        .expect("monitor node in schedule");
    assert_eq!(scheduled.inputs.len(), 2);
    for input in &scheduled.inputs {
        assert_eq!(input.edges.len(), 1, "no fan-in");
        assert!(input.scratch.is_none(), "no materialization scratch");
        assert!(input.edges[0].map.is_none(), "no channel map");
        assert_eq!(input.edges[0].delay_frames, 0, "no PDC delay");
    }
}

fn run_fault_injection(schedule: Option<MonitorRtSchedule>, expected_fault: u64) -> Vec<Vec<f32>> {
    let mut fixture = Fixture::new(0, 4);
    if let Some(schedule) = schedule {
        fixture.control.push_schedule(schedule).expect("raw push");
    }
    let playback = playback_pcm(FRAMES);
    let input = input_pcm(FRAMES);
    write_inputs(&mut fixture, &playback, Some(&input), FRAMES);
    fixture.process(FRAMES);
    let got = fixture.monitor_pcm(FRAMES);
    let status = fixture.control.status();
    assert_ne!(status.fault_flags() & expected_fault, 0);
    assert_eq!(status.applied_blocks(), 0);
    got
}

#[test]
fn monitor_rt_node_schedule_contract_violations_fail_closed() {
    let frames_u32 = u32::try_from(FRAMES).unwrap();
    let valid = [MonitorRtSegment::new(0, frames_u32, MonitorRoute::Playback)];
    let mut cases: Vec<(Option<MonitorRtSchedule>, u64, &str)> = Vec::new();

    cases.push((None, MONITOR_RT_FAULT_SCHEDULE_MISSING, "missing"));
    cases.push((
        Some(MonitorRtSchedule::empty(0, frames_u32)),
        MONITOR_RT_FAULT_SCHEDULE_MALFORMED,
        "no segments",
    ));

    let mut len_exceeds_capacity = MonitorRtSchedule::empty(0, frames_u32);
    len_exceeds_capacity.len = u8::try_from(synvelis_player::MONITOR_RT_SCHEDULE_CAPACITY + 1)
        .expect("capacity plus one fits u8");
    cases.push((
        Some(len_exceeds_capacity),
        MONITOR_RT_FAULT_SCHEDULE_MALFORMED,
        "len exceeds capacity",
    ));

    let mut gap = MonitorRtSchedule::empty(0, frames_u32);
    gap.len = 2;
    gap.segments[0] = MonitorRtSegment::new(0, 32, MonitorRoute::Playback);
    gap.segments[1] = MonitorRtSegment::new(40, frames_u32, MonitorRoute::Playback);
    cases.push((Some(gap), MONITOR_RT_FAULT_SCHEDULE_MALFORMED, "gap"));

    let mut overlap = MonitorRtSchedule::empty(0, frames_u32);
    overlap.len = 2;
    overlap.segments[0] = MonitorRtSegment::new(0, 64, MonitorRoute::Playback);
    overlap.segments[1] = MonitorRtSegment::new(32, frames_u32, MonitorRoute::Playback);
    cases.push((
        Some(overlap),
        MONITOR_RT_FAULT_SCHEDULE_MALFORMED,
        "overlap",
    ));

    let mut empty = MonitorRtSchedule::empty(0, frames_u32);
    empty.len = 1;
    empty.segments[0] = MonitorRtSegment::new(0, 0, MonitorRoute::Playback);
    cases.push((
        Some(empty),
        MONITOR_RT_FAULT_SCHEDULE_MALFORMED,
        "empty segment",
    ));

    let mut out_of_range = MonitorRtSchedule::empty(0, frames_u32);
    out_of_range.len = 1;
    out_of_range.segments[0] = MonitorRtSegment::new(0, frames_u32 + 1, MonitorRoute::Playback);
    cases.push((
        Some(out_of_range),
        MONITOR_RT_FAULT_SCHEDULE_MALFORMED,
        "out of range",
    ));

    let mut frame_mismatch = MonitorRtSchedule::empty(0, 64);
    frame_mismatch.len = 1;
    frame_mismatch.segments[0] = MonitorRtSegment::new(0, 64, MonitorRoute::Playback);
    cases.push((
        Some(frame_mismatch),
        MONITOR_RT_FAULT_SCHEDULE_MALFORMED,
        "frame mismatch",
    ));

    let mut wrong_sequence = MonitorRtSchedule::empty(1, frames_u32);
    wrong_sequence.len = 1;
    wrong_sequence.segments[0] = valid[0];
    cases.push((Some(wrong_sequence), MONITOR_RT_FAULT_SEQUENCE, "sequence"));

    for (schedule, fault, name) in cases {
        let pcm = run_fault_injection(schedule, fault);
        assert_silence(&pcm);
        assert!(
            pcm.iter().all(|channel| channel.len() == FRAMES),
            "{name}: full block length"
        );
    }
}

#[test]
fn monitor_rt_node_control_overflow_mutes_next_observed_block() {
    let mut fixture = Fixture::new(0, 1);
    push_single(&mut fixture, 0, FRAMES, MonitorRoute::Playback);
    let overflow = fixture.control.push_tile(
        1,
        u32::try_from(FRAMES).unwrap(),
        &[MonitorRtSegment::new(
            0,
            u32::try_from(FRAMES).unwrap(),
            MonitorRoute::Playback,
        )],
    );
    assert!(overflow.is_err());
    assert_eq!(fixture.control.status().control_overflows(), 1);

    let playback = playback_pcm(FRAMES);
    let input = input_pcm(FRAMES);
    write_inputs(&mut fixture, &playback, Some(&input), FRAMES);
    fixture.process(FRAMES);

    assert_silence(&fixture.monitor_pcm(FRAMES));
    let status = fixture.control.status();
    assert_ne!(status.fault_flags() & MONITOR_RT_FAULT_CONTROL_OVERFLOW, 0);
    assert_eq!(status.schedule_contract_violations(), 1);
    assert_eq!(status.applied_blocks(), 0);
}
