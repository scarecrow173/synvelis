//! SF-02 (issue #105): the DAW routing *transaction* boundary.
//!
//! One UI gesture — "create a bus and wire it up", "add a send with a
//! level target", "re-route this strip" — is a [`RoutingEditIntent`]:
//! an ordered [`RoutingEdit`] list whose endpoints may reference
//! entities the same gesture creates. [`preflight`] stages the whole
//! intent onto a candidate project: temporary references are resolved
//! to stable IDs drawn from the supplied generator, every per-edit
//! precondition and the full semantic + generic validation sweep run
//! on the candidate, and only a fully valid candidate is returned.
//!
//! Committing stays on the single writer: `ProjectOp::Routing`
//! carries the intent through `ProjectController::apply`, which
//! performs the one CAS — one revision step, one undo entry, and an
//! untouched live project on any rejection. `expected_project_revision`
//! is the `Command::expected_revision` field (single authority), and
//! `expected_port_capability_generation` rejects intents authored
//! against a stale plugin/port capability view.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    control::{ControlOwner, ParameterRole},
    controller::Operation,
    error::Error,
    id::{
        BusId, ChainId, IdGenerator, ParameterTargetId, ProcessorId, RoutingConnectionId,
        SemanticPortId, SendId, StripId,
    },
    ops::ProjectOp,
    project::Project,
    routing::{
        BusKind, ConnectionKind, ExplicitChannelMap, PortBinding, PortDirection, PortRole,
        ProcessorBinding, SendTap,
    },
};

// ---------------------------------------------------------------------------
// temporary references — gesture-scoped, never persisted
// ---------------------------------------------------------------------------

macro_rules! temp_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u32);
    };
}

temp_id!(
    /// A gesture-local bus placeholder resolved inside one transaction.
    TempBusId
);
temp_id!(
    /// A gesture-local send placeholder resolved inside one transaction.
    TempSendId
);
temp_id!(
    /// A gesture-local connection placeholder resolved inside one
    /// transaction.
    TempConnectionId
);

// ---------------------------------------------------------------------------
// target references — stable IDs or gesture temporaries
// ---------------------------------------------------------------------------

/// A bus target: a persisted bus or a same-gesture temporary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BusRef {
    /// An existing [`BusId`].
    Existing(BusId),
    /// A temporary declared by `CreateBus` earlier in the intent.
    Temp(TempBusId),
}

/// A strip target: a persisted strip or the strip a (possibly
/// temporary) bus owns.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StripRef {
    /// An existing [`StripId`].
    Existing(StripId),
    /// The strip owned by this bus.
    OfBus(BusRef),
}

/// A send target: a persisted send or a same-gesture temporary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SendRef {
    /// An existing [`SendId`].
    Existing(SendId),
    /// A temporary declared by `CreateSend` earlier in the intent.
    Temp(TempSendId),
}

/// A connection target: a persisted connection or a same-gesture
/// temporary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConnectionRef {
    /// An existing [`RoutingConnectionId`].
    Existing(RoutingConnectionId),
    /// A temporary declared by `CreateConnection`/`CreateSend` earlier
    /// in the intent.
    Temp(TempConnectionId),
}

/// A semantic-port target. `StripMain` resolves a strip's unique
/// main-role port by direction; `ProcessorPort` resolves a processor's
/// unique (role, direction) port — the way a gesture names a sidechain
/// input without knowing the stable ID.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PortRef {
    /// An existing [`SemanticPortId`].
    Existing(SemanticPortId),
    /// The strip's `Main` port in `direction`.
    StripMain {
        /// Strip the port belongs to.
        strip: StripRef,
        /// Which main port.
        direction: PortDirection,
    },
    /// A processor's unique `(role, direction)` port.
    ProcessorPort {
        /// Owning processor.
        processor: ProcessorId,
        /// Port role (`Sidechain` selects the key input).
        role: PortRole,
        /// Port direction.
        direction: PortDirection,
    },
}

// ---------------------------------------------------------------------------
// intent / edits
// ---------------------------------------------------------------------------

/// Whether the author has approved explicit channel conversions in
/// this intent. Without `Approved`, a layout-mismatched connection —
/// even with a well-formed map — is `RequiresUserDecision`, never a
/// silent up/downmix.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExplicitConversionPolicy {
    /// No conversion approved.
    #[default]
    None,
    /// The author approved every `ExplicitChannelMap` in this intent.
    Approved,
}

/// How a `CreateSend` edit obtains its level target — the control
/// model stays the single persisted gain authority; `SendConfig` only
/// ever references it. Issue #122: a new enabled send always carries
/// a level target — `Adopt` an existing dedicated target or `Create`
/// one inside the same transaction. (`SendConfig.gain_target`'s
/// `Option` remains only for legacy/unconfigured sends written before
/// this rule and for the explicit `SetSendGainTarget { None }`
/// unconfigure path.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "level", deny_unknown_fields)]
pub enum SendGainSpec {
    /// Adopt an existing project-time parameter target.
    Adopt(ParameterTargetId),
    /// Create a new `Send`-owned `SendLevel` target inside the same
    /// transaction.
    Create {
        /// Display name.
        name: String,
        /// Initial normalized base in `[0, 1]` (1.0 ≙ unity send
        /// level).
        initial: f64,
    },
}

