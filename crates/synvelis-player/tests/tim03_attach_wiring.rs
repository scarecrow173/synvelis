//! TIM-03 attach-wiring acceptance tests — Issue #127 slice 2
//! (headless mock; spec: `docs/specs/tim03-process-boundary.md`,
//! test spec rules PB-14..PB-16 in
//! `docs/test-specs/tim03-process-boundary.md`).
//!
//! Two layers:
//!
//! - `tim03_aw_delivery_*` — `ProcessCapability::attach_delivery`,
//!   the composition-root projection of capability + execution
//!   intent + consent/budget onto `AttachConfig::{policy, budget}`:
//!   Exact default, consented Degraded with `min(consent,
//!   budget.max_deferral)` clamping, `OfflineExact` certified-precision
//!   gate, typed `Unsupported`/`OfflineUncertified`.
//! - `tim03_aw_connect_*` — `PluginHostProcessors::connect` end to
//!   end: a registered [`BoundaryDeclaration`] gates the attach
//!   (typed `CapabilityUnsupported`/`OfflineUncertified` before any
//!   instance is minted), consented realtime attaches succeed,
//!   undeclared processors keep the legacy Exact attach
//!   (non-regression).
//!
//! The activated `ProcessConfig` is `(min_frames=1, max_frames)` so
//! every nonzero span is legally partitionable — Exact vs Degraded
//! is behaviorally indistinguishable at the physical-call layer in
//! this slice; the projection itself is pinned by the
//! `tim03_aw_delivery_*` tests and the connect gates by
//! `tim03_aw_connect_*`. Real CLAP/VST3/OS validation stays `NotRun`
//! (separate gate).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_graph::{GraphBuilder, NodeDef, NodeId, NodeKind};
use ruviform_media_core::{BlockContract, DeliveryPolicy, InBlockTransportSupport, ScheduleBudget};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList,
    ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError,
    ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;
use synvelis_player::{
    BoundaryDeclaration, DegradedConsent, ExecutionMode, PluginHostProcessors,
    ProcessBoundaryError, ProcessCapability, ProcessorConnectError, ProcessorConnectRequest,
    ProcessorHost, TimingPrecision,
};
use synvelis_project::{
    BuildMode, PluginRef, PortBinding, PortDirection, PortOwner, PortRole, Processor,
    ProcessorBinding, ProcessorId, SemanticPort, SemanticPortId, SignalDomain,
};

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim03-aw",
    name: "TIM-03 AW Gain",
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

/// Instance-mint counter — boundary refusals must fail before
/// `Instantiate`, so `created` stays 0 on every gated connect.
#[derive(Clone, Default)]
struct Tally {
    created: Arc<AtomicUsize>,
}

struct GainPlugin;

impl PluginProcessor for GainPlugin {
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

impl Plugin for GainPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &[]
    }
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
    }
}

struct GainFactory {
    tally: Tally,
}

impl GainFactory {
    fn mint(&self) -> GainPlugin {
        self.tally.created.fetch_add(1, Ordering::Relaxed);
        GainPlugin
    }
}

impl PluginFactory for GainFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(self.mint())
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(self.mint()))
    }
}

fn tally_host() -> (PluginHostProcessors, Tally) {
    let tally = Tally::default();
    let mut host = PluginHost::new();
    host.register_factory(Box::new(GainFactory {
        tally: tally.clone(),
    }));
    (PluginHostProcessors::new(host), tally)
}

fn fmt() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
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

fn processor(processor_id: ProcessorId) -> Processor {
    Processor {
        id: processor_id,
        name: "gain".into(),
        binding: ProcessorBinding::Resolved(PluginRef {
            descriptor: DESCRIPTOR.id.to_owned(),
            version: "0.1.0".into(),
            capability_revision: 0,
        }),
        port_ids: vec![SemanticPortId::new(1), SemanticPortId::new(2)],
    }
}

fn semantic_port(id: SemanticPortId, owner: ProcessorId, direction: PortDirection) -> SemanticPort {
    SemanticPort {
        id,
        owner: PortOwner::Processor(owner),
        direction,
        role: PortRole::Main,
        domain: SignalDomain::Audio,
        binding: PortBinding::Internal,
        channels: 2,
    }
}

fn connect_request(processor_id: ProcessorId, mode: BuildMode) -> ProcessorConnectRequest {
    let port_in = SemanticPortId::new(1);
    let port_out = SemanticPortId::new(2);
    ProcessorConnectRequest {
        processor: processor(processor_id),
        ports: vec![
            semantic_port(port_in, processor_id, PortDirection::Input),
            semantic_port(port_out, processor_id, PortDirection::Output),
        ],
        mode,
        generation: 1,
        node: mint_node_id(),
        port_format: fmt(),
        max_frames: 256,
    }
}

