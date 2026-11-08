//! SF-03 slice 2 acceptance: the 18-category `RoutingFault`
//! taxonomy, last-good retention across every failure path, retry via
//! fresh builds, `reconcile` drift detection, and the offline
//! capability gate (Issue #106 slice 2).
//!
//! Deterministic evidence: every fault asserts the typed
//! [`RoutingFault`] and the retained last-good generation — never a
//! silent success. Mutated-project cases clone the committed
//! snapshot: the engine keys the build off the clone itself, so the
//! fault under test is the lowering/publish classification, not a
//! stale-revision rejection.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::ImportOutcome;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::resolver::prepare;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::routing_lower::GainProcessor;
use synvelis_project::{
    BuildBudget, BuildMode, BusKind, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    ConnectionKind, CounterIdGenerator, MediaDuration, MediaTimeValue, PlaybackMapping,
    PortDirection, PortRole, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    RoutingConnection, RoutingConnectionId, RoutingFault, RoutingReconcileMismatch,
    RuntimeClockEpochRef, RuntimeInstanceId, SemanticPortId, SendTap, SourceFrame, SourceId,
    SourceRange, StripId, TimeAnchor, routing_fingerprint,
};

use ruviform_audio::typed::{GraphProcessor, NodeIo, OfflineRenderSupport};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;
use ruviform_rt::RtContext;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03l2-{}-{}-{}",
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
        Project::new(ProjectId::new(0x106), "sf03l2"),
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

fn engine() -> synvelis_player::lowering::EngineConfig {
    synvelis_player::lowering::EngineConfig {
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

fn prefetch() -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: 2,
        sample_rate: 48_000,
    }
}

fn request(id: u64, revision: ProjectRevision) -> synvelis_project::RoutingCompileRequest {
    synvelis_project::RoutingCompileRequest {
        request_id: synvelis_project::RoutingBuildRequestId(id),
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

fn import_stereo(
    c: &mut ProjectController,
    root: &Path,
    name: &str,
    frames: usize,
) -> ImportOutcome {
    let wav = root.join(name);
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
}

fn add_track_strip(
    c: &mut ProjectController,
) -> (synvelis_project::TrackId, synvelis_project::LaneId) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    (track, lane)
}

fn add_clip(
    c: &mut ProjectController,
    lane: synvelis_project::LaneId,
    source: SourceId,
    start: u64,
    end: u64,
    offset: i64,
) -> ClipId {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(start, end, 48_000),
            playback: natural_playback(offset, i64::try_from(end - start).unwrap()),
        },
    );
    *c.live().clips.keys().next_back().unwrap()
}

fn strip_of_track(project: &Project, track: synvelis_project::TrackId) -> StripId {
    project.routing.strip_of_track(track).map(|s| s.id).unwrap()
}

fn master_strip(project: &Project) -> StripId {
    project.routing.buses[&project.routing.master].strip
}

fn strip_port(
    project: &Project,
    strip: StripId,
    direction: PortDirection,
    role: PortRole,
) -> SemanticPortId {
    project.routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| {
            project.routing.ports[id].direction == direction
                && project.routing.ports[id].role == role
        })
        .copied()
        .unwrap()
}

fn prepared_clips(
    project: &Project,
    media_root: &Path,
) -> Vec<synvelis_player::resolver::PreparedClip> {
    project
        .clips
        .values()
        .map(|clip| prepare(project, clip.source_id, media_root, &prefetch()).unwrap())
        .collect()
}

/// `RoutingDeps` over the module canonical resolver and prepared
/// clips. `canonical`/`generations` allow fault-injection variants.
fn deps_for<'a>(
    project: &Project,
    media_root: &Path,
    canonical: &'a dyn Fn(SourceId) -> Option<AudioFormat>,
    engine_config_generation: u64,
    source_format_generation: u64,
) -> RoutingDeps<'a> {
    RoutingDeps {
        canonical_of: canonical,
        clips: prepared_clips(project, media_root),
        engine_config_generation,
        source_format_generation,
    }
}

fn deps_default(project: &Project, media_root: &Path) -> RoutingDeps<'static> {
    deps_for(project, media_root, &|_| Some(canonical_format()), 1, 1)
}

