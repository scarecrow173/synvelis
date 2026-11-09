//! SF-04 slice 1 (Issue #107): the semantic probe adapter on
//! `RoutingEngine` — request/queued vs RT-confirmed lifecycle,
//! generation-guarded stale rejection, `RebindReceipt`-driven capture
//! segment cuts at the generation boundary (never silent
//! continuation), semantic re-resolution when a point moves, and
//! explicit remove/stop lifecycle. Headless: every claim is proven by
//! RT-observed status evidence drained through `probe_drain`.

use synvelis_player::driver::PumpPolicy;
use synvelis_player::lowering::EngineConfig;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::ProbeRequestError;
use synvelis_project::probe::{
    CaptureSegmentEnd, ProbeMode, ProbeResolveState, ProbeStatusKind, SemanticProbePoint,
    SemanticSignalEntityRef, SignalBoundary, SubscriptionKind,
};
use synvelis_project::{
    BusKind, BuildBudget, BuildMode, Command, CounterIdGenerator, PortDirection, PortRole,
    ProbeSubscriptionKey, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    ProjectSessionId, RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef,
    RuntimeInstanceId, StripId, TrackId,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};

fn controller() -> ProjectController {
    controller_with_id(0x107)
}

fn controller_with_id(id: u64) -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(u128::from(id)), "sf04"),
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

/// Applies an op that mints a processor and returns its id.
fn apply_op(c: &mut ProjectController, op: ProjectOp) -> synvelis_project::ProcessorId {
    apply(c, op);
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn add_track_strip(c: &mut ProjectController, name: &str) -> (TrackId, StripId) {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, strip)
}

fn add_bus(c: &mut ProjectController, name: &str) -> StripId {
    apply(
        c,
        ProjectOp::AddBus {
            kind: BusKind::Group,
            name: name.into(),
        },
    );
    c.live()
        .routing
        .buses
        .values()
        .find(|b| b.name == name)
        .unwrap()
        .strip
}

fn strip_port(p: &Project, strip: StripId, direction: PortDirection) -> synvelis_project::SemanticPortId {
    p.routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| {
            p.routing.ports[id].direction == direction && p.routing.ports[id].role == PortRole::Main
        })
        .copied()
        .unwrap()
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

fn drain(engine: &mut RoutingEngine) -> usize {
    let mut events = 0usize;
    engine.probe_drain(|_| events += 1);
    events
}

fn key_for(entity: SemanticSignalEntityRef, boundary: SignalBoundary) -> ProbeSubscriptionKey {
    ProbeSubscriptionKey {
        session_id: ProjectSessionId(1),
        point: SemanticProbePoint {
            entity,
            boundary,
            port: None,
        },
        kind: SubscriptionKind::Metrics,
    }
}

fn strip_key(strip: StripId, boundary: SignalBoundary) -> ProbeSubscriptionKey {
    key_for(SemanticSignalEntityRef::Strip(strip), boundary)
}

fn binding<'a>(
    engine: &'a RoutingEngine,
    key: &ProbeSubscriptionKey,
) -> &'a synvelis_project::SemanticProbeBinding {
    &engine.probe_bindings()[key]
}

// ── request → queued → RT-confirmed lifecycle ───────────────────

#[test]
fn sf04_probe_request_queues_then_rt_confirms_bound_active() {
    let mut c = controller();
    let (_track, strip) = add_track_strip(&mut c, "t");
    let mut engine = new_engine();
    let generation = publish(&mut engine, &c, 1);

    // Before any RT pass the binding is queued, never confirmed.
    let key = strip_key(strip, SignalBoundary::Output);
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    let b = binding(&engine, &key);
    assert_eq!(b.state, ProbeResolveState::QueuedForBinding);
    assert_eq!(b.requested_mode, ProbeMode::Monitor);
    assert_eq!(b.confirmed_mode, None);
    assert!(b.runtime_probe_id.is_some());
    assert_eq!(b.active_plan_generation, generation);

    // RT applies the install; drain turns queued into observed.
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    assert!(drain(&mut engine) > 0);
    let b = binding(&engine, &key);
    assert_eq!(b.state, ProbeResolveState::BoundActive);
    let status = b.last_status.as_ref().unwrap();
    assert_eq!(status.kind, ProbeStatusKind::Installed);
    assert_eq!(status.generation, generation);
}