fn declaration(
    precision: TimingPrecision,
    consent: Option<DegradedConsent>,
) -> BoundaryDeclaration {
    BoundaryDeclaration {
        precision,
        in_block: InBlockTransportSupport::Unknown,
        consent,
        budget: ScheduleBudget::new(64, 256, 16, 4096),
    }
}

fn cap(precision: TimingPrecision) -> ProcessCapability {
    ProcessCapability::new(
        BlockContract::new(1, 256, InBlockTransportSupport::Unknown),
        precision,
    )
}

// ---------------------------------------------------------------------
// tim03_aw_delivery_* — attach_delivery projection
// ---------------------------------------------------------------------

/// Realtime without consent attaches `Exact` and passes the budget
/// through untouched — the default is never silently degraded.
#[test]
fn tim03_aw_delivery_realtime_exact_default() {
    let budget = ScheduleBudget::new(8, 64, 4, 128);
    let delivery = cap(TimingPrecision::Unknown)
        .attach_delivery(ExecutionMode::Realtime, None, budget)
        .unwrap();
    assert_eq!(delivery.policy, DeliveryPolicy::Exact);
    assert_eq!(delivery.budget, budget);
}

/// Realtime + `DegradedConsent` attaches `Degraded` with
/// `max_deferral` clamped to `min(consent, budget)`; the other budget
/// fields pass through. Consent below/above the budget bound clamps
/// both directions.
#[test]
fn tim03_aw_delivery_realtime_consent_degraded_clamped() {
    let budget = ScheduleBudget::new(8, 64, 4, 128);
    let below = cap(TimingPrecision::FrameAccurateEvents)
        .attach_delivery(
            ExecutionMode::Realtime,
            Some(DegradedConsent::grant(32)),
            budget,
        )
        .unwrap();
    assert_eq!(below.policy, DeliveryPolicy::Degraded);
    assert_eq!(below.budget.max_deferral, 32);
    assert_eq!(below.budget.max_spans_per_callback, 8);
    assert_eq!(below.budget.max_events_per_span, 64);
    assert_eq!(below.budget.max_pending_applies, 4);

    let above = cap(TimingPrecision::FrameAccurateEvents)
        .attach_delivery(
            ExecutionMode::Realtime,
            Some(DegradedConsent::grant(4096)),
            budget,
        )
        .unwrap();
    assert_eq!(above.policy, DeliveryPolicy::Degraded);
    assert_eq!(above.budget.max_deferral, 128);
}

/// `OfflineExact` requires certified exact-class precision:
/// `ExactWithinVerifiedCapability`/`FrameAccurateEvents` attach
/// `Exact`; `Unknown`/`BlockBoundaryOnly`/`DegradedWithConsent` fail
/// closed with `OfflineUncertified` carrying the certified class.
#[test]
fn tim03_aw_delivery_offline_requires_certified_exact() {
    let budget = ScheduleBudget::default();
    for precision in [
        TimingPrecision::ExactWithinVerifiedCapability,
        TimingPrecision::FrameAccurateEvents,
    ] {
        let delivery = cap(precision)
            .attach_delivery(ExecutionMode::OfflineExact, None, budget)
            .unwrap();
        assert_eq!(delivery.policy, DeliveryPolicy::Exact);
        assert_eq!(delivery.budget, budget);
    }
    for precision in [
        TimingPrecision::Unknown,
        TimingPrecision::BlockBoundaryOnly,
        TimingPrecision::DegradedWithConsent,
    ] {
        let error = cap(precision)
            .attach_delivery(ExecutionMode::OfflineExact, None, budget)
            .unwrap_err();
        assert!(
            matches!(
                error,
                ProcessBoundaryError::OfflineUncertified { precision: p } if p == precision
            ),
            "{precision:?} must fail OfflineUncertified, got {error:?}"
        );
    }
}

/// Certified `Unsupported` and a degenerate contract refuse the
/// attach outright — typed, before any policy is projected.
#[test]
fn tim03_aw_delivery_unsupported_and_degenerate() {
    for precision in [TimingPrecision::Unsupported] {
        for mode in [ExecutionMode::Realtime, ExecutionMode::OfflineExact] {
            let error = cap(precision)
                .attach_delivery(
                    mode,
                    Some(DegradedConsent::grant(64)),
                    ScheduleBudget::default(),
                )
                .unwrap_err();
            assert!(
                matches!(error, ProcessBoundaryError::Unsupported { .. }),
                "{precision:?}/{mode:?} must fail Unsupported, got {error:?}"
            );
        }
    }
    let degenerate = ProcessCapability::new(
        BlockContract::new(0, 0, InBlockTransportSupport::Unknown),
        TimingPrecision::FrameAccurateEvents,
    );
    let error = degenerate
        .attach_delivery(ExecutionMode::Realtime, None, ScheduleBudget::default())
        .unwrap_err();
    assert!(matches!(error, ProcessBoundaryError::Unsupported { .. }));
}

