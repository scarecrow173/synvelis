//! PC-C6 oracle TR rows re-evidenced through the real dispatch-ledger
//! path (issue #103, PC-C3 trace gate × TRACE-C3-05).
//!
//! `crates/synvelis-project/tests/pc_c6_oracle_tr.rs` verified the
//! TR01..TR08 classification rules against hand-built [`ControlTrace`]s
//! and recorded the `synvelis-player` dispatch-ledger path plus bounded
//! history retention (TRACE-C3-05) as `NotRun`. This file closes that
//! gap for the rows the real path can evidence:
//!
//! - TR01 — stage independence: `prepared`/`dispatched`/receipt slots
//!   never synthesize the `plugin_reported`/`observed` channels.
//! - TR03 — context changes classify `Stale` with the named reason,
//!   including a *real* plan-generation re-key through re-dispatch.
//! - TR04 — ledger `Unavailable`/`Reported` terminals vs probe
//!   `Missing`/`Lost`/`Measuring` channel states stay distinct.
//! - TR05 — `plugin_reported` stamp provenance: the report channel
//!   carries no timestamp, so the adapter stamps drain-time transport
//!   position as `Estimated` — never `Current` (adapter honesty
//!   contract).
//! - TR07 — the same probe observation recorded on two targets yields
//!   per-trace verdicts under their own contexts.
//! - TR08 — bounded retention: history ring, target cap, generation
//!   window expiry and the metrics that account for every loss.
//!
//! Every trace under test is produced by the real path —
//! [`ParamDispatch`] staging generation-tagged plans onto an
//! in-process fake plugin node, `GraphRuntime` driving real scheduler
//! receipts back through the node report channel, and
//! [`DispatchTraceAdapter`] projecting ledger observations onto
//! [`ControlTrace`]s — never hand-assembled. Expected freshness /
//! coherence verdicts are still derived by `oracle_*` functions
//! re-implemented from the documented contract (the same rules as the
//! project-layer oracle), never by calling the methods under test.
//! Adapter unit contracts (drop mapping, expiry windows, receipt
//! correlation internals) stay covered by `trace_c3_05.rs`; this
//! slice only re-evidences the TR classification rules end to end.
//!
//! Headless/fake-plugin evidence only — no real GUI, real
//! VST3/CLAP, real OS, real RT pass or consumer acceptance is claimed
//! (those rows stay `NotRun` / delegated to issue #114 and related
//! gates).

use ruviform_audio::typed::{Compiler, CompilerConfig, DEFAULT_ALIGNMENT, GraphRuntime, RtRuntime};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, PluginNodeFactory, attach};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, PortId, PortRef,
    Validator,
};
use ruviform_media_core::{
    ClockEpochId, DeliveryPolicy, EngineFrame, LoopOccurrence, MediaDuration, MediaTime,
    MediaTimestamp, PlanGeneration, ScheduleBudget, SpanContext, TempoMapRevision, TimeBase,
    TimestampProvenance, TransportGeneration, TransportSpan, TransportStatus,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList,
    ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TimedParamEvent, TransportInfo, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::ParamDispatch;
use synvelis_player::trace::{
    DispatchTraceAdapter, TraceAdapterConfig, TraceContext, TraceEventKind,
};
use synvelis_project::trace::{
    ConfiguredEvidence, ControlTrace, DispatchDropReason, DispatchEvidence, FreshnessContext,
    MeasuredValue, ObservationFreshness, PluginReportedEvidence, PreparedEvidence,
    ProbeObservation, ReceiptEvidence, StaleReason, TraceSlot, UnavailableReason,
};
use synvelis_project::{ParameterTargetId, ProjectId, ProjectRevision};

// ---------------------------------------------------------------------------
// Fixtures — deliberately different ids/positions from `trace_c3_05.rs`
// and `pc_c6_oracle_tr.rs` so this oracle does not inherit their choices.
// ---------------------------------------------------------------------------

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c6-tr-dispatch",
    name: "PC-C6 TR Dispatch",
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

/// Five automatable params — enough targets for every row without
/// re-binding mid-test.
const PARAMS: [ParamInfo; 5] = [
    ParamInfo {
        id: ParamId(21),
        name: "Gain",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE,
    },
    ParamInfo {
        id: ParamId(22),
        name: "Pan",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.5,
        flags: ParamFlags::AUTOMATABLE,
    },
    ParamInfo {
        id: ParamId(23),
        name: "Width",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE,
    },
    ParamInfo {
        id: ParamId(24),
        name: "Depth",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE,
    },
    ParamInfo {
        id: ParamId(25),
        name: "Send",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE,
    },
];

/// TR08's five targets map onto params 21..=25 in order.
const TR08_PARAMS: [u32; 5] = [21, 22, 23, 24, 25];

/// Declared project-side context stamped onto every trace key.
fn declared_ctx() -> TraceContext {
    TraceContext {
        project_id: ProjectId::new(7),
        project_revision: ProjectRevision(5),
        graph_generation: 11,
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

/// Position in the plan/span frame base (1 tick == 1 frame @ 48 kHz).
fn pos(ticks: i64) -> MediaTime {
    MediaTime::new(ticks, frame_base())
}

/// Declared measurement age budget — 2 s at 48 kHz.
fn max_age() -> MediaDuration {
    MediaDuration::new(96_000, frame_base())
}

/// Span epoch — a declared fixture value; receipt-attributed targets
/// get the scheduler's own epoch instead.
fn span_epoch() -> ClockEpochId {
    ClockEpochId::new(0)
}

/// A declared `Playing` span of `frames` engine frames whose
/// position window starts at `position_ticks` (frame-base ticks).
/// `plan_generation` must name the staged plan for its events to
/// project; `occurrence`/`position_ticks` are the transport-side
/// context the adapter copies onto trace keys.
fn span(
    plan_generation: PlanGeneration,
    position_ticks: i64,
    occurrence: LoopOccurrence,
    frames: u64,
) -> TransportSpan {
    let epoch = span_epoch();
    TransportSpan {
        start: EngineFrame::new(epoch, 0),
        end: EngineFrame::new(epoch, frames),
        ctx: SpanContext {
            status: TransportStatus::Playing,
            position_ticks,
            position_base: frame_base(),
            loop_ticks: None,
            occurrence,
            clock_epoch: epoch,
            transport_generation: TransportGeneration::new(0),
            plan_generation,
            tempo_revision: TempoMapRevision::new(0),
        },
        deliverable: true,
    }
}

/// The physical call window paired with [`span`] — one call covering
/// the whole span.
fn call_window(frames: u32) -> (EngineFrame, u32) {
    (EngineFrame::new(span_epoch(), 0), frames)
}

/// One scheduled write for `target`'s bound `param` at an absolute
/// frame-base position.
fn write(
    target: ParameterTargetId,
    param: u32,
    position_ticks: i64,
    value: f64,
) -> (ParameterTargetId, TimedParamEvent) {
    (
        target,
        TimedParamEvent {
            position: pos(position_ticks),
            event: ParamEvent {
                param_id: ParamId(param),
                value: ParamValue::new(value).unwrap(),
            },
        },
    )
}

/// A `FreshnessContext` matching `trace.key` on every context
/// dimension — the "live world agrees with the evidence" baseline.
/// `now` is the caller's playhead.
fn live_ctx(trace: &ControlTrace, now: MediaTime) -> FreshnessContext {
    FreshnessContext {
        project_revision: trace.key.project_revision,
        control_plan_generation: trace.key.control_plan_generation,
        graph_generation: trace.key.graph_generation,
        transport_generation: trace.key.transport_generation,
        clock_epoch: trace.key.clock_epoch,
        evaluation_position: trace.key.evaluation_position,
        loop_occurrence: trace.key.loop_occurrence,
        now,
        max_age: max_age(),
    }
}

fn stamp_at(when: MediaTime, provenance: TimestampProvenance) -> MediaTimestamp {
    MediaTimestamp {
        time: when,
        provenance,
    }
}

// ---------------------------------------------------------------------------
// Independent oracle — re-implements the documented contract (mirrors
// `pc_c6_oracle_tr.rs`). None of these functions call `ControlTrace`
// methods; expected verdicts are derived from the rules, not observed.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
enum OracleChannel {
    PluginReported,
    Observed,
}

/// Freshness rank — `Current` is the only live verdict; higher rank is
/// "less trustworthy". `non_exhaustive` forces a wildcard; unknown
/// future variants rank worst so they can never look live.
fn oracle_rank(freshness: &ObservationFreshness) -> u8 {
    match freshness {
        ObservationFreshness::Current => 0,
        ObservationFreshness::Estimated => 1,
        ObservationFreshness::Stale { .. } => 2,
        ObservationFreshness::DataLoss { .. } => 3,
        _ => 4, // Unavailable and any future variant
    }
}

/// First context mismatch in the documented deterministic order:
/// project revision, plan generation, graph generation, transport
/// generation, clock epoch, loop occurrence, evaluation position.
fn oracle_context_stale(
    trace: &ControlTrace,
    context: &FreshnessContext,
) -> Option<ObservationFreshness> {
    let key = &trace.key;
    if key.project_revision != context.project_revision {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::ProjectRevision,
        });
    }
    if key.control_plan_generation != context.control_plan_generation {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::ControlPlan,
        });
    }
    if key.graph_generation != context.graph_generation {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::Graph,
        });
    }
    if key.transport_generation != context.transport_generation {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::Transport,
        });
    }
    if key.clock_epoch != context.clock_epoch {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::ClockEpoch,
        });
    }
    if key.loop_occurrence != context.loop_occurrence {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::Loop,
        });
    }
    if key.evaluation_position != context.evaluation_position {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::Position,
        });
    }
    None
}