/// Adds a group bus and a post-fader send from `track`'s strip to the
/// bus strip — returns the send's destination port for mutation
/// tests.
fn wire_send(c: &mut ProjectController, track: synvelis_project::TrackId) -> SemanticPortId {
    apply(
        c,
        ProjectOp::AddBus {
            kind: BusKind::Group,
            name: "fx".into(),
        },
    );
    let bus = *c
        .live()
        .routing
        .buses
        .keys()
        .find(|b| **b != c.live().routing.master)
        .unwrap();
    let bus_strip = c.live().routing.buses[&bus].strip;
    let track_out = strip_port(
        c.live(),
        strip_of_track(c.live(), track),
        PortDirection::Output,
        PortRole::Main,
    );
    let bus_in = strip_port(c.live(), bus_strip, PortDirection::Input, PortRole::Main);
    apply(
        c,
        ProjectOp::AddSend {
            source: track_out,
            destination: bus_in,
            tap: SendTap::PostFader,
            gain_target: None,
        },
    );
    bus_in
}

/// Minimal project: import + track/strip/lane + one clip. Returns
/// (controller, media root).
fn minimal_project(root: &Path) -> (ProjectController, PathBuf) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    (c, root.join("media"))
}

/// prepare→publish→`run` for the current live revision.
fn drive(
    eng: &mut RoutingEngine,
    c: &mut ProjectController,
    media: &Path,
    request_id: u64,
    blocks: usize,
) -> synvelis_player::PlayReport {
    let revision = c.live_revision();
    let prepared = eng
        .prepare(
            &request(request_id, revision),
            c.live(),
            revision,
            deps_default(c.live(), media),
        )
        .unwrap();
    eng.publish(prepared, c.live(), revision).unwrap();
    eng.run(blocks, PumpPolicy::Every(1)).unwrap()
}

// ------------------------------------------------------- prepare faults

/// Slice-2 fault taxonomy: every `checked_key`/lowering/skip path
/// mints its Issue #106 category deterministically.
#[test]
fn sf03_l2_01_prepare_fault_taxonomy() {
    let root = scratch("l201");
    let (mut c, media) = minimal_project(&root);
    let revision = c.live_revision();
    let mut eng = new_engine();

    // StaleProjectSnapshot: expected revision does not match.
    let stale = synvelis_project::RoutingCompileRequest {
        expected_project_revision: ProjectRevision(revision.0 + 1),
        ..request(1, revision)
    };
    assert_eq!(
        eng.prepare(&stale, c.live(), revision, deps_default(c.live(), &media))
            .err()
            .unwrap(),
        RoutingFault::StaleProjectSnapshot
    );

    // DeviceEpochChanged: foreign runtime instance / clock epoch.
    for req in [
        synvelis_project::RoutingCompileRequest {
            runtime_instance: RuntimeInstanceId(9),
            ..request(2, revision)
        },
        synvelis_project::RoutingCompileRequest {
            device_clock_epoch: RuntimeClockEpochRef(9),
            ..request(3, revision)
        },
    ] {
        assert_eq!(
            eng.prepare(&req, c.live(), revision, deps_default(c.live(), &media))
                .err()
                .unwrap(),
            RoutingFault::DeviceEpochChanged
        );
    }

    // CompileResourceExceeded: frame budget below the engine block.
    let tiny = synvelis_project::RoutingCompileRequest {
        resource_budget: BuildBudget {
            max_frames: 128,
            ..request(4, revision).resource_budget
        },
        ..request(4, revision)
    };
    assert_eq!(
        eng.prepare(&tiny, c.live(), revision, deps_default(c.live(), &media))
            .err()
            .unwrap(),
        RoutingFault::CompileResourceExceeded
    );

    // CompileResourceExceeded: lowering node budget.
    let node_tiny = synvelis_project::RoutingCompileRequest {
        resource_budget: BuildBudget {
            max_nodes: 1,
            ..request(5, revision).resource_budget
        },
        ..request(5, revision)
    };
    assert_eq!(
        eng.prepare(
            &node_tiny,
            c.live(),
            revision,
            deps_default(c.live(), &media)
        )
        .err()
        .unwrap(),
        RoutingFault::CompileResourceExceeded
    );

    // MissingSource: canonical format unresolvable → fail closed,
    // never fake silence.
    let no_canonical = deps_for(c.live(), &media, &|_| None, 1, 1);
    assert_eq!(
        eng.prepare(&request(6, revision), c.live(), revision, no_canonical)
            .err()
            .unwrap(),
        RoutingFault::MissingSource
    );

    // UnsupportedFormatOrTimeMap: source rate differs from engine —
    // implicit resample is forbidden.
    let wrong_rate = AudioFormat {
        sample_rate: SampleRate::new(44_100).unwrap(),
        ..canonical_format()
    };
    let wrong_rate_of = move |_: SourceId| Some(wrong_rate);
    let rate_deps = deps_for(c.live(), &media, &wrong_rate_of, 1, 1);
    assert_eq!(
        eng.prepare(&request(7, revision), c.live(), revision, rate_deps)
            .err()
            .unwrap(),
        RoutingFault::UnsupportedFormatOrTimeMap
    );

    // Intentional silence is allowed: a muted clip lowers to nothing
    // by contract and the build still succeeds. `deps.clips` covers
    // exactly the lowered playbacks — the muted clip needs none.
    let clip = *c.live().clips.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    let rev2 = c.live_revision();
    let mut deps = deps_default(c.live(), &media);
    deps.clips.clear();
    let prepared = eng
        .prepare(&request(8, rev2), c.live(), rev2, deps)
        .unwrap();
    eng.publish(prepared, c.live(), rev2).unwrap();
}

