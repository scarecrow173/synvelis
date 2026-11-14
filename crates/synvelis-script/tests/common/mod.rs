//! Shared fixtures: an in-memory Gateway registry pair covering every
//! `ValueSchema` variant, target contract, domain, and availability
//! state, plus the surface spec and derived model/stub/binding helpers.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityRegistry, CapabilityVersion,
    DataAccessClass, DataAccessSet, EffectKind, EffectSet, ExecutionContract, FieldSchema,
    OperationDomain, PermissionKind, PermissionSet, ResourceKind, SchemaRef, SchemaRegistry,
    SchemaVersion, TargetContract, TimeDomain, ValueSchema,
};
use synvelis_script::{
    BindingSet, GeneratedArtifacts, InvocationCall, LuauStub, RegistrySnapshot, ScriptApiModel,
    ScriptConsistency, ScriptSurfaceEntry, ScriptSurfaceSpec, ScriptTarget, ScriptVisibility,
};

pub fn vid(text: &str) -> CapabilityId {
    CapabilityId::new(text).expect("fixture capability id")
}

pub fn cver(major: u32, minor: u32) -> CapabilityVersion {
    CapabilityVersion::new(major, minor)
}

pub fn sver(major: u32, minor: u32) -> SchemaVersion {
    SchemaVersion::new(major, minor)
}

pub fn sref(name: &str, major: u32, minor: u32) -> SchemaRef {
    SchemaRef::new(name, major, minor)
}

/// `"<kind>-<32hex>"` tagged stable id fixture.
pub fn tagged_id(kind: &str, fill: char) -> String {
    format!("{kind}-{}", fill.to_string().repeat(32))
}

pub fn project_id() -> String {
    tagged_id("project", 'a')
}

pub fn track_id() -> String {
    tagged_id("track", 'b')
}

pub fn clip_id() -> String {
    tagged_id("clip", 'c')
}

pub fn note_id() -> String {
    tagged_id("note", 'd')
}

/// Builds a descriptor with default effect/permission/data-access sets.
#[allow(clippy::too_many_arguments)]
pub fn descriptor(
    id: &str,
    version: CapabilityVersion,
    domain: OperationDomain,
    input: SchemaRef,
    output: SchemaRef,
    target: TargetContract,
    availability: Availability,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: vid(id),
        version,
        domain,
        input_schema: input,
        output_schema: output,
        target,
        effect_bound: EffectSet::of(match domain {
            OperationDomain::Query => vec![EffectKind::Read],
            OperationDomain::Proposal | OperationDomain::AsyncOperation => {
                vec![EffectKind::Read, EffectKind::Write]
            }
            OperationDomain::ProjectMutation => {
                vec![EffectKind::Read, EffectKind::Write, EffectKind::Create]
            }
            OperationDomain::RuntimeControl => vec![EffectKind::ExternalAction],
        }),
        required_permissions: PermissionSet::of(match domain {
            OperationDomain::Query | OperationDomain::Proposal => {
                vec![PermissionKind::ReadContext]
            }
            OperationDomain::ProjectMutation => vec![PermissionKind::MutateProject],
            OperationDomain::RuntimeControl => vec![PermissionKind::ControlTransport],
            OperationDomain::AsyncOperation => vec![PermissionKind::RunJob],
        }),
        data_access: DataAccessSet::of(match domain {
            OperationDomain::Query
            | OperationDomain::Proposal
            | OperationDomain::AsyncOperation => vec![DataAccessClass::MetadataRead],
            OperationDomain::ProjectMutation => vec![
                DataAccessClass::MetadataRead,
                DataAccessClass::StructuredContentRead,
            ],
            OperationDomain::RuntimeControl => vec![],
        }),
        execution: ExecutionContract::Sync,
        availability,
    }
}

