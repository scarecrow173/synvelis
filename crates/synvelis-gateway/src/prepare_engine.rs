//! GW-04 preparation engine — the `synvelis.operation.prepare`
//! capability executor (GTC-005).
//!
//! One `Proposal`-domain capability accepts `{operation}` triples; the
//! engine pins the request's consistency to a committed base through
//! the GW-03 snapshot seam, lowers the named operation to a
//! [`ProjectOp`], validates it on the pin, applies it on a scratch
//! clone for the candidate snapshot, derives the real effects from the
//! before/after attribute diff, and runs GW-02's effect-aware
//! authorization plus the emission boundary before a proposal is
//! recorded. The live [`synvelis_project::Project`] is never touched —
//! preparation is non-destructive by construction: the only mutable
//! value in the pipeline is a private clone.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{Map, Value};
use synvelis_project::variant::{AdoptIntent, BranchEditOp, BranchEditOpIntent, BranchRevision};
use synvelis_project::{
    ApplyApproval, CounterIdGenerator, Operation, Project, ProjectOp, ProjectRevision, VariantSetId,
};

use crate::authorization::AuthorizationDecision;
use crate::capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityVersion,
    DataAccessClass, DataAccessSet, EffectKind, EffectSet, ExecutionContract, OperationDomain,
    PermissionKind, PermissionSet, TargetContract,
};
use crate::context_engine::ContextProjectionEngine;
use crate::grant::ResourceOracle;
use crate::id::ProposalRef;
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::prepare::{
    self, NamedOperation, OpSpec, PrepareAuthorizer, PreparedMutation, PreparedOperation,
    PreparedRecord, SemanticCommand,
};
use crate::proposal::ProposalStore;
use crate::registry::CapabilityOperation;
use crate::request::{
    Consistency, OperationRequest, OperationTarget, PrincipalKind, ResourceKind, ResourceRef,
    TimeDomain,
};
use crate::schema::{FieldSchema, SchemaRef, SchemaVersion, ValueSchema};
use crate::snapshot::PinnedSnapshot;

const PREPARE_ID: &str = "synvelis.operation.prepare";
const PREPARE_INPUT: &str = "synvelis.operation.prepare.input";
const PREPARE_OUTPUT: &str = "synvelis.operation.prepare.output";
const OP_OUTPUT: &str = "synvelis.operation.output";

/// Retention/amount bounds for the engine — same defensive budget
/// philosophy as [`crate::context_engine::ContextBudget`].
#[derive(Clone, Debug)]
pub struct PrepareBudget {
    /// Maximum members inside one batch operation.
    pub max_ops: usize,
    /// Maximum retained proposals (oldest evicted first).
    pub max_proposals: usize,
}

impl Default for PrepareBudget {
    fn default() -> Self {
        Self {
            max_ops: 64,
            max_proposals: 64,
        }
    }
}

/// First raw id of the output-binding placeholder range — mirrors
/// `synvelis_automation::macros::OUTPUT_BINDING_ID_BASE` (the
/// automation crate is only a dev-dependency here, so the shared
/// wire constant is mirrored verbatim). Placeholders are
/// `<kind>-<32hex>` tagged ids at `BASE + slot` for `slot in
/// 0..MAX_BINDING_SLOTS`, far above every issuable id — they can
/// never collide with a real resource id.
const OUTPUT_BINDING_ID_BASE: u128 = (1u128 << 96) + (1u128 << 32);
/// Placeholder slots are `0..64` (`MAX_MACRO_BINDINGS` on the
/// compiler side).
const MAX_BINDING_SLOTS: u128 = 64;

/// The lowering result of [`PrepareEngine::lower_named`]:
/// normalized member commands, the retained [`PreparedMutation`], and
/// the AUTX-04 bindings seam's deterministic scratch post-state
/// (`Some` only for a batch carrying non-empty `bindings`).
type Lowered = (Vec<SemanticCommand>, PreparedMutation, Option<Arc<Project>>);

/// One validated `bindings[]` declaration — the wire contract of
/// `synvelis_automation::macros::MacroBindingWire`
/// (`{name, member, kind}`). Resolution keys on `(member, kind)`
/// positionally; `name` is traceability for diagnostics only.
struct BindingDecl {
    /// The declared binding name.
    name: String,
    /// Producing member index into `ops[]`.
    member: usize,
    /// The resource kind the producer must create exactly one of.
    kind: ResourceKind,
}

/// Parses a reserved-range placeholder tagged id
/// (`<kind-tag>-<32hex>` whose raw sits in
/// `BASE..BASE+MAX_BINDING_SLOTS`) into `(slot, kind_tag)` — mirrors
/// `synvelis_automation::macros::is_reserved_placeholder`, which
/// deliberately does not validate the kind prefix; the caller
/// matches it against the declared binding kind.
fn parse_placeholder(s: &str) -> Option<(usize, &str)> {
    let (tag, hex) = s.rsplit_once('-')?;
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let raw = u128::from_str_radix(hex, 16).ok()?;
    let slot = raw.checked_sub(OUTPUT_BINDING_ID_BASE)?;
    (slot < MAX_BINDING_SLOTS).then_some((slot as usize, tag))
}

/// Rewrites resolved output-binding placeholders inside one batch
/// member's wire JSON (`target`/`args` surfaces). Every
/// reserved-range string must name a declared binding whose
/// producer member already applied — undeclared slots, kind-tag
/// mismatches, backward/self references, placeholders left
/// unresolvable by a failed producer, and placeholder object keys
/// are all typed rejections. Ids are never guessed: an unresolvable
/// placeholder fails the whole batch.
#[allow(clippy::result_large_err)] // typed envelope is the contract
fn rewrite_member(
    value: &Value,
    member_index: usize,
    decls: &[BindingDecl],
    resolved: &[Option<ResourceRef>],
    path: &str,
) -> Result<Value, ErrorEnvelope> {
    match value {
        Value::String(s) => match parse_placeholder(s) {
            None => Ok(value.clone()),
            Some((slot, tag)) => {
                let decl = decls.get(slot).ok_or_else(|| {
                    prepare::violation(
                        path,
                        format!("placeholder {s} names undeclared binding slot {slot}"),
                    )
                })?;
                let kind = ResourceKind::parse(tag).map_err(ErrorEnvelope::from)?;
                if kind != decl.kind {
                    return Err(prepare::violation(
                        path,
                        format!(
                            "placeholder {s} kind does not match binding '{}'",
                            decl.name
                        ),
                    ));
                }
                if decl.member >= member_index {
                    return Err(prepare::violation(
                        path,
                        format!(
                            "placeholder {s} references binding '{}' produced by member {} — \
                             not yet applied at member {member_index}",
                            decl.name, decl.member
                        ),
                    ));
                }
                let Some(resource) = resolved[slot] else {
                    return Err(GatewayError::PreparationRejected {
                        reason: format!(
                            "output binding '{}' could not be resolved: its producer member \
                             did not complete",
                            decl.name
                        ),
                    }
                    .into());
                };
                Ok(Value::String(prepare::ref_tagged(&resource)))
            }
        },
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| rewrite_member(v, member_index, decls, resolved, &format!("{path}[{i}]")))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, v) in map {
                if parse_placeholder(key).is_some() {
                    return Err(prepare::violation(
                        &format!("{path}.{key}"),
                        "a placeholder cannot appear as an object key",
                    ));
                }
                out.insert(
                    key.clone(),
                    rewrite_member(v, member_index, decls, resolved, &format!("{path}.{key}"))?,
                );
            }
            Ok(Value::Object(out))
        }
        _ => Ok(value.clone()),
    }
}

/// The preparation engine — executes `synvelis.operation.prepare`
/// over the GW-03 snapshot seam and a GW-02 prepare authorizer.
/// Retention lives in the shared [`ProposalStore`] — the same store
/// GW-05's lifecycle machines, gate, and preview read.
pub struct PrepareEngine {
    context: Arc<ContextProjectionEngine>,
    authorizer: Arc<dyn PrepareAuthorizer>,
    budget: PrepareBudget,
    descriptor: CapabilityDescriptor,
    ops: BTreeMap<CapabilityRef, OpSpec>,
    /// CB-10: Creative Branch capabilities — lowered to
    /// [`crate::variant::VariantIntent`]s, never to `ProjectOp`s.
    variant_ops: BTreeMap<CapabilityRef, VariantOpSpec>,
    /// synvelis#517: `synvelis.render.*` capabilities — lowered to
    /// [`crate::variant::RenderIntent`]s and dispatched through the
    /// commit gate's render port; they never mutate the project.
    render_ops: BTreeMap<CapabilityRef, VariantOpSpec>,
    /// CB-10: shared variant ports for prepare-time simulation —
    /// `None` fails variant preparation closed (host misconfiguration,
    /// same posture as the media gate).
    variant: Option<Arc<crate::variant::VariantPorts>>,
    proposals: Arc<ProposalStore>,
    /// The GW-07 verified-media gate — `synvelis.media.register`
    /// resolves its staged-media ref through this seam. `None`
    /// fails the op closed: unverifiable media is never registerable.
    media: Option<Arc<dyn crate::media::MediaGate>>,
}

