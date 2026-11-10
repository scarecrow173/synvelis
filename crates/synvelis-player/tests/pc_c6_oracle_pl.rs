//! PC-C6 oracle rows PL01–PL08 (issue #103, PC-C4 plugin gate).
//!
//! Independent-oracle fixtures for the plugin parameter binding /
//! dispatch contract: every expected outcome is derived from a
//! declared model of the write batch and the generation pins — never
//! from `PluginBindingSession`/`ParamDispatch` internals — and compared
//! field-by-field against observed typed errors, staged plans and the
//! fixture plugin's delivered-event log.
//!
//! Evidence boundary: the "plugin" is the in-process `RecPlugin`
//! recorder behind `PluginHost` — **headless fixture evidence only**.
//! Real VST3 capability, real OS scheduling and GUI focus stay `NotRun`
//! (Issue #114 / ledger `docs/test-specs/pc-c6-oracle-rows.md`).

use std::sync::{Arc, Mutex};

use ruviform_audio::typed::{Compiler, CompilerConfig, DEFAULT_ALIGNMENT, GraphRuntime};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, PluginNode, PluginNodeFactory, attach};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, PortId, PortRef,
    Validator,
};
use ruviform_media_core::{DeliveryPolicy, MediaTime, ScheduleBudget, TimeBase, TransportCommand};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, ParamWriteMeans, Plugin,
    PluginDescriptor, PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext,
    ProcessResult, TimedParamEvent, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use synvelis_player::{
    BindingError, DispatchError, ParamDispatch, PluginBindingSession, PluginParameterBinding,
    StablePluginInstanceId,
};
use synvelis_project::{
    AutomationLane, AutomationLaneId, AutomationPoint, AutomationRegion, AutomationTimeScope,
    BeatPoint, BeatRange, ControlGraph, ControlOwner, Interpolation, ParamMapping, ParameterRole,
    ParameterTarget, ParameterTargetId, PluginRef, Processor, ProcessorBinding, ProcessorId,
    Project, ProjectId, TargetCapability, ValueKind,
};

// ---------------------------------------------------------------------------
// fixture (same hosted-recorder stack as pc_c4_binding.rs)
// ---------------------------------------------------------------------------

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c6-pl",
    name: "PC-C6 PL Recorder",
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

/// Two params: `7` automatable continuous, `9` automatable+stepped.
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
        name: "Mode",
        unit: "",
        min_value: 0.0,
        max_value: 1.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED),
    },
];

/// One delivered parameter event at its absolute engine frame.
#[derive(Debug, Clone, PartialEq)]
struct Delivered {
    abs_frame: u64,
    param_id: u32,
    value: f64,
}

struct RecPlugin {
    delivered: Arc<Mutex<Vec<Delivered>>>,
    params: &'static [ParamInfo],
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
        for ev in events.iter() {
            if let EventKind::ParamValue(pe) = ev.kind() {
                #[allow(clippy::cast_sign_loss)]
                self.delivered.lock().unwrap().push(Delivered {
                    abs_frame: (pos + i64::from(ev.frame_offset())) as u64,
                    param_id: pe.param_id.0,
                    value: pe.value.get(),
                });
            }
        }
        ProcessResult::CONTINUE
    }
}

impl Plugin for RecPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        self.params
    }
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
    }
}