/// Non-`Present` slots map to their recorded absence reason. `Present`
/// is handled by callers; unknown future slot variants degrade to the
/// conservative `NoEvidence`.
fn oracle_slot_unavailable<T>(slot: &TraceSlot<T>) -> ObservationFreshness {
    let reason = match slot {
        TraceSlot::Unsupported => UnavailableReason::Unsupported,
        TraceSlot::Dropped => UnavailableReason::Dropped,
        TraceSlot::Expired => UnavailableReason::Expired,
        // Missing and any unknown future variant → conservative
        // NoEvidence (Present is handled by callers).
        _ => UnavailableReason::NoEvidence,
    };
    ObservationFreshness::Unavailable { reason }
}

/// Stamp evaluation: uncomputable deadline or future stamp →
/// `ClockUntrusted`; older than the age budget → `Stale{Age}` (age is
/// checked before provenance); estimated provenance → Estimated;
/// otherwise Current. `now == deadline` and `now == stamp.time` are
/// *not* stale (strict comparisons).
fn oracle_stamp_freshness(
    stamp: &MediaTimestamp,
    context: &FreshnessContext,
) -> ObservationFreshness {
    let Ok(deadline) = stamp.time.checked_add(context.max_age) else {
        return ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        };
    };
    if context.now > deadline {
        return ObservationFreshness::Stale {
            reason: StaleReason::Age,
        };
    }
    if context.now < stamp.time {
        return ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        };
    }
    if stamp.provenance == TimestampProvenance::Estimated {
        return ObservationFreshness::Estimated;
    }
    ObservationFreshness::Current
}

fn oracle_channel_freshness(
    trace: &ControlTrace,
    context: &FreshnessContext,
    channel: OracleChannel,
) -> ObservationFreshness {
    if let Some(stale) = oracle_context_stale(trace, context) {
        return stale;
    }
    match channel {
        OracleChannel::PluginReported => match trace.plugin_reported.present() {
            Some(evidence) => oracle_stamp_freshness(&evidence.stamp, context),
            None => oracle_slot_unavailable(&trace.plugin_reported),
        },
        OracleChannel::Observed => match trace.observed.present() {
            Some(ProbeObservation::Measuring { stamp, .. }) => {
                oracle_stamp_freshness(stamp, context)
            }
            Some(ProbeObservation::Lost { count }) => {
                ObservationFreshness::DataLoss { count: *count }
            }
            Some(ProbeObservation::Missing) => ObservationFreshness::Unavailable {
                reason: UnavailableReason::ProbeMissing,
            },
            Some(ProbeObservation::Disabled) => ObservationFreshness::Unavailable {
                reason: UnavailableReason::ProbeDisabled,
            },
            // Unknown future observation kinds fail closed.
            Some(_) => ObservationFreshness::Unavailable {
                reason: UnavailableReason::NoEvidence,
            },
            None => oracle_slot_unavailable(&trace.observed),
        },
    }
}

/// Combined freshness — the *best* (lowest-rank) channel answer; equal
/// rank prefers a slot that actually holds evidence, then the probe
/// channel.
fn oracle_freshness(trace: &ControlTrace, context: &FreshnessContext) -> ObservationFreshness {
    let observed = oracle_channel_freshness(trace, context, OracleChannel::Observed);
    let reported = oracle_channel_freshness(trace, context, OracleChannel::PluginReported);
    let observed_score = (oracle_rank(&observed), !trace.observed.is_present());
    let reported_score = (oracle_rank(&reported), !trace.plugin_reported.is_present());
    if observed_score <= reported_score {
        observed
    } else {
        reported
    }
}

