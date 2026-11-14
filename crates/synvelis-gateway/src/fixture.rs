//! Static fixtures for contract testing (Issue #53 AC).
//!
//! The contract must be testable while no production operations are
//! registered. This module provides a test-origin [`AdapterSeal`]
//! issuer ([`FixtureAdapter`] — one of three in the crate, beside
//! `McpHost` and `LocalCliAdapter`), a permission-set evaluator, mock
//! handlers, and typed fixtures covering all four operation domains:
//! context query, MIDI mutation, transport, and job.
//!
//! Everything here is test/demonstration machinery — it is
//! honest about what it is: [`OriginKind::TestFixture`] is recorded on
//! every identity it attests.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::authorization::{AuthorizationDecision, AuthorizationStage, PolicyEvaluator};
use crate::capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityVersion, DataAccessClass,
    DataAccessSet, EffectKind, EffectSet, ExecutionContract, OperationDomain, PermissionKind,
    PermissionSet, TargetContract,
};
use crate::gateway::Gateway;
use crate::grant::{ResourceFacts, ResourceOracle};
use crate::id::AdapterId;
use crate::outcome::{ErrorEnvelope, GatewayOutcome};
use crate::registry::CapabilityOperation;
use crate::request::{
    AdapterSeal, CallerIdentity, OperationRequest, OriginKind, Principal, PrincipalKind,
    ResourceKind, ResourceRef, TimeDomain,
};
use crate::schema::{FieldSchema, SchemaRef, SchemaVersion, ValueSchema};

/// The in-crate trusted adapter for contract tests.
///
/// Holds a test-origin [`AdapterSeal`] issuance path: an identity
/// vouched by this adapter is trusted *because the crate said so*,
/// never because a wire payload claimed it.
#[derive(Debug)]
pub struct FixtureAdapter {
    seal: AdapterSeal,
    origin: OriginKind,
}

impl FixtureAdapter {
    /// A fixture adapter with a fixed adapter id.
    ///
    /// The origin is pinned to [`OriginKind::TestFixture`] — a fixture
    /// caller is never indistinguishable from a trusted manual session
    /// or a real transport. Issuance for `ManualUi`/`ExternalAdapter`
    /// origins is GW-08's seam, not a public constructor.
    #[must_use]
    pub fn new(adapter_raw: u128) -> Self {
        Self {
            seal: AdapterSeal::issue(AdapterId::new(adapter_raw)),
            origin: OriginKind::TestFixture,
        }
    }

    /// The conventional test adapter.
    #[must_use]
    pub fn for_testing() -> Self {
        Self::new(0xF1C7_0001)
    }

    /// Attests a caller of `kind` with adapter-side id `id`.
    ///
    /// # Errors
    /// [`crate::GatewayError::MalformedRequest`] when `id` is empty or
    /// exceeds [`Principal::MAX_ID_LEN`].
    pub fn caller(
        &self,
        kind: PrincipalKind,
        id: impl Into<String>,
    ) -> Result<CallerIdentity, crate::GatewayError> {
        Ok(CallerIdentity::new(
            Principal::new(kind, id)?,
            self.origin,
            &self.seal,
        ))
    }

    /// Attests a human (trusted manual session) caller.
    ///
    /// # Errors
    /// See [`caller`][Self::caller].
    pub fn human(&self, id: impl Into<String>) -> Result<CallerIdentity, crate::GatewayError> {
        self.caller(PrincipalKind::Human, id)
    }

    /// Attests an agent caller.
    ///
    /// # Errors
    /// See [`caller`][Self::caller].
    pub fn agent(&self, id: impl Into<String>) -> Result<CallerIdentity, crate::GatewayError> {
        self.caller(PrincipalKind::Agent, id)
    }
}

/// A static resource oracle for policy tests (GW-02).
///
/// Answers [`crate::grant::ResourceFacts::Known`] for resources
/// registered under the exact `(project, resource)` key and
/// `Unresolved` for everything else — the same non-distinguishing
/// verdict a real oracle gives for absent or invisible resources.
#[derive(Debug, Default)]
pub struct StaticOracle {
    facts: std::collections::HashMap<(synvelis_project::ProjectId, ResourceRef), ResourceFacts>,
}