struct RecFactory {
    delivered: Arc<Mutex<Vec<Delivered>>>,
    params: &'static [ParamInfo],
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(RecPlugin {
            delivered: Arc::clone(&self.delivered),
            params: self.params,
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
            delivered: Arc::clone(&self.delivered),
            params: self.params,
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

const PROC: u128 = 42;

fn processor() -> ProcessorId {
    ProcessorId::new(PROC)
}

fn plugin_ref() -> PluginRef {
    PluginRef {
        descriptor: DESCRIPTOR.id.to_owned(),
        version: DESCRIPTOR.version.to_owned(),
        capability_revision: 1,
    }
}

/// A hosted fixture plugin + its `attached` parts.
struct Hosted {
    host: PluginHost,
    attached: ruviform_audio_plugin_bridge::Attached,
    delivered: Arc<Mutex<Vec<Delivered>>>,
}

fn host_plugin(params: &'static [ParamInfo]) -> Hosted {
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        delivered: Arc::clone(&delivered),
        params,
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
    Hosted {
        host,
        attached,
        delivered,
    }
}

/// `Source(2 outs) → plugin → Sink(2 ins)` — shared graph shape.
fn build_graph(
    attached: &ruviform_audio_plugin_bridge::Attached,
) -> (
    ruviform_graph::ValidatedGraph,
    NodeId,
    [PortId; 2],
    [PortId; 2],
) {
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
    (g, proc, [sp[0], sp[0]], [kp[0], kp[0]])
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

/// A `ProcessorParam` target owned by `processor()` whose declared
/// capability matches `PARAMS[0]` (param 7, automatable).
fn param_target(
    id: u128,
    param: u32,
    flags: ParamFlags,
    kind: ValueKind,
    base: f64,
) -> ParameterTarget {
    ParameterTarget {
        id: ParameterTargetId::new(id),
        owner: ControlOwner::Processor(processor()),
        role: ParameterRole::ProcessorParam,
        time_scope: AutomationTimeScope::ProjectMusical,
        name: "P".into(),
        kind,
        mapping: ParamMapping::Linear,
        capability: Some(TargetCapability {
            param_id: param,
            flags_bits: flags.bits(),
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
        }),
        base,
    }
}

fn gain_target(id: u128) -> ParameterTarget {
    param_target(id, 7, ParamFlags::AUTOMATABLE, ValueKind::Continuous, 0.0)
}

/// A project carrying `targets` plus the `Resolved` processor
/// registration the stage-1 semantic binding requires.
fn project_with(targets: Vec<ParameterTarget>) -> Project {
    let mut project = Project::new(ProjectId::new(0), "pcc6pl");
    for target in targets {
        project.control.targets.insert(target.id, target);
    }
    project.routing.processors.insert(
        processor(),
        Processor {
            id: processor(),
            name: "fx".into(),
            binding: ProcessorBinding::Resolved(plugin_ref()),
            port_ids: vec![],
        },
    );
    project
}

/// Opens a session + dispatcher on `control`.
fn open(
    control: ruviform_audio_plugin_bridge::PluginNodeControl,
    node: NodeId,
    instance: u64,
    cap_gen: u64,
    graph_gen: u64,
) -> (PluginBindingSession, ParamDispatch) {
    let session = PluginBindingSession::attach(
        &control,
        node,
        processor(),
        plugin_ref(),
        graph_gen,
        cap_gen,
        StablePluginInstanceId::new(instance),
    );
    let dispatch = ParamDispatch::new(control, node, frame_base()).expect("frame base ok");
    (session, dispatch)
}

/// Expected delivered stream for `events` — derived from the declared
/// write batch only (absolute frame = position frame).
fn expected(events: &[TimedParamEvent]) -> Vec<Delivered> {
    events
        .iter()
        .map(|e| Delivered {
            abs_frame: u64::try_from(e.position.value()).unwrap(),
            param_id: e.event.param_id.0,
            value: e.event.value.get(),
        })
        .collect()
}

/// Full playback drive: compile `graph` with the plugin node, publish,
/// run 3×128-frame blocks. Delivered events land in the fixture's
/// shared log.
fn playback_delivered(
    graph: &ruviform_graph::ValidatedGraph,
    node: PluginNode,
    proc: NodeId,
    srcs: [PortId; 2],
) {
    let mut factory = PluginNodeFactory::new();
    factory.insert(proc, node);
    let (mut runtime, mut rt) = GraphRuntime::new(&compiler_config(), 8, 8);
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
    let ctx = RtContext::mint();
    let zeros = vec![0.0f32; 128];
    for _ in 0..3 {
        let mut slot = rt.input_writer(srcs[0], 128).expect("input bound");
        for ch in 0..2 {
            slot.data.channel_mut(ch).unwrap()[..128].copy_from_slice(&zeros);
        }
        drop(slot);
        rt.process(128, &ctx).expect("process");
    }
}

/// A bound session triple: hosted plugin, graph, session+dispatch and
/// the project's eval plan — the standard oracle fixture opening. The
/// `PluginNode`/`PluginHost`/delivery log are stored separately from
/// `Hosted` so `deliver` can consume the node exactly once.
struct Rig {
    _host: PluginHost,
    delivered: Arc<Mutex<Vec<Delivered>>>,
    node: Option<PluginNode>,
    graph: ruviform_graph::ValidatedGraph,
    proc: NodeId,
    srcs: [PortId; 2],
    project: Project,
    plan: synvelis_project::EvalPlan,
    session: PluginBindingSession,
    dispatch: ParamDispatch,
    target: ParameterTargetId,
    binding: PluginParameterBinding,
}

fn open_rig(target_id: u128) -> Rig {
    let hosted = host_plugin(&PARAMS);
    let (graph, proc, srcs, _sinks) = build_graph(&hosted.attached);
    let project = project_with(vec![gain_target(target_id)]);
    let plan = ControlGraph::compile(&project).expect("compiles");
    let control = hosted.attached.control;
    let node = hosted.attached.node;
    let (mut session, mut dispatch) = open(control, proc, 1, 1, 1);
    let binding = session
        .bind(
            &mut dispatch,
            &project,
            ParameterTargetId::new(target_id),
            &PARAMS,
        )
        .unwrap();
    Rig {
        _host: hosted.host,
        delivered: hosted.delivered,
        node: Some(node),
        graph,
        proc,
        srcs,
        project,
        plan,
        session,
        dispatch,
        target: ParameterTargetId::new(target_id),
        binding,
    }
}

impl Rig {
    /// Evaluates `target` at `pos`/`manual` into a `TimedParamEvent`
    /// scheduled at `frame`.
    fn event(&mut self, pos: BeatPoint, manual: Option<f64>, frame: i64) -> TimedParamEvent {
        let mut scratch = self.plan.scratch();
        self.session
            .eval_event(
                &self.project,
                &self.plan,
                &mut scratch,
                self.target,
                pos,
                manual,
                MediaTime::new(frame, frame_base()),
                ParamWriteMeans::TimedEvent,
            )
            .unwrap()
    }

    /// Dispatches `events` through the session and drives playback;
    /// returns the plugin's delivered log snapshot. One-shot: the
    /// `PluginNode` is consumed by the compiled graph.
    fn deliver(&mut self, events: &[TimedParamEvent]) -> Vec<Delivered> {
        let writes: Vec<(PluginParameterBinding, TimedParamEvent)> =
            events.iter().map(|e| (self.binding.clone(), *e)).collect();
        self.session.dispatch(&mut self.dispatch, &writes).unwrap();
        let node = self.node.take().expect("node consumed once");
        playback_delivered(&self.graph, node, self.proc, self.srcs);
        self.delivered.lock().unwrap().clone()
    }
}

// ---------------------------------------------------------------------------
// PL01 — verified pins: frame base, instance, param id, capability and
// graph generations are all re-checked at dispatch; any drift is a
// typed reject and stages nothing.
// ---------------------------------------------------------------------------

/// Oracle model: one declared write batch; the oracle predicts the
/// accepted/rejected outcome class per pin mutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PinAxis {
    Instance,
    CapabilityGeneration,
    GraphGeneration,
    PluginIdentity,
    DefunctRecord,
}

#[test]
fn pc_c6_pl01_generation_and_identity_pins_reject_stale_records() {
    let mut rig = open_rig(1);
    let ev = rig.event(BeatPoint::ZERO, Some(0.5), 64);

    // Baseline: the unmutated record dispatches — oracle: Accepted.
    let writes = [(rig.binding.clone(), ev)];
    rig.session.dispatch(&mut rig.dispatch, &writes).unwrap();

    // Each single-axis mutation is a typed reject; the oracle maps
    // axis → expected variant and nothing may be staged.
    let cases: [(PinAxis, PluginParameterBinding); 5] = [
        (
            PinAxis::Instance,
            PluginParameterBinding {
                instance_id: StablePluginInstanceId::new(99),
                ..rig.binding.clone()
            },
        ),
        (
            PinAxis::CapabilityGeneration,
            PluginParameterBinding {
                capability_generation: 2,
                ..rig.binding.clone()
            },
        ),
        (
            PinAxis::GraphGeneration,
            PluginParameterBinding {
                graph_generation: 2,
                ..rig.binding.clone()
            },
        ),
        (
            PinAxis::PluginIdentity,
            PluginParameterBinding {
                plugin: PluginRef {
                    descriptor: "org.other.plugin".into(),
                    ..plugin_ref()
                },
                ..rig.binding.clone()
            },
        ),
        // A record whose plugin_param differs from the session's
        // recorded binding → BindingDefunct (record drift).
        (
            PinAxis::DefunctRecord,
            PluginParameterBinding {
                plugin_param: ParamId(9),
                ..rig.binding.clone()
            },
        ),
    ];
    for (axis, record) in cases {
        let err = rig
            .session
            .dispatch(&mut rig.dispatch, &[(record, ev)])
            .unwrap_err();
        let ok = matches!(
            (axis, &err),
            (PinAxis::Instance, BindingError::InstanceMismatch)
                | (
                    PinAxis::CapabilityGeneration,
                    BindingError::CapabilityGeneration {
                        expected: 1,
                        actual: 2,
                    }
                )
                | (
                    PinAxis::GraphGeneration,
                    BindingError::GraphGeneration {
                        expected: 1,
                        actual: 2,
                    }
                )
                | (PinAxis::PluginIdentity, BindingError::PluginMismatch { .. })
                | (PinAxis::DefunctRecord, BindingError::BindingDefunct)
        );
        assert!(ok, "{axis:?} must reject typed, got {err:?}");
    }
    // FrameBase axis: a dispatcher built on a foreign base is refused
    // at construction — the session can never pair with it.
    let hosted_f = host_plugin(&PARAMS);
    let (_gf, proc_f, _sf, _kf) = build_graph(&hosted_f.attached);
    let foreign = TimeBase::sample_rate(44_100).unwrap();
    let err = ParamDispatch::new(hosted_f.attached.control, proc_f, foreign)
        .err()
        .expect("foreign frame base must be rejected");
    assert!(matches!(err, DispatchError::FrameBaseMismatch(_)));
}

// ---------------------------------------------------------------------------
// PL02 — param identity: event param_id must equal the pinned binding;
// live-enumeration drift (removed param, capability drift, read-only,
// owner mismatch) fails closed at bind/revalidate.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_pl02_param_identity_and_capability_contract() {
    let mut rig = open_rig(1);

    // Event addressed at param 9 while the binding pins param 7 →
    // param_mismatch; nothing staged, nothing delivered.
    let mut wrong = rig.event(BeatPoint::ZERO, Some(0.5), 64);
    wrong.event.param_id = ParamId(9);
    let err = rig
        .session
        .dispatch(&mut rig.dispatch, &[(rig.binding.clone(), wrong)])
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::Dispatch(DispatchError::WriteRejected { ref rejection, .. })
            if rejection == "param_mismatch"
    ));
    // Rejected before staging: no plan was ever installed.
    assert_eq!(rig.dispatch.last_plan_generation(), None);
    assert!(rig.delivered.lock().unwrap().is_empty());

