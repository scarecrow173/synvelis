//! TIM-03 acceptance tests — Issue #127 process-boundary slice
//! (headless mock; spec: `docs/specs/tim03-process-boundary.md`).
//!
//! Three layers, matching the slice contract:
//!
//! - `tim03_pb_*` — the Synvelis policy oracle
//!   (`ProcessCapability::evaluate`): 100/128/256-frame windows,
//!   boundaries at 0/1/100/last, min/max process legality, simultaneous
//!   Loop/Seek/PlanInstall kinds, typed `Unsupported`/`ConsentRequired`/
//!   `OfflineExact`/`OfflineUncertified`/`DeferralExceeded`, degraded
//!   coalescing, invalid requests.
//! - `tim03_sched_*` — the real `TransportScheduler` span path: Exact
//!   rejects illegal applies / emits undeliverable wraps, Degraded
//!   records `DeferredApply`, plan events deliver exactly once per
//!   loop occurrence at process-start-relative offsets, stale
//!   generation tags fault with zero wrong-target delivery.
//! - `tim03_node_*` — the real `PluginNode` physical-call path through
//!   `GraphRuntime`: every recorded `process` call satisfies
//!   `min_frames <= frames <= max_frames` (no zero-length calls),
//!   deferred boundaries move to legal frames, param events arrive
//!   with verified frame-base offsets, stale transport generations
//!   deliver nothing.
//!
//! Real CLAP/VST3/OS/backend×rate×block validation is `NotRun` — a
//! separate gate (Ruviform #196/#253 owners).

use std::sync::{Arc, Mutex};