impl StaticOracle {
    /// An oracle with no known resources.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers facts for `resource` in `project`.
    pub fn set(
        &mut self,
        project: synvelis_project::ProjectId,
        resource: ResourceRef,
        facts: ResourceFacts,
    ) {
        self.facts.insert((project, resource), facts);
    }

    /// Convenience for registering known attributes.
    pub fn known(
        &mut self,
        project: synvelis_project::ProjectId,
        resource: ResourceRef,
        attrs: crate::grant::ResourceAttributes,
    ) {
        self.set(project, resource, ResourceFacts::Known(attrs));
    }
}

impl ResourceOracle for StaticOracle {
    fn facts(&self, project: synvelis_project::ProjectId, resource: &ResourceRef) -> ResourceFacts {
        self.facts
            .get(&(project, *resource))
            .cloned()
            .unwrap_or(ResourceFacts::Unresolved)
    }
}

/// Fixture evaluator: permits exactly the permissions in `allowed`.
///
/// The set is fixed at construction — registering new capabilities can
/// never widen it (GT-003 evidence: a new capability whose required
/// permissions are outside this set is denied).
pub struct PermitSetPolicy {
    allowed: PermissionSet,
}

impl PermitSetPolicy {
    /// Permits requests whose required permissions are all in `allowed`.
    #[must_use]
    pub const fn new(allowed: PermissionSet) -> Self {
        Self { allowed }
    }

    /// The fixed permission set this policy permits.
    #[must_use]
    pub const fn allowed(&self) -> &PermissionSet {
        &self.allowed
    }
}

impl PolicyEvaluator for PermitSetPolicy {
    fn decide(
        &self,
        _req: &OperationRequest,
        _stage: AuthorizationStage,
        capability: &CapabilityDescriptor,
    ) -> AuthorizationDecision {
        if capability.required_permissions.is_subset_of(&self.allowed) {
            AuthorizationDecision::PermitForStage
        } else {
            AuthorizationDecision::Deny {
                reason: "required permissions outside the policy set".to_owned(),
            }
        }
    }
}

/// Mock handler that echoes the validated request back as the result —
/// evidence that the typed request reached the executor intact.
pub struct EchoHandler;

impl CapabilityOperation for EchoHandler {
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        Ok(GatewayOutcome::Completed {
            result: json!({
                "operation_id": req.operation_id.to_tagged(),
                "capability": req.capability.to_string(),
                "domain": req.domain,
                "project": req.project.to_tagged(),
                "target": req.target,
                "consistency": req.consistency,
                "args": req.args,
                "caller": req.caller,
            }),
        })
    }
}

/// Mock handler that accepts the operation — the async/job outcome.
pub struct AcceptHandler;

impl CapabilityOperation for AcceptHandler {
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        Ok(GatewayOutcome::Accepted {
            operation: req.operation_id,
        })
    }
}

fn fields(entries: &[(&str, FieldSchema)]) -> BTreeMap<String, FieldSchema> {
    entries
        .iter()
        .map(|(name, field)| ((*name).to_owned(), field.clone()))
        .collect()
}

#[allow(clippy::too_many_arguments)] // flat fixture spec
fn descriptor(
    id: &str,
    minor: u32,
    domain: OperationDomain,
    input: &str,
    target: TargetContract,
    effects: &[EffectKind],
    permissions: &[PermissionKind],
    data_access: &[DataAccessClass],
    execution: ExecutionContract,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("fixture capability ids are valid"),
        version: CapabilityVersion::new(1, minor),
        domain,
        input_schema: SchemaRef::new(format!("{input}.input"), 1, minor),
        output_schema: SchemaRef::new(format!("{input}.output"), 1, 0),
        target,
        effect_bound: EffectSet::of(effects.iter().copied()),
        required_permissions: PermissionSet::of(permissions.iter().copied()),
        data_access: DataAccessSet::of(data_access.iter().copied()),
        execution,
        availability: Availability::Available,
    }
}

