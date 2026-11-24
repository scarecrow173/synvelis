//! TIM-02 clip→node delivery acceptance (issue #623, headless
//! slice; design `docs/designs/tim-02-midi-node-delivery.md`).
//!
//! Covers the `RoutingControlDispatch` MIDI seam that
//! `tim02_midi_playback.rs` left at the `HeadlessDriver` boundary:
//! `stage_midi_plan` installs compiled clip events onto the resolved
//! instrument node (strip-chain head), the staged set merges into
//! every param batch plan the node mints (single-live-plan
//! wholesale replace preserved — a param dispatch can never wipe
//! the clip content), `clear_midi_plan` drops it through an
//! explicit empty-plan install, `sync_midi_plans` reconciles
//! stage/clear, and the TIM-03 boundary path routes the same
//! install through `ProcessBoundarySession::dispatch_midi`.
//!
//! RT contract: every stage/clear call runs on this orchestration
//! thread — never inside a processed block. The no-alloc/no-block
//! inside-callback contract is the workspace `rt_` lane's evidence
//! (`mise run test-rt`), asserted here only by call-site structure.
//!
//! Headless fixtures only: no real OS MIDI backend, device, or
//! plugin acceptance — those stay `NotRun` (external gate).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::{BoundaryDeclaration, PluginHostProcessors};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{
    FastControlWrite, MidiStageError, RoutingControlDispatch, instrument_node_for_clip,
};
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ControlOwner, CounterIdGenerator,
    LaneId, MediaDuration, MediaTimeValue, ParamMapping, ParameterRole, ParameterTargetId,
    PlaybackMapping, ProcessorBinding, ProcessorId, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef,
    RuntimeInstanceId, SourceFrame, SourceId, SourceRange, StripId, TargetCapability, TimeAnchor,
    TrackId, ValueKind,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_core::{
    InBlockTransportSupport, MediaTime, MidiChannel, MidiEvent, MidiGroup, MidiKind, MidiValue,
    NoteKey, ScheduleBudget, TimeBase,
};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamFlags, ParamInfo, ParamWriteMeans, Plugin, PluginDescriptor, PluginFactory,
    PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo,
    erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;
use synvelis_player::process_boundary::TimingPrecision;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-tim02nd-{}-{}-{}",
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
        Project::new(ProjectId::new(0x623), "tim02nd"),
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

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

fn request(id: u64, revision: ProjectRevision) -> RoutingCompileRequest {
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

fn new_engine() -> RoutingEngine {
    RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1))
}

/// The RT-observed active generation as the control gate consumes it:
/// `0` means "never observed" → `None` (always gated).
fn active_generation(eng: &RoutingEngine) -> Option<u64> {
    let generation = eng.rt_status().active_generation;
    (generation != 0).then_some(generation)
}