/// One CB-10 entry in the variant-op table: descriptor + input schema.
/// Lowering is an engine method (the edit op's inner named operation
/// resolves through `ops`), so no `LowerFn` is stored.
struct VariantOpSpec {
    /// The capability descriptor registered for the branch op.
    descriptor: CapabilityDescriptor,
    /// The registered input schema.
    input: ValueSchema,
}

fn obj(fields: &[(&str, ValueSchema)], required: &[&str], deny_unknown: bool) -> ValueSchema {
    let req: BTreeSet<&str> = required.iter().copied().collect();
    ValueSchema::Object {
        fields: fields
            .iter()
            .map(|(name, schema)| {
                (
                    (*name).to_owned(),
                    if req.contains(name) {
                        FieldSchema::required(schema.clone())
                    } else {
                        FieldSchema::optional(schema.clone())
                    },
                )
            })
            .collect(),
        deny_unknown,
    }
}

fn int(min: i64, max: i64) -> ValueSchema {
    ValueSchema::Integer {
        min: Some(min),
        max: Some(max),
    }
}

fn str_(max_len: usize) -> ValueSchema {
    ValueSchema::String {
        max_len: Some(max_len),
    }
}

fn id_of(kind: ResourceKind) -> ValueSchema {
    ValueSchema::Id { kind }
}

fn note_ref() -> ValueSchema {
    id_of(ResourceKind::Note)
}

fn range_in(domain: TimeDomain) -> ValueSchema {
    ValueSchema::TimeRange { domain }
}

fn point_in(domain: TimeDomain) -> ValueSchema {
    obj(
        &[
            (
                "domain",
                ValueSchema::Enum {
                    values: vec![domain_name(domain).to_owned()],
                },
            ),
            ("tick", int(0, i64::MAX)),
        ],
        &["domain", "tick"],
        true,
    )
}

fn domain_name(domain: TimeDomain) -> &'static str {
    match domain {
        TimeDomain::ProjectMusical => "project-musical",
        TimeDomain::Media => "media",
        TimeDomain::SourceNative => "source-native",
    }
}

/// Loose object presence — the typed serde of the project types (all
/// `deny_unknown_fields` + validating conversions) enforces the real
/// shape at lowering; the schema's job here is only structure.
fn any_obj() -> ValueSchema {
    ValueSchema::Object {
        fields: BTreeMap::new(),
        deny_unknown: false,
    }
}

fn empty_args() -> ValueSchema {
    obj(&[], &[], true)
}

