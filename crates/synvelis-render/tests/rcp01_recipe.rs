//! RCP-01 headless fixtures (synvelis#290): canonical Render Recipe,
//! typed dependency closure, temporal invalidation, branch reuse
//! identity and fail-closed resolution.
//!
//! Every case maps to an acceptance id (RCP-REC-01..06). Real plugin
//! hosts, OS gates and render execution are `NotRun` — they belong to
//! later slices; headless PASS here is never promoted to them.

use std::collections::BTreeMap;

use synvelis_project::StateCaptureQuality;
use synvelis_project::control::{BeatPoint, BeatRange};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{ObjectKind, SemanticObjectRef};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor,
};
use synvelis_project::project::Project;
use synvelis_project::range::SourceRange;
use synvelis_project::routing::{
    BusKind, ConnectionKind, PluginRef, ProcessorBinding, SendTap, StripOwner,
};
use synvelis_project::snapshot::{ContentDigest, SnapshotCancel, SnapshotResolver};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::time::{MediaDuration, MediaTimeValue, SourceFrame};
use synvelis_project::variant::{
    FakeDomainAdapter, InMemoryBranchContentStore, VariantScope, edit_branch, fork_variant_set,
};
use synvelis_project::{
    AssetId, AutomationPoint, AutomationRegion, AutomationTimeScope, BranchId, ClipId,
    ConflictPolicy, ControlOwner, Interpolation, LaneId, ParamMapping, ParameterRole,
    ParameterTargetId, PortDirection, PortOwner, PortRole, ProcessorId,
    RENDER_INPUT_SCHEMA_VERSION, SemanticPortId, SourceId, SourceState, StripId, TrackId,
    ValueKind, VariantSetId,
    render_input::{RenderBranchTarget, resolve_render_project},
};
use synvelis_render::dependency::{ExternalAssetProbe, PluginStateProbe};
use synvelis_render::invalidation::{ChangedObject, InvalidationReason, classify_invalidation};
use synvelis_render::recipe::{
    ClockDependency, DenormalMode, FrameInterval, OutputContract, OutputPortContract, PcmFormat,
    RecipeKey, RecipeRecord, RecipeResolution, RenderBoundary, RenderPrecision, RendererProfile,
    ResolutionProvenance, TemporalDomain, UnknownCause, UnsupportedReason, Validity,
    evaluate_validity, recipe_record,
};
use synvelis_render::{
    ExternalPin, NoExternalAssets, NoPluginStates, RecipeBranchPin, RecipeEnvironment,
    RecipeRequest, StaticExternalAssets, StaticPluginStates, resolve_recipe,
};

// ---------------------------------------------------------------------------
// fixture helpers
// ---------------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0x290), "rcp01"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    let rev = c.live_revision();
    let op_dbg = format!("{op:?}");
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap_or_else(|e| panic!("op {op_dbg} failed: {e}"))
}

const RATE: u32 = 48_000;

fn media_clip_placement(start_frame: u64, end_frame: u64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(
            MediaTimeValue::new(i64::try_from(start_frame).unwrap(), 1, RATE).unwrap(),
        ),
        extent: ClipExtent::Absolute(
            MediaDuration::new(i64::try_from(end_frame - start_frame).unwrap(), 1, RATE).unwrap(),
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

/// Deterministic `sha256:`-shaped digest string.
fn sha(byte: u8) -> String {
    format!("sha256:{}", format!("{byte:02x}").repeat(32))
}

#[derive(Clone, Copy)]
struct TrackFixture {
    track: TrackId,
    lane: LaneId,
    strip: StripId,
    asset: AssetId,
    source: SourceId,
    clip: ClipId,
}

/// Track + strip (auto-wired to master) + lane + verified media source
/// + one clip.
fn track_with_clip(c: &mut ProjectController) -> TrackFixture {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let strip = c
        .live()
        .routing
        .strips
        .values()
        .find(|s| s.owner == StripOwner::Track(track))
        .unwrap()
        .id;
    apply(c, ProjectOp::RegisterAsset { digest: sha(0xA1) });
    let asset = *c.live().assets.keys().next_back().unwrap();
    apply(
        c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: sha(0xA1),
        },
    );
    let source = *c.live().sources.keys().next_back().unwrap();
    apply(
        c,
        ProjectOp::SetSourceState {
            source_id: source,
            state: SourceState::Verified,
        },
    );
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_clip_placement(0, 48_000),
            playback: natural_playback(0, 48_000),
        },
    );
    let clip = *c.live().clips.keys().next_back().unwrap();
    TrackFixture {
        track,
        lane,
        strip,
        asset,
        source,
        clip,
    }
}

/// `base` plus one `Resolved`-binding processor on the track strip.
fn base_fx() -> (ProjectController, TrackFixture, ProcessorId) {
    let mut c = controller();
    let fx = track_with_clip(&mut c);
    let processor = insert_plugin(&mut c, fx.strip, false);
    (c, fx, processor)
}

fn base() -> (ProjectController, TrackFixture) {
    let mut c = controller();
    let fx = track_with_clip(&mut c);
    (c, fx)
}

fn strip_port(
    project: &Project,
    strip: StripId,
    direction: PortDirection,
    role: PortRole,
) -> SemanticPortId {
    project
        .routing
        .ports
        .values()
        .find(|p| p.owner == PortOwner::Strip(strip) && p.direction == direction && p.role == role)
        .unwrap()
        .id
}

fn temporal() -> TemporalDomain {
    TemporalDomain {
        sample_rate_hz: RATE,
        channels: 2,
        format: PcmFormat::F32,
        interval: FrameInterval::new(0, 48_000).unwrap(),
        preroll_frames: 0,
        warmup_frames: 0,
        tail_frames: 480,
        lookahead_frames: 64,
        pdc_frames: 32,
        clock: ClockDependency::None,
        loop_region: None,
        seek_boundaries: Vec::new(),
    }
}

