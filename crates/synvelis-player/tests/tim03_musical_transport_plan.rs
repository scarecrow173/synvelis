//! TIM-03 production plan-owned musical transport tests.
//!
//! These tests verify Synvelis's supply responsibility at the production
//! `ParamDispatch` boundary. They do not retest ruviform's tempo math: the
//! oracle is that the exact project-owned map identity, revision, and PPQN
//! are present together, while invalid input remains honestly absent.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, PluginNode, attach};
use ruviform_graph::{GraphBuilder, NodeDef, NodeKind};
use ruviform_media_core::{
    DeliveryPolicy, MediaTime, MidiChannel, MidiEvent, MidiGroup, MidiKind, MidiValue, NoteKey,
    ScheduleBudget, TimeBase, TransportGeneration,
};
use ruviform_plugin_core::{
    BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind, ParamEvent, ParamFlags, ParamId,
    ParamInfo, ParamValue, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, ProcessConfig,
    ProcessContext, ProcessResult, TimedParamEvent, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use serde_json::{Value, json};
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::PluginHostProcessors;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{
    FastControlOutcome, FastControlWrite, MusicalTransportError, ParamDispatch,
    PlanMusicalTransport, RoutingControlDispatch, compile_project_musical_transport,
};
use synvelis_project::tempo::{SignatureDto, TempoEventDto, TempoMapData};
use synvelis_project::{
    AutomationTimeScope, BeatPoint, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    ControlOwner, CounterIdGenerator, LaneId, MediaDuration, MediaTimeValue, ParamMapping,
    ParameterRole, ParameterTargetId, PlaybackMapping, ProcessorBinding, ProcessorId, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, RoutingBuildRequestId,
    RoutingCompileRequest, RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame, SourceId,
    SourceRange, StripId, TargetCapability, TimeAnchor, TrackId, ValueKind,
};

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim03-musical-plan",
    name: "TIM-03 Musical Plan Dummy",
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

struct Dummy;

impl PluginProcessor for Dummy {
    fn prepare(
        &mut self,
        _config: &ProcessConfig,
        _context: &mut ProcessContext,
    ) -> Result<(), ruviform_plugin_core::PrepareError> {
        Ok(())
    }

    fn process<I, O>(
        &mut self,
        _inputs: &[I],
        _outputs: &mut [O],
        _events: ruviform_plugin_core::EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: ruviform_plugin_core::BufferView,
        O: ruviform_plugin_core::BufferViewMut,
    {
        ProcessResult::CONTINUE
    }
}

impl Plugin for Dummy {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &PARAMS
    }
    fn buses(&self) -> Option<&[BusDescriptor]> {
        Some(&BUSES)
    }
}

struct Factory;

impl PluginFactory for Factory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(Dummy)
    }

    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(Dummy))
    }
}

struct Hosted {
    _host: PluginHost,
    control: ruviform_audio_plugin_bridge::PluginNodeControl,
    _node: PluginNode,
}

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

fn host_plugin() -> Hosted {
    let mut host = PluginHost::new();
    host.register_factory(Box::new(Factory));
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
            port_format: ruviform_audio_core::AudioFormat {
                sample_rate: SampleRate::new(48_000).unwrap(),
                layout: ChannelLayout::Stereo,
                sample_format: ruviform_audio_core::SampleFormat::F32,
            },
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
    .unwrap();
    Hosted {
        _host: host,
        control: attached.control,
        _node: attached.node,
    }
}

fn project_480_150bpm() -> Project {
    let mut project = Project::new(ProjectId::new(1), "tim03 musical");
    project.ppqn.0 = 480;
    project.tempo_map.ppqn = 480;
    project.tempo_map.revision = 9;
    project.tempo_map.tempo_events = vec![
        TempoEventDto {
            tick: 0,
            bpm_milli: 150_000,
        },
        TempoEventDto {
            tick: 960,
            bpm_milli: 90_000,
        },
    ];
    project.tempo_map.signature_events = vec![SignatureDto {
        tick: 0,
        numerator: 3,
        denominator: 8,
    }];
    project
}

