//! SF-03 slice 1 acceptance: semantic `RoutingModel` → Ruviform typed
//! graph lowering, generation-scoped `RoutingSourceMap`, and the
//! `Configured→Prepared→Published→Active` activation receipt with
//! last-good preservation (Issue #106, SF-ACT-01..04 系).
//!
//! Deterministic evidence: the golden test compares semantic-engine
//! PCM bit-for-bit against the legacy clip→master driver path; every
//! failure asserts the typed [`RoutingFault`] and the retained
//! last-good generation — never a silent success.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::{EngineConfig, lower};
use synvelis_player::resolver::prepare;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::routing_lower::{SemanticLowered, lower_semantic};
use synvelis_player::{HeadlessDriver, ImportOutcome};
use synvelis_project::{
    BuildBudget, BuildMode, BusKind, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    ControlOwner, CounterIdGenerator, MappingResolution, MediaDuration, MediaTimeValue,
    ParamMapping, PlaybackMapping, PortDirection, PortOwner, PortRole, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, RoutingActivationState, RoutingBuildKey,
    RoutingBuildRequestId, RoutingCompileRequest, RoutingFault, RuntimeClockEpochRef,
    RuntimeInstanceId, SemanticNodeRef, SemanticPortId, SendId, SendTap, SourceFrame, SourceId,
    SourceRange, StripId, TimeAnchor, ValueKind, port_capability_generation, routing_fingerprint,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03-{}-{}-{}",
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
        Project::new(ProjectId::new(0x106), "sf03"),
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

fn prefetch() -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: 2,
        sample_rate: 48_000,
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

/// Imports a stereo fixture and returns the commit outcome.
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

/// Adds a track, gives it a strip (auto-wired to master), adds a lane
/// and returns (track, lane).
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

/// Track + strip + lane + one full-window clip — the reference
/// project shape for equivalence assertions.
fn add_clip_project(c: &mut ProjectController, source: SourceId) {
    let (_, lane) = add_track_strip(c);
    add_clip(c, lane, source, 0, 480, 0);
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

/// Prepares one `PreparedClip` per project clip in `clips` map order
/// — the same order `plan_playbacks` emits (callers must ensure no
/// clip is muted/skipped so counts line up).
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
/// clips (no processor instances). The non-capturing closure
/// promotes to `'static`.
fn deps_for(project: &Project, media_root: &Path) -> RoutingDeps<'static> {
    RoutingDeps {
        canonical_of: &|_| Some(canonical_format()),
        clips: prepared_clips(project, media_root),
        processors: std::collections::BTreeMap::default(),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
}

/// Build key for the standard test engine/instance/epoch.
fn build_key(project: &Project, revision: ProjectRevision) -> RoutingBuildKey {
    RoutingBuildKey {
        project_id: project.project_id,
        project_revision: revision,
        routing_fingerprint: routing_fingerprint(project),
        capability_generation: port_capability_generation(project),
        engine_config_generation: 1,
        source_format_generation: 1,
        lowering_policy_version: synvelis_project::LOWERING_POLICY_VERSION,
        build_mode: BuildMode::Realtime,
        device_clock_epoch: RuntimeClockEpochRef(1),
        runtime_instance: RuntimeInstanceId(1),
    }
}

fn test_budget() -> BuildBudget {
    BuildBudget {
        max_nodes: 4_096,
        max_connections: 16_384,
        max_frames: 512,
    }
}

/// `lower_semantic` under the standard test key/budget — panics on
/// error (tests assert success paths here; failure paths go through
/// the engine).
fn lower_for_test(
    project: &Project,
    revision: ProjectRevision,
    generation: u64,
) -> SemanticLowered {
    lower_semantic(
        project,
        &|_| Some(canonical_format()),
        &engine(),
        &build_key(project, revision),
        generation,
        &test_budget(),
    )
    .unwrap()
}

/// Adds a group bus and a send from `track`'s strip main output to
/// the bus strip main input with the given tap, then binds a
/// `SendLevel` target at `base` (normalized) as the send's gain
/// target.
fn wire_group_send_tap(
    c: &mut ProjectController,
    track: synvelis_project::TrackId,
    tap: SendTap,
    base: f64,
) -> SendId {
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
            tap,
            gain_target: None,
        },
    );
    let send_id = *c.live().routing.sends.keys().next().unwrap();
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: "send".into(),
            owner: ControlOwner::Send(send_id),
            role: synvelis_project::ParameterRole::SendLevel,
            time_scope: synvelis_project::AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base,
        },
    );
    let gain_target = *c
        .live()
        .control
        .targets
        .keys()
        .find(|t| {
            let target = &c.live().control.targets[*t];
            target.owner == ControlOwner::Send(send_id)
                && target.role == synvelis_project::ParameterRole::SendLevel
        })
        .unwrap();
    apply(
        c,
        ProjectOp::SetSendGainTarget {
            send_id,
            gain_target: Some(gain_target),
        },
    );
    send_id
}