/// Mutated-clone fault paths: dangling port references,
/// channel-layout mismatch, broken structure and graph cycles all
/// mint their typed fault — the clone is self-consistent, so the
/// fault is the lowering classification itself.
#[test]
fn sf03_l2_02_lowering_fault_taxonomy() {
    let root = scratch("l202");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let bus_in = wire_send(&mut c, track);
    let revision = c.live_revision();
    let mut eng = new_engine();

    // PortUnresolved: the send's destination port entity is removed —
    // the connection reference can no longer resolve.
    let mut broken = c.live().clone();
    broken.routing.ports.remove(&bus_in);
    assert_eq!(
        eng.prepare(
            &request(1, revision),
            &broken,
            revision,
            deps_default(&broken, &media),
        )
        .err()
        .unwrap(),
        RoutingFault::PortUnresolved
    );

    // UnsupportedFormatOrTimeMap: a strip port's channel count
    // disagrees with the engine format.
    let mut narrow = c.live().clone();
    let port_id = strip_port(
        &narrow,
        strip_of_track(&narrow, track),
        PortDirection::Input,
        PortRole::Main,
    );
    narrow.routing.ports.get_mut(&port_id).unwrap().channels = 1;
    assert_eq!(
        eng.prepare(
            &request(2, revision),
            &narrow,
            revision,
            deps_default(&narrow, &media),
        )
        .err()
        .unwrap(),
        RoutingFault::UnsupportedFormatOrTimeMap
    );

    // InvalidRouting: the master bus is gone — no legal termination.
    let mut headless = c.live().clone();
    let master_id = headless.routing.master;
    headless.routing.buses.remove(&master_id);
    assert_eq!(
        eng.prepare(
            &request(3, revision),
            &headless,
            revision,
            deps_default(&headless, &media),
        )
        .err()
        .unwrap(),
        RoutingFault::InvalidRouting
    );

    // Cycle: master output feeds the track strip input while the
    // track already feeds master — a zero-delay graph cycle.
    let mut cyclic = c.live().clone();
    let master = master_strip(&cyclic);
    let master_out = strip_port(&cyclic, master, PortDirection::Output, PortRole::Main);
    let track_in = strip_port(
        &cyclic,
        strip_of_track(&cyclic, track),
        PortDirection::Input,
        PortRole::Main,
    );
    let conn_id = RoutingConnectionId::new(0xDEAD);
    cyclic.routing.connections.insert(
        conn_id,
        RoutingConnection {
            id: conn_id,
            source: master_out,
            destination: track_in,
            kind: ConnectionKind::Main,
            enabled: true,
            channel_map: None,
        },
    );
    assert_eq!(
        eng.prepare(
            &request(4, revision),
            &cyclic,
            revision,
            deps_default(&cyclic, &media),
        )
        .err()
        .unwrap(),
        RoutingFault::Cycle
    );
}