fn renderer() -> RendererProfile {
    RendererProfile {
        renderer_id: "test-render".into(),
        renderer_version: "0".into(),
        compatibility_profile: ContentDigest::sha256(b"compat-v1"),
        precision: RenderPrecision::F32,
        backend: "offline".into(),
        seed: 0,
        denormal_mode: DenormalMode::Preserve,
    }
}

/// Full resolution pipeline against the live project with the given
/// evidence ports — the shared entry every test uses.
fn resolve_with(
    project: &Project,
    plugins: &dyn PluginStateProbe,
    assets: &dyn ExternalAssetProbe,
    temporal: TemporalDomain,
    renderer: RendererProfile,
) -> RecipeResolution {
    let resolved = resolve_render_project(project, None).unwrap();
    let boundary = RenderBoundary::master(&resolved.project).expect("master boundary");
    let env = RecipeEnvironment {
        plugin_states: plugins,
        external_assets: assets,
    };
    let output = OutputContract {
        ports: boundary
            .ports
            .iter()
            .map(|p| OutputPortContract {
                port: *p,
                channels: 2,
                format: PcmFormat::F32,
            })
            .collect(),
    };
    resolve_recipe(&RecipeRequest {
        project: &resolved.project,
        boundary,
        temporal,
        renderer,
        output,
        environment: &env,
        branch: None,
        provenance: ResolutionProvenance::default(),
        project_schema_version: SCHEMA_VERSION,
        render_input_schema_version: RENDER_INPUT_SCHEMA_VERSION,
    })
}

fn resolve_live(project: &Project) -> RecipeResolution {
    resolve_with(
        project,
        &NoPluginStates,
        &NoExternalAssets,
        temporal(),
        renderer(),
    )
}

fn key_with(
    project: &Project,
    plugins: &dyn PluginStateProbe,
    assets: &dyn ExternalAssetProbe,
) -> RecipeKey {
    match resolve_with(project, plugins, assets, temporal(), renderer()) {
        RecipeResolution::Resolved { key, .. } => key,
        other => panic!("expected Resolved, got {other:?}"),
    }
}

fn plugin_ref() -> ProcessorBinding {
    ProcessorBinding::Resolved(PluginRef {
        descriptor: "test-plugin".into(),
        version: "1.0".into(),
        capability_revision: 1,
    })
}

fn insert_plugin(c: &mut ProjectController, strip: StripId, sidechain: bool) -> ProcessorId {
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding: plugin_ref(),
            sidechain_input: sidechain,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn plugin_param_target(c: &mut ProjectController, processor: ProcessorId) -> ParameterTargetId {
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    c.live()
        .control
        .targets
        .values()
        .filter(|t| t.owner == ControlOwner::Processor(processor))
        .map(|t| t.id)
        .next_back()
        .unwrap()
}

fn object_ref(project: &Project, kind: ObjectKind, tagged_id: String) -> SemanticObjectRef {
    SemanticObjectRef {
        project_id: project.project_id,
        kind,
        stable_id: tagged_id,
        lineage: None,
    }
}

fn quiescent_states(processor: ProcessorId) -> StaticPluginStates {
    let mut states = BTreeMap::new();
    states.insert(
        processor,
        synvelis_render::PluginStateEvidence {
            digest: ContentDigest::sha256(b"state"),
            quality: StateCaptureQuality::Quiescent,
        },
    );
    StaticPluginStates(states)
}

// ---------------------------------------------------------------------------
// RCP-REC-01 — canonical equality + key sensitivity
// ---------------------------------------------------------------------------

#[test]
fn rec01_display_rename_and_insertion_order_do_not_move_key() {
    let (c, fx, processor) = base_fx();
    let p1 = c.live().clone();
    let states = quiescent_states(processor);
    let assets = StaticExternalAssets::default();
    let k1 = key_with(&p1, &states, &assets);

    // UI rename: project/track/processor/bus display names are all
    // outside the canonical entity projection.
    let mut p2 = p1.clone();
    p2.name = "renamed".into();
    p2.tracks.get_mut(&fx.track).unwrap().name = "renamed-track".into();
    for proc_ in p2.routing.processors.values_mut() {
        proc_.name = "renamed-fx".into();
    }
    for bus in p2.routing.buses.values_mut() {
        bus.name = "renamed-bus".into();
    }
    assert_eq!(
        k1,
        key_with(&p2, &states, &assets),
        "display rename moved the key"
    );

    // Map/set insertion order cannot move the canonical fingerprint:
    // the object set is order-canonical by construction.
    let objects = synvelis_project::all_semantic_objects(&p1);
    let forward: std::collections::BTreeSet<_> = objects.iter().cloned().collect();
    let reverse: std::collections::BTreeSet<_> = objects.iter().rev().cloned().collect();
    assert_eq!(forward, reverse);
    assert_eq!(
        synvelis_project::canonical_fingerprint(&p1, &forward).unwrap(),
        synvelis_project::canonical_fingerprint(&p1, &reverse).unwrap(),
    );
}

fn baseline_key() -> RecipeKey {
    // Baseline: one track, one verified clip, one Resolved plugin —
    // plugin state pinned Quiescent so every variant resolves.
    let (c, _, processor) = base_fx();
    let states = quiescent_states(processor);
    let assets = StaticExternalAssets::default();
    key_with(c.live(), &states, &assets)
}

/// Quiescent evidence for *every* resolved processor in the project —
/// needed once a fixture holds more than one plugin.
fn all_quiescent_states(c: &ProjectController) -> StaticPluginStates {
    StaticPluginStates(
        c.live()
            .routing
            .processors
            .keys()
            .map(|id| {
                (
                    *id,
                    synvelis_render::PluginStateEvidence {
                        digest: ContentDigest::sha256(b"state"),
                        quality: StateCaptureQuality::Quiescent,
                    },
                )
            })
            .collect(),
    )
}

fn key_plugin_param_change() -> RecipeKey {
    let (mut c, _, processor) = base_fx();
    let target = plugin_param_target(&mut c, processor);
    apply(&mut c, ProjectOp::SetParameterBase { target, value: 0.9 });
    let states = quiescent_states(processor);
    let assets = StaticExternalAssets::default();
    key_with(c.live(), &states, &assets)
}

fn key_fx_reorder() -> RecipeKey {
    let (mut c, fx, _) = base_fx();
    insert_plugin(&mut c, fx.strip, false);
    let chain = c.live().routing.strips[&fx.strip].chain;
    let first = c.live().routing.chains[&chain].processors[0];
    apply(
        &mut c,
        ProjectOp::ReorderProcessor {
            chain_id: chain,
            processor_id: first,
            new_index: 1,
        },
    );
    // Both processors are Resolved bindings — pin both.
    let states = all_quiescent_states(&c);
    let assets = StaticExternalAssets::default();
    key_with(c.live(), &states, &assets)
}

fn key_sidechain_feed() -> RecipeKey {
    // Sidechain connection into a sidechain-capable processor —
    // sourced from a second strip (a strip may not sidechain itself).
    let (mut c, fx, _) = base_fx();
    let proc_sc = insert_plugin(&mut c, fx.strip, true);
    let sc_port = *c.live().routing.processors[&proc_sc]
        .port_ids
        .iter()
        .find(|p| c.live().routing.ports[p].role == PortRole::Sidechain)
        .unwrap();
    apply(&mut c, ProjectOp::AddTrack { name: "sc".into() });
    let track_sc = *c.live().tracks.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddTrackStrip { track_id: track_sc });
    let strip_sc = c
        .live()
        .routing
        .strips
        .values()
        .find(|s| s.owner == StripOwner::Track(track_sc))
        .unwrap()
        .id;
    let feed_port = strip_port(c.live(), strip_sc, PortDirection::Output, PortRole::Main);
    apply(
        &mut c,
        ProjectOp::AddConnection {
            source: feed_port,
            destination: sc_port,
            kind: ConnectionKind::Sidechain,
            channel_map: None,
        },
    );
    let states = all_quiescent_states(&c);
    let assets = StaticExternalAssets::default();
    key_with(c.live(), &states, &assets)
}