/// One routing edit inside a [`RoutingEditIntent`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", deny_unknown_fields)]
pub enum RoutingEdit {
    /// Create a group/return bus (strip + chain + main ports).
    CreateBus {
        /// Gesture-local ID the resolved [`BusId`] is recorded under.
        temp_id: TempBusId,
        /// Bus kind (`Master` is rejected).
        kind: BusKind,
        /// Display name.
        name: String,
    },
    /// Remove a bus and its owned strip/chain/ports.
    RemoveBus {
        /// Target bus.
        bus: BusRef,
    },
    /// Create a `Main`/`Sidechain` connection.
    CreateConnection {
        /// Gesture-local ID the resolved connection is recorded under.
        temp_id: TempConnectionId,
        /// Source output port.
        source: PortRef,
        /// Destination input port.
        destination: PortRef,
        /// `Main` or `Sidechain` (`Send` is rejected — use
        /// `CreateSend`).
        kind: ConnectionKind,
        /// Explicit channel map (required for differing layouts).
        channel_map: Option<ExplicitChannelMap>,
    },
    /// Remove a connection (its `SendConfig` goes with it).
    RemoveConnection {
        /// Target connection.
        connection: ConnectionRef,
    },
    /// Enable or disable a connection.
    SetConnectionEnabled {
        /// Target connection.
        connection: ConnectionRef,
        /// New enabled flag.
        enabled: bool,
    },
    /// Replace a connection's explicit channel map.
    ChangeChannelMap {
        /// Target connection.
        connection: ConnectionRef,
        /// New map (`None` clears it — legal only when layouts match).
        channel_map: Option<ExplicitChannelMap>,
    },
    /// Create a send: the `Send`-kind connection, its `SendConfig`,
    /// and its level target in one atomic edit.
    CreateSend {
        /// Gesture-local send ID.
        temp_id: TempSendId,
        /// Gesture-local connection ID the send edge is recorded under.
        temp_connection: TempConnectionId,
        /// Source strip main output.
        source: PortRef,
        /// Destination strip main input.
        destination: PortRef,
        /// Pre/post-fader tap.
        tap: SendTap,
        /// Level target spec.
        level: SendGainSpec,
        /// Explicit channel map (required for differing layouts).
        channel_map: Option<ExplicitChannelMap>,
    },
    /// Remove a send and its connection.
    RemoveSend {
        /// Target send.
        send: SendRef,
    },
    /// Change a send's tap point.
    SetSendTap {
        /// Target send.
        send: SendRef,
        /// New tap.
        tap: SendTap,
    },
    /// Rebind a send's level target.
    SetSendGainTarget {
        /// Target send.
        send: SendRef,
        /// Target to bind (`None` clears the binding).
        target: Option<ParameterTargetId>,
    },
    /// Move a processor inside its chain — the persisted DSP order.
    ReorderProcessor {
        /// Owning chain.
        chain: ChainId,
        /// Processor to move.
        processor: ProcessorId,
        /// New index.
        new_index: usize,
    },
    /// Set a strip's persisted mute (Boolean control target).
    SetMute {
        /// Target strip.
        strip: StripRef,
        /// Muted flag.
        muted: bool,
    },
    /// Set a strip's persisted solo (Boolean control target).
    SetSolo {
        /// Target strip.
        strip: StripRef,
        /// Soloed flag.
        soloed: bool,
    },
    /// Change a bus's persisted configuration (name/kind).
    ChangeBusConfiguration {
        /// Target bus.
        bus: BusRef,
        /// New name (`None` keeps it).
        name: Option<String>,
        /// New kind (`None` keeps it; the master kind is fixed).
        kind: Option<BusKind>,
    },
}

/// A routing gesture authored against a specific project state —
/// carried by `ProjectOp::Routing` so the whole edit list commits or
/// rejects atomically.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingEditIntent {
    /// Client-supplied correlation ID echoed in the receipt.
    pub operation_id: String,
    /// The port/processor capability generation the intent was
    /// authored against ([`port_capability_generation`]); a stale
    /// value rejects as [`RoutingEditError::StalePortCapability`].
    pub expected_port_capability_generation: u64,
    /// Ordered edits — each sees the state its predecessors produced.
    pub edits: Vec<RoutingEdit>,
    /// Approved conversion policy for layout-mismatched connections.
    #[serde(default)]
    pub approved_conversion: ExplicitConversionPolicy,
}

// ---------------------------------------------------------------------------
// typed rejection
// ---------------------------------------------------------------------------

/// Why a routing intent was rejected — typed so callers can route the
/// failure to the right UX (consent prompt vs. hard error vs. stale
/// refresh), instead of string-matching a generic validation error.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingEditError {
    /// The intent was authored against a different project revision
    /// (the controller's CAS reports this before staging).
    StaleRevision {
        /// Revision carried by the command.
        expected: u64,
        /// Revision actually live.
        actual: u64,
    },
    /// `expected_port_capability_generation` is stale — a plugin
    /// reconfiguration or rebind moved the generation.
    StalePortCapability {
        /// Generation the intent carried.
        expected: u64,
        /// Generation actually live.
        actual: u64,
    },
    /// An entity the intent claims ownership of already exists under
    /// another owner or a duplicate edge was declared.
    DuplicateOwnership {
        /// What collided.
        detail: String,
    },
    /// A port exists but its binding is `Unresolved` — it cannot be a
    /// connection endpoint.
    UnresolvedPort {
        /// The unresolvable port.
        port: SemanticPortId,
    },
    /// A temporary or structural reference could not be resolved.
    UnresolvedReference {
        /// What could not resolve.
        detail: String,
    },
    /// Direction or signal-domain legality broke (output→input,
    /// same-domain only).
    InvalidDomainOrDirection {
        /// What was illegal.
        detail: String,
    },
    /// A channel map is malformed or inapplicable to the endpoints.
    UnsupportedChannelMap {
        /// Why the map fails.
        detail: String,
    },
    /// A feedback cycle would form — all cycles are rejected,
    /// delayed or not (product policy).
    FeedbackCycle {
        /// The cyclic path.
        detail: String,
    },
    /// A referenced entity does not exist.
    DanglingReference {
        /// What is missing.
        detail: String,
    },
    /// A referenced entity is still in use by another entity.
    DependencyInUse {
        /// What still uses it.
        detail: String,
    },
    /// The edit needs an explicit user decision (e.g. a channel map
    /// for differing layouts) that was not approved in the intent.
    RequiresUserDecision {
        /// What must be decided.
        detail: String,
    },
    /// A port's connection-count bound was exceeded.
    MultiplicityExceeded {
        /// Which bound and by how much.
        detail: String,
    },
    /// A resource-budget limit was hit.
    ResourceBudget {
        /// Which budget.
        detail: String,
    },
    /// The generic `ruviform-graph` validator rejected the projected
    /// graph — violations are surfaced verbatim.
    GenericGraphRejected {
        /// Human-readable violations.
        violations: Vec<String>,
    },
    /// The project oracle (`validate_references`) rejected the
    /// candidate — the same oracle a load enforces.
    OracleRejected {
        /// The oracle's error.
        detail: String,
    },
}

