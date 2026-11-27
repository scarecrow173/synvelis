//! TIM-03 per-request plan wiring + precision provenance — Issue
//! #127 wiring slice (headless mock; spec:
//! `docs/specs/tim03-process-boundary.md`).
//!
//! `ProcessBoundarySession` is the runtime half of the TIM-03 oracle:
//! every stage evaluates the upcoming window *before* touching the
//! node's control channel and re-checks [`PrecisionProvenance`] pins
//! (plugin instance / node / capability generation / graph
//! generation) plus the verified attach clock context. These tests
//! drive the real `PluginNode` through `GraphRuntime` and assert:
//!
//! - `tim03_req_*` — the boundary matrix through the wired session:
//!   100/128/256-frame windows, boundaries at 0/1/100/last, min/max
//!   process-length legality, simultaneous Loop/Seek/Tempo/Param at
//!   one boundary — every recorded physical call satisfies
//!   `min_frames <= frames <= max_frames` (no zero-length calls).
//! - `tim03_cap_*` — the three capability branches as independent
//!   cases: typed `Unsupported`, realtime degraded requiring explicit
//!   `DegradedConsent`, offline exact failing closed.
//! - `tim03_prov_*` — precision provenance: instance/node/capability/
//!   graph generation drift and clock-mapping mismatch are typed
//!   refusals *before* staging, and stale context delivers zero param
//!   events (the `dispatch_midi` ledger/generation pattern on top).
//!
//! Real CLAP/VST3/OS/backend×rate×block validation is `NotRun` — a
//! separate gate (Ruviform #196/#253 owners).

use std::sync::{Arc, Mutex};

use ruviform_audio::typed::{Compiler, CompilerConfig, DEFAULT_ALIGNMENT, GraphRuntime, RtRuntime};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{
    AttachConfig, NodeReport, PluginNode, PluginNodeControl, PluginNodeFactory, attach,
};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, PortId, PortRef,
    Validator,
};
use ruviform_media_core::{
    ApplyDegradation, BlockContract, DeliveryPolicy, InBlockTransportSupport, MediaDuration,
    MediaRange, MediaTime, MidiChannel, MidiEvent, MidiGroup, MidiKind, NoteKey, PPQN,
    PlanGeneration, Ppqn, ScheduleBudget, TempoEvent, TempoMapBuilder, TempoMapRevision, Tick,
    TickPosition, TimeBase, TransportCommand, TransportPlan, TransportPlanBuilder, TransportStatus,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TimedParamEvent, TransportInfo, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::{
    BoundaryKind, BoundaryPoint, BoundarySessionError, DegradedConsent, ExecutionMode,
    ParamDispatch, PrecisionProvenance, ProcessBoundaryError, ProcessBoundarySession,
    ProcessCapability, StablePluginInstanceId, TimingPrecision,
};
use synvelis_project::ParameterTargetId;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim03-req",
    name: "TIM-03 Request Recorder",
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

/// Two automatable params so bound/absolute events stay distinguishable.
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
        id: ParamId(9),
        name: "Pan",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE,
    },
];

// ---------------------------------------------------------------------
// Recording mock plugin — fixed-capacity log, no allocation inside
// `process` (same shape as tim03_process_boundary.rs).
// ---------------------------------------------------------------------

const MAX_CALLS: usize = 256;
const MAX_PARAMS_PER_CALL: usize = 16;

/// One parameter hit inside a physical call.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct ParamHit {
    frame_offset: u32,
    position: i64,
    abs: i64,
    param_id: u32,
    value: f64,
}

/// One physical `process` call as the plugin observed it.
#[derive(Clone, Copy, Default, Debug)]
struct CallHit {
    frames: u32,
    /// Transport position (media ticks) at call start.
    position: i64,
    /// `TransportStatus::Playing` at call start.
    playing: bool,
    /// Plan-owned musical transport visible at call start, or zeroes.
    musical_bpm_milli: u32,
    musical_ppqn: u16,
    n_params: u8,
    params: [ParamHit; MAX_PARAMS_PER_CALL],
}

impl CallHit {
    fn param_hits(&self) -> &[ParamHit] {
        &self.params[..usize::from(self.n_params)]
    }
}

struct CallLog {
    len: usize,
    calls: Vec<CallHit>,
}

impl Default for CallLog {
    fn default() -> Self {
        Self {
            len: 0,
            calls: vec![CallHit::default(); MAX_CALLS],
        }
    }
}

impl CallLog {
    fn len(&self) -> usize {
        self.len
    }

    fn hit(&self, i: usize) -> CallHit {
        self.calls[i]
    }

    /// Every parameter hit across all recorded calls.
    fn all_params(&self) -> Vec<ParamHit> {
        (0..self.len)
            .flat_map(|i| self.calls[i].param_hits().to_vec())
            .collect()
    }
}

struct RecPlugin {
    log: Arc<Mutex<CallLog>>,
}

impl PluginProcessor for RecPlugin {
    fn prepare(
        &mut self,
        _config: &ProcessConfig,
        _context: &mut ProcessContext,
    ) -> Result<(), PrepareError> {
        Ok(())
    }