/// Send + return bus path; returns the key before and after a
/// send-tap change (the tap rides in the edge detail and must move
/// the key on its own).
fn key_send_return_path() -> (RecipeKey, RecipeKey) {
    let (mut c, fx, _) = base_fx();
    apply(
        &mut c,
        ProjectOp::AddBus {
            kind: BusKind::Return,
            name: "ret".into(),
        },
    );
    // Well-known entities (master) live in a separate ID namespace —
    // select by kind, never by map order.
    let ret = c
        .live()
        .routing
        .buses
        .values()
        .find(|b| b.kind == BusKind::Return)
        .unwrap()
        .id;
    let ret_strip = c
        .live()
        .routing
        .strips
        .values()
        .find(|s| s.owner == StripOwner::Bus(ret))
        .unwrap()
        .id;
    let send_src = strip_port(c.live(), fx.strip, PortDirection::Output, PortRole::Main);
    let send_dst = strip_port(c.live(), ret_strip, PortDirection::Input, PortRole::Main);
    apply(
        &mut c,
        ProjectOp::AddSend {
            source: send_src,
            destination: send_dst,
            tap: SendTap::PostFader,
            gain_target: None,
        },
    );
    let send_id = *c.live().routing.sends.keys().next_back().unwrap();
    // Route the return bus into the master so the send reaches the
    // render boundary.
    let bus_out = strip_port(c.live(), ret_strip, PortDirection::Output, PortRole::Main);
    let master_in = {
        let master_bus = c.live().routing.master;
        let master_strip = c.live().routing.buses[&master_bus].strip;
        strip_port(c.live(), master_strip, PortDirection::Input, PortRole::Main)
    };
    apply(
        &mut c,
        ProjectOp::AddConnection {
            source: bus_out,
            destination: master_in,
            kind: ConnectionKind::Main,
            channel_map: None,
        },
    );
    let send_level = {
        apply(
            &mut c,
            ProjectOp::AddParameterTarget {
                name: "send-level".into(),
                owner: ControlOwner::Send(send_id),
                role: ParameterRole::SendLevel,
                time_scope: AutomationTimeScope::ProjectMusical,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.8,
            },
        );
        c.live()
            .control
            .targets
            .values()
            .find(|t| t.owner == ControlOwner::Send(send_id))
            .map(|t| t.id)
            .unwrap()
    };
    apply(
        &mut c,
        ProjectOp::SetSendGainTarget {
            send_id,
            gain_target: Some(send_level),
        },
    );
    let processor = *c.live().routing.processors.keys().next_back().unwrap();
    let states = quiescent_states(processor);
    let assets = StaticExternalAssets::default();
    let k_send = key_with(c.live(), &states, &assets);
    apply(
        &mut c,
        ProjectOp::SetSendTap {
            send_id,
            tap: SendTap::PreFader,
        },
    );
    let k_tap = key_with(c.live(), &states, &assets);
    (k_send, k_tap)
}

fn key_asset_digest_change() -> RecipeKey {
    // External asset digest — clone the live project and rewrite the
    // persisted digest claim (still a valid `sha256:` string).
    let (c5, fx5, _p5) = base_fx();
    let mut p5 = c5.live().clone();
    p5.assets.get_mut(&fx5.asset).unwrap().digest = sha(0xB2);
    let processor5 = *p5.routing.processors.keys().next_back().unwrap();
    let states5 = quiescent_states(processor5);
    let assets5 = StaticExternalAssets::default();
    key_with(&p5, &states5, &assets5)
}

fn key_tempo_change() -> RecipeKey {
    let (mut c6, _, processor6) = base_fx();
    apply(
        &mut c6,
        ProjectOp::AddTempoEvent {
            tick: 3_840,
            bpm_milli: 140_000,
        },
    );
    let states6 = quiescent_states(processor6);
    let assets6 = StaticExternalAssets::default();
    key_with(c6.live(), &states6, &assets6)
}

