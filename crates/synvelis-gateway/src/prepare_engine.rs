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

use serde_json::Value;
use synvelis_project::{CounterIdGenerator, Operation, ProjectOp};

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
    self, NamedOperation, OpSpec, PrepareAuthorizer, PreparedOperation, PreparedRecord,
    SemanticCommand,
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
    proposals: Arc<ProposalStore>,
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
        mut budget: PrepareBudget,
        proposals: Arc<ProposalStore>,
    ) -> Self {
        budget.max_ops = budget.max_ops.max(1);
        budget.max_proposals = budget.max_proposals.max(1);
        Self {
            context,
            authorizer,
            budget,
            descriptor: Self::descriptor(),
            ops: Self::op_specs(),
            proposals,
        }
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
                    &[(
                        "ops",
                        ValueSchema::Array {
                            item: Box::new(any_obj()),
                            max_len: Some(64),
                        },
                    )],
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

    /// Lowers one named operation to `(commands, ProjectOp)` —
    /// recursing through the batch capability so member commands are
    /// normalized individually.
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn lower_named(
        &self,
        named: &NamedOperation,
        path: &str,
    ) -> Result<(Vec<SemanticCommand>, ProjectOp), ErrorEnvelope> {
        let Some(spec) = self.ops.get(&named.capability) else {
            let supported: Vec<CapabilityVersion> = self
                .ops
                .keys()
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
                let (member_commands, op) =
                    self.lower_named(&member_named, &format!("{path}.args.ops[{i}]"))?;
                commands.extend(member_commands);
                ops.push(op);
            }
            return Ok((commands, ProjectOp::Composite { ops }));
        }
        // An ordinary op: bind the declared target, lower
        // syntactically (args were schema-validated above).
        let target = crate::gateway::Gateway::bind_target(&spec.descriptor, named.target.as_ref())
            .map_err(ErrorEnvelope::from)?;
        let op = (spec.lower)(&target, &named.args).map_err(ErrorEnvelope::from)?;
        Ok((
            vec![SemanticCommand {
                capability: named.capability.clone(),
                op: prepare::op_name(&op),
                target,
                args: named.args.clone(),
            }],
            op,
        ))
    }

    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn store_record(
        &self,
        request: OperationRequest,
        op: ProjectOp,
        prepared: PreparedOperation,
    ) -> Result<
        (
            ProposalRef,
            Arc<PreparedRecord>,
            crate::proposal::PreparedChangeSet,
        ),
        GatewayError,
    > {
        self.proposals.insert(request, op, prepared)
    }

    fn is_external(req: &OperationRequest) -> bool {
        matches!(req.caller().principal().kind(), PrincipalKind::Agent)
            || matches!(
                req.caller().origin(),
                crate::request::OriginKind::ExternalAdapter
            )
    }

    #[allow(clippy::result_large_err)] // typed envelope is the contract
    fn run(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let pin = self.pin_base(req)?;
        let operation_value = req
            .args()
            .get("operation")
            .ok_or_else(|| prepare::violation("args.operation", "missing required field"))?;
        let named = NamedOperation::parse(operation_value, "args.operation")?;
        let (commands, op) = self.lower_named(&named, "args.operation")?;
        let (facts, capabilities) = self.provisional_gate(req, &pin, &named, &op, &commands)?;

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
        let candidate = Arc::new(candidate);
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
        let (proposal, stored, changeset) = self.store_record(req.clone(), op, prepared)?;
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
        op: &ProjectOp,
        commands: &[SemanticCommand],
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

        // Semantic facts are known before domain validation runs —
        // the read/impact sets let the provisional scope gate below
        // deny *out-of-scope-or-absent* resources identically, so a
        // domain `NotFound` can never distinguish "does not exist"
        // from "exists outside the grant" (SEC-03 existence blinding).
        let facts = prepare::op_facts(op, &pin.state);

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
        let base_oracle: Arc<dyn ResourceOracle> =
            Arc::new(crate::snapshot::ProjectOracle::new(pin.state.clone()));
        let decision =
            self.authorizer
                .authorize_effects(req, &self.descriptor, &provisional, base_oracle);
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