impl PrepareEngine {
    /// The `synvelis.operation.prepare` descriptor — `Proposal`
    /// domain (stage [`crate::authorization::AuthorizationStage::PrepareProposal`]),
    /// target-less (the semantic op's own target rides inside args),
    /// effect bound `Read` (preparation only reads a pinned base —
    /// the *derived* effects are data in its output, evaluated by
    /// GW-02, not effects of this invocation).
    ///
    /// # Panics
    /// Panics if the compile-time capability id is malformed (it is not).
    #[must_use]
    pub fn descriptor() -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new(PREPARE_ID).expect("capability id is valid"),
            version: CapabilityVersion::new(1, 0),
            domain: OperationDomain::Proposal,
            input_schema: SchemaRef::new(PREPARE_INPUT, 1, 0),
            output_schema: SchemaRef::new(PREPARE_OUTPUT, 1, 0),
            target: TargetContract::None,
            effect_bound: EffectSet::of([EffectKind::Read]),
            required_permissions: PermissionSet::of([PermissionKind::MutateProject]),
            data_access: DataAccessSet::of([DataAccessClass::MetadataRead]),
            execution: ExecutionContract::Sync,
            availability: Availability::Available,
        }
    }

    /// A mutation-op descriptor — registered without an executor so a
    /// direct `invoke` reports `NoExecutor` until GW-06 wires the
    /// apply path (the op is invocable only *through* prepare).
    fn op_descriptor(
        id: &str,
        target: TargetContract,
        effects: &[EffectKind],
        input: &str,
    ) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new(id).expect("capability id is valid"),
            version: CapabilityVersion::new(1, 0),
            domain: OperationDomain::ProjectMutation,
            input_schema: SchemaRef::new(input, 1, 0),
            output_schema: SchemaRef::new(OP_OUTPUT, 1, 0),
            target,
            effect_bound: EffectSet::of(effects.iter().copied()),
            required_permissions: PermissionSet::of([PermissionKind::MutateProject]),
            data_access: DataAccessSet::of([DataAccessClass::MetadataRead]),
            execution: ExecutionContract::Sync,
            availability: Availability::Available,
        }
    }

    /// The semantic-operation table — every `ProjectMutation`
    /// capability GW-04 can prepare. Each entry is the op's single
    /// contract for both preparation (this engine's lowering) and
    /// the later apply path (GW-06 invokes the same descriptor).
    #[allow(clippy::too_many_lines)] // one descriptor+schema per op — a flat table is clearer
    fn op_table() -> Vec<(
        &'static str,
        TargetContract,
        &'static [EffectKind],
        ValueSchema,
    )> {
        use EffectKind::{Create, Delete, Read, Write};
        vec![
            (
                "synvelis.track.create",
                TargetContract::Create {
                    parent: ResourceKind::Project,
                    creates: ResourceKind::Track,
                },
                &[Create, Write] as &[_],
                obj(&[("name", str_(256))], &["name"], true),
            ),
            (
                "synvelis.track.remove",
                TargetContract::Resource {
                    kind: ResourceKind::Track,
                },
                &[Delete],
                empty_args(),
            ),
            (
                "synvelis.lane.create",
                TargetContract::Create {
                    parent: ResourceKind::Track,
                    creates: ResourceKind::Lane,
                },
                &[Create, Write],
                empty_args(),
            ),
            (
                "synvelis.lane.remove",
                TargetContract::Resource {
                    kind: ResourceKind::Lane,
                },
                &[Delete],
                empty_args(),
            ),
            (
                "synvelis.clip.add",
                TargetContract::Create {
                    parent: ResourceKind::Lane,
                    creates: ResourceKind::Clip,
                },
                &[Create, Write],
                obj(
                    &[
                        ("source", id_of(ResourceKind::Source)),
                        ("placement", any_obj()),
                        ("playback", any_obj()),
                    ],
                    &["source", "placement", "playback"],
                    true,
                ),
            ),
            (
                "synvelis.clip.remove",
                TargetContract::Resource {
                    kind: ResourceKind::Clip,
                },
                &[Delete],
                empty_args(),
            ),
            (
                "synvelis.clip.move",
                TargetContract::Resource {
                    kind: ResourceKind::Clip,
                },
                &[Write],
                obj(&[("anchor", any_obj())], &["anchor"], true),
            ),
            (
                "synvelis.clip.resize",
                TargetContract::Resource {
                    kind: ResourceKind::Clip,
                },
                &[Write],
                obj(&[("extent", any_obj())], &["extent"], true),
            ),
            (
                "synvelis.clip.playback",
                TargetContract::Resource {
                    kind: ResourceKind::Clip,
                },
                &[Write],
                obj(&[("playback", any_obj())], &["playback"], true),
            ),
            (
                "synvelis.clip.mute",
                TargetContract::Resource {
                    kind: ResourceKind::Clip,
                },
                &[Write],
                obj(&[("muted", ValueSchema::Bool)], &["muted"], true),
            ),
            (
                "synvelis.clip.split",
                TargetContract::Resource {
                    kind: ResourceKind::Clip,
                },
                &[Create, Write],
                obj(
                    &[
                        ("at", int(i64::MIN, i64::MAX)),
                        ("clip_b_source_offset", int(0, i64::MAX)),
                    ],
                    &["at", "clip_b_source_offset"],
                    true,
                ),
            ),
            (
                "synvelis.clip.detach-source",
                TargetContract::Resource {
                    kind: ResourceKind::Clip,
                },
                &[Read, Create, Write],
                empty_args(),
            ),
            (
                "synvelis.clip.remove-in-range",
                TargetContract::ResourceSet {
                    kind: ResourceKind::Lane,
                },
                &[Delete],
                obj(
                    &[("range", range_in(TimeDomain::ProjectMusical))],
                    &["range"],
                    true,
                ),
            ),
            (
                "synvelis.note.add",
                TargetContract::Resource {
                    kind: ResourceKind::Clip,
                },
                &[Create, Write],
                obj(
                    &[
                        ("key", int(0, 127)),
                        ("start", any_obj()),
                        ("len", any_obj()),
                    ],
                    &["key", "start", "len"],
                    true,
                ),
            ),
            (
                "synvelis.note.remove",
                TargetContract::Resource {
                    kind: ResourceKind::Note,
                },
                &[Delete],
                empty_args(),
            ),
            (
                "synvelis.midi.source.create",
                TargetContract::Create {
                    parent: ResourceKind::Project,
                    creates: ResourceKind::Source,
                },
                &[Create],
                obj(&[("division", any_obj())], &["division"], true),
            ),
            (
                "synvelis.midi.source.remove",
                TargetContract::Resource {
                    kind: ResourceKind::Source,
                },
                &[Delete],
                empty_args(),
            ),
            (
                "synvelis.midi.note.add",
                TargetContract::Resource {
                    kind: ResourceKind::Source,
                },
                &[Create, Write],
                obj(
                    &[
                        ("key", int(0, 127)),
                        ("velocity", int(1, 127)),
                        ("range", range_in(TimeDomain::SourceNative)),
                    ],
                    &["key", "velocity", "range"],
                    true,
                ),
            ),
            (
                "synvelis.midi.note.remove",
                TargetContract::Resource {
                    kind: ResourceKind::Source,
                },
                &[Delete, Write],
                obj(&[("note", note_ref())], &["note"], true),
            ),
            (
                "synvelis.midi.note.set-velocity",
                TargetContract::Resource {
                    kind: ResourceKind::Source,
                },
                &[Write],
                obj(
                    &[("note", note_ref()), ("velocity", int(1, 127))],
                    &["note", "velocity"],
                    true,
                ),
            ),
            (
                "synvelis.midi.note.move",
                TargetContract::Resource {
                    kind: ResourceKind::Source,
                },
                &[Write],
                obj(
                    &[
                        ("note", note_ref()),
                        ("position", point_in(TimeDomain::SourceNative)),
                    ],
                    &["note", "position"],
                    true,
                ),
            ),
            (
                "synvelis.asset.register",
                TargetContract::Create {
                    parent: ResourceKind::Project,
                    creates: ResourceKind::Asset,
                },
                &[Create],
                obj(&[("digest", str_(256))], &["digest"], true),
            ),
            (
                "synvelis.source.register",
                TargetContract::Create {
                    parent: ResourceKind::Asset,
                    creates: ResourceKind::Source,
                },
                &[Create],
                obj(&[("content_digest", str_(256))], &["content_digest"], true),
            ),
            (
                crate::job_engine::MEDIA_REGISTER_ID,
                TargetContract::Create {
                    parent: ResourceKind::Asset,
                    creates: ResourceKind::Source,
                },
                &[Create],
                // The staged-media ref only — the content digest is
                // gate-resolved at lowering, never agent-declared.
                obj(&[("media", str_(256))], &["media"], true),
            ),
            (
                "synvelis.source.set-state",
                TargetContract::Resource {
                    kind: ResourceKind::Source,
                },
                &[Write],
                obj(&[("state", any_obj())], &["state"], true),
            ),
            (
                "synvelis.clock-epoch.register",
                TargetContract::None,
                &[Write],
                obj(&[("description", str_(256))], &["description"], true),
            ),
        ]
    }

    /// The engine over the GW-03 context engine (its snapshot seam
    /// supplies every pin), a prepare authorizer, and the shared
    /// proposal store (`budget.max_proposals` is mirrored into it at
    /// construction — pass a store already sized for the rig).
    #[must_use]
    pub fn new(
        context: Arc<ContextProjectionEngine>,
        authorizer: Arc<dyn PrepareAuthorizer>,
        budget: PrepareBudget,
        proposals: Arc<ProposalStore>,
    ) -> Self {
        Self::with_media(context, authorizer, budget, proposals, None)
    }

    /// The engine with the GW-07 verified-media gate wired —
    /// `synvelis.media.register` resolves staged-media refs through
    /// `media`; absent a gate the op fails closed.
    #[must_use]
    pub fn with_media(
        context: Arc<ContextProjectionEngine>,
        authorizer: Arc<dyn PrepareAuthorizer>,
        mut budget: PrepareBudget,
        proposals: Arc<ProposalStore>,
        media: Option<Arc<dyn crate::media::MediaGate>>,
    ) -> Self {
        budget.max_ops = budget.max_ops.max(1);
        budget.max_proposals = budget.max_proposals.max(1);
        Self {
            context,
            authorizer,
            budget,
            descriptor: Self::descriptor(),
            ops: Self::op_specs(),
            variant_ops: Self::variant_specs(),
            render_ops: Self::render_specs(),
            variant: None,
            proposals,
            media,
        }
    }

    /// Wires the CB-10 variant ports — preparation simulates branch
    /// mutations on a scratch controller through these ports so the
    /// retained candidate/effects are real. `None` fails variant
    /// preparation closed (same posture as the media gate).
    #[must_use]
    pub fn with_variant_ports(mut self, ports: Option<Arc<crate::variant::VariantPorts>>) -> Self {
        self.variant = ports;
        self
    }

    fn op_specs() -> BTreeMap<CapabilityRef, OpSpec> {
        let mut map = BTreeMap::new();
        for (id, target, effects, input) in Self::op_table() {
            let lower = prepare::lower_for(id).expect("every table entry lowers");
            let descriptor = Self::op_descriptor(id, target, effects, &format!("{id}.input"));
            map.insert(
                descriptor.capability_ref(),
                OpSpec {
                    descriptor,
                    input,
                    lower,
                },
            );
        }
        // The batch capability lowers through the same table — its
        // members are other table entries, resolved recursively (the
        // engine's `lower_named` intercepts it before `lower` runs).
        let batch = Self::op_descriptor(
            "synvelis.operation.batch",
            TargetContract::None,
            &[
                EffectKind::Read,
                EffectKind::Write,
                EffectKind::Create,
                EffectKind::Delete,
            ],
            "synvelis.operation.batch.input",
        );
        map.insert(
            batch.capability_ref(),
            OpSpec {
                descriptor: batch,
                input: obj(
                    &[
                        (
                            "ops",
                            ValueSchema::Array {
                                item: Box::new(any_obj()),
                                max_len: Some(64),
                            },
                        ),
                        // AUTX-04: output-binding declarations for
                        // create-then-reference — resolved by the
                        // scratch-ID/rewrite seam in `lower_bound_batch`.
                        (
                            "bindings",
                            ValueSchema::Array {
                                item: Box::new(any_obj()),
                                max_len: Some(64),
                            },
                        ),
                    ],
                    &["ops"],
                    true,
                ),
                lower: |_, _| {
                    Err(GatewayError::Internal {
                        reason: "batch lowers through member recursion".to_owned(),
                    })
                },
            },
        );
        map
    }

    /// The CB-10 variant-op table — `synvelis.branch.{fork,edit,
    /// adopt,discard}` descriptors + input schemas. These never join
    /// `ops`: their lowering produces [`PreparedMutation::Variant`],
    /// not a `ProjectOp`.
    fn variant_specs() -> BTreeMap<CapabilityRef, VariantOpSpec> {
        crate::variant::mutation_specs()
            .into_iter()
            .map(|(descriptor, input)| {
                (
                    descriptor.capability_ref(),
                    VariantOpSpec { descriptor, input },
                )
            })
            .collect()
    }

    /// The synvelis#517 render-op table — `synvelis.render.*`
    /// descriptors + input schemas. Like the variant table these never
    /// join `ops`: their lowering produces
    /// [`PreparedMutation::Render`], dispatched at commit through the
    /// host's render port.
    fn render_specs() -> BTreeMap<CapabilityRef, VariantOpSpec> {
        crate::variant::render_specs()
            .into_iter()
            .map(|(descriptor, input)| {
                (
                    descriptor.capability_ref(),
                    VariantOpSpec { descriptor, input },
                )
            })
            .collect()
    }

    /// Registers every op input schema, the shared output schema, the
    /// mutation-op descriptors (no executors — apply is GW-06), and
    /// the `synvelis.operation.prepare` capability with `self`.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — schema or capability registration conflict.
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    pub fn install(self: &Arc<Self>, gateway: &mut crate::Gateway) -> Result<(), ErrorEnvelope> {
        for spec in self.ops.values() {
            gateway
                .schemas_mut()
                .register(
                    spec.descriptor.input_schema.name.clone(),
                    SchemaVersion::new(1, 0),
                    spec.input.clone(),
                )
                .map_err(ErrorEnvelope::from)?;
        }
        for spec in self.variant_ops.values() {
            gateway
                .schemas_mut()
                .register(
                    spec.descriptor.input_schema.name.clone(),
                    SchemaVersion::new(1, 0),
                    spec.input.clone(),
                )
                .map_err(ErrorEnvelope::from)?;
        }
        for spec in self.render_ops.values() {
            gateway
                .schemas_mut()
                .register(
                    spec.descriptor.input_schema.name.clone(),
                    SchemaVersion::new(1, 0),
                    spec.input.clone(),
                )
                .map_err(ErrorEnvelope::from)?;
        }
        for name in [OP_OUTPUT, PREPARE_OUTPUT] {
            gateway
                .schemas_mut()
                .register(
                    (*name).to_owned(),
                    SchemaVersion::new(1, 0),
                    ValueSchema::Object {
                        fields: BTreeMap::new(),
                        deny_unknown: false,
                    },
                )
                .map_err(ErrorEnvelope::from)?;
        }
        gateway
            .schemas_mut()
            .register(
                PREPARE_INPUT.to_owned(),
                SchemaVersion::new(1, 0),
                Self::input_schema(),
            )
            .map_err(ErrorEnvelope::from)?;
        for spec in self.ops.values() {
            gateway.register_capability(spec.descriptor.clone(), None)?;
        }
        for spec in self.variant_ops.values() {
            gateway.register_capability(spec.descriptor.clone(), None)?;
        }
        for spec in self.render_ops.values() {
            gateway.register_capability(spec.descriptor.clone(), None)?;
        }
        gateway.register_capability(self.descriptor.clone(), Some(self.clone()))
    }

    fn input_schema() -> ValueSchema {
        obj(
            &[(
                "operation",
                ValueSchema::Object {
                    fields: [
                        ("capability".to_owned(), FieldSchema::required(any_obj())),
                        ("target".to_owned(), FieldSchema::optional(any_obj())),
                        ("args".to_owned(), FieldSchema::optional(any_obj())),
                    ]
                    .into_iter()
                    .collect(),
                    deny_unknown: true,
                },
            )],
            &["operation"],
            true,
        )
    }

    /// Looks up a retained proposal — the GW-06 commit seam and the
    /// test surface for `PreparedRecord`. Present only while the
    /// proposal is `Prepared`; expiry/invalidation drops it.
    #[must_use]
    pub fn prepared(&self, proposal: &ProposalRef) -> Option<Arc<PreparedRecord>> {
        self.proposals.record(proposal)
    }

    /// Number of retained proposals (test/introspection).
    #[must_use]
    pub fn proposal_count(&self) -> usize {
        self.proposals.count()
    }

    /// Evicts a retained proposal — the host's retention control;
    /// the proposal transitions to `Invalidated` and its record drops.
    #[must_use]
    pub fn evict(&self, proposal: &ProposalRef) -> bool {
        self.proposals.evict(proposal)
    }

    /// Resolves the request's consistency to a pinned base through
    /// the GW-03 engine — `ExpectedRevision` names the base revision
    /// the eventual apply expects; it asserts nothing about liveness
    /// at prepare time.
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn pin_base(&self, req: &OperationRequest) -> Result<PinnedSnapshot, ErrorEnvelope> {
        match req.consistency() {
            Consistency::ExpectedRevision { revision } => {
                self.context.pin_revision(req.project(), *revision)
            }
            Consistency::Latest
            | Consistency::ReadAt { .. }
            | Consistency::FixedSnapshot { .. } => self.context.pin_for(req),
            Consistency::Unconditional => Err(GatewayError::Internal {
                reason: "prepare bound an unconditional consistency".to_owned(),
            }
            .into()),
        }
    }

    /// The gated `synvelis.media.register` lowering — the staged-media
    /// ref must resolve to a `Verified` record owned by this caller
    /// and project (unknown/rejected/foreign share one blind deny),
    /// and the resulting `RegisterSource` carries the gate's
    /// re-derived content digest, never an agent-declared one.
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn lower_media(
        &self,
        req: &OperationRequest,
        target: &OperationTarget,
        args: &Value,
    ) -> Result<ProjectOp, ErrorEnvelope> {
        let Some(gate) = &self.media else {
            // Host misconfiguration, not an authorization verdict.
            return Err(GatewayError::Internal {
                reason: "no media verification authority is wired".to_owned(),
            }
            .into());
        };
        let media = crate::MediaRef::new(
            args.get("media")
                .and_then(Value::as_str)
                .ok_or_else(|| prepare::violation("args.media", "expected a string"))?,
        )
        .map_err(ErrorEnvelope::from)?;
        let verified = gate
            .verified_media(req.caller().principal(), req.project(), &media)
            .ok_or_else(|| GatewayError::Denied {
                stage: crate::authorization::AuthorizationStage::PrepareProposal,
                reason: Some("media is not registerable".to_owned()),
            })?;
        let asset_id = match target {
            OperationTarget::Create {
                parent: ResourceRef::Asset(id),
                ..
            } => *id,
            _ => {
                return Err(GatewayError::Internal {
                    reason: "media.register expected an asset create target".to_owned(),
                }
                .into());
            }
        };
        Ok(ProjectOp::RegisterSource {
            asset_id,
            content_digest: verified.content_digest,
        })
    }

    /// Lowers one named operation to `(commands, ProjectOp, scratch)`
    /// — recursing through the batch capability so member commands are
    /// normalized individually. `req` is the bound request the gated
    /// lowerings (verified media) evaluate ownership against; `base`
    /// is the pinned project state the AUTX-04 bindings seam clones
    /// for scratch id resolution (unused by every other lowering).
    ///
    /// The third return element is `Some` only for a batch carrying
    /// non-empty `bindings`: the seam's deterministic post-state —
    /// the same id series and member order `ProjectOp::Composite`
    /// reproduces at commit — which the provisional gate resolves
    /// transaction-created resources against. It is `None` for every
    /// other operation.
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    #[allow(clippy::too_many_lines)]
    fn lower_named(
        &self,
        named: &NamedOperation,
        path: &str,
        req: &OperationRequest,
        base: &Project,
    ) -> Result<Lowered, ErrorEnvelope> {
        let Some(spec) = self.ops.get(&named.capability) else {
            // CB-10: branch capabilities lower to `VariantIntent`s —
            // never `ProjectOp`s — through the same schema/bind gates.
            if let Some(vspec) = self.variant_ops.get(&named.capability) {
                return self.lower_variant(named, vspec, path);
            }
            // synvelis#517: render capabilities lower to `RenderIntent`s
            // — dispatched at commit, never a `ProjectOp`.
            if let Some(rspec) = self.render_ops.get(&named.capability) {
                return Self::lower_render(named, rspec, path);
            }
            let supported: Vec<CapabilityVersion> = self
                .ops
                .keys()
                .chain(self.variant_ops.keys())
                .chain(self.render_ops.keys())
                .filter(|c| c.id == named.capability.id)
                .map(|c| c.version)
                .collect();
            return Err(if supported.is_empty() {
                GatewayError::UnknownCapability {
                    id: named.capability.id.to_string(),
                }
            } else {
                GatewayError::UnsupportedCapabilityVersion {
                    id: named.capability.id.to_string(),
                    requested: named.capability.version,
                    supported,
                }
            }
            .into());
        };
        // Every named op — batch included — validates its args against
        // the registered input schema before lowering.
        spec.input
            .validate(&format!("{path}.args"), &named.args)
            .map_err(ErrorEnvelope::from)?;
        // Batch: members are themselves named operations — lowered by
        // the same machinery into one `Composite`.
        if named.capability.id.as_str() == "synvelis.operation.batch" {
            let members = named
                .args
                .get("ops")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    prepare::violation(&format!("{path}.args.ops"), "expected an array")
                })?;
            if members.is_empty() {
                return Err(prepare::violation(
                    &format!("{path}.args.ops"),
                    "batch requires at least one member",
                ));
            }
            if members.len() > self.budget.max_ops {
                return Err(GatewayError::LimitExceeded {
                    limit: "batch members".to_owned(),
                    max: self.budget.max_ops as u64,
                }
                .into());
            }
            if named.target.is_some() {
                return Err(prepare::violation(
                    &format!("{path}.target"),
                    "batch takes no target",
                ));
            }
            // AUTX-04 (synvelis#263): a non-empty `bindings` array
            // declares create-then-reference output bindings — the
            // GW-04 scratch-ID/rewrite seam resolves compiler
            // placeholders to the ids the sequential scratch apply
            // mints (design-atomic-macro §3 target form).
            if let Some(bindings) = named
                .args
                .get("bindings")
                .and_then(Value::as_array)
                .filter(|b| !b.is_empty())
            {
                return self.lower_bound_batch(path, req, base, members, bindings);
            }
            let mut commands = Vec::with_capacity(members.len());
            let mut ops = Vec::with_capacity(members.len());
            for (i, member) in members.iter().enumerate() {
                let member_named = NamedOperation::parse(member, &format!("{path}.args.ops[{i}]"))?;
                if member_named.capability.id.as_str() == "synvelis.operation.batch" {
                    return Err(prepare::violation(
                        &format!("{path}.args.ops[{i}]"),
                        "nested batch is not allowed",
                    ));
                }
                let (member_commands, member_mutation, _) =
                    self.lower_named(&member_named, &format!("{path}.args.ops[{i}]"), req, base)?;
                let PreparedMutation::Project(member_op) = member_mutation else {
                    return Err(prepare::violation(
                        &format!("{path}.args.ops[{i}]"),
                        "non-project operations cannot be batch members",
                    ));
                };
                commands.extend(member_commands);
                ops.push(member_op);
            }
            return Ok((
                commands,
                PreparedMutation::Project(ProjectOp::Composite { ops }),
                None,
            ));
        }
        // An ordinary op: bind the declared target, lower
        // syntactically (args were schema-validated above).
        let target = crate::gateway::Gateway::bind_target(&spec.descriptor, named.target.as_ref())
            .map_err(ErrorEnvelope::from)?;
        let op = if named.capability.id.as_str() == crate::job_engine::MEDIA_REGISTER_ID {
            // GW-07: the verified-media op lowers through the media
            // gate — the digest is gate-derived, never agent-declared.
            self.lower_media(req, &target, &named.args)?
        } else {
            (spec.lower)(&target, &named.args).map_err(ErrorEnvelope::from)?
        };
        Ok((
            vec![SemanticCommand {
                capability: named.capability.clone(),
                op: prepare::op_name(&op),
                target,
                args: named.args.clone(),
            }],
            PreparedMutation::Project(op),
            None,
        ))
    }

    /// The bound `VariantSet` a variant op targets — `bind_target`
    /// already proved the shape; anything else is an internal error.
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn variant_set_target(target: &OperationTarget) -> Result<VariantSetId, ErrorEnvelope> {
        match target {
            OperationTarget::Resource {
                resource: ResourceRef::VariantSet(id),
            } => Ok(*id),
            _ => Err(GatewayError::Internal {
                reason: "variant op bound a non-variant-set target".to_owned(),
            }
            .into()),
        }
    }

    /// CB-10 lowering — schema-validates args, binds the declared
    /// target, then produces a [`PreparedMutation::Variant`]. The
    /// edit op's `op` member is a *named operation* lowered through
    /// the ordinary `ops` table: its capability joins the proposal's
    /// floor and its `ProjectOp` rides inside
    /// [`BranchEditOp::Project`]. Comp-domain inner ops are rejected —
    /// the gateway table carries no comp capabilities in this slice.
    #[allow(clippy::too_many_lines, clippy::result_large_err)] // typed envelope is the contract
    fn lower_variant(
        &self,
        named: &NamedOperation,
        spec: &VariantOpSpec,
        path: &str,
    ) -> Result<Lowered, ErrorEnvelope> {
        spec.input
            .validate(&format!("{path}.args"), &named.args)
            .map_err(ErrorEnvelope::from)?;
        let target = crate::gateway::Gateway::bind_target(&spec.descriptor, named.target.as_ref())
            .map_err(ErrorEnvelope::from)?;
        let command = |op: &'static str| SemanticCommand {
            capability: named.capability.clone(),
            op,
            target: target.clone(),
            args: named.args.clone(),
        };
        let id = named.capability.id.as_str();
        match id {
            crate::variant::FORK_ID => {
                let scope = crate::variant::lower_fork(&named.args)?;
                Ok((
                    vec![command("branch-fork")],
                    PreparedMutation::Variant(crate::variant::VariantIntent::Fork { scope }),
                    None,
                ))
            }
            crate::variant::EDIT_ID => {
                let set = Self::variant_set_target(&target)?;
                let (branch, revision, op_json) = crate::variant::lower_edit(&named.args)?;
                let inner = NamedOperation::parse(&op_json, &format!("{path}.args.op"))?;
                if inner.capability.id.as_str() == "synvelis.operation.batch"
                    || self.variant_ops.contains_key(&inner.capability)
                {
                    return Err(prepare::violation(
                        &format!("{path}.args.op"),
                        "branch edits accept ordinary project operations only",
                    ));
                }
                let Some(ispec) = self.ops.get(&inner.capability) else {
                    let supported: Vec<CapabilityVersion> = self
                        .ops
                        .keys()
                        .filter(|c| c.id == inner.capability.id)
                        .map(|c| c.version)
                        .collect();
                    return Err(if supported.is_empty() {
                        GatewayError::UnknownCapability {
                            id: inner.capability.id.to_string(),
                        }
                    } else {
                        GatewayError::UnsupportedCapabilityVersion {
                            id: inner.capability.id.to_string(),
                            requested: inner.capability.version,
                            supported,
                        }
                    }
                    .into());
                };
                ispec
                    .input
                    .validate(&format!("{path}.args.op.args"), &inner.args)
                    .map_err(ErrorEnvelope::from)?;
                let itarget =
                    crate::gateway::Gateway::bind_target(&ispec.descriptor, inner.target.as_ref())
                        .map_err(ErrorEnvelope::from)?;
                let iop = (ispec.lower)(&itarget, &inner.args).map_err(ErrorEnvelope::from)?;
                let mut commands = vec![command("branch-edit")];
                commands.push(SemanticCommand {
                    capability: inner.capability.clone(),
                    op: prepare::op_name(&iop),
                    target: itarget,
                    args: inner.args.clone(),
                });
                Ok((
                    commands,
                    PreparedMutation::Variant(crate::variant::VariantIntent::Edit {
                        intent: BranchEditOpIntent {
                            variant_set: set,
                            branch,
                            expected_branch_revision: BranchRevision::new(revision),
                            op: BranchEditOp::Project(iop),
                        },
                    }),
                    None,
                ))
            }
            crate::variant::ADOPT_ID => {
                let set = Self::variant_set_target(&target)?;
                let (branch, project_revision, branch_revision, scope_digest) =
                    crate::variant::lower_adopt(&named.args)?;
                Ok((
                    vec![command("branch-adopt")],
                    PreparedMutation::Variant(crate::variant::VariantIntent::Adopt {
                        intent: AdoptIntent {
                            variant_set: set,
                            branch,
                            expected_project_revision: ProjectRevision(project_revision),
                            expected_branch_revision: BranchRevision::new(branch_revision),
                            expected_scope_digest: scope_digest,
                            // Placeholder — the commit gate substitutes
                            // the consumed approval receipt's id; an
                            // adopt without one fails `Unapproved`.
                            approval: ApplyApproval {
                                ticket: crate::variant::SIMULATION_TICKET.to_owned(),
                            },
                        },
                    }),
                    None,
                ))
            }
            crate::variant::DISCARD_ID => {
                let set = Self::variant_set_target(&target)?;
                let branch = crate::variant::lower_discard(&named.args)?;
                Ok((
                    vec![command("branch-discard")],
                    PreparedMutation::Variant(crate::variant::VariantIntent::Discard {
                        variant_set: set,
                        branch,
                    }),
                    None,
                ))
            }
            _ => Err(GatewayError::Internal {
                reason: format!("unhandled variant capability {id}"),
            }
            .into()),
        }
    }

    /// synvelis#517 render lowering — schema-validates args, binds the
    /// (empty) target contract, and produces a
    /// [`PreparedMutation::Render`]. Render intents carry no project
    /// mutation: the candidate is the unchanged base and the commit
    /// gate dispatches through the host's render port.
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn lower_render(
        named: &NamedOperation,
        spec: &VariantOpSpec,
        path: &str,
    ) -> Result<Lowered, ErrorEnvelope> {
        spec.input
            .validate(&format!("{path}.args"), &named.args)
            .map_err(ErrorEnvelope::from)?;
        let target = crate::gateway::Gateway::bind_target(&spec.descriptor, named.target.as_ref())
            .map_err(ErrorEnvelope::from)?;
        let action = crate::variant::RenderAction::from_id(named.capability.id.as_str())
            .ok_or_else(|| GatewayError::Internal {
                reason: format!("unhandled render capability {}", named.capability.id),
            })?;
        Ok((
            vec![SemanticCommand {
                capability: named.capability.clone(),
                op: action.op_name(),
                target,
                args: named.args.clone(),
            }],
            PreparedMutation::Render(crate::variant::RenderIntent {
                action,
                args: named.args.clone(),
            }),
            None,
        ))
    }

    /// The AUTX-04 GW-04 bindings seam — lowers a
    /// `synvelis.operation.batch` whose non-empty `bindings` declare
    /// create-then-reference output bindings (the
    /// `docs/automation/design-atomic-macro.md` §3 target form).
    ///
    /// Members are processed in order on a private scratch clone of
    /// the pinned base, seeded at `base.id_watermark` — the same
    /// deterministic id series `ProjectOp::Composite::validate` /
    /// `apply` reproduces at commit. Per member: resolved
    /// placeholders in its wire JSON are rewritten to real tagged
    /// ids, the member lowers through the ordinary machinery, then
    /// validate+apply on the scratch yields the member's created
    /// resources (before/after [`prepare::resource_map`] diff). A
    /// binding resolves iff its producer created *exactly one*
    /// resource of the declared kind — zero or several is a typed
    /// rejection; ids are never guessed.
    ///
    /// Member domain failures are deliberately *not* raised here:
    /// the scratch is disposable and id resolution only needs the
    /// deterministic mint order, so lowering continues where it can
    /// and the emitted `Composite` lets `op.validate` reproduce the
    /// same failure at the correct pipeline stage — after the
    /// provisional authorization gate (SEC-03 ordering preserved).
    /// Only request-shape failures (malformed declarations,
    /// undeclared/premature/unresolvable placeholders) reject during
    /// lowering, matching where schema violations already surface.
    ///
    /// Returns `(commands, Composite, Some(scratch))` — the scratch
    /// post-state is the deterministic candidate the provisional
    /// gate resolves transaction-created resources against. It equals
    /// the base plus every member whose validate+apply succeeded, in
    /// member order: each member trial-applies on a clone, and a
    /// failed member (which dooms the composite — `Composite::validate`
    /// aborts on the same error) is skipped while later independent
    /// producers still resolve their bindings.
    #[allow(clippy::result_large_err, clippy::too_many_lines)]
    fn lower_bound_batch(
        &self,
        path: &str,
        req: &OperationRequest,
        base: &Project,
        members: &[Value],
        bindings: &[Value],
    ) -> Result<Lowered, ErrorEnvelope> {
        // Strict declaration parse — `{name, member, kind}` objects
        // only, mirroring `MacroBindingWire`'s wire contract.
        let mut decls = Vec::with_capacity(bindings.len());
        let mut declared = BTreeSet::new();
        for (j, binding) in bindings.iter().enumerate() {
            let bpath = format!("{path}.args.bindings[{j}]");
            let obj = binding
                .as_object()
                .ok_or_else(|| prepare::violation(&bpath, "expected an object"))?;
            for key in obj.keys() {
                if !matches!(key.as_str(), "name" | "member" | "kind") {
                    return Err(prepare::violation(
                        &format!("{bpath}.{key}"),
                        "unknown field; only name, member, kind are allowed",
                    ));
                }
            }
            let name = obj
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| prepare::violation(&format!("{bpath}.name"), "expected a string"))?
                .to_owned();
            let member = obj
                .get("member")
                .and_then(Value::as_u64)
                .and_then(|m| usize::try_from(m).ok())
                .ok_or_else(|| {
                    prepare::violation(&format!("{bpath}.member"), "expected a member index")
                })?;
            let kind_name = obj
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| prepare::violation(&format!("{bpath}.kind"), "expected a string"))?;
            let kind = ResourceKind::parse(kind_name).map_err(ErrorEnvelope::from)?;
            if member >= members.len() {
                return Err(prepare::violation(
                    &format!("{bpath}.member"),
                    format!(
                        "binding member {member} is outside ops[] ({} members)",
                        members.len()
                    ),
                ));
            }
            if !declared.insert((member, kind)) {
                return Err(prepare::violation(
                    &bpath,
                    format!("duplicate binding for member {member} kind {kind:?}"),
                ));
            }
            decls.push(BindingDecl { name, member, kind });
        }

        // Producer member → its binding slots, for the post-apply
        // created-resource diff.
        let mut by_member: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (j, decl) in decls.iter().enumerate() {
            by_member.entry(decl.member).or_default().push(j);
        }

        let mut scratch = (*base).clone();
        let mut ids = CounterIdGenerator::new(base.id_watermark);
        let mut resolved: Vec<Option<ResourceRef>> = vec![None; decls.len()];

        let mut commands = Vec::with_capacity(members.len());
        let mut ops = Vec::with_capacity(members.len());
        for (i, member) in members.iter().enumerate() {
            let mpath = format!("{path}.args.ops[{i}]");
            let rewritten = rewrite_member(member, i, &decls, &resolved, &mpath)?;
            let member_named = NamedOperation::parse(&rewritten, &mpath)?;
            if member_named.capability.id.as_str() == "synvelis.operation.batch" {
                return Err(prepare::violation(&mpath, "nested batch is not allowed"));
            }
            let (member_commands, member_mutation, _) =
                self.lower_named(&member_named, &mpath, req, base)?;
            let PreparedMutation::Project(member_op) = member_mutation else {
                return Err(prepare::violation(
                    &mpath,
                    "non-project operations cannot be batch members",
                ));
            };
            commands.extend(member_commands);

            // Trial-apply on a clone: the scratch always equals the
            // base plus every member that applied successfully, in
            // member order — the same validate-then-apply sequence
            // `Composite::apply` commits — so the created-resource
            // diff below and the provisional oracle see a consistent
            // state. A member whose validate/apply fails dooms the
            // composite (the same error resurfaces at `op.validate`
            // after the provisional gate); it is skipped — scratch
            // and the id sequence stay where the last success left
            // them — while later members still process.
            let before = by_member.get(&i).map(|_| prepare::resource_map(&scratch));
            if member_op.validate(&scratch).is_ok() {
                let mut trial = scratch.clone();
                let mut trial_ids = ids.clone();
                if member_op.apply(&mut trial, &mut trial_ids).is_ok() {
                    scratch = trial;
                    ids = trial_ids;
                    if let (Some(produced), Some(before)) = (by_member.get(&i), before) {
                        let after = prepare::resource_map(&scratch);
                        for &j in produced {
                            let created: Vec<ResourceRef> = after
                                .keys()
                                .filter(|r| !before.contains_key(*r) && r.kind() == decls[j].kind)
                                .copied()
                                .collect();
                            let [resource] = created.as_slice() else {
                                return Err(GatewayError::PreparationRejected {
                                    reason: format!(
                                        "output binding '{}' (member {i}) created {} resource(s) \
                                         of its declared kind — exactly one is required; ids are \
                                         never guessed",
                                        decls[j].name,
                                        created.len()
                                    ),
                                }
                                .into());
                            };
                            resolved[j] = Some(*resource);
                        }
                    }
                }
            }
            ops.push(member_op);
        }
        Ok((
            commands,
            PreparedMutation::Project(ProjectOp::Composite { ops }),
            Some(Arc::new(scratch)),
        ))
    }

    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn store_record(
        &self,
        request: OperationRequest,
        mutation: PreparedMutation,
        prepared: PreparedOperation,
    ) -> Result<
        (
            ProposalRef,
            Arc<PreparedRecord>,
            crate::proposal::PreparedChangeSet,
        ),
        GatewayError,
    > {
        self.proposals.insert(request, mutation, prepared)
    }

    fn is_external(req: &OperationRequest) -> bool {
        matches!(req.caller().principal().kind(), PrincipalKind::Agent)
            || matches!(
                req.caller().origin(),
                crate::request::OriginKind::ExternalAdapter
            )
    }

    #[allow(clippy::too_many_lines, clippy::result_large_err)] // typed envelope is the contract
    fn run(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let pin = self.pin_base(req)?;
        let operation_value = req
            .args()
            .get("operation")
            .ok_or_else(|| prepare::violation("args.operation", "missing required field"))?;
        let named = NamedOperation::parse(operation_value, "args.operation")?;
        let (commands, mutation, scratch) =
            self.lower_named(&named, "args.operation", req, &pin.state)?;

        // Provisional gate + candidate — a `ProjectOp` validates and
        // applies on a scratch clone; a CB-10 variant intent simulates
        // on a scratch controller through the wired ports, so the
        // retained candidate and derived effects are the *real*
        // post-state either way (never a declared approximation).
        let (facts, capabilities, candidate) = match &mutation {
            PreparedMutation::Project(op) => {
                let (facts, capabilities) = self.provisional_gate(
                    req,
                    &pin,
                    &named,
                    prepare::op_facts(op, &pin.state),
                    &commands,
                    scratch.as_ref(),
                )?;

                // The project's own validator is the semantic authority —
                // a rejection here is a typed prepare rejection, never a
                // synthesized verdict.
                op.validate(&pin.state).map_err(|e| map_project_error(&e))?;

                // Candidate: apply on a private clone seeded at the base
                // watermark — the same ids the controller would draw at this
                // base, so the candidate previews the real post-state.
                let mut candidate = (*pin.state).clone();
                let mut ids = CounterIdGenerator::new(pin.state.id_watermark);
                op.apply(&mut candidate, &mut ids)
                    .map_err(|e| map_project_error(&e))?;
                candidate
                    .validate_references()
                    .map_err(|e| map_project_error(&e))?;
                (facts, capabilities, Arc::new(candidate))
            }
            PreparedMutation::Variant(intent) => {
                let (facts, capabilities) = self.provisional_gate(
                    req,
                    &pin,
                    &named,
                    crate::variant::declared_facts(intent, &pin.state),
                    &commands,
                    None,
                )?;
                let Some(ports) = &self.variant else {
                    // Host misconfiguration, not an authorization verdict.
                    return Err(GatewayError::Internal {
                        reason: "variant ports are not wired".to_owned(),
                    }
                    .into());
                };
                let post = crate::variant::simulate(&pin.state, ports, intent).map_err(|e| {
                    GatewayError::PreparationRejected {
                        reason: format!(
                            "variant simulation failed: {}",
                            crate::variant::variant_reason(&e)
                        ),
                    }
                })?;
                (facts, capabilities, Arc::new(post))
            }
            PreparedMutation::Render(intent) => {
                // Render commands mutate no project state — the
                // candidate is the pinned base verbatim and the real
                // effect is the commit-time dispatch outcome.
                let (facts, capabilities) = self.provisional_gate(
                    req,
                    &pin,
                    &named,
                    crate::variant::render_declared_facts(intent, &pin.state),
                    &commands,
                    None,
                )?;
                (facts, capabilities, Arc::new((*pin.state).clone()))
            }
        };

        let before_map = prepare::resource_map(&pin.state);
        let after_map = prepare::resource_map(&candidate);
        let mut effects = prepare::derive_effects(&before_map, &after_map, &facts.reads);
        // A write landing on a shared source names every linked clip
        // in its references — the explicit "this edit propagates to
        // all readers" edge (linked-S1 semantics, GTC-005).
        for effect in &mut effects {
            if effect.kind == EffectKind::Write
                && let Some(clips) = facts.impacted.get(&effect.resource)
            {
                let mut refs: BTreeSet<ResourceRef> = effect.references.iter().copied().collect();
                refs.extend(clips.iter().copied());
                effect.references = refs.into_iter().collect();
            }
        }
        let prepared_fx =
            prepare::prepared_effects(&effects, capabilities, facts.unsupported.clone());

        // GW-02 effect-aware authorization over the *derived* set —
        // pinned-base facts for existing resources (never a live
        // view), candidate facts only for transaction-created ids.
        let facts_oracle: Arc<dyn ResourceOracle> =
            Arc::new(prepare::BaseFirstOracle::new(&pin.state, &candidate));
        let decision =
            self.authorizer
                .authorize_effects(req, &self.descriptor, &prepared_fx, facts_oracle);
        verdict(decision)?;

        // Emission boundary — the result's real classes, plus
        // ExternalDisclosure for external callers.
        let emitted = prepare::emitted_classes(&effects);
        let decision = self.authorizer.authorize_emission(
            req,
            &self.descriptor,
            &emitted,
            Self::is_external(req),
        );
        verdict(decision)?;

        let prepared = PreparedOperation {
            operation: req.operation_id(),
            base: prepare::PreparationBasis {
                project: req.project(),
                revision: pin.revision,
                snapshot: pin.reference.clone(),
                tempo_map_revision: pin.state.tempo_map.revision,
            },
            commands,
            preconditions: facts.preconditions,
            effects,
            unsupported: facts.unsupported,
            candidate,
        };
        let (proposal, stored, changeset) = self.store_record(req.clone(), mutation, prepared)?;
        let mut result = prepare::result_json(&proposal, &stored.prepared);
        result["digest"] = Value::from(changeset.digest.as_str());
        result["expires_at"] = Value::from(changeset.expires_at.0);
        Ok(GatewayOutcome::Completed { result })
    }

    /// The pre-domain-validation gate: create-parent identity,
    /// semantic facts, the fail-closed completeness verdict, and the
    /// provisional scope gate — every check that must run *before*
    /// `op.validate` so an absent resource and an out-of-scope
    /// resource deny identically (SEC-03 existence blinding) and so
    /// an unverifiable effect set fails for every caller (SEC-14).
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn provisional_gate(
        &self,
        req: &OperationRequest,
        pin: &PinnedSnapshot,
        named: &NamedOperation,
        facts: prepare::OpFacts,
        commands: &[SemanticCommand],
        scratch: Option<&Arc<Project>>,
    ) -> Result<(prepare::OpFacts, Vec<CapabilityRef>), ErrorEnvelope> {
        // A project-level create's declared parent is semantic, not
        // decorative — `bind_target` validates the shape, and the
        // parent must name the request's own project (a prepare is
        // pinned to exactly one project).
        for c in commands {
            if let OperationTarget::Create {
                parent: ResourceRef::Project(id),
                ..
            } = &c.target
                && *id != req.project()
            {
                return Err(GatewayError::PreparationRejected {
                    reason: format!("create parent {id} is not the request project"),
                }
                .into());
            }
        }

        // An unverifiable effect set fails preparation for *every*
        // caller — it is a completeness verdict on the derivation,
        // not a permission verdict (SEC-14, and the human-caller
        // early-permit in GW-02 must not route around it).
        if !facts.unsupported.is_empty() {
            return Err(GatewayError::PreparationRejected {
                reason: format!("unverifiable effects: {}", facts.unsupported.join("; ")),
            }
            .into());
        }

        // The proposal's capability floor covers the *executed*
        // operation as well as its normalized members — a batch ref
        // must be granted even when every member is.
        let mut capabilities: Vec<CapabilityRef> =
            commands.iter().map(|c| c.capability.clone()).collect();
        if !capabilities.contains(&named.capability) {
            capabilities.push(named.capability.clone());
        }

        // Provisional scope gate over the read/impact set — runs
        // *before* domain validation so an absent resource and an
        // out-of-scope resource deny with the same verdict. The full
        // derived set (writes/creates/deletes) is authorized again
        // after the candidate exists.
        let provisional = prepare::PreparedEffects {
            scoped: crate::grant::ScopedEffects {
                reads: facts.reads.iter().copied().collect(),
                writes: facts.impacted.keys().copied().collect(),
                ..crate::grant::ScopedEffects::default()
            },
            after_states: Vec::new(),
            capabilities: capabilities.clone(),
        };
        // A `bindings` batch hands over the seam's deterministic
        // scratch post-state — identical to the candidate the
        // derived-effects gate below authorizes against — so
        // create-then-reference reads resolve as transaction-created
        // resources instead of failing closed as unresolved. Every
        // other operation keeps the pinned-base-only oracle.
        let oracle: Arc<dyn ResourceOracle> = match scratch {
            Some(candidate) => Arc::new(prepare::BaseFirstOracle::new(&pin.state, candidate)),
            None => Arc::new(crate::snapshot::ProjectOracle::new(pin.state.clone())),
        };
        let decision =
            self.authorizer
                .authorize_effects(req, &self.descriptor, &provisional, oracle);
        verdict(decision)?;

        Ok((facts, capabilities))
    }
}

