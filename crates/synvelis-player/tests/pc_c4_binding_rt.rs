//! PC-C4 plugin-binding RT fixture (issue #101): the *session-bound*
//! dispatch path — canonical `plugin_parameter_instance_binding`
//! resolution at `PluginBindingSession::bind`, verified
//! `AttachClockContext`, `eval_event` → `session.dispatch` staging —
//! delivers staged `TimedParamEvent`s through the real `PluginNode`
//! hot path with zero heap allocation once warm, measured with
//! `stats_alloc` (same method as `pc_c6_oracle_pl_rt`).
//!
//! This file is a separate test binary on purpose: `stats_alloc`'s
//! `Region` observes the process-global allocator, so the measured
//! window must not share a process with other tests. Evidence is
//! **headless fixture** only — real-plugin RT behaviour stays
//! `NotRun`.

use std::alloc::System;
use std::sync::{Arc, Mutex};

use ruviform_audio::typed::{Compiler, CompilerConfig, DEFAULT_ALIGNMENT, GraphRuntime, RtRuntime};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, PluginNode, PluginNodeFactory, attach};
use ruviform_graph::{
    ConnDef, Direction, EdgeDelay, GraphBuilder, NodeDef, NodeId, NodeKind, PortId, PortRef,
    ValidatedGraph, Validator,
};
use ruviform_media_core::{DeliveryPolicy, MediaTime, ScheduleBudget, TimeBase};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamFlags, ParamId, ParamInfo, Plugin, PluginDescriptor, PluginFactory,
    PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo,
    erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use ruviform_rt::RtContext;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_player::{ParamDispatch, PluginBindingSession, StablePluginInstanceId};
use synvelis_project::{
    AutomationTimeScope, BeatPoint, ControlGraph, ControlOwner, ParamMapping, ParameterRole,
    ParameterTarget, ParameterTargetId, PluginRef, Processor, ProcessorBinding, ProcessorId,
    Project, ProjectId, TargetCapability, ValueKind,
};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c4-bind-rt",
    name: "PC-C4 Bind RT Recorder",
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

const LOG_CAPACITY: usize = 8192;

#[derive(Debug, Clone, PartialEq)]
struct Delivered {
    abs_frame: u64,
    param_id: u32,
    value: f64,
}