    fn process<I, O>(
        &mut self,
        inputs: &[I],
        _outputs: &mut [O],
        events: EventList<'_>,
        transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        let frames = inputs.first().map_or(0, BufferView::frames);
        let position = transport.map_or(0, |t| t.position().value());
        let playing = transport.is_some_and(|t| t.status() == TransportStatus::Playing);
        let musical = transport.and_then(TransportInfo::musical);
        let mut log = self.log.lock().unwrap();
        if log.len < MAX_CALLS {
            let mut hit = CallHit {
                frames: u32::try_from(frames).unwrap_or(u32::MAX),
                position,
                playing,
                musical_bpm_milli: musical.map_or(0, |m| m.bpm_milli),
                musical_ppqn: musical.map_or(0, |m| m.ppqn),
                n_params: 0,
                params: [ParamHit::default(); MAX_PARAMS_PER_CALL],
            };
            for ev in events.iter() {
                if let EventKind::ParamValue(pe) = ev.kind()
                    && usize::from(hit.n_params) < MAX_PARAMS_PER_CALL
                {
                    hit.params[usize::from(hit.n_params)] = ParamHit {
                        frame_offset: ev.frame_offset(),
                        position,
                        abs: position + i64::from(ev.frame_offset()),
                        param_id: pe.param_id.0,
                        value: pe.value.get(),
                    };
                    hit.n_params += 1;
                }
            }
            let slot = log.len;
            log.calls[slot] = hit;
            log.len = slot + 1;
        }
        ProcessResult::CONTINUE
    }
}

impl Plugin for RecPlugin {
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

struct RecFactory {
    log: Arc<Mutex<CallLog>>,
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        ruviform_plugin_core::erase(RecPlugin {
            log: Arc::clone(&self.log),
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
            log: Arc::clone(&self.log),
        }))
    }
}

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

/// A hosted fixture plugin + its `attached` parts.
struct Hosted {
    _host: PluginHost,
    attached: ruviform_audio_plugin_bridge::Attached,
    log: Arc<Mutex<CallLog>>,
}

/// Attaches the recorder with `ProcessConfig(48k, min_frames, max_frames)`,
/// the given delivery `policy` and `budget.max_deferral` — the same
/// attach inputs `PluginHostProcessors::connect` projects from a
/// `BoundaryDeclaration`.
fn host_plugin(min_frames: u32, max_frames: u32, policy: DeliveryPolicy, deferral: u32) -> Hosted {
    let log = Arc::new(Mutex::new(CallLog::default()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        log: Arc::clone(&log),
    }));
    let HostReply::Instantiated { handle } = host
        .execute(HostCommand::Instantiate {
            plugin_id: DESCRIPTOR.id.to_owned(),
        })
        .unwrap()
    else {
        panic!("Instantiate")
    };
    host.execute(HostCommand::Initialize { handle }).unwrap();
    let process_config =
        ProcessConfig::new(SampleRate::new(48_000).unwrap(), min_frames, max_frames).unwrap();
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
            policy,
            budget: ScheduleBudget::new(64, 256, 16, deferral),
            input: None,
        },
    )
    .expect("attach");
    Hosted {
        _host: host,
        attached,
        log,
    }
}