/// Receipt-generation coherence, re-derived: prepared plan generation
/// must equal the key's; scheduled/applied receipts must match plan,
/// transport and clock epoch; dispatched Scheduled/Dispatched/Dropped
/// must match plan generation (Reported/Unavailable carry none).
fn oracle_is_coherent(trace: &ControlTrace) -> bool {
    let key = &trace.key;
    if let Some(evidence) = trace.prepared.present()
        && evidence.plan_generation != key.control_plan_generation
    {
        return false;
    }
    let receipt_coherent = |receipt: ReceiptEvidence| {
        receipt.plan_generation == key.control_plan_generation
            && receipt.transport_generation == key.transport_generation
            && receipt.clock_epoch == key.clock_epoch
    };
    if let Some(receipt) = trace.scheduled.present()
        && !receipt_coherent(receipt.receipt())
    {
        return false;
    }
    if let Some(receipt) = trace.applied.present()
        && !receipt_coherent(receipt.receipt())
    {
        return false;
    }
    if let Some(evidence) = trace.dispatched.present() {
        let plan_generation = match evidence {
            DispatchEvidence::Scheduled {
                plan_generation, ..
            }
            | DispatchEvidence::Dispatched {
                plan_generation, ..
            }
            | DispatchEvidence::Dropped {
                plan_generation, ..
            } => Some(*plan_generation),
            DispatchEvidence::Reported { .. } | DispatchEvidence::Unavailable { .. } => None,
            // Unknown future variants cannot be verified — fail closed.
            _ => return false,
        };
        if let Some(generation) = plan_generation
            && generation != key.control_plan_generation
        {
            return false;
        }
    }
    true
}

/// The core oracle assertion: all three freshness entry points and the
/// coherence verdict must equal the independently derived answers.
fn assert_trace_agrees(trace: &ControlTrace, context: &FreshnessContext) {
    assert_eq!(
        trace.freshness(context),
        oracle_freshness(trace, context),
        "freshness oracle mismatch"
    );
    assert_eq!(
        trace.plugin_reported_freshness(context),
        oracle_channel_freshness(trace, context, OracleChannel::PluginReported),
        "plugin_reported freshness oracle mismatch"
    );
    assert_eq!(
        trace.observed_freshness(context),
        oracle_channel_freshness(trace, context, OracleChannel::Observed),
        "observed freshness oracle mismatch"
    );
    assert_eq!(
        trace.is_coherent(),
        oracle_is_coherent(trace),
        "coherence oracle mismatch"
    );
}

// ---------------------------------------------------------------------------
// Fake plugin + GraphRuntime rig (trace_c3_05.rs style)
// ---------------------------------------------------------------------------

/// No-op processor — the trace observes the control/report path only;
/// plugin audio output is irrelevant to these tests.
struct NoopPlugin;

impl PluginProcessor for NoopPlugin {
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
        _events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        ProcessResult::CONTINUE
    }
}

impl Plugin for NoopPlugin {
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

struct NoopFactory;

impl PluginFactory for NoopFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        ruviform_plugin_core::erase(NoopPlugin)
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(NoopPlugin))
    }
}

struct Hosted {
    host: PluginHost,
    attached: ruviform_audio_plugin_bridge::Attached,
}

fn host_plugin() -> Hosted {
    let mut host = PluginHost::new();
    host.register_factory(Box::new(NoopFactory));
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
            port_format: fmt(),
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
    .expect("attach");
    Hosted { host, attached }
}

/// Mints a real `NodeId` from a trivial graph — `NodeId::new` is
/// crate-private upstream; the fabricated-span path only needs a
/// consistent id for ledger attribution.
fn mint_node_id() -> NodeId {
    let mut b = GraphBuilder::new();
    let (node, _ports) = b.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: Vec::new(),
    });
    node
}

/// Adapter over `hosted`'s control handle for the fabricated-span
/// path — consumes `attached.control`. No `GraphRuntime` drives the
/// node, so no scheduler receipts ever arrive; used where exact
/// event/metric accounting must not depend on receipt traffic.
fn fabricated_adapter(hosted: Hosted, config: TraceAdapterConfig) -> DispatchTraceAdapter {
    let dispatch =
        ParamDispatch::new(hosted.attached.control, mint_node_id(), frame_base()).unwrap();
    DispatchTraceAdapter::new(dispatch, frame_base(), config, declared_ctx())
}

/// The real-path rig: fake plugin node compiled into a `GraphRuntime`
/// so `install_plan`/`Play` produce genuine scheduler receipts, plus
/// the owning [`DispatchTraceAdapter`].
struct LedgerRig {
    _host: PluginHost,
    _runtime: GraphRuntime,
    rt: RtRuntime,
    src_port: PortId,
    adapter: DispatchTraceAdapter,
}

fn ledger_rig(config: TraceAdapterConfig) -> LedgerRig {
    let hosted = host_plugin();
    // Graph: Source → plugin processor → Sink (same shape as
    // trace_c3_05's receipt-correlation rig).
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
    let (p, pp) = b.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: hosted.attached.ports.clone(),
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
    b.connect(&conn(pref(src, sp[0]), pref(p, pp[0])));
    b.connect(&conn(pref(p, pp[1]), pref(sink, kp[0])));
    let graph = Validator::validate(&b.build()).expect("graph validates");

    let mut factory = PluginNodeFactory::new();
    factory.insert(p, hosted.attached.node);
    let compiler_config = CompilerConfig {
        max_frames: 512,
        alignment: DEFAULT_ALIGNMENT,
        max_slots: 64,
        max_channels: 8,
        max_delay_lines: 16,
        max_delay_frames: 4096,
        max_processors: 16,
    };
    let (mut runtime, rt) = GraphRuntime::new(&compiler_config, 8, 8);
    let compiled = Compiler::new(compiler_config)
        .compile(
            &graph,
            &mut factory,
            runtime.registry(),
            runtime.allocator(),
            runtime.generation() + 1,
        )
        .expect("compiles");
    runtime.publish(compiled).expect("publish");

    let dispatch = ParamDispatch::new(hosted.attached.control, p, frame_base()).unwrap();
    let adapter = DispatchTraceAdapter::new(dispatch, frame_base(), config, declared_ctx());
    LedgerRig {
        _host: hosted.host,
        _runtime: runtime,
        rt,
        src_port: sp[0],
        adapter,
    }
}

impl LedgerRig {
    /// Drives the RT runtime so the scheduler applies queued
    /// `Play`/`install_plan` commands and posts receipts into the
    /// node's report channel — the real receipt path the adapter
    /// drains on the next `observe_cycle`.
    fn drive(&mut self, blocks: usize, frames: usize) {
        let rctx = RtContext::mint();
        let zeros = vec![0.0f32; frames];
        for _ in 0..blocks {
            let mut slot = self
                .rt
                .input_writer(self.src_port, frames)
                .expect("input bound");
            for ch in 0..2 {
                slot.data.channel_mut(ch).unwrap()[..frames].copy_from_slice(&zeros);
            }
            drop(slot);
            self.rt.process(frames, &rctx).expect("process");
        }
    }

