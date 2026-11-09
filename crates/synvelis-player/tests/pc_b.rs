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
use ruviform_io_file::{Durability, PublishMode, PublishOptions, SealedFile, StagedFile};
use ruviform_io_wav::WavOutputSpec;
use ruviform_media_core::{
    BlockContract, ClockEpochId, DeliveryPolicy, EngineFrame, InBlockTransportSupport,
    LoopOccurrence, MediaTime, ScheduleBudget, ScheduledEvent, SpanContext, TempoMapRevision,
    TimeBase, TransportCommand, TransportGeneration, TransportScheduler, TransportSpan,
    TransportStatus,
};
use ruviform_media_io::Muxer;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TimedParamEvent, TransportInfo, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::driver::ActivateError;
use synvelis_player::lowering::{EngineConfig, lower_with_inserts};
use synvelis_player::{AttachedNode, DispatchError, HeadlessDriver, ParamDispatch, PumpPolicy};
use synvelis_project::{ParameterTargetId, Project, ProjectId, SourceId};

/// Muxer constructor over the `&mut StagedFile` borrow — the
/// `RenderOutput::make_muxer` shape for the offline-render parity
/// drive.
type DynMuxerCtor = Box<
    dyn for<'a> FnOnce(&'a mut StagedFile) -> Result<Box<dyn Muxer + 'a>, ruviform_media_io::Error>
        + Send,
>;

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

    let mut dispatch = ParamDispatch::new(hosted.attached.control, proc, frame_base()).unwrap();
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
    let mut dispatch = ParamDispatch::new(hosted.attached.control, proc, frame_base()).unwrap();

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
    let mut dispatch_a =
        ParamDispatch::new(hosted_a.attached.control, proc_a, frame_base()).unwrap();
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
    let mut dispatch_b =
        ParamDispatch::new(hosted_b.attached.control, proc_b, frame_base()).unwrap();
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
        "playback drive and offline render deliver the same stream"
    );
    assert_eq!(playback_events.len(), 4);
    let _ = std::fs::remove_file(&dest);
}

/// PC-B-05 — `ParamDispatch` wired through `HeadlessDriver`: the
/// attached plugin installs into a lowered `Processor` insert slot,
/// per-block `observe` advances the ledger to `Dispatched`, an empty
/// batch stages nothing, and `seek` re-anchors the node's cursor +
/// position so absolute-frame events still land after the jump.
#[test]
fn pc_b_05_headless_driver_dispatch_and_seek() {
    let hosted = host_plugin(&PARAMS);
    let project = Project::new(ProjectId::new(0), "drv");
    let engine = EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 128,
    };
    let lowered = lower_with_inserts(
        &project,
        &|_: SourceId| -> Option<AudioFormat> { None },
        &engine,
        1,
    );
    assert!(lowered.playbacks.is_empty());
    let insert = lowered.inserts[0].node_id;

    let mut driver = HeadlessDriver::new(engine);
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, insert, frame_base())
                    .unwrap(),
            }],
        )
        .expect("activate");

    let target = ParameterTargetId::new(1);
    {
        let d = driver.attached_dispatch(insert).expect("dispatch bound");
        // An empty batch stages nothing — no plan minted, no Play.
        assert!(matches!(d.dispatch(&[]), Err(DispatchError::Empty)));
        assert_eq!(d.last_plan_generation(), None);
        d.bind(target, ParamId(7), &PARAMS).unwrap();
        d.dispatch(&writes()).unwrap();
    }

    driver.run(3, PumpPolicy::Every(1)).expect("run");

    let expected: Vec<Delivered> = writes()
        .iter()
        .map(|(_, e)| Delivered {
            abs_frame: u64::try_from(e.position.value()).unwrap(),
            param_id: 7,
            value: e.event.value.get(),
        })
        .collect();
    assert_eq!(
        *hosted.delivered.lock().unwrap(),
        expected,
        "endpoint saw the dispatched events through the driver"
    );
    // Per-block observe advanced the ledger — the merged observation
    // map reflects the last call covering the stream.
    assert!(matches!(
        driver.param_observations().get(&target).map(|o| o.stage),
        Some(DispatchStage::Dispatched { .. })
    ));

    // Seek re-anchors node cursor + position: a fresh plan's absolute
    // frame events land on the new frame series — without the
    // `set_clock`/`Seek` pair the node would keep running at frame 384.
    driver.seek(0);
    let writes2: Vec<(ParameterTargetId, TimedParamEvent)> = [(64_i64, 0.3_f64), (192, 0.6)]
        .into_iter()
        .map(|(pos, v)| {
            (
                target,
                TimedParamEvent {
                    position: MediaTime::new(pos, frame_base()),
                    event: ParamEvent {
                        param_id: ParamId(7),
                        value: ParamValue::new(v).unwrap(),
                    },
                },
            )
        })
        .collect();
    driver
        .attached_dispatch(insert)
        .unwrap()
        .dispatch(&writes2)
        .unwrap();
    driver.run(2, PumpPolicy::Every(1)).expect("run");

    let mut all = expected;
    all.extend(writes2.iter().map(|(_, e)| Delivered {
        abs_frame: u64::try_from(e.position.value()).unwrap(),
        param_id: 7,
        value: e.event.value.get(),
    }));
    assert_eq!(
        *hosted.delivered.lock().unwrap(),
        all,
        "post-seek events landed at their absolute engine frames"
    );
}