    // A literal event (no session eval needed — the binding is gone).
    let ev = TimedParamEvent {
        position: MediaTime::new(64, frame_base()),
        event: ParamEvent {
            param_id: ParamId(7),
            value: ParamValue::new(0.5).unwrap(),
        },
    };

    // Live enumeration without param 7 → revalidate flips defunct,
    // session drops the record, dispatch is TargetNotBound, re-bind is
    // ParamRemoved — three distinct typed outcomes, zero delivery.
    assert!(
        !rig.session
            .revalidate(&mut rig.dispatch, rig.target, &PARAMS[1..])
    );
    let err = rig
        .session
        .dispatch(&mut rig.dispatch, &[(rig.binding.clone(), ev)])
        .unwrap_err();
    assert!(matches!(err, BindingError::TargetNotBound));
    let err = rig
        .session
        .bind(&mut rig.dispatch, &rig.project, rig.target, &PARAMS[1..])
        .unwrap_err();
    assert!(matches!(err, BindingError::ParamRemoved));
    assert!(rig.delivered.lock().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// PL03 — QueueFull and atomic batches: a full bounded queue rejects
// typed; a batch with one stale record rejects whole — partial
// delivery is impossible by construction.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_pl03_queue_full_and_atomic_batch_rejection() {
    let mut rig = open_rig(6);
    let ev = rig.event(BeatPoint::ZERO, Some(0.5), 64);

    // Flood the bounded control channel (capacity 64 upstream) until
    // the queue reports full — oracle: at least one request fails.
    let mut flooded = false;
    for _ in 0..16_384 {
        if rig
            .dispatch
            .control()
            .request(TransportCommand::Play, 0)
            .is_err()
        {
            flooded = true;
            break;
        }
    }
    assert!(flooded, "bounded queue must report QueueFull");
    let err = rig
        .session
        .dispatch(&mut rig.dispatch, &[(rig.binding.clone(), ev)])
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::Dispatch(DispatchError::QueueFull)
    ));
    assert!(rig.delivered.lock().unwrap().is_empty());

    // Fresh rig for the atomic-batch case: one good + one stale record
    // → whole batch rejected before staging; delivered stays empty.
    let mut rig2 = open_rig(7);
    let good = rig2.event(BeatPoint::ZERO, Some(0.5), 64);
    let stale = PluginParameterBinding {
        capability_generation: 9,
        ..rig2.binding.clone()
    };
    let err = rig2
        .session
        .dispatch(
            &mut rig2.dispatch,
            &[(rig2.binding.clone(), good), (stale, good)],
        )
        .unwrap_err();
    assert!(matches!(err, BindingError::CapabilityGeneration { .. }));
    assert!(rig2.delivered.lock().unwrap().is_empty());
    // And an empty batch is a typed Empty, never a staged no-op plan.
    let err = rig2.session.dispatch(&mut rig2.dispatch, &[]).unwrap_err();
    assert!(matches!(err, BindingError::Dispatch(DispatchError::Empty)));
}

