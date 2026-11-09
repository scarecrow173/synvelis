//! SF-04 slice 2 (Issue #107): `ProbeTraceBridge` — the player-side
//! [`ProbeTracePort`] — translating real `RoutingEngine` probe events
//! into contract updates and routing them into node-keyed
//! `DispatchTraceAdapter` `observed` slots. Headless: the engine is
//! real, the plugin host is the in-process fake from PC-C3-05, and
//! every claim is proven through the port contract — no OS/RT/plugin
//! observation is claimed. `ProbeSession::pair` wires the probe
//! clock to the RT frame cursor, so stamps are `Derived` and can
//! classify `Current`; an unwired clock would stay `Estimated` and
//! could never reach `Current` (fail-closed, contract-tested at the
//! workspace layer).

use std::collections::BTreeMap;

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, attach};
use ruviform_graph::{GraphBuilder, NodeDef, NodeId, NodeKind};
use ruviform_media_core::{
    DeliveryPolicy, MediaDuration, MediaTimestamp, ScheduleBudget, TimeBase, TimestampProvenance,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList,
    ParamFlags, ParamId, ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor,
    PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::lowering::EngineConfig;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::trace::{DispatchTraceAdapter, TraceAdapterConfig, TraceContext};
use synvelis_player::{ParamDispatch, ProbeTraceBridge};
use synvelis_project::probe::{
    ProbeMode, SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary, SubscriptionKind,
};
use synvelis_project::trace::{ObservationFreshness, ProbeObservation, UnavailableReason};
use synvelis_project::{
    BuildBudget, BuildMode, Command, CounterIdGenerator, ParameterTargetId, ProbeSubscriptionKey,
    ProbeTracePort, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    ProjectSessionId, RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef,
    RuntimeInstanceId, StripId, TrackId,
};

// ---------------------------------------------------------------------------
// engine fixture (mirrors sf04_probe_adapter.rs)
// ---------------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(u128::from(0x107_u64)), "sf04-s2-bridge"),
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

fn add_track_strip(c: &mut ProjectController, name: &str) -> (TrackId, StripId) {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, strip)
}

fn engine() -> EngineConfig {
    EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    }
}

fn canonical_format() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn request(id: u64, revision: ProjectRevision) -> RoutingCompileRequest {
    RoutingCompileRequest {
        request_id: RoutingBuildRequestId(id),
        expected_project_revision: revision,
        mode: BuildMode::Realtime,
        resource_budget: BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        runtime_instance: RuntimeInstanceId(1),
        device_clock_epoch: RuntimeClockEpochRef(1),
    }
}

fn new_engine() -> RoutingEngine {
    RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1))
}

/// prepare + publish the live project — returns the published
/// generation.
fn publish(engine: &mut RoutingEngine, c: &ProjectController, request_id: u64) -> u64 {
    let revision = c.live_revision();
    let canonical = |_: synvelis_project::SourceId| Some(canonical_format());
    let deps = RoutingDeps {
        canonical_of: &canonical,
        clips: Vec::new(),
        engine_config_generation: 1,
        source_format_generation: 1,
    };
    let prepared = engine
        .prepare(&request(request_id, revision), c.live(), revision, deps)
        .unwrap()
        .expect_built();
    let generation = prepared.compiled.plan.generation;
    engine.publish(prepared, c.live(), revision).unwrap();
    generation
}

fn strip_key(strip: StripId, boundary: SignalBoundary) -> ProbeSubscriptionKey {
    ProbeSubscriptionKey {
        session_id: ProjectSessionId(1),
        point: SemanticProbePoint {
            entity: SemanticSignalEntityRef::Strip(strip),
            boundary,
            port: None,
        },
        kind: SubscriptionKind::Metrics,
    }
}

// ---------------------------------------------------------------------------
// fake plugin host fixture (mirrors trace_c3_05.rs)
// ---------------------------------------------------------------------------

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.sf04-s2-bridge",
    name: "SF04 S2 Bridge",
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

/// Mints a real `NodeId` — `NodeId::new` is crate-private upstream.
fn mint_node_id() -> NodeId {
    let mut b = GraphBuilder::new();
    let (node, _ports) = b.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: Vec::new(),
    });
    node
}

fn trace_ctx(c: &ProjectController) -> TraceContext {
    TraceContext {
        project_id: c.live().project_id,
        project_revision: c.live_revision(),
        graph_generation: 1,
    }
}

/// Bridge over a fresh `RoutingEngine` plus one fake-plugin-backed
/// adapter keyed by `node`. The returned `PluginHost` must outlive
/// the bridge — the adapter's control channel dies with it.
fn bridge(c: &ProjectController, node: NodeId) -> (ProbeTraceBridge, PluginHost) {
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
    let dispatch = ParamDispatch::new(attached.control, node, frame_base());
    let adapter = DispatchTraceAdapter::new(
        dispatch,
        frame_base(),
        TraceAdapterConfig::default(),
        trace_ctx(c),
    );
    let mut adapters = BTreeMap::new();
    adapters.insert(node, adapter);
    (ProbeTraceBridge::new(new_engine(), adapters), host)
}

/// Freshness context reconstructed from the recorded trace key —
/// the honest way to evaluate: key fields come from the trace
/// itself, position/now from the observation stamp.
fn freshness_ctx(
    trace: &synvelis_project::ControlTrace,
    stamp: MediaTimestamp,
) -> synvelis_project::FreshnessContext {
    synvelis_project::FreshnessContext {
        project_revision: trace.key.project_revision,
        control_plan_generation: trace.key.control_plan_generation,
        graph_generation: trace.key.graph_generation,
        transport_generation: trace.key.transport_generation,
        clock_epoch: trace.key.clock_epoch,
        loop_occurrence: trace.key.loop_occurrence,
        evaluation_position: stamp.time,
        now: stamp.time,
        max_age: MediaDuration::new(180_000, frame_base()),
    }
}

