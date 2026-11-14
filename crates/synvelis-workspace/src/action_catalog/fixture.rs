//! Shared headless fixture for the AUTX-01 Action Catalog.
//!
//! Consumer-neutral contract both GUI lowering (#266's
//! `automation_requests`) and the CLI surface (#262's lease) share: the
//! same [`CapabilityRef`] id+version, the same declared input schema,
//! and the same explicit stable [`WireTarget`] + domain-correct
//! [`WireConsistency`]. Nothing here infers a target from selection,
//! focus, or the active window — every invocation names its target.
//!
//! The base capability set (`Query` `synvelis.context.source-notes`,
//! `ProjectMutation` `synvelis.midi.*`, `RuntimeControl`
//! `synvelis.transport.set-position`, `AsyncOperation`
//! `synvelis.job.render-export`) comes from
//! [`synvelis_gateway::fixture`]; this module adds the Track/Clip Query
//! coverage that set lacks and the product [`ActionPresentation`]s that
//! join display metadata onto every fixture capability.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::fixture::EchoHandler;
use synvelis_gateway::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityVersion,
    DataAccessClass, DataAccessSet, EffectKind, EffectSet, ErrorEnvelope, ExecutionContract,
    FieldSchema, Gateway, OperationDomain, PermissionKind, PermissionSet, RequestId, ResourceKind,
    SchemaRef, SchemaVersion, TargetContract, ValueSchema, WireConsistency, WireRequest,
    WireTarget,
};
use synvelis_project::{ClipId, ProjectId, ProjectRevision, SourceId, TrackId};

use super::{ActionPresentation, VisibilityHint};

/// Track query capability — `Query` domain, explicit `track` target.
pub const TRACK_QUERY_ID: &str = "synvelis.track.query";
/// Clip query capability — `Query` domain, explicit `clip` target.
pub const CLIP_QUERY_ID: &str = "synvelis.clip.query";

fn cap(id: &str, minor: u32) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id).expect("fixture capability ids are valid"),
        version: CapabilityVersion::new(1, minor),
    }
}

fn fields(entries: &[(&str, FieldSchema)]) -> BTreeMap<String, FieldSchema> {
    entries
        .iter()
        .map(|(name, field)| ((*name).to_owned(), field.clone()))
        .collect()
}

fn object_schema(entries: &[(&str, FieldSchema)], deny_unknown: bool) -> ValueSchema {
    ValueSchema::Object {
        fields: fields(entries),
        deny_unknown,
    }
}

fn empty_output() -> ValueSchema {
    ValueSchema::Object {
        fields: BTreeMap::new(),
        deny_unknown: false,
    }
}

/// Value schemas for the catalog-fixture capabilities.
#[must_use]
pub fn schemas() -> Vec<(String, SchemaVersion, ValueSchema)> {
    let detail = || {
        FieldSchema::optional(ValueSchema::Enum {
            values: vec!["summary".to_owned(), "full".to_owned()],
        })
    };
    vec![
        (
            format!("{TRACK_QUERY_ID}.input"),
            SchemaVersion::new(1, 0),
            object_schema(&[("detail", detail())], true),
        ),
        (
            format!("{TRACK_QUERY_ID}.output"),
            SchemaVersion::new(1, 0),
            empty_output(),
        ),
        (
            format!("{CLIP_QUERY_ID}.input"),
            SchemaVersion::new(1, 0),
            object_schema(&[("detail", detail())], true),
        ),
        (
            format!("{CLIP_QUERY_ID}.output"),
            SchemaVersion::new(1, 0),
            empty_output(),
        ),
    ]
}

fn query_descriptor(
    id: &str,
    kind: ResourceKind,
    data_access: &[DataAccessClass],
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("fixture capability ids are valid"),
        version: CapabilityVersion::new(1, 0),
        domain: OperationDomain::Query,
        input_schema: SchemaRef::new(format!("{id}.input"), 1, 0),
        output_schema: SchemaRef::new(format!("{id}.output"), 1, 0),
        target: TargetContract::Resource { kind },
        effect_bound: EffectSet::of([EffectKind::Read]),
        required_permissions: PermissionSet::of([PermissionKind::ReadContext]),
        data_access: DataAccessSet::of(data_access.iter().copied()),
        execution: ExecutionContract::Sync,
        availability: Availability::Available,
    }
}