#[test]
fn sf04_probe_request_requires_a_published_plan() {
    let mut c = controller();
    let (_track, strip) = add_track_strip(&mut c, "t");
    let mut engine = new_engine();
    let key = strip_key(strip, SignalBoundary::Output);
    assert!(matches!(
        engine.probe_request(c.live(), key, ProbeMode::Monitor),
        Err(ProbeRequestError::NoActivePlan)
    ));

    // A project that is not the published plan's project mismatches.
    publish(&mut engine, &c, 1);
    let mut other = controller_with_id(0x999);
    add_track_strip(&mut other, "other");
    assert!(matches!(
        engine.probe_request(other.live(), key, ProbeMode::Monitor),
        Err(ProbeRequestError::ProjectMismatch)
    ));
}

#[test]
fn sf04_probe_unresolvable_point_is_recorded_not_faked() {
    let mut c = controller();
    let (_track, _strip) = add_track_strip(&mut c, "t");
    let mut engine = new_engine();
    publish(&mut engine, &c, 1);

    let key = strip_key(StripId::new(999), SignalBoundary::Output);
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    let b = binding(&engine, &key);
    assert_eq!(b.state, ProbeResolveState::Unresolved);
    assert_eq!(b.runtime_probe_id, None);
    // No runtime probe exists — control calls fail closed.
    assert!(matches!(
        engine.probe_set_mode(key, ProbeMode::Capture),
        Err(ProbeRequestError::NotBound)
    ));
    assert!(matches!(
        engine.probe_capture(key, 1_024),
        Err(ProbeRequestError::NotBound)
    ));
}

// ── generation-guarded stale rejection ──────────────────────────

#[test]
fn sf04_probe_stale_generation_command_is_rejected_and_rebound() {
    let mut c = controller();
    let (_track, strip) = add_track_strip(&mut c, "t");
    let mut engine = new_engine();
    publish(&mut engine, &c, 1);

    let key = strip_key(strip, SignalBoundary::Output);
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();

    // Publish generation 2 *before* the RT side ever saw generation
    // 1's install — the stale `Bind` must be dropped, counted, and
    // the probe must land on generation 2 via the staged rebind.
    let (_t2, strip2) = add_track_strip(&mut c, "t2");
    let _ = strip2;
    let generation = publish(&mut engine, &c, 2);
    assert_eq!(generation, 2);

    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert!(engine.rt_status().stale_commands > 0);
    let b = binding(&engine, &key);
    assert_eq!(b.state, ProbeResolveState::BoundActive);
    assert_eq!(b.active_plan_generation, 2);
    assert_eq!(b.last_status.as_ref().unwrap().generation, 2);
}

// ── capture segment cut at the generation boundary ──────────────