/// Post-fader convenience wrapper around [`wire_group_send_tap`].
fn wire_group_send(
    c: &mut ProjectController,
    track: synvelis_project::TrackId,
    base: f64,
) -> SendId {
    wire_group_send_tap(c, track, SendTap::PostFader, base)
}

/// Full prepare→publish→`run` drive for `blocks`.
fn drive(
    engine: &mut RoutingEngine,
    c: &mut ProjectController,
    media: &Path,
    request_id: u64,
    blocks: usize,
) -> synvelis_player::PlayReport {
    let revision = c.live_revision();
    let canonical = |_: SourceId| Some(canonical_format());
    let deps = RoutingDeps {
        canonical_of: &canonical,
        clips: c
            .live()
            .clips
            .values()
            .map(|clip| prepare(c.live(), clip.source_id, media, &prefetch()).unwrap())
            .collect(),
        processors: std::collections::BTreeMap::default(),
        engine_config_generation: 1,
        source_format_generation: 1,
    };
    let prepared = engine
        .prepare(&request(request_id, revision), c.live(), revision, deps)
        .unwrap();
    engine.publish(prepared, c.live(), revision).unwrap();
    engine.run(blocks, PumpPolicy::Every(1)).unwrap()
}

// ---------------------------------------------------------------- SF-ACT-01

/// SF-ACT-01: semantic lowering emits a deterministic typed graph and
/// a generation-scoped `RoutingSourceMap` — identical inputs produce
/// identical specs and maps; every semantic entity resolves
/// explicitly (never silently).
#[test]
fn sf03_01_lower_semantic_topology_deterministic() {
    let root = scratch("sf0301");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let source = out.commit.source_id;
    let (track, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, source, 0, 480, 0);

    let project = c.live();
    let revision = c.live_revision();
    let a = lower_for_test(project, revision, 7);
    let b = lower_for_test(project, revision, 7);

    // Identical inputs → identical spec and map.
    assert_eq!(format!("{:?}", a.spec), format!("{:?}", b.spec));
    assert_eq!(a.source_map, b.source_map);

    // Topology: device sink + master fader + track fader + playback = 4 nodes.
    assert_eq!(a.spec.nodes().len(), 4);
    // Edges: clip→track-in, track-out→master-in, master-out→sink.
    assert_eq!(a.spec.connections().len(), 3);

    // SourceMap resolutions.
    let track_strip = strip_of_track(project, track);
    let master = master_strip(project);
    assert!(matches!(
        a.source_map.semantic_nodes[&SemanticNodeRef::Strip(track_strip)],
        MappingResolution::Resolved(_)
    ));
    assert!(matches!(
        a.source_map.semantic_nodes[&SemanticNodeRef::Bus(project.routing.master)],
        MappingResolution::Resolved(_)
    ));
    assert!(matches!(
        a.source_map.semantic_nodes[&SemanticNodeRef::Strip(master)],
        MappingResolution::Resolved(_)
    ));

    // The auto-wired track→master Main connection resolves to a live edge.
    let conn = project
        .routing
        .connections
        .values()
        .find(|c| c.enabled)
        .unwrap();
    assert!(matches!(
        a.source_map.connections[&conn.id],
        MappingResolution::Resolved(_)
    ));

    // Strip ports resolve; the playback binds to the track strip's
    // main input.
    let track_in = strip_port(project, track_strip, PortDirection::Input, PortRole::Main);
    let track_out = strip_port(project, track_strip, PortDirection::Output, PortRole::Main);
    assert!(matches!(
        a.source_map.ports[&track_in],
        MappingResolution::Resolved(_)
    ));
    assert!(matches!(
        a.source_map.ports[&track_out],
        MappingResolution::Resolved(_)
    ));
    assert_eq!(a.source_map.playback_bindings.len(), 1);
    assert_eq!(a.source_map.playback_bindings[0].destination, track_in);
    assert_eq!(a.playbacks.len(), 1);
    assert_eq!(a.skipped.len(), 0);
    assert_eq!(a.source_map.generation, 7);
    assert_eq!(a.source_map.project_revision, revision);
}

