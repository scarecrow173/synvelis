//! PC-C3-05 (issue #100, TRACE-C3-05): `DispatchTraceAdapter` maps
//! real dispatch-ledger events and scheduler receipts onto
//! `ControlTrace` stage slots — bounded history, generation-window
//! expiry, drop honesty and receipt correlation, driven through an
//! in-process fake plugin + `GraphRuntime`. No real OS/plugin
//! observation is claimed.

use ruviform_audio::typed::{Compiler, CompilerConfig, DEFAULT_ALIGNMENT, GraphRuntime};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, PluginNodeFactory, attach};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, PortRef, Validator,
};
use ruviform_media_core::{
    ClockEpochId, DeliveryPolicy, EngineFrame, LoopOccurrence, MediaTime, MediaTimestamp,
    PlanGeneration, ScheduleBudget, SpanContext, TempoMapRevision, TimeBase, TimestampProvenance,
    TransportGeneration, TransportSpan, TransportStatus,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList,
    ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TimedParamEvent, TransportInfo, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::ParamDispatch;
use synvelis_player::trace::{
    DispatchTraceAdapter, TraceAdapterConfig, TraceContext, TraceEventKind,
};
use synvelis_project::trace::{
    ConfiguredEvidence, DispatchDropReason, DispatchEvidence, MeasuredValue, ProbeObservation,
    TraceSlot,
};
use synvelis_project::{ParameterTargetId, ProjectId, ProjectRevision};

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c3-trace",
    name: "PC-C3 Trace",
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
        id: ParamId(8),
        name: "Pan",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.5,
        flags: ParamFlags::AUTOMATABLE,
    },
];

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

fn ctx() -> TraceContext {
    TraceContext {
        project_id: ProjectId::new(1),
        project_revision: ProjectRevision(1),
        graph_generation: 1,
    }
}

fn span_for(plan_generation: PlanGeneration, end: u64) -> TransportSpan {
    TransportSpan {
        start: EngineFrame::new(ClockEpochId::new(0), 0),
        end: EngineFrame::new(ClockEpochId::new(0), end),
        ctx: SpanContext {
            status: TransportStatus::Playing,
            position_ticks: 0,
            position_base: frame_base(),
            loop_ticks: None,
            occurrence: LoopOccurrence::new(0),
            clock_epoch: ClockEpochId::new(0),
            transport_generation: TransportGeneration::new(0),
            plan_generation,
            tempo_revision: TempoMapRevision::new(0),
        },
        deliverable: true,
    }
}

/// No-op processor — the trace adapter observes the control/report
/// path only; plugin audio output is irrelevant to these tests.
struct NoopPlugin;

impl PluginProcessor for NoopPlugin {
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
        _events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        ProcessResult::CONTINUE
    }
}

impl Plugin for NoopPlugin {
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

struct NoopFactory;

impl PluginFactory for NoopFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        ruviform_plugin_core::erase(NoopPlugin)
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(NoopPlugin))
    }
}

struct Hosted {
    _host: PluginHost,
    attached: ruviform_audio_plugin_bridge::Attached,
}

fn host_plugin() -> Hosted {
    let mut host = PluginHost::new();
    host.register_factory(Box::new(NoopFactory));
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
    }
}

/// Mints a real `NodeId` from a trivial graph — `NodeId::new` is
/// crate-private upstream, and the ledger attributes by whatever id
/// the dispatcher was built with, so any consistent id works for the
/// non-runtime (fabricated-span) paths.
fn mint_node_id() -> NodeId {
    let mut b = GraphBuilder::new();
    let (node, _ports) = b.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: Vec::new(),
    });
    node
}

/// Adapter over `hosted`'s control handle for the fabricated-span
/// paths — consumes `attached.control`.
fn fabricated_adapter(hosted: Hosted, config: TraceAdapterConfig) -> DispatchTraceAdapter {
    let dispatch =
        ParamDispatch::new(hosted.attached.control, mint_node_id(), frame_base()).unwrap();
    DispatchTraceAdapter::new(dispatch, frame_base(), config, ctx())
}