fn param_write(pos: i64, value: f64) -> (ParameterTargetId, TimedParamEvent) {
    (
        ParameterTargetId::new(1),
        TimedParamEvent {
            position: MediaTime::new(pos, frame_base()),
            event: ParamEvent {
                param_id: ParamId(7),
                value: ParamValue::new(value).unwrap(),
            },
        },
    )
}

fn note_on(key: u8) -> MidiEvent {
    MidiEvent {
        group: MidiGroup::ZERO,
        channel: MidiChannel::new(0).unwrap(),
        kind: MidiKind::NoteOn {
            key: NoteKey::new(key).unwrap(),
            velocity: MidiValue::from_u7(100),
        },
    }
}

fn issued_node() -> ruviform_graph::NodeId {
    let mut builder = GraphBuilder::new();
    builder
        .add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: Vec::new(),
        })
        .0
}

fn dispatch_with_source() -> (ParamDispatch, synvelis_player::PlanMusicalTransport) {
    let hosted = host_plugin();
    let node = issued_node();
    let mut dispatch = ParamDispatch::new(hosted.control, node, frame_base()).unwrap();
    dispatch
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .unwrap();
    let project = project_480_150bpm();
    let source = compile_project_musical_transport(&project).unwrap();
    assert!(dispatch.set_musical_transport(Some(source.clone())));
    (dispatch, source)
}

fn assert_complete_musical(
    plan: &ruviform_media_core::TransportPlan<EventKind>,
    source: &synvelis_player::PlanMusicalTransport,
) {
    assert!(
        plan.tempo_map.is_some(),
        "plan must own a compiled tempo map"
    );
    assert_eq!(plan.tempo_revision, source.tempo_revision());
    assert_eq!(plan.musical_ppqn, Some(source.ppqn()));
}

#[test]
fn production_plan_mints_carry_complete_project_musical_source() {
    let (mut dispatch, source) = dispatch_with_source();

    let param_plan = dispatch.dispatch(&[param_write(64, 0.25)]).unwrap();
    assert_complete_musical(&param_plan, &source);

    let midi = [(32, note_on(60)), (96, note_on(64))];
    let midi_plan = dispatch.dispatch_midi(&midi).unwrap();
    assert_complete_musical(&midi_plan, &source);
    assert_eq!(midi_plan.events.len(), 3);
    assert_eq!(
        midi_plan
            .events
            .iter()
            .filter(|event| matches!(event.payload, EventKind::Midi(_)))
            .count(),
        2
    );

    let midi_free_plan = dispatch.clear_midi_plan().unwrap();
    assert_complete_musical(&midi_free_plan, &source);
    assert!(
        midi_free_plan
            .events
            .iter()
            .all(|event| !matches!(event.payload, EventKind::Midi(_)))
    );
}

#[test]
fn invalid_project_state_is_honestly_absent_not_fixed_fallback() {
    let mut project = project_480_150bpm();
    project.tempo_map.ppqn = 960;
    let err = compile_project_musical_transport(&project).unwrap_err();
    assert!(matches!(
        err,
        MusicalTransportError::PpqnMismatch {
            project_ppqn: 480,
            tempo_map_ppqn: 960
        }
    ));

    let hosted = host_plugin();
    let mut dispatch = ParamDispatch::new(hosted.control, issued_node(), frame_base()).unwrap();
    dispatch
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .unwrap();
    let plan = dispatch.dispatch(&[param_write(64, 0.25)]).unwrap();
    assert!(plan.tempo_map.is_none());
    assert_eq!(plan.musical_ppqn, None, "no fixed 960 fallback may appear");
    println!(
        "tim03-dynamic-tempo-undo-redo {}",
        json!({
            "schema": "tim03-dynamic-tempo-undo-redo/v1",
            "row_family": "negative_and_constraint_evidence",
            "test": "invalid_project_state_is_honestly_absent_not_fixed_fallback",
            "tested_sha": std::env::var("TIM03_VERIFIED_SHA").ok(),
            "invalid_musical_input": "PpqnMismatch(480, 960)",
            "missing_tempo_map": true,
            "missing_ppqn": true,
            "fixed_960_fallback": false,
            "verdict": "pass",
        })
    );
}