fn import_stereo(
    c: &mut ProjectController,
    root: &Path,
    name: &str,
    frames: usize,
) -> synvelis_player::ImportOutcome {
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

fn add_track_strip(c: &mut ProjectController) -> (TrackId, LaneId, StripId) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
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

fn insert_processor(
    c: &mut ProjectController,
    strip: StripId,
    index: usize,
    name: &str,
    descriptor: &str,
) -> ProcessorId {
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index,
            name: name.into(),
            binding: ProcessorBinding::Resolved(synvelis_project::PluginRef {
                descriptor: descriptor.into(),
                version: "0.1.0".into(),
                capability_revision: 0,
            }),
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn prepared_clips(project: &Project, media_root: &Path) -> Vec<synvelis_player::PreparedClip> {
    project
        .clips
        .values()
        .map(|clip| {
            synvelis_player::resolver::prepare(project, clip.source_id, media_root, &prefetch())
                .unwrap()
        })
        .collect()
}

fn deps_default<'a>(project: &Project, media_root: &Path) -> RoutingDeps<'a> {
    RoutingDeps {
        canonical_of: &|_| Some(canonical_format()),
        clips: prepared_clips(project, media_root),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
}

/// Minimal project: one stereo import, one track strip, one clip,
/// one chain-head processor bound to the MIDI recorder plugin.
fn plugin_project(root: &Path) -> (ProjectController, PathBuf, ClipId, ProcessorId) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let (_, lane, strip) = add_track_strip(&mut c);
    let clip = add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let processor = insert_processor(&mut c, strip, 0, "inst", MIDI_DESCRIPTOR.id);
    (c, root.join("media"), clip, processor)
}

fn publish_first(c: &ProjectController, media: &Path, eng: &mut RoutingEngine) -> u64 {
    let p = eng
        .prepare(
            &request(1, c.live_revision()),
            c.live(),
            c.live_revision(),
            deps_default(c.live(), media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p, c.live(), c.live_revision()).unwrap();
    eng.source_map().map(|m| m.generation).unwrap()
}

// ---------------------------------------------------------------- plugins

const STEREO_BUSES: [BusDescriptor; 2] = [
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

const MIDI_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim02nd-midi",
    name: "TIM-02ND MIDI Recorder",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const MIDI_PARAMS: [ParamInfo; 1] = [ParamInfo {
    id: ruviform_plugin_core::ParamId(3),
    name: "Cut",
    unit: "",
    min_value: 0.0,
    max_value: 1.0,
    default_value: 0.0,
    flags: ruviform_plugin_core::ParamFlags::AUTOMATABLE,
}];

/// One delivered event at its position-domain frame — the merged
/// plan's `Midi` and `ParamValue` payloads share this log so tests
/// prove both kinds ride the same install.
#[derive(Debug, Clone, PartialEq)]
enum Rec {
    Midi {
        abs_frame: i64,
        event: MidiEvent,
    },
    Param {
        abs_frame: i64,
        param_id: u32,
        value: f64,
    },
}

struct MidiRecPlugin {
    delivered: Arc<Mutex<Vec<Rec>>>,
}

impl PluginProcessor for MidiRecPlugin {
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
        let mut log = self.delivered.lock().unwrap();
        for ev in events.iter() {
            let abs_frame = pos + i64::from(ev.frame_offset());
            match ev.kind() {
                EventKind::Midi(event) => log.push(Rec::Midi { abs_frame, event }),
                EventKind::ParamValue(param) => log.push(Rec::Param {
                    abs_frame,
                    param_id: param.param_id.0,
                    value: param.value.get(),
                }),
                _ => {}
            }
        }
        ProcessResult::CONTINUE
    }
}

impl Plugin for MidiRecPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        MIDI_DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &MIDI_PARAMS
    }
    fn buses(&self) -> &[BusDescriptor] {
        &STEREO_BUSES
    }
}

type InstanceLogs = Arc<Mutex<Vec<Arc<Mutex<Vec<Rec>>>>>>;

struct MidiFactory {
    instances: InstanceLogs,
}

impl PluginFactory for MidiFactory {
    fn descriptor(&self) -> PluginDescriptor {
        MIDI_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        erase(MidiRecPlugin { delivered })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        Some(erase_sendable(MidiRecPlugin { delivered }))
    }
}

fn midi_host() -> (PluginHostProcessors, InstanceLogs) {
    let instances = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(MidiFactory {
        instances: Arc::clone(&instances),
    }));
    (PluginHostProcessors::new(host), instances)
}

/// Adds a `ProcessorParam` target on `processor` declaring plugin
/// param `param_id`. Returns the issued target id (`ctl_02` pattern).
fn add_plugin_param(
    c: &mut ProjectController,
    processor: ProcessorId,
    name: &str,
    param_id: u32,
) -> ParameterTargetId {
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: synvelis_project::AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id,
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
        .find(|t| t.name == name)
        .unwrap()
        .id
}

/// The caller-vended catalog the surface binds against — the
/// recorder plugin's live `ParamInfo` enumeration.
fn catalog(processor: ProcessorId) -> synvelis_player::ParamCatalog {
    BTreeMap::from([(processor, MIDI_PARAMS.to_vec())])
}

fn fast_write(
    target: ParameterTargetId,
    frame: i64,
    value: f64,
    revision: ProjectRevision,
) -> FastControlWrite {
    FastControlWrite {
        target,
        value,
        means: ParamWriteMeans::TimedEvent,
        position: MediaTime::new(frame, frame_base()),
        project_revision: revision,
    }
}

