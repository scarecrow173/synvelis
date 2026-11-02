//! PC-B acceptance tests — `docs/test-specs/pc-b-e2e-parameter-dispatch.md`.
//!
//! PC-B-01: host-resolved values reach a real plugin endpoint through
//! the ruviform plugin bridge (`PluginHost` → `attach` → `PluginNode`
//! → `RtRuntime`), and rejections propagate as typed `param_*` tokens.
//! PC-B-04: the same dispatched event stream is delivered identically
//! under a playback-style `RtRuntime` drive and the offline `render()`
//! pipeline (absolute-frame comparison — call partitioning may differ).
//! Upstream `PluginNode` still reports `OfflineRenderSupport::Unknown`,
//! so the render drive installs a test-scoped `Verified` declaration
//! for this stateless, transport-deterministic fixture — the parity
//! assertion itself is the verification evidence. See the PC-B spec
//! follow-up note (rf#197 lineage).

use std::sync::{Arc, Mutex};

use ruviform_audio::render::{
    PreparedRender, RenderOutput, RenderSpec, RenderTransport, SilentFeeder, render,
};
use ruviform_audio::typed::{
    Compiler, CompilerConfig, DEFAULT_ALIGNMENT, FactoryError, GraphProcessor, GraphRuntime,
    InstalledInfo, NodeEvent, NodeIo, OfflineRenderSupport, PrepareOutcome,
};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{
    AttachConfig, DispatchStage, PluginNode, PluginNodeFactory, attach,
};
use ruviform_exec::{CancellationToken, Progress};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeKind, NodeSpec, PortId, PortRef,
    Validator,
};
use ruviform_io_wav::WavOutputSpec;
use ruviform_media_core::{
    BlockContract, ClockEpochId, DeliveryPolicy, EngineFrame, InBlockTransportSupport,
    LoopOccurrence, MediaTime, ScheduleBudget, ScheduledEvent, SpanContext, TempoMapRevision,
    TimeBase, TransportCommand, TransportGeneration, TransportScheduler, TransportSpan,
    TransportStatus,
};
use ruviform_media_io::{Muxer, PublishOptions, StagedOutput};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TimedParamEvent, TransportInfo, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::{DispatchError, ParamDispatch};
use synvelis_project::ParameterTargetId;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-b-rec",
    name: "PC-B Recorder",
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

/// One delivered parameter event at its absolute engine frame.
#[derive(Debug, Clone, PartialEq)]
struct Delivered {
    abs_frame: u64,
    param_id: u32,
    value: f64,
}