impl RoutingEditError {
    /// Collapses into the shared [`Error`] type for controller paths —
    /// the variant name is preserved as a `routing-*` code prefix.
    #[must_use]
    pub fn into_error(self) -> Error {
        match self {
            Self::StaleRevision { expected, actual } => Error::StaleRevision { expected, actual },
            other => Error::Validation(format!("{other}")),
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::StaleRevision { .. } => "routing-stale-revision",
            Self::StalePortCapability { .. } => "routing-stale-capability",
            Self::DuplicateOwnership { .. } => "routing-duplicate",
            Self::UnresolvedPort { .. } => "routing-unresolved-port",
            Self::UnresolvedReference { .. } => "routing-unresolved-ref",
            Self::InvalidDomainOrDirection { .. } => "routing-invalid-domain-direction",
            Self::UnsupportedChannelMap { .. } => "routing-unsupported-map",
            Self::FeedbackCycle { .. } => "routing-cycle",
            Self::DanglingReference { .. } => "routing-dangling",
            Self::DependencyInUse { .. } => "routing-dependency-in-use",
            Self::RequiresUserDecision { .. } => "routing-requires-decision",
            Self::MultiplicityExceeded { .. } => "routing-multiplicity",
            Self::ResourceBudget { .. } => "routing-resource-budget",
            Self::GenericGraphRejected { .. } => "routing-generic-rejected",
            Self::OracleRejected { .. } => "routing-oracle-rejected",
        }
    }
}

impl std::fmt::Display for RoutingEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = self.code();
        match self {
            Self::StaleRevision { expected, actual }
            | Self::StalePortCapability { expected, actual } => {
                write!(f, "{code}: expected {expected}, live {actual}")
            }
            Self::UnresolvedPort { port } => write!(f, "{code}: port {port}"),
            Self::GenericGraphRejected { violations } => {
                write!(f, "{code}: {}", violations.join("; "))
            }
            Self::DuplicateOwnership { detail }
            | Self::UnresolvedReference { detail }
            | Self::InvalidDomainOrDirection { detail }
            | Self::UnsupportedChannelMap { detail }
            | Self::FeedbackCycle { detail }
            | Self::DanglingReference { detail }
            | Self::DependencyInUse { detail }
            | Self::RequiresUserDecision { detail }
            | Self::MultiplicityExceeded { detail }
            | Self::ResourceBudget { detail }
            | Self::OracleRejected { detail } => write!(f, "{code}: {detail}"),
        }
    }
}

impl std::error::Error for RoutingEditError {}

// ---------------------------------------------------------------------------
// plan / receipt
// ---------------------------------------------------------------------------

/// The fully-staged transaction result — a validated candidate project
/// plus the deterministic temporary→stable ID record.
#[derive(Clone, Debug)]
pub struct RoutingPlan {
    /// The candidate project after every edit (fully validated).
    pub project: Project,
    /// Temporary→stable ID mapping produced by this staging.
    pub receipt: RoutingReceipt,
}

/// The committed-identity record of a routing transaction: which
/// stable IDs temporaries resolved to, and which gain targets were
/// created. Deterministic for a given (base, intent, generator seed).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutingReceipt {
    /// Echo of `RoutingEditIntent::operation_id`.
    pub operation_id: String,
    /// `TempBusId` → [`BusId`].
    pub bus_ids: BTreeMap<TempBusId, BusId>,
    /// `TempSendId` → [`SendId`].
    pub send_ids: BTreeMap<TempSendId, SendId>,
    /// `TempConnectionId` → [`RoutingConnectionId`].
    pub connection_ids: BTreeMap<TempConnectionId, RoutingConnectionId>,
    /// `TempSendId` → created [`ParameterTargetId`] for `Create` gain.
    pub gain_targets: BTreeMap<TempSendId, ParameterTargetId>,
}

// ---------------------------------------------------------------------------
// generation
// ---------------------------------------------------------------------------

/// The port/processor capability generation an intent is authored
/// against: a fingerprint of every resolved plugin binding
/// (`ProcessorBinding::Resolved` and `PluginDeclared` port bindings)
/// *inside the project model*. Any reconfigure or rebind changes it,
/// so an intent authored against the previous view rejects.
///
/// This is the project-side half of the capability pin only —
/// processor-host state (boundary/catalog/factory registrations)
/// lives outside `Project` and is tracked separately by the runtime
/// engine's build/publish/reconcile gates (SF-03 slice 9).
#[must_use]
pub fn port_capability_generation(project: &Project) -> u64 {
    let processors: u64 = project
        .routing
        .processors
        .values()
        .map(|p| match &p.binding {
            ProcessorBinding::Resolved(reference) => reference.capability_revision,
            ProcessorBinding::Unresolved { .. } => 0,
        })
        .fold(0u64, u64::saturating_add);
    let ports: u64 = project
        .routing
        .ports
        .values()
        .map(|p| match &p.binding {
            PortBinding::PluginDeclared {
                capability_revision,
                ..
            } => *capability_revision,
            _ => 0,
        })
        .fold(0u64, u64::saturating_add);
    processors.saturating_add(ports)
}