/// PC-B-05 — an [`AttachedNode`] must fill a `Processor` slot in the
/// spec: two dispatchers claiming the same insert are refused as
/// [`ActivateError::UnmatchedAttached`] before anything publishes, and
/// the driver's last-good state is untouched.
#[test]
fn pc_b_05_unmatched_attached_rejected() {
    let project = Project::new(ProjectId::new(0), "drv");
    let engine = EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 128,
    };
    let lowered = lower_with_inserts(
        &project,
        &|_: SourceId| -> Option<AudioFormat> { None },
        &engine,
        1,
    );
    let insert = lowered.inserts[0].node_id;
    let a = host_plugin(&PARAMS);
    let b = host_plugin(&PARAMS);
    let mut driver = HeadlessDriver::new(engine);
    let err = driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![
                AttachedNode {
                    node: a.attached.node,
                    dispatch: ParamDispatch::new(a.attached.control, insert, frame_base()).unwrap(),
                },
                AttachedNode {
                    node: b.attached.node,
                    dispatch: ParamDispatch::new(b.attached.control, insert, frame_base()).unwrap(),
                },
            ],
        )
        .unwrap_err();
    assert!(matches!(
        err,
        ActivateError::UnmatchedAttached(id) if id == insert
    ));
    assert_eq!(driver.published_revision(), None);
    assert_eq!(driver.state(), synvelis_player::DriverState::Stopped);
}

/// Issue #64 review: `activate_with_attached` must re-anchor a fresh
/// node's cursor to the driver cursor — a node first activated
/// mid-run starts at frame 0, and without the `set_clock`/`Seek`
/// pair its position series lags the driver's by the cursor at
/// activation: absolute-frame events would land late while the
/// ledger's fabricated spans claimed `Dispatched`.
#[test]
fn pc_b_05_midrun_activation_reanchors_node() {
    let engine = EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 128,
    };
    let project = Project::new(ProjectId::new(0), "drv");
    let lowered = lower_with_inserts(
        &project,
        &|_: SourceId| -> Option<AudioFormat> { None },
        &engine,
        1,
    );
    let insert = lowered.inserts[0].node_id;
    let first = host_plugin(&PARAMS);
    let second = host_plugin(&PARAMS);

    let mut driver = HeadlessDriver::new(engine);
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: first.attached.node,
                dispatch: ParamDispatch::new(first.attached.control, insert, frame_base()).unwrap(),
            }],
        )
        .expect("activate");
    driver.run(3, PumpPolicy::Every(1)).expect("run");
    assert_eq!(driver.cursor(), 384);

    // Re-activate mid-run with a fresh node (e.g. an insert swap):
    // its transport must continue at driver frame 384, not restart
    // at node frame 0.
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: second.attached.node,
                dispatch: ParamDispatch::new(second.attached.control, insert, frame_base())
                    .unwrap(),
            }],
        )
        .expect("re-activate");
    let target = ParameterTargetId::new(1);
    let event = |pos: i64, value: f64| {
        (
            target,
            TimedParamEvent {
                position: MediaTime::new(pos, frame_base()),
                event: ParamEvent {
                    param_id: ParamId(7),
                    value: ParamValue::new(value).unwrap(),
                },
            },
        )
    };
    {
        let d = driver.attached_dispatch(insert).unwrap();
        d.bind(target, ParamId(7), &PARAMS).unwrap();
        d.dispatch(&[event(448, 0.5)]).unwrap();
    }
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    assert_eq!(
        *second.delivered.lock().unwrap(),
        vec![Delivered {
            abs_frame: 448,
            param_id: 7,
            value: 0.5,
        }],
        "event lands on the driver's frame series, not the node's"
    );
    assert!(matches!(
        driver.param_observations().get(&target).map(|o| o.stage),
        Some(DispatchStage::Dispatched { .. })
    ));
}