struct RecPlugin {
    delivered: Arc<Mutex<Vec<Delivered>>>,
    params: &'static [ParamInfo],
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
        for ev in events.iter() {
            if let EventKind::ParamValue(pe) = ev.kind() {
                #[allow(clippy::cast_sign_loss)]
                self.delivered.lock().unwrap().push(Delivered {
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
        self.params
    }
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
    }
}

struct RecFactory {
    delivered: Arc<Mutex<Vec<Delivered>>>,
    params: &'static [ParamInfo],
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        ruviform_plugin_core::erase(RecPlugin {
            delivered: Arc::clone(&self.delivered),
            params: self.params,
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
            delivered: Arc::clone(&self.delivered),
            params: self.params,
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
/// the runtime (playback `RtRuntime` or offline `PreparedRender`).
struct Hosted {
    _host: PluginHost,
    attached: ruviform_audio_plugin_bridge::Attached,
    delivered: Arc<Mutex<Vec<Delivered>>>,
}

fn host_plugin(params: &'static [ParamInfo]) -> Hosted {
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        delivered: Arc::clone(&delivered),
        params,
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
        _host: host,
        attached,
        delivered,
    }
}

/// Test-scoped `OfflineRenderSupport::Verified` declaration for the
/// fixture plugin node: the recorder is stateless apart from the
/// `delivered` log and transport-deterministic, so its sequential-
/// offline equivalence is verified by the PC-B-04 parity assertion
/// itself. Upstream `PluginNode` deliberately stays `Unknown` until
/// ruviform verifies the bridge node generically (rf#197 lineage).
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

/// `Source(2 outs) → plugin → Sink(2 ins)` — shared graph shape for
/// both drives. Returns (validated graph, plugin node id, src out
/// ports, sink in ports).
fn build_graph(
    attached: &ruviform_audio_plugin_bridge::Attached,
) -> (
    ruviform_graph::ValidatedGraph,
    ruviform_graph::NodeId,
    [PortId; 2],
    [PortId; 2],
) {
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

/// The lowered host-resolved writes both drives deliver.
fn writes() -> Vec<(ParameterTargetId, TimedParamEvent)> {
    let base = frame_base();
    let target = ParameterTargetId::new(1);
    [(64_i64, 0.25_f64), (128, 0.5), (192, 0.75), (320, 1.0)]
        .into_iter()
        .map(|(pos, v)| {
            (
                target,
                TimedParamEvent {
                    position: MediaTime::new(pos, base),
                    event: ParamEvent {
                        param_id: ParamId(7),
                        value: ParamValue::new(v).unwrap(),
                    },
                },
            )
        })
        .collect()
}

/// PC-B-01 — the real dispatch path: `ParamDispatch` → `install_plan`
/// → `PluginNode` internal scheduler → endpoint `ParamEvent`s at
/// their absolute frames, plus `Dispatched` ledger observations.
#[test]
fn pc_b_01_dispatch_delivers_param_events() {
    let hosted = host_plugin(&PARAMS);
    let (graph, proc, srcs, _sinks) = build_graph(&hosted.attached);
    let mut factory = PluginNodeFactory::new();
    factory.insert(proc, hosted.attached.node);
    let (mut runtime, mut rt) = GraphRuntime::new(&compiler_config(), 8, 8);
    let compiled = Compiler::new(compiler_config())
        .compile(
            &graph,
            &mut factory,
            runtime.registry(),
            runtime.allocator(),
            runtime.generation() + 1,
        )
        .expect("compiles");
    runtime.publish(compiled).expect("publish");

    let mut dispatch = ParamDispatch::new(hosted.attached.control, proc, frame_base());
    let target = ParameterTargetId::new(1);
    dispatch.bind(target, ParamId(7), &PARAMS).unwrap();
    let plan = dispatch.dispatch(&writes()).unwrap();

    let ctx = RtContext::mint();
    let zeros = vec![0.0f32; 128];
    for _ in 0..3 {
        let mut slot = rt.input_writer(srcs[0], 128).expect("input bound");
        for ch in 0..2 {
            slot.data.channel_mut(ch).unwrap()[..128].copy_from_slice(&zeros);
        }
        drop(slot);
        rt.process(128, &ctx).expect("process");
    }

    let got = hosted.delivered.lock().unwrap().clone();
    let expected: Vec<Delivered> = writes()
        .iter()
        .map(|(_, e)| Delivered {
            abs_frame: u64::try_from(e.position.value()).unwrap(),
            param_id: 7,
            value: e.event.value.get(),
        })
        .collect();
    assert_eq!(got, expected, "endpoint saw the dispatched events");

    // Ledger: all four events reached Dispatched.
    let span = TransportSpan {
        start: EngineFrame::new(ClockEpochId::new(0), 0),
        end: EngineFrame::new(ClockEpochId::new(0), 384),
        ctx: SpanContext {
            status: TransportStatus::Playing,
            position_ticks: 0,
            position_base: frame_base(),
            loop_ticks: None,
            occurrence: LoopOccurrence::new(0),
            clock_epoch: ClockEpochId::new(0),
            transport_generation: TransportGeneration::new(0),
            plan_generation: plan.generation,
            tempo_revision: TempoMapRevision::new(0),
        },
        deliverable: true,
    };
    let obs = dispatch.observe(&span, EngineFrame::new(ClockEpochId::new(0), 0), 384);
    assert_eq!(obs.len(), 1);
    assert!(matches!(
        dispatch.stage(target),
        Some(DispatchStage::Dispatched { .. })
    ));
}

/// PC-B-01 — rejections propagate as typed tokens: unknown param at
/// bind, unresolved/mismatched targets at dispatch. Nothing is staged.
#[test]
fn pc_b_01_rejections_propagate() {
    let hosted = host_plugin(&PARAMS);
    let (_graph, proc, _s, _k) = build_graph(&hosted.attached);
    let mut dispatch = ParamDispatch::new(hosted.attached.control, proc, frame_base());

    // Unknown param refused at bind with the upstream token.
    let err = dispatch
        .bind(ParameterTargetId::new(1), ParamId(99), &PARAMS)
        .unwrap_err();
    assert!(matches!(
        err,
        DispatchError::WriteRejected { ref rejection, .. } if rejection == "param_unknown"
    ));

    // Unbound target refused at dispatch — before anything is staged.
    let err = dispatch.dispatch(&writes()).unwrap_err();
    assert!(matches!(
        err,
        DispatchError::WriteRejected { ref rejection, .. } if rejection == "param_unresolved"
    ));

    // Bound to param 7 but lowered for a different param id →
    // param_mismatch, not a silent redirect.
    dispatch
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .unwrap();
    let mut bad = writes();
    bad[0].1.event.param_id = ParamId(8);
    let err = dispatch.dispatch(&bad).unwrap_err();
    assert!(matches!(
        err,
        DispatchError::WriteRejected { ref rejection, .. } if rejection == "param_mismatch"
    ));

    // Bounded control queue: enough staged requests hit QueueFull as
    // a typed error, never a silent drop.
    let mut queue_err = None;
    for _ in 0..16_384 {
        if dispatch
            .control()
            .request(TransportCommand::Play, 0)
            .is_err()
        {
            queue_err = Some(());
            break;
        }
    }
    assert!(queue_err.is_some(), "bounded queue must report QueueFull");
}

/// PC-B-04 — the same dispatched stream lands identically under the
/// playback-style block drive and the offline `render()` pipeline.
#[test]
#[expect(clippy::too_many_lines)]
fn pc_b_04_offline_render_matches_playback() {
    // ---- playback-style drive ---------------------------------------
    let hosted_a = host_plugin(&PARAMS);
    let (graph_a, proc_a, srcs_a, _s) = build_graph(&hosted_a.attached);
    let mut factory_a = PluginNodeFactory::new();
    factory_a.insert(proc_a, hosted_a.attached.node);
    let (mut runtime_a, mut rt_a) = GraphRuntime::new(&compiler_config(), 8, 8);
    let compiled_a = Compiler::new(compiler_config())
        .compile(
            &graph_a,
            &mut factory_a,
            runtime_a.registry(),
            runtime_a.allocator(),
            runtime_a.generation() + 1,
        )
        .expect("compiles");
    runtime_a.publish(compiled_a).expect("publish");
    let mut dispatch_a = ParamDispatch::new(hosted_a.attached.control, proc_a, frame_base());
    dispatch_a
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .unwrap();
    dispatch_a.dispatch(&writes()).unwrap();
    let ctx = RtContext::mint();
    let zeros = vec![0.0f32; 128];
    for _ in 0..3 {
        let mut slot = rt_a.input_writer(srcs_a[0], 128).expect("input bound");
        for ch in 0..2 {
            slot.data.channel_mut(ch).unwrap()[..128].copy_from_slice(&zeros);
        }
        drop(slot);
        rt_a.process(128, &ctx).expect("process");
    }
    let playback_events = hosted_a.delivered.lock().unwrap().clone();

    // ---- offline render drive ---------------------------------------
    let hosted_b = host_plugin(&PARAMS);
    // Upstream does not declare offline support for the bridge node;
    // the wrapper below is this fixture's own instance-level claim.
    assert_eq!(
        hosted_b.attached.node.offline_support(),
        OfflineRenderSupport::Unknown
    );
    let (graph_b, proc_b, _sb, sinks_b) = build_graph(&hosted_b.attached);
    let mut wrapped = Some(VerifiedOfflineNode(hosted_b.attached.node));
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
    let mut dispatch_b = ParamDispatch::new(hosted_b.attached.control, proc_b, frame_base());
    dispatch_b
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .unwrap();
    dispatch_b.dispatch(&writes()).unwrap();

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
        range_end: 384,
        tail: ruviform_audio::render::TailSpec::None,
        job_start: 0,
        job_end: 384,
        callback_chunk: 64,
    };
    let dest =
        std::env::temp_dir().join(format!("synvelis-pc-b-parity-{}.wav", std::process::id()));
    let staged = StagedOutput::create(&dest).expect("staged output");
    let info = WavOutputSpec {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
    .stream_info();
    let encoder = ruviform_io_wav::make_encoder(&info).expect("encoder");
    let muxer_info = info.clone();
    let output = RenderOutput {
        staged,
        make_muxer: Box::new(move |file| {
            ruviform_io_wav::open_muxer(file, std::slice::from_ref(&muxer_info))
                .map(|m| Box::new(m) as Box<dyn Muxer>)
        }),
        encoder: Box::new(encoder),
        publish_options: PublishOptions {
            replace_existing: true,
            sync_before_publish: false,
        },
        verify: None::<
            Box<
                dyn FnOnce(
                    &std::path::Path,
                    &ruviform_audio::render::OutputFacts,
                ) -> Result<(), ruviform_media_io::Error>,
            >,
        >,
        publish: Box::new(StagedOutput::publish),
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
        "playback drive and offline render deliver the same stream"
    );
    assert_eq!(playback_events.len(), 4);
    let _ = std::fs::remove_file(&dest);
}
