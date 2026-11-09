//! PC-C4 acceptance tests — `docs/test-specs/pc-c4-e2e-plugin-param-binding.md`.
//!
//! PLUG-C4-01: eval → bind → dispatch → delivered frame/value receipt
//! matches between the playback-style `RtRuntime` drive and the
//! offline `render()` pipeline (same verified attach context).
//! PLUG-C4-02: mismatched time base, same raw `ParamId` on another
//! instance, removed/read-only params, unsupported means and queue
//! full are all typed rejects — wrong-target delivery stays 0.
//! PLUG-C4-03: automation+manual+LFO/Macro/StepSeq/depth-driver
//! composite evaluation, DAG cycle rejection, discrete-kind
//! modulation refusal.
//! PLUG-C4-04: re-attach / generation drift / invalidate / stale plan
//! fail closed; transport commands never invalidate bindings.
//! PLUG-C4-05: Ruviform #307 attach-time frame-base contract consumed
//! through `frame_base()`/`check_frame_base()` — real `VST3` is `NotRun`.

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
    AttachConfig, DispatchStage, FrameBaseMismatch, PluginNode, PluginNodeControl,
    PluginNodeFactory, attach,
};
use ruviform_exec::{CancellationToken, Progress};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, NodeSpec, PortId,
    PortRef, Validator,
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
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, ParamWriteMeans, Plugin,
    PluginDescriptor, PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext,
    ProcessResult, TimedParamEvent, TransportInfo, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::{
    AttachClockContext, BindingError, DispatchError, ParamDispatch, PluginBindingSession,
    PluginParameterBinding, StablePluginInstanceId,
};
use synvelis_project::{
    AutomationLane, AutomationLaneId, AutomationPoint, AutomationRegion, AutomationTimeScope,
    BeatPoint, BeatRange, ControlGraph, ControlOwner, Interpolation, LfoShape,
    ModulationAssignment, ModulationAssignmentId, ModulationSource, ModulationSourceId,
    ParamMapping, ParameterRole, ParameterTarget, ParameterTargetId, Polarity, ProcessorId,
    Project, ProjectId, SourceKind, TargetCapability, Transform, ValueKind,
};

/// Muxer constructor over the `&mut StagedFile` borrow — same shape as
/// the PC-B parity drive.
type DynMuxerCtor = Box<
    dyn for<'a> FnOnce(&'a mut StagedFile) -> Result<Box<dyn Muxer + 'a>, ruviform_media_io::Error>
        + Send,
>;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c4-rec",
    name: "PC-C4 Recorder",
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

/// Two params: `7` automatable continuous, `9` automatable+stepped.
const PARAMS: [ParamInfo; 2] = [
    ParamInfo {
        id: ParamId(7),
        name: "Gain",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE,
    },
    ParamInfo {
        id: ParamId(9),
        name: "Mode",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED),
    },
];