fn note_on(ch: u8, key: u8, vel: u8) -> MidiEvent {
    MidiEvent {
        group: MidiGroup::ZERO,
        channel: MidiChannel::new(ch).unwrap(),
        kind: MidiKind::NoteOn {
            key: NoteKey::new(key).unwrap(),
            velocity: MidiValue::from_u7(vel),
        },
    }
}

fn note_off(ch: u8, key: u8) -> MidiEvent {
    MidiEvent {
        group: MidiGroup::ZERO,
        channel: MidiChannel::new(ch).unwrap(),
        kind: MidiKind::NoteOff {
            key: NoteKey::new(key).unwrap(),
            velocity: 0,
        },
    }
}

fn delivered_midi(instances: &InstanceLogs, index: usize) -> Vec<Rec> {
    instances.lock().unwrap()[index].lock().unwrap().clone()
}

// ---------------------------------------------------------------- tests

/// AC1: `instrument_node_for_clip` resolves the chain-head
/// processor and `stage_midi_plan` installs the compiled events —
/// the plugin's `EventList` receives `EventKind::Midi` at the
/// plan's absolute frames.
#[test]
fn clip_events_deliver_to_chain_head_node() {
    let root = scratch("deliver");
    let (c, media, clip, processor) = plugin_project(&root);
    assert_eq!(
        instrument_node_for_clip(c.live(), clip),
        Some(processor),
        "clip must resolve to the chain-head instrument"
    );
    let (adapter, instances) = midi_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));
    let generation = publish_first(&c, &media, &mut eng);
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, generation, c.live(), &BTreeMap::new()).unwrap();
    let events = vec![(100, note_on(0, 60, 100)), (200, note_off(0, 60))];
    let plan = dispatch
        .stage_midi_plan(c.live(), processor, &events)
        .unwrap();
    assert!(
        plan.events
            .iter()
            .all(|e| matches!(e.payload, EventKind::Midi(_))),
        "a MIDI-only stage mints a MIDI-only plan"
    );
    assert_eq!(dispatch.staged_midi(processor), Some(events.as_slice()));

    eng.run(2, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let delivered = delivered_midi(&instances, 0);
    assert_eq!(
        delivered,
        vec![
            Rec::Midi {
                abs_frame: 100,
                event: note_on(0, 60, 100)
            },
            Rec::Midi {
                abs_frame: 200,
                event: note_off(0, 60)
            },
        ],
        "compiled clip events must reach the instrument node's EventList"
    );
}

/// AC3 merge semantics: a param batch dispatch re-mints the node's
/// live plan — without the staged-MIDI merge the param install
/// would wipe the clip content wholesale. Both the param event and
/// the late-frame MIDI must deliver through the merged plan.
#[test]
fn merged_plan_keeps_staged_midi_across_param_dispatches() {
    let root = scratch("merged");
    let (mut c, media, _clip, processor) = plugin_project(&root);
    let target = add_plugin_param(&mut c, processor, "cut", 3);
    let (adapter, instances) = midi_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));
    let generation = publish_first(&c, &media, &mut eng);
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    let rev = c.live_revision();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, generation, c.live(), &catalog(processor))
            .unwrap();
    // Stage clip content entirely past the param event's frame —
    // if the param install replaced the plan wholesale, these
    // events would never deliver.
    dispatch
        .stage_midi_plan(
            c.live(),
            processor,
            &[(500, note_on(0, 64, 90)), (640, note_off(0, 64))],
        )
        .unwrap();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 120, 0.25, rev),
        active_generation(&eng),
    );
    assert_eq!(
        outcome,
        synvelis_player::FastControlOutcome::Delivered,
        "param write must deliver on the merged node"
    );

    eng.run(3, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let delivered = delivered_midi(&instances, 0);
    assert!(
        delivered.contains(&Rec::Param {
            abs_frame: 120,
            param_id: 3,
            value: 0.25
        }),
        "param event must deliver: {delivered:?}"
    );
    assert!(
        delivered.contains(&Rec::Midi {
            abs_frame: 500,
            event: note_on(0, 64, 90)
        }),
        "staged MIDI must survive the param install: {delivered:?}"
    );
    assert!(
        delivered.contains(&Rec::Midi {
            abs_frame: 640,
            event: note_off(0, 64)
        }),
        "staged note-off must survive too: {delivered:?}"
    );
}

