//! SF-03 slice 3 — processor host connection boundary.
//!
//! [`ProcessorHost`] is the single seam through which semantic-routing
//! chain processors obtain their runtime [`GraphProcessor`] plus a
//! non-RT teardown [`ProcessorLease`]. Two implementations ship:
//!
//! - [`StaticProcessorHost`] — headless tests and callers that already
//!   hold processor instances; consumes one instance per connect and
//!   returns a no-op lease.
//! - [`PluginHostProcessors`] — the real `ruviform-plugin-host`
//!   adapter: resolves the semantic [`ProcessorBinding`], drives the
//!   full host lifecycle (`Instantiate → Initialize → Activate →
//!   StartProcessing → attach`) and returns the bridge
//!   [`PluginNode`](ruviform_audio_plugin_bridge::PluginNode) as the
//!   RT processor. The lease performs best-effort
//!   `StopProcessing → Deactivate → Destroy` on drop.
//!
//! **RT contract:** `connect` and lease teardown are non-RT only —
//! host lifecycle commands may allocate and call plugin hooks. Only
//! the returned `GraphProcessor` crosses into the runtime graph;
//! `PluginHost`, handles, endpoints' non-RT halves and leases never
//! do. `PluginHostProcessors` is deliberately `!Send` (`Rc<RefCell>`)
//! — the engine is a single-owner headless object; moving it across
//! threads is out of this slice's contract.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ruviform_audio::typed::GraphProcessor;
use ruviform_audio_core::AudioFormat;
use ruviform_audio_plugin_bridge::{AttachConfig, AttachError, PluginNodeControl, attach};
use ruviform_graph::NodeId;
use ruviform_media_core::{DeliveryPolicy, InBlockTransportSupport, ScheduleBudget, TimeBase};
use ruviform_plugin_core::{BusDescriptor, BusDirection, BusRole, ProcessConfig};
use ruviform_plugin_host::{
    EndpointBounds, HostCommand, HostError, HostReply, PluginHandle, PluginHost,
};

use synvelis_project::{
    BuildMode, PluginRef, PortDirection, PortRole, Processor, ProcessorBinding, ProcessorId,
    RoutingFault, SemanticPort,
};

use crate::process_boundary::{
    AttachDelivery, DegradedConsent, ExecutionMode, ProcessBoundaryError, ProcessCapability,
    TimingPrecision,
};

/// One processor provisioning request — everything a host needs to
/// connect a semantic chain processor for one build generation.
///
/// `ports` carries the processor's [`SemanticPort`]s in
/// `processor.port_ids` order; the host derives the declared bus table
/// from it and the runtime pairing (`io.input(i)`/`io.output(i)`)
/// depends on that order being preserved per direction.
#[derive(Clone, Debug)]
pub struct ProcessorConnectRequest {
    /// The semantic processor being connected (identity + binding).
    pub processor: Processor,
    /// Resolved semantic ports in `processor.port_ids` order.
    pub ports: Vec<SemanticPort>,
    /// Realtime vs offline build intent.
    pub mode: BuildMode,
    /// Build generation this connection belongs to.
    pub generation: u64,
    /// Generation-local graph node the processor will fill.
    pub node: NodeId,
    /// Engine audio format every port binds.
    pub port_format: AudioFormat,
    /// Engine block size ceiling (`u32` frames per process call).
    pub max_frames: u32,
}