// ---------------------------------------------------------------------------
// staging
// ---------------------------------------------------------------------------

/// Staging state — the candidate project plus the temporary→stable
/// resolution tables built as edits apply in order.
struct Staging {
    project: Project,
    buses: BTreeMap<TempBusId, BusId>,
    sends: BTreeMap<TempSendId, SendId>,
    connections: BTreeMap<TempConnectionId, RoutingConnectionId>,
    gain_targets: BTreeMap<TempSendId, ParameterTargetId>,
}

impl Staging {
    fn bus(&self, reference: BusRef) -> Result<BusId, RoutingEditError> {
        match reference {
            BusRef::Existing(id) if self.project.routing.buses.contains_key(&id) => Ok(id),
            BusRef::Existing(id) => Err(RoutingEditError::DanglingReference {
                detail: format!("bus {id} does not exist"),
            }),
            BusRef::Temp(temp) => self.buses.get(&temp).copied().ok_or_else(|| {
                RoutingEditError::UnresolvedReference {
                    detail: format!("temporary bus {temp:?} not declared by a prior edit"),
                }
            }),
        }
    }

    fn strip(&self, reference: StripRef) -> Result<StripId, RoutingEditError> {
        match reference {
            StripRef::Existing(id) if self.project.routing.strips.contains_key(&id) => Ok(id),
            StripRef::Existing(id) => Err(RoutingEditError::DanglingReference {
                detail: format!("strip {id} does not exist"),
            }),
            StripRef::OfBus(bus_ref) => {
                let bus = self.bus(bus_ref)?;
                Ok(self.project.routing.buses[&bus].strip)
            }
        }
    }

    fn send(&self, reference: SendRef) -> Result<SendId, RoutingEditError> {
        match reference {
            SendRef::Existing(id) if self.project.routing.sends.contains_key(&id) => Ok(id),
            SendRef::Existing(id) => Err(RoutingEditError::DanglingReference {
                detail: format!("send {id} does not exist"),
            }),
            SendRef::Temp(temp) => self.sends.get(&temp).copied().ok_or_else(|| {
                RoutingEditError::UnresolvedReference {
                    detail: format!("temporary send {temp:?} not declared by a prior edit"),
                }
            }),
        }
    }

    fn connection(
        &self,
        reference: ConnectionRef,
    ) -> Result<RoutingConnectionId, RoutingEditError> {
        match reference {
            ConnectionRef::Existing(id) if self.project.routing.connections.contains_key(&id) => {
                Ok(id)
            }
            ConnectionRef::Existing(id) => Err(RoutingEditError::DanglingReference {
                detail: format!("connection {id} does not exist"),
            }),
            ConnectionRef::Temp(temp) => self.connections.get(&temp).copied().ok_or_else(|| {
                RoutingEditError::UnresolvedReference {
                    detail: format!("temporary connection {temp:?} not declared by a prior edit"),
                }
            }),
        }
    }

    fn port(&self, reference: &PortRef) -> Result<SemanticPortId, RoutingEditError> {
        match reference {
            PortRef::Existing(id) if self.project.routing.ports.contains_key(id) => Ok(*id),
            PortRef::Existing(id) => Err(RoutingEditError::DanglingReference {
                detail: format!("port {id} does not exist"),
            }),
            PortRef::StripMain { strip, direction } => {
                let strip_id = self.strip(*strip)?;
                let strip_ref = &self.project.routing.strips[&strip_id];
                strip_ref
                    .port_ids
                    .iter()
                    .find(|id| {
                        self.project
                            .routing
                            .ports
                            .get(id)
                            .is_some_and(|p| p.role == PortRole::Main && p.direction == *direction)
                    })
                    .copied()
                    .ok_or_else(|| RoutingEditError::UnresolvedReference {
                        detail: format!("strip {strip_id} has no main {direction:?} port"),
                    })
            }
            PortRef::ProcessorPort {
                processor,
                role,
                direction,
            } => {
                let proc_ref = self
                    .project
                    .routing
                    .processors
                    .get(processor)
                    .ok_or_else(|| RoutingEditError::DanglingReference {
                        detail: format!("processor {processor} does not exist"),
                    })?;
                proc_ref
                    .port_ids
                    .iter()
                    .find(|id| {
                        self.project
                            .routing
                            .ports
                            .get(id)
                            .is_some_and(|p| p.role == *role && p.direction == *direction)
                    })
                    .copied()
                    .ok_or_else(|| RoutingEditError::UnresolvedReference {
                        detail: format!("processor {processor} has no {role:?} {direction:?} port"),
                    })
            }
        }
    }