/// The fixture capability descriptors (all four domains).
///
/// - `synvelis.context.source-notes` — context **Query**: explicit
///   source target + source-native range in args.
/// - `synvelis.midi.add-note` — **`ProjectMutation`**: explicit source
///   target, source-native position, `ExpectedRevision` required.
/// - `synvelis.midi.create-source` — **`ProjectMutation`** on the
///   `Create` target contract: explicit project parent + source kind.
/// - `synvelis.transport.set-position` — **`RuntimeControl`**: no target.
/// - `synvelis.job.render-export` — **`AsyncOperation`**: accepted for
///   later completion.
#[must_use]
pub fn fixture_descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        descriptor(
            "synvelis.context.source-notes",
            0,
            OperationDomain::Query,
            "synvelis.context.source-notes",
            TargetContract::Resource {
                kind: ResourceKind::Source,
            },
            &[EffectKind::Read, EffectKind::Disclose],
            &[PermissionKind::ReadContext, PermissionKind::DiscloseContext],
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::MediaBytesRead,
                DataAccessClass::ExternalDisclosure,
            ],
            ExecutionContract::Sync,
        ),
        descriptor(
            "synvelis.midi.add-note",
            1,
            OperationDomain::ProjectMutation,
            "synvelis.midi.add-note",
            TargetContract::Resource {
                kind: ResourceKind::Source,
            },
            &[EffectKind::Read, EffectKind::Write],
            &[PermissionKind::MutateProject],
            &[DataAccessClass::MetadataRead],
            ExecutionContract::Sync,
        ),
        descriptor(
            "synvelis.midi.create-source",
            0,
            OperationDomain::ProjectMutation,
            "synvelis.midi.create-source",
            TargetContract::Create {
                parent: ResourceKind::Project,
                creates: ResourceKind::Source,
            },
            &[EffectKind::Create],
            &[PermissionKind::MutateProject],
            &[],
            ExecutionContract::Sync,
        ),
        descriptor(
            "synvelis.transport.set-position",
            0,
            OperationDomain::RuntimeControl,
            "synvelis.transport.set-position",
            TargetContract::None,
            &[],
            &[PermissionKind::ControlTransport],
            &[],
            ExecutionContract::Sync,
        ),
        descriptor(
            "synvelis.job.render-export",
            0,
            OperationDomain::AsyncOperation,
            "synvelis.job.render-export",
            TargetContract::None,
            &[EffectKind::Read, EffectKind::ExternalAction],
            &[PermissionKind::RunJob],
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::ProviderTransfer,
            ],
            ExecutionContract::AsyncAccept,
        ),
    ]
}