/// SF-ACT-01: `routing_fingerprint` is a change detector — a
/// view-only/name change keeps it stable while a semantic change
/// (mute target) moves it; `semantic_eq` ignores only the revision.
#[test]
fn sf03_01b_fingerprint_and_semantic_eq() {
    let root = scratch("sf0301b");
    let mut c = controller();
    import_stereo(&mut c, &root, "a.wav", 960);
    let (track, lane) = add_track_strip(&mut c);
    let source = *c.live().sources.keys().next().unwrap();
    add_clip(&mut c, lane, source, 0, 480, 0);

    let p1 = c.live();
    let fp1 = routing_fingerprint(p1);
    let rev1 = c.live_revision();
    let key1 = build_key(p1, rev1);

    // Rename: revision moves, fingerprint and semantic_eq hold.
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let p2 = c.live();
    assert_eq!(routing_fingerprint(p2), fp1);
    let key2 = build_key(p2, c.live_revision());
    assert_ne!(key1.project_revision, key2.project_revision);
    assert!(key1.semantic_eq(&key2));
    assert_ne!(key1, key2);

    // A semantic change (mute the strip) moves the fingerprint.
    let mute_strip = strip_of_track(c.live(), track);
    apply(
        &mut c,
        ProjectOp::SetStripMute {
            strip_id: mute_strip,
            muted: true,
        },
    );
    assert_ne!(routing_fingerprint(c.live()), fp1);
}

// ---------------------------------------------------------------- SF-ACT-02

/// SF-ACT-02 (golden): a single track+clip lowered through the
/// semantic path produces **bit-identical** master PCM to the legacy
/// clip→master driver path — the semantic graph is a faithful
/// projection, not an approximation.
#[test]
fn sf03_02_semantic_audio_matches_legacy_bitexact() {
    let root = scratch("sf0302");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let source = out.commit.source_id;
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, source, 64, 764, 13);
    let media = root.join("media");

    // Legacy path.
    let eng = engine();
    let lowered = lower(c.live(), &|_| Some(canonical_format()), &eng);
    assert_eq!(lowered.playbacks.len(), 1);
    let legacy_prepared = vec![prepare(c.live(), source, &media, &prefetch()).unwrap()];
    let mut driver = HeadlessDriver::new(eng);
    driver.activate(&lowered, legacy_prepared).unwrap();
    let legacy = driver.run(8, PumpPolicy::Every(1)).unwrap();

    // Semantic path.
    let mut routing_engine = new_engine();
    let revision = c.live_revision();
    let prepared = routing_engine
        .prepare(
            &request(1, revision),
            c.live(),
            revision,
            deps_for(c.live(), &media),
        )
        .unwrap();
    let published = routing_engine
        .publish(prepared, c.live(), revision)
        .unwrap();
    assert!(matches!(
        published.state,
        RoutingActivationState::Published { .. }
    ));
    let semantic = routing_engine.run(8, PumpPolicy::Every(1)).unwrap();

    assert_eq!(semantic.pcm, legacy.pcm);
    assert_eq!(semantic.crc, legacy.crc);
}