#[test]
fn sf04_probe_capture_segment_cuts_at_generation_boundary() {
    let mut c = controller();
    let (_track, strip) = add_track_strip(&mut c, "t");
    let mut engine = new_engine();
    publish(&mut engine, &c, 1);

    let key = strip_key(strip, SignalBoundary::Output);
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(binding(&engine, &key).state, ProbeResolveState::BoundActive);

    // Explicit bounded capture — a new session.
    engine.probe_capture(key, 8_192).unwrap();
    engine.run(4, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(binding(&engine, &key).confirmed_mode, Some(ProbeMode::Capture));

    // A second capture while the segment is open is refused.
    assert!(matches!(
        engine.probe_capture(key, 1_024),
        Err(ProbeRequestError::AlreadyCapturing)
    ));

    // Publish generation 2 — StopAtBoundary must cut the segment and
    // never silently continue capture under the new generation.
    let (_t2, _s2) = add_track_strip(&mut c, "t2");
    publish(&mut engine, &c, 2);
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    let segments = engine.probe_segments();
    assert_eq!(segments.len(), 1, "no automatic recapture");
    let segment = &segments[0];
    assert_eq!(segment.ended_by, Some(CaptureSegmentEnd::GenerationBoundary));
    assert_eq!(segment.active_plan_generation, 1);
    assert!(segment.frame_range.is_some(), "captured PCM observed");
    assert_eq!(segment.completeness, synvelis_project::CaptureCompleteness::Complete);

    // The binding rebound under generation 2 in Monitor — the
    // capture request is preserved as requested state, the RT mode
    // is honestly Monitor.
    let b = binding(&engine, &key);
    assert_eq!(b.state, ProbeResolveState::BoundActive);
    assert_eq!(b.active_plan_generation, 2);
    assert_eq!(b.requested_mode, ProbeMode::Capture);
    assert_eq!(b.confirmed_mode, Some(ProbeMode::Monitor));
    assert_eq!(engine.probe_segments().len(), 1);
}

// ── semantic re-resolution when the point moves ────────────────

#[test]
fn sf04_probe_reinstalls_when_semantic_point_moves() {
    let mut c = controller();
    let (_track, _strip) = add_track_strip(&mut c, "t");
    let bus_strip = add_bus(&mut c, "fx");
    // Send track strip out → bus strip in so the bus input has a
    // physical producer.
    let track_strip = c
        .live()
        .routing
        .strip_of_track(*c.live().tracks.keys().next_back().unwrap())
        .unwrap()
        .id;
    let src_out = strip_port(c.live(), track_strip, PortDirection::Output);
    let dst_in = strip_port(c.live(), bus_strip, PortDirection::Input);
    apply(
        &mut c,
        ProjectOp::AddSend {
            source: src_out,
            destination: dst_in,
            tap: synvelis_project::SendTap::PreFader,
            gain_target: None,
        },
    );

    let mut engine = new_engine();
    publish(&mut engine, &c, 1);

    // Bus strip Input at generation 1: empty chain → fader input.
    let key = strip_key(bus_strip, SignalBoundary::Input);
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    let before = binding(&engine, &key);
    assert_eq!(before.state, ProbeResolveState::BoundActive);
    let first_probe = before.runtime_probe_id.unwrap();

    // Generation 2 inserts a processor at the head of the bus chain —
    // the same semantic point now resolves to the processor's input,
    // a *different* runtime slot. The adapter must replace the probe,
    // never let the old slot keep observing under the new plan.
    let processor = apply_op(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: bus_strip,
            index: 0,
            name: "eq".into(),
            binding: synvelis_project::ProcessorBinding::Resolved(
                synvelis_project::PluginRef {
                    descriptor: "vendor.eq".into(),
                    version: "1.0".into(),
                    capability_revision: 3,
                },
            ),
            sidechain_input: false,
        },
    );
    // Chain processors are host-connected at prepare time; register
    // a no-op gain instance for the new semantic processor.
    let mut host = synvelis_player::host::StaticProcessorHost::new();
    host.insert(
        processor,
        Box::new(synvelis_player::routing_lower::GainProcessor::new(1.0)),
    );
    engine.set_processor_host(Box::new(host));
    publish(&mut engine, &c, 2);
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    let after = binding(&engine, &key);
    assert_eq!(after.state, ProbeResolveState::BoundActive);
    assert_eq!(after.active_plan_generation, 2);
    assert_ne!(
        after.runtime_probe_id.unwrap(),
        first_probe,
        "a moved point gets a new runtime probe, not a stale slot"
    );
    assert_eq!(after.last_status.as_ref().unwrap().generation, 2);
}

// ── remove lifecycle ────────────────────────────────────────────

#[test]
fn sf04_probe_remove_records_stopped_lifecycle() {
    let mut c = controller();
    let (_track, strip) = add_track_strip(&mut c, "t");
    let mut engine = new_engine();
    publish(&mut engine, &c, 1);

    let key = strip_key(strip, SignalBoundary::Output);
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(binding(&engine, &key).state, ProbeResolveState::BoundActive);

    engine.probe_remove(key).unwrap();
    let b = binding(&engine, &key);
    assert_eq!(b.state, ProbeResolveState::Stopped);
    assert_eq!(b.runtime_probe_id, None);

    // RT-side removal lands as evidence; the stopped record stays.
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(binding(&engine, &key).state, ProbeResolveState::Stopped);

    // A removed subscription cannot be commanded.
    assert!(matches!(
        engine.probe_set_mode(key, ProbeMode::Capture),
        Err(ProbeRequestError::NotBound)
    ));
    assert!(matches!(
        engine.probe_capture(key, 1_024),
        Err(ProbeRequestError::NotBound)
    ));
    // Re-removal is idempotent; an unknown key is rejected.
    engine.probe_remove(key).unwrap();
    assert!(matches!(
        engine.probe_remove(strip_key(StripId::new(555), SignalBoundary::Output)),
        Err(ProbeRequestError::UnknownKey)
    ));
}
