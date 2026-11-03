//! Internal operation-request contract (GTC-002).
//!
//! [`OperationRequest`] is the *validated* internal form of an invocation.
//! It is built only by [`crate::Gateway::bind`] and never deserialized —
//! wire data cannot construct it. The trusted caller
//! ([`CallerIdentity`]) is supplied separately by an adapter holding an
//! [`AdapterSeal`]; the seal is unforgeable outside this crate, so a wire
//! payload or an external crate cannot mint a caller (GT-002).

use serde::{Deserialize, Serialize};
use synvelis_project::{
    AssetId, AutomationLaneId, ClipId, CompId, LaneId, NoteId, ParameterTargetId, ProjectId,
    ProjectRevision, SourceId, TakeGroupId, TrackId,
};

use crate::capability::{CapabilityRef, OperationDomain};
use crate::id::{
    AdapterId, GrantRef, IdempotencyKey, OperationId, RequestId, SnapshotRef, TaskRef,
};
use crate::outcome::GatewayError;
use crate::schema::SchemaRef;

/// Kind of a typed stable resource id.
///
/// Mirrors the stable-id space owned by `synvelis-project`; the mapping
/// is one-way (gateway → project types) and exists so a wire-declared
/// kind can be checked against the tagged id that actually arrived.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    /// The project aggregate itself.
    Project,
    /// A track.
    Track,
    /// A lane inside a track.
    Lane,
    /// A timeline clip.
    Clip,
    /// A readable source — media `Source` or project-owned `MidiSource`
    /// (the two share `SourceId`).
    Source,
    /// A stored asset.
    Asset,
    /// A note entity.
    Note,
    /// A take group.
    TakeGroup,
    /// A comp.
    Comp,
    /// A parameter target.
    ParameterTarget,
    /// An automation lane.
    AutomationLane,
}

impl ResourceKind {
    /// Parses the wire kind tag.
    ///
    /// # Errors
    /// [`GatewayError::UnknownResourceKind`] when `name` is not a known
    /// kind tag.
    pub fn parse(name: &str) -> Result<Self, GatewayError> {
        Ok(match name {
            "project" => Self::Project,
            "track" => Self::Track,
            "lane" => Self::Lane,
            "clip" => Self::Clip,
            "source" => Self::Source,
            "asset" => Self::Asset,
            "note" => Self::Note,
            "take-group" => Self::TakeGroup,
            "comp" => Self::Comp,
            "param" | "parameter-target" => Self::ParameterTarget,
            "automation-lane" => Self::AutomationLane,
            _ => {
                return Err(GatewayError::UnknownResourceKind {
                    name: name.to_owned(),
                });
            }
        })
    }
}

/// A typed stable resource reference — the parsed form of
/// `"<kind>-<32hex>"` bound to its [`ResourceKind`].
///
/// Parsing checks the declared kind *and* the id's own tag: a
/// `"clip-…"` value passed where `"source"` was declared is a typed
/// [`GatewayError::WrongResourceKind`], not a silent reinterpretation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "kebab-case")]
pub enum ResourceRef {
    /// [`ProjectId`].
    Project(ProjectId),
    /// [`TrackId`].
    Track(TrackId),
    /// [`LaneId`].
    Lane(LaneId),
    /// [`ClipId`].
    Clip(ClipId),
    /// [`SourceId`].
    Source(SourceId),
    /// [`AssetId`].
    Asset(AssetId),
    /// [`NoteId`].
    Note(NoteId),
    /// [`TakeGroupId`].
    TakeGroup(TakeGroupId),
    /// [`CompId`].
    Comp(CompId),
    /// [`ParameterTargetId`].
    ParameterTarget(ParameterTargetId),
    /// [`AutomationLaneId`].
    AutomationLane(AutomationLaneId),
}