/// `Source → plugin → Sink` — same shape as the TIM-03 boundary suite.
fn build_graph(
    attached: &ruviform_audio_plugin_bridge::Attached,
) -> (ruviform_graph::ValidatedGraph, NodeId, [PortId; 2]) {
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
    let (proc, pp) = b.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: attached.ports.clone(),
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
    b.connect(&conn(pref(src, sp[0]), pref(proc, pp[0])));
    b.connect(&conn(pref(proc, pp[1]), pref(sink, kp[0])));
    let g = Validator::validate(&b.build()).expect("graph validates");
    (g, proc, [sp[0], sp[0]])
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

fn compiler_config() -> CompilerConfig {
    CompilerConfig {
        max_frames: 512,
        alignment: DEFAULT_ALIGNMENT,
        max_slots: 64,
        max_channels: 8,
        max_delay_lines: 16,
        max_delay_frames: 4096,
        max_processors: 16,
    }
}

/// Compiled+published graph drive: `process(frames)` feeds silence and
/// runs one callback of `frames`.
struct Drive {
    rt: RtRuntime,
    ctx: RtContext,
    src: PortId,
    zeros: Vec<f32>,
}

impl Drive {
    fn new(
        graph: &ruviform_graph::ValidatedGraph,
        node: PluginNode,
        proc: NodeId,
        srcs: [PortId; 2],
    ) -> Self {
        let mut factory = PluginNodeFactory::new();
        factory.insert(proc, node);
        let (mut runtime, rt) = GraphRuntime::new(&compiler_config(), 8, 8);
        let compiled = Compiler::new(compiler_config())
            .compile(
                graph,
                &mut factory,
                runtime.registry(),
                runtime.allocator(),
                runtime.generation() + 1,
            )
            .expect("compiles");
        runtime.publish(compiled).expect("publish");
        Self {
            rt,
            ctx: RtContext::mint(),
            src: srcs[0],
            zeros: vec![0.0; 512],
        }
    }

    fn process(&mut self, frames: usize) {
        let mut slot = self.rt.input_writer(self.src, frames).expect("input bound");
        for ch in 0..2 {
            slot.data.channel_mut(ch).unwrap()[..frames].copy_from_slice(&self.zeros[..frames]);
        }
        drop(slot);
        self.rt.process(frames, &self.ctx).expect("process");
    }
}

// ---------------------------------------------------------------------
// Session/provenance helpers
// ---------------------------------------------------------------------

fn cap(
    min: u32,
    max: u32,
    in_block: InBlockTransportSupport,
    precision: TimingPrecision,
) -> ProcessCapability {
    ProcessCapability::new(BlockContract::new(min, max, in_block), precision)
}

fn budget(deferral: u32) -> ScheduleBudget {
    ScheduleBudget::new(64, 256, 16, deferral)
}

fn prov(instance: u64, node: NodeId, cap_gen: u64, graph_gen: u64) -> PrecisionProvenance {
    PrecisionProvenance::new(
        StablePluginInstanceId::new(instance),
        node,
        cap_gen,
        graph_gen,
    )
}

/// Opens a session on `control` with the attach-consistent
/// mode/consent/budget the fixture attached with.
fn session(
    control: &PluginNodeControl,
    capability: ProcessCapability,
    mode: ExecutionMode,
    consent: Option<DegradedConsent>,
    deferral: u32,
    provenance: PrecisionProvenance,
) -> ProcessBoundarySession {
    ProcessBoundarySession::open(
        control,
        capability,
        mode,
        consent,
        budget(deferral),
        provenance,
    )
    .expect("session opens")
}

fn t(value: i64) -> MediaTime {
    MediaTime::new(value, frame_base())
}

fn loop_range(start: i64, end: i64) -> MediaRange {
    MediaRange::new(t(start), MediaDuration::new(end - start, frame_base()))
}

/// Asserts the zero-illegal-process-length invariant over every
/// recorded call: `0 < frames <= max_frames` and `frames >= min_frames`.
fn assert_legal_calls(log: &CallLog, min: u32, max: u32) {
    for i in 0..log.len() {
        let frames = log.hit(i).frames;
        assert!(frames > 0, "call {i}: zero-length process call");
        assert!(
            frames >= min && frames <= max,
            "call {i}: {frames} frames outside [{min}, {max}]"
        );
    }
}

/// The lowered host-resolved writes `dispatch_params` stages.
fn writes() -> Vec<(ParameterTargetId, TimedParamEvent)> {
    let target = ParameterTargetId::new(1);
    [(64_i64, 0.25_f64), (128, 0.5), (192, 0.75)]
        .into_iter()
        .map(|(pos, v)| {
            (
                target,
                TimedParamEvent {
                    position: t(pos),
                    event: ParamEvent {
                        param_id: ParamId(7),
                        value: ParamValue::new(v).unwrap(),
                    },
                },
            )
        })
        .collect()
}

fn midi_events() -> Vec<(i64, MidiEvent)> {
    vec![(
        64,
        MidiEvent {
            group: MidiGroup::ZERO,
            channel: MidiChannel::new(0).unwrap(),
            kind: MidiKind::NoteOn {
                key: NoteKey::new(60).unwrap(),
                velocity: 100,
            },
        },
    )]
}

// ---------------------------------------------------------------------
// tim03_req_* — boundary matrix through the wired session
// ---------------------------------------------------------------------

/// 100/128/256-frame windows × valid boundary offsets 0/1/100/last
/// through `stage_window`: every cell uses a fresh node/control/session,
/// evaluates Exact, and records the exact physical process-length
/// sequence. Window=100/offset=100 is out-of-window and is covered as a
/// typed negative by the policy matrix rather than being silently clamped.
#[test]
fn tim03_req_exact_matrix_windows_and_offsets() {
    let mut rows = Vec::new();
    for window in [100_u32, 128, 256] {
        for offset in [0_u32, 1, 100, window - 1] {
            if offset >= window {
                continue;
            }
            let hosted = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
            let (graph, proc, srcs) = build_graph(&hosted.attached);
            let node = hosted.attached.node;
            let mut control = hosted.attached.control;
            let mut drive = Drive::new(&graph, node, proc, srcs);

            let c = cap(
                1,
                512,
                InBlockTransportSupport::Unknown,
                TimingPrecision::FrameAccurateEvents,
            );
            let s = session(
                &control,
                c,
                ExecutionMode::Realtime,
                None,
                4096,
                prov(1, proc, 10, 20),
            );
            let current = prov(1, proc, 10, 20);
            let plan = s
                .stage_window(
                    &mut control,
                    &current,
                    window,
                    vec![],
                    &[
                        (TransportCommand::Play, 0),
                        (TransportCommand::Seek(t(500)), u64::from(offset)),
                    ],
                )
                .unwrap_or_else(|e| panic!("window={window} offset={offset}: {e}"));
            assert_eq!(plan.policy, DeliveryPolicy::Exact);
            drive.process(usize::try_from(window).unwrap());
            let log = hosted.log.lock().unwrap();
            let lengths: Vec<u32> = (0..log.len()).map(|i| log.hit(i).frames).collect();
            let expected = if offset == 0 {
                vec![window]
            } else {
                vec![offset, window - offset]
            };
            assert_eq!(lengths, expected, "window={window} offset={offset}");
            assert_legal_calls(&log, 1, 512);
            rows.push(serde_json::json!({
                "window_frames": window,
                "requested_boundary": offset,
                "exact_process_lengths": lengths,
                "boundary_kinds": ["Seek"],
                "combined_case": false,
                "param_positions": [],
                "fresh_instance": true,
                "queue_residue_dependency": false,
                "verdict": "pass"
            }));
        }
    }
    println!(
        "tim03-wired-session-matrix {}",
        serde_json::json!({
            "schema":"tim03-frame-boundary-matrix/wired-session/v1",
            "invalid_window100_boundary100":"typed-invalid-covered-by-policy-matrix; never clamped",
            "rows": rows,
        })
    );
}

/// Min/max process length is enforced at the wired boundary: a
/// 256-frame window against `min_frames = 300` has no legal partition
/// — realtime without consent refuses typed, offline exact refuses
/// typed, and *nothing* is staged.
#[test]
fn tim03_req_min_max_process_length_fail_closed() {
    let hosted = host_plugin(300, 512, DeliveryPolicy::Exact, 4096);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    let c = cap(
        300,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let current = prov(1, proc, 10, 20);

    // Realtime, no consent → ConsentRequired.
    let s = session(&control, c, ExecutionMode::Realtime, None, 4096, current);
    assert!(matches!(
        s.stage_window(
            &mut control,
            &current,
            256,
            vec![],
            &[(TransportCommand::Play, 0)],
        ),
        Err(BoundarySessionError::Evaluate(
            ProcessBoundaryError::ConsentRequired { .. }
        ))
    ));

    // Offline exact, certified precision → OfflineExact (the window
    // itself is inexact — no boundary can defer a too-short window).
    let s_off = session(
        &control,
        c,
        ExecutionMode::OfflineExact,
        None,
        4096,
        current,
    );
    assert!(matches!(
        s_off.stage_window(
            &mut control,
            &current,
            256,
            vec![],
            &[(TransportCommand::Play, 0)],
        ),
        Err(BoundarySessionError::Evaluate(
            ProcessBoundaryError::OfflineExact { .. }
        ))
    ));

    // Nothing was staged: Play never reached the node, so transport
    // stays stopped — and the 256-frame window is itself below
    // `min_frames = 300`, an undeliverable span (silence, no calls).
    drive.process(256);
    let log = hosted.log.lock().unwrap();
    assert_eq!(log.len(), 0, "refused staging must deliver nothing");
    assert_eq!(log.all_params(), []);
}

/// Loop + Seek + plan install (with param event and tempo map)
/// converging at each valid W×B matrix boundary — the wired
/// session stages all of it, the node coalesces to one boundary and
/// exact process lengths and the param event frame offset are fixed.
fn simultaneous_tempo_plan(
    control: &mut PluginNodeControl,
    param_pos: i64,
) -> Arc<TransportPlan<EventKind>> {
    let mut tempo = TempoMapBuilder::new();
    tempo.add_tempo_event(TempoEvent::at_tick(
        TickPosition::new(Tick::new(0), PPQN),
        120_000,
    ));
    let tempo_map = Arc::new(tempo.build().expect("tempo map"));
    let mut b = control
        .plan_builder(PlanGeneration::new(7))
        .with_musical_transport(tempo_map, TempoMapRevision::new(1), Ppqn::new(480).unwrap());
    for (sequence, param_id, value) in [(3, 7, 0.75), (1, 9, 0.2), (2, 7, 0.25)] {
        b.event(
            t(param_pos),
            sequence,
            EventKind::ParamValue(ParamEvent {
                param_id: ParamId(param_id),
                value: ParamValue::new(value).unwrap(),
            }),
        )
        .unwrap();
    }
    Arc::new(b.build())
}
fn boundary_offsets(boundary: u32) -> Vec<u32> {
    if boundary == 0 {
        Vec::new()
    } else {
        vec![boundary]
    }
}

fn split_lengths(window: u32, boundary: u32) -> Vec<u32> {
    if boundary == 0 {
        vec![window]
    } else {
        vec![boundary, window - boundary]
    }
}

struct CombinedRowInput {
    window: u32,
    boundary: u32,
    loop_len: i64,
    initial_pos: i64,
    target_position: i64,
    musical_bpm_milli: u32,
    musical_ppqn: u16,
    frame_offset: u32,
    abs: i64,
    lengths: Vec<u32>,
}

fn combined_row(input: &CombinedRowInput) -> serde_json::Value {
    serde_json::json!({
        "window_frames": input.window,
        "requested_boundary": input.boundary,
        "boundary_kinds": ["LoopWrap","Seek","PlanInstall"],
        "scenario": "combined-loop-seek-tempo-plan-param",
        "combined_case": true,
        "simultaneous_command_order": ["Play@0","SetLoop@0","initial-Seek@0","Seek@boundary","PlanInstall@boundary"],
        "tie_rule": "same-frame apply wins over wrap; wrap re-evaluated in post-apply state",
        "loop_range_ticks": [0,input.loop_len],
        "initial_position_ticks": input.initial_pos,
        "seek_target_ticks": 307,
        "post_seek_loop_position": input.target_position,
        "exact_process_lengths": input.lengths,
        "param_positions": [{"position_ticks": input.abs, "post_seek_base_position": 7, "process_start_relative_frame_offset": input.frame_offset, "absolute_position": input.abs, "exactly_once": true}],
        "tempo_plan": {"tempo_map_revision": 1, "param_source_sequences": [3,1,2], "expected_delivery_order": [[9,0.2],[7,0.25],[7,0.75]], "same_param_last_wins": {"param_id":7,"final_value":0.75}, "musical_ppqn": 480, "observed_call_start_bpm_milli": input.musical_bpm_milli, "observed_call_start_ppqn": input.musical_ppqn, "tempo_is_not_transport_command": true},
        "frame_offset_meaning": "process-start-relative EventList.frame_offset",
        "fresh_instance": true,
        "verdict": "pass"
    })
}

#[test]
fn tim03_req_simultaneous_loop_seek_tempo_param() {
    let mut rows = Vec::new();
    for window in [100_u32, 128, 256] {
        for boundary in [0_u32, 1, 100, window - 1] {
            if boundary >= window {
                continue;
            }
            let hosted = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
            let (graph, proc, srcs) = build_graph(&hosted.attached);
            let node = hosted.attached.node;
            let mut control = hosted.attached.control;
            let mut drive = Drive::new(&graph, node, proc, srcs);
            let c = cap(
                1,
                512,
                InBlockTransportSupport::Unknown,
                TimingPrecision::FrameAccurateEvents,
            );
            let current = prov(1, proc, 10, 20);
            let s = session(&control, c, ExecutionMode::Realtime, None, 4096, current);

            // Head setup places playback `boundary` frames before the end of a
            // 300-tick loop, making the natural wrap converge with Seek/PlanInstall.
            let loop_len = 300_i64;
            let initial_pos = loop_len - i64::from(boundary);
            let commands = [
                (TransportCommand::Play, 0),
                (TransportCommand::SetLoop(loop_range(0, loop_len)), 0),
                (TransportCommand::Seek(t(initial_pos)), 0),
                (TransportCommand::Seek(t(307)), u64::from(boundary)),
            ];
            let plan = s
                .stage_window(&mut control, &current, window, vec![], &commands)
                .expect("simultaneous window plan");
            let expected_boundaries = boundary_offsets(boundary);
            assert_eq!(plan.boundaries, expected_boundaries);

            let remaining = window - boundary;
            let param_offset = i64::from(if remaining > 20 { 20 } else { remaining - 1 });
            let param_pos = 7 + param_offset;
            let install = simultaneous_tempo_plan(&mut control, param_pos);
            let evaluated = s
                .stage_plan(
                    &mut control,
                    &current,
                    window,
                    vec![],
                    install,
                    u64::from(boundary),
                )
                .expect("simultaneous plan install");
            assert_eq!(evaluated.boundaries, expected_boundaries);

            drive.process(usize::try_from(window).unwrap());
            let log = hosted.log.lock().unwrap();
            let lengths: Vec<u32> = (0..log.len()).map(|i| log.hit(i).frames).collect();
            let expected_lengths = split_lengths(window, boundary);
            assert_eq!(lengths, expected_lengths, "W{window}/B{boundary}");
            let target = log.hit(usize::from(boundary != 0));
            assert_eq!(
                target.position, 7,
                "post-seek loop collapse W{window}/B{boundary}"
            );
            assert_eq!(
                (target.musical_bpm_milli, target.musical_ppqn),
                (120_000, 480)
            );
            let params = target.param_hits();
            assert_eq!(
                params.len(),
                3,
                "param events exactly once W{window}/B{boundary}"
            );
            assert_eq!(params[0].frame_offset, u32::try_from(param_offset).unwrap());
            assert!(params.iter().all(|param| param.abs == param_pos));
            assert_eq!(
                params
                    .iter()
                    .map(|param| (param.param_id, param.value))
                    .collect::<Vec<_>>(),
                vec![(9, 0.2), (7, 0.25), (7, 0.75)]
            );
            rows.push(combined_row(&CombinedRowInput {
                window,
                boundary,
                loop_len,
                initial_pos,
                target_position: target.position,
                musical_bpm_milli: target.musical_bpm_milli,
                musical_ppqn: target.musical_ppqn,
                frame_offset: params[0].frame_offset,
                abs: params[0].abs,
                lengths,
            }));
        }
    }
    println!(
        "tim03-wired-session-matrix {}",
        serde_json::json!({
            "schema":"tim03-frame-boundary-matrix/wired-combined/v1",
            "rows": rows,
        })
    );
}
/// `stage_plan` legality-checks the install's own offset: a plan due
/// inside the window at an illegal boundary is refused before
/// `install_plan` runs — nothing reaches the node.
#[test]
fn tim03_req_plan_install_boundary_checked() {
    let hosted = host_plugin(128, 512, DeliveryPolicy::Exact, 4096);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    let c = cap(
        128,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let current = prov(1, proc, 10, 20);
    let s = session(&control, c, ExecutionMode::Realtime, None, 4096, current);

    // Install due at offset 100 inside a 256 window → segment
    // [0,100) is 100 < min_frames → illegal → ConsentRequired.
    let b = control.plan_builder(PlanGeneration::new(3));
    let install: Arc<TransportPlan<EventKind>> = Arc::new(b.build());
    assert!(matches!(
        s.stage_plan(&mut control, &current, 256, vec![], install, 100),
        Err(BoundarySessionError::Evaluate(
            ProcessBoundaryError::ConsentRequired { .. }
        ))
    ));

    // A plan built on a foreign frame base is refused by
    // `install_plan` itself — typed `InstallPlan`, nothing staged.
    let foreign = TransportPlanBuilder::<EventKind>::new(
        TimeBase::sample_rate(44_100).unwrap(),
        PlanGeneration::new(4),
    );
    let foreign_plan: Arc<TransportPlan<EventKind>> = Arc::new(foreign.build());
    assert!(matches!(
        s.stage_plan(&mut control, &current, 256, vec![], foreign_plan, 0),
        Err(BoundarySessionError::InstallPlan(_))
    ));

    // The window is legal so the node still processes (stopped
    // transport) — but no plan installed means zero param delivery
    // and transport never started.
    drive.process(256);
    let log = hosted.log.lock().unwrap();
    assert!(
        log.all_params().is_empty(),
        "refused installs deliver nothing"
    );
    assert!(
        (0..log.len()).all(|i| !log.hit(i).playing),
        "Play was never staged — transport stays stopped"
    );
}

// ---------------------------------------------------------------------
// tim03_cap_* — capability branches as independent cases
// ---------------------------------------------------------------------

/// Branch 1 — certified `Unsupported` capability: `open` refuses
/// typed before a session exists; no delivery path is reachable.
#[test]
fn tim03_cap_unsupported_typed_refusal() {
    let hosted = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
    let (_graph, proc, _srcs) = build_graph(&hosted.attached);
    let control = &hosted.attached.control;
    let c = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::Unsupported,
    );
    for mode in [ExecutionMode::Realtime, ExecutionMode::OfflineExact] {
        let err = ProcessBoundarySession::open(
            control,
            c,
            mode,
            None,
            budget(4096),
            prov(1, proc, 10, 20),
        )
        .expect_err("unsupported capability must refuse at open");
        assert!(
            matches!(
                err,
                BoundarySessionError::Capability(ProcessBoundaryError::Unsupported { .. })
            ),
            "mode={mode:?}: {err}"
        );
    }
}

/// Branch 2 — realtime degraded needs explicit consent: the same
/// illegal boundary refuses `ConsentRequired` without it and plans
/// `Degraded` with it — and the node's attached `Degraded` policy
/// defers identically (consent bound == attach budget bound).
#[test]
fn tim03_cap_realtime_degraded_requires_consent() {
    let hosted = host_plugin(128, 512, DeliveryPolicy::Degraded, 64);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    let c = cap(
        128,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let current = prov(1, proc, 10, 20);

    // Without consent the session still opens (attach was Degraded)
    // but the per-request plan refuses — consent is per-request
    // evidence, not attach state.
    let s = session(&control, c, ExecutionMode::Realtime, None, 64, current);
    assert!(matches!(
        s.stage_window(
            &mut control,
            &current,
            256,
            vec![],
            &[(TransportCommand::Seek(t(1000)), 100)],
        ),
        Err(BoundarySessionError::Evaluate(
            ProcessBoundaryError::ConsentRequired { .. }
        ))
    ));

    // With the same consent the attach used, the plan is Degraded and
    // predicts the deferral the scheduler then performs.
    let consented = ProcessBoundarySession::open(
        &control,
        c,
        ExecutionMode::Realtime,
        Some(DegradedConsent::grant(64)),
        budget(64),
        current,
    )
    .expect("consented session opens");
    let plan = consented
        .stage_window(
            &mut control,
            &current,
            256,
            vec![],
            &[
                (TransportCommand::Play, 0),
                (TransportCommand::Seek(t(1000)), 100),
            ],
        )
        .expect("consented degraded plan");
    assert_eq!(plan.policy, DeliveryPolicy::Degraded);
    assert_eq!(plan.degradations.len(), 1);
    assert_eq!(plan.degradations[0].requested, 100);
    assert_eq!(plan.degradations[0].applied, 128);

    drive.process(256);
    let log = hosted.log.lock().unwrap();
    assert_legal_calls(&log, 128, 512);
    // Deferred boundary at 128 → two legal calls; seek applied there.
    assert_eq!(log.len(), 2);
    assert_eq!(log.hit(0).frames, 128);
    assert_eq!(log.hit(1).frames, 128);
    assert_eq!(log.hit(1).position, 1000);
    let reports = control.drain_reports();
    assert!(reports.iter().any(|r| matches!(
        r,
        NodeReport::Receipt(rec)
            if matches!(rec.degradation, Some(ApplyDegradation::DeferredApply { .. }))
    )));
}

/// Branch 3 — offline exact fails closed: an illegal window is
/// `OfflineExact`, an uncertified capability is `OfflineUncertified`
/// at `open`, and a legal window on a certified capability plans
/// Exact.
#[test]
fn tim03_cap_offline_exact_fail_closed() {
    let hosted = host_plugin(128, 512, DeliveryPolicy::Exact, 4096);
    let (_graph, proc, _srcs) = build_graph(&hosted.attached);
    let control = &hosted.attached.control;
    let current = prov(1, proc, 10, 20);
    let c = cap(
        128,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );

    // Illegal window → OfflineExact at request time.
    let s = session(control, c, ExecutionMode::OfflineExact, None, 4096, current);
    assert!(matches!(
        s.plan_window(
            &current,
            256,
            vec![BoundaryPoint {
                offset: 100,
                kind: BoundaryKind::Seek,
            }],
        ),
        Err(BoundarySessionError::Evaluate(
            ProcessBoundaryError::OfflineExact { .. }
        ))
    ));

    // Uncertified precision → refused at open (same gate connect uses).
    let unmeasured =
        ProcessCapability::unmeasured(BlockContract::new(1, 512, InBlockTransportSupport::Unknown));
    let err = ProcessBoundarySession::open(
        control,
        unmeasured,
        ExecutionMode::OfflineExact,
        None,
        budget(4096),
        current,
    )
    .expect_err("uncertified offline exact must refuse at open");
    assert!(matches!(
        err,
        BoundarySessionError::Capability(ProcessBoundaryError::OfflineUncertified { .. })
    ));

    // Certified + legal window → Exact plan.
    let plan = s
        .plan_window(&current, 256, vec![])
        .expect("legal offline window");
    assert_eq!(plan.policy, DeliveryPolicy::Exact);
}

// ---------------------------------------------------------------------
// tim03_prov_* — precision provenance gates
// ---------------------------------------------------------------------

/// Every provenance axis refuses typed before staging — and a refused
/// `dispatch_params`/`dispatch_midi` stages no plan, so the plugin
/// records zero param delivery (誤 Param 配送 0).
#[test]
fn tim03_prov_stale_generations_zero_delivery() {
    let hosted = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    let mut dispatch = ParamDispatch::new(control, proc, frame_base()).expect("dispatch");
    dispatch
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .expect("bind");

    let c = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let pinned = prov(7, proc, 10, 20);
    let s = ProcessBoundarySession::open(
        dispatch.control(),
        c,
        ExecutionMode::Realtime,
        None,
        budget(4096),
        pinned,
    )
    .expect("session");

    // Baseline: current provenance delivers param events. (The node's
    // live plan is single/last-writer-wins — a MIDI plan staged now
    // would replace the param plan before it ever runs, so the MIDI
    // baseline goes through its own dispatch below.)
    s.dispatch_params(&mut dispatch, &pinned, &writes())
        .expect("baseline dispatch");
    drive.process(256);
    let baseline = hosted.log.lock().unwrap().all_params().len();
    assert!(baseline > 0, "baseline param delivery works");

    // The MIDI surface passes the same gate — last-writer-wins makes
    // it replace the param plan, which is fine: its events are not
    // `ParamValue` and record no param hits either way.
    s.dispatch_midi(&mut dispatch, &pinned, &midi_events())
        .expect("baseline midi dispatch");

    // Each stale axis → typed refusal on every gated surface.
    let stale: Vec<(&str, PrecisionProvenance)> = vec![
        ("instance", prov(8, proc, 10, 20)),
        ("node", prov(7, mint_node_id(), 10, 20)),
        ("capability_generation", prov(7, proc, 11, 20)),
        ("graph_generation", prov(7, proc, 10, 21)),
    ];
    for (axis, current) in &stale {
        // plan_window
        assert!(
            matches!(
                s.plan_window(current, 256, vec![]),
                Err(BoundarySessionError::InstanceMismatch { .. }
                    | BoundarySessionError::NodeMismatch { .. }
                    | BoundarySessionError::CapabilityGeneration { .. }
                    | BoundarySessionError::GraphGeneration { .. })
            ),
            "{axis}: plan_window must refuse stale provenance"
        );
        // dispatch_params — refused before any write resolves.
        let before_gen = dispatch.last_plan_generation();
        assert!(
            s.dispatch_params(&mut dispatch, current, &writes())
                .is_err(),
            "{axis}: dispatch_params must refuse"
        );
        assert_eq!(
            dispatch.last_plan_generation(),
            before_gen,
            "{axis}: refused dispatch must not mint a plan"
        );
        // dispatch_midi — same gate.
        assert!(
            s.dispatch_midi(&mut dispatch, current, &midi_events())
                .is_err(),
            "{axis}: dispatch_midi must refuse"
        );
        // stage_window — refused before commands reach the queue.
        assert!(
            s.stage_window(
                dispatch.control(),
                current,
                256,
                vec![],
                &[(TransportCommand::Seek(t(900)), 50)],
            )
            .is_err(),
            "{axis}: stage_window must refuse"
        );
    }

    // Drive several windows: refused staging delivered nothing —
    // the param log holds only the baseline hits.
    for _ in 0..4 {
        drive.process(256);
    }
    let log = hosted.log.lock().unwrap();
    assert_eq!(
        log.all_params().len(),
        baseline,
        "stale provenance must deliver zero additional param events"
    );
}

/// A `ParamDispatch` on a *different* node, or one whose clock
/// mapping (frame base) differs from the session's verified attach
/// context, is refused — wrong-target/stale-context param delivery is
/// structurally impossible through the gated path.
#[test]
fn tim03_prov_foreign_dispatch_refused() {
    let hosted_a = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
    let (graph_a, proc_a, srcs_a) = build_graph(&hosted_a.attached);
    let node_a = hosted_a.attached.node;
    let control_a = hosted_a.attached.control;
    let mut drive_a = Drive::new(&graph_a, node_a, proc_a, srcs_a);

    // Second attach on the *same* NodeId but a foreign frame base —
    // the clock-mapping-mismatch axis (positions authored against a
    // different base must never deliver through this session).
    let hosted_b = host_plugin_at_base(44_100, 1, 512, DeliveryPolicy::Exact, 4096);
    let mut dispatch_b = ParamDispatch::new(
        hosted_b.attached.control,
        proc_a, // same node id, foreign attach/frame base
        TimeBase::sample_rate(44_100).unwrap(),
    )
    .expect("foreign dispatch");
    dispatch_b
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .expect("bind foreign");

    let c = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let pinned = prov(3, proc_a, 5, 6);
    let s = ProcessBoundarySession::open(
        &control_a,
        c,
        ExecutionMode::Realtime,
        None,
        budget(4096),
        pinned,
    )
    .expect("session on node A");

    // dispatch_b drives proc_a's id but was built on a 44.1 kHz
    // attach — the session's verified 48 kHz clock refuses it.
    let err = s
        .dispatch_params(&mut dispatch_b, &pinned, &writes())
        .expect_err("foreign clock mapping must refuse");
    assert!(
        matches!(err, BoundarySessionError::FrameBaseMismatch(_)),
        "expected FrameBaseMismatch, got {err}"
    );

    // And a dispatch pinned to a different node id refuses
    // NodeMismatch. (`NodeId`s are graph-generation-scoped — a fresh
    // `build_graph` can mint the same raw id, which is exactly why
    // `graph_generation` is a separate provenance axis; the foreign
    // dispatch here carries a genuinely different id.)
    let hosted_c = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
    let mut dispatch_c =
        ParamDispatch::new(hosted_c.attached.control, mint_node_id(), frame_base())
            .expect("dispatch C");
    dispatch_c
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .expect("bind C");
    let err = s
        .dispatch_params(&mut dispatch_c, &pinned, &writes())
        .expect_err("foreign node must refuse");
    assert!(
        matches!(err, BoundarySessionError::NodeMismatch { .. }),
        "expected NodeMismatch, got {err}"
    );

    // Node A still delivers through its own dispatch — the refusals
    // above touched nothing on A.
    let mut dispatch_a = ParamDispatch::new(control_a, proc_a, frame_base()).expect("dispatch A");
    dispatch_a
        .bind(ParameterTargetId::new(1), ParamId(7), &PARAMS)
        .expect("bind A");
    s.dispatch_params(&mut dispatch_a, &pinned, &writes())
        .expect("own dispatch delivers");
    drive_a.process(256);
    assert!(
        !hosted_a.log.lock().unwrap().all_params().is_empty(),
        "node A param delivery unaffected by foreign refusals"
    );
}

/// `host_plugin` variant attaching at an arbitrary sample rate — the
/// clock-mapping axis needs a second frame base.
fn host_plugin_at_base(
    rate: u32,
    min_frames: u32,
    max_frames: u32,
    policy: DeliveryPolicy,
    deferral: u32,
) -> Hosted {
    let log = Arc::new(Mutex::new(CallLog::default()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        log: Arc::clone(&log),
    }));
    let HostReply::Instantiated { handle } = host
        .execute(HostCommand::Instantiate {
            plugin_id: DESCRIPTOR.id.to_owned(),
        })
        .unwrap()
    else {
        panic!("Instantiate")
    };
    host.execute(HostCommand::Initialize { handle }).unwrap();
    let process_config =
        ProcessConfig::new(SampleRate::new(rate).unwrap(), min_frames, max_frames).unwrap();
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
            port_format: AudioFormat {
                sample_rate: SampleRate::new(rate).unwrap(),
                layout: ChannelLayout::Stereo,
                sample_format: SampleFormat::F32,
            },
            bounds: EndpointBounds {
                max_frames: 512,
                channels: 2,
            },
            process_config,
            frame_base: TimeBase::sample_rate(rate).unwrap(),
            policy,
            budget: ScheduleBudget::new(64, 256, 16, deferral),
            input: None,
        },
    )
    .expect("attach");
    Hosted {
        _host: host,
        attached,
        log,
    }
}