/// A read-only param — the capability gate must refuse writes.
const READONLY_PARAMS: [ParamInfo; 1] = [ParamInfo {
    id: ParamId(7),
    name: "Gain",
    unit: "",
    min_value: 0.0,
    max_value: 1.0,
    default_value: 0.0,
    flags: ParamFlags::READONLY,
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

const PROC: u128 = 42;

fn processor() -> ProcessorId {
    ProcessorId::new(PROC)
}

/// A hosted fixture plugin + its `attached` parts.
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

/// Test-scoped `OfflineRenderSupport::Verified` declaration — same
/// rationale as PC-B-04 (upstream `PluginNode` stays `Unknown`).
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

/// `Source(2 outs) → plugin → Sink(2 ins)` — shared graph shape.
fn build_graph(
    attached: &ruviform_audio_plugin_bridge::Attached,
) -> (
    ruviform_graph::ValidatedGraph,
    NodeId,
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

/// A `ProcessorParam` target owned by `processor()` whose declared
/// capability matches `PARAMS[0]` (param 7, automatable).
fn param_target(
    id: u128,
    param: u32,
    flags: ParamFlags,
    kind: ValueKind,
    base: f64,
) -> ParameterTarget {
    ParameterTarget {
        id: ParameterTargetId::new(id),
        owner: ControlOwner::Processor(processor()),
        role: ParameterRole::ProcessorParam,
        time_scope: AutomationTimeScope::ProjectMusical,
        name: "P".into(),
        kind,
        mapping: ParamMapping::Linear,
        capability: Some(TargetCapability {
            param_id: param,
            flags_bits: flags.bits(),
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
        }),
        base,
    }
}

fn gain_target(id: u128) -> ParameterTarget {
    param_target(id, 7, ParamFlags::AUTOMATABLE, ValueKind::Continuous, 0.0)
}

/// A project carrying `targets` (the master-fader bootstrap target
/// stays — it is `Strip`-owned and can never bind to a plugin).
fn project_with(targets: Vec<ParameterTarget>) -> Project {
    let mut project = Project::new(ProjectId::new(0), "pcc4");
    for target in targets {
        project.control.targets.insert(target.id, target);
    }
    project
}

/// Opens a session + dispatcher on `control`: the session mints its
/// verified clock context from `control.frame_base()` before the
/// control moves into `ParamDispatch::new` (which re-verifies the same
/// base). Returns both — `bind` is the caller's next step so failure
/// cases can assert the typed error.
fn open(
    control: PluginNodeControl,
    node: NodeId,
    instance: u64,
    cap_gen: u64,
    graph_gen: u64,
) -> (PluginBindingSession, ParamDispatch) {
    let session = PluginBindingSession::attach(
        &control,
        node,
        processor(),
        graph_gen,
        cap_gen,
        StablePluginInstanceId::new(instance),
    );
    let dispatch = ParamDispatch::new(control, node, frame_base()).expect("frame base ok");
    (session, dispatch)
}

/// Expected delivered stream for `events`.
fn expected(events: &[TimedParamEvent]) -> Vec<Delivered> {
    events
        .iter()
        .map(|e| Delivered {
            abs_frame: u64::try_from(e.position.value()).unwrap(),
            param_id: e.event.param_id.0,
            value: e.event.value.get(),
        })
        .collect()
}

/// Full playback drive: compile `graph` with the plugin node, publish,
/// run 3×128-frame blocks. Delivered events land in the fixture's
/// shared log.
fn playback_delivered(
    graph: &ruviform_graph::ValidatedGraph,
    node: PluginNode,
    proc: NodeId,
    srcs: [PortId; 2],
) {
    let mut factory = PluginNodeFactory::new();
    factory.insert(proc, node);
    let (mut runtime, mut rt) = GraphRuntime::new(&compiler_config(), 8, 8);
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
}

/// Offline `render()` drive over `range_end` frames — same delivery
/// path as `playback_delivered`.
fn offline_delivered(
    graph: &ruviform_graph::ValidatedGraph,
    node: PluginNode,
    proc: NodeId,
    sink_port: PortId,
    range_end: u64,
) {
    let mut wrapped = Some(VerifiedOfflineNode(node));
    let mut factory = move |spec: &NodeSpec,
                            _existing: Option<&InstalledInfo>|
          -> Result<PrepareOutcome, FactoryError> {
        if spec.kind() == NodeKind::Processor {
            assert_eq!(spec.id(), proc);
            Ok(PrepareOutcome::New(Box::new(
                wrapped.take().expect("one processor node"),
            )))
        } else {
            Ok(PrepareOutcome::Boundary)
        }
    };
    let mut prepared = PreparedRender::compile(graph, &mut factory, compiler_config(), 8, 8)
        .expect("prepared render");
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
        output_port: sink_port,
        range_start: 0,
        range_end,
        tail: ruviform_audio::render::TailSpec::None,
        job_start: 0,
        job_end: range_end,
        callback_chunk: 64,
    };
    let dest =
        std::env::temp_dir().join(format!("synvelis-pc-c4-parity-{}.wav", std::process::id()));
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
    let _ = std::fs::remove_file(&dest);
}

/// PC-C4-05 — the attach-time frame base is the only verified clock
/// context: `AttachClockContext` mints from `frame_base()`, rejects a
/// foreign base with the upstream `FrameBaseMismatch`, and
/// `ParamDispatch::new` refuses the same mismatch before staging.
#[test]
fn pc_c4_05_verified_frame_base_contract() {
    let hosted = host_plugin(&PARAMS);
    let (_g, node, _s, _k) = build_graph(&hosted.attached);

    let clock = AttachClockContext::of(&hosted.attached.control);
    assert_eq!(clock.frame_base(), frame_base());
    assert_eq!(clock.frame_base(), hosted.attached.control.frame_base());
    clock.check_frame_base(frame_base()).unwrap();

    let foreign = TimeBase::sample_rate(44_100).unwrap();
    let err = clock.check_frame_base(foreign).unwrap_err();
    assert_eq!(
        err,
        FrameBaseMismatch {
            expected: frame_base(),
            actual: foreign,
        }
    );

    // `ParamDispatch::new` verifies the same contract at construction —
    // a foreign base is refused before any plan exists.
    let foreign_hosted = host_plugin(&PARAMS);
    let err = ParamDispatch::new(foreign_hosted.attached.control, node, foreign)
        .err()
        .expect("foreign frame base must be rejected");
    assert!(matches!(
        err,
        DispatchError::FrameBaseMismatch(m)
            if m.expected == frame_base() && m.actual == foreign
    ));

    // The session pins the verified base; `is_current` tracks the two
    // generation axes independently of it.
    let session = PluginBindingSession::attach(
        &hosted.attached.control,
        node,
        processor(),
        1,
        1,
        StablePluginInstanceId::new(1),
    );
    assert_eq!(session.frame_base(), frame_base());
    assert_eq!(session.node(), node);
    assert_eq!(session.instance_id(), StablePluginInstanceId::new(1));
    assert!(session.is_current(1, 1));
    assert!(!session.is_current(2, 1));
    assert!(!session.is_current(1, 2));
}

/// PC-C4-01/PL06/PL08 — same attach context + matching time base:
/// `eval_event → session.dispatch → install_plan → endpoint` delivers
/// identical absolute-frame/value streams under the RT-style block
/// drive and the offline `render()` pipeline, including an event at a
/// non-block-aligned frame (min-process offset).
#[test]
#[expect(clippy::too_many_lines)]
fn pc_c4_01_eval_dispatch_receipt_rt_offline_parity() {
    // Automation: linear 0.2 → 0.6 over beats [0, 4); manual override
    // is exercised by one eval position.
    let target = gain_target(1);
    let mut project = project_with(vec![target.clone()]);
    project.control.lanes.insert(
        AutomationLaneId::new(1),
        AutomationLane {
            id: AutomationLaneId::new(1),
            target: target.id,
            regions: vec![AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(0, 1).unwrap(),
                        value: 0.2,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(4, 1).unwrap(),
                        value: 0.6,
                    },
                ],
            }],
        },
    );
    let plan = ControlGraph::compile(&project).expect("compiles");
    let target_id = target.id;
    let spec = [
        (BeatPoint::new(2, 1).unwrap(), None, 64_i64),
        (BeatPoint::new(2, 1).unwrap(), Some(0.8), 100),
        (BeatPoint::new(1, 1).unwrap(), None, 200),
        (BeatPoint::new(3, 1).unwrap(), None, 320),
    ];

    // ---- playback-style drive --------------------------------------
    let hosted_a = host_plugin(&PARAMS);
    let (graph_a, proc_a, srcs_a, _sinks_a) = build_graph(&hosted_a.attached);
    let delivered_a = Arc::clone(&hosted_a.delivered);
    let node_a = hosted_a.attached.node;
    let (mut session_a, mut dispatch_a) = open(hosted_a.attached.control, proc_a, 1, 1, 1);
    let binding_a = session_a
        .bind(&mut dispatch_a, &project, target_id, &PARAMS)
        .unwrap();
    session_a.check_plan(&plan, &project).unwrap();
    let mut scratch_a = plan.scratch();
    let events_a: Vec<(PluginParameterBinding, TimedParamEvent)> = spec
        .iter()
        .map(|&(beat, manual, frame)| {
            let ev = session_a
                .eval_event(
                    &project,
                    &plan,
                    &mut scratch_a,
                    target_id,
                    beat,
                    manual,
                    MediaTime::new(frame, frame_base()),
                    ParamWriteMeans::TimedEvent,
                )
                .unwrap();
            (binding_a.clone(), ev)
        })
        .collect();
    let staged_a = session_a.dispatch(&mut dispatch_a, &events_a).unwrap();

    // Hand-checkable expectations: beat2 automation = 0.4, manual = 0.8,
    // beat1 = 0.3, beat3 = 0.5 (linear 0.2→0.6 over [0,4)).
    let values: Vec<f64> = events_a.iter().map(|(_, e)| e.event.value.get()).collect();
    assert_eq!(values, vec![0.4, 0.8, 0.3, 0.5]);

    playback_delivered(&graph_a, node_a, proc_a, srcs_a);
    let playback_events = delivered_a.lock().unwrap().clone();
    let evs_a: Vec<TimedParamEvent> = events_a.iter().map(|(_, e)| *e).collect();
    assert_eq!(
        playback_events,
        expected(&evs_a),
        "endpoint saw eval→dispatch events at absolute frames"
    );
    assert!(playback_events.iter().any(|d| d.abs_frame == 100));

    // Ledger reached Dispatched through the session-driven plan.
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
            plan_generation: staged_a.generation,
            tempo_revision: TempoMapRevision::new(0),
        },
        deliverable: true,
    };
    let _ = dispatch_a.observe(&span, EngineFrame::new(ClockEpochId::new(0), 0), 384);
    assert!(matches!(
        dispatch_a.stage(target_id),
        Some(DispatchStage::Dispatched { .. })
    ));

    // ---- offline render drive --------------------------------------
    let hosted_b = host_plugin(&PARAMS);
    let (graph_b, proc_b, _srcs_b, sinks_b) = build_graph(&hosted_b.attached);
    let delivered_b = Arc::clone(&hosted_b.delivered);
    let node_b = hosted_b.attached.node;
    let (mut session_b, mut dispatch_b) = open(hosted_b.attached.control, proc_b, 2, 1, 1);
    let binding_b = session_b
        .bind(&mut dispatch_b, &project, target_id, &PARAMS)
        .unwrap();
    let mut scratch_b = plan.scratch();
    let events_b: Vec<(PluginParameterBinding, TimedParamEvent)> = spec
        .iter()
        .map(|&(beat, manual, frame)| {
            let ev = session_b
                .eval_event(
                    &project,
                    &plan,
                    &mut scratch_b,
                    target_id,
                    beat,
                    manual,
                    MediaTime::new(frame, frame_base()),
                    ParamWriteMeans::TimedEvent,
                )
                .unwrap();
            (binding_b.clone(), ev)
        })
        .collect();
    session_b.dispatch(&mut dispatch_b, &events_b).unwrap();
    offline_delivered(&graph_b, node_b, proc_b, sinks_b[0], 384);
    let render_events = delivered_b.lock().unwrap().clone();

    assert_eq!(
        playback_events, render_events,
        "RT drive and offline render deliver the same stream"
    );
}