struct RecPlugin {
    delivered: Arc<Mutex<Vec<Delivered>>>,
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
        let mut log = self.delivered.lock().unwrap();
        for ev in events.iter() {
            if let EventKind::ParamValue(pe) = ev.kind() {
                #[allow(clippy::cast_sign_loss)]
                log.push(Delivered {
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
        &PARAMS
    }
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
    }
}

struct RecFactory {
    delivered: Arc<Mutex<Vec<Delivered>>>,
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(RecPlugin {
            delivered: Arc::clone(&self.delivered),
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
            delivered: Arc::clone(&self.delivered),
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

fn plugin_ref() -> PluginRef {
    PluginRef {
        descriptor: DESCRIPTOR.id.to_owned(),
        version: DESCRIPTOR.version.to_owned(),
        capability_revision: 1,
    }
}

fn processor() -> ProcessorId {
    ProcessorId::new(42)
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

/// The compiled-graph RT drive: input writer + process.
struct Drive {
    rt: RtRuntime,
    ctx: RtContext,
    src: PortId,
    zeros: Vec<f32>,
}

impl Drive {
    fn new(graph: &ValidatedGraph, node: PluginNode, proc: NodeId, srcs: [PortId; 2]) -> Self {
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

/// A project whose processor carries a `Resolved` binding, a synced
/// persisted instance identity, and one capability-declared plugin
/// param target — the full stage-1/1.5 authority `bind` verifies.
fn project() -> (Project, ParameterTargetId) {
    let target_id = ParameterTargetId::new(1);
    let mut project = Project::new(ProjectId::new(0), "pcc4rt");
    project.control.targets.insert(
        target_id,
        ParameterTarget {
            id: target_id,
            owner: ControlOwner::Processor(processor()),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            name: "Gain".into(),
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
        .plugin_instances
        .ensure(processor(), &plugin_ref())
        .unwrap();
    (project, target_id)
}

/// `Source(2 outs) → plugin → Sink(2 ins)` — the shared graph shape.
fn build_graph(
    attached: &ruviform_audio_plugin_bridge::Attached,
) -> (ValidatedGraph, NodeId, [PortId; 2]) {
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
    let graph = Validator::validate(&b.build()).expect("graph validates");
    (graph, proc, [sp[0], sp[0]])
}

/// Hosted recorder + compiled graph drive + bound session triple.
struct Rig {
    _host: PluginHost,
    delivered: Arc<Mutex<Vec<Delivered>>>,
    drive: Drive,
    project: Project,
    plan: synvelis_project::EvalPlan,
    session: PluginBindingSession,
    dispatch: ParamDispatch,
    binding: synvelis_player::PluginParameterBinding,
}

fn rig(project: Project, target_id: ParameterTargetId) -> Rig {
    let delivered = Arc::new(Mutex::new(Vec::with_capacity(LOG_CAPACITY)));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        delivered: Arc::clone(&delivered),
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
    let (graph, proc, srcs) = build_graph(&attached);

    let plan = ControlGraph::compile(&project).expect("compiles");
    let instance = project
        .plugin_instances
        .resolve(processor(), &plugin_ref())
        .expect("store vouches");
    let node = attached.node;
    let session = PluginBindingSession::attach(
        &attached.control,
        proc,
        processor(),
        plugin_ref(),
        1,
        1,
        StablePluginInstanceId::new(instance.get()),
    );
    let mut dispatch = ParamDispatch::new(attached.control, proc, frame_base()).unwrap();
    let mut session = session;
    let binding = session
        .bind(&mut dispatch, &project, target_id, &PARAMS)
        .expect("canonical binding binds");
    let drive = Drive::new(&graph, node, proc, srcs);
    Rig {
        _host: host,
        delivered,
        drive,
        project,
        plan,
        session,
        dispatch,
        binding,
    }
}

/// Session-bound delivery: eval → verified dispatch staging → real
/// `PluginNode` delivery performs no heap allocation once warm —
/// the canonical instance authority + attach-time frame base add no
/// RT cost to the hot path.
#[test]
fn rt_pc_c4_session_bound_dispatch_alloc_free() {
    let (project, target_id) = project();
    let mut rig = rig(project, target_id);
    let project = &rig.project;
    let plan = &rig.plan;

    // Stage once (non-RT): three events off every 256-frame boundary,
    // evaluated through the verified session path.
    let mut scratch = plan.scratch();
    let mut writes = Vec::new();
    for (beat, frame) in [
        (BeatPoint::ZERO, 10_i64),
        (BeatPoint::new(1, 1).unwrap(), 130),
        (BeatPoint::new(2, 1).unwrap(), 300),
    ] {
        let ev = rig
            .session
            .eval_event(
                project,
                plan,
                &mut scratch,
                target_id,
                beat,
                None,
                MediaTime::new(frame, frame_base()),
                ruviform_plugin_core::ParamWriteMeans::TimedEvent,
            )
            .unwrap();
        writes.push((rig.binding.clone(), ev));
    }
    rig.session.dispatch(&mut rig.dispatch, &writes).unwrap();

    // Warm-up outside the measured window.
    for _ in 0..8 {
        rig.drive.process(256);
    }
    assert!(!rig.delivered.lock().unwrap().is_empty());

    {
        let region = Region::new(ALLOC);
        for _ in 0..64 {
            rig.drive.process(256);
        }
        let stats = region.change();
        assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
        assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    }

    // Every declared event landed at its exact absolute frame.
    let log = rig.delivered.lock().unwrap();
    let mut got: Vec<(u64, u32, u64)> = log
        .iter()
        .map(|d| (d.abs_frame, d.param_id, d.value.to_bits()))
        .collect();
    got.sort_unstable();
    assert_eq!(got.len(), 3, "delivered: {got:?}");
    assert_eq!(got[0].0, 10);
    assert_eq!(got[1].0, 130);
    assert_eq!(got[2].0, 300);
    assert!(got.iter().all(|(_, p, _)| *p == 7));
}