/// The catalog-fixture descriptors — Track/Clip `Query` coverage the
/// gateway fixture set does not provide.
#[must_use]
pub fn capabilities() -> Vec<CapabilityDescriptor> {
    vec![
        query_descriptor(
            TRACK_QUERY_ID,
            ResourceKind::Track,
            &[DataAccessClass::MetadataRead],
        ),
        query_descriptor(
            CLIP_QUERY_ID,
            ResourceKind::Clip,
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::StructuredContentRead,
            ],
        ),
    ]
}

fn pres(
    capability: CapabilityRef,
    display_key: &str,
    category: &str,
    description_key: &str,
    hints: &[(&str, &str)],
    visibility_hint: Option<VisibilityHint>,
) -> ActionPresentation {
    ActionPresentation {
        capability,
        display_key: display_key.to_owned(),
        category: category.to_owned(),
        description_key: description_key.to_owned(),
        parameter_hints: (!hints.is_empty()).then(|| {
            hints
                .iter()
                .map(|(name, hint)| ((*name).to_owned(), (*hint).to_owned()))
                .collect()
        }),
        visibility_hint,
    }
}

/// Product presentations for every catalog-fixture capability — the
/// base gateway fixture set plus the Track/Clip queries. Display/search
/// metadata only; `synvelis.midi.create-source` is marked `Hidden` to
/// exercise browse-vs-describe visibility.
#[must_use]
pub fn presentations() -> Vec<ActionPresentation> {
    vec![
        pres(
            cap("synvelis.context.source-notes", 0),
            "catalog.display.source-notes",
            "query",
            "catalog.desc.source-notes",
            &[("range", "param.hint.range.timerange")],
            None,
        ),
        pres(
            cap("synvelis.midi.add-note", 1),
            "catalog.display.midi-add-note",
            "editing",
            "catalog.desc.midi-add-note",
            &[
                ("key", "param.hint.key.piano-roll"),
                ("velocity", "param.hint.velocity.slider"),
                ("position", "param.hint.position.timerange"),
                ("off_velocity", "param.hint.off-velocity.slider"),
            ],
            None,
        ),
        pres(
            cap("synvelis.midi.create-source", 0),
            "catalog.display.midi-create-source",
            "editing",
            "catalog.desc.midi-create-source",
            &[("division", "param.hint.division.select")],
            Some(VisibilityHint::Hidden),
        ),
        pres(
            cap("synvelis.transport.set-position", 0),
            "catalog.display.transport-set-position",
            "transport",
            "catalog.desc.transport-set-position",
            &[
                ("action", "param.hint.action.transport-buttons"),
                ("position", "param.hint.position.transport-locate"),
            ],
            None,
        ),
        pres(
            cap("synvelis.job.render-export", 0),
            "catalog.display.job-render-export",
            "jobs",
            "catalog.desc.job-render-export",
            &[("format", "param.hint.format.select")],
            None,
        ),
        pres(
            cap(TRACK_QUERY_ID, 0),
            "catalog.display.track-query",
            "query",
            "catalog.desc.track-query",
            &[("detail", "param.hint.detail.enum")],
            None,
        ),
        pres(
            cap(CLIP_QUERY_ID, 0),
            "catalog.display.clip-query",
            "query",
            "catalog.desc.clip-query",
            &[("detail", "param.hint.detail.enum")],
            None,
        ),
    ]
}

/// Registers the catalog-fixture schemas and capabilities on `gateway`.
///
/// Call [`synvelis_gateway::fixture::install`] first for the base set —
/// this installs only the Track/Clip Query additions. Handlers are
/// [`EchoHandler`]s: the fixture proves the wire contract end-to-end,
/// not domain behavior.
///
/// # Errors
/// [`ErrorEnvelope`] — duplicate registration or schema incoherence.
#[allow(clippy::result_large_err)] // typed envelope is the contract
pub fn install(gateway: &mut Gateway) -> Result<(), ErrorEnvelope> {
    for (name, version, schema) in schemas() {
        gateway
            .schemas_mut()
            .register(name, version, schema)
            .map_err(ErrorEnvelope::from)?;
    }
    for descriptor in capabilities() {
        gateway.register_capability(descriptor, Some(Arc::new(EchoHandler)))?;
    }
    Ok(())
}