/// SF-ACT-02: two tracks mix through their own strips — the semantic
/// path sums per-strip then routes to master, and stays bit-exact
/// against an equivalent single-clip-per-track reference computed
/// by the legacy driver on a manually summed expectation is *not*
/// available, so instead assert the mix equals the sum of two
/// independent single-track runs (linearity of the unity-gain path).
#[test]
fn sf03_02b_two_track_mix_is_additive() {
    let root = scratch("sf0302b");
    let mut c_mix = controller();
    let out_a = import_stereo(&mut c_mix, &root, "a.wav", 4_800);
    let out_b = import_stereo(&mut c_mix, &root, "b.wav", 4_800);
    let (ta, la) = add_track_strip(&mut c_mix);
    let (tb, lb) = add_track_strip(&mut c_mix);
    add_clip(&mut c_mix, la, out_a.commit.source_id, 0, 480, 0);
    add_clip(&mut c_mix, lb, out_b.commit.source_id, 0, 480, 0);
    let _ = (ta, tb);
    let media = root.join("media");

    // Reference: each source alone through the semantic engine.
    let mut ref_a = controller();
    let imp_a = import_stereo(&mut ref_a, &root, "a.wav", 4_800);
    add_clip_project(&mut ref_a, imp_a.commit.source_id);
    let mut ref_b = controller();
    let imp_b = import_stereo(&mut ref_b, &root, "b.wav", 4_800);
    add_clip_project(&mut ref_b, imp_b.commit.source_id);

    let mut engine = new_engine();
    let mix = drive(&mut engine, &mut c_mix, &media, 1, 4);
    let mut engine_a = new_engine();
    let ra = drive(&mut engine_a, &mut ref_a, &media, 1, 4);
    let mut engine_b = new_engine();
    let rb = drive(&mut engine_b, &mut ref_b, &media, 1, 4);

    // Unity gain everywhere → the mix is the per-sample sum of the
    // two single-track references (each strip passes its clip at
    // unity, master sums the two strip outputs).
    for (cm, (ca, cb)) in mix.pcm.iter().zip(ra.pcm.iter().zip(rb.pcm.iter())) {
        for (m, (a, b)) in cm.iter().zip(ca.iter().zip(cb.iter())) {
            assert_eq!(*m, a + b, "mix != a + b");
        }
    }
}

// ---------------------------------------------------------------- SF-ACT-03

/// SF-ACT-03: muting a strip is baked at lowering via
/// `audible_strips` — the muted strip's outgoing edge is recorded
/// `NotLowered(MutedStrip)` and contributes silence, while the
/// unmuted strip still sounds.
#[test]
fn sf03_03_strip_mute_prunes_edge() {
    let root = scratch("sf0303");
    let mut c = controller();
    let out_a = import_stereo(&mut c, &root, "a.wav", 4_800);
    let out_b = import_stereo(&mut c, &root, "b.wav", 4_800);
    let (ta, la) = add_track_strip(&mut c);
    let (tb, lb) = add_track_strip(&mut c);
    add_clip(&mut c, la, out_a.commit.source_id, 0, 480, 0);
    add_clip(&mut c, lb, out_b.commit.source_id, 0, 480, 0);
    let mute_a = strip_of_track(c.live(), ta);
    apply(
        &mut c,
        ProjectOp::SetStripMute {
            strip_id: mute_a,
            muted: true,
        },
    );
    let _ = tb;

    // Lower and inspect the map.
    let project = c.live();
    let revision = c.live_revision();
    let lowered = lower_for_test(project, revision, 1);

    // The muted track's Main connection is explicitly not lowered.
    let muted_conn = project
        .routing
        .connections
        .values()
        .find(|conn| {
            let src = project.routing.ports[&conn.source].owner;
            let PortOwner::Strip(s) = src else {
                return false;
            };
            s == strip_of_track(project, ta)
        })
        .unwrap();
    assert!(matches!(
        lowered.source_map.connections[&muted_conn.id],
        MappingResolution::NotLowered(synvelis_project::NotLoweredReason::MutedStrip)
    ));
    // Its playback node still exists (clip plays into a silent strip).
    assert_eq!(lowered.playbacks.len(), 2);

    // Audio: output equals the unmuted track alone.
    let media = root.join("media");
    let mut eng = new_engine();
    let muted_report = drive(&mut eng, &mut c, &media, 2, 4);

    let mut c_ref = controller();
    let imp_b = import_stereo(&mut c_ref, &root, "b.wav", 4_800);
    add_clip_project(&mut c_ref, imp_b.commit.source_id);
    let mut eng_ref = new_engine();
    let ref_report = drive(&mut eng_ref, &mut c_ref, &media, 1, 4);

    assert_eq!(muted_report.pcm, ref_report.pcm);
    assert_eq!(muted_report.crc, ref_report.crc);
}