/// A successful connect: the RT processor plus its teardown lease.
pub struct ProcessorConnection {
    /// The runtime processor — installed into the compiled plan.
    pub processor: Box<dyn GraphProcessor>,
    /// Non-RT teardown token; drop runs the host-side teardown once.
    pub lease: ProcessorLease,
    /// The node's non-RT control half (`attach`'s
    /// [`PluginNodeControl`]) — `Some` only for real plugin hosts.
    /// The engine retains it for the connection's generation
    /// lifetime and may vend it once via
    /// [`crate::routing_engine::RoutingEngine::take_processor_control`];
    /// untaken controls drop together with the generation's lease.
    /// `None` for [`StaticProcessorHost`] and other non-plugin hosts.
    pub control: Option<PluginNodeControl>,
    /// The plugin instance handle for non-RT state operations
    /// (TIM-04 `SaveState`/`RestoreState`) — `Some` only for real
    /// plugin hosts. Retained per generation and vended through
    /// [`crate::routing_engine::RoutingEngine::plugin_state_handles`];
    /// the handle is generation-checked by the host, so a stale
    /// handle fails closed instead of reaching another generation's
    /// plugin. `None` for [`StaticProcessorHost`].
    pub state_handle: Option<PluginHandle>,
}

/// One-shot access to a published generation's chain-processor
/// control channel — the non-RT halves a caller needs to drive
/// [`crate::ParamDispatch`] for exactly this generation's plugin
/// instance.
pub struct ProcessorControl {
    /// Generation-local graph node the control drives.
    pub node: NodeId,
    /// The node's non-RT control handle — its channel is dedicated
    /// to this generation's plugin instance, so staged transport
    /// commands can never reach another generation's plugin
    /// (wrong-plugin 0 by construction).
    pub control: PluginNodeControl,
    /// The `AttachConfig::frame_base` the node was bound with.
    /// [`PluginNodeControl::frame_base`] exposes the same verified
    /// value (Ruviform #307); the engine still vends it alongside the
    /// control so callers can construct a [`crate::ParamDispatch`]
    /// (whose `new` verifies the base via `check_frame_base`) or a
    /// [`crate::PluginBindingSession`] without a second lookup.
    pub frame_base: TimeBase,
}

/// Opaque per-connection teardown token. Dropping it runs the
/// host-side teardown exactly once (best-effort); the engine only
/// stores and drops it — generation lifecycle decides *when*.
pub struct ProcessorLease {
    teardown: Option<Box<dyn FnOnce()>>,
}

impl ProcessorLease {
    /// Wraps a teardown closure. The closure runs once on drop and
    /// must be non-blocking and best-effort — it executes on whatever
    /// thread drops the lease (always non-RT in the engine contract).
    /// External [`ProcessorHost`] implementations mint leases through
    /// this constructor.
    pub fn new(teardown: impl FnOnce() + 'static) -> Self {
        Self {
            teardown: Some(Box::new(teardown)),
        }
    }

    /// No-op lease for hosts whose instances need no teardown
    /// ([`StaticProcessorHost`]).
    pub(crate) fn noop() -> Self {
        Self { teardown: None }
    }
}

impl Drop for ProcessorLease {
    fn drop(&mut self) {
        if let Some(teardown) = self.teardown.take() {
            teardown();
        }
    }
}

/// Why a [`ProcessorHost::connect`] failed. [`Self::fault`] maps each
/// variant onto the existing [`RoutingFault`] taxonomy — no new fault
/// categories are introduced.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProcessorConnectError {
    /// The plugin/instance cannot be provided: unresolved binding,
    /// unknown plugin id, instance exhaustion or an unavailable
    /// endpoint.
    Unavailable,
    /// A host lifecycle step rejected the instance (initialize,
    /// activate, start-processing or attach-time host failure).
    Lifecycle,
    /// The declared bus table or bounds are incompatible with what
    /// the plugin/host reports.
    Incompatible,
    /// The request itself is malformed (bad config, port/binding
    /// inconsistency).
    Invalid,
    /// The instance's certified boundary capability is
    /// [`TimingPrecision::Unsupported`] or its activated block
    /// contract is degenerate — the requested delivery cannot be
    /// provided (TIM-03 `ProcessBoundaryError::Unsupported`).
    CapabilityUnsupported,
    /// An offline build requires certified exact-class timing
    /// precision and the registered [`BoundaryDeclaration`]'s
    /// certified [`TimingPrecision`] is not exact-class (TIM-03
    /// `ProcessBoundaryError::OfflineUncertified`).
    OfflineUncertified,
}