    /// One full observe cycle against a declared span/call window.
    fn observe(&mut self, s: &TransportSpan) {
        let (call_start, call_frames) = call_window(s.frames());
        self.adapter
            .observe_cycle(s, call_start, call_frames, &declared_ctx());
    }
}

/// One observe cycle for the fabricated adapter — same declared
/// span/call window contract as [`LedgerRig::observe`].
fn observe(ad: &mut DispatchTraceAdapter, s: &TransportSpan) {
    let (call_start, call_frames) = call_window(s.frames());
    ad.observe_cycle(s, call_start, call_frames, &declared_ctx());
}

// ---------------------------------------------------------------------------
// TR01 — stage independence through the real ledger path.
// ---------------------------------------------------------------------------

/// Dispatch fills `prepared`; `observe_cycle` projects `dispatched`
/// from real ledger events and attributes real scheduler receipts —
/// yet neither observation channel is synthesized, so freshness stays
/// `Unavailable{NoEvidence}` until real evidence arrives.
#[test]
#[allow(clippy::too_many_lines)] // sequential oracle scenario
fn tr01_stage_independence_real_ledger() {
    let mut rig = ledger_rig(TraceAdapterConfig::default());
    let t1 = ParameterTargetId::new(101);
    rig.adapter.bind(t1, ParamId(21), &PARAMS).unwrap();

    // No dispatch yet — the adapter honestly tracks nothing.
    assert!(rig.adapter.trace(t1).is_none());

    let plan1 = rig.adapter.dispatch(&[write(t1, 21, 256, 0.5)]).unwrap();
    let gen1 = plan1.generation;

    // After dispatch, before any observe: `prepared` is Present with
    // the staged generation; every observation channel is Missing.
    let trace = rig.adapter.trace(t1).unwrap();
    assert!(matches!(
        trace.prepared,
        TraceSlot::Present(PreparedEvidence { plan_generation }) if plan_generation == gen1
    ));
    // Every stage/observation slot stays honestly `Missing` — the
    // slots have different evidence types, so each is pinned
    // individually rather than iterated.
    assert!(matches!(trace.scheduled, TraceSlot::Missing));
    assert!(matches!(trace.applied, TraceSlot::Missing));
    assert!(matches!(trace.dispatched, TraceSlot::Missing));
    assert!(matches!(trace.plugin_reported, TraceSlot::Missing));
    assert!(matches!(trace.observed, TraceSlot::Missing));
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );
    assert!(trace.is_coherent());

    // Real path: GraphRuntime applies Play/install_plan → receipts →
    // observe_cycle projects the call window onto the ledger.
    rig.drive(4, 128);
    let s = span(gen1, 0, LoopOccurrence::new(1), 512);
    rig.observe(&s);

    let trace = rig.adapter.trace(t1).unwrap();
    // `dispatched` is the plan×span projection — *sent*, never a
    // plugin-side acknowledgment. Field-level pins against the
    // declared write (position 256, value 0.5, plan gen1).
    match &trace.dispatched {
        TraceSlot::Present(DispatchEvidence::Dispatched {
            value,
            position,
            plan_generation,
            call_start,
            frame_offset,
        }) => {
            assert_eq!(value.get(), 0.5);
            assert_eq!(*position, pos(256));
            assert_eq!(*plan_generation, gen1);
            assert_eq!(call_start.frame(), 0);
            assert_eq!(*frame_offset, 256);
        }
        other => panic!("expected Dispatched projection, got {other:?}"),
    }
    // Real scheduler receipts flowed through the report channel —
    // at least one of scheduled/applied holds receipt evidence.
    let has_receipt = matches!(trace.scheduled, TraceSlot::Present(_))
        || matches!(trace.applied, TraceSlot::Present(_));
    assert!(
        has_receipt,
        "expected a real scheduler receipt, got scheduled={:?} applied={:?}",
        trace.scheduled, trace.applied
    );
    // Stage independence: dispatch coverage + receipts still imply
    // nothing about the observation channels.
    assert!(matches!(trace.plugin_reported, TraceSlot::Missing));
    assert!(matches!(trace.observed, TraceSlot::Missing));
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        },
        "Dispatched/receipts must not synthesize observation evidence"
    );
    assert!(trace.is_coherent());

    // Configured evidence fills only its own slot — still no
    // observation channel.
    rig.adapter.record_configured(
        t1,
        ConfiguredEvidence {
            host_normalized: 0.5,
            saturated: false,
            quantized: true,
        },
    );
    let trace = rig.adapter.trace(t1).unwrap();
    assert!(matches!(
        trace.configured,
        TraceSlot::Present(ConfiguredEvidence {
            host_normalized: 0.5,
            quantized: true,
            ..
        })
    ));
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );

    // Probe channel states are recorded verbatim — `Missing` is a
    // channel state (ProbeMissing), `Lost` is data loss, never a
    // measured value.
    rig.adapter.record_probe(t1, ProbeObservation::Missing);
    let trace = rig.adapter.trace(t1).unwrap();
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ProbeMissing,
        }
    );

    rig.adapter
        .record_probe(t1, ProbeObservation::Lost { count: 3 });
    let trace = rig.adapter.trace(t1).unwrap();
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::DataLoss { count: 3 }
    );
    // DataLoss (rank 3) beats the empty plugin channel (rank 4).
    assert_eq!(
        trace.freshness(&live),
        ObservationFreshness::DataLoss { count: 3 }
    );
}

// ---------------------------------------------------------------------------
// TR03 — context changes classify Stale with the named reason; a real
// re-dispatch re-keys the trace and strands the old context.
// ---------------------------------------------------------------------------