/// Maps a policy decision to a verdict — `PermitForStage` proceeds;
/// every other shape denies at `PrepareProposal`.
#[allow(clippy::result_large_err)] // typed envelope is the contract
fn verdict(decision: AuthorizationDecision) -> Result<(), ErrorEnvelope> {
    match decision {
        AuthorizationDecision::PermitForStage => Ok(()),
        AuthorizationDecision::Deny { reason } => Err(GatewayError::Denied {
            stage: crate::authorization::AuthorizationStage::PrepareProposal,
            reason: Some(reason),
        }
        .into()),
        AuthorizationDecision::ApprovalRequired => Err(GatewayError::Denied {
            stage: crate::authorization::AuthorizationStage::PrepareProposal,
            reason: Some("approval is not issuable at preparation".to_owned()),
        }
        .into()),
    }
}

/// `synvelis_project::Error` → the gateway taxonomy: absent targets,
/// illegal transitions and invariant violations are prepare-time
/// semantic rejections; arithmetic/representation failures are too.
fn map_project_error(e: &synvelis_project::Error) -> ErrorEnvelope {
    match e {
        synvelis_project::Error::StaleRevision { expected, actual } => {
            GatewayError::RevisionConflict {
                expected: *expected,
                actual: *actual,
            }
        }
        _ => GatewayError::PreparationRejected {
            reason: e.to_string(),
        },
    }
    .into()
}