// ---------------------------------------------------------------------------
// PL04 — GraphSwap/Reload: invalidate drops every binding; a re-attached
// session mints a new instance id. Records from the old generation
// can never dispatch through the new session — old-target misdelivery
// stays 0 while the new binding delivers only to its own log.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_pl04_reload_mints_new_instance_old_records_dead() {
    let mut rig = open_rig(1);
    let ev = rig.event(BeatPoint::ZERO, Some(0.5), 64);

    // Invalidate (plugin reload / graph swap path): session bindings
    // and dispatch table both drop.
    rig.session.invalidate(&mut rig.dispatch);
    assert!(rig.session.bindings().next().is_none());
    let err = rig
        .session
        .dispatch(&mut rig.dispatch, &[(rig.binding.clone(), ev)])
        .unwrap_err();
    assert!(matches!(err, BindingError::TargetNotBound));

    // Re-attach as a *new* instance on the same node control: the
    // session pins instance 2 and fresh generations.
    let control = rig.dispatch.control();
    let mut session2 = PluginBindingSession::attach(
        control,
        rig.proc,
        processor(),
        plugin_ref(),
        2,
        2,
        StablePluginInstanceId::new(2),
    );
    let binding2 = session2
        .bind(&mut rig.dispatch, &rig.project, rig.target, &PARAMS)
        .unwrap();
    assert_eq!(binding2.instance_id, StablePluginInstanceId::new(2));
    assert_eq!(binding2.capability_generation, 2);
    assert_eq!(binding2.graph_generation, 2);

    // The old-generation record is dead on every axis — oracle:
    // InstanceMismatch first (record.instance_id != session pin).
    let err = session2
        .dispatch(&mut rig.dispatch, &[(rig.binding.clone(), ev)])
        .unwrap_err();
    assert!(matches!(err, BindingError::InstanceMismatch));

    // The new record delivers; the fixture log receives exactly the
    // declared events — and only through the live binding.
    let writes = [(binding2.clone(), ev)];
    session2.dispatch(&mut rig.dispatch, &writes).unwrap();
    let node = rig.node.take().expect("node consumed once");
    playback_delivered(&rig.graph, node, rig.proc, rig.srcs);
    let got = rig.delivered.lock().unwrap().clone();
    assert_eq!(got, expected(&[ev]));
    // A second dispatch of the stale record still cannot deliver.
    let err = session2
        .dispatch(&mut rig.dispatch, &[(rig.binding.clone(), ev)])
        .unwrap_err();
    assert!(matches!(err, BindingError::InstanceMismatch));
}