// ------------------------------------------------------ publish faults

/// `publish` re-validation: semantic drift is `SpeculativeMismatch`,
/// an incoherent prepared bundle is `BadSourceMap`, and an
/// out-of-order generation is `Superseded` — each leaves last-good
/// untouched.
#[test]
fn sf03_l2_03_publish_fault_taxonomy() {
    let root = scratch("l203");
    let (mut c, media) = minimal_project(&root);
    let revision = c.live_revision();
    let mut eng = new_engine();

    // Activate gen 1 so last-good exists.
    drive(&mut eng, &mut c, &media, 1, 1);
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));

    // SpeculativeMismatch: same revision, different semantic content.
    let prepared = eng
        .prepare(
            &request(2, revision),
            c.live(),
            revision,
            deps_default(c.live(), &media),
        )
        .unwrap();
    let mut speculative = c.live().clone();
    let clip = *speculative.clips.keys().next().unwrap();
    speculative.clips.get_mut(&clip).unwrap().muted = true;
    assert_ne!(
        routing_fingerprint(&speculative),
        routing_fingerprint(c.live())
    );
    let err = eng.publish(prepared, &speculative, revision).err().unwrap();
    assert_eq!(err, RoutingFault::SpeculativeMismatch);
    assert!(matches!(
        eng.receipt().unwrap().state,
        synvelis_project::RoutingActivationState::Blocked {
            fault: RoutingFault::SpeculativeMismatch,
            last_good_generation: Some(1),
        }
    ));

    // BadSourceMap: the prepared bundle's source map generation does
    // not match the compiled plan generation.
    let mut incoherent = eng
        .prepare(
            &request(3, revision),
            c.live(),
            revision,
            deps_default(c.live(), &media),
        )
        .unwrap();
    incoherent.source_map.generation = incoherent.generation + 100;
    let err = eng.publish(incoherent, c.live(), revision).err().unwrap();
    assert_eq!(err, RoutingFault::BadSourceMap);
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));

    // Superseded: publish the newer generation first, then an older
    // prepared plan — the older one must not displace it.
    let p_old = eng
        .prepare(
            &request(4, revision),
            c.live(),
            revision,
            deps_default(c.live(), &media),
        )
        .unwrap();
    let p_new = eng
        .prepare(
            &request(5, revision),
            c.live(),
            revision,
            deps_default(c.live(), &media),
        )
        .unwrap();
    let new_gen = p_new.generation;
    eng.publish(p_new, c.live(), revision).unwrap();
    let err = eng.publish(p_old, c.live(), revision).err().unwrap();
    assert_eq!(err, RoutingFault::Superseded);
    assert!(matches!(
        eng.receipt().unwrap().state,
        synvelis_project::RoutingActivationState::Blocked {
            fault: RoutingFault::Superseded,
            last_good_generation: Some(1),
        }
    ));

    // The newer plan still activates — last-good advances past the
    // rejected older publish.
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(eng.rt_status().active_generation, new_gen);
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(new_gen));
}

// ------------------------------------------------------------- last-good