/// `clear_midi_plan` drops the staged set through an explicit
/// empty-plan install — staged content beyond the run window can
/// never deliver, and the accessor reads empty.
#[test]
fn clear_midi_plan_silences_staged_content() {
    let root = scratch("clear");
    let (c, media, _clip, processor) = plugin_project(&root);
    let (adapter, instances) = midi_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));
    let generation = publish_first(&c, &media, &mut eng);
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, generation, c.live(), &BTreeMap::new()).unwrap();
    dispatch
        .stage_midi_plan(c.live(), processor, &[(600, note_on(0, 60, 100))])
        .unwrap();
    let empty = dispatch.clear_midi_plan(c.live(), processor).unwrap();
    assert!(empty.events.is_empty(), "clear mints an empty plan");
    assert_eq!(dispatch.staged_midi(processor), Some(&[][..]));

    eng.run(3, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    assert!(
        delivered_midi(&instances, 0).is_empty(),
        "cleared content must never deliver"
    );
}

/// `sync_midi_plans` reconciles stage/clear in one call — listed
/// nodes stage, previously-staged unlisted nodes clear.
#[test]
fn sync_midi_plans_reconciles_stage_and_clear() {
    let root = scratch("reconcile");
    let (c, media, _clip, processor) = plugin_project(&root);
    let (adapter, _instances) = midi_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));
    let generation = publish_first(&c, &media, &mut eng);
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, generation, c.live(), &BTreeMap::new()).unwrap();
    let events = vec![(100, note_on(0, 60, 100))];
    let report = dispatch.sync_midi_plans(c.live(), &BTreeMap::from([(processor, events.clone())]));
    assert_eq!(report.staged, vec![processor]);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(dispatch.staged_midi(processor), Some(events.as_slice()));

    // Removing the clip between builds must not keep sounding —
    // an empty reconcile map clears the previously staged node.
    let report = dispatch.sync_midi_plans(c.live(), &BTreeMap::new());
    assert_eq!(report.cleared, vec![processor]);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(dispatch.staged_midi(processor), Some(&[][..]));
}

/// Resolution is strict head-of-chain: a second insert at index 1
/// never becomes the delivery target, and a clip on a processor-
/// less track resolves `None` (typed no-instrument evidence).
#[test]
fn instrument_resolution_is_strict_chain_head() {
    let root = scratch("resolve");
    let (mut c, media, clip, head) = plugin_project(&root);
    let strip = c
        .live()
        .routing
        .strip_of_track(*c.live().tracks.keys().next_back().unwrap())
        .unwrap()
        .id;
    let tail = insert_processor(&mut c, strip, 1, "fx", MIDI_DESCRIPTOR.id);
    assert_eq!(instrument_node_for_clip(c.live(), clip), Some(head));
    assert_ne!(instrument_node_for_clip(c.live(), clip), Some(tail));

    // A track with no chain processors resolves `None`.
    apply(
        &mut c,
        ProjectOp::AddTrack {
            name: "bare".into(),
        },
    );
    let bare = *c.live().tracks.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddTrackStrip { track_id: bare });
    apply(&mut c, ProjectOp::AddLane { track_id: bare });
    let bare_lane = *c.live().tracks[&bare].lane_ids.last().unwrap();
    let out = import_stereo(&mut c, &root, "b.wav", 480);
    let bare_clip = add_clip(&mut c, bare_lane, out.commit.source_id, 0, 480, 0);
    assert_eq!(instrument_node_for_clip(c.live(), bare_clip), None);
    let _ = media;
}