/// A trace carrying live evidence on both observation channels goes
/// `Stale{reason}` for every moved context dimension — and a second
/// real dispatch moves `key.control_plan_generation`, so evidence
/// recorded under gen1 classifies `Stale{ControlPlan}` against the
/// gen2 context without any synthetic key edit.
#[test]
#[allow(clippy::too_many_lines)] // sequential oracle scenario
fn tr03_context_change_stale_real_ledger() {
    let mut rig = ledger_rig(TraceAdapterConfig::default());
    let t1 = ParameterTargetId::new(201);
    rig.adapter.bind(t1, ParamId(21), &PARAMS).unwrap();

    // Plan 1 → Reported entry + real receipts + a Carried probe stamp.
    let plan1 = rig.adapter.dispatch(&[write(t1, 21, 256, 0.5)]).unwrap();
    let gen1 = plan1.generation;
    assert!(
        rig.adapter
            .record_reported(t1, ParamValue::new(0.9).unwrap())
    );
    rig.drive(4, 128);
    let s1 = span(gen1, 0, LoopOccurrence::new(1), 512);
    rig.observe(&s1);
    // Probe evidence lands after the observe so its stamp position
    // becomes the trace's evaluation position.
    rig.adapter.record_probe(
        t1,
        ProbeObservation::Measuring {
            stamp: stamp_at(pos(48_000), TimestampProvenance::Carried),
            value: MeasuredValue::Signal(ParamValue::new(0.4).unwrap()),
        },
    );

    let trace = rig.adapter.trace(t1).unwrap();
    // Both observation channels hold real evidence: Reported →
    // plugin_reported (Estimated), probe → observed (Carried).
    assert!(matches!(
        trace.dispatched,
        TraceSlot::Present(DispatchEvidence::Reported { .. })
    ));
    assert!(matches!(
        trace.plugin_reported,
        TraceSlot::Present(PluginReportedEvidence { .. })
    ));
    assert!(matches!(
        trace.observed,
        TraceSlot::Present(ProbeObservation::Measuring { .. })
    ));
    assert!(trace.is_coherent());

    // Live context: probe stamp == now → Current; the Estimated
    // report stays Estimated; combined freshness is Current.
    let live = live_ctx(trace, pos(48_000));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Current
    );
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Estimated
    );
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);

    // Scenario table (mirrors project TR03): each transport-ish move
    // bumps exactly the documented context dimension; the real trace
    // must go Stale with that dimension named — never silently
    // Current. Coherence is key-internal and stays true throughout.
    let scenarios: [(FreshnessContext, StaleReason, &str); 6] = [
        (
            FreshnessContext {
                transport_generation: TransportGeneration::new(live.transport_generation.get() + 1),
                evaluation_position: pos(live.evaluation_position.value() + 1),
                ..live
            },
            StaleReason::Transport,
            "seek",
        ),
        (
            FreshnessContext {
                transport_generation: TransportGeneration::new(live.transport_generation.get() + 1),
                ..live
            },
            StaleReason::Transport,
            "pause/resume",
        ),
        (
            FreshnessContext {
                loop_occurrence: LoopOccurrence::new(live.loop_occurrence.get() + 1),
                ..live
            },
            StaleReason::Loop,
            "loop wrap",
        ),
        (
            FreshnessContext {
                clock_epoch: ClockEpochId::new(live.clock_epoch.get() + 1),
                ..live
            },
            StaleReason::ClockEpoch,
            "device restart",
        ),
        (
            FreshnessContext {
                control_plan_generation: PlanGeneration::new(
                    live.control_plan_generation.get() + 1,
                ),
                ..live
            },
            StaleReason::ControlPlan,
            "plan republish",
        ),
        (
            FreshnessContext {
                project_revision: ProjectRevision(live.project_revision.0 + 1),
                ..live
            },
            StaleReason::ProjectRevision,
            "project edit",
        ),
    ];
    for (moved, reason, label) in scenarios {
        let expected = ObservationFreshness::Stale { reason };
        assert_eq!(oracle_freshness(trace, &moved), expected, "oracle: {label}");
        assert_trace_agrees(trace, &moved);
        assert_eq!(trace.freshness(&moved), expected, "{label}");
        assert!(trace.is_coherent(), "{label}: coherence is ctx-free");
        assert!(oracle_is_coherent(trace));
    }

    // Real re-key: a second dispatch mints plan gen2, real receipts
    // re-attribute, and the trace key moves — no synthetic edit.
    let plan2 = rig.adapter.dispatch(&[write(t1, 21, 320, 0.7)]).unwrap();
    let gen2 = plan2.generation;
    assert!(
        rig.adapter
            .record_reported(t1, ParamValue::new(0.8).unwrap())
    );
    rig.drive(4, 128);
    let s2 = span(gen2, 0, LoopOccurrence::new(1), 512);
    rig.observe(&s2);
    // Re-assert the probe stamp so evaluation_position tracks the
    // measurement again after the observe side-effects.
    rig.adapter.record_probe(
        t1,
        ProbeObservation::Measuring {
            stamp: stamp_at(pos(48_000), TimestampProvenance::Carried),
            value: MeasuredValue::Signal(ParamValue::new(0.4).unwrap()),
        },
    );

    let trace = rig.adapter.trace(t1).unwrap();
    assert_eq!(trace.key.control_plan_generation, gen2);
    // The declared context (fixture pins) survives the re-key
    // unchanged — only the plan generation moved.
    assert_eq!(trace.key.project_revision, ProjectRevision(5));
    assert_eq!(trace.key.graph_generation, 11);
    assert!(matches!(
        trace.dispatched,
        TraceSlot::Present(DispatchEvidence::Reported { .. })
    ));
    let live2 = live_ctx(trace, pos(48_000));
    assert_trace_agrees(trace, &live2);
    assert_eq!(trace.freshness(&live2), ObservationFreshness::Current);
    assert!(trace.is_coherent());

    // The gen1 context is now genuinely stale — the ledger moved the
    // key, not the test. `ControlPlan` is the first mismatched
    // dimension in check order.
    assert_eq!(
        trace.freshness(&live),
        ObservationFreshness::Stale {
            reason: StaleReason::ControlPlan,
        },
        "evidence recorded under gen1 must stale against the gen2 key"
    );
    assert_trace_agrees(trace, &live);
    assert!(trace.is_coherent());
}

// ---------------------------------------------------------------------------
// TR04 — ledger Unavailable/Reported terminals vs probe channel states.
// ---------------------------------------------------------------------------