impl CapabilityOperation for PrepareEngine {
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        self.run(req)
    }
}

// ---------------------------------------------------------------------------
// AUTX-04 in-file tests — batch `bindings` contract (synvelis#263).
// ---------------------------------------------------------------------------
//
// The create-then-reference fallback lives entirely inside
// `lower_named`'s batch arm: `bindings` must pass the input schema
// (so the semantic layer — not a generic unknown-field violation —
// answers), then any non-empty declaration fails closed with
// `PreparationRejected` before member lowering. These tests wire the
// real engine through `Gateway::handle` on a minimal pinned project —
// the reject fires before authorization, effects, or the candidate
// apply, so no grant/proposal surface is needed.

#[cfg(test)]
mod tests {
    use serde_json::json;
    use synvelis_project::{LaneId, Project, ProjectId, ProjectRevision, TrackId};

    use super::*;
    use crate::clock::{ManualClock, PolicyInstant};
    use crate::context::{ContextBudget, NoSelectionSource};
    use crate::fixture::{FixtureAdapter, StaticOracle};
    use crate::proposal_policy::ProposalGate;
    use crate::request::CallerIdentity;
    use crate::snapshot::MemorySnapshotSource;
    use crate::store::InMemoryPolicyStore;
    use crate::{GrantPolicy, PreparePolicy};