/// Generation swap: the new surface mints fresh node controls, so
/// previously staged MIDI is gone — re-staging on the new
/// generation delivers again.
#[test]
fn generation_swap_requires_restage() {
    let root = scratch("genswap");
    let (mut c, media, _clip, processor) = plugin_project(&root);
    let (adapter, instances) = midi_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));
    let gen1 = publish_first(&c, &media, &mut eng);
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, gen1, c.live(), &BTreeMap::new()).unwrap();
    dispatch
        .stage_midi_plan(c.live(), processor, &[(700, note_on(0, 60, 100))])
        .unwrap();

    // Force a second generation: any project mutation that rebuilds
    // the routing plan.
    apply(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "x".into(),
            owner: synvelis_project::ControlOwner::Processor(processor),
            role: synvelis_project::ParameterRole::ProcessorParam,
            time_scope: synvelis_project::AutomationTimeScope::ProjectMusical,
            kind: synvelis_project::ValueKind::Continuous,
            mapping: synvelis_project::ParamMapping::Linear,
            capability: None,
            base: 0.0,
        },
    );
    let p2 = eng
        .prepare(
            &request(2, c.live_revision()),
            c.live(),
            c.live_revision(),
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p2, c.live(), c.live_revision()).unwrap();
    let gen2 = eng.source_map().map(|m| m.generation).unwrap();
    assert_ne!(gen1, gen2, "second publish must mint a new generation");
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    dispatch
        .sync(&mut eng, gen2, c.live(), &BTreeMap::new())
        .unwrap();
    assert_eq!(
        dispatch.staged_midi(processor),
        Some(&[][..]),
        "a surface swap mints fresh node controls — staged content is gone"
    );

    dispatch
        .stage_midi_plan(c.live(), processor, &[(80, note_on(0, 62, 90))])
        .unwrap();
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let delivered = delivered_midi(&instances, 1);
    assert!(
        delivered.contains(&Rec::Midi {
            abs_frame: 80,
            event: note_on(0, 62, 90)
        }),
        "re-staged content must deliver on the new generation: {delivered:?}"
    );
}

/// TIM-03 consistency: a boundary-declared processor routes the
/// install through `ProcessBoundarySession::dispatch_midi` — the
/// same generation-tagged plan, provenance-gated.
#[test]
fn boundary_declared_node_stages_through_session_gate() {
    let root = scratch("boundary");
    let (c, media, _clip, processor) = plugin_project(&root);
    let (mut adapter, instances) = midi_host();
    adapter.register_boundary(
        processor,
        BoundaryDeclaration {
            precision: TimingPrecision::FrameAccurateEvents,
            in_block: InBlockTransportSupport::Unknown,
            consent: None,
            budget: ScheduleBudget::new(64, 256, 16, 4096),
        },
    );
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));
    let generation = publish_first(&c, &media, &mut eng);
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, generation, c.live(), &BTreeMap::new()).unwrap();
    assert!(
        dispatch.boundary_session(processor).is_some(),
        "declared processor must carry a boundary session"
    );
    dispatch
        .stage_midi_plan(c.live(), processor, &[(120, note_on(0, 65, 80))])
        .unwrap();

    eng.run(2, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let delivered = delivered_midi(&instances, 0);
    assert!(
        delivered.contains(&Rec::Midi {
            abs_frame: 120,
            event: note_on(0, 65, 80)
        }),
        "boundary-gated staging must deliver: {delivered:?}"
    );
}

/// Fail-closed staging: a processor with no vended node and an
/// empty stage are typed errors — never silent skips.
#[test]
fn staging_failures_are_typed() {
    let root = scratch("typed");
    let (c, media, _clip, processor) = plugin_project(&root);
    let (adapter, _instances) = midi_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));
    let generation = publish_first(&c, &media, &mut eng);
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, generation, c.live(), &BTreeMap::new()).unwrap();
    // A foreign processor id has no node on the surface.
    let foreign = ProcessorId::new(0xffff);
    let err = dispatch
        .stage_midi_plan(c.live(), foreign, &[(0, note_on(0, 60, 100))])
        .unwrap_err();
    assert!(
        matches!(err, MidiStageError::NoNode(p) if p == foreign),
        "{err:?}"
    );

    // An empty stage is refused — clearing is the explicit API.
    let err = dispatch
        .stage_midi_plan(c.live(), processor, &[])
        .unwrap_err();
    assert!(
        matches!(
            err,
            MidiStageError::Boundary(_) | MidiStageError::Dispatch(_)
        ),
        "{err:?}"
    );
}