impl ResourceRef {
    /// The kind of this reference.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::Project(_) => ResourceKind::Project,
            Self::Track(_) => ResourceKind::Track,
            Self::Lane(_) => ResourceKind::Lane,
            Self::Clip(_) => ResourceKind::Clip,
            Self::Source(_) => ResourceKind::Source,
            Self::Asset(_) => ResourceKind::Asset,
            Self::Note(_) => ResourceKind::Note,
            Self::TakeGroup(_) => ResourceKind::TakeGroup,
            Self::Comp(_) => ResourceKind::Comp,
            Self::ParameterTarget(_) => ResourceKind::ParameterTarget,
            Self::AutomationLane(_) => ResourceKind::AutomationLane,
        }
    }

    /// Resolves the kind tag embedded in a tagged id string.
    ///
    /// # Errors
    /// [`GatewayError::InvalidId`] when the string is not in canonical
    /// `"<kind>-<32hex>"` form.
    fn kind_of_tagged(text: &str) -> Result<ResourceKind, GatewayError> {
        let (kind, hex) = text
            .rsplit_once('-')
            .ok_or_else(|| GatewayError::InvalidId {
                text: text.to_owned(),
            })?;
        if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(GatewayError::InvalidId {
                text: text.to_owned(),
            });
        }
        Ok(match kind {
            "project" => ResourceKind::Project,
            "track" => ResourceKind::Track,
            "lane" => ResourceKind::Lane,
            "clip" => ResourceKind::Clip,
            "source" => ResourceKind::Source,
            "asset" => ResourceKind::Asset,
            "note" => ResourceKind::Note,
            "take-group" => ResourceKind::TakeGroup,
            "comp" => ResourceKind::Comp,
            "param" => ResourceKind::ParameterTarget,
            "automation-lane" => ResourceKind::AutomationLane,
            _ => {
                return Err(GatewayError::InvalidId {
                    text: text.to_owned(),
                });
            }
        })
    }

    /// Parses a wire-declared `kind` and tagged `id` into a typed
    /// reference, rejecting kind disagreement.
    ///
    /// # Errors
    /// - [`GatewayError::UnknownResourceKind`] — unparseable `kind` name.
    /// - [`GatewayError::InvalidId`] — malformed tagged id.
    /// - [`GatewayError::WrongResourceKind`] — declared kind and the id's
    ///   own tag disagree.
    pub fn from_parts(kind_name: &str, id: &str) -> Result<Self, GatewayError> {
        let declared = ResourceKind::parse(kind_name)?;
        let actual = Self::kind_of_tagged(id)?;
        if declared != actual {
            return Err(GatewayError::WrongResourceKind {
                expected: declared,
                actual,
            });
        }
        let invalid = || GatewayError::InvalidId {
            text: id.to_owned(),
        };
        Ok(match declared {
            ResourceKind::Project => {
                Self::Project(ProjectId::from_tagged(id).map_err(|_| invalid())?)
            }
            ResourceKind::Track => Self::Track(TrackId::from_tagged(id).map_err(|_| invalid())?),
            ResourceKind::Lane => Self::Lane(LaneId::from_tagged(id).map_err(|_| invalid())?),
            ResourceKind::Clip => Self::Clip(ClipId::from_tagged(id).map_err(|_| invalid())?),
            ResourceKind::Source => Self::Source(SourceId::from_tagged(id).map_err(|_| invalid())?),
            ResourceKind::Asset => Self::Asset(AssetId::from_tagged(id).map_err(|_| invalid())?),
            ResourceKind::Note => Self::Note(NoteId::from_tagged(id).map_err(|_| invalid())?),
            ResourceKind::TakeGroup => {
                Self::TakeGroup(TakeGroupId::from_tagged(id).map_err(|_| invalid())?)
            }
            ResourceKind::Comp => Self::Comp(CompId::from_tagged(id).map_err(|_| invalid())?),
            ResourceKind::ParameterTarget => {
                Self::ParameterTarget(ParameterTargetId::from_tagged(id).map_err(|_| invalid())?)
            }
            ResourceKind::AutomationLane => {
                Self::AutomationLane(AutomationLaneId::from_tagged(id).map_err(|_| invalid())?)
            }
        })
    }

    /// Parses a tagged id string, deriving the kind from the id's own
    /// tag (used by policy evaluation for args-carried resource refs,
    /// where the kind is not pre-declared).
    ///
    /// # Errors
    /// [`GatewayError::InvalidId`] for malformed tagged ids.
    pub(crate) fn parse_tagged(id: &str) -> Result<Self, GatewayError> {
        let kind = Self::kind_of_tagged(id)?;
        Self::expect(kind, id)
    }

    /// Parses a tagged id string whose kind is fixed by the caller.
    ///
    /// # Errors
    /// [`GatewayError::InvalidId`] for malformed ids;
    /// [`GatewayError::WrongResourceKind`] when the id's tag is not
    /// `expected`.
    pub fn expect(expected: ResourceKind, id: &str) -> Result<Self, GatewayError> {
        let actual = Self::kind_of_tagged(id)?;
        if actual != expected {
            return Err(GatewayError::WrongResourceKind { expected, actual });
        }
        Self::from_parts(
            match expected {
                ResourceKind::Project => "project",
                ResourceKind::Track => "track",
                ResourceKind::Lane => "lane",
                ResourceKind::Clip => "clip",
                ResourceKind::Source => "source",
                ResourceKind::Asset => "asset",
                ResourceKind::Note => "note",
                ResourceKind::TakeGroup => "take-group",
                ResourceKind::Comp => "comp",
                ResourceKind::ParameterTarget => "param",
                ResourceKind::AutomationLane => "automation-lane",
            },
            id,
        )
    }
}