/// Issue #64 review: a `dispatch` staged *before* `seek` must drain
/// at the old cursor — node controls compute their due frame at pop
/// time, so without the quiesce pass a plan queued before the rewind
/// installed mid-rewind and the old plan re-fired. `seek`'s quiesce
/// pass drains pending controls first, then re-anchors.
#[test]
fn pc_b_05_dispatch_before_seek_drains_at_old_cursor() {
    let hosted = host_plugin(&PARAMS);
    let project = Project::new(ProjectId::new(0), "drv");
    let engine = EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 128,
    };
    let lowered = lower_with_inserts(
        &project,
        &|_: SourceId| -> Option<AudioFormat> { None },
        &engine,
        1,
    );
    let insert = lowered.inserts[0].node_id;

    let mut driver = HeadlessDriver::new(engine);
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, insert, frame_base())
                    .unwrap(),
            }],
        )
        .expect("activate");

    let target = ParameterTargetId::new(1);
    {
        let d = driver.attached_dispatch(insert).unwrap();
        d.bind(target, ParamId(7), &PARAMS).unwrap();
        d.dispatch(&writes()).unwrap(); // plan A: 64, 128, 192, 320
    }
    driver.run(3, PumpPolicy::Every(1)).expect("run");
    let base_len = hosted.delivered.lock().unwrap().len();
    assert_eq!(base_len, 4, "plan A delivered");

    // Stage plan B *before* the seek — the pre-fix bug installed it
    // mid-rewind while plan A re-fired at the rewound positions.
    let writes_b: Vec<(ParameterTargetId, TimedParamEvent)> = [(32_i64, 0.3_f64), (96, 0.6)]
        .into_iter()
        .map(|(pos, v)| {
            (
                target,
                TimedParamEvent {
                    position: MediaTime::new(pos, frame_base()),
                    event: ParamEvent {
                        param_id: ParamId(7),
                        value: ParamValue::new(v).unwrap(),
                    },
                },
            )
        })
        .collect();
    driver
        .attached_dispatch(insert)
        .unwrap()
        .dispatch(&writes_b)
        .unwrap();
    driver.seek(0);
    driver.run(2, PumpPolicy::Every(1)).expect("run");

    let all = hosted.delivered.lock().unwrap().clone();
    let extra = &all[base_len..];
    let mut expected_extra: Vec<Delivered> = writes_b
        .iter()
        .map(|(_, e)| Delivered {
            abs_frame: u64::try_from(e.position.value()).unwrap(),
            param_id: 7,
            value: e.event.value.get(),
        })
        .collect();
    expected_extra.sort_by_key(|d| d.abs_frame);
    let mut extra_sorted = extra.to_vec();
    extra_sorted.sort_by_key(|d| d.abs_frame);
    assert_eq!(
        extra_sorted, expected_extra,
        "plan B delivers post-seek; plan A never re-fires"
    );
    assert_eq!(
        extra.len(),
        expected_extra.len(),
        "no duplicate deliveries from the rewind window"
    );
}

/// Issue #64 review: an `install_plan` apply rejected inside the
/// scheduler (`ScheduleBudget::max_pending_applies = 16`) must not
/// mint `Dispatched` — `observe` gates projection on the plan
/// generation the node's receipts report live. Nine dispatches in
/// one inter-callback window stage 18 applies; the ninth plan's
/// `Play`+`install_plan` are rejected, the receipts keep reporting
/// the eighth generation, and plan-9 entries stay `Scheduled`.
#[test]
fn pc_b_05_rejected_install_never_projects_dispatched() {
    let hosted = host_plugin(&PARAMS);
    let project = Project::new(ProjectId::new(0), "drv");
    let engine = EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 128,
    };
    let lowered = lower_with_inserts(
        &project,
        &|_: SourceId| -> Option<AudioFormat> { None },
        &engine,
        1,
    );
    let insert = lowered.inserts[0].node_id;

    let mut driver = HeadlessDriver::new(engine);
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, insert, frame_base())
                    .unwrap(),
            }],
        )
        .expect("activate");

    let target = ParameterTargetId::new(1);
    {
        let d = driver.attached_dispatch(insert).unwrap();
        d.bind(target, ParamId(7), &PARAMS).unwrap();
        for _ in 0..9 {
            d.dispatch(&writes()).unwrap();
        }
    }
    driver.run(1, PumpPolicy::Every(1)).expect("run");

    // The ninth install never applied — projection must not claim
    // `Dispatched` for events the plugin never received.
    let dispatch = driver.attached_dispatch(insert).unwrap();
    assert!(
        matches!(dispatch.stage(target), Some(DispatchStage::Scheduled)),
        "rejected plan stays Scheduled, got {:?}",
        dispatch.stage(target)
    );
}