/// The input schema for `synvelis.midi.add-note` — exercises every
/// `ValueSchema` variant including out-of-f64-range `Integer`,
/// `TimeRange`, `Id`, `Enum`, nested `Object`/`Array`, `Optional`,
/// required vs optional fields, and `deny_unknown`.
pub fn add_note_input() -> ValueSchema {
    let mut fields = BTreeMap::new();
    fields.insert(
        "position".to_owned(),
        FieldSchema::required(ValueSchema::TimeRange {
            domain: TimeDomain::ProjectMusical,
        }),
    );
    fields.insert(
        "pitch".to_owned(),
        FieldSchema::required(ValueSchema::Integer {
            min: Some(0),
            max: Some(127),
        }),
    );
    fields.insert(
        "velocity".to_owned(),
        FieldSchema::optional(ValueSchema::Integer {
            min: Some(1),
            max: Some(127),
        }),
    );
    fields.insert(
        "channel".to_owned(),
        FieldSchema::optional(ValueSchema::Integer {
            min: Some(0),
            max: Some(15),
        }),
    );
    fields.insert(
        "curve".to_owned(),
        FieldSchema::required(ValueSchema::Enum {
            values: vec!["linear".to_owned(), "exp".to_owned(), "log".to_owned()],
        }),
    );
    fields.insert(
        "gain_db".to_owned(),
        FieldSchema::required(ValueSchema::Optional(Box::new(ValueSchema::Number))),
    );
    fields.insert(
        "macro".to_owned(),
        FieldSchema::optional(ValueSchema::Optional(Box::new(ValueSchema::Integer {
            min: Some(0),
            max: Some(127),
        }))),
    );
    fields.insert(
        "tags".to_owned(),
        FieldSchema::optional(ValueSchema::Array {
            item: Box::new(ValueSchema::String { max_len: Some(64) }),
            max_len: Some(8),
        }),
    );
    fields.insert(
        "sample_hint".to_owned(),
        FieldSchema::optional(ValueSchema::Integer {
            min: None,
            max: None,
        }),
    );
    let mut meta_fields = BTreeMap::new();
    meta_fields.insert("inner".to_owned(), FieldSchema::required(ValueSchema::Bool));
    meta_fields.insert(
        "label".to_owned(),
        FieldSchema::optional(ValueSchema::String { max_len: Some(32) }),
    );
    fields.insert(
        "meta".to_owned(),
        FieldSchema::optional(ValueSchema::Object {
            fields: meta_fields,
            deny_unknown: true,
        }),
    );
    fields.insert(
        "owner".to_owned(),
        FieldSchema::optional(ValueSchema::Id {
            kind: ResourceKind::Track,
        }),
    );
    ValueSchema::Object {
        fields,
        deny_unknown: true,
    }
}

pub fn add_note_output() -> ValueSchema {
    let mut fields = BTreeMap::new();
    fields.insert(
        "note_id".to_owned(),
        FieldSchema::required(ValueSchema::Id {
            kind: ResourceKind::Note,
        }),
    );
    fields.insert(
        "accepted".to_owned(),
        FieldSchema::required(ValueSchema::Bool),
    );
    ValueSchema::Object {
        fields,
        deny_unknown: false,
    }
}

/// Output schema for `synvelis.query.track-info` — non-object top level
/// is covered by other capabilities; this one is a rich object.
pub fn track_info_output() -> ValueSchema {
    let mut fields = BTreeMap::new();
    fields.insert(
        "name".to_owned(),
        FieldSchema::required(ValueSchema::String { max_len: Some(256) }),
    );
    fields.insert(
        "duration".to_owned(),
        FieldSchema::required(ValueSchema::TimeRange {
            domain: TimeDomain::Media,
        }),
    );
    fields.insert(
        "peak".to_owned(),
        FieldSchema::optional(ValueSchema::Number),
    );
    fields.insert(
        "sample_count".to_owned(),
        FieldSchema::optional(ValueSchema::Integer {
            min: None,
            max: None,
        }),
    );
    fields.insert(
        "flags".to_owned(),
        FieldSchema::required(ValueSchema::Enum {
            values: vec!["clean".to_owned(), "dirty".to_owned()],
        }),
    );
    let mut note_fields = BTreeMap::new();
    note_fields.insert(
        "pitch".to_owned(),
        FieldSchema::required(ValueSchema::Integer {
            min: Some(0),
            max: Some(127),
        }),
    );
    note_fields.insert(
        "start".to_owned(),
        FieldSchema::required(ValueSchema::Integer {
            min: None,
            max: None,
        }),
    );
    fields.insert(
        "notes".to_owned(),
        FieldSchema::optional(ValueSchema::Array {
            item: Box::new(ValueSchema::Object {
                fields: note_fields,
                deny_unknown: true,
            }),
            max_len: None,
        }),
    );
    ValueSchema::Object {
        fields,
        deny_unknown: true,
    }
}