fn observed_freshness(
    bridge: &ProbeTraceBridge,
    target: ParameterTargetId,
    ctx: &synvelis_project::FreshnessContext,
) -> ObservationFreshness {
    bridge.trace(target).map_or(
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        },
        |trace| trace.observed_freshness(ctx),
    )
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[test]
fn sf04_s2_metric_events_become_measuring_updates_with_derived_stamp() {
    let mut c = controller();
    let (_t, strip) = add_track_strip(&mut c, "t");
    let node = mint_node_id();
    let (mut bridge, _host) = bridge(&c, node);
    let target = ParameterTargetId::new(42);
    bridge.route_target(target, node);

    publish(bridge.engine_mut(), &c, 1);
    let key = strip_key(strip, SignalBoundary::Output);
    bridge
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    bridge.engine_mut().run(4, PumpPolicy::Every(1)).unwrap();

    // Drain → contract updates only. The probe clock is wired to the
    // RT frame cursor, so stamps are Derived — position derived from
    // the cursor, never container-carried.
    let updates = bridge.probe_drain();
    assert!(!updates.is_empty(), "expected probe updates");
    let measuring: Vec<_> = updates
        .iter()
        .filter(|u| matches!(u.observation, ProbeObservation::Measuring { .. }))
        .collect();
    assert!(!measuring.is_empty(), "expected Measuring updates");
    for update in &updates {
        assert_eq!(update.key, key, "updates must carry the subscription key");
        if let ProbeObservation::Measuring { stamp, .. } = update.observation {
            assert_eq!(
                stamp.provenance,
                TimestampProvenance::Derived,
                "wired probe clock derives stamps from the frame cursor"
            );
        }
    }

    // Route one update into the target's observed slot, then classify.
    let update = measuring[0];
    assert!(bridge.record_probe(target, update.observation));
    let trace = bridge.trace(target).expect("routed trace");
    let ProbeObservation::Measuring { stamp, .. } = trace.observed.present().unwrap() else {
        panic!("observed slot must hold the recorded Measuring observation")
    };
    let freshness = observed_freshness(&bridge, target, &freshness_ctx(trace, *stamp));
    assert_eq!(freshness, ObservationFreshness::Current);
    // Other stages remain honestly Missing — probe evidence never
    // synthesizes dispatch/plugin stages.
    assert!(trace.dispatched.present().is_none());
    assert!(trace.plugin_reported.present().is_none());
}

#[test]
fn sf04_s2_stale_generation_records_are_rejected() {
    let mut c = controller();
    let (_t, strip) = add_track_strip(&mut c, "t");
    let node = mint_node_id();
    let (mut bridge, _host) = bridge(&c, node);

    publish(bridge.engine_mut(), &c, 1);
    let key = strip_key(strip, SignalBoundary::Output);
    bridge
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    // RT produces generation-1 records — leave them queued.
    bridge.engine_mut().run(4, PumpPolicy::Every(1)).unwrap();

    // Publish generation 2 before draining: the rebind moves the
    // binding's active generation, so every queued generation-1
    // record is stale evidence.
    let (_t2, _s2) = add_track_strip(&mut c, "t2");
    let generation = publish(bridge.engine_mut(), &c, 2);
    assert_eq!(generation, 2);

    let stale = bridge.probe_drain();
    assert!(
        !stale
            .iter()
            .any(|u| matches!(u.observation, ProbeObservation::Measuring { .. })),
        "stale-generation metric records must never become observations"
    );

    // New-generation records flow normally afterwards.
    bridge.engine_mut().run(4, PumpPolicy::Every(1)).unwrap();
    let fresh = bridge.probe_drain();
    assert!(
        fresh
            .iter()
            .any(|u| matches!(u.observation, ProbeObservation::Measuring { .. })),
        "current-generation records must produce Measuring updates"
    );
}

#[test]
fn sf04_s2_mode_off_maps_to_disabled_and_unrouted_record_returns_false() {
    let mut c = controller();
    let (_t, strip) = add_track_strip(&mut c, "t");
    let node = mint_node_id();
    let (mut bridge, _host) = bridge(&c, node);
    let target = ParameterTargetId::new(43);
    let unrouted = ParameterTargetId::new(44);
    bridge.route_target(target, node);

    publish(bridge.engine_mut(), &c, 1);
    let key = strip_key(strip, SignalBoundary::Output);
    bridge
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    bridge.engine_mut().run(2, PumpPolicy::Every(1)).unwrap();
    let _ = bridge.probe_drain();

    // Off → Disabled observation state on the channel.
    bridge.probe_set_mode(key, ProbeMode::Off).unwrap();
    bridge.engine_mut().run(2, PumpPolicy::Every(1)).unwrap();
    let updates = bridge.probe_drain();
    assert!(
        updates
            .iter()
            .any(|u| { u.key == key && matches!(u.observation, ProbeObservation::Disabled) })
    );

    // record_probe: routed target accepts, unrouted refuses visibly.
    assert!(bridge.record_probe(target, ProbeObservation::Missing));
    assert!(!bridge.record_probe(unrouted, ProbeObservation::Missing));
    assert!(bridge.trace(unrouted).is_none());

    // Binding/segment reads pass through the port contract.
    assert!(bridge.probe_bindings().contains_key(&key));
    let _ = bridge.probe_segments();
}