/// Strip-fader automation lane; returns the keys before and after an
/// interpolation-only change.
fn key_automation_change() -> (RecipeKey, RecipeKey) {
    let (mut c7, fx7, processor7) = base_fx();
    let fader = c7
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(fx7.strip) && t.role == ParameterRole::Fader)
        .map(|t| t.id)
        .unwrap();
    apply(&mut c7, ProjectOp::AddAutomationLane { target: fader });
    let lane_id = *c7.live().control.lanes.keys().next_back().unwrap();
    let region = AutomationRegion {
        range: BeatRange::try_new(BeatPoint::new(0, 1).unwrap(), BeatPoint::new(4, 1).unwrap())
            .unwrap(),
        interpolation: Interpolation::Linear,
        points: vec![
            AutomationPoint {
                pos: BeatPoint::new(0, 1).unwrap(),
                value: 0.25,
            },
            AutomationPoint {
                pos: BeatPoint::new(3, 1).unwrap(),
                value: 0.75,
            },
        ],
    };
    apply(
        &mut c7,
        ProjectOp::WriteAutomationRegion {
            lane: lane_id,
            region: region.clone(),
            policy: ConflictPolicy::Reject,
        },
    );
    let states7 = quiescent_states(processor7);
    let assets7 = StaticExternalAssets::default();
    let k7 = key_with(c7.live(), &states7, &assets7);

    let mut stepped = region;
    stepped.interpolation = Interpolation::Step;
    apply(
        &mut c7,
        ProjectOp::WriteAutomationRegion {
            lane: lane_id,
            region: stepped,
            policy: ConflictPolicy::Replace,
        },
    );
    let k8 = key_with(c7.live(), &states7, &assets7);
    (k7, k8)
}

#[test]
fn rec01_acoustic_changes_move_key() {
    let k0 = baseline_key();

    assert_ne!(
        k0,
        key_plugin_param_change(),
        "plugin parameter did not move key"
    );
    assert_ne!(k0, key_fx_reorder(), "FX reorder did not move key");
    assert_ne!(k0, key_sidechain_feed(), "sidechain did not move key");

    let (k_send, k_tap) = key_send_return_path();
    assert_ne!(k0, k_send, "send/return path did not move key");
    assert_ne!(k_send, k_tap, "send tap did not move key");

    assert_ne!(
        k0,
        key_asset_digest_change(),
        "asset digest did not move key"
    );
    assert_ne!(k0, key_tempo_change(), "tempo did not move key");

    let (k_auto, k_interp) = key_automation_change();
    assert_ne!(k0, k_auto, "automation did not move key");
    assert_ne!(k_auto, k_interp, "interpolation change did not move key");

    // Renderer compatibility profile.
    let (c9, _, processor9) = base_fx();
    let states9 = quiescent_states(processor9);
    let assets9 = StaticExternalAssets::default();
    let mut r2 = renderer();
    r2.compatibility_profile = ContentDigest::sha256(b"compat-v2");
    let k9 = match resolve_with(c9.live(), &states9, &assets9, temporal(), r2) {
        RecipeResolution::Resolved { key, .. } => key,
        other => panic!("{other:?}"),
    };
    assert_ne!(k0, k9, "renderer did not move key");
}

// ---------------------------------------------------------------------------
// RCP-REC-02 — fail-closed resolution
// ---------------------------------------------------------------------------

#[test]
fn rec02_plugin_state_unknown_and_degraded_fail_closed() {
    let (c, _, processor) = base_fx();

    // No evidence at all → typed Unknown, never a cache hit.
    match resolve_live(c.live()) {
        RecipeResolution::Unknown { dependencies } => {
            assert!(
                dependencies
                    .iter()
                    .any(|d| matches!(d.cause, UnknownCause::PluginStateUnknown { .. }))
            );
        }
        other => panic!("expected Unknown, got {other:?}"),
    }

    // Degraded quality → still Unknown.
    let mut degraded = BTreeMap::new();
    degraded.insert(
        processor,
        synvelis_render::PluginStateEvidence {
            digest: ContentDigest::sha256(b"state"),
            quality: StateCaptureQuality::BestEffort,
        },
    );
    let plugins = StaticPluginStates(degraded);
    let assets = StaticExternalAssets::default();
    match resolve_with(c.live(), &plugins, &assets, temporal(), renderer()) {
        RecipeResolution::Unknown { dependencies } => {
            assert!(
                dependencies
                    .iter()
                    .any(|d| matches!(d.cause, UnknownCause::PluginStateDegraded { .. }))
            );
        }
        other => panic!("expected Unknown, got {other:?}"),
    }

    // Exact-eligible quality resolves.
    let plugins = quiescent_states(processor);
    let assets = StaticExternalAssets::default();
    match resolve_with(c.live(), &plugins, &assets, temporal(), renderer()) {
        RecipeResolution::Resolved { .. } => {}
        other => panic!("expected Resolved, got {other:?}"),
    }
}

/// Track + lane + clip fed by a weak-digest (non-content-addressed)
/// source pinned `Verified` — the shared fixture for the unpinned /
/// weak asset digest cases.
fn weak_digest_clip_project() -> (ProjectController, AssetId) {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: "0123456789abcdef".into(),
        },
    );
    let asset = *c.live().assets.keys().next_back().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "0123456789abcdef".into(),
        },
    );
    let source = *c.live().sources.keys().next_back().unwrap();
    apply(
        &mut c,
        ProjectOp::SetSourceState {
            source_id: source,
            state: SourceState::Verified,
        },
    );
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_clip_placement(0, 48_000),
            playback: natural_playback(0, 48_000),
        },
    );
    (c, asset)
}