impl ProcessorConnectError {
    /// Deterministic [`RoutingFault`] mapping (design §fault table):
    /// `Unavailable`/`Lifecycle`/`CapabilityUnsupported` →
    /// `PluginUnavailable`, `Incompatible` →
    /// `UnsupportedFormatOrTimeMap`, `Invalid` → `InvalidRouting`,
    /// `OfflineUncertified` → `OfflineCapabilityMismatch`.
    #[must_use]
    pub const fn fault(self) -> RoutingFault {
        match self {
            Self::Unavailable | Self::Lifecycle | Self::CapabilityUnsupported => {
                RoutingFault::PluginUnavailable
            }
            Self::Incompatible => RoutingFault::UnsupportedFormatOrTimeMap,
            Self::Invalid => RoutingFault::InvalidRouting,
            Self::OfflineUncertified => RoutingFault::OfflineCapabilityMismatch,
        }
    }
}

/// Processor provisioning boundary — the only way semantic chain
/// processors reach the runtime graph.
///
/// **RT contract:** non-RT only. Implementations may run plugin
/// lifecycle hooks; they must return before the engine compiles.
pub trait ProcessorHost {
    /// Connects one semantic processor for `request`'s generation.
    ///
    /// # Errors
    /// [`ProcessorConnectError`] — the engine records `fault()` on the
    /// activation receipt and any already-minted leases drop (teardown)
    /// before the error propagates.
    fn connect(
        &mut self,
        request: ProcessorConnectRequest,
    ) -> Result<ProcessorConnection, ProcessorConnectError>;
}

/// Headless/test host: a `ProcessorId → GraphProcessor` instance map.
/// `connect` consumes the instance (a failed build needs a fresh host,
/// same contract the old `RoutingDeps.processors` map had) and returns
/// a no-op lease — instances are caller-owned plain processors with
/// no host lifecycle to tear down.
#[derive(Default)]
pub struct StaticProcessorHost {
    instances: BTreeMap<ProcessorId, Box<dyn GraphProcessor>>,
}

impl StaticProcessorHost {
    /// An empty host — every chain processor connect reports
    /// [`ProcessorConnectError::Unavailable`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `processor` under `id`, replacing any prior entry.
    pub fn insert(&mut self, id: ProcessorId, processor: Box<dyn GraphProcessor>) {
        self.instances.insert(id, processor);
    }
}

impl ProcessorHost for StaticProcessorHost {
    fn connect(
        &mut self,
        request: ProcessorConnectRequest,
    ) -> Result<ProcessorConnection, ProcessorConnectError> {
        let processor = self
            .instances
            .remove(&request.processor.id)
            .ok_or(ProcessorConnectError::Unavailable)?;
        Ok(ProcessorConnection {
            processor,
            lease: ProcessorLease::noop(),
            control: None,
            state_handle: None,
        })
    }
}

/// The composition root's attach-time boundary declaration for one
/// semantic processor (TIM-03): the capability claims and delivery
/// consent/budget [`PluginHostProcessors::connect`] projects into
/// `AttachConfig::{policy, budget}` through
/// [`ProcessCapability::attach_delivery`].
///
/// `precision` is the certified [`TimingPrecision`] claim for the
/// instance — [`TimingPrecision::Unknown`] until measured; it is
/// never upgraded by assumption. `in_block` is the adapter-attested
/// [`InBlockTransportSupport`] (only `Verified` claims verified
/// in-block transport, per the upstream contract). `consent` is the
/// explicit degraded consent for realtime delivery — `None` attaches
/// `Exact`; `Some` attaches `Degraded` bounded by
/// `min(consent.max_deferral, budget.max_deferral)`. `budget` is the
/// node's bounded-work budget. Offline builds ignore `consent` —
/// delivery is always `Exact` and uncertified precision fails the
/// connect closed.
#[derive(Clone, Debug)]
pub struct BoundaryDeclaration {
    /// Certified timing precision for this instance.
    pub precision: TimingPrecision,
    /// Adapter-attested in-block transport support.
    pub in_block: InBlockTransportSupport,
    /// Explicit degraded consent for realtime delivery.
    pub consent: Option<DegradedConsent>,
    /// The node's bounded-work budget.
    pub budget: ScheduleBudget,
}