/// PC-C4-02 — typed rejects with zero wrong-target delivery:
/// foreign-instance record, generation drift, defunct/drifted record,
/// param removed, read-only capability, unsupported means, queue full.
#[test]
#[expect(clippy::too_many_lines)]
fn pc_c4_02_typed_rejects_no_misdelivery() {
    let target = gain_target(1);
    let project = project_with(vec![target.clone()]);
    let target_id = target.id;

    // Two hosted instances sharing the same raw ParamId(7) — the
    // "same raw id, different instance" case.
    let hosted_a = host_plugin(&PARAMS);
    let (_ga, proc_a, _sa, _ka) = build_graph(&hosted_a.attached);
    let delivered_a = Arc::clone(&hosted_a.delivered);
    let (mut session_a, mut dispatch_a) = open(hosted_a.attached.control, proc_a, 1, 1, 1);
    let binding_a = session_a
        .bind(&mut dispatch_a, &project, target_id, &PARAMS)
        .unwrap();
    let hosted_b = host_plugin(&PARAMS);
    let (_gb, proc_b, _sb, _kb) = build_graph(&hosted_b.attached);
    let delivered_b = Arc::clone(&hosted_b.delivered);
    let (mut session_b, mut dispatch_b) = open(hosted_b.attached.control, proc_b, 2, 1, 1);
    session_b
        .bind(&mut dispatch_b, &project, target_id, &PARAMS)
        .unwrap();

    let foreign_event = TimedParamEvent {
        position: MediaTime::new(64, frame_base()),
        event: ParamEvent {
            param_id: ParamId(7),
            value: ParamValue::new(0.5).unwrap(),
        },
    };

    // A record minted by session A can never dispatch through session
    // B's node — InstanceMismatch before anything is staged.
    let err = session_b
        .dispatch(&mut dispatch_b, &[(binding_a.clone(), foreign_event)])
        .unwrap_err();
    assert!(matches!(err, BindingError::InstanceMismatch));
    assert_eq!(dispatch_b.last_plan_generation(), None);
    assert!(delivered_b.lock().unwrap().is_empty());

    // Generation drift on an otherwise valid record → typed reject.
    let mut stale_cap = binding_a.clone();
    stale_cap.capability_generation = 99;
    let err = session_a
        .dispatch(&mut dispatch_a, &[(stale_cap.clone(), foreign_event)])
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::CapabilityGeneration {
            expected: 1,
            actual: 99
        }
    ));
    let mut stale_graph = binding_a.clone();
    stale_graph.graph_generation = 77;
    let err = session_a
        .dispatch(&mut dispatch_a, &[(stale_graph, foreign_event)])
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::GraphGeneration {
            expected: 1,
            actual: 77
        }
    ));

    // A record whose param pin drifted from the session's recorded
    // binding → BindingDefunct.
    let mut drifted = binding_a.clone();
    drifted.plugin_param = ParamId(9);
    let err = session_a
        .dispatch(&mut dispatch_a, &[(drifted, foreign_event)])
        .unwrap_err();
    assert!(matches!(err, BindingError::BindingDefunct));

    // Event addressed at a different param than the pinned binding →
    // param_mismatch (never a silent redirect).
    let mut wrong_param = foreign_event;
    wrong_param.event.param_id = ParamId(9);
    let err = session_a
        .dispatch(&mut dispatch_a, &[(binding_a.clone(), wrong_param)])
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::Dispatch(DispatchError::WriteRejected { ref rejection, .. })
            if rejection == "param_mismatch"
    ));

    // Atomic batch: one good + one stale record → whole batch
    // rejected, nothing staged.
    let err = session_a
        .dispatch(
            &mut dispatch_a,
            &[
                (binding_a.clone(), foreign_event),
                (stale_cap, foreign_event),
            ],
        )
        .unwrap_err();
    assert!(matches!(err, BindingError::CapabilityGeneration { .. }));
    assert_eq!(dispatch_a.last_plan_generation(), None);
    assert!(delivered_a.lock().unwrap().is_empty());

    // Param removed: revalidate against an enumeration without param 7
    // flips the binding defunct; the session drops its record.
    assert!(!session_a.revalidate(&mut dispatch_a, target_id, &PARAMS[1..]));
    let err = session_a
        .dispatch(&mut dispatch_a, &[(binding_a.clone(), foreign_event)])
        .unwrap_err();
    assert!(matches!(err, BindingError::TargetNotBound));
    // Re-bind against the shrunken enumeration fails typed too.
    let err = session_a
        .bind(&mut dispatch_a, &project, target_id, &PARAMS[1..])
        .unwrap_err();
    assert!(matches!(err, BindingError::ParamRemoved));

    // Read-only live param with a matching declared capability →
    // param_readonly at bind (upstream token preserved).
    let readonly_target = param_target(2, 7, ParamFlags::READONLY, ValueKind::Continuous, 0.0);
    let readonly_project = project_with(vec![readonly_target]);
    let hosted_r = host_plugin(&READONLY_PARAMS);
    let (_gr, proc_r, _sr, _kr) = build_graph(&hosted_r.attached);
    let (mut session_r, mut dispatch_r) = open(hosted_r.attached.control, proc_r, 3, 1, 1);
    let err = session_r
        .bind(
            &mut dispatch_r,
            &readonly_project,
            ParameterTargetId::new(2),
            &READONLY_PARAMS,
        )
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::WriteRejected { ref rejection, .. } if rejection == "param_readonly"
    ));

    // Unsupported means through eval_event → lower_event gate.
    let hosted_c = host_plugin(&PARAMS);
    let (_gc, proc_c, _sc, _kc) = build_graph(&hosted_c.attached);
    let project_c = project_with(vec![gain_target(5)]);
    let plan_c = ControlGraph::compile(&project_c).unwrap();
    let mut scratch_c = plan_c.scratch();
    let (mut session_c, mut dispatch_c) = open(hosted_c.attached.control, proc_c, 4, 1, 1);
    session_c
        .bind(
            &mut dispatch_c,
            &project_c,
            ParameterTargetId::new(5),
            &PARAMS,
        )
        .unwrap();
    let err = session_c
        .eval_event(
            &project_c,
            &plan_c,
            &mut scratch_c,
            ParameterTargetId::new(5),
            BeatPoint::ZERO,
            None,
            MediaTime::new(64, frame_base()),
            ParamWriteMeans::Immediate,
        )
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::WriteRejected { ref rejection, .. }
            if rejection == "param_unsupported_means"
    ));

    // Queue full surfaces as typed Dispatch(QueueFull) — never a
    // silent drop.
    let hosted_q = host_plugin(&PARAMS);
    let (_gq, proc_q, _sq, _kq) = build_graph(&hosted_q.attached);
    let project_q = project_with(vec![gain_target(6)]);
    let (mut session_q, mut dispatch_q) = open(hosted_q.attached.control, proc_q, 5, 1, 1);
    let binding_q = session_q
        .bind(
            &mut dispatch_q,
            &project_q,
            ParameterTargetId::new(6),
            &PARAMS,
        )
        .unwrap();
    let mut flooded = false;
    for _ in 0..16_384 {
        if dispatch_q
            .control()
            .request(TransportCommand::Play, 0)
            .is_err()
        {
            flooded = true;
            break;
        }
    }
    assert!(flooded, "bounded queue must report QueueFull");
    let err = session_q
        .dispatch(
            &mut dispatch_q,
            &[(
                binding_q,
                TimedParamEvent {
                    position: MediaTime::new(64, frame_base()),
                    event: ParamEvent {
                        param_id: ParamId(7),
                        value: ParamValue::new(0.5).unwrap(),
                    },
                },
            )],
        )
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::Dispatch(DispatchError::QueueFull)
    ));
}