/// SF-ACT-03: a disabled connection produces no edge and is recorded
/// `NotLowered(DisabledConnection)` — explicit, never silent.
#[test]
fn sf03_03b_disabled_connection_not_lowered() {
    let root = scratch("sf0303b");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);

    let conn_id = *c.live().routing.connections.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::SetConnectionEnabled {
            connection_id: conn_id,
            enabled: false,
        },
    );

    let project = c.live();
    let revision = c.live_revision();
    let lowered = lower_for_test(project, revision, 1);
    assert!(matches!(
        lowered.source_map.connections[&conn_id],
        MappingResolution::NotLowered(synvelis_project::NotLoweredReason::DisabledConnection)
    ));
    // Only master→sink edge remains.
    assert_eq!(lowered.spec.connections().len(), 2);
}

// ---------------------------------------------------------------- SF-ACT-04

/// SF-ACT-04: `Published` is non-RT evidence — the receipt stays
/// `Published` until `run` observes the generation on the RT side,
/// then transitions to `Active` with last-good recorded.
#[test]
fn sf03_04_publish_then_active_receipt() {
    let root = scratch("sf0304");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let media = root.join("media");

    let mut eng = new_engine();
    let revision = c.live_revision();
    let prepared = eng
        .prepare(
            &request(1, revision),
            c.live(),
            revision,
            deps_for(c.live(), &media),
        )
        .unwrap();
    assert_eq!(prepared.generation, 1);
    assert_eq!(prepared.source_map.generation, 1);

    let receipt = eng.publish(prepared, c.live(), revision).unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Published { generation: 1, .. }
    ));
    assert_eq!(receipt.published_generation, Some(1));
    // Not yet active — RT has not processed a frame.
    assert_ne!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active {
            project_revision: revision,
            generation: 1,
            epoch: RuntimeClockEpochRef(1),
        }
    );
    assert_eq!(eng.rt_status().active_generation, 0);

    eng.run(2, PumpPolicy::Every(1)).unwrap();
    let receipt = eng.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active { generation: 1, .. }
    ));
    assert_eq!(receipt.observed_active_generation, Some(1));
    assert_eq!(receipt.last_good_generation, Some(1));
    assert_eq!(eng.rt_status().active_generation, 1);
    assert!(eng.source_map().is_some());
    assert_eq!(eng.source_map().unwrap().generation, 1);
}

