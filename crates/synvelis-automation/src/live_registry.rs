//! Live [`RegistrySnapshot`] capture — projects the gateway's
//! [`CapabilityRegistry`] and [`SchemaRegistry`] into the
//! gateway-independent snapshot the plan compiler and coordinator
//! consume (AUTX-06 / synvelis#265).
//!
//! This module is the *only* owner of the live→snapshot conversion
//! rules: exact version preservation (no "latest" resolution),
//! domain/target/effect/permission/data-access/execution/availability
//! fidelity, and the `ValueSchema` → [`SchemaNode`] structural mirror
//! (the workflow-local `job` node never appears — capability schemas
//! cannot contain it by contract).
//!
//! Scope note: the snapshot covers every schema a registered
//! capability declares as input or output — the exact set
//! [`crate::PlanCompiler`] can resolve through capability descriptors.
//! A workflow that names an unrelated schema on `await-job` fails
//! closed at compile time with `UnknownSchema`, which is the honest
//! answer: the snapshot never invents registrations the gateway does
//! not have.

use std::collections::BTreeSet;

use synvelis_gateway::Gateway;
use synvelis_gateway::capability::{
    Availability, CapabilityDescriptor, DataAccessClass, EffectKind, ExecutionContract,
    OperationDomain, PermissionKind, TargetContract,
};
use synvelis_gateway::request::{ResourceKind, TimeDomain};
use synvelis_gateway::schema::{SchemaRef, SchemaVersion, ValueSchema};

use crate::error::AutomationError;
use crate::refs::{Version, VersionedRef};
use crate::registry::{
    CapabilityDescriptor as SnapshotDescriptor, DataAccessSet, EffectSet, PermissionSet,
    RegistrySnapshot, SNAPSHOT_SCHEMA_VERSION, SchemaEntry, TargetContract as SnapshotTarget,
};
use crate::schema::{
    FieldNode, ResourceKind as SnapshotKind, SchemaNode, TimeDomain as SnapshotTime,
};

fn reg_err(reason: impl Into<String>) -> AutomationError {
    AutomationError::Registry {
        reason: reason.into(),
    }
}

fn domain_of(domain: OperationDomain) -> crate::registry::OperationDomain {
    match domain {
        OperationDomain::Query => crate::registry::OperationDomain::Query,
        OperationDomain::Proposal => crate::registry::OperationDomain::Proposal,
        OperationDomain::ProjectMutation => crate::registry::OperationDomain::ProjectMutation,
        OperationDomain::RuntimeControl => crate::registry::OperationDomain::RuntimeControl,
        OperationDomain::AsyncOperation => crate::registry::OperationDomain::AsyncOperation,
    }
}

fn kind_of(kind: ResourceKind) -> SnapshotKind {
    match kind {
        ResourceKind::Project => SnapshotKind::Project,
        ResourceKind::Track => SnapshotKind::Track,
        ResourceKind::Lane => SnapshotKind::Lane,
        ResourceKind::Clip => SnapshotKind::Clip,
        ResourceKind::Source => SnapshotKind::Source,
        ResourceKind::Asset => SnapshotKind::Asset,
        ResourceKind::Note => SnapshotKind::Note,
        ResourceKind::TakeGroup => SnapshotKind::TakeGroup,
        ResourceKind::Comp => SnapshotKind::Comp,
        ResourceKind::ParameterTarget => SnapshotKind::ParameterTarget,
        ResourceKind::AutomationLane => SnapshotKind::AutomationLane,
        ResourceKind::Strip => SnapshotKind::Strip,
        ResourceKind::Send => SnapshotKind::Send,
        ResourceKind::Processor => SnapshotKind::Processor,
    }
}

fn target_of(contract: TargetContract) -> SnapshotTarget {
    match contract {
        TargetContract::None => SnapshotTarget::None,
        TargetContract::Resource { kind } => SnapshotTarget::Resource {
            kind: kind_of(kind),
        },
        TargetContract::ResourceSet { kind } => SnapshotTarget::ResourceSet {
            kind: kind_of(kind),
        },
        TargetContract::Create { parent, creates } => SnapshotTarget::Create {
            parent: kind_of(parent),
            creates: kind_of(creates),
        },
    }
}

fn effect_of(kind: EffectKind) -> crate::registry::EffectKind {
    match kind {
        EffectKind::Read => crate::registry::EffectKind::Read,
        EffectKind::Write => crate::registry::EffectKind::Write,
        EffectKind::Create => crate::registry::EffectKind::Create,
        EffectKind::Delete => crate::registry::EffectKind::Delete,
        EffectKind::Disclose => crate::registry::EffectKind::Disclose,
        EffectKind::ExternalAction => crate::registry::EffectKind::ExternalAction,
    }
}

fn permission_of(kind: PermissionKind) -> crate::registry::PermissionKind {
    match kind {
        PermissionKind::ReadContext => crate::registry::PermissionKind::ReadContext,
        PermissionKind::DiscloseContext => crate::registry::PermissionKind::DiscloseContext,
        PermissionKind::MutateProject => crate::registry::PermissionKind::MutateProject,
        PermissionKind::ControlTransport => crate::registry::PermissionKind::ControlTransport,
        PermissionKind::RunJob => crate::registry::PermissionKind::RunJob,
        PermissionKind::ExternalAction => crate::registry::PermissionKind::ExternalAction,
    }
}