/// Last-good matrix: after gen 1 is RT-active, every failure path —
/// stale prepare, missing plugin, superseded publish, reconcile
/// drift — preserves `last_good_generation` and the live plan's
/// audio.
#[test]
fn sf03_l2_04_last_good_matrix() {
    let root = scratch("l204");
    let (mut c, media) = minimal_project(&root);
    let mut eng = new_engine();

    let rev1 = c.live_revision();
    let before = drive(&mut eng, &mut c, &media, 1, 2);
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));

    // (a) Stale prepare.
    let stale = synvelis_project::RoutingCompileRequest {
        expected_project_revision: ProjectRevision(rev1.0 + 9),
        ..request(2, rev1)
    };
    assert_eq!(
        eng.prepare(&stale, c.live(), rev1, deps_default(c.live(), &media))
            .err()
            .unwrap(),
        RoutingFault::StaleProjectSnapshot
    );

    // (b) PluginUnavailable prepare (unresolved chain binding).
    let track = *c.live().tracks.keys().next_back().unwrap();
    let strip = strip_of_track(c.live(), track);
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "gain".into(),
            binding: synvelis_project::ProcessorBinding::Unresolved {
                descriptor: "test.gain".into(),
            },
            sidechain_input: false,
        },
    );
    let rev2 = c.live_revision();
    assert_eq!(
        eng.prepare(
            &request(3, rev2),
            c.live(),
            rev2,
            deps_default(c.live(), &media),
        )
        .err()
        .unwrap(),
        RoutingFault::PluginUnavailable
    );
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));

    // (c) Reconcile drift — the committed revision moved; the report
    // records every aspect and the receipt keeps last-good.
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let rev3 = c.live_revision();
    let report = eng.reconcile(c.live(), rev3, 1, 1);
    assert_eq!(report.fault, Some(RoutingFault::StaleProjectSnapshot));
    assert!(
        report
            .mismatches
            .contains(&RoutingReconcileMismatch::ProjectRevision)
    );
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));
    assert_eq!(eng.rt_status().active_generation, 1);

    // Last-good audio is bit-identical after all the failures.
    eng.seek(0);
    let after = eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(after.pcm, before.pcm);
    assert_eq!(after.crc, before.crc);
}

// ------------------------------------------------------------- reconcile

/// `reconcile` drift matrix: clean state reports nothing; each drift
/// aspect lands in `mismatches` with the precedence-mapped fault;
/// the published plan stays live throughout.
#[test]
fn sf03_l2_05_reconcile_drift_matrix() {
    let root = scratch("l205");
    let (mut c, media) = minimal_project(&root);
    let mut eng = new_engine();

    // Nothing published → empty report.
    let empty = eng.reconcile(c.live(), c.live_revision(), 1, 1);
    assert_eq!(empty.plan_key, None);
    assert_eq!(empty.mismatches, Vec::new());
    assert_eq!(empty.fault, None);

    drive(&mut eng, &mut c, &media, 1, 1);
    let rev1 = c.live_revision();

    // Clean reconcile: same snapshot, same generations.
    let clean = eng.reconcile(c.live(), rev1, 1, 1);
    assert_eq!(clean.mismatches, Vec::new());
    assert_eq!(clean.fault, None);
    assert!(clean.plan_key.is_some());

    // Engine/source-format generation drift → UnsupportedFormatOrTimeMap.
    let fmt = eng.reconcile(c.live(), rev1, 2, 1);
    assert!(
        fmt.mismatches
            .contains(&RoutingReconcileMismatch::EngineConfigGeneration)
    );
    assert_eq!(fmt.fault, Some(RoutingFault::UnsupportedFormatOrTimeMap));
    let fmt2 = eng.reconcile(c.live(), rev1, 1, 7);
    assert!(
        fmt2.mismatches
            .contains(&RoutingReconcileMismatch::SourceFormatGeneration)
    );
    assert_eq!(fmt2.fault, Some(RoutingFault::UnsupportedFormatOrTimeMap));

    // Semantic drift at the same revision → SpeculativeMismatch.
    let mut speculative = c.live().clone();
    let clip = *speculative.clips.keys().next().unwrap();
    speculative.clips.get_mut(&clip).unwrap().muted = true;
    let spec = eng.reconcile(&speculative, rev1, 1, 1);
    assert!(
        spec.mismatches
            .contains(&RoutingReconcileMismatch::RoutingFingerprint)
    );
    assert_eq!(spec.fault, Some(RoutingFault::SpeculativeMismatch));

    // Committed revision moved → StaleProjectSnapshot, and the drift
    // report carries every aspect at once.
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "moved".into(),
        },
    );
    let rev2 = c.live_revision();
    let drift = eng.reconcile(c.live(), rev2, 1, 1);
    assert!(
        drift
            .mismatches
            .contains(&RoutingReconcileMismatch::ProjectRevision)
    );
    assert_eq!(drift.fault, Some(RoutingFault::StaleProjectSnapshot));
    // Name-only change keeps the fingerprint — no speculative drift.
    assert!(
        !drift
            .mismatches
            .contains(&RoutingReconcileMismatch::RoutingFingerprint)
    );
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));
    assert_eq!(eng.rt_status().active_generation, 1);
}