fn writes_for(target: ParameterTargetId, param: u32) -> Vec<(ParameterTargetId, TimedParamEvent)> {
    let base = frame_base();
    [(64_i64, 0.25_f64), (128, 0.5), (192, 0.75)]
        .into_iter()
        .map(|(pos, v)| {
            (
                target,
                TimedParamEvent {
                    position: MediaTime::new(pos, base),
                    event: ParamEvent {
                        param_id: ParamId(param),
                        value: ParamValue::new(v).unwrap(),
                    },
                },
            )
        })
        .collect()
}

// ---- tests ------------------------------------------------------------

/// TRC-T1/T5: dispatch fills `prepared`, `observe_cycle` fills
/// `dispatched` from real ledger events; configured/probe ingest
/// fills the external channels verbatim.
#[test]
fn trace_c3_05_stages_fill_from_ledger_events() {
    let mut ad = fabricated_adapter(host_plugin(), TraceAdapterConfig::default());
    let target = ParameterTargetId::new(1);
    ad.bind(target, ParamId(7), &PARAMS).unwrap();

    let plan = ad.dispatch(&writes_for(target, 7)).unwrap();
    let trace = ad.trace(target).unwrap();
    assert_eq!(
        trace.key.control_plan_generation, plan.generation,
        "key tracks the staged plan generation"
    );
    assert!(matches!(
        trace.prepared,
        TraceSlot::Present(synvelis_project::trace::PreparedEvidence { .. })
    ));
    assert!(
        matches!(ad.trace(target).unwrap().dispatched, TraceSlot::Missing),
        "no observation yet — dispatched stays honestly Missing"
    );

    ad.observe_cycle(
        &span_for(plan.generation, 384),
        EngineFrame::new(ClockEpochId::new(0), 0),
        384,
        &ctx(),
    );
    let trace = ad.trace(target).unwrap();
    match &trace.dispatched {
        TraceSlot::Present(DispatchEvidence::Dispatched {
            plan_generation,
            frame_offset,
            ..
        }) => {
            assert_eq!(*plan_generation, plan.generation);
            assert!(*frame_offset < 384);
        }
        other => panic!("expected Dispatched evidence, got {other:?}"),
    }
    assert!(trace.is_coherent());

    ad.record_configured(
        target,
        ConfiguredEvidence {
            host_normalized: 0.5,
            saturated: false,
            quantized: false,
        },
    );
    ad.record_probe(
        target,
        ProbeObservation::Measuring {
            stamp: MediaTimestamp {
                time: MediaTime::new(64, frame_base()),
                provenance: TimestampProvenance::Estimated,
            },
            value: MeasuredValue::Signal(ParamValue::new(0.5).unwrap()),
        },
    );
    let trace = ad.trace(target).unwrap();
    assert!(matches!(
        trace.configured,
        TraceSlot::Present(ConfiguredEvidence {
            host_normalized: 0.5,
            ..
        })
    ));
    assert!(matches!(
        trace.observed,
        TraceSlot::Present(ProbeObservation::Measuring { .. })
    ));
    let m = ad.metrics();
    assert!(m.events_recorded > 0);
    assert_eq!(m.last_value, Some(0.75));
    assert!(m.last_call_start.is_some());
    assert_eq!(m.last_plan_generation, Some(plan.generation));
    assert!(
        ad.history()
            .iter()
            .any(|e| e.kind == TraceEventKind::DispatchPlanned)
    );
}