/// The time coordinate domain of an explicit range (GTC-005).
///
/// Project musical ticks, media time-base instants, and source-native
/// ticks are disjoint — a wire range's declared domain is checked
/// against what the capability's schema expects.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimeDomain {
    /// Project timeline in PPQN ticks (`MusicalTick`).
    ProjectMusical,
    /// Media time-base instants (`MediaTimeValue`).
    Media,
    /// Source-native ticks (`MidiTick` for MIDI sources).
    SourceNative,
}

/// A validated explicit range: declared time domain + integer extent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct TypedRange {
    /// The coordinate domain the range is expressed in.
    pub domain: TimeDomain,
    /// Inclusive start offset in domain-native units.
    pub start: i64,
    /// Extent length in domain-native units (must be positive).
    pub len: i64,
}

/// The resolved, typed operation target (GTC-002).
///
/// Serialized with a `shape` discriminator — `kind` is already claimed
/// by the inner [`ResourceRef`] tag, and internally-tagging a set
/// cannot represent a sequence, so every variant uses named fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum OperationTarget {
    /// The capability declares no resource target.
    None,
    /// One existing resource.
    Resource {
        /// The validated resource reference.
        resource: ResourceRef,
    },
    /// A non-empty set of existing resources of one kind.
    ResourceSet {
        /// The validated resource references.
        resources: Vec<ResourceRef>,
    },
    /// Create under an explicit parent.
    Create {
        /// The validated parent reference.
        parent: ResourceRef,
        /// The kind being created.
        creates: ResourceKind,
    },
}

/// Per-domain consistency requirement (GTC-002).
///
/// The variants deliberately encode *meaning*, not just a revision
/// value: a mutation's `ExpectedRevision` and a query's `ReadAt` pin are
/// different variants, so the two can never be confused even when they
/// carry the same number (Issue #53 AC).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Consistency {
    /// Read the latest committed state. Query domain only.
    Latest,
    /// Read pinned to an exact committed revision. Query domain only.
    ReadAt {
        /// The committed revision to read.
        revision: ProjectRevision,
    },
    /// Read from a fixed immutable snapshot token. Query domain only.
    FixedSnapshot {
        /// Reference to the snapshot (GW-03 owns issuance).
        snapshot: SnapshotRef,
    },
    /// Mutate only if the live project revision equals this expectation.
    /// Required — and only legal — for `ProjectMutation`.
    ExpectedRevision {
        /// The revision the mutation expects.
        revision: ProjectRevision,
    },
    /// No project-revision condition. `RuntimeControl` and
    /// `AsyncOperation` only.
    Unconditional,
}

impl Consistency {
    /// The variant's kind tag (for typed errors).
    #[must_use]
    pub const fn kind(&self) -> ConsistencyKind {
        match self {
            Self::Latest => ConsistencyKind::Latest,
            Self::ReadAt { .. } => ConsistencyKind::ReadAt,
            Self::FixedSnapshot { .. } => ConsistencyKind::FixedSnapshot,
            Self::ExpectedRevision { .. } => ConsistencyKind::ExpectedRevision,
            Self::Unconditional => ConsistencyKind::Unconditional,
        }
    }

