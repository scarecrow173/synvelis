//! PC-C3-05 RT contract evidence (TRACE-C3-05): the trace adapter's
//! hot path — `record_configured` / `record_probe` / `trace()` /
//! `metrics()` — performs no heap allocation or deallocation once
//! warm, measured with `stats_alloc` (same method as the ruviform
//! RT contract tests; setup and warm-up stay outside the window).
//!
//! `observe_cycle`/`dispatch`/`bind` are non-RT orchestration paths
//! by contract and are deliberately excluded from the window.

use std::alloc::System;

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, attach};
use ruviform_graph::{GraphBuilder, NodeDef, NodeKind};
use ruviform_media_core::{
    DeliveryPolicy, MediaTime, ScheduleBudget, TimeBase, TimestampProvenance,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList,
    ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor, PluginFactory,
    PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo,
    erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_player::ParamDispatch;
use synvelis_player::trace::{DispatchTraceAdapter, TraceAdapterConfig, TraceContext};
use synvelis_project::trace::{ConfiguredEvidence, MeasuredValue, ProbeObservation};
use synvelis_project::{ParameterTargetId, ProjectId, ProjectRevision};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c3-trace-rt",
    name: "PC-C3 Trace RT",
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

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

fn adapter() -> DispatchTraceAdapter {
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
            port_format: AudioFormat {
                sample_rate: SampleRate::new(48_000).unwrap(),
                layout: ChannelLayout::Stereo,
                sample_format: SampleFormat::F32,
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
    .expect("attach");
    // The PluginHost must outlive the control channel — leak it for
    // the test's lifetime (single-test file, process-scoped).
    std::mem::forget(host);
    let mut b = GraphBuilder::new();
    let (node, _ports) = b.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: Vec::new(),
    });
    let dispatch = ParamDispatch::new(attached.control, node, frame_base()).unwrap();
    DispatchTraceAdapter::new(
        dispatch,
        frame_base(),
        TraceAdapterConfig::default(),
        TraceContext {
            project_id: ProjectId::new(1),
            project_revision: ProjectRevision(1),
            graph_generation: 1,
        },
    )
}

#[test]
fn rt_trace_adapter_hot_path_alloc_free() {
    let mut ad = adapter();
    let target = ParameterTargetId::new(1);
    // Warm-up: first-touch BTreeMap node, ring capacity, lazy paths —
    // all outside the measured window.
    ad.record_configured(
        target,
        ConfiguredEvidence {
            host_normalized: 0.5,
            saturated: false,
            quantized: false,
        },
    );
    ad.record_probe(
        target,
        ProbeObservation::Measuring {
            stamp: ruviform_media_core::MediaTimestamp {
                time: MediaTime::new(0, frame_base()),
                provenance: TimestampProvenance::Estimated,
            },
            value: MeasuredValue::Signal(ParamValue::new(0.5).unwrap()),
        },
    );
    let _ = ad.trace(target);
    let _ = ad.metrics();

    {
        let region = Region::new(ALLOC);
        for i in 0..64u64 {
            ad.record_configured(
                target,
                ConfiguredEvidence {
                    host_normalized: 0.5,
                    saturated: false,
                    quantized: false,
                },
            );
            ad.record_probe(target, ProbeObservation::Lost { count: i });
            let trace = ad.trace(target);
            assert!(trace.is_some());
            let _ = ad.metrics();
            let _ = ad.history_len();
        }
        let stats = region.change();
        assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
        assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    }
}