/// SF-ACT-04: prepare/compile/publish failures preserve last-good —
/// the previous active plan keeps running and the receipt records
/// the fault with `last_good_generation`.
#[test]
fn sf03_04b_failure_preserves_last_good() {
    let root = scratch("sf0304b");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let media = root.join("media");

    // Gen 1: good build, published and activated.
    let mut eng = new_engine();
    let rev1 = c.live_revision();
    let good = eng
        .prepare(
            &request(1, rev1),
            c.live(),
            rev1,
            deps_for(c.live(), &media),
        )
        .unwrap();
    eng.publish(good, c.live(), rev1).unwrap();
    let before = eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { generation: 1, .. }
    ));

    // (a) Stale expected revision → prepare rejects before graph work.
    let stale_request = RoutingCompileRequest {
        expected_project_revision: ProjectRevision(rev1.0 + 1),
        ..request(2, rev1)
    };
    let err = eng
        .prepare(&stale_request, c.live(), rev1, deps_for(c.live(), &media))
        .err()
        .unwrap();
    assert_eq!(err, RoutingFault::StaleProjectRevision);
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));

    // (b) A chain processor with no supplied instance → compile fails
    // closed; last-good gen 1 keeps running.
    let strip = {
        let track = *c.live().tracks.keys().next_back().unwrap();
        strip_of_track(c.live(), track)
    };
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
    let err = eng
        .prepare(
            &request(3, rev2),
            c.live(),
            rev2,
            deps_for(c.live(), &media),
        )
        .err()
        .unwrap();
    assert_eq!(err, RoutingFault::PluginUnavailable);
    let receipt = eng.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Failed {
            fault: RoutingFault::PluginUnavailable,
            last_good_generation: Some(1),
        }
    ));

    // Last-good still produces identical audio: rewind to the same
    // engine window and replay — `run` only reports the frames it
    // processed, so a bare second `run` would cover later (silent)
    // frames and prove nothing.
    eng.seek(0);
    let after = eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(after.pcm, before.pcm);
    assert_eq!(after.crc, before.crc);
    assert_eq!(eng.rt_status().active_generation, 1);
}

/// SF-ACT-04: `publish` re-validates against the live snapshot — a
/// prepared plan published after the project moved is rejected
/// `StaleProjectRevision` (Blocked), never displacing last-good.
#[test]
fn sf03_04c_publish_rejects_stale_revision() {
    let root = scratch("sf0304c");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let media = root.join("media");

    let mut eng = new_engine();
    let rev1 = c.live_revision();
    let prepared = eng
        .prepare(
            &request(1, rev1),
            c.live(),
            rev1,
            deps_for(c.live(), &media),
        )
        .unwrap();

    // Project moves before publish.
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "moved".into(),
        },
    );
    let rev2 = c.live_revision();
    assert_ne!(rev1, rev2);

    let err = eng.publish(prepared, c.live(), rev2).unwrap_err();
    assert_eq!(err, RoutingFault::StaleProjectRevision);
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Blocked {
            fault: RoutingFault::StaleProjectRevision,
            ..
        }
    ));
    // Nothing was published — no active generation observed.
    assert_eq!(eng.rt_status().active_generation, 0);
}

/// SF-ACT-04: a second successful build gets the next generation and
/// replaces the source map wholesale — the old map's runtime IDs are
/// never reused across generations.
#[test]
fn sf03_04d_generation_scoped_source_map() {
    let root = scratch("sf0304d");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let media = root.join("media");

    let mut eng = new_engine();
    let rev1 = c.live_revision();
    let p1 = eng
        .prepare(
            &request(1, rev1),
            c.live(),
            rev1,
            deps_for(c.live(), &media),
        )
        .unwrap();
    eng.publish(p1, c.live(), rev1).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    let map1 = eng.source_map().unwrap().clone();
    assert_eq!(map1.generation, 1);

    // Semantic change → new revision → new build.
    let track = *c.live().tracks.keys().next_back().unwrap();
    let mute_strip = strip_of_track(c.live(), track);
    apply(
        &mut c,
        ProjectOp::SetStripMute {
            strip_id: mute_strip,
            muted: true,
        },
    );
    let rev2 = c.live_revision();
    let p2 = eng
        .prepare(
            &request(2, rev2),
            c.live(),
            rev2,
            deps_for(c.live(), &media),
        )
        .unwrap();
    assert_eq!(p2.generation, 2);
    eng.publish(p2, c.live(), rev2).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let receipt = eng.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active { generation: 2, .. }
    ));
    assert_eq!(receipt.last_good_generation, Some(2));
    let map2 = eng.source_map().unwrap();
    assert_eq!(map2.generation, 2);
    assert_eq!(map2.project_revision, rev2);
    assert_ne!(map1.generation, map2.generation);
}

// ---------------------------------------------------------------- sends