    /// A `Gateway` with the real `PrepareEngine` installed over one
    /// pinned (empty) project at revision 1.
    struct Rig {
        gw: crate::Gateway,
        engine: Arc<PrepareEngine>,
    }

    fn rig() -> Rig {
        let store = Arc::new(InMemoryPolicyStore::new());
        let clock = Arc::new(ManualClock::new(PolicyInstant::new(1_000)));
        let source = Arc::new(MemorySnapshotSource::new());
        let project_id = ProjectId::new(7);
        let mut project = Project::new(project_id, "demo");
        project.id_watermark = 1_000;
        source.commit(project_id, ProjectRevision(1), project);
        let oracle: Arc<dyn ResourceOracle> = Arc::new(StaticOracle::new());
        let ctx = Arc::new(ContextProjectionEngine::new(
            source,
            Arc::new(NoSelectionSource),
            Arc::new(GrantPolicy::new(
                store.clone(),
                clock.clone(),
                oracle.clone(),
            )),
            clock.clone(),
            ContextBudget::default(),
        ));
        let proposals = Arc::new(ProposalStore::new(clock.clone(), 60_000, 30_000, 8));
        let engine = Arc::new(PrepareEngine::new(
            ctx,
            Arc::new(PreparePolicy::new(store.clone(), clock.clone())),
            PrepareBudget::default(),
            proposals.clone(),
        ));
        let mut gw = crate::Gateway::with_evaluator(Box::new(ProposalGate::new(
            proposals,
            Box::new(GrantPolicy::new(store, clock, oracle)),
        )));
        engine.install(&mut gw).unwrap();
        Rig { gw, engine }
    }