// ---------------------------------------------------------------------------
// PL05 (non-RT half) — min process frame offset: events at
// non-block-aligned frames land at exactly their declared absolute
// frame through the real `PluginNode` delivery path.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_pl05_min_process_frame_offset_delivery() {
    let mut rig = open_rig(1);
    // Frames 100 and 320 sit inside 128-frame blocks but off every
    // block boundary — the scheduler must preserve the offset.
    let evs = [
        rig.event(BeatPoint::ZERO, Some(0.25), 100),
        rig.event(BeatPoint::new(1, 1).unwrap(), Some(0.75), 320),
    ];
    let got = rig.deliver(&evs);
    assert_eq!(got, expected(&evs));
    assert_eq!(got[0].abs_frame, 100);
    assert_eq!(got[1].abs_frame, 320);
}

// ---------------------------------------------------------------------------
// PL06 — plan staleness: check_plan detects any control-surface drift;
// the two generation axes (capability vs graph) are independent; a
// stale EvalPlan can never vend events.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_pl06_plan_staleness_and_generation_axes() {
    let mut rig = open_rig(1);
    rig.session.check_plan(&rig.plan, &rig.project).unwrap();
    assert!(rig.session.is_current(1, 1));
    assert!(!rig.session.is_current(2, 1));
    assert!(!rig.session.is_current(1, 2));

    // Mutating the control surface (adding an automation lane) makes
    // the compiled plan stale — oracle: PlanStale, eval refused.
    let target = rig.target;
    rig.project.control.lanes.insert(
        AutomationLaneId::new(1),
        AutomationLane {
            id: AutomationLaneId::new(1),
            target,
            regions: vec![AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(0, 1).unwrap(),
                        value: 0.2,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(4, 1).unwrap(),
                        value: 0.6,
                    },
                ],
            }],
        },
    );
    let err = rig.session.check_plan(&rig.plan, &rig.project).unwrap_err();
    assert!(matches!(err, BindingError::PlanStale));
    // eval_event does not re-check plan freshness itself — the
    // documented orchestration-boundary gate is `check_plan`; the
    // oracle asserts only that verdict plus zero delivery.
    assert!(rig.delivered.lock().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// PL07 — old-target misdelivery stays 0: a second target on the same
// param is refused (param_in_use); after rebind the stale record can
// never reach either plugin instance's log.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_pl07_shared_param_and_stale_record_zero_misdelivery() {
    // Two targets declaring param 7: the second bind is refused —
    // the ledger attributes by (node, param), so a shared binding
    // would silently misattribute writes.
    let hosted = host_plugin(&PARAMS);
    let (_graph, proc, _s, _k) = build_graph(&hosted.attached);
    let project = project_with(vec![gain_target(10), gain_target(11)]);
    let (mut session, mut dispatch) = open(hosted.attached.control, proc, 1, 1, 1);
    session
        .bind(&mut dispatch, &project, ParameterTargetId::new(10), &PARAMS)
        .unwrap();
    let err = session
        .bind(&mut dispatch, &project, ParameterTargetId::new(11), &PARAMS)
        .unwrap_err();
    assert!(matches!(
        err,
        BindingError::Dispatch(DispatchError::WriteRejected { ref rejection, .. })
            if rejection == "param_in_use"
    ));
    // A target owned by a different processor can never bind.
    let foreign = ParameterTarget {
        owner: ControlOwner::Project,
        ..gain_target(12)
    };
    let project2 = project_with(vec![foreign]);
    let hosted2 = host_plugin(&PARAMS);
    let (_g2, proc2, _s2, _k2) = build_graph(&hosted2.attached);
    let (mut session2, mut dispatch2) = open(hosted2.attached.control, proc2, 1, 1, 1);
    let err = session2
        .bind(
            &mut dispatch2,
            &project2,
            ParameterTargetId::new(12),
            &PARAMS,
        )
        .unwrap_err();
    assert!(matches!(err, BindingError::OwnerMismatch));
    // No binding was ever minted → nothing can be dispatched or
    // delivered on either instance.
    assert!(hosted.delivered.lock().unwrap().is_empty());
    assert!(hosted2.delivered.lock().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// PL08 — evidence boundary: this fixture's "plugin" is the in-process
// recorder. The test asserts the boundary itself — every delivered
// record came through `RecPlugin`, no VST3/OS path exists here — so
// the ledger can never mistake this Pass for real-plugin evidence.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_pl08_headless_fixture_boundary_marker() {
    let mut rig = open_rig(1);
    let ev = rig.event(BeatPoint::ZERO, Some(0.5), 64);
    let got = rig.deliver(&[ev]);
    // Exactly the declared events, nothing more — the only delivery
    // path exercised is the headless fixture's EventList log.
    assert_eq!(got, expected(&[ev]));
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].param_id, 7);
    // Real-plugin capability rows stay NotRun — recorded in
    // docs/test-specs/pc-c6-oracle-rows.md, never asserted here.
}