fn access_of(class: DataAccessClass) -> crate::registry::DataAccessClass {
    match class {
        DataAccessClass::MetadataRead => crate::registry::DataAccessClass::MetadataRead,
        DataAccessClass::MediaBytesRead => crate::registry::DataAccessClass::MediaBytesRead,
        DataAccessClass::StructuredContentRead => {
            crate::registry::DataAccessClass::StructuredContentRead
        }
        DataAccessClass::DerivedDataRead => crate::registry::DataAccessClass::DerivedDataRead,
        DataAccessClass::WorkspaceSelectionRead => {
            crate::registry::DataAccessClass::WorkspaceSelectionRead
        }
        DataAccessClass::ExternalDisclosure => crate::registry::DataAccessClass::ExternalDisclosure,
        DataAccessClass::ProviderTransfer => crate::registry::DataAccessClass::ProviderTransfer,
    }
}

fn execution_of(contract: ExecutionContract) -> crate::registry::ExecutionContract {
    match contract {
        ExecutionContract::Sync => crate::registry::ExecutionContract::Sync,
        ExecutionContract::AsyncAccept => crate::registry::ExecutionContract::AsyncAccept,
    }
}

fn availability_of(availability: &Availability) -> crate::registry::Availability {
    match availability {
        Availability::Available => crate::registry::Availability::Available,
        Availability::Disabled { reason } => crate::registry::Availability::Disabled {
            reason: reason.clone(),
        },
        Availability::Planned => crate::registry::Availability::Planned,
        Availability::Retired => crate::registry::Availability::Retired,
    }
}

fn time_domain_of(domain: TimeDomain) -> SnapshotTime {
    match domain {
        TimeDomain::ProjectMusical => SnapshotTime::ProjectMusical,
        TimeDomain::Media => SnapshotTime::Media,
        TimeDomain::SourceNative => SnapshotTime::SourceNative,
    }
}

fn node_of(schema: &ValueSchema) -> SchemaNode {
    match schema {
        ValueSchema::Bool => SchemaNode::Bool,
        ValueSchema::Integer { min, max } => SchemaNode::Integer {
            min: *min,
            max: *max,
        },
        ValueSchema::Number => SchemaNode::Number,
        ValueSchema::String { max_len } => SchemaNode::String { max_len: *max_len },
        ValueSchema::Id { kind } => SchemaNode::Id {
            kind: kind_of(*kind),
        },
        ValueSchema::Enum { values } => SchemaNode::Enum {
            values: values.clone(),
        },
        ValueSchema::Array { item, max_len } => SchemaNode::Array {
            item: Box::new(node_of(item)),
            max_len: *max_len,
        },
        ValueSchema::Object {
            fields,
            deny_unknown,
        } => SchemaNode::Object {
            fields: fields
                .iter()
                .map(|(name, field)| {
                    (
                        name.clone(),
                        FieldNode {
                            schema: node_of(&field.schema),
                            required: field.required,
                        },
                    )
                })
                .collect(),
            deny_unknown: *deny_unknown,
        },
        ValueSchema::Optional(inner) => SchemaNode::Optional {
            inner: Box::new(node_of(inner)),
        },
        ValueSchema::TimeRange { domain } => SchemaNode::TimeRange {
            domain: time_domain_of(*domain),
        },
    }
}

fn schema_ref_of(schema_ref: &SchemaRef) -> VersionedRef {
    VersionedRef::new(
        schema_ref.name.clone(),
        schema_ref.version.major,
        schema_ref.version.minor,
    )
}

fn descriptor_of(descriptor: &CapabilityDescriptor) -> SnapshotDescriptor {
    SnapshotDescriptor {
        id: descriptor.id.to_string(),
        version: Version {
            major: descriptor.version.major,
            minor: descriptor.version.minor,
        },
        domain: domain_of(descriptor.domain),
        input_schema: schema_ref_of(&descriptor.input_schema),
        output_schema: schema_ref_of(&descriptor.output_schema),
        target: target_of(descriptor.target),
        effect_bound: EffectSet::of(descriptor.effect_bound.iter().map(effect_of)),
        required_permissions: PermissionSet::of(
            descriptor.required_permissions.iter().map(permission_of),
        ),
        data_access: DataAccessSet::of(descriptor.data_access.iter().map(access_of)),
        execution: execution_of(descriptor.execution),
        availability: availability_of(&descriptor.availability),
    }
}

/// Captures the gateway's live registries into a validated
/// [`RegistrySnapshot`].
///
/// # Errors
/// [`AutomationError::Registry`] when a registered capability's
/// declared schema does not resolve (a registration invariant the
/// gateway itself enforces, so this is defensive) or when the produced
/// snapshot fails [`RegistrySnapshot::validate`].
pub fn capture(gateway: &Gateway) -> Result<RegistrySnapshot, AutomationError> {
    let mut capabilities = Vec::new();
    let mut schema_names: BTreeSet<(String, u32, u32)> = BTreeSet::new();
    for capability in gateway.registry().list() {
        let descriptor = gateway.registry().describe(&capability).map_err(|e| {
            reg_err(format!(
                "capability {} does not resolve: {e:?}",
                capability.id
            ))
        })?;
        capabilities.push(descriptor_of(descriptor));
        for schema_ref in [&descriptor.input_schema, &descriptor.output_schema] {
            schema_names.insert((
                schema_ref.name.clone(),
                schema_ref.version.major,
                schema_ref.version.minor,
            ));
        }
    }
    let mut schemas = Vec::new();
    for (name, major, minor) in schema_names {
        let schema_ref = SchemaRef {
            name: name.clone(),
            version: SchemaVersion::new(major, minor),
        };
        let value = gateway.schemas().resolve(&schema_ref).map_err(|e| {
            reg_err(format!(
                "schema {name}@{major}.{minor} does not resolve: {e:?}"
            ))
        })?;
        schemas.push(SchemaEntry {
            name,
            version: Version { major, minor },
            schema: node_of(value),
        });
    }
    let snapshot = RegistrySnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        capabilities,
        schemas,
    };
    snapshot.validate()?;
    Ok(snapshot)
}