/// `mark_reporting_unavailable` produces the honest `Unavailable`
/// dispatch terminal (no report channel) while `record_reported`
/// produces `Reported` + `plugin_reported` — and probe `Missing`/
/// `Lost`/`Measuring` stay distinct channel states on the same real
/// traces.
#[test]
#[allow(clippy::too_many_lines)] // sequential oracle scenario
fn tr04_plugin_unavailable_vs_probe_states() {
    let mut rig = ledger_rig(TraceAdapterConfig::default());
    let t1 = ParameterTargetId::new(301);
    let t2 = ParameterTargetId::new(302);
    rig.adapter.bind(t1, ParamId(21), &PARAMS).unwrap();
    rig.adapter.bind(t2, ParamId(22), &PARAMS).unwrap();

    // One plan covering both targets; t2 gets a plugin report before
    // the unavailable declaration so its entry is terminal `Reported`
    // while t1's pending entry becomes `Unavailable`.
    let plan1 = rig
        .adapter
        .dispatch(&[write(t1, 21, 256, 0.5), write(t2, 22, 320, 0.6)])
        .unwrap();
    let gen1 = plan1.generation;
    assert!(
        rig.adapter
            .record_reported(t2, ParamValue::new(0.9).unwrap())
    );
    rig.adapter.mark_reporting_unavailable();
    rig.drive(4, 128);
    let s1 = span(gen1, 0, LoopOccurrence::new(1), 512);
    rig.observe(&s1);

    // t1: no report channel → dispatched=Unavailable{position}; both
    // observation channels honestly empty.
    let trace1 = rig.adapter.trace(t1).unwrap();
    match &trace1.dispatched {
        TraceSlot::Present(DispatchEvidence::Unavailable { position }) => {
            assert_eq!(*position, pos(256));
        }
        other => panic!("expected Unavailable terminal, got {other:?}"),
    }
    assert!(matches!(trace1.plugin_reported, TraceSlot::Missing));
    assert!(matches!(trace1.observed, TraceSlot::Missing));
    let live1 = live_ctx(trace1, pos(0));
    assert_trace_agrees(trace1, &live1);
    assert_eq!(
        trace1.freshness(&live1),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        },
        "Unavailable dispatch terminal implies no observation"
    );
    assert!(trace1.is_coherent());

    // t2: Reported → dispatched=Reported + plugin_reported Present
    // with an Estimated stamp (TR05 overlap, asserted in depth there).
    let trace2 = rig.adapter.trace(t2).unwrap();
    assert!(matches!(
        trace2.dispatched,
        TraceSlot::Present(DispatchEvidence::Reported { .. })
    ));
    assert!(matches!(
        trace2.plugin_reported,
        TraceSlot::Present(PluginReportedEvidence { .. })
    ));
    let live2 = live_ctx(trace2, pos(0));
    assert_trace_agrees(trace2, &live2);
    assert_eq!(
        trace2.plugin_reported_freshness(&live2),
        ObservationFreshness::Estimated
    );
    assert_eq!(
        trace2.freshness(&live2),
        ObservationFreshness::Estimated,
        "Estimated report is the best available answer so far"
    );

    // Probe channel states on t1 — each a distinct honest answer,
    // never conflated with the dispatch terminal or a measurement.
    rig.adapter.record_probe(t1, ProbeObservation::Missing);
    let trace1 = rig.adapter.trace(t1).unwrap();
    let live1 = live_ctx(trace1, pos(0));
    assert_trace_agrees(trace1, &live1);
    assert_eq!(
        trace1.observed_freshness(&live1),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ProbeMissing,
        }
    );

    rig.adapter
        .record_probe(t1, ProbeObservation::Lost { count: 5 });
    let trace1 = rig.adapter.trace(t1).unwrap();
    let live1 = live_ctx(trace1, pos(0));
    assert_trace_agrees(trace1, &live1);
    assert_eq!(
        trace1.observed_freshness(&live1),
        ObservationFreshness::DataLoss { count: 5 }
    );
    assert_eq!(
        trace1.freshness(&live1),
        ObservationFreshness::DataLoss { count: 5 }
    );

    // A trusted probe measurement on t2 outranks the Estimated
    // plugin claim — channels stay independent on the same cycle.
    rig.adapter.record_probe(
        t2,
        ProbeObservation::Measuring {
            stamp: stamp_at(pos(0), TimestampProvenance::Carried),
            value: MeasuredValue::Signal(ParamValue::new(0.4).unwrap()),
        },
    );
    let trace2 = rig.adapter.trace(t2).unwrap();
    let live2 = live_ctx(trace2, pos(0));
    assert_trace_agrees(trace2, &live2);
    assert_eq!(
        trace2.observed_freshness(&live2),
        ObservationFreshness::Current
    );
    assert_eq!(
        trace2.plugin_reported_freshness(&live2),
        ObservationFreshness::Estimated
    );
    assert_eq!(trace2.freshness(&live2), ObservationFreshness::Current);
    // t1 keeps its own verdict — the probe evidence on t2 does not
    // leak across targets.
    let trace1 = rig.adapter.trace(t1).unwrap();
    let live1 = live_ctx(trace1, pos(0));
    assert_trace_agrees(trace1, &live1);
    assert_eq!(
        trace1.freshness(&live1),
        ObservationFreshness::DataLoss { count: 5 }
    );
}

// ---------------------------------------------------------------------------
// TR05 — plugin_reported stamp provenance: Estimated only, never
// Current (adapter honesty contract).
// ---------------------------------------------------------------------------

/// The report channel carries no timestamp, so the adapter stamps
/// drain-time transport position as `Estimated` — field-level proof
/// plus the freshness matrix showing it can never classify `Current`,
/// while a trusted probe channel still can.
#[test]
#[allow(clippy::too_many_lines)] // sequential oracle scenario
fn tr05_plugin_reported_stamp_provenance() {
    let mut rig = ledger_rig(TraceAdapterConfig::default());
    let t1 = ParameterTargetId::new(401);
    rig.adapter.bind(t1, ParamId(21), &PARAMS).unwrap();

    let plan1 = rig.adapter.dispatch(&[write(t1, 21, 256, 0.5)]).unwrap();
    let gen1 = plan1.generation;
    assert!(
        rig.adapter
            .record_reported(t1, ParamValue::new(0.9).unwrap())
    );
    rig.drive(4, 128);
    let s1 = span(gen1, 0, LoopOccurrence::new(1), 512);
    rig.observe(&s1);

    // Field-level honesty pins: the adapter minted the stamp itself —
    // drain-time span position, `Estimated` provenance, the reported
    // value verbatim. It is *not* a plugin timestamp.
    let trace = rig.adapter.trace(t1).unwrap();
    let reported_stamp = match &trace.plugin_reported {
        TraceSlot::Present(PluginReportedEvidence { value, stamp }) => {
            assert_eq!(value.get(), 0.9);
            *stamp
        }
        other => panic!("expected plugin_reported Present, got {other:?}"),
    };
    assert_eq!(reported_stamp.provenance, TimestampProvenance::Estimated);
    assert_eq!(reported_stamp.time, pos(0), "drain-time span position");
    assert_eq!(reported_stamp.time, s1.ctx.position());
    assert!(matches!(
        trace.dispatched,
        TraceSlot::Present(DispatchEvidence::Reported { .. })
    ));

    // now == stamp.time exactly and context fully matching: the
    // provenance gate still yields Estimated — never Current.
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Estimated
    );
    assert_ne!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Current,
        "plugin_reported must never classify Current"
    );
    assert_eq!(trace.freshness(&live), ObservationFreshness::Estimated);

    // Within the age budget the answer stays Estimated; past the
    // deadline the same stamp degrades to Stale{Age} — age is
    // checked before provenance.
    let live = live_ctx(trace, pos(1));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Estimated
    );
    let live = live_ctx(trace, pos(200_000));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Stale {
            reason: StaleReason::Age,
        }
    );

    // A trusted probe channel outranks the Estimated claim: combined
    // freshness is Current while plugin_reported stays Estimated.
    rig.adapter.record_probe(
        t1,
        ProbeObservation::Measuring {
            stamp: stamp_at(pos(0), TimestampProvenance::Carried),
            value: MeasuredValue::Signal(ParamValue::new(0.4).unwrap()),
        },
    );
    let trace = rig.adapter.trace(t1).unwrap();
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Current
    );
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Estimated
    );
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);

    // An Estimated probe stamp keeps both channels at Estimated —
    // provenance honesty is symmetric across channels.
    rig.adapter.record_probe(
        t1,
        ProbeObservation::Measuring {
            stamp: stamp_at(pos(0), TimestampProvenance::Estimated),
            value: MeasuredValue::Signal(ParamValue::new(0.4).unwrap()),
        },
    );
    let trace = rig.adapter.trace(t1).unwrap();
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Estimated
    );
    assert_eq!(trace.freshness(&live), ObservationFreshness::Estimated);

    // A future probe stamp is ClockUntrusted on its own channel; the
    // Estimated report still answers for the combined verdict.
    rig.adapter.record_probe(
        t1,
        ProbeObservation::Measuring {
            stamp: stamp_at(pos(500_000), TimestampProvenance::Carried),
            value: MeasuredValue::Signal(ParamValue::new(0.4).unwrap()),
        },
    );
    let trace = rig.adapter.trace(t1).unwrap();
    let live = live_ctx(trace, pos(0));
    assert_trace_agrees(trace, &live);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        }
    );
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Estimated
    );
    assert_eq!(trace.freshness(&live), ObservationFreshness::Estimated);
}