/// PC-C4-03 — composite control: automation + manual + LFO/Macro/
/// `StepSeq` assignments + depth driver evaluate through `eval_rt` and
/// reach the plugin; DAG cycles and discrete-kind modulation are
/// refused.
#[test]
#[expect(clippy::too_many_lines)]
fn pc_c4_03_composite_control_and_gates() {
    let gain = gain_target(1);
    let driver = gain_target(2); // depth driver — also processor-owned
    let mut project = project_with(vec![gain.clone(), driver.clone()]);

    // Automation lane on gain: 0.2 → 0.6 linear over [0, 4).
    project.control.lanes.insert(
        AutomationLaneId::new(1),
        AutomationLane {
            id: AutomationLaneId::new(1),
            target: gain.id,
            regions: vec![AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(0, 1).unwrap(),
                        value: 0.2,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(4, 1).unwrap(),
                        value: 0.6,
                    },
                ],
            }],
        },
    );

    // Three modulation sources covering LFO / Macro / StepSeq.
    let sources = [
        (
            ModulationSourceId::new(1),
            SourceKind::Lfo {
                rate: BeatPoint::new(1, 1).unwrap(),
                shape: LfoShape::Sine,
                bipolar: true,
            },
        ),
        (ModulationSourceId::new(2), SourceKind::Macro { value: 0.5 }),
        (
            ModulationSourceId::new(3),
            SourceKind::StepSeq {
                values: vec![0.5, -0.5],
                step: BeatPoint::new(1, 1).unwrap(),
            },
        ),
    ];
    for (id, kind) in &sources {
        project.control.sources.insert(
            *id,
            ModulationSource {
                id: *id,
                owner: ControlOwner::Processor(processor()),
                kind: kind.clone(),
            },
        );
    }
    // LFO assignment on gain depth-modulated by the driver target;
    // Macro + StepSeq at fixed depth.
    let assignments = [
        ModulationAssignment {
            id: ModulationAssignmentId::new(1),
            source: ModulationSourceId::new(1),
            target: gain.id,
            depth: 0.5,
            depth_source: Some(driver.id),
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
        ModulationAssignment {
            id: ModulationAssignmentId::new(2),
            source: ModulationSourceId::new(2),
            target: gain.id,
            depth: 0.1,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
        ModulationAssignment {
            id: ModulationAssignmentId::new(3),
            source: ModulationSourceId::new(3),
            target: gain.id,
            depth: 0.1,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    ];
    for a in assignments {
        project.control.assignments.insert(a.id, a);
    }

    let plan = ControlGraph::compile(&project).expect("DAG compiles");
    let mut scratch = plan.scratch();

    // The composite event must carry exactly the plan's evaluated
    // value, and modulation must have contributed (≠ bare manual).
    let hosted = host_plugin(&PARAMS);
    let (_g, proc, _s, _k) = build_graph(&hosted.attached);
    let (mut session, mut dispatch) = open(hosted.attached.control, proc, 1, 1, 1);
    let binding = session
        .bind(&mut dispatch, &project, gain.id, &PARAMS)
        .unwrap();
    let pos = BeatPoint::new(2, 1).unwrap();
    let manual = Some(0.5);
    let ev = session
        .eval_event(
            &project,
            &plan,
            &mut scratch,
            gain.id,
            pos,
            manual,
            MediaTime::new(64, frame_base()),
            ParamWriteMeans::TimedEvent,
        )
        .unwrap();
    let expected_norm = plan
        .eval_rt(&project, gain.id, pos, manual, &mut scratch)
        .unwrap();
    assert_eq!(ev.event.value.get(), expected_norm);
    assert_ne!(ev.event.value.get(), 0.5);
    session.dispatch(&mut dispatch, &[(binding, ev)]).unwrap();

    // DAG cycle: two targets depth-driving each other → ControlCycle.
    let mut cyclic = project_with(vec![gain.clone(), driver.clone()]);
    for (id, target, depth_source) in [
        (ModulationAssignmentId::new(10), gain.id, driver.id),
        (ModulationAssignmentId::new(11), driver.id, gain.id),
    ] {
        cyclic.control.assignments.insert(
            id,
            ModulationAssignment {
                id,
                source: ModulationSourceId::new(1),
                target,
                depth: 0.5,
                depth_source: Some(depth_source),
                polarity: Polarity::Bipolar,
                transform: Transform::Identity,
                enabled: true,
            },
        );
    }
    let err = ControlGraph::compile(&cyclic).unwrap_err();
    assert!(matches!(err, synvelis_project::Error::ControlCycle { .. }));

    // Discrete ValueKind + enabled modulation → UnsupportedModulation
    // at bind (declared capability matches live param 9's flags).
    let discrete = param_target(
        3,
        9,
        ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED),
        ValueKind::Boolean,
        0.0,
    );
    let mut discrete_project = project_with(vec![discrete]);
    discrete_project.control.sources.insert(
        ModulationSourceId::new(9),
        ModulationSource {
            id: ModulationSourceId::new(9),
            owner: ControlOwner::Processor(processor()),
            kind: SourceKind::Macro { value: 1.0 },
        },
    );
    discrete_project.control.assignments.insert(
        ModulationAssignmentId::new(9),
        ModulationAssignment {
            id: ModulationAssignmentId::new(9),
            source: ModulationSourceId::new(9),
            target: ParameterTargetId::new(3),
            depth: 0.5,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    let hosted_d = host_plugin(&PARAMS);
    let (_gd, proc_d, _sd, _kd) = build_graph(&hosted_d.attached);
    let (mut session_d, mut dispatch_d) = open(hosted_d.attached.control, proc_d, 6, 1, 1);
    let err = session_d
        .bind(
            &mut dispatch_d,
            &discrete_project,
            ParameterTargetId::new(3),
            &PARAMS,
        )
        .unwrap_err();
    assert!(matches!(err, BindingError::UnsupportedModulation { .. }));

    // Owner gate: a Strip-owned target can never bind to a plugin
    // session — the bootstrap master fader is exactly that.
    let strip_target = project
        .control
        .targets
        .values()
        .find(|t| t.owner != ControlOwner::Processor(processor()))
        .unwrap()
        .clone();
    let hosted_o = host_plugin(&PARAMS);
    let (_go, proc_o, _so, _ko) = build_graph(&hosted_o.attached);
    let (mut session_o, mut dispatch_o) = open(hosted_o.attached.control, proc_o, 7, 1, 1);
    let err = session_o
        .bind(&mut dispatch_o, &project, strip_target.id, &PARAMS)
        .unwrap_err();
    assert!(matches!(err, BindingError::OwnerMismatch));

    // A target with no capability contract → CapabilityMissing.
    let mut no_cap = gain.clone();
    no_cap.capability = None;
    let project_nc = project_with(vec![no_cap]);
    let hosted_n = host_plugin(&PARAMS);
    let (_gn, proc_n, _sn, _kn) = build_graph(&hosted_n.attached);
    let (mut session_n, mut dispatch_n) = open(hosted_n.attached.control, proc_n, 8, 1, 1);
    let err = session_n
        .bind(
            &mut dispatch_n,
            &project_nc,
            ParameterTargetId::new(1),
            &PARAMS,
        )
        .unwrap_err();
    assert!(matches!(err, BindingError::CapabilityMissing));
}

/// PC-C4-04 — generation consistency: re-attach mints a new instance
/// id (old records are `InstanceMismatch`), invalidate drops every
/// binding, stale `EvalPlan` is refused, and transport commands
/// (`Stop`/`Seek`) never invalidate the verified clock context.
#[test]
fn pc_c4_04_generation_consistency() {
    let target = gain_target(1);
    let project = project_with(vec![target.clone()]);
    let target_id = target.id;

    // Session A (instance 1) bound; session B is a *re-attach* — new
    // plugin instance, new node, new instance id.
    let hosted_a = host_plugin(&PARAMS);
    let (_ga, proc_a, _sa, _ka) = build_graph(&hosted_a.attached);
    let (mut session_a, mut dispatch_a) = open(hosted_a.attached.control, proc_a, 1, 1, 1);
    let binding_a = session_a
        .bind(&mut dispatch_a, &project, target_id, &PARAMS)
        .unwrap();
    let hosted_b = host_plugin(&PARAMS);
    let (_gb, proc_b, _sb, _kb) = build_graph(&hosted_b.attached);
    let (mut session_b, mut dispatch_b) = open(hosted_b.attached.control, proc_b, 2, 1, 1);
    session_b
        .bind(&mut dispatch_b, &project, target_id, &PARAMS)
        .unwrap();

    let ev = TimedParamEvent {
        position: MediaTime::new(64, frame_base()),
        event: ParamEvent {
            param_id: ParamId(7),
            value: ParamValue::new(0.5).unwrap(),
        },
    };

    // Session A's record through the re-attached session →
    // InstanceMismatch (same raw ParamId, different instance).
    let err = session_b
        .dispatch(&mut dispatch_b, &[(binding_a.clone(), ev)])
        .unwrap_err();
    assert!(matches!(err, BindingError::InstanceMismatch));

    // Transport commands do not touch the verified clock context —
    // Stop + Seek keep the binding fully dispatchable (PL04: seek/
    // transport change preserves generation correctness).
    dispatch_a
        .control()
        .request(TransportCommand::Stop, 0)
        .unwrap();
    dispatch_a
        .control()
        .request(
            TransportCommand::Seek(MediaTime::new(1024, frame_base())),
            0,
        )
        .unwrap();
    session_a
        .dispatch(&mut dispatch_a, &[(binding_a.clone(), ev)])
        .unwrap();

    // Invalidate (node retire / re-attach on the same session) drops
    // every binding — subsequent dispatch is TargetNotBound and the
    // underlying table reports param_defunct.
    session_a.invalidate(&mut dispatch_a);
    assert!(session_a.binding(target_id).is_none());
    let err = session_a
        .dispatch(&mut dispatch_a, &[(binding_a.clone(), ev)])
        .unwrap_err();
    assert!(matches!(err, BindingError::TargetNotBound));
    let err = dispatch_a.dispatch(&[(target_id, ev)]).unwrap_err();
    assert!(matches!(
        err,
        DispatchError::WriteRejected { ref rejection, .. } if rejection == "param_defunct"
    ));

    // Stale EvalPlan: mutate the project after compile → PlanStale;
    // recompile → current again (graph-plan swap detection).
    let plan_good = ControlGraph::compile(&project).unwrap();
    session_b.check_plan(&plan_good, &project).unwrap();
    let mut project_mut = project.clone();
    project_mut
        .control
        .targets
        .get_mut(&target_id)
        .unwrap()
        .base = 0.9;
    let err = session_b.check_plan(&plan_good, &project_mut).unwrap_err();
    assert!(matches!(err, BindingError::PlanStale));
    let plan_new = ControlGraph::compile(&project_mut).unwrap();
    session_b.check_plan(&plan_new, &project_mut).unwrap();
}