    /// Whether this consistency is legal for `domain`.
    #[must_use]
    pub const fn allowed_in(&self, domain: OperationDomain) -> bool {
        match domain {
            OperationDomain::Query => matches!(
                self,
                Self::Latest | Self::ReadAt { .. } | Self::FixedSnapshot { .. }
            ),
            OperationDomain::ProjectMutation => matches!(self, Self::ExpectedRevision { .. }),
            OperationDomain::RuntimeControl | OperationDomain::AsyncOperation => {
                matches!(self, Self::Unconditional)
            }
        }
    }
}

/// Variant tags of [`Consistency`] — used where the requirement itself
/// is reported rather than the carried value.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsistencyKind {
    /// See [`Consistency::Latest`].
    Latest,
    /// See [`Consistency::ReadAt`].
    ReadAt,
    /// See [`Consistency::FixedSnapshot`].
    FixedSnapshot,
    /// See [`Consistency::ExpectedRevision`].
    ExpectedRevision,
    /// See [`Consistency::Unconditional`].
    Unconditional,
}

impl ConsistencyKind {
    /// The kebab-case variant name used in external messages.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Latest => "latest",
            Self::ReadAt => "read-at",
            Self::FixedSnapshot => "fixed-snapshot",
            Self::ExpectedRevision => "expected-revision",
            Self::Unconditional => "unconditional",
        }
    }
}

/// Whether the caller is a human session or an agent.
///
/// GTC-001: trusted human authority and agent grant/task authority are
/// distinct authorization grounds — the kind is part of the identity so
/// the evaluator can never conflate them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrincipalKind {
    /// Trusted human (manual DAW session).
    Human,
    /// Agent principal (external MCP client or builtin agent).
    Agent,
}

/// An authenticated principal as attested by a trusted adapter.
///
/// The id string is adapter-claimed but the *attestation* is gateway
/// work: a principal becomes a [`CallerIdentity`] only through an
/// [`AdapterSeal`]. Fields are private so a principal value can only be
/// produced through the bounded constructor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Principal {
    pub(crate) kind: PrincipalKind,
    pub(crate) id: String,
}

impl Principal {
    /// Maximum accepted id length.
    pub const MAX_ID_LEN: usize = 256;

    /// Wraps an adapter-attested principal id, rejecting empty or
    /// over-long values.
    ///
    /// # Errors
    /// [`GatewayError::MalformedRequest`] when `id` is empty or exceeds
    /// [`MAX_ID_LEN`][Self::MAX_ID_LEN].
    pub fn new(kind: PrincipalKind, id: impl Into<String>) -> Result<Self, GatewayError> {
        let id = id.into();
        if id.is_empty() || id.chars().count() > Self::MAX_ID_LEN {
            return Err(GatewayError::MalformedRequest {
                reason: format!("principal id must be 1..={} chars", Self::MAX_ID_LEN),
            });
        }
        Ok(Self { kind, id })
    }

    /// Human or agent.
    #[must_use]
    pub const fn kind(&self) -> PrincipalKind {
        self.kind
    }

    /// Adapter-attested principal identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Which adapter channel attested the caller.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OriginKind {
    /// The trusted manual DAW session.
    ManualUi,
    /// An external adapter (e.g. future MCP transport — GW-08).
    ExternalAdapter,
    /// The in-crate fixture adapter used by contract tests.
    TestFixture,
}

/// Capability token proving adapter trust.
///
/// Private field, no public constructor: only this crate can mint a
/// seal. [`CallerIdentity`] construction requires one, so a wire payload
/// or an external crate cannot fabricate a trusted caller (GT-002).
/// Issuance for real transports is GW-08's seam; GW-01's
/// [`crate::fixture::FixtureAdapter`] is the only issuer.
#[derive(Copy, Clone, Debug)]
pub struct AdapterSeal {
    adapter: AdapterId,
}

impl AdapterSeal {
    /// Mints a seal for `adapter`. Crate-internal by design.
    #[must_use]
    pub(crate) fn issue(adapter: AdapterId) -> Self {
        Self { adapter }
    }

    /// The adapter this seal attests for.
    #[must_use]
    pub const fn adapter(&self) -> AdapterId {
        self.adapter
    }
}