/// TRC-T2: real node processing mints scheduler receipts; the adapter
/// attributes them by plan generation into scheduled/applied slots.
#[test]
fn trace_c3_05_receipt_correlation() {
    let hosted = host_plugin();
    // Graph: Source → plugin processor → Sink (same shape as pc_b).
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
    let (p, pp) = b.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: hosted.attached.ports.clone(),
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
    b.connect(&conn(pref(src, sp[0]), pref(p, pp[0])));
    b.connect(&conn(pref(p, pp[1]), pref(sink, kp[0])));
    let graph = Validator::validate(&b.build()).expect("graph validates");

    let mut factory = PluginNodeFactory::new();
    factory.insert(p, hosted.attached.node);
    let config = CompilerConfig {
        max_frames: 512,
        alignment: DEFAULT_ALIGNMENT,
        max_slots: 64,
        max_channels: 8,
        max_delay_lines: 16,
        max_delay_frames: 4096,
        max_processors: 16,
    };
    let (mut runtime, mut rt) = GraphRuntime::new(&config, 8, 8);
    let compiled = Compiler::new(config)
        .compile(
            &graph,
            &mut factory,
            runtime.registry(),
            runtime.allocator(),
            runtime.generation() + 1,
        )
        .expect("compiles");
    runtime.publish(compiled).expect("publish");

    let dispatch = ParamDispatch::new(hosted.attached.control, p, frame_base()).unwrap();
    let mut ad =
        DispatchTraceAdapter::new(dispatch, frame_base(), TraceAdapterConfig::default(), ctx());
    let target = ParameterTargetId::new(1);
    ad.bind(target, ParamId(7), &PARAMS).unwrap();
    let plan = ad.dispatch(&writes_for(target, 7)).unwrap();

    // Drive the runtime so the scheduler applies Play/install_plan
    // and posts receipts into the report channel.
    let rctx = RtContext::mint();
    let zeros = vec![0.0f32; 128];
    for _ in 0..4 {
        let mut slot = rt.input_writer(sp[0], 128).expect("input bound");
        for ch in 0..2 {
            slot.data.channel_mut(ch).unwrap()[..128].copy_from_slice(&zeros);
        }
        drop(slot);
        rt.process(128, &rctx).expect("process");
    }

    ad.observe_cycle(
        &span_for(plan.generation, 512),
        EngineFrame::new(ClockEpochId::new(0), 0),
        512,
        &ctx(),
    );

    let trace = ad.trace(target).expect("target tracked");
    let has_receipt = matches!(trace.scheduled, TraceSlot::Present(_))
        || matches!(trace.applied, TraceSlot::Present(_));
    assert!(
        has_receipt,
        "expected a scheduler receipt in scheduled/applied, got scheduled={:?} applied={:?}",
        trace.scheduled, trace.applied
    );
    assert!(
        trace.is_coherent(),
        "receipt-attributed generations must agree with the key"
    );
    let m = ad.metrics();
    assert!(
        m.latency_samples > 0,
        "applied receipts yield latency samples"
    );
    assert!(m.last_latency_seconds.is_some());
    assert!(m.last_transport_generation.is_some() || m.last_clock_epoch.is_some());
}

/// TRC-T3/T4: `record_reported` moves the ledger entry to `Reported`
/// (dispatched=Reported + `plugin_reported` Present with an Estimated
/// stamp); `mark_reporting_unavailable` yields the honest
/// `Unavailable` terminal.
#[test]
fn trace_c3_05_reported_and_unavailable_stages() {
    let mut ad = fabricated_adapter(host_plugin(), TraceAdapterConfig::default());
    let target = ParameterTargetId::new(1);
    let target2 = ParameterTargetId::new(2);
    ad.bind(target, ParamId(7), &PARAMS).unwrap();
    ad.bind(target2, ParamId(8), &PARAMS).unwrap();
    let plan = ad
        .dispatch(&[writes_for(target, 7), writes_for(target2, 8)].concat())
        .unwrap();
    ad.observe_cycle(
        &span_for(plan.generation, 384),
        EngineFrame::new(ClockEpochId::new(0), 0),
        384,
        &ctx(),
    );

    assert!(ad.record_reported(target, ParamValue::new(0.9).unwrap()));
    ad.mark_reporting_unavailable();
    ad.observe_cycle(
        &span_for(plan.generation, 384),
        EngineFrame::new(ClockEpochId::new(0), 0),
        384,
        &ctx(),
    );

    let trace = ad.trace(target).unwrap();
    assert!(matches!(
        trace.dispatched,
        TraceSlot::Present(DispatchEvidence::Reported { .. })
    ));
    match &trace.plugin_reported {
        TraceSlot::Present(ev) => {
            assert_eq!(
                ev.stamp.provenance,
                TimestampProvenance::Estimated,
                "report channel has no timestamp — never Current"
            );
            assert_eq!(ev.value.get(), 0.9);
        }
        other => panic!("expected plugin_reported Present, got {other:?}"),
    }

    let trace2 = ad.trace(target2).unwrap();
    assert!(matches!(
        trace2.dispatched,
        TraceSlot::Present(DispatchEvidence::Unavailable { .. })
    ));
}

