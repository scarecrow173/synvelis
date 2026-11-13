//! SF-PROBE acceptance audit (Issue #107): headless evidence for the
//! SF-PROBE-01..08 cases the slice fixtures do not already cover —
//! a bound probe whose semantic entity disappears must unbind
//! honestly on republish (`SourceMap` invalidation → `Unresolved`,
//! never a stale-slot observation), a re-attached entity binds fresh
//! under the new generation, and the player layer's subscription
//! dedup is exactly per [`ProbeSubscriptionKey`] (cross-window
//! lease/ownership aggregation is the session layer's contract —
//! SF-04 slice5). Everything below is proven through RT-observed
//! status evidence drained via `probe_drain`; no OS/RT/plugin
//! observation is claimed.

use synvelis_player::ProbeRequestError;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::lowering::EngineConfig;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_project::probe::{
    ProbeMode, ProbeResolveState, SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary,
    SubscriptionKind,
};
use synvelis_project::{
    BuildBudget, BuildMode, Command, CounterIdGenerator, ProbeSubscriptionKey, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, ProjectSessionId,
    RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef, RuntimeInstanceId, StripId,
    TrackId,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(u128::from(0x107_u64)), "sf04-acceptance"),
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

fn gain_binding() -> synvelis_project::ProcessorBinding {
    synvelis_project::ProcessorBinding::Resolved(synvelis_project::PluginRef {
        descriptor: "vendor.eq".into(),
        version: "1.0".into(),
        capability_revision: 3,
    })
}

fn insert_processor(c: &mut ProjectController, strip: StripId) -> synvelis_project::ProcessorId {
    apply_op(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "eq".into(),
            binding: gain_binding(),
            sidechain_input: false,
        },
    )
}

fn host_with(
    processors: &[synvelis_project::ProcessorId],
) -> synvelis_player::host::StaticProcessorHost {
    let mut host = synvelis_player::host::StaticProcessorHost::new();
    for &processor in processors {
        host.insert(
            processor,
            Box::new(synvelis_player::routing_lower::GainProcessor::new(1.0)),
        );
    }
    host
}

// ── SF-PROBE-01/03: entity loss → honest unbind, fresh re-attach ──

#[test]
fn sf04_probe_removed_entity_unbinds_and_reattach_binds_fresh() {
    let mut c = controller();
    let (_track, strip) = add_track_strip(&mut c, "t");
    let processor = insert_processor(&mut c, strip);

    let mut engine = new_engine();
    engine.set_processor_host(Box::new(host_with(&[processor])));
    publish(&mut engine, &c, 1);

    // Bound probe on the processor's main output.
    let key = key_for(
        SemanticSignalEntityRef::Processor(processor),
        SignalBoundary::Output,
    );
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(binding(&engine, &key).state, ProbeResolveState::BoundActive);
    let first_probe = binding(&engine, &key).runtime_probe_id.unwrap();

    // The entity disappears — SourceMap invalidation must unbind the
    // probe honestly on republish, never keep observing a stale slot.
    apply(
        &mut c,
        ProjectOp::RemoveProcessor {
            processor_id: processor,
        },
    );
    publish(&mut engine, &c, 2);
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    let gone = binding(&engine, &key);
    assert_eq!(gone.state, ProbeResolveState::Unresolved);
    assert_eq!(gone.runtime_probe_id, None);
    assert!(
        engine.probe_segments().is_empty(),
        "an unbound probe owns no capture segment"
    );

    // Re-requesting the dead point is idempotent — it cannot
    // resurrect a binding whose entity no longer exists.
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    assert_eq!(binding(&engine, &key).state, ProbeResolveState::Unresolved);

    // Re-attach: a NEW processor id is a NEW semantic point — fresh
    // resolution under the new generation, never confused with the
    // retired binding (old slot reuse cannot misattribute evidence).
    let processor2 = insert_processor(&mut c, strip);
    engine.set_processor_host(Box::new(host_with(&[processor2])));
    publish(&mut engine, &c, 3);

    let key2 = key_for(
        SemanticSignalEntityRef::Processor(processor2),
        SignalBoundary::Output,
    );
    engine
        .probe_request(c.live(), key2, ProbeMode::Monitor)
        .unwrap();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    let rebound = binding(&engine, &key2);
    assert_eq!(rebound.state, ProbeResolveState::BoundActive);
    assert_eq!(rebound.active_plan_generation, 3);
    assert_ne!(
        rebound.runtime_probe_id.unwrap(),
        first_probe,
        "a re-attached point gets a fresh runtime probe"
    );
    assert_eq!(
        binding(&engine, &key).state,
        ProbeResolveState::Unresolved,
        "the retired binding stays honestly unresolved"
    );
}

// ── SF-PROBE-06: per-key dedup; session boundary is honest ──────

#[test]
fn sf04_probe_same_key_request_is_idempotent_single_install() {
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
    let probe = binding(&engine, &key).runtime_probe_id.unwrap();

    // A second request for the SAME subscription key is idempotent —
    // no second broker install. Mode changes go through
    // `probe_set_mode`, never through re-request.
    engine
        .probe_request(c.live(), key, ProbeMode::Capture)
        .unwrap();
    assert_eq!(binding(&engine, &key).runtime_probe_id, Some(probe));
    assert_eq!(binding(&engine, &key).requested_mode, ProbeMode::Monitor);

    // Capture is a single explicit session per binding: a duplicate
    // request cannot open a second one.
    engine.probe_capture(key, 4_096).unwrap();
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    assert!(matches!(
        engine.probe_capture(key, 1_024),
        Err(ProbeRequestError::AlreadyCapturing)
    ));
    engine.probe_stop_capture(key).unwrap();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    // A DIFFERENT session id on the same point is a different
    // subscription — it installs its own bounded broker probe. The
    // player layer dedups per key only; cross-window lease/ownership
    // aggregation is the session layer's contract (SF-04 slice5).
    let other = ProbeSubscriptionKey {
        session_id: ProjectSessionId(2),
        ..key
    };
    engine
        .probe_request(c.live(), other, ProbeMode::Monitor)
        .unwrap();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    let other_binding = binding(&engine, &other);
    assert_eq!(other_binding.state, ProbeResolveState::BoundActive);
    assert_ne!(
        other_binding.runtime_probe_id.unwrap(),
        probe,
        "distinct subscription keys get distinct bounded probes"
    );
}