/// The trusted caller bound to an [`OperationRequest`].
///
/// Received *from* a trusted adapter — never parsed from wire arguments.
/// Serializable for audit output; deliberately not `Deserialize`.
/// Fields are private: a caller identity is produced only through
/// [`new`][CallerIdentity::new], which requires an [`AdapterSeal`] — an
/// external crate can neither forge nor mutate one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CallerIdentity {
    pub(crate) principal: Principal,
    pub(crate) origin: OriginKind,
    pub(crate) adapter: AdapterId,
}

impl CallerIdentity {
    /// Binds a principal attested through `seal`'s adapter channel.
    ///
    /// `seal` is unforgeable outside this crate, so this constructor can
    /// only be reached by code the gateway crate endowed with adapter
    /// trust.
    #[must_use]
    pub fn new(principal: Principal, origin: OriginKind, seal: &AdapterSeal) -> Self {
        Self {
            principal,
            origin,
            adapter: seal.adapter(),
        }
    }

    /// The attested principal.
    #[must_use]
    pub const fn principal(&self) -> &Principal {
        &self.principal
    }

    /// Which channel attested this caller.
    #[must_use]
    pub const fn origin(&self) -> OriginKind {
        self.origin
    }

    /// The adapter that attested this caller.
    #[must_use]
    pub const fn adapter(&self) -> AdapterId {
        self.adapter
    }
}

/// The validated internal operation request (GTC-002).
///
/// Constructed exclusively by [`crate::Gateway::bind`] after every
/// wire-level check passed. Fields are private — outside this crate
/// there is no way to construct one or to mutate a bound request, so
/// every value reaching a [`crate::CapabilityOperation`] provably passed
/// the full validation pipeline. Access is read-only through getters.
#[derive(Clone, Debug)]
pub struct OperationRequest {
    pub(crate) request_id: RequestId,
    pub(crate) operation_id: OperationId,
    pub(crate) caller: CallerIdentity,
    pub(crate) capability: CapabilityRef,
    pub(crate) domain: OperationDomain,
    pub(crate) project: ProjectId,
    pub(crate) target: OperationTarget,
    pub(crate) args: serde_json::Value,
    pub(crate) consistency: Consistency,
    pub(crate) input_schema: SchemaRef,
    pub(crate) task: Option<TaskRef>,
    pub(crate) grant: Option<GrantRef>,
    pub(crate) idempotency: Option<IdempotencyKey>,
}

impl OperationRequest {
    /// Caller-assigned correlation id (wire).
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Gateway-assigned operation instance id.
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    /// Trusted caller supplied by the adapter — never wire-derived.
    #[must_use]
    pub const fn caller(&self) -> &CallerIdentity {
        &self.caller
    }

    /// The resolved capability identity.
    #[must_use]
    pub const fn capability(&self) -> &CapabilityRef {
        &self.capability
    }

    /// Domain resolved from the descriptor (not wire-claimed).
    #[must_use]
    pub const fn domain(&self) -> OperationDomain {
        self.domain
    }

    /// The project the operation addresses.
    #[must_use]
    pub const fn project(&self) -> ProjectId {
        self.project
    }

    /// Validated, typed operation target.
    #[must_use]
    pub const fn target(&self) -> &OperationTarget {
        &self.target
    }

    /// Arguments validated against the input schema.
    #[must_use]
    pub const fn args(&self) -> &serde_json::Value {
        &self.args
    }

    /// Domain-checked consistency requirement.
    #[must_use]
    pub const fn consistency(&self) -> &Consistency {
        &self.consistency
    }

    /// The input schema version the request was validated under.
    #[must_use]
    pub const fn input_schema(&self) -> &SchemaRef {
        &self.input_schema
    }

    /// Optional opaque task reference — no authorization by itself.
    #[must_use]
    pub const fn task(&self) -> Option<&TaskRef> {
        self.task.as_ref()
    }

    /// Optional opaque grant reference — no authorization by itself.
    #[must_use]
    pub const fn grant(&self) -> Option<&GrantRef> {
        self.grant.as_ref()
    }

    /// Optional idempotency key.
    #[must_use]
    pub const fn idempotency(&self) -> Option<&IdempotencyKey> {
        self.idempotency.as_ref()
    }

    /// The expected project revision, when this is a `ProjectMutation`.
    #[must_use]
    pub fn expected_revision(&self) -> Option<ProjectRevision> {
        match &self.consistency {
            Consistency::ExpectedRevision { revision } => Some(*revision),
            _ => None,
        }
    }
}