/// Registers the `midi.*`, `transport.*`, and `query.*` fixture schemas.
fn register_core_schemas(schemas: &mut SchemaRegistry) {
    schemas
        .register("midi.add-note.input", sver(1, 0), add_note_input())
        .unwrap();
    schemas
        .register("midi.add-note.output", sver(1, 0), add_note_output())
        .unwrap();
    schemas
        .register(
            "transport.play.input",
            sver(1, 0),
            ValueSchema::Object {
                fields: BTreeMap::new(),
                deny_unknown: true,
            },
        )
        .unwrap();
    schemas
        .register("transport.play.output", sver(1, 0), ValueSchema::Bool)
        .unwrap();
    schemas
        .register(
            "query.track-info.input",
            sver(1, 0),
            ValueSchema::Object {
                fields: BTreeMap::new(),
                deny_unknown: true,
            },
        )
        .unwrap();
    schemas
        .register("query.track-info.output", sver(1, 0), track_info_output())
        .unwrap();
}

/// Registers the `clip.split` and `batch.move` fixture schemas.
fn register_split_schemas(schemas: &mut SchemaRegistry) {
    let mut split_fields = BTreeMap::new();
    split_fields.insert(
        "at".to_owned(),
        FieldSchema::required(ValueSchema::TimeRange {
            domain: TimeDomain::ProjectMusical,
        }),
    );
    split_fields.insert(
        "mode".to_owned(),
        FieldSchema::required(ValueSchema::Enum {
            values: vec!["copy".to_owned(), "move".to_owned()],
        }),
    );
    schemas
        .register(
            "clip.split.input",
            sver(1, 0),
            ValueSchema::Object {
                fields: split_fields,
                deny_unknown: true,
            },
        )
        .unwrap();
    let mut split_out_fields = BTreeMap::new();
    split_out_fields.insert(
        "clips".to_owned(),
        FieldSchema::required(ValueSchema::Array {
            item: Box::new(ValueSchema::Id {
                kind: ResourceKind::Clip,
            }),
            max_len: None,
        }),
    );
    schemas
        .register(
            "clip.split.output",
            sver(1, 0),
            ValueSchema::Object {
                fields: split_out_fields,
                deny_unknown: false,
            },
        )
        .unwrap();
    let mut move_fields = BTreeMap::new();
    move_fields.insert(
        "delta".to_owned(),
        FieldSchema::required(ValueSchema::Integer {
            min: None,
            max: None,
        }),
    );
    schemas
        .register(
            "batch.move.input",
            sver(1, 0),
            ValueSchema::Object {
                fields: move_fields,
                deny_unknown: true,
            },
        )
        .unwrap();
    let mut move_out_fields = BTreeMap::new();
    move_out_fields.insert(
        "moved".to_owned(),
        FieldSchema::required(ValueSchema::Integer {
            min: None,
            max: None,
        }),
    );
    schemas
        .register(
            "batch.move.output",
            sver(1, 0),
            ValueSchema::Object {
                fields: move_out_fields,
                deny_unknown: false,
            },
        )
        .unwrap();
}

/// Registers the `clip.create` fixture schemas.
fn register_create_schemas(schemas: &mut SchemaRegistry) {
    let mut create_fields = BTreeMap::new();
    create_fields.insert(
        "name".to_owned(),
        FieldSchema::required(ValueSchema::String { max_len: None }),
    );
    create_fields.insert(
        "start".to_owned(),
        FieldSchema::required(ValueSchema::TimeRange {
            domain: TimeDomain::Media,
        }),
    );
    schemas
        .register(
            "clip.create.input",
            sver(1, 0),
            ValueSchema::Object {
                fields: create_fields,
                deny_unknown: true,
            },
        )
        .unwrap();
    schemas
        .register(
            "clip.create.output",
            sver(1, 0),
            ValueSchema::Id {
                kind: ResourceKind::Clip,
            },
        )
        .unwrap();
}

/// Registers a descriptor with no operation handler — the model only
/// consumes discovery metadata, never the handler.
fn register_cap(caps: &mut CapabilityRegistry, d: CapabilityDescriptor) {
    caps.register(d, None::<Arc<dyn synvelis_gateway::CapabilityOperation>>)
        .unwrap();
}

