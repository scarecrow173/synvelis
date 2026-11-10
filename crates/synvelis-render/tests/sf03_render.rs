//! SF-03 slice 7 — cross-consumer same-Revision fixture (Issue #106,
//! §8.2-1, scoped to render + player routing + project routing).
//!
//! One committed project revision is lowered by three consumers:
//! the offline export (`synvelis-render`, `BuildMode::Offline`), the
//! realtime routing engine (`synvelis-player`, `BuildMode::Realtime`)
//! and the persisted semantic model itself (`synvelis-project`).
//! Every consumer's [`RoutingSourceMap`] must point at the *same*
//! project revision, routing fingerprint and capability generation,
//! resolve the *same* semantic IDs, and fail closed with the *same*
//! [`RoutingFault`] class on the same defect — never a parallel
//! legacy path, never fake silence.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::EngineConfig;
use synvelis_player::resolver::prepare;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::routing_lower::GainProcessor;
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    CounterIdGenerator, LOWERING_POLICY_VERSION, MediaDuration, MediaTimeValue, PlaybackContentRef,
    PlaybackMapping, PortDirection, PortRole, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, RoutingCompileRequest, RoutingFault, RoutingSourceMap, RuntimeClockEpochRef,
    RuntimeInstanceId, SemanticNodeRef, SourceFrame, SourceId, SourceRange, TimeAnchor,
    port_capability_generation, routing_fingerprint,
};
use synvelis_render::job::{ExportJob, ExportOutcome, ExportStage};
use synvelis_render::snapshot::RenderSnapshot;
use synvelis_render::spec::{ExportRequest, OutputHooks, TailPolicy};
use synvelis_render::{PreflightError, export, export_with_host};

use ruviform_audio::typed::{GraphProcessor, NodeIo, OfflineRenderSupport};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;
use ruviform_rt::RtContext;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03render-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0x106), "sf03render"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap()
}

fn media_placement(start_frame: u64, end_frame: u64, rate: u32) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(
            MediaTimeValue::new(i64::try_from(start_frame).unwrap(), 1, rate).unwrap(),
        ),
        extent: ClipExtent::Absolute(
            MediaDuration::new(i64::try_from(end_frame - start_frame).unwrap(), 1, rate).unwrap(),
        ),
    }
}

fn natural_playback(offset: i64, frames: i64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(offset).unwrap(),
            SourceFrame::new(offset + frames).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn import_stereo(c: &mut ProjectController, root: &Path, frames: usize) -> SourceId {
    let wav = root.join("s.wav");
    write_fixture(&wav, 2, 48_000, frames).unwrap();
    import_local(
        c,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap()
    .commit
    .source_id
}

/// Track + strip + lane — the semantic chain every clip needs.
fn lane(c: &mut ProjectController) -> synvelis_project::LaneId {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    *c.live().tracks[&track].lane_ids.last().unwrap()
}

fn add_clip(
    c: &mut ProjectController,
    lane: synvelis_project::LaneId,
    source: SourceId,
    start: u64,
    end: u64,
) -> ClipId {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(start, end, 48_000),
            playback: natural_playback(0, i64::try_from(end - start).unwrap()),
        },
    );
    *c.live().clips.keys().next_back().unwrap()
}

fn engine_config(channels: u16) -> EngineConfig {
    EngineConfig {
        sample_rate: 48_000,
        channels,
        block_frames: 256,
    }
}

fn export_request(destination: PathBuf, channels: u16) -> ExportRequest {
    ExportRequest {
        engine: engine_config(channels),
        range_start: 0,
        range_end: 512,
        tail: Some(TailPolicy::None),
        preroll: 0,
        destination,
        replace_existing: false,
        wire_format: SampleFormat::F32,
        inserts: Vec::new(),
        max_block: 256,
        clip_support: OfflineRenderSupport::Verified,
        plugin_state: None,
    }
}

fn completed(outcome: ExportOutcome) -> synvelis_render::ExportReceipt {
    match outcome {
        ExportOutcome::Completed { receipt } => *receipt,
        other => panic!("expected Completed, got {other:?}"),
    }
}

fn canonical_format() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn prefetch() -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: 2,
        sample_rate: 48_000,
    }
}