/// Stale provenance also gates the pure `plan_window`/`stage_plan`
/// surfaces — a refused window stages no transport command and no
/// plan install (verified through the node's report stream).
#[test]
fn tim03_prov_stale_window_stages_nothing() {
    let hosted = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    let c = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let pinned = prov(1, proc, 10, 20);
    let s = session(&control, c, ExecutionMode::Realtime, None, 4096, pinned);
    let stale = prov(1, proc, 10, 99); // graph generation drift

    // plan_window / stage_window / stage_plan all refuse.
    assert!(s.plan_window(&stale, 256, vec![]).is_err());
    assert!(
        s.stage_window(
            &mut control,
            &stale,
            256,
            vec![],
            &[(TransportCommand::Play, 0)],
        )
        .is_err()
    );
    let b = control.plan_builder(PlanGeneration::new(9));
    let install: Arc<TransportPlan<EventKind>> = Arc::new(b.build());
    assert!(
        s.stage_plan(&mut control, &stale, 256, vec![], install, 0)
            .is_err()
    );

    // Nothing reached the node: no Play, no install — zero param
    // delivery and transport stays stopped (the node still runs
    // callbacks on a legal window).
    drive.process(256);
    let log = hosted.log.lock().unwrap();
    assert!(log.all_params().is_empty(), "stale window delivers nothing");
    assert!(
        (0..log.len()).all(|i| !log.hit(i).playing),
        "Play was never staged — transport stays stopped"
    );
}