// ---------------------------------------------------------------------------
// TR07 — the same probe observation on two targets: per-trace verdicts.
// ---------------------------------------------------------------------------

/// Two targets share the *identical* `ProbeObservation` bytes; each
/// trace still classifies under its own key context — the live plan
/// answers `Current`, the previous plan answers `Stale{ControlPlan}`.
#[test]
fn tr07_shared_probe_two_contexts_real_ledger() {
    let mut rig = ledger_rig(TraceAdapterConfig::default());
    let t1 = ParameterTargetId::new(501);
    let t2 = ParameterTargetId::new(502);
    rig.adapter.bind(t1, ParamId(21), &PARAMS).unwrap();
    rig.adapter.bind(t2, ParamId(22), &PARAMS).unwrap();

    // Plan 1 covers t1 only; plan 2 covers t2 only — the two traces
    // land on different plan generations through the real ledger.
    let plan1 = rig.adapter.dispatch(&[write(t1, 21, 256, 0.5)]).unwrap();
    let gen1 = plan1.generation;
    rig.drive(4, 128);
    let s1 = span(gen1, 0, LoopOccurrence::new(1), 512);
    rig.observe(&s1);

    let plan2 = rig.adapter.dispatch(&[write(t2, 22, 320, 0.6)]).unwrap();
    let gen2 = plan2.generation;
    assert_ne!(gen1, gen2);
    rig.drive(4, 128);
    let s2 = span(gen2, 0, LoopOccurrence::new(1), 512);
    rig.observe(&s2);

    // The same probe evidence recorded verbatim on both targets.
    let shared = ProbeObservation::Measuring {
        stamp: stamp_at(pos(0), TimestampProvenance::Carried),
        value: MeasuredValue::Signal(ParamValue::new(0.5).unwrap()),
    };
    rig.adapter.record_probe(t1, shared);
    rig.adapter.record_probe(t2, shared);

    let trace1 = rig.adapter.trace(t1).unwrap();
    let trace2 = rig.adapter.trace(t2).unwrap();
    // Literally the same observation bytes on both traces.
    assert_eq!(trace1.observed, trace2.observed);
    assert_eq!(trace1.key.control_plan_generation, gen1);
    assert_eq!(trace2.key.control_plan_generation, gen2);

    let ctx1 = live_ctx(trace1, pos(0));
    let ctx2 = live_ctx(trace2, pos(0));

    // Same evidence, opposite verdicts — freshness is per-trace,
    // never a global property of the observation.
    assert_trace_agrees(trace1, &ctx1);
    assert_trace_agrees(trace2, &ctx2);
    assert_trace_agrees(trace1, &ctx2);
    assert_trace_agrees(trace2, &ctx1);

    assert_eq!(
        trace1.observed_freshness(&ctx1),
        ObservationFreshness::Current
    );
    assert_eq!(
        trace2.observed_freshness(&ctx2),
        ObservationFreshness::Current
    );
    assert_eq!(
        trace1.observed_freshness(&ctx2),
        ObservationFreshness::Stale {
            reason: StaleReason::ControlPlan,
        },
        "t1's gen1 evidence is stale under t2's gen2 context"
    );
    assert_eq!(
        trace2.observed_freshness(&ctx1),
        ObservationFreshness::Stale {
            reason: StaleReason::ControlPlan,
        },
        "t2's gen2 evidence is stale under t1's gen1 context"
    );
    assert_eq!(trace1.freshness(&ctx1), ObservationFreshness::Current);
    assert_eq!(trace2.freshness(&ctx2), ObservationFreshness::Current);
    assert_eq!(
        trace1.freshness(&ctx2),
        ObservationFreshness::Stale {
            reason: StaleReason::ControlPlan,
        }
    );
    assert_eq!(
        trace2.freshness(&ctx1),
        ObservationFreshness::Stale {
            reason: StaleReason::ControlPlan,
        }
    );
}

// ---------------------------------------------------------------------------
// TR08 — bounded retention: history ring, target cap, generation-window
// expiry and the metrics that account for every loss.
// ---------------------------------------------------------------------------