fn routing_deps<'a>(
    project: &Project,
    media_root: &Path,
    canonical: &'a dyn Fn(SourceId) -> Option<AudioFormat>,
) -> RoutingDeps<'a> {
    RoutingDeps {
        canonical_of: canonical,
        clips: project
            .clips
            .values()
            .map(|clip| prepare(project, clip.source_id, media_root, &prefetch()).unwrap())
            .collect(),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
}

fn realtime_request(revision: ProjectRevision) -> RoutingCompileRequest {
    RoutingCompileRequest {
        request_id: synvelis_project::RoutingBuildRequestId(1),
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

/// The identity fields every consumer's build key/source map must
/// agree on for the same committed revision — `build_mode`, runtime
/// instance/epoch and generations legitimately differ per consumer.
fn assert_same_revision_evidence(
    map: &RoutingSourceMap,
    project: &Project,
    revision: ProjectRevision,
    mode: BuildMode,
) {
    assert_eq!(map.project_id, project.project_id);
    assert_eq!(map.project_revision, revision);
    assert_eq!(map.build_key.project_id, project.project_id);
    assert_eq!(map.build_key.project_revision, revision);
    assert_eq!(map.build_key.build_mode, mode);
    assert_eq!(
        map.build_key.routing_fingerprint,
        routing_fingerprint(project)
    );
    assert_eq!(
        map.build_key.capability_generation,
        port_capability_generation(project)
    );
    assert_eq!(
        map.build_key.lowering_policy_version,
        LOWERING_POLICY_VERSION
    );
}

/// §8.2-1: render (Offline), player routing (Realtime) and the
/// persisted semantic model resolve the same committed revision —
/// same semantic IDs, same revision evidence, consumer-scoped
/// runtime IDs. The export also proves the explicit output tap:
/// a mono `engine.channels` request renders a stereo semantic graph
/// through the sink-edge channel map, never an implicit downmix.
#[allow(clippy::too_many_lines)] // sequential consumer evidence assertions
#[test]
fn sf03_render_01_same_revision_across_consumers() {
    let root = scratch("same-rev");
    let mut c = controller();
    let source = import_stereo(&mut c, &root, 4_800);
    let lane = lane(&mut c);
    let clip = add_clip(&mut c, lane, source, 0, 512);
    let revision = c.live_revision();
    let project = c.live().clone();

    // ---- consumer 1: offline export (mono output tap) --------------
    let snapshot = RenderSnapshot::freeze(&c, root.join("media"));
    assert_eq!(snapshot.ids.live_revision, revision);
    let receipt = completed(export(
        &snapshot,
        &export_request(root.join("out.wav"), 1),
        OutputHooks::default(),
        &ExportJob::new(),
    ));
    assert!(root.join("out.wav").exists());
    let render_map = &receipt.source_map;
    assert_same_revision_evidence(render_map, &project, revision, BuildMode::Offline);
    assert_eq!(render_map.generation, 1);

    // ---- consumer 2: realtime routing engine ------------------------
    let mut engine = RoutingEngine::new(
        engine_config(2),
        RuntimeInstanceId(1),
        RuntimeClockEpochRef(1),
    );
    let deps = routing_deps(c.live(), &root.join("media"), &|_| Some(canonical_format()));
    let prepared = engine
        .prepare(&realtime_request(revision), c.live(), revision, deps)
        .unwrap()
        .expect_built();
    let player_map = prepared.source_map.clone();
    assert_same_revision_evidence(&player_map, &project, revision, BuildMode::Realtime);
    engine.publish(prepared, c.live(), revision).unwrap();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    // Published + RT-observed at the same committed revision.
    assert!(matches!(
        engine.receipt().unwrap().state,
        synvelis_project::RoutingActivationState::Active { .. }
    ));
    assert_eq!(
        engine.source_map().unwrap().project_revision,
        render_map.project_revision
    );

    // ---- consumer 3: the persisted semantic model -------------------
    // Both maps' semantic side must be a projection of THIS routing
    // model: every resolved semantic node/port/connection key exists
    // in `project.routing`, and both consumers bind the same clip to
    // the same semantic destination port.
    for map in [render_map, &player_map] {
        for node in map.semantic_nodes.keys() {
            match node {
                SemanticNodeRef::Strip(id) => {
                    assert!(project.routing.strips.contains_key(id), "{node:?}");
                }
                SemanticNodeRef::Bus(id) => {
                    assert!(project.routing.buses.contains_key(id), "{node:?}");
                }
                SemanticNodeRef::Processor(id) => {
                    assert!(project.routing.processors.contains_key(id), "{node:?}");
                }
            }
        }
        for port in map.ports.keys() {
            assert!(project.routing.ports.contains_key(port), "{port:?}");
        }
        for playback in &map.playback_bindings {
            assert!(
                project.routing.ports.contains_key(&playback.destination),
                "{:?}",
                playback.destination
            );
        }
    }

    // Same semantic provenance → same (content, destination) pairs;
    // only the runtime NodeIds may differ between consumers.
    let semantic_bindings = |map: &RoutingSourceMap| {
        map.playback_bindings
            .iter()
            .map(|b| {
                (
                    match b.content {
                        PlaybackContentRef::Clip(id) => format!("clip:{id}"),
                        PlaybackContentRef::CompSegment { comp_id, region_id } => {
                            format!("segment:{comp_id}:{region_id}")
                        }
                        PlaybackContentRef::CompOverlap {
                            comp_id,
                            transition_id,
                            side,
                        } => format!("overlap:{comp_id}:{transition_id}:{side:?}"),
                    },
                    b.destination,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        semantic_bindings(render_map),
        semantic_bindings(&player_map)
    );
    assert_eq!(render_map.playback_bindings.len(), 1);
    assert_eq!(
        render_map.playback_bindings[0].content,
        PlaybackContentRef::Clip(clip)
    );
    // The clip's destination is its track strip's main input — the
    // same semantic port both consumers resolved.
    let strip = project
        .routing
        .strip_of_track(
            project
                .clips
                .get(&clip)
                .map(|c| c.lane_id)
                .and_then(|lane| project.lanes.get(&lane))
                .map(|l| l.track_id)
                .unwrap(),
        )
        .unwrap()
        .id;
    let strip_in = project.routing.strips[&strip]
        .port_ids
        .iter()
        .copied()
        .find(|id| {
            let p = &project.routing.ports[id];
            p.direction == PortDirection::Input && p.role == PortRole::Main
        })
        .unwrap();
    assert_eq!(
        render_map.playback_bindings[0].destination,
        player_map.playback_bindings[0].destination
    );
    assert_eq!(render_map.playback_bindings[0].destination, strip_in);
    // Runtime instance/epoch legitimately differ per consumer —
    // semantic identity is what must agree.
    assert_ne!(
        render_map.build_key.runtime_instance,
        player_map.build_key.runtime_instance
    );
}

/// A gain instance that honestly reports `Unknown` offline support —
/// the offline gate must reject it in *both* consumers.
struct UnknownGain(f32);

impl GraphProcessor for UnknownGain {
    fn process(&mut self, io: &mut NodeIo<'_>, ctx: &RtContext) {
        GainProcessor::new(self.0).process(io, ctx);
    }
}

/// Same gain DSP but honestly reports `Verified` — the positive
/// control proving the offline gate checks the capability claim,
/// not the processor kind.
struct VerifiedGain(f32);

impl GraphProcessor for VerifiedGain {
    fn process(&mut self, io: &mut NodeIo<'_>, ctx: &RtContext) {
        GainProcessor::new(self.0).process(io, ctx);
    }
    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }
}

/// SF-ACT-07 deterministic half: the same semantic defect produces
/// the same [`RoutingFault`] class in the offline export and the
/// realtime engine — missing strip, unprovisioned chain processor,
/// uncertified offline instance — each fail-closed before any output.
#[allow(clippy::too_many_lines)] // sequential refusal-case assertions
#[test]
fn sf03_render_02_fail_closed_same_fault() {
    let root = scratch("fail-closed");

    // --- missing strip: clip's track owns no channel strip ---------
    let mut c = controller();
    let source = import_stereo(&mut c, &root, 4_800);
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane_id = *c.live().tracks[&track].lane_ids.last().unwrap();
    let _clip = add_clip(&mut c, lane_id, source, 0, 512);
    let revision = c.live_revision();

    let snapshot = RenderSnapshot::freeze(&c, root.join("media"));
    let dest = root.join("missing-strip.wav");
    let outcome = export(
        &snapshot,
        &export_request(dest.clone(), 2),
        OutputHooks::default(),
        &ExportJob::new(),
    );
    let ExportOutcome::Failed { stage, error } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert_eq!(stage, ExportStage::Preflight);
    let synvelis_render::ExportError::Preflight(PreflightError::RoutingLowering { fault, .. }) =
        error
    else {
        panic!("expected RoutingLowering, got {error:?}");
    };
    assert_eq!(fault, RoutingFault::InvalidRouting);
    assert!(!dest.exists());

    // The realtime consumer fails the same lowering with the same
    // fault — preview and export agree, no fake silence anywhere.
    let mut engine = RoutingEngine::new(
        engine_config(2),
        RuntimeInstanceId(1),
        RuntimeClockEpochRef(1),
    );
    let deps = routing_deps(c.live(), &root.join("media"), &|_| Some(canonical_format()));
    assert_eq!(
        engine
            .prepare(&realtime_request(revision), c.live(), revision, deps)
            .err()
            .unwrap(),
        RoutingFault::InvalidRouting
    );

    // --- unprovisioned chain processor: no host → PluginUnavailable --
    let mut c2 = controller();
    let source2 = import_stereo(&mut c2, &root, 4_800);
    let lane2 = lane(&mut c2);
    let _clip2 = add_clip(&mut c2, lane2, source2, 0, 512);
    let track2 = c2
        .live()
        .clips
        .values()
        .next()
        .and_then(|cl| c2.live().lanes.get(&cl.lane_id))
        .map(|l| l.track_id)
        .unwrap();
    let strip2 = c2.live().routing.strip_of_track(track2).unwrap().id;
    apply(
        &mut c2,
        ProjectOp::InsertProcessor {
            strip_id: strip2,
            index: 0,
            name: "gain".into(),
            binding: synvelis_project::ProcessorBinding::Unresolved {
                descriptor: "test.gain".into(),
            },
            sidechain_input: false,
        },
    );
    let revision2 = c2.live_revision();
    let snapshot2 = RenderSnapshot::freeze(&c2, root.join("media"));
    let dest2 = root.join("no-host.wav");
    let outcome = export(
        &snapshot2,
        &export_request(dest2.clone(), 2),
        OutputHooks::default(),
        &ExportJob::new(),
    );
    let ExportOutcome::Failed { error, .. } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    let synvelis_render::ExportError::Preflight(PreflightError::RoutingLowering { fault, .. }) =
        error
    else {
        panic!("expected RoutingLowering, got {error:?}");
    };
    assert_eq!(fault, RoutingFault::PluginUnavailable);
    assert!(!dest2.exists());

    // --- uncertified offline instance: export_with_host fails closed --
    let processor_id = *c2.live().routing.processors.keys().next().unwrap();
    let mut host = synvelis_player::StaticProcessorHost::new();
    host.insert(processor_id, Box::new(UnknownGain(0.5)));
    let outcome = export_with_host(
        &snapshot2,
        &export_request(root.join("uncertified.wav"), 2),
        OutputHooks::default(),
        &ExportJob::new(),
        &mut host,
    );
    let ExportOutcome::Failed { error, .. } = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    let synvelis_render::ExportError::Preflight(PreflightError::RoutingLowering { fault, .. }) =
        error
    else {
        panic!("expected RoutingLowering, got {error:?}");
    };
    assert_eq!(fault, RoutingFault::OfflineCapabilityMismatch);
    assert!(!root.join("uncertified.wav").exists());

    // The realtime offline-mode prepare rejects the same instance.
    let mut engine2 = RoutingEngine::new(
        engine_config(2),
        RuntimeInstanceId(1),
        RuntimeClockEpochRef(1),
    );
    let mut host2 = synvelis_player::StaticProcessorHost::new();
    host2.insert(processor_id, Box::new(UnknownGain(0.5)));
    engine2.set_processor_host(Box::new(host2));
    let mut offline_req = realtime_request(revision2);
    offline_req.mode = BuildMode::Offline;
    let deps2 = routing_deps(
        c2.live(),
        &root.join("media"),
        &|_| Some(canonical_format()),
    );
    assert_eq!(
        engine2
            .prepare(&offline_req, c2.live(), revision2, deps2)
            .err()
            .unwrap(),
        RoutingFault::OfflineCapabilityMismatch
    );

    // A *verified* instance completes the same export — the failure
    // was the capability claim, not the topology.
    let mut host3 = synvelis_player::StaticProcessorHost::new();
    host3.insert(processor_id, Box::new(VerifiedGain(0.5)));
    completed(export_with_host(
        &snapshot2,
        &export_request(root.join("verified.wav"), 2),
        OutputHooks::default(),
        &ExportJob::new(),
        &mut host3,
    ));
    assert!(root.join("verified.wav").exists());
}