/// A published-but-not-yet-run plan is transient, not a fault:
/// `reconcile` reports no `PendingActivation` while `frames == 0`.
#[test]
fn sf03_l2_06_pending_publish_is_transient() {
    let root = scratch("l206");
    let (c, media) = minimal_project(&root);
    let mut eng = new_engine();
    let rev = c.live_revision();
    let prepared = eng
        .prepare(
            &request(1, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap();
    eng.publish(prepared, c.live(), rev).unwrap();

    let report = eng.reconcile(c.live(), rev, 1, 1);
    assert!(
        !report
            .mismatches
            .contains(&RoutingReconcileMismatch::PendingActivation)
    );
    assert_eq!(report.fault, None);
}

// ------------------------------------------------------------- retry

/// Retry contract: a stale prepared plan is rejected, and the retry
/// is a fresh prepare against the new committed snapshot — new
/// generation, new key, new source map — which then activates.
#[test]
fn sf03_l2_07_retry_uses_fresh_build() {
    let root = scratch("l207");
    let (mut c, media) = minimal_project(&root);
    let mut eng = new_engine();

    let rev1 = c.live_revision();
    let stale_prepared = eng
        .prepare(
            &request(1, rev1),
            c.live(),
            rev1,
            deps_default(c.live(), &media),
        )
        .unwrap();

    // Project moves before publish — the prepared plan is stale.
    apply(&mut c, ProjectOp::SetProjectName { name: "v2".into() });
    let rev2 = c.live_revision();
    assert_eq!(
        eng.publish(stale_prepared, c.live(), rev2).err().unwrap(),
        RoutingFault::StaleProjectSnapshot
    );

    // Retry: fresh prepare against the committed rev2 snapshot.
    let fresh = eng
        .prepare(
            &request(2, rev2),
            c.live(),
            rev2,
            deps_default(c.live(), &media),
        )
        .unwrap();
    let fresh_gen = fresh.generation;
    assert!(fresh_gen > 1);
    eng.publish(fresh, c.live(), rev2).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    let receipt = eng.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        synvelis_project::RoutingActivationState::Active { generation, .. } if generation == fresh_gen
    ));
    assert_eq!(receipt.last_good_generation, Some(fresh_gen));
    // frames_observed survives the pending slot being cleared.
    assert!(receipt.frames_observed > 0);
}

// ------------------------------------------------- RT counter faults

/// Two publishes before any `run` leave the first plan's install
/// commands queued; activation of the newer plan discards them as
/// stale — `reconcile` surfaces `EventStale`.
#[test]
fn sf03_l2_08_event_stale_via_reconcile() {
    let root = scratch("l208");
    let (c, media) = minimal_project(&root);
    let mut eng = new_engine();
    let rev = c.live_revision();

    for id in 1..=2_u64 {
        let prepared = eng
            .prepare(
                &request(id, rev),
                c.live(),
                rev,
                deps_default(c.live(), &media),
            )
            .unwrap();
        eng.publish(prepared, c.live(), rev).unwrap();
    }
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert!(eng.rt_status().stale_commands > 0 || eng.rt_status().stale_events > 0);

    let report = eng.reconcile(c.live(), rev, 1, 1);
    assert!(
        report
            .mismatches
            .contains(&RoutingReconcileMismatch::RuntimeCounters)
    );
    assert_eq!(report.fault, Some(RoutingFault::EventStale));
}