/// TRC-T9: a superseded plan's pending entries surface as
/// `Dropped{PlanReplaced}` — never rewritten into success.
#[test]
fn trace_c3_05_plan_replaced_drop_is_explicit() {
    let mut ad = fabricated_adapter(
        host_plugin(),
        TraceAdapterConfig {
            plan_gen_window: 4,
            ..TraceAdapterConfig::default()
        },
    );
    let t1 = ParameterTargetId::new(1);
    let t2 = ParameterTargetId::new(2);
    ad.bind(t1, ParamId(7), &PARAMS).unwrap();
    ad.bind(t2, ParamId(8), &PARAMS).unwrap();

    let g1 = ad.dispatch(&writes_for(t1, 7)).unwrap();
    // No observe yet — t1's entries are still `Scheduled`, which is
    // the only stage `mark_plan_replaced` can drop.

    // Second plan replaces the first — t1's pending entries drop.
    let g2 = ad.dispatch(&writes_for(t2, 8)).unwrap();
    ad.observe_cycle(
        &span_for(g2.generation, 384),
        EngineFrame::new(ClockEpochId::new(0), 0),
        384,
        &ctx(),
    );

    let trace1 = ad.trace(t1).unwrap();
    match &trace1.dispatched {
        TraceSlot::Present(DispatchEvidence::Dropped { reason, .. }) => {
            assert_eq!(*reason, DispatchDropReason::PlanReplaced);
        }
        other => panic!("expected Dropped{{PlanReplaced}}, got {other:?}"),
    }
    // t1's key still names plan1 — the dropped evidence is coherent
    // with it (same generation), and nothing claims success.
    assert_eq!(trace1.key.control_plan_generation, g1.generation);
    assert!(trace1.is_coherent());
    assert!(ad.metrics().dropped_observations > 0);
    assert!(matches!(
        ad.trace(t2).unwrap().dispatched,
        TraceSlot::Present(
            DispatchEvidence::Scheduled { .. } | DispatchEvidence::Dispatched { .. }
        )
    ));
}

/// TRC-T6/T7/T8: bounded history + target cap surface loss;
/// observations whose revision left the generation window surface
/// `Expired` — never a fabricated generation.
#[test]
fn trace_c3_05_bounded_history_and_expired() {
    let mut ad = fabricated_adapter(
        host_plugin(),
        TraceAdapterConfig {
            history_capacity: 4,
            max_targets: 8,
            plan_gen_window: 1,
        },
    );
    let t1 = ParameterTargetId::new(1);
    let t2 = ParameterTargetId::new(2);
    ad.bind(t1, ParamId(7), &PARAMS).unwrap();
    ad.bind(t2, ParamId(8), &PARAMS).unwrap();

    // Three dispatches with window=1: only the newest revision keeps
    // its generation mapping. t1 is never re-dispatched, so its first
    // observation arrives after revision 1 already left the window.
    let _g1 = ad.dispatch(&writes_for(t1, 7)).unwrap();
    let g2 = ad.dispatch(&writes_for(t2, 8)).unwrap();
    let g3 = ad.dispatch(&writes_for(t2, 8)).unwrap();
    ad.observe_cycle(
        &span_for(g3.generation, 384),
        EngineFrame::new(ClockEpochId::new(0), 0),
        384,
        &ctx(),
    );

    let trace1 = ad.trace(t1).unwrap();
    assert!(
        matches!(trace1.dispatched, TraceSlot::Expired),
        "out-of-window revision must expire, got {:?}",
        trace1.dispatched
    );
    assert!(ad.metrics().expired_correlations > 0);
    assert!(matches!(
        ad.trace(t2).unwrap().dispatched,
        TraceSlot::Present(
            DispatchEvidence::Scheduled { .. } | DispatchEvidence::Dispatched { .. }
        )
    ));
    let _ = g2;

    // History ring cap.
    for i in 0..10u64 {
        ad.record_probe(t1, ProbeObservation::Lost { count: i });
    }
    assert_eq!(ad.history_len(), 4);
    assert!(ad.metrics().history_lost > 0);
    assert!(ad.is_truncated());

    // Target cap — LRU eviction beyond max_targets.
    for i in 100..112u128 {
        ad.record_configured(
            ParameterTargetId::new(i),
            ConfiguredEvidence {
                host_normalized: 0.1,
                saturated: false,
                quantized: false,
            },
        );
    }
    assert!(ad.target_count() <= 8);
    assert!(ad.metrics().targets_lost > 0);
}