/// One consumer-neutral fixture invocation: the domain it exercises and
/// the exact [`WireRequest`] a GUI lowering or CLI call would send.
#[derive(Clone, Debug)]
pub struct FixtureInvocation {
    /// Stable label for evidence/test output.
    pub name: &'static str,
    /// The operation domain this invocation exercises.
    pub domain: OperationDomain,
    /// The wire request — exact `CapabilityRef`, declared input schema,
    /// explicit stable target (or none where the contract says `None`),
    /// and domain-correct consistency.
    pub wire: WireRequest,
}

fn wire(
    request_id: &str,
    capability: CapabilityRef,
    schema_name: String,
    project: &str,
    target: Option<WireTarget>,
    args: Value,
    consistency: WireConsistency,
) -> WireRequest {
    WireRequest {
        request_id: RequestId::new(request_id).expect("fixture request ids are valid"),
        schema: SchemaRef::new(schema_name, 1, capability.version.minor),
        capability,
        project: project.to_owned(),
        target,
        args,
        consistency,
        task: None,
        grant: None,
        idempotency_key: None,
    }
}

/// Canonical fixture invocations — one per operation domain the catalog
/// must represent, each with an explicit stable target (or the `None`
/// contract's explicit absence) and domain-correct consistency.
///
/// GUI consumers lower view actions to these same refs through
/// `automation_requests`; CLI consumers send the same wire shape. The
/// fixture deliberately covers `Query` on both `track` and `clip`
/// resources so neither kind is implied by the other.
#[must_use]
pub fn invocations() -> Vec<FixtureInvocation> {
    let project = ProjectId::new(7).to_tagged();
    vec![
        FixtureInvocation {
            name: "track.query",
            domain: OperationDomain::Query,
            wire: wire(
                "autx01-track-query",
                cap(TRACK_QUERY_ID, 0),
                format!("{TRACK_QUERY_ID}.input"),
                &project,
                Some(WireTarget::Resource {
                    kind: "track".to_owned(),
                    id: TrackId::new(3).to_tagged(),
                }),
                json!({ "detail": "summary" }),
                WireConsistency::ReadAt {
                    revision: ProjectRevision(4),
                },
            ),
        },
        FixtureInvocation {
            name: "clip.query",
            domain: OperationDomain::Query,
            wire: wire(
                "autx01-clip-query",
                cap(CLIP_QUERY_ID, 0),
                format!("{CLIP_QUERY_ID}.input"),
                &project,
                Some(WireTarget::Resource {
                    kind: "clip".to_owned(),
                    id: ClipId::new(11).to_tagged(),
                }),
                json!({ "detail": "full" }),
                WireConsistency::Latest,
            ),
        },
        FixtureInvocation {
            name: "midi.add-note",
            domain: OperationDomain::ProjectMutation,
            wire: wire(
                "autx01-add-note",
                cap("synvelis.midi.add-note", 1),
                "synvelis.midi.add-note.input".to_owned(),
                &project,
                Some(WireTarget::Resource {
                    kind: "source".to_owned(),
                    id: SourceId::new(9).to_tagged(),
                }),
                json!({
                    "key": 60,
                    "velocity": 100,
                    "position": { "domain": "source-native", "start": 240, "len": 480 },
                }),
                WireConsistency::ExpectedRevision {
                    revision: ProjectRevision(3),
                },
            ),
        },
        FixtureInvocation {
            name: "transport.set-position",
            domain: OperationDomain::RuntimeControl,
            wire: wire(
                "autx01-set-position",
                cap("synvelis.transport.set-position", 0),
                "synvelis.transport.set-position.input".to_owned(),
                &project,
                None,
                json!({ "action": "play" }),
                WireConsistency::Unconditional,
            ),
        },
        FixtureInvocation {
            name: "job.render-export",
            domain: OperationDomain::AsyncOperation,
            wire: wire(
                "autx01-render-export",
                cap("synvelis.job.render-export", 0),
                "synvelis.job.render-export.input".to_owned(),
                &project,
                None,
                json!({ "format": "wav" }),
                WireConsistency::Unconditional,
            ),
        },
    ]
}