/// Flooding the command channel with unpublished deltas forces
/// `PublishQueueFull`; the receipt keeps last-good and the last
/// successfully published plan still activates on `run`.
#[test]
fn sf03_l2_09_publish_queue_full_preserves_last_good() {
    let root = scratch("l209");
    let (mut c, media) = minimal_project(&root);
    let mut eng = new_engine();
    let rev = c.live_revision();

    // Gen 1 active → last-good baseline.
    drive(&mut eng, &mut c, &media, 1, 1);
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));

    // Re-publish identical builds without running — each publish
    // re-installs every processor, filling the bounded command
    // channel until it rejects.
    let mut last_err = None;
    for id in 2..200_u64 {
        let prepared = eng
            .prepare(
                &request(id, rev),
                c.live(),
                rev,
                deps_default(c.live(), &media),
            )
            .unwrap();
        match eng.publish(prepared, c.live(), rev) {
            Ok(_) => {}
            Err(fault) => {
                last_err = Some(fault);
                break;
            }
        }
    }
    assert_eq!(last_err, Some(RoutingFault::PublishQueueFull));
    let receipt = eng.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        synvelis_project::RoutingActivationState::Failed {
            fault: RoutingFault::PublishQueueFull,
            last_good_generation: Some(1),
        }
    ));

    // The last successfully published plan still activates — the
    // failed publish displaced nothing.
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert!(eng.receipt().unwrap().last_good_generation.unwrap() > 1);
    assert_eq!(
        eng.receipt().unwrap().last_good_generation,
        Some(eng.rt_status().active_generation)
    );
}

// -------------------------------------------------------- offline gate

/// A chain processor instance that honestly reports verified offline
/// support.
struct VerifiedGain(f32);

impl GraphProcessor for VerifiedGain {
    fn process(&mut self, io: &mut NodeIo<'_>, ctx: &RtContext) {
        GainProcessor::new(self.0).process(io, ctx);
    }

    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }
}

/// `BuildMode::Offline` requires every lowered chain instance to
/// report `OfflineRenderSupport::Verified` — `Unknown` (the default)
/// fails `OfflineCapabilityMismatch`, `Verified` builds succeed.
#[test]
fn sf03_l2_10_offline_capability_gate() {
    let root = scratch("l210");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let strip = strip_of_track(c.live(), track);
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "gain".into(),
            binding: synvelis_project::ProcessorBinding::Unresolved {
                descriptor: "test.gain".into(),
            },
            sidechain_input: false,
        },
    );
    let processor_id = *c.live().routing.processors.keys().next().unwrap();
    let rev = c.live_revision();
    let offline_req = synvelis_project::RoutingCompileRequest {
        mode: BuildMode::Offline,
        ..request(1, rev)
    };

    // Unknown support (GainProcessor does not override the default)
    // → OfflineCapabilityMismatch.
    let mut eng = new_engine();
    let mut host = synvelis_player::StaticProcessorHost::new();
    host.insert(processor_id, Box::new(GainProcessor::new(0.5)));
    eng.set_processor_host(Box::new(host));
    let deps = deps_default(c.live(), &media);
    assert_eq!(
        eng.prepare(&offline_req, c.live(), rev, deps)
            .err()
            .unwrap(),
        RoutingFault::OfflineCapabilityMismatch
    );

    // Verified support → the offline build proceeds.
    let mut eng2 = new_engine();
    let mut host2 = synvelis_player::StaticProcessorHost::new();
    host2.insert(processor_id, Box::new(VerifiedGain(0.5)));
    eng2.set_processor_host(Box::new(host2));
    let deps2 = deps_default(c.live(), &media);
    let prepared = eng2.prepare(&offline_req, c.live(), rev, deps2).unwrap();
    eng2.publish(prepared, c.live(), rev).unwrap();
    eng2.run(1, PumpPolicy::Every(1)).unwrap();
    assert!(matches!(
        eng2.receipt().unwrap().state,
        synvelis_project::RoutingActivationState::Active { .. }
    ));
}

/// Unused processor instances do not gate the offline build — only
/// instances that actually enter the graph are checked.
#[test]
fn sf03_l2_11_offline_ignores_unused_instances() {
    let root = scratch("l211");
    let (c, media) = minimal_project(&root);
    let rev = c.live_revision();
    let offline_req = synvelis_project::RoutingCompileRequest {
        mode: BuildMode::Offline,
        ..request(1, rev)
    };
    let mut eng = new_engine();
    // No chain processors in the graph; an unverified instance for a
    // nonexistent processor ID is simply unused.
    let mut host = synvelis_player::StaticProcessorHost::new();
    host.insert(
        synvelis_project::ProcessorId::new(0xFFFF),
        Box::new(GainProcessor::new(1.0)),
    );
    eng.set_processor_host(Box::new(host));
    let deps = deps_default(c.live(), &media);
    eng.prepare(&offline_req, c.live(), rev, deps).unwrap();
}