/// Real `ruviform-plugin-host` adapter. Owns (a shared handle to) one
/// [`PluginHost`]; every `connect` instantiates a fresh plugin
/// instance, drives it to `Processing`, attaches a bridge endpoint and
/// returns the [`PluginNode`](ruviform_audio_plugin_bridge::PluginNode)
/// plus a lease that tears the instance down on drop.
///
/// `!Send` by design — `PluginHost` is single-owner non-RT state.
pub struct PluginHostProcessors {
    host: Rc<RefCell<PluginHost>>,
    /// TIM-03 attach-time boundary declarations keyed by semantic
    /// [`ProcessorId`] — processors without a declaration keep the
    /// legacy `Exact` + default-budget attach.
    boundaries: BTreeMap<ProcessorId, BoundaryDeclaration>,
}

impl PluginHostProcessors {
    /// Wraps `host`. The same `Rc` is shared with every minted lease so
    /// teardown reaches the host regardless of which generation drops
    /// first.
    #[must_use]
    pub fn new(host: PluginHost) -> Self {
        Self {
            host: Rc::new(RefCell::new(host)),
            boundaries: BTreeMap::new(),
        }
    }

    /// Registers (or replaces) the TIM-03 [`BoundaryDeclaration`] for
    /// `processor`. Non-RT orchestration — takes effect on the next
    /// `connect` for that processor; other processors are unaffected.
    pub fn register_boundary(&mut self, processor: ProcessorId, declaration: BoundaryDeclaration) {
        self.boundaries.insert(processor, declaration);
    }

    /// Projects the registered [`BoundaryDeclaration`] for
    /// `request.processor` onto the attach-time delivery decision.
    /// `None` — no declaration: the legacy `Exact` +
    /// default-budget attach. `Some` — the oracle's
    /// [`ProcessCapability::attach_delivery`] verdict; boundary
    /// refusals fail the connect before any instance is minted.
    fn boundary_delivery(
        &self,
        request: &ProcessorConnectRequest,
        process_config: &ProcessConfig,
    ) -> Result<Option<AttachDelivery>, ProcessorConnectError> {
        let Some(declaration) = self.boundaries.get(&request.processor.id) else {
            return Ok(None);
        };
        let capability = ProcessCapability::from_process_config(
            process_config,
            declaration.in_block,
            declaration.precision,
        );
        let mode = match request.mode {
            BuildMode::Realtime => ExecutionMode::Realtime,
            BuildMode::Offline => ExecutionMode::OfflineExact,
        };
        capability
            .attach_delivery(mode, declaration.consent, declaration.budget)
            .map(Some)
            .map_err(|error| boundary_error(&error))
    }

    /// Shared access to the underlying host — tests/observers only;
    /// the engine never calls back through it. Do not hold a
    /// `borrow_mut` across engine calls: `connect` borrows the same
    /// cell and would panic.
    #[must_use]
    pub fn host(&self) -> Rc<RefCell<PluginHost>> {
        Rc::clone(&self.host)
    }
}