/// Registers the six `Available` fixture capabilities.
fn register_invocable_capabilities(caps: &mut CapabilityRegistry) {
    register_cap(
        caps,
        descriptor(
            "synvelis.midi.add-note",
            cver(1, 0),
            OperationDomain::ProjectMutation,
            sref("midi.add-note.input", 1, 0),
            sref("midi.add-note.output", 1, 0),
            TargetContract::Resource {
                kind: ResourceKind::Track,
            },
            Availability::Available,
        ),
    );
    register_cap(
        caps,
        descriptor(
            "synvelis.transport.play",
            cver(1, 0),
            OperationDomain::RuntimeControl,
            sref("transport.play.input", 1, 0),
            sref("transport.play.output", 1, 0),
            TargetContract::None,
            Availability::Available,
        ),
    );
    register_cap(
        caps,
        descriptor(
            "synvelis.query.track-info",
            cver(1, 0),
            OperationDomain::Query,
            sref("query.track-info.input", 1, 0),
            sref("query.track-info.output", 1, 0),
            TargetContract::Resource {
                kind: ResourceKind::Track,
            },
            Availability::Available,
        ),
    );
    register_cap(
        caps,
        descriptor(
            "synvelis.clip.split",
            cver(1, 0),
            OperationDomain::ProjectMutation,
            sref("clip.split.input", 1, 0),
            sref("clip.split.output", 1, 0),
            TargetContract::Resource {
                kind: ResourceKind::Clip,
            },
            Availability::Available,
        ),
    );
    register_cap(
        caps,
        descriptor(
            "synvelis.batch.move",
            cver(1, 0),
            OperationDomain::ProjectMutation,
            sref("batch.move.input", 1, 0),
            sref("batch.move.output", 1, 0),
            TargetContract::ResourceSet {
                kind: ResourceKind::Clip,
            },
            Availability::Available,
        ),
    );
    register_cap(
        caps,
        descriptor(
            "synvelis.clip.create",
            cver(1, 0),
            OperationDomain::ProjectMutation,
            sref("clip.create.input", 1, 0),
            sref("clip.create.output", 1, 0),
            TargetContract::Create {
                parent: ResourceKind::Track,
                creates: ResourceKind::Clip,
            },
            Availability::Available,
        ),
    );
}

/// Registers the Disabled/Planned/Retired fixture capabilities.
fn register_blocked_capabilities(caps: &mut CapabilityRegistry) {
    register_cap(
        caps,
        descriptor(
            "synvelis.legacy.thing",
            cver(1, 0),
            OperationDomain::Query,
            sref("query.track-info.input", 1, 0),
            sref("query.track-info.output", 1, 0),
            TargetContract::None,
            Availability::Disabled {
                reason: "superseded by synvelis.query.track-info".to_owned(),
            },
        ),
    );
    register_cap(
        caps,
        descriptor(
            "synvelis.future.thing",
            cver(1, 0),
            OperationDomain::Query,
            sref("query.track-info.input", 1, 0),
            sref("query.track-info.output", 1, 0),
            TargetContract::None,
            Availability::Planned,
        ),
    );
    register_cap(
        caps,
        descriptor(
            "synvelis.old.thing",
            cver(1, 0),
            OperationDomain::Query,
            sref("query.track-info.input", 1, 0),
            sref("query.track-info.output", 1, 0),
            TargetContract::None,
            Availability::Retired,
        ),
    );
}

/// Registers the fixture capability set. Returns the registries.
pub fn sample_registries() -> (CapabilityRegistry, SchemaRegistry) {
    let mut caps = CapabilityRegistry::new();
    let mut schemas = SchemaRegistry::new();
    register_core_schemas(&mut schemas);
    register_split_schemas(&mut schemas);
    register_create_schemas(&mut schemas);
    register_invocable_capabilities(&mut caps);
    register_blocked_capabilities(&mut caps);
    (caps, schemas)
}