#[test]
fn musical_refresh_preserves_event_payload_positions_and_order() {
    let (mut dispatch, first) = dispatch_with_source();
    let midi = [(32, note_on(60)), (96, note_on(64))];
    let before = dispatch.dispatch_midi(&midi).unwrap();
    assert!(dispatch.last_plan_musical_is_current());

    let mut project = project_480_150bpm();
    project.tempo_map.revision = 10;
    project.tempo_map.tempo_events[0].bpm_milli = 144_000;
    let second = compile_project_musical_transport(&project).unwrap();
    assert!(!first.same_project_basis(&second));
    assert!(dispatch.set_musical_transport(Some(second.clone())));
    assert!(!dispatch.last_plan_musical_is_current());

    let after = dispatch.refresh_musical_plan().unwrap().unwrap();
    assert_complete_musical(&after, &second);
    assert_plan_events_preserved(&before, &after);
    assert_eq!(dispatch.staged_midi(), &midi);
    assert!(dispatch.last_plan_musical_is_current());
}

// -------------------------------------------------------------

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("synvelis-tim03mt-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn controller_with_musical_project() -> ProjectController {
    ProjectController::new(project_480_150bpm(), Box::new(CounterIdGenerator::new(1)))
}

fn apply_project_op(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
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

fn import_stereo(
    c: &mut ProjectController,
    root: &Path,
    frames: usize,
) -> synvelis_player::ImportOutcome {
    let wav = root.join("a.wav");
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

fn add_track_strip(c: &mut ProjectController) -> (TrackId, LaneId, StripId) {
    apply_project_op(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply_project_op(c, ProjectOp::AddTrackStrip { track_id: track });
    apply_project_op(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, lane, strip)
}

fn add_clip(
    c: &mut ProjectController,
    lane: LaneId,
    source: SourceId,
    start: u64,
    end: u64,
    offset: i64,
) -> ClipId {
    apply_project_op(
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

fn insert_instrument(c: &mut ProjectController, strip: StripId) -> ProcessorId {
    apply_project_op(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "inst".into(),
            binding: ProcessorBinding::Resolved(synvelis_project::PluginRef {
                descriptor: DESCRIPTOR.id.into(),
                version: "0.1.0".into(),
                capability_revision: 0,
            }),
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn add_gain_target(c: &mut ProjectController, processor: ProcessorId) -> ParameterTargetId {
    apply_project_op(
        c,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id: 7,
                flags_bits: ParamFlags::AUTOMATABLE.bits(),
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
            }),
            base: 0.0,
        },
    );
    c.live()
        .control
        .targets
        .values()
        .find(|target| target.name == "gain")
        .unwrap()
        .id
}

fn canonical_format() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn prefetch() -> ruviform_media_reader::PrefetchConfig {
    ruviform_media_reader::PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: 2,
        sample_rate: 48_000,
    }
}

fn routing_request(id: u64, revision: ProjectRevision) -> RoutingCompileRequest {
    RoutingCompileRequest {
        request_id: RoutingBuildRequestId(id),
        expected_project_revision: revision,
        mode: synvelis_project::BuildMode::Realtime,
        resource_budget: synvelis_project::BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        runtime_instance: RuntimeInstanceId(1),
        device_clock_epoch: RuntimeClockEpochRef(1),
    }
}

fn routing_deps<'a>(project: &Project, media_root: &Path) -> RoutingDeps<'a> {
    RoutingDeps {
        canonical_of: &|_| Some(canonical_format()),
        clips: project
            .clips
            .values()
            .map(|clip| {
                synvelis_player::resolver::prepare(project, clip.source_id, media_root, &prefetch())
                    .unwrap()
            })
            .collect(),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
}

fn publish_first(c: &ProjectController, media: &Path, eng: &mut RoutingEngine) -> u64 {
    let prepared = eng
        .prepare(
            &routing_request(1, c.live_revision()),
            c.live(),
            c.live_revision(),
            routing_deps(c.live(), media),
        )
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), c.live_revision()).unwrap();
    eng.source_map().map(|map| map.generation).unwrap()
}

fn active_generation(eng: &RoutingEngine) -> Option<u64> {
    let generation = eng.rt_status().active_generation;
    (generation != 0).then_some(generation)
}

fn fast_write(
    target: ParameterTargetId,
    frame: i64,
    value: f64,
    revision: ProjectRevision,
    transport_generation: Option<TransportGeneration>,
) -> FastControlWrite {
    FastControlWrite {
        target,
        value,
        means: ruviform_plugin_core::ParamWriteMeans::TimedEvent,
        position: MediaTime::new(frame, frame_base()),
        project_revision: revision,
        transport_generation,
    }
}

fn event_at(
    plan: &ruviform_media_core::TransportPlan<EventKind>,
    frame: i64,
) -> &ruviform_media_core::ScheduledEvent<EventKind> {
    let position = MediaTime::new(frame, frame_base());
    plan.events
        .iter()
        .find(|event| event.position == position)
        .unwrap_or_else(|| panic!("missing event at frame {frame}: {:?}", plan.events))
}

fn mixed_refresh_control(
    scratch_name: &str,
) -> (
    ProjectController,
    ProcessorId,
    ParameterTargetId,
    RoutingEngine,
    RoutingControlDispatch,
) {
    let root = scratch(scratch_name);
    let mut c = controller_with_musical_project();
    let imported = import_stereo(&mut c, &root, 4_800);
    let (_, lane, strip) = add_track_strip(&mut c);
    add_clip(&mut c, lane, imported.commit.source_id, 0, 480, 0);
    let processor = insert_instrument(&mut c, strip);
    let target = add_gain_target(&mut c, processor);

    let mut host = PluginHost::new();
    host.register_factory(Box::new(Factory));
    let mut eng = RoutingEngine::new(
        synvelis_player::lowering::EngineConfig {
            sample_rate: 48_000,
            channels: 2,
            block_frames: 256,
        },
        RuntimeInstanceId(1),
        RuntimeClockEpochRef(1),
    );
    eng.set_processor_host(Box::new(PluginHostProcessors::new(host)));
    let media = root.join("media");
    let generation = publish_first(&c, &media, &mut eng);
    let catalog = BTreeMap::from([(processor, PARAMS.to_vec())]);
    let control = RoutingControlDispatch::attach(&mut eng, generation, c.live(), &catalog).unwrap();
    (c, processor, target, eng, control)
}

fn assert_plan_events_preserved(
    before: &ruviform_media_core::TransportPlan<EventKind>,
    after: &ruviform_media_core::TransportPlan<EventKind>,
) {
    assert_eq!(after.events.len(), before.events.len());
    for (old, new) in before.events.iter().zip(&after.events) {
        assert_eq!(new.position, old.position);
        assert_eq!(new.source_sequence, old.source_sequence);
        assert_eq!(new.payload, old.payload);
        assert_eq!(new.transport_generation, old.transport_generation);
    }
}

fn non_midi_events(
    plan: &ruviform_media_core::TransportPlan<EventKind>,
) -> Vec<ruviform_media_core::ScheduledEvent<EventKind>> {
    plan.events
        .iter()
        .filter(|event| !matches!(event.payload, EventKind::Midi(_)))
        .copied()
        .collect()
}

fn assert_restaged_midi_preserves_params(
    control: &mut RoutingControlDispatch,
    project: &Project,
    processor: ProcessorId,
    refreshed: &ruviform_media_core::TransportPlan<EventKind>,
) {
    let params_before = non_midi_events(refreshed);
    let restaged = control
        .stage_midi_plan(project, processor, &[(700, note_on(62))])
        .unwrap();
    let params_after = non_midi_events(&restaged);
    assert_eq!(params_after.len(), params_before.len());
    for (before, after) in params_before.iter().zip(&params_after) {
        assert_eq!(
            (
                after.position,
                after.source_sequence,
                after.payload,
                after.transport_generation
            ),
            (
                before.position,
                before.source_sequence,
                before.payload,
                before.transport_generation
            )
        );
    }
    let current_source = compile_project_musical_transport(project).unwrap();
    assert_eq!(restaged.tempo_revision, current_source.tempo_revision());
    assert_eq!(restaged.musical_ppqn, Some(current_source.ppqn()));
}

fn assert_invalid_ppqn_refresh_is_honestly_absent(
    control: &mut RoutingControlDispatch,
    project: &Project,
    processor: ProcessorId,
) {
    let mut invalid = project.clone();
    invalid.tempo_map.ppqn = 960;
    let invalid_sync = control.sync_musical_transport(&invalid);
    assert!(matches!(
        invalid_sync.source,
        synvelis_player::MusicalTransportEvidence::Unavailable {
            error: MusicalTransportError::PpqnMismatch {
                project_ppqn: 480,
                tempo_map_ppqn: 960
            }
        }
    ));
    let invalid_plan = control.last_plan(processor).unwrap();
    assert!(invalid_plan.tempo_map.is_none());
    assert_eq!(invalid_plan.musical_ppqn, None);
}

fn stage_mixed_plan(
    c: &ProjectController,
    processor: ProcessorId,
    target: ParameterTargetId,
    eng: &mut RoutingEngine,
    control: &mut RoutingControlDispatch,
    midi: &[(i64, MidiEvent)],
) -> std::sync::Arc<ruviform_media_core::TransportPlan<EventKind>> {
    control.stage_midi_plan(c.live(), processor, midi).unwrap();
    let revision = c.live_revision();
    let unbound = fast_write(target, 1_200, 0.25, revision, None);
    let bound = fast_write(
        target,
        1_280,
        0.50,
        revision,
        Some(TransportGeneration::new(1)),
    );
    assert!(matches!(
        control.fast_control(c.live(), unbound, None),
        FastControlOutcome::Queued { .. }
    ));
    assert!(matches!(
        control.fast_control(c.live(), bound, None),
        FastControlOutcome::Queued { .. }
    ));

    eng.run(1, synvelis_player::driver::PumpPolicy::Every(1))
        .unwrap();
    let tick = control.tick(
        c.live(),
        BeatPoint::new(0, 480).unwrap(),
        MediaTime::new(0, frame_base()),
        active_generation(eng),
    );
    assert!(!tick.gated, "active surface must drain queued writes");
    assert_eq!(tick.queue_delivered, 2);

    let before = control.last_plan(processor).unwrap().clone();
    assert_eq!(
        before
            .events
            .iter()
            .filter(|event| matches!(event.payload, EventKind::Midi(_)))
            .count(),
        2,
        "staged MIDI must be in the mixed plan"
    );
    let unbound_before = event_at(&before, 1_200);
    assert_eq!(unbound_before.transport_generation, None);
    let bound_before = event_at(&before, 1_280);
    assert_eq!(
        bound_before.transport_generation,
        Some(TransportGeneration::new(1))
    );
    before
}

/// The representative mixed-plan oracle the earlier MIDI-only refresh test
/// could not express. This is control-surface evidence, not a direct endpoint injection.
#[test]
fn routing_control_musical_refresh_preserves_representative_mixed_plan() {
    let (mut c, processor, target, mut eng, mut control) = mixed_refresh_control("mixed-refresh");
    let initial_midi = [(500, note_on(60)), (640, note_on(64))];
    let before = stage_mixed_plan(&c, processor, target, &mut eng, &mut control, &initial_midi);

    let pre_revision = c.live().tempo_map.revision;
    apply_project_op(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 4_800,
            bpm_milli: 75_000,
        },
    );
    assert_eq!(c.live().tempo_map.revision, pre_revision + 1);
    let sync = control.sync_musical_transport(c.live());
    assert!(sync.refresh_errors.is_empty());
    assert!(sync.refreshed.contains(&processor));

    let after = control.last_plan(processor).unwrap().clone();
    assert_eq!(control.last_plan_musical_is_current(processor), Some(true));
    assert_eq!(
        after
            .musical_ppqn
            .map(ruviform_media_core::Ppqn::ticks_per_quarter),
        Some(480)
    );
    assert_plan_events_preserved(&before, &after);
    assert_eq!(
        control.staged_midi(processor),
        Some(initial_midi.as_slice())
    );
    let report = control.report().unwrap();
    assert!(report.musical_refresh_errors.is_empty());
    assert!(
        matches!(
            report.musical_transport,
            synvelis_player::MusicalTransportEvidence::Available { tempo_revision, ppqn }
            if tempo_revision == c.live().tempo_map.revision && ppqn == 480
        ),
        "surface evidence must name the current complete source: {:?}",
        report.musical_transport
    );

    assert_restaged_midi_preserves_params(&mut control, c.live(), processor, &after);

    // Boundary fault injection (not a committed production edit): a cloned
    // project with mismatched PPQN must become typed unavailable evidence,
    // clear both musical halves, and never synthesize fixed 960.
    assert_invalid_ppqn_refresh_is_honestly_absent(&mut control, c.live(), processor);
}

const IDENTITY_EDIT_TICK: i64 = 9_600;
const SOURCE_A: u32 = 75_000;
const SOURCE_B: u32 = 110_000;

fn add_identity_tempo(c: &mut ProjectController, bpm_milli: u32) {
    apply_project_op(
        c,
        ProjectOp::AddTempoEvent {
            tick: IDENTITY_EDIT_TICK,
            bpm_milli,
        },
    );
}

fn project_source_evidence(c: &ProjectController) -> Value {
    json!({
        "project_revision": c.live_revision().0,
        "tempo_revision": c.live().tempo_map.revision,
        "tempo_content_fingerprint": c.live().tempo_map.tempo_events,
    })
}

fn expected_tempo_add(base: &TempoMapData, bpm_milli: u32) -> TempoMapData {
    let mut edited = base.clone();
    edited.revision += 1;
    edited.tempo_events.push(TempoEventDto {
        tick: IDENTITY_EDIT_TICK,
        bpm_milli,
    });
    edited
}

fn assert_same_revision_switch(
    a: &PlanMusicalTransport,
    b: &PlanMusicalTransport,
    midi: &[(i64, MidiEvent)],
) -> Value {
    assert_eq!(a.tempo_revision(), b.tempo_revision());
    assert!(!a.same_project_basis(b));

    // A baseline sync between A and B would hide a revision-only freshness key.
    let (mut dispatch, _) = dispatch_with_source();
    assert!(dispatch.set_musical_transport(Some(a.clone())));
    let stale = dispatch.dispatch_midi(midi).unwrap();
    assert!(a.plan_carries(&stale));
    assert!(dispatch.set_musical_transport(Some(b.clone())));
    assert!(!dispatch.last_plan_musical_is_current());
    assert!(!b.plan_carries(&stale));
    let fresh = dispatch.refresh_musical_plan().unwrap().unwrap();
    assert!(b.plan_carries(&fresh));
    assert!(!a.plan_carries(&fresh));
    assert!(dispatch.last_plan_musical_is_current());
    assert_plan_events_preserved(&stale, &fresh);
    json!({
        "same_revision": true,
        "same_project_basis": false,
        "stale_a_current_before_replacement": false,
        "exact_b_allocation_carried_after_replacement": true
    })
}

#[test]
fn routing_control_undo_redo_refresh_preserves_mixed_plan_and_exact_source() {
    let (mut c, processor, target, mut eng, mut control) = mixed_refresh_control("undo-redo");
    let midi = [(500, note_on(60)), (640, note_on(64))];
    let initial = stage_mixed_plan(&c, processor, target, &mut eng, &mut control, &midi);
    let baseline = c.live().tempo_map.clone();
    let source_a = expected_tempo_add(&baseline, SOURCE_A);
    let source_b = expected_tempo_add(&baseline, SOURCE_B);

    let mut previous_generation = initial.generation;
    let mut a_source = None;
    let mut rows = Vec::new();
    for (operation, expected) in [
        ("add", &source_a),
        ("undo", &baseline),
        ("redo", &source_a),
        ("undo", &baseline),
        ("divergent-add", &source_b),
    ] {
        let before = project_source_evidence(&c);
        let revision = c.live_revision();
        match operation {
            "add" => add_identity_tempo(&mut c, SOURCE_A),
            "undo" => assert!(c.undo().unwrap()),
            "redo" => assert!(c.redo().unwrap()),
            "divergent-add" => add_identity_tempo(&mut c, SOURCE_B),
            _ => unreachable!(),
        }
        assert_eq!(c.live_revision().0, revision.0 + 1);
        assert_eq!(&c.live().tempo_map, expected, "{operation}");

        let sync = control.sync_musical_transport(c.live());
        assert!(sync.refresh_errors.is_empty());
        assert_eq!(sync.refreshed, [processor]);
        let plan = control.last_plan(processor).unwrap();
        assert_ne!(plan.generation, previous_generation);
        assert_eq!(control.last_plan_musical_is_current(processor), Some(true));
        assert_eq!(control.staged_midi(processor), Some(midi.as_slice()));
        assert_plan_events_preserved(&initial, plan);
        let source = compile_project_musical_transport(c.live()).unwrap();
        assert_complete_musical(plan, &source);
        let ppqn = plan.musical_ppqn.unwrap();
        let probe = BeatPoint::new(IDENTITY_EDIT_TICK + 1, 480)
            .unwrap()
            .to_beat();
        let live_music = plan
            .tempo_map
            .as_ref()
            .unwrap()
            .time_info(probe, ppqn)
            .unwrap();
        let expected_music = expected.compile().unwrap().time_info(probe, ppqn).unwrap();
        assert_eq!(live_music, expected_music, "{operation}");
        previous_generation = plan.generation;

        let collision = match operation {
            "redo" => {
                a_source = Some(source);
                None
            }
            "divergent-add" => {
                let a = a_source.as_ref().unwrap();
                Some(assert_same_revision_switch(a, &source, &midi))
            }
            _ => None,
        };
        rows.push(json!({
            "operation": operation,
            "before": before,
            "after": project_source_evidence(&c),
            "plan_generation": plan.generation.get(),
            "plan_tempo_revision": plan.tempo_revision.get(),
            "plan_ppqn": ppqn.ticks_per_quarter(),
            "plan_bpm_milli_after_edit_tick": live_music.bpm_milli,
            "last_plan_musical_is_current": true,
            "refresh_errors": sync.refresh_errors,
            "same_revision_divergence": collision,
        }));
    }
    assert!(
        !c.redo().unwrap(),
        "divergent add must discard the redo history"
    );
    println!(
        "tim03-dynamic-tempo-undo-redo {}",
        json!({
            "schema": "tim03-dynamic-tempo-undo-redo/v1",
            "row_family": "player_control_undo_redo_matrix",
            "test": "routing_control_undo_redo_refresh_preserves_mixed_plan_and_exact_source",
            "tested_sha": std::env::var("TIM03_VERIFIED_SHA").ok(),
            "notification_scope": "ProjectController + explicit RoutingControlDispatch sync, not ProjectSession notification",
            "sequence": rows,
            "events_preserved": initial.events.iter().map(|event| json!({
                "position": event.position.value(),
                "source_sequence": format!("{:?}", event.source_sequence),
                "payload": format!("{:?}", event.payload),
                "transport_generation": event.transport_generation.map(TransportGeneration::get),
            })).collect::<Vec<_>>(),
            "staged_midi_preserved": true,
            "verdict": "pass",
        })
    );
}