/// SF-ACT-02/03: a send with a bound gain target inserts a gain stage
/// — the tap is recorded in `send_taps`, the semantic connection
/// resolves through the gain node, and the level is the control
/// model's plain value (0.5 → half contribution).
#[test]
fn sf03_05_send_gain_stage() {
    let root = scratch("sf0305");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (track, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let send_id = wire_group_send(&mut c, track, 0.5);

    let project = c.live();
    let revision = c.live_revision();
    let lowered = lower_for_test(project, revision, 3);

    // send_taps records the post-fader tap; a SendGain node role exists.
    assert!(lowered.source_map.send_taps.contains_key(&send_id));
    assert!(lowered
        .node_roles
        .values()
        .any(|r| matches!(r, synvelis_player::routing_lower::SemanticNodeRole::SendGain { send, gain } if *send == send_id && (*gain - 0.5).abs() < 1e-6)));
    // The send connection resolves to the gain→bus edge.
    let send_conn = project
        .routing
        .connections
        .values()
        .find(|conn| conn.kind == synvelis_project::ConnectionKind::Send(send_id))
        .unwrap();
    assert!(matches!(
        lowered.source_map.connections[&send_conn.id],
        MappingResolution::Resolved(_)
    ));
}

/// SF-ACT-02: `SendTap::PreFader` taps the strip's dry pre-fader
/// output — a different runtime port than the post-fader main out —
/// while `PostFader` taps the fader output itself.
#[test]
fn sf03_05b_send_tap_pre_vs_post() {
    let root = scratch("sf0305b");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (track, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let pre_send = wire_group_send_tap(&mut c, track, SendTap::PreFader, 1.0);

    let project = c.live();
    let revision = c.live_revision();
    let lowered = lower_for_test(project, revision, 3);

    let track_strip = strip_of_track(project, track);
    let track_out = strip_port(project, track_strip, PortDirection::Output, PortRole::Main);
    let MappingResolution::Resolved(post_ref) = lowered.source_map.ports[&track_out] else {
        panic!("main out must resolve");
    };
    let pre_tap = lowered.source_map.send_taps[&pre_send];
    // Same fader node, different port: pre-fader dry out ≠ post out.
    assert_eq!(pre_tap.node, post_ref.node);
    assert_ne!(pre_tap.port, post_ref.port);
}

// ---------------------------------------------------------------- chain

/// SF-ACT-02: a chain processor with a supplied instance runs inline —
/// clip → processor(0.5) → fader → master halves the output vs the
/// same project without the processor.
#[test]
fn sf03_06_chain_processor_instance() {
    let root = scratch("sf0306");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (track, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
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
    let media = root.join("media");

    // With a 0.5 gain instance.
    let mut eng = new_engine();
    let rev = c.live_revision();
    let mut deps = deps_for(c.live(), &media);
    deps.processors.insert(
        processor_id,
        Box::new(synvelis_player::routing_lower::GainProcessor::new(0.5)),
    );
    let prepared = eng.prepare(&request(1, rev), c.live(), rev, deps).unwrap();
    eng.publish(prepared, c.live(), rev).unwrap();
    let halved = eng.run(4, PumpPolicy::Every(1)).unwrap();

    // Reference: same project minus the processor.
    let mut c_ref = controller();
    let out_r = import_stereo(&mut c_ref, &root, "a.wav", 4_800);
    let (_, lane_r) = add_track_strip(&mut c_ref);
    add_clip(&mut c_ref, lane_r, out_r.commit.source_id, 0, 480, 0);
    let mut eng_ref = new_engine();
    let rev_r = c_ref.live_revision();
    let p_ref = eng_ref
        .prepare(
            &request(1, rev_r),
            c_ref.live(),
            rev_r,
            deps_for(c_ref.live(), &media),
        )
        .unwrap();
    eng_ref.publish(p_ref, c_ref.live(), rev_r).unwrap();
    let full = eng_ref.run(4, PumpPolicy::Every(1)).unwrap();

    for (h, f) in halved.pcm.iter().zip(full.pcm.iter()) {
        for (hs, fs) in h.iter().zip(f.iter()) {
            assert!((*hs - fs * 0.5).abs() < 1e-6, "expected half gain");
        }
    }
}