    /// Applies one `ProjectOp` to the candidate — the same
    /// validate-then-apply discipline the controller enforces, so a
    /// transaction never smuggles a shape a standalone op rejects.
    fn apply_op(
        &mut self,
        op: &ProjectOp,
        ids: &mut dyn IdGenerator,
    ) -> Result<(), RoutingEditError> {
        op.validate(&self.project)
            .map_err(|e| RoutingEditError::OracleRejected {
                detail: e.to_string(),
            })?;
        op.apply(&mut self.project, &mut *ids)
            .map_err(|e| RoutingEditError::OracleRejected {
                detail: e.to_string(),
            })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// preflight — whole-transaction staging
// ---------------------------------------------------------------------------

/// Stages `intent` onto a candidate built from `base`, drawing new
/// stable IDs from `ids`.
///
/// Pure with respect to `base` (cloned, never mutated): the returned
/// [`RoutingPlan`] carries a *fully validated* candidate — every edit
/// applied in order, the project oracle satisfied, and the generic
/// `ruviform-graph` validator passed. A `Err` returns no partial
/// state; `base` and `ids` semantics for the caller are unchanged
/// beyond the draws the staging consumed (which is why callers stage
/// on a scratch generator for previews).
///
/// # Errors
/// [`RoutingEditError`] — typed rejection; see the variant docs.
pub fn preflight(
    base: &Project,
    intent: &RoutingEditIntent,
    ids: &mut dyn IdGenerator,
) -> Result<RoutingPlan, RoutingEditError> {
    let actual = port_capability_generation(base);
    if actual != intent.expected_port_capability_generation {
        return Err(RoutingEditError::StalePortCapability {
            expected: intent.expected_port_capability_generation,
            actual,
        });
    }
    let mut staging = Staging {
        project: base.clone(),
        buses: BTreeMap::new(),
        sends: BTreeMap::new(),
        connections: BTreeMap::new(),
        gain_targets: BTreeMap::new(),
    };
    for edit in &intent.edits {
        apply_edit(&mut staging, edit, intent.approved_conversion, ids)?;
    }
    // Whole-candidate sweeps: the generic typed-graph validator owns
    // direction/domain/layout/multiplicity/cycle; the project oracle
    // owns persisted DAW invariants (ownership, send 1:1, references).
    validate_generic(&staging.project)?;
    staging
        .project
        .validate_references()
        .map_err(|e| RoutingEditError::OracleRejected {
            detail: e.to_string(),
        })?;
    Ok(RoutingPlan {
        project: staging.project,
        receipt: RoutingReceipt {
            operation_id: intent.operation_id.clone(),
            bus_ids: staging.buses,
            send_ids: staging.sends,
            connection_ids: staging.connections,
            gain_targets: staging.gain_targets,
        },
    })
}

/// Applies one edit to the staging candidate — resolves references,
/// enforces per-edit semantics, then delegates the mutation to the
/// existing `ProjectOp` so apply logic stays single-sourced.
#[allow(clippy::too_many_lines)] // flat match per edit variant
fn apply_edit(
    staging: &mut Staging,
    edit: &RoutingEdit,
    approved: ExplicitConversionPolicy,
    ids: &mut dyn IdGenerator,
) -> Result<(), RoutingEditError> {
    match edit {
        RoutingEdit::CreateBus {
            temp_id,
            kind,
            name,
        } => {
            if staging.buses.contains_key(temp_id) {
                return Err(RoutingEditError::DuplicateOwnership {
                    detail: format!("temporary bus {temp_id:?} declared twice"),
                });
            }
            let before: BTreeSet<BusId> = staging.project.routing.buses.keys().copied().collect();
            staging.apply_op(
                &ProjectOp::AddBus {
                    kind: *kind,
                    name: name.clone(),
                },
                ids,
            )?;
            let created = staging
                .project
                .routing
                .buses
                .keys()
                .find(|id| !before.contains(*id))
                .copied()
                .ok_or_else(|| RoutingEditError::OracleRejected {
                    detail: "AddBus produced no bus".into(),
                })?;
            staging.buses.insert(*temp_id, created);
        }
        RoutingEdit::RemoveBus { bus } => {
            let bus_id = staging.bus(*bus)?;
            staging.apply_op(&ProjectOp::RemoveBus { bus_id }, ids)?;
        }
        RoutingEdit::CreateConnection {
            temp_id,
            source,
            destination,
            kind,
            channel_map,
        } => {
            apply_create_connection(
                staging,
                *temp_id,
                source,
                destination,
                *kind,
                channel_map.as_ref(),
                approved,
                ids,
            )?;
        }
        RoutingEdit::RemoveConnection { connection } => {
            let connection_id = staging.connection(*connection)?;
            staging.apply_op(&ProjectOp::RemoveConnection { connection_id }, ids)?;
        }
        RoutingEdit::SetConnectionEnabled {
            connection,
            enabled,
        } => {
            let connection_id = staging.connection(*connection)?;
            staging.apply_op(
                &ProjectOp::SetConnectionEnabled {
                    connection_id,
                    enabled: *enabled,
                },
                ids,
            )?;
        }
        RoutingEdit::ChangeChannelMap {
            connection,
            channel_map,
        } => {
            let connection_id = staging.connection(*connection)?;
            let conn = &staging.project.routing.connections[&connection_id];
            let (src, dst) = (conn.source, conn.destination);
            check_channel_policy(staging, src, dst, channel_map.as_ref(), approved)?;
            staging.apply_op(
                &ProjectOp::SetConnectionChannelMap {
                    connection_id,
                    channel_map: channel_map.clone(),
                },
                ids,
            )?;
        }
        RoutingEdit::CreateSend {
            temp_id,
            temp_connection,
            source,
            destination,
            tap,
            level,
            channel_map,
        } => {
            apply_create_send(
                staging,
                *temp_id,
                *temp_connection,
                source,
                destination,
                *tap,
                level,
                channel_map.as_ref(),
                approved,
                ids,
            )?;
        }
        RoutingEdit::RemoveSend { send } => {
            let send_id = staging.send(*send)?;
            staging.apply_op(&ProjectOp::RemoveSend { send_id }, ids)?;
        }
        RoutingEdit::SetSendTap { send, tap } => {
            let send_id = staging.send(*send)?;
            staging.apply_op(&ProjectOp::SetSendTap { send_id, tap: *tap }, ids)?;
        }
        RoutingEdit::SetSendGainTarget { send, target } => {
            let send_id = staging.send(*send)?;
            staging.apply_op(
                &ProjectOp::SetSendGainTarget {
                    send_id,
                    gain_target: *target,
                },
                ids,
            )?;
        }
        RoutingEdit::ReorderProcessor {
            chain,
            processor,
            new_index,
        } => {
            staging.apply_op(
                &ProjectOp::ReorderProcessor {
                    chain_id: *chain,
                    processor_id: *processor,
                    new_index: *new_index,
                },
                ids,
            )?;
        }
        RoutingEdit::SetMute { strip, muted } => {
            let strip_id = staging.strip(*strip)?;
            staging.apply_op(
                &ProjectOp::SetStripMute {
                    strip_id,
                    muted: *muted,
                },
                ids,
            )?;
        }
        RoutingEdit::SetSolo { strip, soloed } => {
            let strip_id = staging.strip(*strip)?;
            staging.apply_op(
                &ProjectOp::SetStripSolo {
                    strip_id,
                    soloed: *soloed,
                },
                ids,
            )?;
        }
        RoutingEdit::ChangeBusConfiguration { bus, name, kind } => {
            let bus_id = staging.bus(*bus)?;
            staging.apply_op(
                &ProjectOp::ConfigureBus {
                    bus_id,
                    name: name.clone(),
                    kind: *kind,
                },
                ids,
            )?;
        }
    }
    Ok(())
}

/// `CreateConnection` edit body — declares the temp edge, validates
/// endpoints + channel policy, applies `AddConnection`, then records
/// the issued stable ID against the temporary.
#[allow(clippy::too_many_arguments)] // edit field pack — kept flat like the match arm it was
fn apply_create_connection(
    staging: &mut Staging,
    temp_id: TempConnectionId,
    source: &PortRef,
    destination: &PortRef,
    kind: ConnectionKind,
    channel_map: Option<&ExplicitChannelMap>,
    approved: ExplicitConversionPolicy,
    ids: &mut dyn IdGenerator,
) -> Result<(), RoutingEditError> {
    if staging.connections.contains_key(&temp_id) {
        return Err(RoutingEditError::DuplicateOwnership {
            detail: format!("temporary connection {temp_id:?} declared twice"),
        });
    }
    if matches!(kind, ConnectionKind::Send(_)) {
        return Err(RoutingEditError::InvalidDomainOrDirection {
            detail: "send connections are created by CreateSend".into(),
        });
    }
    let (src, dst) = resolved_endpoints(staging, source, destination)?;
    check_channel_policy(staging, src, dst, channel_map, approved)?;
    let before: BTreeSet<RoutingConnectionId> = staging
        .project
        .routing
        .connections
        .keys()
        .copied()
        .collect();
    staging.apply_op(
        &ProjectOp::AddConnection {
            source: src,
            destination: dst,
            kind,
            channel_map: channel_map.cloned(),
        },
        ids,
    )?;
    let created = staging
        .project
        .routing
        .connections
        .keys()
        .find(|id| !before.contains(*id))
        .copied()
        .ok_or_else(|| RoutingEditError::OracleRejected {
            detail: "AddConnection produced no connection".into(),
        })?;
    staging.connections.insert(temp_id, created);
    Ok(())
}

/// `CreateSend` edit body — the send edge, its `SendConfig`, and its
/// level target in one atomic step (the channel map follows on the
/// issued connection, the `Create` gain target binds last).
#[allow(clippy::too_many_arguments)] // edit field pack — kept flat like the match arm it was
fn apply_create_send(
    staging: &mut Staging,
    temp_id: TempSendId,
    temp_connection: TempConnectionId,
    source: &PortRef,
    destination: &PortRef,
    tap: SendTap,
    level: &SendGainSpec,
    channel_map: Option<&ExplicitChannelMap>,
    approved: ExplicitConversionPolicy,
    ids: &mut dyn IdGenerator,
) -> Result<(), RoutingEditError> {
    if staging.sends.contains_key(&temp_id) || staging.connections.contains_key(&temp_connection) {
        return Err(RoutingEditError::DuplicateOwnership {
            detail: "temporary send/connection declared twice".into(),
        });
    }
    let (src, dst) = resolved_endpoints(staging, source, destination)?;
    check_channel_policy(staging, src, dst, channel_map, approved)?;
    let adopt = match level {
        SendGainSpec::Adopt(target) => Some(*target),
        SendGainSpec::Create { .. } => None,
    };
    let sends_before: BTreeSet<SendId> = staging.project.routing.sends.keys().copied().collect();
    staging.apply_op(
        &ProjectOp::AddSend {
            source: src,
            destination: dst,
            tap,
            gain_target: adopt,
        },
        ids,
    )?;
    let send_id = staging
        .project
        .routing
        .sends
        .keys()
        .find(|id| !sends_before.contains(*id))
        .copied()
        .ok_or_else(|| RoutingEditError::OracleRejected {
            detail: "AddSend produced no send".into(),
        })?;
    let connection_id = staging.project.routing.sends[&send_id].connection;
    staging.sends.insert(temp_id, send_id);
    staging.connections.insert(temp_connection, connection_id);
    if channel_map.is_some() {
        staging.apply_op(
            &ProjectOp::SetConnectionChannelMap {
                connection_id,
                channel_map: channel_map.cloned(),
            },
            ids,
        )?;
    }
    if let SendGainSpec::Create { name, initial } = level {
        let targets_before: BTreeSet<ParameterTargetId> =
            staging.project.control.targets.keys().copied().collect();
        staging.apply_op(
            &ProjectOp::AddParameterTarget {
                name: name.clone(),
                owner: ControlOwner::Send(send_id),
                role: ParameterRole::SendLevel,
                time_scope: crate::control::AutomationTimeScope::ProjectMusical,
                kind: crate::control::ValueKind::Continuous,
                mapping: crate::control::ParamMapping::Linear,
                capability: None,
                base: *initial,
            },
            ids,
        )?;
        let target = staging
            .project
            .control
            .targets
            .keys()
            .find(|id| !targets_before.contains(*id))
            .copied()
            .ok_or_else(|| RoutingEditError::OracleRejected {
                detail: "AddParameterTarget produced no target".into(),
            })?;
        staging.apply_op(
            &ProjectOp::SetSendGainTarget {
                send_id,
                gain_target: Some(target),
            },
            ids,
        )?;
        staging.gain_targets.insert(temp_id, target);
    }
    Ok(())
}

/// Resolves both endpoints of a connection-declaring edit to stable
/// port IDs and enforces the per-edit structural preconditions the
/// generic validator cannot see: direction (output → input), matching
/// signal domain, and resolvable port bindings.
fn resolved_endpoints(
    staging: &Staging,
    source: &PortRef,
    destination: &PortRef,
) -> Result<(SemanticPortId, SemanticPortId), RoutingEditError> {
    let src = staging.port(source)?;
    let dst = staging.port(destination)?;
    let src_port = &staging.project.routing.ports[&src];
    let dst_port = &staging.project.routing.ports[&dst];
    if src_port.direction != PortDirection::Output {
        return Err(RoutingEditError::InvalidDomainOrDirection {
            detail: format!(
                "source port {src} is {:?}; an output is required",
                src_port.direction
            ),
        });
    }
    if dst_port.direction != PortDirection::Input {
        return Err(RoutingEditError::InvalidDomainOrDirection {
            detail: format!(
                "destination port {dst} is {:?}; an input is required",
                dst_port.direction
            ),
        });
    }
    if src_port.domain != dst_port.domain {
        return Err(RoutingEditError::InvalidDomainOrDirection {
            detail: format!(
                "domain mismatch: {src} is {:?}, {dst} is {:?}",
                src_port.domain, dst_port.domain
            ),
        });
    }
    for (id, port) in [(src, src_port), (dst, dst_port)] {
        if matches!(port.binding, PortBinding::Unresolved { .. }) {
            return Err(RoutingEditError::UnresolvedPort { port: id });
        }
    }
    Ok((src, dst))
}

/// The intent's explicit-conversion policy applied per edit: a
/// declared channel map requires `approved`, a layout mismatch
/// requires a declared map. No silent up/downmix — ever.
fn check_channel_policy(
    staging: &Staging,
    src: SemanticPortId,
    dst: SemanticPortId,
    channel_map: Option<&ExplicitChannelMap>,
    approved: ExplicitConversionPolicy,
) -> Result<(), RoutingEditError> {
    let src_port = &staging.project.routing.ports[&src];
    let dst_port = &staging.project.routing.ports[&dst];
    match channel_map {
        Some(map) => {
            if approved != ExplicitConversionPolicy::Approved {
                return Err(RoutingEditError::RequiresUserDecision {
                    detail: "an explicit channel map requires the intent's \
                             approved conversion policy"
                        .into(),
                });
            }
            if map.entries.is_empty() {
                return Err(RoutingEditError::UnsupportedChannelMap {
                    detail: "channel map is empty".into(),
                });
            }
            if map.entries.len() != usize::try_from(dst_port.channels).unwrap_or(usize::MAX) {
                return Err(RoutingEditError::UnsupportedChannelMap {
                    detail: format!(
                        "channel map covers {} destination channels; destination has {}",
                        map.entries.len(),
                        dst_port.channels
                    ),
                });
            }
            for (d, s) in map.entries.iter().enumerate() {
                if *s >= src_port.channels {
                    return Err(RoutingEditError::UnsupportedChannelMap {
                        detail: format!(
                            "channel map entry {d} reads source channel {s}; \
                             source has {}",
                            src_port.channels
                        ),
                    });
                }
            }
            Ok(())
        }
        None if src_port.channels != dst_port.channels => {
            Err(RoutingEditError::RequiresUserDecision {
                detail: format!(
                    "channel layout mismatch ({} → {}) needs an explicit \
                     channel map",
                    src_port.channels, dst_port.channels
                ),
            })
        }
        None => Ok(()),
    }
}

/// Projects the candidate's enabled routing edges into
/// `ruviform-graph`'s typed graph and runs its `Validator` —
/// direction, domain, format/layout, channel-map legality,
/// multiplicity, and cycle detection are all generic responsibilities
/// owned there, not re-implemented here.
///
/// Projection choices (DAW semantics the generic model has no opinion
/// on):
/// - one node per strip and per processor; its declared ports map 1:1;
/// - every port is `Multiplicity::unbounded(0)` + `Availability::
///   Optional` — fan-out/fan-in and required-wiring policy live in the
///   Synvelis oracle (`enabled_strip_edges`, duplicate-endpoint, send
///   1:1), not in generic bounds;
/// - `SemanticPort::channels` maps to a canonical `ChannelLayout`
///   (1→Mono, 2→Stereo, 4→Quad, 6→5.1, 8→7.1, else a custom mask of
///   `n` low bits) so equal counts never false-mismatch;
/// - a nominal 48 kHz/F32 `AudioFormat` on every port — the persisted
///   model carries no per-port rate, so format checks stay inert until
///   a real rate contract lands;
/// - only `enabled` connections become edges (a disabled edge carries
///   no signal and cannot close a cycle — the same rule the acyclicity
///   oracle uses); all edges are `EdgeDelay::Zero`, so every cycle —
///   delayed or not — rejects as `FeedbackCycle` (product policy).
fn validate_generic(project: &Project) -> Result<(), RoutingEditError> {
    use ruviform_graph::{ValidationError, Validator};

    let spec = project_graph_spec(project)?;
    match Validator::validate(&spec) {
        Ok(_) => Ok(()),
        Err(errors) => {
            for error in errors.errors() {
                match error {
                    ValidationError::ZeroDelayCycle { .. }
                    | ValidationError::DelayedCycleUnsupported { .. } => {
                        return Err(RoutingEditError::FeedbackCycle {
                            detail: error.to_string(),
                        });
                    }
                    ValidationError::ImplicitConversionRequired { .. } => {
                        return Err(RoutingEditError::RequiresUserDecision {
                            detail: error.to_string(),
                        });
                    }
                    ValidationError::InvalidChannelMap { .. } => {
                        return Err(RoutingEditError::UnsupportedChannelMap {
                            detail: error.to_string(),
                        });
                    }
                    ValidationError::CapacityExceeded { .. } => {
                        return Err(RoutingEditError::MultiplicityExceeded {
                            detail: error.to_string(),
                        });
                    }
                    ValidationError::InvalidDirection { .. }
                    | ValidationError::DomainMismatch { .. }
                    | ValidationError::FormatMismatch { .. } => {
                        return Err(RoutingEditError::InvalidDomainOrDirection {
                            detail: error.to_string(),
                        });
                    }
                    ValidationError::MissingNode { .. }
                    | ValidationError::MissingPort { .. }
                    | ValidationError::PortNotOnNode { .. } => {
                        return Err(RoutingEditError::DanglingReference {
                            detail: error.to_string(),
                        });
                    }
                    _ => {}
                }
            }
            Err(RoutingEditError::GenericGraphRejected {
                violations: errors.errors().iter().map(ToString::to_string).collect(),
            })
        }
    }
}

/// `SemanticPort::channels` → canonical `ChannelLayout` (equal counts
/// always produce equal layouts so the generic layout check only fires
/// on real mismatches; counts > 32 clamp to a 32-bit mask — the layout
/// still differs and the mismatch surfaces correctly).
fn layout_for(channels: u32) -> ruviform_audio_core::ChannelLayout {
    use ruviform_audio_core::{ChannelLayout, SpeakerMask};
    match channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        4 => ChannelLayout::Quad,
        6 => ChannelLayout::FivePointOne,
        8 => ChannelLayout::SevenPointOne,
        n => {
            let bits = if n >= 32 { u32::MAX } else { (1u32 << n) - 1 };
            ChannelLayout::custom(SpeakerMask::from_bits(bits)).unwrap_or(ChannelLayout::Stereo)
        }
    }
}

/// Builds the `ruviform-graph` projection of a candidate project —
/// see [`validate_generic`] for the projection contract.
fn project_graph_spec(project: &Project) -> Result<ruviform_graph::GraphSpec, RoutingEditError> {
    use ruviform_audio_core::{AudioFormat, SampleFormat, SampleRate};
    use ruviform_graph::{
        Activation, Availability, ChannelMap as GraphChannelMap, ConnDef, Direction, EdgeDelay,
        GraphBuilder, Multiplicity, NodeDef, NodeKind, PortDef, PortFormat,
        PortRef as GraphPortRef, PortRole as GraphPortRole,
    };

    let format = |channels: u32| {
        PortFormat::Audio(AudioFormat {
            sample_rate: SampleRate::new(48_000).unwrap_or_else(|_| unreachable!("nonzero rate")),
            layout: layout_for(channels),
            sample_format: SampleFormat::F32,
        })
    };
    let mut builder = GraphBuilder::new();
    let mut projected: BTreeMap<SemanticPortId, GraphPortRef> = BTreeMap::new();
    let mut add_owner = |owner_ports: &[SemanticPortId]| {
        let defs: Vec<PortDef> = owner_ports
            .iter()
            .filter_map(|id| project.routing.ports.get(id))
            .map(|port| PortDef {
                direction: match port.direction {
                    PortDirection::Input => Direction::Input,
                    PortDirection::Output => Direction::Output,
                },
                role: match port.role {
                    PortRole::Main => GraphPortRole::Main,
                    PortRole::Sidechain => GraphPortRole::Sidechain,
                },
                format: format(port.channels),
                multiplicity: Multiplicity::unbounded(0),
                capabilities: Vec::new(),
                availability: Availability::Optional,
                activation: Activation::Enabled,
            })
            .collect();
        let (node, port_ids) = builder.add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: defs,
        });
        for (sem_id, graph_id) in owner_ports.iter().zip(port_ids) {
            if project.routing.ports.contains_key(sem_id) {
                projected.insert(
                    *sem_id,
                    GraphPortRef {
                        node,
                        port: graph_id,
                    },
                );
            }
        }
    };
    for strip in project.routing.strips.values() {
        add_owner(&strip.port_ids);
    }
    for processor in project.routing.processors.values() {
        add_owner(&processor.port_ids);
    }
    for conn in project.routing.connections.values() {
        if !conn.enabled {
            continue;
        }
        let (Some(from), Some(to)) = (
            projected.get(&conn.source).copied(),
            projected.get(&conn.destination).copied(),
        ) else {
            // Missing endpoints are the oracle's `DanglingReference`
            // report — never silently skipped here.
            return Err(RoutingEditError::DanglingReference {
                detail: format!("connection {} references an undeclared port", conn.id),
            });
        };
        let channel_map = conn.channel_map.as_ref().map(|map| GraphChannelMap {
            entries: map
                .entries
                .iter()
                .enumerate()
                .map(|(d, s)| {
                    (
                        u16::try_from(*s).unwrap_or(u16::MAX),
                        u16::try_from(d).unwrap_or(u16::MAX),
                    )
                })
                .collect(),
        });
        builder.connect(&ConnDef {
            from,
            to,
            delay: EdgeDelay::Zero,
            channel_map,
        });
    }
    Ok(builder.build())
}