#[test]
fn rec02_unpinned_asset_unverified_source_live_clock() {
    // Weak (non-content-addressed) asset digest + no pin → Unknown.
    let (c, asset) = weak_digest_clip_project();
    match resolve_live(c.live()) {
        RecipeResolution::Unknown { dependencies } => {
            assert!(dependencies.iter().any(|d| matches!(
                d.cause,
                UnknownCause::UnpinnedExternalAsset { .. } | UnknownCause::WeakAssetDigest { .. }
            )));
        }
        other => panic!("expected Unknown, got {other:?}"),
    }

    // Same weak digest but pinned via the probe → Resolved.
    let mut pins = BTreeMap::new();
    pins.insert(
        asset,
        ExternalPin::Pinned {
            digest: ContentDigest::sha256(b"verified"),
        },
    );
    let plugins = NoPluginStates;
    let assets = StaticExternalAssets(pins);
    match resolve_with(c.live(), &plugins, &assets, temporal(), renderer()) {
        RecipeResolution::Resolved { .. } => {}
        other => panic!("expected Resolved, got {other:?}"),
    }

    // `Live` pin → typed unknown (opaque dependency), never resolved.
    let mut live_pins = BTreeMap::new();
    live_pins.insert(asset, ExternalPin::Live);
    let assets = StaticExternalAssets(live_pins);
    match resolve_with(c.live(), &plugins, &assets, temporal(), renderer()) {
        RecipeResolution::Unknown { .. } => {}
        other => panic!("expected Unknown, got {other:?}"),
    }

    // Imported (unverified) source → Unknown even with sha256 asset.
    let (mut c2, fx2) = base();
    apply(&mut c2, ProjectOp::RegisterAsset { digest: sha(0xC3) });
    let fresh_asset = *c2.live().assets.keys().next_back().unwrap();
    apply(
        &mut c2,
        ProjectOp::RegisterSource {
            asset_id: fresh_asset,
            content_digest: sha(0xC3),
        },
    );
    let source2 = *c2.live().sources.keys().next_back().unwrap();
    // Never SetSourceState → stays Imported.
    apply(
        &mut c2,
        ProjectOp::AddClip {
            lane_id: fx2.lane,
            source_id: source2,
            placement: media_clip_placement(48_000, 96_000),
            playback: natural_playback(0, 48_000),
        },
    );
    match resolve_live(c2.live()) {
        RecipeResolution::Unknown { dependencies } => {
            assert!(
                dependencies
                    .iter()
                    .any(|d| matches!(d.cause, UnknownCause::UnverifiedSource { .. }))
            );
        }
        other => panic!("expected Unknown, got {other:?}"),
    }

    // Live clock → Unsupported; unpinned clock → Unknown.
    let plugins = NoPluginStates;
    let assets = NoExternalAssets;
    let mut t_live = temporal();
    t_live.clock = ClockDependency::Live;
    match resolve_with(c2.live(), &plugins, &assets, t_live, renderer()) {
        RecipeResolution::Unsupported { reason } => {
            assert!(matches!(reason, UnsupportedReason::LiveClock));
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
    let mut t_unpinned = temporal();
    t_unpinned.clock = ClockDependency::Unpinned;
    match resolve_with(c2.live(), &plugins, &assets, t_unpinned, renderer()) {
        RecipeResolution::Unknown { dependencies } => {
            assert!(
                dependencies
                    .iter()
                    .any(|d| matches!(d.cause, UnknownCause::UnpinnedClock))
            );
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[test]
fn rec02_nonfinite_and_schema_skew_fail_closed() {
    // Non-finite parameter base → typed failure (canonical extraction
    // rejects it; the reference oracle may reject first — either way
    // the resolution is never Resolved).
    let (c, fx) = base();
    let fader = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(fx.strip) && t.role == ParameterRole::Fader)
        .map(|t| t.id)
        .unwrap();
    let mut p = c.live().clone();
    p.control.targets.get_mut(&fader).unwrap().base = f64::NAN;
    // resolve_render_project would reject via validate_references;
    // resolve_recipe applies the same oracle and then canonicalization.
    let boundary = RenderBoundary::master(&p).unwrap();
    let plugins = NoPluginStates;
    let assets = NoExternalAssets;
    let env = RecipeEnvironment {
        plugin_states: &plugins,
        external_assets: &assets,
    };
    let out = OutputContract {
        ports: boundary
            .ports
            .iter()
            .map(|p| OutputPortContract {
                port: *p,
                channels: 2,
                format: PcmFormat::F32,
            })
            .collect(),
    };
    match resolve_recipe(&RecipeRequest {
        project: &p,
        boundary,
        temporal: temporal(),
        renderer: renderer(),
        output: out,
        environment: &env,
        branch: None,
        provenance: ResolutionProvenance::default(),
        project_schema_version: SCHEMA_VERSION,
        render_input_schema_version: RENDER_INPUT_SCHEMA_VERSION,
    }) {
        RecipeResolution::Unknown { dependencies } => {
            assert!(
                dependencies
                    .iter()
                    .any(|d| matches!(d.cause, UnknownCause::InvalidValue { .. }))
            );
        }
        RecipeResolution::Unsupported { .. } => {}
        other @ RecipeResolution::Resolved { .. } => {
            panic!("expected fail-closed, got {other:?}")
        }
    }

    // Old project schema → typed Unknown(UnsupportedSchema).
    let (c2, _) = base();
    let plugins = NoPluginStates;
    let assets = NoExternalAssets;
    let resolved = resolve_render_project(c2.live(), None).unwrap();
    let boundary = RenderBoundary::master(&resolved.project).unwrap();
    let env = RecipeEnvironment {
        plugin_states: &plugins,
        external_assets: &assets,
    };
    let out = OutputContract {
        ports: boundary
            .ports
            .iter()
            .map(|p| OutputPortContract {
                port: *p,
                channels: 2,
                format: PcmFormat::F32,
            })
            .collect(),
    };
    match resolve_recipe(&RecipeRequest {
        project: &resolved.project,
        boundary,
        temporal: temporal(),
        renderer: renderer(),
        output: out,
        environment: &env,
        branch: None,
        provenance: ResolutionProvenance::default(),
        project_schema_version: SCHEMA_VERSION - 1,
        render_input_schema_version: RENDER_INPUT_SCHEMA_VERSION,
    }) {
        RecipeResolution::Unknown { dependencies } => {
            assert!(
                dependencies
                    .iter()
                    .any(|d| matches!(d.cause, UnknownCause::UnsupportedSchema { .. }))
            );
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// RCP-REC-03 — closure consistency + unrelated subgraph invariance
// ---------------------------------------------------------------------------

#[test]
fn rec03_closure_covers_signal_and_unrelated_subgraph_isolated() {
    let mut c = controller();
    let a = track_with_clip(&mut c);
    let processor = insert_plugin(&mut c, a.strip, true);
    let param = plugin_param_target(&mut c, processor);
    apply(
        &mut c,
        ProjectOp::SetParameterBase {
            target: param,
            value: 0.7,
        },
    );

    // Second track whose strip is disconnected from master — its whole
    // subgraph must stay outside the closure.
    let b = track_with_clip(&mut c);
    let conn = c
        .live()
        .routing
        .connections
        .values()
        .find(|cn| {
            cn.kind == ConnectionKind::Main
                && c.live().routing.ports[&cn.source].owner == PortOwner::Strip(b.strip)
        })
        .unwrap()
        .id;
    apply(
        &mut c,
        ProjectOp::RemoveConnection {
            connection_id: conn,
        },
    );

    let plugins = quiescent_states(processor);
    let assets = StaticExternalAssets::default();
    let resolution = resolve_with(c.live(), &plugins, &assets, temporal(), renderer());
    let RecipeResolution::Resolved { recipe, .. } = resolution else {
        panic!("expected Resolved, got {resolution:?}");
    };

    let kinds: std::collections::BTreeSet<_> =
        recipe.closure.nodes.iter().map(|n| n.object.kind).collect();
    for expected in [
        ObjectKind::Project,
        ObjectKind::Bus,
        ObjectKind::Strip,
        ObjectKind::ProcessorChain,
        ObjectKind::Processor,
        ObjectKind::SemanticPort,
        ObjectKind::Track,
        ObjectKind::Lane,
        ObjectKind::Clip,
        ObjectKind::MediaSource,
        ObjectKind::Asset,
        ObjectKind::ParameterTarget,
        ObjectKind::RoutingModel,
    ] {
        assert!(kinds.contains(&expected), "closure missing {expected:?}");
    }
    // Disconnected track B subgraph absent.
    for node in &recipe.closure.nodes {
        assert_ne!(
            node.object.stable_id,
            b.clip.to_tagged(),
            "disconnected clip entered closure"
        );
        assert_ne!(
            node.object.stable_id,
            b.source.to_tagged(),
            "disconnected source entered closure"
        );
    }
    // Typed edge kinds present.
    let edge_kinds: std::collections::BTreeSet<_> =
        recipe.closure.edges.iter().map(|e| e.kind).collect();
    assert!(edge_kinds.contains(&synvelis_render::DependencyEdgeKind::Audio));
    assert!(edge_kinds.contains(&synvelis_render::DependencyEdgeKind::Event));
    assert!(edge_kinds.contains(&synvelis_render::DependencyEdgeKind::Parameter));
    assert!(edge_kinds.contains(&synvelis_render::DependencyEdgeKind::TimeMapping));

    // Muting clip B (outside closure) must not move the key.
    let k1 = key_with(c.live(), &plugins, &assets);
    apply(
        &mut c,
        ProjectOp::SetClipMuted {
            clip_id: b.clip,
            muted: true,
        },
    );
    assert_eq!(
        k1,
        key_with(c.live(), &plugins, &assets),
        "unrelated subgraph moved key"
    );

    // Muting clip A (inside closure) must move the key.
    apply(
        &mut c,
        ProjectOp::SetClipMuted {
            clip_id: a.clip,
            muted: true,
        },
    );
    assert_ne!(
        k1,
        key_with(c.live(), &plugins, &assets),
        "in-closure change did not move key"
    );
}

// ---------------------------------------------------------------------------
// RCP-REC-04 — temporal invalidation / partial re-render
// ---------------------------------------------------------------------------

#[test]
fn rec04_dirty_interval_expansion_and_fail_closed_full() {
    let (c, fx) = base();
    let resolution = resolve_live(c.live());
    let RecipeResolution::Resolved { recipe, .. } = resolution else {
        panic!("expected Resolved, got {resolution:?}");
    };

    let clip_obj = object_ref(c.live(), ObjectKind::Clip, fx.clip.to_tagged());
    // Dirty [1000,2000) expands back by lookahead+PDC (96) and forward
    // by tail (480), clipped to [0,48000).
    let plan = classify_invalidation(
        &recipe,
        &[ChangedObject {
            object: clip_obj.clone(),
            interval: Some(FrameInterval::new(1000, 2000).unwrap()),
            temporal: false,
        }],
    );
    assert!(!plan.full);
    assert_eq!(plan.dirty_intervals.len(), 1);
    let iv = plan.dirty_intervals[0];
    assert_eq!(iv.start, 904);
    assert_eq!(iv.end, 2480);
    assert!(plan.reasons.contains(&InvalidationReason::LookaheadPdc));
    assert!(plan.reasons.contains(&InvalidationReason::Tail));

    // Object outside the closure → no dirty interval at all.
    let foreign = object_ref(
        c.live(),
        ObjectKind::Clip,
        "clip-ffffffffffffffffffffffffffffffff".into(),
    );
    let plan = classify_invalidation(
        &recipe,
        &[ChangedObject {
            object: foreign,
            interval: Some(FrameInterval::new(0, 100).unwrap()),
            temporal: false,
        }],
    );
    assert_eq!(plan.dirty_intervals, Vec::<FrameInterval>::new());
    assert!(!plan.full);

    // Unknown extent → full invalidation (never a guessed sub-interval).
    let plan = classify_invalidation(
        &recipe,
        &[ChangedObject {
            object: clip_obj.clone(),
            interval: None,
            temporal: false,
        }],
    );
    assert!(plan.full);
    assert_eq!(
        plan.dirty_intervals,
        vec![FrameInterval::new(0, 48_000).unwrap()]
    );
    assert!(plan.reasons.contains(&InvalidationReason::UnknownExtent));

    // Temporal-domain change (tempo-map carrier) → full.
    let project_obj = object_ref(
        c.live(),
        ObjectKind::Project,
        c.live().project_id.to_tagged(),
    );
    let plan = classify_invalidation(
        &recipe,
        &[ChangedObject {
            object: project_obj,
            interval: Some(FrameInterval::new(0, 10).unwrap()),
            temporal: true,
        }],
    );
    assert!(plan.full);
    assert!(plan.reasons.contains(&InvalidationReason::TemporalDomain));
}

#[test]
fn rec04_loop_seek_and_tail_expansion() {
    let (c, fx) = base();
    let mut t = temporal();
    t.loop_region = Some(FrameInterval::new(4_000, 8_000).unwrap());
    t.seek_boundaries = vec![20_000];
    let plugins = NoPluginStates;
    let assets = NoExternalAssets;
    let resolution = resolve_with(c.live(), &plugins, &assets, t, renderer());
    let RecipeResolution::Resolved { recipe, .. } = resolution else {
        panic!("expected Resolved, got {resolution:?}");
    };
    let clip_obj = object_ref(c.live(), ObjectKind::Clip, fx.clip.to_tagged());

    // Dirty inside the loop region → whole loop coverage re-renders.
    let plan = classify_invalidation(
        &recipe,
        &[ChangedObject {
            object: clip_obj.clone(),
            interval: Some(FrameInterval::new(4_500, 5_000).unwrap()),
            temporal: false,
        }],
    );
    assert!(plan.reasons.contains(&InvalidationReason::LoopRegion));
    assert!(
        plan.dirty_intervals
            .iter()
            .any(|iv| iv.start <= 4_000 && iv.end >= 8_000)
    );

    // Dirty touching a seek boundary → extends to interval end.
    let plan = classify_invalidation(
        &recipe,
        &[ChangedObject {
            object: clip_obj,
            interval: Some(FrameInterval::new(19_950, 20_050).unwrap()),
            temporal: false,
        }],
    );
    assert!(plan.reasons.contains(&InvalidationReason::SeekBoundary));
    assert_eq!(plan.dirty_intervals.last().unwrap().end, 48_000);
}

// ---------------------------------------------------------------------------
// RCP-REC-05 — branch reuse identity
// ---------------------------------------------------------------------------

struct BranchFixture {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    adapter: FakeDomainAdapter,
    resolver: SnapshotResolver,
}

impl BranchFixture {
    fn new(c: ProjectController) -> Self {
        Self {
            controller: c,
            store: InMemoryBranchContentStore::default(),
            adapter: FakeDomainAdapter::new(),
            resolver: SnapshotResolver::new(),
        }
    }

    fn fork(&mut self, scope: VariantScope) -> synvelis_project::variant::ForkReceipt {
        fork_variant_set(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &self.resolver,
            scope,
            &SnapshotCancel::new(),
        )
        .unwrap()
    }

    fn edit(&mut self, set: VariantSetId, branch: BranchId, op: ProjectOp) {
        let expected = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .revision();
        edit_branch(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &synvelis_project::variant::BranchEditIntent {
                variant_set: set,
                branch,
                expected_branch_revision: expected,
                op,
            },
        )
        .unwrap();
    }

    fn resolve_branch(&self, set: VariantSetId, branch: BranchId) -> RecipeResolution {
        let live = self.controller.live();
        let target = RenderBranchTarget {
            variant_set: set,
            branch,
            store: &self.store,
        };
        let resolved = resolve_render_project(live, Some(target)).unwrap();
        let boundary = RenderBoundary::master(&resolved.project).unwrap();
        let plugins = NoPluginStates;
        let assets = NoExternalAssets;
        let env = RecipeEnvironment {
            plugin_states: &plugins,
            external_assets: &assets,
        };
        let out = OutputContract {
            ports: boundary
                .ports
                .iter()
                .map(|p| OutputPortContract {
                    port: *p,
                    channels: 2,
                    format: PcmFormat::F32,
                })
                .collect(),
        };
        let pin = resolved.branch.as_ref().unwrap();
        let branch_pin = RecipeBranchPin {
            scope_digest: pin.scope_digest,
            shared_context_digest: pin.shared_context_digest,
        };
        let provenance = ResolutionProvenance {
            live_revision: Some(self.controller.live_revision().0),
            variant_set: Some(set),
            branch: Some(branch),
            branch_revision: Some(pin.branch_revision.raw()),
            payload_digest: pin.payload_digest,
            ..ResolutionProvenance::default()
        };
        resolve_recipe(&RecipeRequest {
            project: &resolved.project,
            boundary,
            temporal: temporal(),
            renderer: renderer(),
            output: out,
            environment: &env,
            branch: Some(branch_pin),
            provenance,
            project_schema_version: SCHEMA_VERSION,
            render_input_schema_version: RENDER_INPUT_SCHEMA_VERSION,
        })
    }
}

fn branch_key(resolution: &RecipeResolution) -> RecipeKey {
    match resolution {
        RecipeResolution::Resolved { key, .. } => *key,
        other => panic!("expected Resolved, got {other:?}"),
    }
}

#[test]
fn rec05_shared_edit_invalidates_all_branches_internal_edit_isolated() {
    let mut c = controller();
    let fx = track_with_clip(&mut c);
    // A second clip on another lane — outside the AudioClip scope but
    // inside the shared context (and inside the render closure).
    apply(&mut c, ProjectOp::AddLane { track_id: fx.track });
    let lane2 = *c.live().tracks[&fx.track].lane_ids.last().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane2,
            source_id: fx.source,
            placement: media_clip_placement(96_000, 144_000),
            playback: natural_playback(0, 48_000),
        },
    );
    let clip_b = *c.live().clips.keys().next_back().unwrap();

    let mut fixture = BranchFixture::new(c);
    let receipt = fixture.fork(VariantScope::AudioClip { clip: fx.clip });
    let set = receipt.variant_set;
    let adopted = receipt.adopted;
    let candidate = receipt.candidate;

    // Baseline keys for both branches (candidate starts as live mirror).
    let k_adopted_0 = branch_key(&fixture.resolve_branch(set, adopted));
    let k_candidate_0 = branch_key(&fixture.resolve_branch(set, candidate));
    assert_eq!(
        k_adopted_0, k_candidate_0,
        "identical acoustic graphs differ"
    );

    // Branch-internal edit on the candidate (mute the scoped clip).
    fixture.edit(
        set,
        candidate,
        ProjectOp::SetClipMuted {
            clip_id: fx.clip,
            muted: true,
        },
    );
    let k_candidate_1 = branch_key(&fixture.resolve_branch(set, candidate));
    let k_adopted_1 = branch_key(&fixture.resolve_branch(set, adopted));
    assert_ne!(k_candidate_0, k_candidate_1, "branch edit did not move key");
    assert_eq!(
        k_adopted_0, k_adopted_1,
        "sibling branch moved on internal edit"
    );

    // Shared upstream edit: mute clip_b — outside the AudioClip scope,
    // inside the shared context → every branch's key moves.
    apply(
        &mut fixture.controller,
        ProjectOp::SetClipMuted {
            clip_id: clip_b,
            muted: true,
        },
    );
    let k_adopted_2 = branch_key(&fixture.resolve_branch(set, adopted));
    let k_candidate_2 = branch_key(&fixture.resolve_branch(set, candidate));
    assert_ne!(
        k_adopted_1, k_adopted_2,
        "shared edit did not move adopted key"
    );
    assert_ne!(
        k_candidate_1, k_candidate_2,
        "shared edit did not move candidate key"
    );
}

#[test]
fn rec05_revision_only_change_does_not_invalidate() {
    let (c, fx) = base();
    let mut fixture = BranchFixture::new(c);
    let receipt = fixture.fork(VariantScope::AudioClip { clip: fx.clip });
    let set = receipt.variant_set;
    let candidate = receipt.candidate;

    // Capture the candidate (scoped edit) then resolve, touch the live
    // project outside the scope (project rename is display-only and
    // outside scope objects entirely), resolve again: identical
    // acoustic content → identical key. Revisions/payload digests are
    // provenance, never hashed.
    fixture.edit(
        set,
        candidate,
        ProjectOp::SetClipMuted {
            clip_id: fx.clip,
            muted: true,
        },
    );
    let k1 = branch_key(&fixture.resolve_branch(set, candidate));
    apply(
        &mut fixture.controller,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let r2 = fixture.resolve_branch(set, candidate);
    let k2 = branch_key(&r2);
    assert_eq!(k1, k2, "revision/display-only change moved key");
    if let RecipeResolution::Resolved { provenance, .. } = &r2 {
        assert!(provenance.variant_set.is_some());
        assert!(provenance.branch.is_some());
    }
}

// ---------------------------------------------------------------------------
// RCP-REC-06 — versioning, lookup/migration, collision injection
// ---------------------------------------------------------------------------

#[test]
fn rec06_record_verify_and_collision_fail_closed() {
    let (c, _) = base();
    let resolution = resolve_live(c.live());
    let RecipeResolution::Resolved { recipe, key, .. } = resolution else {
        panic!("expected Resolved");
    };

    // Stored record verifies → Match on identical resolution.
    let record: RecipeRecord = recipe_record(&recipe, key).unwrap();
    assert_eq!(record.verify(), Ok(()));
    assert_eq!(
        evaluate_validity(&record, &resolve_live(c.live())),
        Validity::Match
    );

    // Acoustic change → Stale (both sides resolved, keys differ).
    let (mut c2, _) = base();
    apply(
        &mut c2,
        ProjectOp::AddTempoEvent {
            tick: 3_840,
            bpm_milli: 120_000,
        },
    );
    assert_eq!(
        evaluate_validity(&record, &resolve_live(c2.live())),
        Validity::Stale
    );

    // Hash-collision injection: record claims the right key over
    // different bytes → verify fails → evaluate is Unknown, never Match.
    let mut forged = record.clone();
    let mut other_recipe = recipe.clone();
    other_recipe.renderer_digest = ContentDigest::sha256(b"other");
    forged.canonical_bytes = recipe_record(&other_recipe, key).unwrap().canonical_bytes;
    assert!(forged.verify().is_err());
    assert_eq!(
        evaluate_validity(&forged, &resolve_live(c.live())),
        Validity::Unknown
    );

    // Old schema record → VersionMismatch → Unknown (no silent
    // re-interpretation under new rules).
    let mut old = record.clone();
    old.schema_version = synvelis_render::RECIPE_SCHEMA_VERSION - 1;
    old.canonical_bytes = recipe.canonical_bytes().unwrap();
    // Recompute the key over the same bytes so only the version check
    // can fail.
    let mut input = synvelis_render::RECIPE_HASH_DOMAIN.to_vec();
    input.extend_from_slice(&old.canonical_bytes);
    old.key = RecipeKey(ContentDigest::sha256(&input));
    assert!(matches!(
        old.verify(),
        Err(synvelis_render::RecipeRecordError::VersionMismatch { .. })
    ));
    assert_eq!(
        evaluate_validity(&old, &resolve_live(c.live())),
        Validity::Unknown
    );

    // Unknown current resolution → Unknown regardless of record.
    let (mut c3, fx3) = base();
    insert_plugin(&mut c3, fx3.strip, false); // Resolved binding, no state evidence
    assert_eq!(
        evaluate_validity(&record, &resolve_live(c3.live())),
        Validity::Unknown
    );
}