/// Offline delivery is always `Exact`: consent is ignored because
/// degraded is never accepted for offline exact render.
#[test]
fn tim03_aw_delivery_offline_ignores_consent() {
    let delivery = cap(TimingPrecision::FrameAccurateEvents)
        .attach_delivery(
            ExecutionMode::OfflineExact,
            Some(DegradedConsent::grant(64)),
            ScheduleBudget::new(8, 64, 4, 128),
        )
        .unwrap();
    assert_eq!(delivery.policy, DeliveryPolicy::Exact);
    assert_eq!(delivery.budget.max_deferral, 128);
}

// ---------------------------------------------------------------------
// tim03_aw_connect_* — PluginHostProcessors::connect wiring
// ---------------------------------------------------------------------

/// No registered declaration: the connect keeps the legacy
/// `Exact` + default-budget attach — non-regression for every
/// existing caller.
#[test]
fn tim03_aw_connect_undeclared_processor_legacy() {
    let (mut adapter, tally) = tally_host();
    let connection = adapter
        .connect(connect_request(ProcessorId::new(1), BuildMode::Realtime))
        .unwrap();
    assert!(connection.control.is_some());
    assert!(connection.state_handle.is_some());
    assert_eq!(tally.created.load(Ordering::Relaxed), 1);
}

/// A declared processor with realtime consent attaches successfully —
/// the consented `Degraded` projection is installed (the activated
/// contract is `min_frames=1`, so physical legality is unaffected;
/// the projection itself is pinned by `tim03_aw_delivery_*`).
#[test]
fn tim03_aw_connect_realtime_consented() {
    let (mut adapter, tally) = tally_host();
    let id = ProcessorId::new(1);
    adapter.register_boundary(
        id,
        declaration(
            TimingPrecision::FrameAccurateEvents,
            Some(DegradedConsent::grant(128)),
        ),
    );
    let connection = adapter
        .connect(connect_request(id, BuildMode::Realtime))
        .unwrap();
    assert!(connection.control.is_some());
    assert_eq!(tally.created.load(Ordering::Relaxed), 1);
}

/// Certified `Unsupported` is a typed connect refusal — and it fails
/// *before* `Instantiate`, so no plugin instance is ever minted.
#[test]
fn tim03_aw_connect_unsupported_typed() {
    let (mut adapter, tally) = tally_host();
    let id = ProcessorId::new(1);
    adapter.register_boundary(id, declaration(TimingPrecision::Unsupported, None));
    let error = adapter
        .connect(connect_request(id, BuildMode::Realtime))
        .err()
        .unwrap();
    assert_eq!(error, ProcessorConnectError::CapabilityUnsupported);
    assert_eq!(
        error.fault(),
        synvelis_project::RoutingFault::PluginUnavailable
    );
    assert_eq!(tally.created.load(Ordering::Relaxed), 0);
}

/// An offline build against an uncertified declaration fails closed
/// with `OfflineUncertified` → `OfflineCapabilityMismatch`, again
/// before any instance exists; a certified exact-class declaration
/// connects.
#[test]
fn tim03_aw_connect_offline_uncertified() {
    let (mut adapter, tally) = tally_host();
    let id = ProcessorId::new(1);
    adapter.register_boundary(id, declaration(TimingPrecision::Unknown, None));
    let error = adapter
        .connect(connect_request(id, BuildMode::Offline))
        .err()
        .unwrap();
    assert_eq!(error, ProcessorConnectError::OfflineUncertified);
    assert_eq!(
        error.fault(),
        synvelis_project::RoutingFault::OfflineCapabilityMismatch
    );
    assert_eq!(tally.created.load(Ordering::Relaxed), 0);

    // Certified exact-class precision passes the offline gate.
    adapter.register_boundary(id, declaration(TimingPrecision::FrameAccurateEvents, None));
    adapter
        .connect(connect_request(id, BuildMode::Offline))
        .unwrap();
    assert_eq!(tally.created.load(Ordering::Relaxed), 1);
}

/// The declaration is keyed by semantic `ProcessorId`: a second
/// processor on the same plugin without a declaration keeps the
/// legacy attach even while the first is gated.
#[test]
fn tim03_aw_connect_declaration_scoped_per_processor() {
    let (mut adapter, _tally) = tally_host();
    let gated = ProcessorId::new(1);
    let plain = ProcessorId::new(2);
    adapter.register_boundary(gated, declaration(TimingPrecision::Unsupported, None));
    assert_eq!(
        adapter
            .connect(connect_request(gated, BuildMode::Realtime))
            .err()
            .unwrap(),
        ProcessorConnectError::CapabilityUnsupported
    );
    adapter
        .connect(connect_request(plain, BuildMode::Realtime))
        .unwrap();
}