    fn caller() -> CallerIdentity {
        FixtureAdapter::for_testing().human("user-1").unwrap()
    }

    fn prepare_req(operation: &Value) -> Value {
        json!({
            "request_id": "req-autx04-bindings",
            "capability": {
                "id": "synvelis.operation.prepare",
                "version": {"major": 1, "minor": 0},
            },
            "schema": {
                "name": "synvelis.operation.prepare.input",
                "version": {"major": 1, "minor": 0},
            },
            "project": "project-00000000000000000000000000000007",
            "args": {"operation": operation},
            "consistency": {"kind": "expected-revision", "revision": 1},
        })
    }

    fn batch_op(args: &Value) -> Value {
        json!({
            "capability": {
                "id": "synvelis.operation.batch",
                "version": {"major": 1, "minor": 0},
            },
            "args": args,
        })
    }

    /// One schema-valid member triple — never reached by the
    /// bindings tests (the reject precedes member lowering).
    fn member() -> Value {
        json!({
            "capability": {
                "id": "synvelis.clip.mute",
                "version": {"major": 1, "minor": 0},
            },
            "args": {},
        })
    }

    /// The pinned demo project's tagged id (`ProjectId::new(7)`).
    fn project_tagged() -> String {
        ProjectId::new(7).to_tagged()
    }