/// The input schemas the fixtures validate against.
///
/// `synvelis.midi.add-note.input` is registered at three versions to
/// exercise schema compatibility: `1.0` (base), `1.1` (adds an
/// *optional* field — backward compatible), `2.0` (breaking shape).
/// The capability declares `1.1`.
#[allow(clippy::too_many_lines)] // flat schema table
#[must_use]
pub fn fixture_schemas() -> Vec<(String, SchemaVersion, ValueSchema)> {
    let mut v10 = BTreeMap::new();
    v10.insert(
        "key".to_owned(),
        FieldSchema::required(ValueSchema::Integer {
            min: Some(0),
            max: Some(127),
        }),
    );
    v10.insert(
        "velocity".to_owned(),
        FieldSchema::required(ValueSchema::Integer {
            min: Some(1),
            max: Some(127),
        }),
    );
    v10.insert(
        "position".to_owned(),
        FieldSchema::required(ValueSchema::TimeRange {
            domain: TimeDomain::SourceNative,
        }),
    );
    let mut v11 = v10.clone();
    v11.insert(
        "off_velocity".to_owned(),
        FieldSchema::optional(ValueSchema::Integer {
            min: Some(0),
            max: Some(127),
        }),
    );
    let mut v20 = BTreeMap::new();
    v20.insert(
        "note".to_owned(),
        FieldSchema::required(ValueSchema::Object {
            fields: fields(&[
                (
                    "pitch",
                    FieldSchema::required(ValueSchema::Integer {
                        min: Some(0),
                        max: Some(127),
                    }),
                ),
                (
                    "on",
                    FieldSchema::required(ValueSchema::Integer {
                        min: Some(0),
                        max: None,
                    }),
                ),
            ]),
            deny_unknown: true,
        }),
    );

    vec![
        (
            "synvelis.context.source-notes.input".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: fields(&[(
                    "range",
                    FieldSchema::required(ValueSchema::TimeRange {
                        domain: TimeDomain::SourceNative,
                    }),
                )]),
                deny_unknown: true,
            },
        ),
        (
            "synvelis.context.source-notes.output".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: BTreeMap::new(),
                deny_unknown: false,
            },
        ),
        (
            "synvelis.midi.add-note.input".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: v10,
                deny_unknown: true,
            },
        ),
        (
            "synvelis.midi.add-note.input".to_owned(),
            SchemaVersion::new(1, 1),
            ValueSchema::Object {
                fields: v11,
                deny_unknown: true,
            },
        ),
        (
            "synvelis.midi.add-note.input".to_owned(),
            SchemaVersion::new(2, 0),
            ValueSchema::Object {
                fields: v20,
                deny_unknown: true,
            },
        ),
        (
            "synvelis.midi.add-note.output".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: BTreeMap::new(),
                deny_unknown: false,
            },
        ),
        (
            "synvelis.midi.create-source.input".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: fields(&[(
                    "division",
                    FieldSchema::required(ValueSchema::Integer {
                        min: Some(1),
                        max: None,
                    }),
                )]),
                deny_unknown: true,
            },
        ),
        (
            "synvelis.midi.create-source.output".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: BTreeMap::new(),
                deny_unknown: false,
            },
        ),
        (
            "synvelis.transport.set-position.input".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: fields(&[
                    (
                        "action",
                        FieldSchema::required(ValueSchema::Enum {
                            values: vec!["play".to_owned(), "stop".to_owned(), "locate".to_owned()],
                        }),
                    ),
                    (
                        "position",
                        FieldSchema::optional(ValueSchema::TimeRange {
                            domain: TimeDomain::ProjectMusical,
                        }),
                    ),
                ]),
                deny_unknown: true,
            },
        ),
        (
            "synvelis.transport.set-position.output".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: BTreeMap::new(),
                deny_unknown: false,
            },
        ),
        (
            "synvelis.job.render-export.input".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: fields(&[(
                    "format",
                    FieldSchema::required(ValueSchema::Enum {
                        values: vec!["wav".to_owned(), "flac".to_owned()],
                    }),
                )]),
                deny_unknown: true,
            },
        ),
        (
            "synvelis.job.render-export.output".to_owned(),
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: BTreeMap::new(),
                deny_unknown: false,
            },
        ),
    ]
}

/// Installs all fixture schemas and capabilities on `gateway`.
///
/// Handlers: the job fixture gets [`AcceptHandler`] (async accept);
/// everything else gets [`EchoHandler`].
///
/// # Errors
/// [`ErrorEnvelope`] — duplicate registration when fixtures are
/// installed twice.
#[allow(clippy::result_large_err)] // typed envelope is the contract
pub fn install(gateway: &mut Gateway) -> Result<(), ErrorEnvelope> {
    for (name, version, schema) in fixture_schemas() {
        gateway
            .schemas_mut()
            .register(name, version, schema)
            .map_err(ErrorEnvelope::from)?;
    }
    for descriptor in fixture_descriptors() {
        let handler: Arc<dyn CapabilityOperation> =
            if matches!(descriptor.execution, ExecutionContract::AsyncAccept) {
                Arc::new(AcceptHandler)
            } else {
                Arc::new(EchoHandler)
            };
        gateway.register_capability(descriptor, Some(handler))?;
    }
    Ok(())
}