impl ProcessorHost for PluginHostProcessors {
    fn connect(
        &mut self,
        request: ProcessorConnectRequest,
    ) -> Result<ProcessorConnection, ProcessorConnectError> {
        let ProcessorBinding::Resolved(plugin) = &request.processor.binding else {
            return Err(ProcessorConnectError::Unavailable);
        };
        let max_frames = request.max_frames;
        if max_frames == 0 {
            return Err(ProcessorConnectError::Invalid);
        }
        let process_config = ProcessConfig::new(request.port_format.sample_rate, 1, max_frames)
            .map_err(|_| ProcessorConnectError::Invalid)?;
        // TIM-03: the boundary declaration is evaluated before any
        // instance exists — a refused capability/precision fails the
        // connect with nothing to tear down.
        let delivery = self.boundary_delivery(&request, &process_config)?;
        let mut host = self.host.borrow_mut();
        let handle = instantiate(&mut host, plugin)?;
        // From here on the instance exists — every failure path must
        // run the same best-effort teardown before returning.
        let result = connect_attached(&mut host, handle, &request, process_config, delivery);
        match result {
            Ok(attached) => {
                let host_rc = Rc::clone(&self.host);
                let lease = ProcessorLease::new(move || {
                    // `try_borrow_mut`: a lease may drop while the host
                    // is mutably borrowed elsewhere only on a bug —
                    // teardown is best-effort and must never panic in
                    // `Drop`.
                    if let Ok(mut host) = host_rc.try_borrow_mut() {
                        teardown(&mut host, handle);
                    }
                });
                Ok(ProcessorConnection {
                    processor: Box::new(attached.node),
                    lease,
                    control: Some(attached.control),
                    state_handle: Some(handle),
                })
            }
            Err(error) => {
                teardown(&mut host, handle);
                Err(error)
            }
        }
    }
}

/// `Instantiate` + the three lifecycle commands up to
/// `StartProcessing`. Any failure before `attach` maps through
/// [`host_error`]; the caller owns teardown of `handle`.
fn connect_attached(
    host: &mut PluginHost,
    handle: PluginHandle,
    request: &ProcessorConnectRequest,
    process_config: ProcessConfig,
    delivery: Option<AttachDelivery>,
) -> Result<ruviform_audio_plugin_bridge::Attached, ProcessorConnectError> {
    exec(host, HostCommand::Initialize { handle }).map_err(lifecycle)?;
    exec(
        host,
        HostCommand::Activate {
            handle,
            config: process_config,
        },
    )
    .map_err(lifecycle)?;
    exec(host, HostCommand::StartProcessing { handle }).map_err(lifecycle)?;
    let buses = semantic_buses(request)?;
    let bounds = EndpointBounds {
        max_frames: request.max_frames,
        channels: request.port_format.layout.channels(),
    };
    let frame_base = TimeBase::sample_rate(request.port_format.sample_rate.get())
        .map_err(|_| ProcessorConnectError::Invalid)?;
    // TIM-03: a projected delivery installs its policy/budget;
    // undeclared processors keep the legacy Exact/default attach.
    let (policy, budget) = delivery.map_or_else(
        || (DeliveryPolicy::Exact, ScheduleBudget::default()),
        |delivery| (delivery.policy, delivery.budget),
    );
    attach(
        host,
        handle,
        &AttachConfig {
            buses,
            port_format: request.port_format,
            bounds,
            process_config,
            frame_base,
            policy,
            budget,
            input: None,
        },
    )
    .map_err(|error| attach_error(&error))
}

/// `Instantiate { plugin_id }` → handle. Unknown/exhausted plugin
/// supply is [`ProcessorConnectError::Unavailable`]; other host
/// rejections are [`ProcessorConnectError::Lifecycle`].
fn instantiate(
    host: &mut PluginHost,
    plugin: &PluginRef,
) -> Result<PluginHandle, ProcessorConnectError> {
    match host.execute(HostCommand::Instantiate {
        plugin_id: plugin.descriptor.clone(),
    }) {
        Ok(HostReply::Instantiated { handle }) => Ok(handle),
        Ok(_) => Err(ProcessorConnectError::Lifecycle),
        Err(error) => Err(match error {
            HostError::UnknownPluginId | HostError::InstanceLimit => {
                ProcessorConnectError::Unavailable
            }
            _ => ProcessorConnectError::Lifecycle,
        }),
    }
}