/// The canonical surface spec covering every verdict/visibility path.
pub fn sample_surface() -> ScriptSurfaceSpec {
    let entry = |name: &str, id: &str, version: CapabilityVersion| ScriptSurfaceEntry {
        name: name.to_owned(),
        capability: synvelis_gateway::CapabilityRef {
            id: vid(id),
            version,
        },
        visibility: ScriptVisibility::Public,
        description: None,
    };
    ScriptSurfaceSpec {
        schema_version: synvelis_script::SURFACE_SCHEMA_VERSION,
        entries: vec![
            entry("addNote", "synvelis.midi.add-note", cver(1, 0)),
            entry("play", "synvelis.transport.play", cver(1, 0)),
            entry("trackInfo", "synvelis.query.track-info", cver(1, 0)),
            entry("splitClip", "synvelis.clip.split", cver(1, 0)),
            entry("moveClips", "synvelis.batch.move", cver(1, 0)),
            entry("createClip", "synvelis.clip.create", cver(1, 0)),
            entry("legacyOp", "synvelis.legacy.thing", cver(1, 0)),
            entry("futureOp", "synvelis.future.thing", cver(1, 0)),
            entry("oldOp", "synvelis.old.thing", cver(1, 0)),
            ScriptSurfaceEntry {
                name: "hiddenOp".to_owned(),
                capability: synvelis_gateway::CapabilityRef {
                    id: vid("synvelis.midi.add-note"),
                    version: cver(1, 0),
                },
                visibility: ScriptVisibility::Hidden,
                description: None,
            },
            entry("bad name!", "synvelis.query.track-info", cver(1, 0)),
            entry("end", "synvelis.query.track-info", cver(1, 0)),
            entry("newThing", "synvelis.nope.x", cver(1, 0)),
            entry("wrongVer", "synvelis.midi.add-note", cver(9, 9)),
        ],
    }
}

pub fn sample_snapshot() -> RegistrySnapshot {
    let (caps, schemas) = sample_registries();
    RegistrySnapshot::capture(&caps, &schemas).expect("fixture snapshot captures")
}

pub fn sample_model() -> ScriptApiModel {
    ScriptApiModel::build(&sample_snapshot(), &sample_surface()).expect("fixture model builds")
}

pub fn sample_artifacts() -> GeneratedArtifacts {
    GeneratedArtifacts::generate(&sample_model()).expect("fixture artifacts generate")
}

/// A minimal valid [`InvocationCall`] for `addNote` (`ProjectMutation` →
/// `ExpectedRevision` consistency, `Resource{Track}` target).
pub fn add_note_call() -> InvocationCall {
    InvocationCall {
        args: json!({
            "position": {"domain": "project-musical", "start": "960", "len": "480"},
            "pitch": 60,
            "velocity": 100,
            "channel": 0,
            "curve": "exp",
            "gain_db": -3.5,
            "macro": null,
            "tags": ["verse", "hook"],
            "sample_hint": "9223372036854775807",
            "meta": {"inner": true, "label": "take-a"},
            "owner": track_id(),
        }),
        target: Some(ScriptTarget::Resource {
            kind: ResourceKind::Track,
            id: track_id(),
        }),
        consistency: ScriptConsistency::ExpectedRevision { revision: 7 },
        project: project_id(),
        request_id: "req-fixture-0001".to_owned(),
        task: None,
        grant: None,
        idempotency_key: Some("idem-fixture-1".to_owned()),
    }
}

/// A minimal valid call for `play` (`RuntimeControl` → `Unconditional`,
/// `TargetContract::None`).
pub fn play_call() -> InvocationCall {
    InvocationCall {
        args: json!({}),
        target: None,
        consistency: ScriptConsistency::Unconditional,
        project: project_id(),
        request_id: "req-fixture-0002".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
    }
}

/// A minimal valid call for `trackInfo` (Query → `Latest`,
/// `Resource{Track}` target).
pub fn track_info_call() -> InvocationCall {
    InvocationCall {
        args: json!({}),
        target: Some(ScriptTarget::Resource {
            kind: ResourceKind::Track,
            id: track_id(),
        }),
        consistency: ScriptConsistency::Latest,
        project: project_id(),
        request_id: "req-fixture-0003".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
    }
}

/// The canonical serialized snapshot fixture (regenerate with
/// `WRITE_SNAPSHOT_FIXTURE=1 cargo test -p synvelis-script --test golden`).
pub fn snapshot_fixture_json() -> Value {
    serde_json::from_str(include_str!("../fixtures/registry_snapshot.json"))
        .expect("snapshot fixture parses")
}

pub fn binding_set() -> BindingSet {
    BindingSet::generate(&sample_model()).expect("fixture binding set")
}

pub fn stub() -> LuauStub {
    LuauStub::generate(&sample_model()).expect("fixture stub")
}