/// With `history_capacity: 4`, `max_targets: 3`, `plan_gen_window: 2`,
/// a declared sequence of dispatches/observes/records produces an
/// exact event ledger: out-of-window revisions surface `Expired`,
/// LRU target eviction is counted, and every recorded event is either
/// retained or accounted as `history_lost`.
#[test]
#[allow(clippy::too_many_lines)] // sequential oracle scenario
fn tr08_bounded_retention_metrics_real_ledger() {
    let mut ad = fabricated_adapter(
        host_plugin(),
        TraceAdapterConfig {
            history_capacity: 4,
            max_targets: 3,
            plan_gen_window: 2,
        },
    );
    // t1..t5 ↔ params 21..25.
    let t: Vec<ParameterTargetId> = (0..5_u128)
        .map(|i| ParameterTargetId::new(601 + i))
        .collect();
    for (i, target) in t.iter().enumerate() {
        ad.bind(*target, ParamId(TR08_PARAMS[i]), &PARAMS).unwrap();
    }

    // Declared operation sequence (each `ad.dispatch` mints the next
    // plan generation; ledger revisions track dispatch order):
    //   d1: t1        → rev1/gen1, t1 entry Scheduled
    //   o1: span gen1 → t1 projects Dispatched(rev1)
    //   d2: t2        → rev2/gen2 (t1's Dispatched entry untouched)
    //   d3: t3        → rev3/gen3; t2's still-Scheduled entry drops PlanReplaced
    //   d4: t4        → rev4/gen4; t3's Scheduled entry drops PlanReplaced;
    //                   target cap evicts t1 (LRU)
    //   o2: span gen4 → t4 projects Dispatched(rev4); t2's rev2 fell out
    //                   of the gen window → Expired; t3's rev3 Dropped maps
    //                   Present(Dropped{PlanReplaced}); t1 evicted → its
    //                   observation is dropped with the target
    //   c5: configured t5 → target cap evicts t2; +2 events
    //   pr: probes on t3/t4/t5 → +3 events
    let g1 = ad
        .dispatch(&[write(t[0], TR08_PARAMS[0], 256, 0.5)])
        .unwrap();
    observe(
        &mut ad,
        &span(g1.generation, 0, LoopOccurrence::new(0), 512),
    );
    ad.dispatch(&[write(t[1], TR08_PARAMS[1], 256, 0.5)])
        .unwrap();
    let g3 = ad
        .dispatch(&[write(t[2], TR08_PARAMS[2], 256, 0.5)])
        .unwrap();
    let g4 = ad
        .dispatch(&[write(t[3], TR08_PARAMS[3], 256, 0.5)])
        .unwrap();
    observe(
        &mut ad,
        &span(g4.generation, 0, LoopOccurrence::new(0), 512),
    );

    ad.record_configured(
        t[4],
        ConfiguredEvidence {
            host_normalized: 0.1,
            saturated: false,
            quantized: false,
        },
    );
    ad.record_probe(t[2], ProbeObservation::Lost { count: 2 });
    ad.record_probe(
        t[3],
        ProbeObservation::Measuring {
            stamp: stamp_at(pos(0), TimestampProvenance::Carried),
            value: MeasuredValue::Signal(ParamValue::new(0.5).unwrap()),
        },
    );
    ad.record_probe(t[4], ProbeObservation::Missing);

    // --- bounded retention, field by field -------------------------
    let m = ad.metrics();
    assert_eq!(ad.history_len(), 4, "history ring holds capacity");
    assert_eq!(m.events_recorded, 14, "every pushed event counted");
    assert_eq!(m.history_lost, 10, "recorded - retained = lost");
    assert_eq!(m.history_lost, m.events_recorded - 4);
    assert!(ad.is_truncated());
    assert_eq!(ad.target_count(), 3, "target cap holds");
    assert_eq!(m.targets_lost, 2, "t1 and t2 evicted by LRU");
    assert_eq!(m.expired_correlations, 1, "rev2 fell out of window=2");
    assert_eq!(m.dropped_observations, 1, "rev3 PlanReplaced surfaced");
    assert_eq!(m.latency_samples, 0, "fabricated path has no receipts");
    assert_eq!(m.last_plan_generation, Some(g4.generation));
    assert_eq!(m.last_value, Some(0.5));

    // The retained history is the *most recent* events in push order —
    // the ring evicts oldest first: configured(t5), then the three
    // probe records.
    let history = ad.history();
    let kinds: Vec<TraceEventKind> = history.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            TraceEventKind::ConfiguredRecorded,
            TraceEventKind::ProbeRecorded,
            TraceEventKind::ProbeRecorded,
            TraceEventKind::ProbeRecorded,
        ]
    );
    assert!(history.windows(2).all(|w| w[0].seq < w[1].seq));

    // Evicted targets are honestly gone — no resurrected trace.
    assert!(ad.trace(t[0]).is_none());
    assert!(ad.trace(t[1]).is_none());

    // t3 (surviving): its rev3 entry dropped PlanReplaced and mapped
    // to Present(Dropped) — coherent with its gen3 key.
    let trace3 = ad.trace(t[2]).unwrap();
    match &trace3.dispatched {
        TraceSlot::Present(DispatchEvidence::Dropped {
            plan_generation,
            reason,
            ..
        }) => {
            assert_eq!(*reason, DispatchDropReason::PlanReplaced);
            assert_eq!(*plan_generation, g3.generation);
        }
        other => panic!("expected Dropped{{PlanReplaced}}, got {other:?}"),
    }
    assert_eq!(trace3.key.control_plan_generation, g3.generation);
    let live3 = live_ctx(trace3, pos(0));
    assert_trace_agrees(trace3, &live3);
    assert!(trace3.is_coherent());

    // t4 (surviving): rev4 projected Dispatched — field-level pins.
    let trace4 = ad.trace(t[3]).unwrap();
    match &trace4.dispatched {
        TraceSlot::Present(DispatchEvidence::Dispatched {
            value,
            position,
            plan_generation,
            frame_offset,
            ..
        }) => {
            assert_eq!(value.get(), 0.5);
            assert_eq!(*position, pos(256));
            assert_eq!(*plan_generation, g4.generation);
            assert_eq!(*frame_offset, 256);
        }
        other => panic!("expected Dispatched projection, got {other:?}"),
    }
    let live4 = live_ctx(trace4, pos(0));
    assert_trace_agrees(trace4, &live4);
    assert!(trace4.is_coherent());

    // t5: configured + probe evidence only — freshness still honestly
    // empty of measurements.
    let trace5 = ad.trace(t[4]).unwrap();
    assert!(matches!(trace5.configured, TraceSlot::Present(_)));
    assert!(matches!(
        trace5.observed,
        TraceSlot::Present(ProbeObservation::Missing)
    ));
    let live5 = live_ctx(trace5, pos(0));
    assert_trace_agrees(trace5, &live5);
    assert_eq!(
        trace5.observed_freshness(&live5),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ProbeMissing,
        }
    );

    // Deterministic repeated evaluation on the real bounded traces.
    let first = (
        trace3.freshness(&live3),
        trace3.plugin_reported_freshness(&live3),
        trace3.observed_freshness(&live3),
        trace3.is_coherent(),
    );
    for _ in 0..64 {
        assert_eq!(
            (
                trace3.freshness(&live3),
                trace3.plugin_reported_freshness(&live3),
                trace3.observed_freshness(&live3),
                trace3.is_coherent(),
            ),
            first,
            "freshness/coherence must be deterministic"
        );
    }

    // The trace remains a fixed-size projection regardless of how
    // much history the ledger retained.
    assert!(std::mem::size_of::<ControlTrace>() <= 1024);
}