/// Executes a command that must answer `HostReply::Ok`.
fn exec(host: &mut PluginHost, command: HostCommand) -> Result<HostReply, HostError> {
    host.execute(command).and_then(|reply| match reply {
        HostReply::Ok => Ok(reply),
        _ => Err(HostError::Failed),
    })
}

/// Lifecycle-command failure → `Lifecycle` fault class (a plugin
/// that refuses `Initialize`/`Activate`/`StartProcessing` is
/// unavailable to this build, not a format problem).
fn lifecycle(_error: HostError) -> ProcessorConnectError {
    ProcessorConnectError::Lifecycle
}

/// TIM-03 boundary refusal → connect error.
/// [`ProcessCapability::attach_delivery`] can only produce
/// `Unsupported`/`OfflineUncertified`; any future variant fails
/// closed as `Invalid`.
fn boundary_error(error: &ProcessBoundaryError) -> ProcessorConnectError {
    match error {
        ProcessBoundaryError::Unsupported { .. } => ProcessorConnectError::CapabilityUnsupported,
        ProcessBoundaryError::OfflineUncertified { .. } => {
            ProcessorConnectError::OfflineUncertified
        }
        _ => ProcessorConnectError::Invalid,
    }
}

/// `attach` failure mapping: bus-table/bounds disagreement is
/// `Incompatible`; a host-side endpoint refusal is `Unavailable`;
/// every other known or future variant fails closed as `Lifecycle`.
fn attach_error(error: &AttachError) -> ProcessorConnectError {
    match error {
        AttachError::Bus(_) | AttachError::BoundsMismatch => ProcessorConnectError::Incompatible,
        AttachError::InputConfig => ProcessorConnectError::Invalid,
        AttachError::Host(HostError::EndpointUnavailable) => ProcessorConnectError::Unavailable,
        // `AttachError` is `#[non_exhaustive]` — an unknown future
        // variant fails closed as a lifecycle rejection.
        _ => ProcessorConnectError::Lifecycle,
    }
}

/// Semantic ports → declared [`BusDescriptor`]s, preserving
/// `port_ids` order — the bridge pairs the k-th input/output bus
/// with the k-th same-direction `NodeIo` port, which is exactly the
/// order `emit_processor_node` emitted the graph spec in.
fn semantic_buses(
    request: &ProcessorConnectRequest,
) -> Result<Vec<BusDescriptor>, ProcessorConnectError> {
    if request.ports.is_empty() {
        return Err(ProcessorConnectError::Invalid);
    }
    let mut buses = Vec::with_capacity(request.ports.len());
    for port in &request.ports {
        if port.owner != synvelis_project::PortOwner::Processor(request.processor.id)
            || port.domain != synvelis_project::SignalDomain::Audio
        {
            return Err(ProcessorConnectError::Invalid);
        }
        let (direction, role) = match (port.direction, port.role) {
            (PortDirection::Input, PortRole::Main) => (BusDirection::Input, BusRole::Main),
            (PortDirection::Input, PortRole::Sidechain) => {
                (BusDirection::Input, BusRole::Sidechain)
            }
            (PortDirection::Output, PortRole::Main) => (BusDirection::Output, BusRole::Main),
            // `PortRole`/`PortDirection` are non-exhaustive upstream —
            // an unknown combination cannot be declared honestly.
            _ => return Err(ProcessorConnectError::Invalid),
        };
        buses.push(BusDescriptor {
            direction,
            layout: request.port_format.layout,
            name: "",
            role,
            enabled: true,
        });
    }
    Ok(buses)
}

/// Best-effort reverse lifecycle: stop processing, deactivate,
/// destroy. Every step's result is ignored — teardown must never
/// panic or block; a failed step leaves the host's own state machine
/// to reject the next one.
fn teardown(host: &mut PluginHost, handle: PluginHandle) {
    for command in [
        HostCommand::StopProcessing { handle },
        HostCommand::Deactivate { handle },
        HostCommand::Destroy { handle },
    ] {
        let _ = host.execute(command);
    }
}