    /// A `synvelis.track.create` member — creates exactly one track.
    fn track_create_member() -> Value {
        json!({
            "capability": {
                "id": "synvelis.track.create",
                "version": {"major": 1, "minor": 0},
            },
            "target": {
                "shape": "create",
                "parent_kind": "project",
                "parent": project_tagged(),
                "creates": "track",
            },
            "args": {"name": "t-new"},
        })
    }

    /// A `synvelis.lane.create` member whose create-parent is the
    /// tagged id `parent_id` (real or compiler placeholder).
    fn lane_create_member(parent_id: &str) -> Value {
        json!({
            "capability": {
                "id": "synvelis.lane.create",
                "version": {"major": 1, "minor": 0},
            },
            "target": {
                "shape": "create",
                "parent_kind": "track",
                "parent": parent_id,
                "creates": "lane",
            },
            "args": {},
        })
    }

    /// The compiler placeholder for binding `slot` of `kind` —
    /// `<kind>-<32hex>` over `OUTPUT_BINDING_ID_BASE + slot`.
    fn placeholder(kind: &str, slot: usize) -> String {
        format!("{kind}-{:032x}", OUTPUT_BINDING_ID_BASE + slot as u128)
    }

    #[test]
    fn batch_bindings_resolve_create_then_reference() {
        // AUTX-04 GW-04: `track.create` → `lane.create` with the
        // lane's create-parent carried as the slot-0 placeholder.
        // The seam rewrites it to the track's real minted id
        // (watermark 1000 → raw 1000; the lane draws 1001) and the
        // composite prepares, previews, and commits deterministically.
        let mut rig = rig();
        let outcome = rig
            .gw
            .handle(
                &prepare_req(&batch_op(&json!({
                    "ops": [
                        track_create_member(),
                        lane_create_member(&placeholder("track", 0)),
                    ],
                    "bindings": [
                        {"name": "new_track", "member": 0, "kind": "track"}
                    ],
                }))),
                caller(),
            )
            .expect("bindings batch must prepare");
        let GatewayOutcome::Completed { result } = outcome else {
            panic!("expected Completed, got {outcome:?}");
        };
        let proposal = ProposalRef::new(result["proposal"].as_str().unwrap()).unwrap();
        let record = rig.engine.prepared(&proposal).expect("proposal retained");
        // The candidate shows the resolved ids — track raw 1000,
        // lane raw 1001 parented to it.
        let track_id = TrackId::new(1000);
        let lane_id = LaneId::new(1001);
        assert!(record.prepared.candidate.tracks.contains_key(&track_id));
        assert_eq!(record.prepared.candidate.lanes[&lane_id].track_id, track_id);
        // Commands carry the rewritten ids — no placeholder survives
        // into the digest/approval surface.
        assert_eq!(record.prepared.commands.len(), 2);
        assert!(matches!(
            record.project_op(),
            Some(ProjectOp::Composite { ops }) if ops.len() == 2
        ));
        // Create effects name the concrete resources.
        assert!(
            record
                .prepared
                .effects
                .iter()
                .any(|e| e.kind == EffectKind::Create
                    && e.resource == ResourceRef::Track(track_id))
        );
        assert!(
            record
                .prepared
                .effects
                .iter()
                .any(|e| e.kind == EffectKind::Create && e.resource == ResourceRef::Lane(lane_id))
        );
    }

    #[test]
    fn batch_bindings_reject_producer_wrong_kind_arity() {
        // The producer creates a track but the binding demands a
        // lane — zero matching resources is a typed rejection, ids
        // are never guessed.
        let mut rig = rig();
        let err = rig
            .gw
            .handle(
                &prepare_req(&batch_op(&json!({
                    "ops": [track_create_member()],
                    "bindings": [
                        {"name": "wrong", "member": 0, "kind": "lane"}
                    ],
                }))),
                caller(),
            )
            .expect_err("a producer creating zero of the kind must reject");
        assert!(matches!(err.code, GatewayError::PreparationRejected { .. }));
    }

    #[test]
    fn batch_bindings_reject_malformed_declarations() {
        // Every malformed declaration shape fails closed with the
        // existing typed vocabulary — non-object items, unknown
        // fields, out-of-range member indexes, duplicate
        // (member, kind) pairs, and unknown kind tags.
        let cases: Vec<(&str, Value)> = vec![
            ("non-object", json!("yes")),
            (
                "unknown field",
                json!({"name": "x", "member": 0, "kind": "track", "extra": 1}),
            ),
            (
                "member out of range",
                json!({"name": "x", "member": 3, "kind": "track"}),
            ),
            (
                "unknown kind",
                json!({"name": "x", "member": 0, "kind": "bogus"}),
            ),
        ];
        for (label, binding) in cases {
            let mut rig = rig();
            let Err(err) = rig.gw.handle(
                &prepare_req(&batch_op(&json!({
                    "ops": [track_create_member()],
                    "bindings": [binding],
                }))),
                caller(),
            ) else {
                panic!("{label} must reject")
            };
            assert!(
                matches!(
                    err.code,
                    GatewayError::SchemaViolation { .. }
                        | GatewayError::UnknownResourceKind { .. }
                        | GatewayError::PreparationRejected { .. }
                ),
                "{label}: unexpected error {:?}",
                err.code
            );
        }
        // Duplicate (member, kind) declarations reject.
        let mut rig = rig();
        let err = rig
            .gw
            .handle(
                &prepare_req(&batch_op(&json!({
                    "ops": [track_create_member()],
                    "bindings": [
                        {"name": "a", "member": 0, "kind": "track"},
                        {"name": "b", "member": 0, "kind": "track"},
                    ],
                }))),
                caller(),
            )
            .expect_err("duplicate (member, kind) must reject");
        assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
    }

    #[test]
    fn batch_bindings_reject_premature_and_undeclared_placeholders() {
        // A placeholder whose producer member has not applied yet
        // (self/backward reference) and a placeholder naming a slot
        // no binding declares are both typed rejections.
        let err = {
            let mut rig = rig();
            rig.gw
                .handle(
                    &prepare_req(&batch_op(&json!({
                        "ops": [
                            lane_create_member(&placeholder("track", 0)),
                            track_create_member(),
                        ],
                        "bindings": [
                            {"name": "t", "member": 1, "kind": "track"}
                        ],
                    }))),
                    caller(),
                )
                .expect_err("use-before-produce must reject")
        };
        assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));

        let err = {
            let mut rig = rig();
            rig.gw
                .handle(
                    &prepare_req(&batch_op(&json!({
                        "ops": [
                            track_create_member(),
                            lane_create_member(&placeholder("track", 7)),
                        ],
                        "bindings": [
                            {"name": "t", "member": 0, "kind": "track"}
                        ],
                    }))),
                    caller(),
                )
                .expect_err("undeclared slot must reject")
        };
        assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
    }

    #[test]
    fn batch_without_bindings_still_lowers() {
        // `bindings: []` carries no create-then-reference — the batch
        // proceeds past the bindings gate into member lowering, where
        // an unknown member capability produces its own typed error.
        let mut rig = rig();
        let err = rig
            .gw
            .handle(
                &prepare_req(&batch_op(&json!({
                    "ops": [{
                        "capability": {
                            "id": "synvelis.no.such",
                            "version": {"major": 1, "minor": 0},
                        },
                        "args": {},
                    }],
                    "bindings": [],
                }))),
                caller(),
            )
            .expect_err("unknown member capability must fail");
        assert!(matches!(err.code, GatewayError::UnknownCapability { .. }));
    }

    #[test]
    fn batch_bindings_shape_is_schema_enforced() {
        // A non-array `bindings` never reaches the semantic check —
        // the registered input schema rejects it first.
        let mut rig = rig();
        let err = rig
            .gw
            .handle(
                &prepare_req(&batch_op(&json!({
                    "ops": [member()],
                    "bindings": "yes",
                }))),
                caller(),
            )
            .expect_err("non-array bindings must fail schema validation");
        assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
    }
}