use ruviform_audio::typed::{Compiler, CompilerConfig, DEFAULT_ALIGNMENT, GraphRuntime, RtRuntime};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{
    AttachConfig, NodeReport, PluginNode, PluginNodeFactory, attach,
};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, PortId, PortRef,
    Validator,
};
use ruviform_media_core::{
    ApplyDegradation, ApplyStatus, BlockContract, CallPartition, ClockEpochId, DeliveryPolicy,
    EngineFrame, FaultKind, InBlockTransportSupport, MediaDuration, MediaRange, MediaTime,
    PlanGeneration, RejectReason, ScheduleBudget, ScheduledEvent, SpanSink, TimeBase,
    TransportCommand, TransportGeneration, TransportPlanBuilder, TransportScheduler, TransportSpan,
    TransportStatus, event_engine_frame,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TransportInfo, calls_for_span, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::{
    BoundaryDegradation, BoundaryKind, BoundaryPoint, DegradedConsent, ExecutionMode,
    ProcessBoundaryError, ProcessBoundaryRequest, ProcessCapability, TimingPrecision,
};

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim03-rec",
    name: "TIM-03 Recorder",
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
// `process` (the same shape the `rt_` contract file measures).
// ---------------------------------------------------------------------

const MAX_CALLS: usize = 256;
const MAX_PARAMS_PER_CALL: usize = 16;

/// One parameter hit inside a physical call: the process-start-relative
/// `frame_offset`, the transport position at call start and the payload.
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
            .flat_map(|i| {
                let hit = self.calls[i];
                hit.param_hits().to_vec()
            })
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
        let mut log = self.log.lock().unwrap();
        if log.len < MAX_CALLS {
            let mut hit = CallHit {
                frames: u32::try_from(frames).unwrap_or(u32::MAX),
                position,
                playing,
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
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
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
/// the given delivery `policy` and `budget.max_deferral`.
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

/// `Source → plugin → Sink` — shared graph shape (same as PC-C4).
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
// Evaluator helpers
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

fn req(
    window: u32,
    boundaries: &[BoundaryPoint],
    mode: ExecutionMode,
    consent: Option<DegradedConsent>,
    deferral: u32,
) -> ProcessBoundaryRequest {
    ProcessBoundaryRequest {
        window_frames: window,
        boundaries: boundaries.to_vec(),
        mode,
        consent,
        budget: budget(deferral),
    }
}

fn bp(offset: u32, kind: BoundaryKind) -> BoundaryPoint {
    BoundaryPoint { offset, kind }
}

/// `(offset, frames)` of every chunk of `partition`.
fn chunks(partition: &CallPartition) -> Vec<(u32, u32)> {
    (0..partition.chunks())
        .map(|i| partition.chunk(i))
        .collect()
}

// ---------------------------------------------------------------------
// tim03_pb_* — policy oracle
// ---------------------------------------------------------------------

/// 100/128/256-frame windows × boundaries at 0 / 1 / middle / last —
/// every segment legally partitionable → Exact plan, contiguous
/// coverage, legal chunks only.
#[test]
fn tim03_pb_exact_windows_and_boundary_positions() {
    let c = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    for window in [100_u32, 128, 256] {
        for offset in [0, 1, window / 2, window - 1] {
            let pts = [bp(offset, BoundaryKind::Seek)];
            let plan = c
                .evaluate(&req(window, &pts, ExecutionMode::Realtime, None, 4096))
                .unwrap_or_else(|e| panic!("window={window} offset={offset}: {e}"));
            assert_eq!(
                plan.policy,
                DeliveryPolicy::Exact,
                "window={window} offset={offset}"
            );
            assert_eq!(plan.precision, TimingPrecision::FrameAccurateEvents);
            assert_eq!(
                plan.degradations,
                [] as [synvelis_player::BoundaryDegradation; 0]
            );
            // Segments tile [0, window) contiguously.
            let mut pos = 0_u32;
            for seg in &plan.segments {
                assert_eq!(seg.start, pos, "window={window} offset={offset}");
                assert!(seg.end <= window);
                pos = seg.end;
            }
            assert_eq!(pos, window, "window={window} offset={offset}");
            // Interior cuts only — offset 0 applies at the head.
            let expected_cuts: Vec<u32> = pts.iter().map(|p| p.offset).filter(|o| *o > 0).collect();
            assert_eq!(plan.boundaries, expected_cuts);
            // Every physical call legal and inside its segment.
            for seg in &plan.segments {
                for (off, frames) in chunks(&seg.partition) {
                    assert!((1..=512).contains(&frames), "{seg:?} chunk {off}+{frames}");
                    assert!(off + frames <= seg.end - seg.start);
                }
            }
        }
        // No boundaries → single segment.
        let plan = c
            .evaluate(&req(window, &[], ExecutionMode::Realtime, None, 4096))
            .unwrap();
        assert_eq!(plan.segments.len(), 1);
        assert_eq!((plan.segments[0].start, plan.segments[0].end), (0, window));
    }
}

/// min=128 / max=512: a boundary at offset 100 of a 256 window leaves
/// `[0,100)` illegal — typed outcomes per mode, then a consented
/// degraded plan deferring the boundary to frame 128.
#[test]
fn tim03_pb_min_max_illegal_segment_policy() {
    let c = cap(
        128,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let pts = [bp(100, BoundaryKind::LoopWrap)];

    // Realtime without consent → typed ConsentRequired naming the
    // illegal segment start.
    match c.evaluate(&req(256, &pts, ExecutionMode::Realtime, None, 4096)) {
        Err(ProcessBoundaryError::ConsentRequired { boundaries }) => {
            assert_eq!(boundaries, vec![0]);
        }
        other => panic!("expected ConsentRequired, got {other:?}"),
    }

    // Offline exact → fail closed, even though realtime could degrade.
    match c.evaluate(&req(256, &pts, ExecutionMode::OfflineExact, None, 4096)) {
        Err(ProcessBoundaryError::OfflineExact { boundary, len }) => {
            assert_eq!((boundary, len), (0, 100));
        }
        other => panic!("expected OfflineExact, got {other:?}"),
    }
    // Consent does not rescue offline exact.
    assert!(matches!(
        c.evaluate(&req(
            256,
            &pts,
            ExecutionMode::OfflineExact,
            Some(DegradedConsent::grant(64)),
            4096
        )),
        Err(ProcessBoundaryError::OfflineExact { .. })
    ));

    // Realtime + consent → degraded plan: boundary 100 → 128, both
    // adjacent segments legal, precision reports the consented loss.
    let plan = c
        .evaluate(&req(
            256,
            &pts,
            ExecutionMode::Realtime,
            Some(DegradedConsent::grant(64)),
            4096,
        ))
        .unwrap();
    assert_eq!(plan.policy, DeliveryPolicy::Degraded);
    assert_eq!(plan.precision, TimingPrecision::DegradedWithConsent);
    assert_eq!(plan.boundaries, vec![128]);
    assert_eq!(
        plan.degradations,
        vec![BoundaryDegradation {
            requested: 100,
            applied: 128,
            kind: BoundaryKind::LoopWrap,
        }]
    );
    let lens: Vec<(u32, u32)> = plan.segments.iter().map(|s| (s.start, s.end)).collect();
    assert_eq!(lens, vec![(0, 128), (128, 256)]);
    for seg in &plan.segments {
        for (_, frames) in chunks(&seg.partition) {
            assert!((128..=512).contains(&frames), "illegal call {frames}");
        }
    }

    // Deferral bound too small (consent or budget) → DeferralExceeded.
    for (consent, deferral) in [(64, 10), (10, 4096)] {
        match c.evaluate(&req(
            256,
            &pts,
            ExecutionMode::Realtime,
            Some(DegradedConsent::grant(consent)),
            deferral,
        )) {
            Err(ProcessBoundaryError::DeferralExceeded { boundary }) => {
                assert_eq!(boundary, 100);
            }
            other => panic!(
                "consent={consent} deferral={deferral}: expected DeferralExceeded, got {other:?}"
            ),
        }
    }
}

/// Boundary offsets 0 / 1 / last under a min=2 contract: offset 0 is a
/// head apply (no split), offsets producing a 1-frame segment are
/// illegal and need consent or fail per mode.
#[test]
fn tim03_pb_offsets_zero_one_last() {
    let c = cap(
        2,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );

    // offset 0 → applies at callback head; single legal segment.
    let plan = c
        .evaluate(&req(
            256,
            &[bp(0, BoundaryKind::Seek)],
            ExecutionMode::Realtime,
            None,
            4096,
        ))
        .unwrap();
    assert_eq!(plan.boundaries, [] as [u32; 0]);
    assert_eq!(plan.segments.len(), 1);
    assert_eq!((plan.segments[0].start, plan.segments[0].end), (0, 256));

    // offset 1 → [0,1) is a 1-frame segment < min → illegal.
    assert!(matches!(
        c.evaluate(&req(
            256,
            &[bp(1, BoundaryKind::Seek)],
            ExecutionMode::Realtime,
            None,
            4096
        )),
        Err(ProcessBoundaryError::ConsentRequired { .. })
    ));
    // offset last (255) → [255,256) 1-frame tail segment < min.
    match c.evaluate(&req(
        256,
        &[bp(255, BoundaryKind::Seek)],
        ExecutionMode::Realtime,
        None,
        4096,
    )) {
        Err(ProcessBoundaryError::ConsentRequired { boundaries }) => {
            assert_eq!(boundaries, vec![255]);
        }
        other => panic!("expected ConsentRequired, got {other:?}"),
    }

    // Same offsets legal under min=1.
    let c1 = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    for offset in [1, 255] {
        let plan = c1
            .evaluate(&req(
                256,
                &[bp(offset, BoundaryKind::Seek)],
                ExecutionMode::Realtime,
                None,
                4096,
            ))
            .unwrap();
        assert_eq!(plan.boundaries, vec![offset]);
        assert_eq!(plan.segments.len(), 2);
    }
}

/// `LoopWrap` + Seek + `PlanInstall` at the *same* offset coalesce into a
/// single boundary — matching the scheduler's same-frame apply rule.
#[test]
fn tim03_pb_simultaneous_kinds_dedupe() {
    let c = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let pts = [
        bp(100, BoundaryKind::PlanInstall),
        bp(100, BoundaryKind::LoopWrap),
        bp(100, BoundaryKind::Seek),
        bp(200, BoundaryKind::Other),
    ];
    let plan = c
        .evaluate(&req(256, &pts, ExecutionMode::Realtime, None, 4096))
        .unwrap();
    assert_eq!(plan.boundaries, vec![100, 200]);
    assert_eq!(plan.segments.len(), 3);

    // Illegal variant: one degradation record for the coalesced offset.
    let cmin = cap(
        128,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    let plan = cmin
        .evaluate(&req(
            256,
            &pts[..3],
            ExecutionMode::Realtime,
            Some(DegradedConsent::grant(64)),
            4096,
        ))
        .unwrap();
    assert_eq!(plan.degradations.len(), 1);
    assert_eq!(plan.degradations[0].requested, 100);
}

/// Capability precision gates: `Unsupported` refuses outright;
/// offline exact requires a certified exact class; `Unknown` never
/// upgrades itself; verified in-block lifts the delivered claim to
/// `ExactWithinVerifiedCapability`.
#[test]
fn tim03_pb_precision_gates() {
    // Certified Unsupported → typed Unsupported in both modes.
    let bad = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::Unsupported,
    );
    for mode in [ExecutionMode::Realtime, ExecutionMode::OfflineExact] {
        assert!(matches!(
            bad.evaluate(&req(256, &[], mode, None, 4096)),
            Err(ProcessBoundaryError::Unsupported { .. })
        ));
    }

    // Offline exact + un-certified precision → OfflineUncertified.
    for precision in [
        TimingPrecision::Unknown,
        TimingPrecision::BlockBoundaryOnly,
        TimingPrecision::DegradedWithConsent,
    ] {
        let c = cap(1, 512, InBlockTransportSupport::Unknown, precision);
        match c.evaluate(&req(256, &[], ExecutionMode::OfflineExact, None, 4096)) {
            Err(ProcessBoundaryError::OfflineUncertified { precision: p }) => {
                assert_eq!(p, precision);
            }
            other => panic!("precision={precision:?}: expected OfflineUncertified, got {other:?}"),
        }
    }

    // Certified exact classes pass the offline gate.
    for precision in [
        TimingPrecision::ExactWithinVerifiedCapability,
        TimingPrecision::FrameAccurateEvents,
    ] {
        let c = cap(1, 512, InBlockTransportSupport::Unknown, precision);
        assert!(
            c.evaluate(&req(256, &[], ExecutionMode::OfflineExact, None, 4096))
                .is_ok(),
            "precision={precision:?}"
        );
    }

    // Unknown capability still delivers structurally on the RT path —
    // the *claim* stays honest (delivered precision is reported, the
    // certified precision remains Unknown on the capability).
    let unknown =
        ProcessCapability::unmeasured(BlockContract::new(1, 512, InBlockTransportSupport::Unknown));
    assert_eq!(unknown.precision(), TimingPrecision::Unknown);
    let plan = unknown
        .evaluate(&req(256, &[], ExecutionMode::Realtime, None, 4096))
        .unwrap();
    assert_eq!(plan.precision, TimingPrecision::FrameAccurateEvents);

    // Verified in-block transport lifts the exact claim.
    let verified = cap(
        1,
        512,
        InBlockTransportSupport::Verified,
        TimingPrecision::ExactWithinVerifiedCapability,
    );
    let plan = verified
        .evaluate(&req(256, &[], ExecutionMode::Realtime, None, 4096))
        .unwrap();
    assert_eq!(
        plan.precision,
        TimingPrecision::ExactWithinVerifiedCapability
    );
}

/// Malformed requests and degenerate contracts are typed, never
/// panics (a `max_frames == 0` contract would divide by zero inside
/// `legal_span_len` — the evaluator refuses before reaching it).
#[test]
fn tim03_pb_invalid_requests() {
    let c = cap(
        1,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );
    assert!(matches!(
        c.evaluate(&req(0, &[], ExecutionMode::Realtime, None, 4096)),
        Err(ProcessBoundaryError::InvalidRequest { .. })
    ));
    for offset in [256, 257] {
        assert!(matches!(
            c.evaluate(&req(
                256,
                &[bp(offset, BoundaryKind::Seek)],
                ExecutionMode::Realtime,
                None,
                4096
            )),
            Err(ProcessBoundaryError::InvalidRequest { .. })
        ));
    }
    let degenerate = ProcessCapability::new(
        BlockContract::new(0, 0, InBlockTransportSupport::Unknown),
        TimingPrecision::FrameAccurateEvents,
    );
    assert!(matches!(
        degenerate.evaluate(&req(256, &[], ExecutionMode::Realtime, None, 4096)),
        Err(ProcessBoundaryError::Unsupported { .. })
    ));
}

/// Degraded coalescing: a deferred boundary can carry later requested
/// offsets into the applied frame; loop wraps never bound the deferral
/// scan (position-driven, matching upstream `find_legal_boundary`).
#[test]
fn tim03_pb_degraded_coalescing_and_wrap_bounds() {
    let c = cap(
        128,
        512,
        InBlockTransportSupport::Unknown,
        TimingPrecision::FrameAccurateEvents,
    );

    // Seek@100 defers to 128 — past the LoopWrap@120, which coalesces
    // into the applied frame; Seek@272 stays legal.
    let pts = [
        bp(100, BoundaryKind::Seek),
        bp(120, BoundaryKind::LoopWrap),
        bp(272, BoundaryKind::Seek),
    ];
    let plan = c
        .evaluate(&req(
            400,
            &pts,
            ExecutionMode::Realtime,
            Some(DegradedConsent::grant(64)),
            4096,
        ))
        .unwrap();
    assert_eq!(plan.boundaries, vec![128, 272]);
    assert_eq!(
        plan.degradations,
        vec![BoundaryDegradation {
            requested: 100,
            applied: 128,
            kind: BoundaryKind::Seek,
        }]
    );
    let lens: Vec<(u32, u32)> = plan.segments.iter().map(|s| (s.start, s.end)).collect();
    assert_eq!(lens, vec![(0, 128), (128, 272), (272, 400)]);

    // The wrap's own deferral fails → the whole plan is refused
    // (fail-closed: upstream would emit an undeliverable span + wrap
    // anyway — the policy oracle does not certify that as consented
    // degraded delivery).
    let pts2 = [bp(100, BoundaryKind::Seek), bp(150, BoundaryKind::LoopWrap)];
    match c.evaluate(&req(
        256,
        &pts2,
        ExecutionMode::Realtime,
        Some(DegradedConsent::grant(200)),
        4096,
    )) {
        Err(ProcessBoundaryError::DeferralExceeded { boundary }) => {
            assert_eq!(boundary, 150);
        }
        other => panic!("expected DeferralExceeded at wrap, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// tim03_sched_* — real TransportScheduler span path
// ---------------------------------------------------------------------

fn ef(frame: u64) -> EngineFrame {
    EngineFrame::new(ClockEpochId::new(0), frame)
}

fn sched(
    contract: BlockContract,
    policy: DeliveryPolicy,
    deferral: u32,
) -> TransportScheduler<u32> {
    TransportScheduler::new(frame_base(), contract, policy, budget(deferral))
}

fn t(value: i64) -> MediaTime {
    MediaTime::new(value, frame_base())
}

fn loop_range(start: i64, end: i64) -> MediaRange {
    MediaRange::new(t(start), MediaDuration::new(end - start, frame_base()))
}

/// What the sink observed for one span.
#[derive(Debug)]
struct SpanRec {
    start: u64,
    end: u64,
    deliverable: bool,
    /// `(payload, engine_frame or None)` per windowed event.
    events: Vec<(u32, Option<u64>)>,
}

struct RecSink {
    contract: BlockContract,
    spans: Vec<SpanRec>,
}

impl RecSink {
    fn new(contract: BlockContract) -> Self {
        Self {
            contract,
            spans: Vec::new(),
        }
    }
}

impl SpanSink<u32> for RecSink {
    fn span(&mut self, span: &TransportSpan, events: &[ScheduledEvent<u32>]) {
        let evs = events
            .iter()
            .map(|e| {
                (
                    e.payload,
                    event_engine_frame(span, e).map(EngineFrame::frame),
                )
            })
            .collect();
        self.spans.push(SpanRec {
            start: span.start.frame(),
            end: span.end.frame(),
            deliverable: span.deliverable,
            events: evs,
        });
        // Every deliverable span must partition into legal calls —
        // the zero-illegal-process-length invariant at span level.
        if span.deliverable {
            let part = calls_for_span(span, &self.contract).expect("deliverable span partitions");
            for i in 0..part.chunks() {
                let (_, frames) = part.chunk(i);
                assert!(
                    self.contract.legal_call_len(frames),
                    "illegal call {frames} in span {}..{}",
                    span.start.frame(),
                    span.end.frame()
                );
            }
        }
    }
}

/// Exact policy + illegal apply boundary → typed reject, last-good
/// state kept, the window continues as one deliverable span.
#[test]
fn tim03_sched_exact_rejects_illegal_apply() {
    let contract = BlockContract::new(128, 512, InBlockTransportSupport::Unknown);
    let mut s = sched(contract, DeliveryPolicy::Exact, 4096);
    s.request(TransportCommand::Play, ef(0)).unwrap();
    s.request(TransportCommand::Seek(t(1000)), ef(100)).unwrap();

    let mut sink = RecSink::new(contract);
    s.run_callback(ef(0), 256, &mut sink);

    // Seek rejected with the typed reason; Play applied.
    let mut rejected = false;
    while let Some(r) = s.pop_receipt() {
        if r.status == ApplyStatus::Rejected {
            rejected = true;
            assert_eq!(r.reason, Some(RejectReason::UnsupportedExactTransport));
            assert_eq!(r.requested_frame.frame(), 100);
        }
    }
    assert!(rejected, "seek must be rejected under Exact");
    assert!(
        s.pop_fault()
            .is_some_and(|f| f.kind == FaultKind::UnrepresentableBoundary)
    );

    // One deliverable span covering the whole window under last-good.
    assert_eq!(sink.spans.len(), 1);
    let span = &sink.spans[0];
    assert!(span.deliverable);
    assert_eq!((span.start, span.end), (0, 256));
}

/// Exact policy + illegal loop wrap → spans emitted *undeliverable*
/// (time keeps running), wraps still apply, faults recorded. The
/// policy oracle refuses this scenario up front; the scheduler's
/// defined-loss path is the observable backstop.
#[test]
fn tim03_sched_exact_wrap_undeliverable() {
    let contract = BlockContract::new(128, 512, InBlockTransportSupport::Unknown);
    let mut s = sched(contract, DeliveryPolicy::Exact, 4096);
    s.request(TransportCommand::Play, ef(0)).unwrap();
    s.request(TransportCommand::SetLoop(loop_range(0, 100)), ef(0))
        .unwrap();

    let mut sink = RecSink::new(contract);
    s.run_callback(ef(0), 256, &mut sink);

    // Wraps at frames 100 and 200 each leave an illegal 100-frame
    // span; the 56-frame tail is illegal too — all undeliverable.
    assert_eq!(sink.spans.len(), 3);
    assert!(sink.spans.iter().all(|s| !s.deliverable));
    assert_eq!(
        sink.spans
            .iter()
            .map(|s| (s.start, s.end))
            .collect::<Vec<_>>(),
        vec![(0, 100), (100, 200), (200, 256)]
    );
    let mut wrap_faults = 0;
    while let Some(f) = s.pop_fault() {
        wrap_faults += usize::from(f.kind == FaultKind::UnrepresentableLoopWrap);
    }
    assert_eq!(wrap_faults, 2, "one fault per undeliverable wrap");
}

/// Degraded policy + consented deferral budget → the illegal apply
/// boundary moves to the next legal frame with a `DeferredApply`
/// receipt; every emitted span stays deliverable.
#[test]
fn tim03_sched_degraded_defers_apply() {
    let contract = BlockContract::new(128, 512, InBlockTransportSupport::Unknown);
    let mut s = sched(contract, DeliveryPolicy::Degraded, 64);
    s.request(TransportCommand::Play, ef(0)).unwrap();
    s.request(TransportCommand::Seek(t(1000)), ef(100)).unwrap();

    let mut sink = RecSink::new(contract);
    s.run_callback(ef(0), 256, &mut sink);

    assert_eq!(sink.spans.len(), 2);
    assert_eq!((sink.spans[0].start, sink.spans[0].end), (0, 128));
    assert_eq!((sink.spans[1].start, sink.spans[1].end), (128, 256));
    assert!(sink.spans.iter().all(|s| s.deliverable));

    let mut deferred = false;
    while let Some(r) = s.pop_receipt() {
        if let Some(ApplyDegradation::DeferredApply { requested, applied }) = r.degradation {
            deferred = true;
            assert_eq!(requested.frame(), 100);
            assert_eq!(applied.frame(), 128);
        }
    }
    assert!(deferred, "seek must record DeferredApply 100→128");
}

/// Simultaneous Loop wrap + Seek + plan install converging at one
/// boundary frame: deterministic single split, apply wins the tie,
/// wraps collapse under the post-seek position (600 in loop[0,200] →
/// position 0, occurrence 3).
#[test]
fn tim03_sched_simultaneous_loop_seek_plan() {
    let contract = BlockContract::new(1, 512, InBlockTransportSupport::Unknown);
    let mut s = sched(contract, DeliveryPolicy::Exact, 4096);
    s.request(TransportCommand::Play, ef(0)).unwrap();
    s.request(TransportCommand::SetLoop(loop_range(0, 200)), ef(0))
        .unwrap();
    s.request(TransportCommand::Seek(t(600)), ef(200)).unwrap();

    // Plan install at the same boundary frame; event@pos20 is inside
    // the post-wrap position range [0,56) of span [200,256).
    let mut b = TransportPlanBuilder::<u32>::new(frame_base(), PlanGeneration::new(7));
    b.event(t(20), 1, 42).unwrap();
    let plan = Arc::new(b.build());
    s.install_plan(plan, ef(200)).unwrap();

    let mut sink = RecSink::new(contract);
    s.run_callback(ef(0), 256, &mut sink);

    // One boundary at frame 200 — not three.
    assert_eq!(sink.spans.len(), 2);
    assert_eq!((sink.spans[0].start, sink.spans[0].end), (0, 200));
    assert_eq!((sink.spans[1].start, sink.spans[1].end), (200, 256));
    // Post-seek+wrap position: 600 mod loop[0,200] → position 0.
    // Span0 is emitted before the plan install at frame 200 applies,
    // so no plan is live for it — no events windowed. The event
    // delivers exactly once in span1 at engine frame 220 = span start
    // 200 + position offset 20.
    assert_eq!(
        sink.spans[0].events,
        [] as [(u32, std::option::Option<u64>); 0]
    );
    assert_eq!(sink.spans[1].events, vec![(42, Some(220))]);
    let delivered: Vec<(u32, u64)> = sink
        .spans
        .iter()
        .flat_map(|s| s.events.iter().filter_map(|(p, f)| f.map(|f| (*p, f))))
        .collect();
    assert_eq!(delivered, vec![(42, 220)]);
}

/// Plan events deliver exactly once per loop occurrence at
/// process-start-relative engine frames; an event bound to a stale
/// transport generation faults (`StaleEventGeneration`) and is never
/// delivered while absolute events still fire.
#[test]
fn tim03_sched_events_boundary_and_stale_generation() {
    let contract = BlockContract::new(1, 512, InBlockTransportSupport::Unknown);
    let mut s = sched(contract, DeliveryPolicy::Exact, 4096);
    s.request(TransportCommand::Play, ef(0)).unwrap();
    let mut sink = RecSink::new(contract);
    s.run_callback(ef(0), 128, &mut sink);
    sink.spans.clear();
    // Play applied → transport_generation == 1.
    assert_eq!(s.transport_generation(), TransportGeneration::new(1));

    // Plan gen 7: absolute@200 (p1), bound tg=1 @500 (p2), absolute@350 (p3).
    let mut b = TransportPlanBuilder::<u32>::new(frame_base(), PlanGeneration::new(7));
    b.event(t(200), 1, 1).unwrap();
    b.bound_event(t(500), 2, TransportGeneration::new(1), 2)
        .unwrap();
    b.event(t(350), 3, 3).unwrap();
    let plan = Arc::new(b.build());
    s.install_plan(plan, ef(128)).unwrap();

    // Window [128,384): positions 128→384 while playing.
    s.run_callback(ef(128), 256, &mut sink);
    let delivered: Vec<(u32, u64)> = sink
        .spans
        .iter()
        .flat_map(|s| s.events.iter().filter_map(|(p, f)| f.map(|f| (*p, f))))
        .collect();
    // p1@200 → frame 200, p3@350 → frame 350; p2@500 is ahead — not yet.
    assert_eq!(delivered, vec![(1, 200), (3, 350)]);

    // Seek bumps transport_generation → the tg=1 bound event goes
    // stale; position 400 at frame 400, p2@500 would land at frame 500.
    sink.spans.clear();
    s.request(TransportCommand::Seek(t(400)), ef(400)).unwrap();
    s.run_callback(ef(384), 256, &mut sink);

    // p2 is inside the post-seek position window but generation-stale:
    // visible to the sink, deliverable to nobody.
    let p2_entries: Vec<(u32, Option<u64>)> = sink
        .spans
        .iter()
        .flat_map(|s| s.events.iter().copied())
        .filter(|(p, _)| *p == 2)
        .collect();
    assert!(!p2_entries.is_empty(), "stale event still windowed");
    assert!(p2_entries.iter().all(|(_, f)| f.is_none()));

    let mut stale = false;
    while let Some(f) = s.pop_fault() {
        stale |= f.kind == FaultKind::StaleEventGeneration;
    }
    assert!(stale, "stale bound event must fault");
}

// ---------------------------------------------------------------------
// tim03_node_* — real PluginNode physical calls via GraphRuntime
// ---------------------------------------------------------------------

/// Exact attach + illegal seek boundary → typed reject upstream, the
/// window continues under last-good; every physical call legal.
#[test]
fn tim03_node_exact_rejects_illegal_boundary() {
    let hosted = host_plugin(128, 512, DeliveryPolicy::Exact, 4096);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    control.request(TransportCommand::Play, 0).unwrap();
    control
        .request(TransportCommand::Seek(t(1000)), 100)
        .unwrap();
    drive.process(256);

    let log = hosted.log.lock().unwrap();
    assert_eq!(log.len(), 1, "one deliverable span → partitioned calls");
    let hit = log.hit(0);
    assert_eq!(hit.frames, 256, "single legal 256-frame call");
    assert!(hit.playing);
    assert!(
        control
            .drain_reports()
            .iter()
            .any(|r| matches!(r, NodeReport::Receipt(rec) if rec.status == ApplyStatus::Rejected))
    );
}

/// Degraded attach (the consented policy) → the same boundary defers
/// to a legal frame: two 128-frame calls, `DeferredApply` receipt.
#[test]
fn tim03_node_degraded_consented_boundary() {
    let hosted = host_plugin(128, 512, DeliveryPolicy::Degraded, 64);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    control.request(TransportCommand::Play, 0).unwrap();
    control
        .request(TransportCommand::Seek(t(1000)), 100)
        .unwrap();
    drive.process(256);

    let log = hosted.log.lock().unwrap();
    assert_eq!(log.len(), 2, "deferred boundary splits into two calls");
    assert_eq!(log.hit(0).frames, 128);
    assert_eq!(log.hit(1).frames, 128);
    assert!(log.hit(0).playing && log.hit(1).playing);
    // The seek applied at frame 128: call 2 sees the post-seek position.
    assert_eq!(log.hit(1).position, 1000);

    let reports = control.drain_reports();
    assert!(reports.iter().any(|r| matches!(
        r,
        NodeReport::Receipt(rec)
            if matches!(rec.degradation, Some(ApplyDegradation::DeferredApply { .. }))
    )));
}

/// Param event inside a loop: delivered exactly once per loop
/// occurrence, each at the verified process-start-relative offset —
/// the wrap boundary never produces an illegal call.
#[test]
fn tim03_node_param_event_per_loop_occurrence() {
    let hosted = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    control.request(TransportCommand::Play, 0).unwrap();
    control
        .request(TransportCommand::SetLoop(loop_range(0, 100)), 0)
        .unwrap();
    // Plan: param7 @ position 50 (inside the loop → fires once per
    // occurrence at engine frames 50 / 150 / 250).
    let mut b = control.plan_builder(PlanGeneration::new(7));
    b.event(
        t(50),
        1,
        EventKind::ParamValue(ParamEvent {
            param_id: ParamId(7),
            value: ParamValue::new(0.5).unwrap(),
        }),
    )
    .unwrap();
    let plan = Arc::new(b.build());
    control.install_plan(plan, 0).unwrap();

    drive.process(256);

    let log = hosted.log.lock().unwrap();
    // Wraps at 100 and 200 → calls [0,100), [100,200), [200,256).
    assert_eq!(log.len(), 3);
    assert_eq!(log.hit(0).frames, 100);
    assert_eq!(log.hit(1).frames, 100);
    assert_eq!(log.hit(2).frames, 56);
    // Post-wrap calls see position 0 (loop re-anchor).
    assert_eq!(log.hit(0).position, 0);
    assert_eq!(log.hit(1).position, 0);
    assert_eq!(log.hit(2).position, 0);
    // One hit per call — each occurrence delivers the event once at
    // process-start-relative offset 50 → absolute position 50.
    for i in 0..3 {
        let hit = log.hit(i);
        let hits = hit.param_hits();
        assert_eq!(hits.len(), 1, "call {i}");
        assert_eq!(hits[0].param_id, 7);
        assert_eq!(hits[0].frame_offset, 50);
        assert_eq!(hits[0].abs, 50);
        assert!((hits[0].value - 0.5).abs() < 1e-9);
    }
}

/// Loop wrap + Seek + plan install converging at frame 200 plus a
/// param event downstream: one boundary, deterministic delivery.
/// Seek→600 inside loop[0,200] collapses to position 0 — the event
/// at position 20 fires once at engine frame 220.
#[test]
fn tim03_node_simultaneous_loop_seek_plan_param() {
    let hosted = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    control.request(TransportCommand::Play, 0).unwrap();
    control
        .request(TransportCommand::SetLoop(loop_range(0, 200)), 0)
        .unwrap();
    control
        .request(TransportCommand::Seek(t(600)), 200)
        .unwrap();
    let mut b = control.plan_builder(PlanGeneration::new(7));
    b.event(
        t(20),
        1,
        EventKind::ParamValue(ParamEvent {
            param_id: ParamId(7),
            value: ParamValue::new(0.25).unwrap(),
        }),
    )
    .unwrap();
    let plan = Arc::new(b.build());
    control.install_plan(plan, 200).unwrap();

    drive.process(256);

    let log = hosted.log.lock().unwrap();
    // Boundary only at 200 → calls [0,200) and [200,256).
    assert_eq!(log.len(), 2);
    assert_eq!(log.hit(0).frames, 200);
    assert_eq!(log.hit(1).frames, 56);
    // The event is generation-stale for call 0 (plan installs at 200)
    // and delivers exactly once in call 1: position 0 + offset 20.
    assert_eq!(log.hit(0).param_hits(), []);
    assert_eq!(log.hit(1).position, 0);
    let second = log.hit(1);
    let params = second.param_hits();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].param_id, 7);
    assert_eq!(params[0].frame_offset, 20);
    assert_eq!(params[0].abs, 20);
}

/// Transport-generation invalidation: an event bound to the pre-seek
/// generation is stale afterwards — zero wrong-target delivery, a
/// `StaleEventGeneration` fault is reported; the absolute event still
/// delivers exactly once.
#[test]
fn tim03_node_stale_transport_generation_zero_delivery() {
    let hosted = host_plugin(1, 512, DeliveryPolicy::Exact, 4096);
    let (graph, proc, srcs) = build_graph(&hosted.attached);
    let node = hosted.attached.node;
    let mut control = hosted.attached.control;
    let mut drive = Drive::new(&graph, node, proc, srcs);

    control.request(TransportCommand::Play, 0).unwrap();
    drive.process(128); // Play applies → transport_generation == 1

    // Plan bound to tg=1: bound param7@pos 430, absolute param9@pos 470.
    let mut b = control.plan_builder(PlanGeneration::new(7));
    b.bound_event(
        t(430),
        1,
        TransportGeneration::new(1),
        EventKind::ParamValue(ParamEvent {
            param_id: ParamId(7),
            value: ParamValue::new(0.9).unwrap(),
        }),
    )
    .unwrap();
    b.event(
        t(470),
        2,
        EventKind::ParamValue(ParamEvent {
            param_id: ParamId(9),
            value: ParamValue::new(0.7).unwrap(),
        }),
    )
    .unwrap();
    let plan = Arc::new(b.build());
    control.install_plan(plan, 0).unwrap();
    // Seek inside the next window: due 22 → engine frame 150.
    control.request(TransportCommand::Seek(t(400)), 22).unwrap();

    drive.process(256); // frames 128..384; seek@150 → pos 400

    let log = hosted.log.lock().unwrap();
    let all = log.all_params();
    // Bound param7 event: zero hits anywhere.
    assert!(
        all.iter().all(|h| h.param_id != 7),
        "stale-generation param7 must not deliver: {all:?}"
    );
    // Absolute param9@pos470: pos 400@frame150 → pos470@frame220,
    // inside window [128,384) → delivered once, offset 70 in its call.
    let p9: Vec<&ParamHit> = all.iter().filter(|h| h.param_id == 9).collect();
    assert_eq!(p9.len(), 1, "absolute event delivers exactly once: {all:?}");
    assert_eq!(p9[0].abs, 470);
    assert_eq!(p9[0].frame_offset, 70);

    let reports = control.drain_reports();
    assert!(reports.iter().any(|r| matches!(
        r,
        NodeReport::Fault(f)
            if f.kind == FaultKind::StaleEventGeneration
    )));
}
