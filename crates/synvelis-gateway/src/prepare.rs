//! GW-04 semantic operation preparation — typed model (GTC-005).
//!
//! A *prepare* request carries a snapshot revision (consistency pin),
//! an explicit target, and a typed semantic operation — and returns a
//! [`PreparedOperation`]: normalized semantic commands, preconditions,
//! evaluated effects, and a non-destructive candidate snapshot. The
//! preparation engine lives in [`crate::prepare_engine`]; this module
//! owns the types and the lowering/effect-derivation rules.
//!
//! # The port around `Operation::validate/apply`
//!
//! `synvelis-project`'s `Operation` contract exposes only
//! `validate(&Project)` and `apply(&mut Project, ids)` — there is no
//! Effect API. GW-04 adds the preparation port around it without
//! duplicating the validator:
//!
//! 1. **Lower** — a bound named operation (`capability`, `target`,
//!    `args`) maps to exactly one [`ProjectOp`] variant. The mapping is
//!    syntactic: ids come from the bound [`OperationTarget`], typed
//!    payloads deserialize into the project's own serde shapes
//!    (`TimeAnchor`, `ClipExtent`, `MidiDivision`, `BeatPoint`, …).
//! 2. **Validate** — `Operation::validate` runs against the pinned
//!    base. Its checks are mirrored into named [`Precondition`]s —
//!    the record of what the base satisfied and what GW-06 must
//!    re-verify at commit time.
//! 3. **Apply on a scratch clone** — `Operation::apply` mutates a
//!    private clone seeded with a [`CounterIdGenerator`] at the base
//!    `id_watermark`, producing the *candidate snapshot*. The pinned
//!    `Arc<Project>` is never touched (the clone is the only mutable
//!    value in the pipeline).
//! 4. **Derive effects** — the before/after resource-attribute diff
//!    (`kind`/`owner`/`range`/`related` per
//!    [`crate::snapshot::attributes`]) classifies every observable
//!    change as Write/Create/Delete; op-declared *internal reads*
//!    name the resources validation consulted. Dimensions outside
//!    the observable model (opaque plugin state, resource kinds the
//!    gateway cannot see) land in `unsupported` — never silently
//!    dropped (SEC-14).
//!
//! # What an effect carries
//!
//! Every [`DerivedEffect`] names the real resource id, its kind,
//! before/after attribute state (placement range in its own
//! [`TimeDomain`] — musical ticks, source-native MIDI ticks, media
//! time — never converted across domains), and structural references
//! (`related`) such as a clip's linked source. Reads distinguish
//! *internal* validation reads from *external* disclosure.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};
use synvelis_project::{
    ClipId, LaneId, MidiTick, MusicalTick, NoteId, Project, ProjectId, ProjectOp, ProjectRevision,
    SourceFrame, SourceId, TrackId,
};

use crate::authorization::AuthorizationDecision;
use crate::capability::{CapabilityRef, DataAccessClass, EffectKind};
use crate::grant::{CreateEffect, ResourceAttributes, ResourceOracle, ScopedEffects};
use crate::id::{OperationId, ProposalRef, SnapshotRef};
use crate::outcome::{ErrorEnvelope, GatewayError};
use crate::request::{
    OperationRequest, OperationTarget, ResourceKind, ResourceRef, TimeDomain, TypedRange,
};
use crate::snapshot;
use crate::wire::WireTarget;

// ── Typed input ──────────────────────────────────────────────

/// An inner semantic operation — the `{capability, target?, args?}`
/// triple carried inside a prepare request's `args.operation` (and by
/// every member of a batch's `args.ops`). The triple is the GTC-005
/// "typed semantic operation": the capability names *what*, the target
/// names *on which resource*, the args carry the typed payload.
#[derive(Clone, Debug)]
pub struct NamedOperation {
    /// The semantic-operation capability (id + exact version).
    pub capability: CapabilityRef,
    /// The wire-shaped target, bound against the capability's
    /// declared [`crate::capability::TargetContract`].
    pub target: Option<WireTarget>,
    /// The operation arguments (validated against the capability's
    /// registered input schema).
    pub args: Value,
}

pub(crate) fn violation(path: &str, reason: impl Into<String>) -> ErrorEnvelope {
    GatewayError::SchemaViolation {
        path: path.to_owned(),
        reason: reason.into(),
    }
    .into()
}

impl NamedOperation {
    /// Parses one `{capability, target?, args?}` object.
    ///
    /// Strict shape: exactly the three keys; `capability` must carry
    /// `id` + `version`; `target`, when present, is a [`WireTarget`]
    /// object (GW-01's explicit-target shape — the implicit forms the
    /// wire rejects never reach here).
    ///
    /// # Errors
    /// [`GatewayError::SchemaViolation`] / [`GatewayError::MalformedRequest`]
    /// on a malformed triple.
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    pub fn parse(value: &Value, path: &str) -> Result<Self, ErrorEnvelope> {
        let obj = value
            .as_object()
            .ok_or_else(|| violation(path, "expected an object"))?;
        for key in obj.keys() {
            if !matches!(key.as_str(), "capability" | "target" | "args") {
                return Err(violation(
                    &format!("{path}.{key}"),
                    "unknown field; only capability, target, args are allowed",
                ));
            }
        }
        let capability: CapabilityRef =
            serde_json::from_value(obj.get("capability").cloned().ok_or_else(|| {
                violation(&format!("{path}.capability"), "missing required field")
            })?)
            .map_err(|e| violation(&format!("{path}.capability"), e.to_string()))?;
        let target = match obj.get("target") {
            Some(v) if !v.is_null() => {
                // serde's deny_unknown_fields is inert on internally
                // tagged enums — scan the shape keys the same way the
                // outer wire layer does (prepare.rs#NamedOperation).
                if let Some(allowed) = crate::wire::allowed_target_keys(
                    v.get("shape").and_then(Value::as_str).unwrap_or(""),
                ) {
                    crate::wire::deny_extra_keys(v, &format!("{path}.target"), allowed)
                        .map_err(ErrorEnvelope::from)?;
                }
                Some(
                    serde_json::from_value::<WireTarget>(v.clone())
                        .map_err(|e| violation(&format!("{path}.target"), e.to_string()))?,
                )
            }
            _ => None,
        };
        let args = match obj.get("args") {
            Some(v) if v.is_object() => v.clone(),
            Some(_) => return Err(violation(&format!("{path}.args"), "expected an object")),
            None => Value::Object(Map::new()),
        };
        Ok(Self {
            capability,
            target,
            args,
        })
    }
}

// ── Normalized product ───────────────────────────────────────

/// One normalized semantic command — the bound form of a
/// [`NamedOperation`]: capability ref, the [`ProjectOp`] variant it
/// lowered to, the bound target, and the schema-validated args.
#[derive(Clone, Debug)]
pub struct SemanticCommand {
    /// The semantic-operation capability.
    pub capability: CapabilityRef,
    /// The lowered `ProjectOp` variant tag (kebab-case).
    pub op: &'static str,
    /// The bound explicit target.
    pub target: OperationTarget,
    /// The schema-validated args.
    pub args: Value,
}

/// A named legality condition the pinned base satisfied.
///
/// Mirrors one check inside `Operation::validate` — not a second
/// validator, but the *record* of what the pinned state proved, so the
/// commit path (GW-06) knows exactly which conditions to re-verify
/// against the live revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Precondition {
    /// Machine-stable condition code.
    pub code: &'static str,
    /// The resource the condition is about, when it names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceRef>,
    /// Human-readable detail (internal surface only).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// Whether an effect's observation stays inside trusted validation or
/// crosses an external boundary — the GTC-005 internal/external
/// disclosure distinction on reads.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectVisibility {
    /// Consulted by validation/apply internally — never disclosed.
    Internal,
    /// Disclosed to a caller — evaluated against `data_access`.
    External,
}

/// One derived effect — the GTC-005 effect record.
///
/// `before`/`after` carry the resource's attribute state across the
/// candidate application (placement range in its own time domain,
/// owner, references) — `None` on the side where the resource is
/// absent (create/delete). `references` names structurally related
/// resources (e.g. a clip's linked source) so consumers see the
/// sharing edge the effect rides on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedEffect {
    /// The effect kind.
    pub kind: EffectKind,
    /// The real resource the effect lands on.
    pub resource: ResourceRef,
    /// Internal or external visibility (reads only).
    pub visibility: EffectVisibility,
    /// Attribute state in the pinned base, when the resource existed.
    pub before: Option<ResourceAttributes>,
    /// Attribute state in the candidate, when the resource survives.
    pub after: Option<ResourceAttributes>,
    /// The entity's serialized state in the pinned base — the
    /// detail-level `Before` (muted, velocity, source state… every
    /// field `ResourceAttributes` does not model).
    pub before_detail: Option<Value>,
    /// The entity's serialized state in the candidate.
    pub after_detail: Option<Value>,
    /// Structural references (the after-state's `related` set, or the
    /// before-state's for deletes) plus every linked clip for a
    /// shared-source write.
    pub references: Vec<ResourceRef>,
    /// The data-access class a `Disclose` effect emits (`None` for
    /// every other kind — the initial op set derives no disclosure
    /// effects; internal reads are never disclosures).
    pub data_class: Option<DataAccessClass>,
}

/// The proposal's base pins — the dependency snapshot/version data
/// GW-06 revalidates before commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparationBasis {
    /// The project.
    pub project: ProjectId,
    /// The committed revision the candidate was derived from.
    pub revision: ProjectRevision,
    /// The pinned-snapshot reference serving the base.
    pub snapshot: SnapshotRef,
    /// The tempo map revision the base carries — musical-time
    /// evaluation is meaningless if the map moved under the proposal.
    pub tempo_map_revision: u64,
}

/// The full non-destructive preparation product (GTC-005
/// `PreparedOperation`).
#[derive(Clone)]
pub struct PreparedOperation {
    /// The gateway operation id this prepare ran under.
    pub operation: OperationId,
    /// The pinned base.
    pub base: PreparationBasis,
    /// Normalized semantic commands (one per lowered member).
    pub commands: Vec<SemanticCommand>,
    /// Preconditions the pinned base satisfied.
    pub preconditions: Vec<Precondition>,
    /// Effects derived from the actual candidate application.
    pub effects: Vec<DerivedEffect>,
    /// Effect dimensions outside the observable model — non-empty
    /// fails authorization closed (SEC-14).
    pub unsupported: Vec<String>,
    /// The candidate snapshot — the would-be project state, never
    /// committed and never aliased to live state.
    pub candidate: Arc<Project>,
}

/// The authorization input: GW-02's flat [`ScopedEffects`] plus the
/// records needed for after-state coverage (SEC-06) and the inner
/// capability floor (SEC-13).
#[derive(Clone, Debug)]
pub struct PreparedEffects {
    /// The flat scope/create/disclosure view.
    pub scoped: ScopedEffects,
    /// `(resource, after-state)` pairs for writes whose attributes
    /// changed — the destination must satisfy the same coverage as
    /// the origin (a move may not escape the granted bound).
    pub after_states: Vec<(ResourceRef, ResourceAttributes)>,
    /// The semantic-operation capabilities the proposal executes —
    /// each must be covered by the grant.
    pub capabilities: Vec<CapabilityRef>,
}

/// The retained proposal — what GW-06's commit path consumes.
#[derive(Clone)]
pub struct PreparedRecord {
    /// The proposal reference handed to the caller.
    pub proposal: ProposalRef,
    /// The bound request (caller, task, grant refs — commit-time
    /// revalidation re-runs the binding chain on this identity).
    pub request: OperationRequest,
    /// The lowered `ProjectOp` — fed to `ProjectController::apply`
    /// verbatim inside `Command { expected_revision, op }`.
    pub op: ProjectOp,
    /// The preparation product.
    pub prepared: PreparedOperation,
}

// ── Lowering ─────────────────────────────────────────────────

/// The syntactic lowering `(bound target, validated args) -> ProjectOp`.
pub(crate) type LowerFn = fn(&OperationTarget, &Value) -> Result<ProjectOp, GatewayError>;

/// One entry in the semantic-operation table: a `ProjectMutation`
/// capability plus its input schema and syntactic lowering.
pub(crate) struct OpSpec {
    /// The capability descriptor registered for the semantic op.
    pub descriptor: crate::capability::CapabilityDescriptor,
    /// The registered input schema.
    pub input: crate::schema::ValueSchema,
    /// The lowering function.
    pub lower: LowerFn,
}

fn malformed(reason: impl Into<String>) -> GatewayError {
    GatewayError::MalformedRequest {
        reason: reason.into(),
    }
}

fn field<'a>(args: &'a Value, name: &str) -> Result<&'a Value, GatewayError> {
    args.get(name)
        .ok_or_else(|| malformed(format!("args.{name} is required")))
}

fn arg_u8(args: &Value, name: &str, min: u8, max: u8) -> Result<u8, GatewayError> {
    let n = field(args, name)?
        .as_i64()
        .ok_or_else(|| malformed(format!("args.{name} must be an integer")))?;
    u8::try_from(n)
        .ok()
        .filter(|v| *v >= min && *v <= max)
        .ok_or_else(|| malformed(format!("args.{name} must be within {min}..={max}")))
}

fn arg_i64(args: &Value, name: &str) -> Result<i64, GatewayError> {
    field(args, name)?
        .as_i64()
        .ok_or_else(|| malformed(format!("args.{name} must be an integer")))
}

fn arg_string(args: &Value, name: &str) -> Result<String, GatewayError> {
    field(args, name)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| malformed(format!("args.{name} must be a string")))
}

fn arg_bool(args: &Value, name: &str) -> Result<bool, GatewayError> {
    field(args, name)?
        .as_bool()
        .ok_or_else(|| malformed(format!("args.{name} must be a boolean")))
}

fn arg_typed<T: serde::de::DeserializeOwned>(args: &Value, name: &str) -> Result<T, GatewayError> {
    serde_json::from_value(field(args, name)?.clone())
        .map_err(|e| malformed(format!("args.{name}: {e}")))
}

fn arg_id(args: &Value, name: &str, kind: ResourceKind) -> Result<ResourceRef, GatewayError> {
    let text = field(args, name)?
        .as_str()
        .ok_or_else(|| malformed(format!("args.{name} must be a tagged id string")))?;
    ResourceRef::expect(kind, text)
}

/// A `{domain, start, len}` object — the schema already proved the
/// domain and `len > 0`; the parse layer re-reads them typed.
fn arg_range(args: &Value, name: &str) -> Result<TypedRange, GatewayError> {
    let obj = field(args, name)?
        .as_object()
        .ok_or_else(|| malformed(format!("args.{name} must be a range object")))?;
    let domain = match obj
        .get("domain")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(format!("args.{name}.domain is required")))?
    {
        "project-musical" => TimeDomain::ProjectMusical,
        "media" => TimeDomain::Media,
        "source-native" => TimeDomain::SourceNative,
        other => return Err(malformed(format!("args.{name}.domain: unknown {other}"))),
    };
    let start = obj
        .get("start")
        .and_then(Value::as_i64)
        .ok_or_else(|| malformed(format!("args.{name}.start must be an integer")))?;
    let len = obj
        .get("len")
        .and_then(Value::as_i64)
        .ok_or_else(|| malformed(format!("args.{name}.len must be an integer")))?;
    if len <= 0 {
        return Err(malformed(format!("args.{name}.len must be positive")));
    }
    Ok(TypedRange { domain, start, len })
}

/// A `{domain, tick}` point — the source-native tick domain the
/// capability contract fixes (domain tags stay explicit on the wire).
fn arg_point(args: &Value, name: &str) -> Result<(TimeDomain, i64), GatewayError> {
    let obj = field(args, name)?
        .as_object()
        .ok_or_else(|| malformed(format!("args.{name} must be a point object")))?;
    let domain = match obj
        .get("domain")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(format!("args.{name}.domain is required")))?
    {
        "project-musical" => TimeDomain::ProjectMusical,
        "media" => TimeDomain::Media,
        "source-native" => TimeDomain::SourceNative,
        other => return Err(malformed(format!("args.{name}.domain: unknown {other}"))),
    };
    let tick = obj
        .get("tick")
        .and_then(Value::as_i64)
        .ok_or_else(|| malformed(format!("args.{name}.tick must be an integer")))?;
    Ok((domain, tick))
}

fn resource(target: &OperationTarget) -> Result<ResourceRef, GatewayError> {
    match target {
        OperationTarget::Resource { resource } => Ok(*resource),
        _ => Err(GatewayError::Internal {
            reason: "lowering expected a resource target".to_owned(),
        }),
    }
}

fn parent(target: &OperationTarget) -> Result<ResourceRef, GatewayError> {
    match target {
        OperationTarget::Create { parent, .. } => Ok(*parent),
        _ => Err(GatewayError::Internal {
            reason: "lowering expected a create target".to_owned(),
        }),
    }
}

fn target_clip(target: &OperationTarget) -> Result<ClipId, GatewayError> {
    match resource(target)? {
        ResourceRef::Clip(id) => Ok(id),
        _ => Err(malformed("target is not a clip")),
    }
}

fn target_source(target: &OperationTarget) -> Result<SourceId, GatewayError> {
    match resource(target)? {
        ResourceRef::Source(id) => Ok(id),
        _ => Err(malformed("target is not a source")),
    }
}

fn target_note(target: &OperationTarget) -> Result<NoteId, GatewayError> {
    match resource(target)? {
        ResourceRef::Note(id) => Ok(id),
        _ => Err(malformed("target is not a note")),
    }
}

fn parent_track(target: &OperationTarget) -> Result<TrackId, GatewayError> {
    match parent(target)? {
        ResourceRef::Track(id) => Ok(id),
        _ => Err(malformed("create parent is not a track")),
    }
}

fn parent_lane(target: &OperationTarget) -> Result<LaneId, GatewayError> {
    match parent(target)? {
        ResourceRef::Lane(id) => Ok(id),
        _ => Err(malformed("create parent is not a lane")),
    }
}

fn parent_asset(target: &OperationTarget) -> Result<synvelis_project::AssetId, GatewayError> {
    match parent(target)? {
        ResourceRef::Asset(id) => Ok(id),
        _ => Err(malformed("create parent is not an asset")),
    }
}

// The lowering table — one `ProjectOp` per capability. Purely
// syntactic: no project state is consulted; legality is
// `Operation::validate`'s sole authority.

fn lower_track_create(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    let _ = parent(target)?;
    Ok(ProjectOp::AddTrack {
        name: arg_string(args, "name")?,
    })
}

fn lower_track_remove(target: &OperationTarget, _args: &Value) -> Result<ProjectOp, GatewayError> {
    match resource(target)? {
        ResourceRef::Track(track_id) => Ok(ProjectOp::RemoveTrack { track_id }),
        _ => Err(malformed("target is not a track")),
    }
}

fn lower_lane_create(target: &OperationTarget, _args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::AddLane {
        track_id: parent_track(target)?,
    })
}

fn lower_lane_remove(target: &OperationTarget, _args: &Value) -> Result<ProjectOp, GatewayError> {
    match resource(target)? {
        ResourceRef::Lane(lane_id) => Ok(ProjectOp::RemoveLane { lane_id }),
        _ => Err(malformed("target is not a lane")),
    }
}

fn lower_clip_add(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    let lane_id = parent_lane(target)?;
    let ResourceRef::Source(source_id) = arg_id(args, "source", ResourceKind::Source)? else {
        unreachable!("expect() enforces the kind")
    };
    Ok(ProjectOp::AddClip {
        lane_id,
        source_id,
        placement: arg_typed::<synvelis_project::ClipPlacement>(args, "placement")?,
        playback: arg_typed::<synvelis_project::ClipPlayback>(args, "playback")?,
    })
}

fn lower_clip_remove(target: &OperationTarget, _args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::RemoveClip {
        clip_id: target_clip(target)?,
    })
}

fn lower_clip_move(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::MoveClip {
        clip_id: target_clip(target)?,
        anchor: arg_typed::<synvelis_project::TimeAnchor>(args, "anchor")?,
    })
}

fn lower_clip_resize(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::SetClipExtent {
        clip_id: target_clip(target)?,
        extent: arg_typed::<synvelis_project::ClipExtent>(args, "extent")?,
    })
}

fn lower_clip_playback(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::SetClipPlayback {
        clip_id: target_clip(target)?,
        playback: arg_typed::<synvelis_project::ClipPlayback>(args, "playback")?,
    })
}

fn lower_clip_mute(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::SetClipMuted {
        clip_id: target_clip(target)?,
        muted: arg_bool(args, "muted")?,
    })
}

fn lower_clip_split(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::SplitClip {
        clip_id: target_clip(target)?,
        at: MusicalTick::new(arg_i64(args, "at")?),
        clip_b_source_offset: SourceFrame::new(arg_i64(args, "clip_b_source_offset")?)
            .map_err(|e| malformed(format!("args.clip_b_source_offset: {e}")))?,
    })
}

fn lower_clip_detach(target: &OperationTarget, _args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::DetachClipSource {
        clip_id: target_clip(target)?,
    })
}

fn lower_clip_remove_in_range(
    target: &OperationTarget,
    args: &Value,
) -> Result<ProjectOp, GatewayError> {
    let lanes = match target {
        OperationTarget::ResourceSet { resources } => resources
            .iter()
            .map(|r| match r {
                ResourceRef::Lane(id) => Ok(*id),
                _ => Err(malformed("target set member is not a lane")),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(GatewayError::Internal {
                reason: "lowering expected a resource-set target".to_owned(),
            });
        }
    };
    let range = arg_range(args, "range")?;
    let end = range
        .start
        .checked_add(range.len)
        .ok_or_else(|| malformed("args.range: range end overflows"))?;
    Ok(ProjectOp::RemoveClipsInRange {
        lanes,
        range: synvelis_project::MusicalRange::try_new(
            MusicalTick::new(range.start),
            MusicalTick::new(end),
        )
        .map_err(|e| malformed(format!("args.range: {e}")))?,
    })
}

fn lower_note_add(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::AddNote {
        clip: target_clip(target)?,
        key: arg_u8(args, "key", 0, 127)?,
        start: arg_typed::<synvelis_project::control::BeatPoint>(args, "start")?,
        len: arg_typed::<synvelis_project::control::BeatPoint>(args, "len")?,
    })
}

fn lower_note_remove(target: &OperationTarget, _args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::RemoveNote {
        note: target_note(target)?,
    })
}

fn lower_midi_source_create(
    _target: &OperationTarget,
    args: &Value,
) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::AddMidiSource {
        division: arg_typed::<synvelis_project::MidiDivision>(args, "division")?,
    })
}

fn lower_midi_source_remove(
    target: &OperationTarget,
    _args: &Value,
) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::RemoveMidiSource {
        source_id: target_source(target)?,
    })
}

fn lower_midi_note_add(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    let range = arg_range(args, "range")?;
    Ok(ProjectOp::AddMidiNote {
        source_id: target_source(target)?,
        key: arg_u8(args, "key", 0, 127)?,
        velocity: arg_u8(args, "velocity", 1, 127)?,
        start: MidiTick::new(range.start)
            .map_err(|e| malformed(format!("args.range.start: {e}")))?,
        len: MidiTick::new(range.len).map_err(|e| malformed(format!("args.range.len: {e}")))?,
    })
}

fn midi_note_id(args: &Value) -> Result<NoteId, GatewayError> {
    let ResourceRef::Note(id) = arg_id(args, "note", ResourceKind::Note)? else {
        unreachable!("expect() enforces the kind")
    };
    Ok(id)
}

fn lower_midi_note_remove(
    target: &OperationTarget,
    args: &Value,
) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::RemoveMidiNote {
        source_id: target_source(target)?,
        note_id: midi_note_id(args)?,
    })
}

fn lower_midi_note_set_velocity(
    target: &OperationTarget,
    args: &Value,
) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::SetMidiNoteVelocity {
        source_id: target_source(target)?,
        note_id: midi_note_id(args)?,
        velocity: arg_u8(args, "velocity", 1, 127)?,
    })
}

fn lower_midi_note_move(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    let (domain, tick) = arg_point(args, "position")?;
    if domain != TimeDomain::SourceNative {
        return Err(GatewayError::WrongTimeDomain {
            expected: TimeDomain::SourceNative,
            actual: domain,
        });
    }
    Ok(ProjectOp::MoveMidiNote {
        source_id: target_source(target)?,
        note_id: midi_note_id(args)?,
        start: MidiTick::new(tick).map_err(|e| malformed(format!("args.position.tick: {e}")))?,
    })
}

fn lower_asset_register(target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    let _ = parent(target)?;
    Ok(ProjectOp::RegisterAsset {
        digest: arg_string(args, "digest")?,
    })
}

fn lower_source_register(
    target: &OperationTarget,
    args: &Value,
) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::RegisterSource {
        asset_id: parent_asset(target)?,
        content_digest: arg_string(args, "content_digest")?,
    })
}

fn lower_source_set_state(
    target: &OperationTarget,
    args: &Value,
) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::SetSourceState {
        source_id: target_source(target)?,
        state: arg_typed::<synvelis_project::SourceState>(args, "state")?,
    })
}

fn lower_clock_epoch(_target: &OperationTarget, args: &Value) -> Result<ProjectOp, GatewayError> {
    Ok(ProjectOp::RegisterClockEpoch {
        description: arg_string(args, "description")?,
    })
}

/// The capability-id → lowering dispatch. `None` for the batch
/// capability (its members lower through the same table, handled by
/// the engine's recursion) and for any id outside the table.
pub(crate) fn lower_for(id: &str) -> Option<LowerFn> {
    Some(match id {
        "synvelis.track.create" => lower_track_create,
        "synvelis.track.remove" => lower_track_remove,
        "synvelis.lane.create" => lower_lane_create,
        "synvelis.lane.remove" => lower_lane_remove,
        "synvelis.clip.add" => lower_clip_add,
        "synvelis.clip.remove" => lower_clip_remove,
        "synvelis.clip.move" => lower_clip_move,
        "synvelis.clip.resize" => lower_clip_resize,
        "synvelis.clip.playback" => lower_clip_playback,
        "synvelis.clip.mute" => lower_clip_mute,
        "synvelis.clip.split" => lower_clip_split,
        "synvelis.clip.detach-source" => lower_clip_detach,
        "synvelis.clip.remove-in-range" => lower_clip_remove_in_range,
        "synvelis.note.add" => lower_note_add,
        "synvelis.note.remove" => lower_note_remove,
        "synvelis.midi.source.create" => lower_midi_source_create,
        "synvelis.midi.source.remove" => lower_midi_source_remove,
        "synvelis.midi.note.add" => lower_midi_note_add,
        "synvelis.midi.note.remove" => lower_midi_note_remove,
        "synvelis.midi.note.set-velocity" => lower_midi_note_set_velocity,
        "synvelis.midi.note.move" => lower_midi_note_move,
        "synvelis.asset.register" => lower_asset_register,
        "synvelis.source.register" => lower_source_register,
        "synvelis.source.set-state" => lower_source_set_state,
        "synvelis.clock-epoch.register" => lower_clock_epoch,
        // GW-07: the verified-media placement op — its content digest
        // must come from the media gate, so the engine's gated path
        // builds the op; this entry only keeps the table total.
        crate::job_engine::MEDIA_REGISTER_ID => |_, _| {
            Err(GatewayError::Internal {
                reason: "media.register lowers through the media gate".to_owned(),
            })
        },
        _ => return None,
    })
}

/// The `ProjectOp` variant tag emitted in normalized commands.
pub(crate) fn op_name(op: &ProjectOp) -> &'static str {
    match op {
        ProjectOp::SetProjectName { .. } => "set-project-name",
        ProjectOp::SetPpqn { .. } => "set-ppqn",
        ProjectOp::AddTempoEvent { .. } => "add-tempo-event",
        ProjectOp::AddSignatureEvent { .. } => "add-signature-event",
        ProjectOp::AddTrack { .. } => "add-track",
        ProjectOp::RemoveTrack { .. } => "remove-track",
        ProjectOp::AddLane { .. } => "add-lane",
        ProjectOp::RemoveLane { .. } => "remove-lane",
        ProjectOp::AddClip { .. } => "add-clip",
        ProjectOp::RemoveClip { .. } => "remove-clip",
        ProjectOp::MoveClip { .. } => "move-clip",
        ProjectOp::SetClipExtent { .. } => "set-clip-extent",
        ProjectOp::SetClipPlayback { .. } => "set-clip-playback",
        ProjectOp::SetClipMuted { .. } => "set-clip-muted",
        ProjectOp::AddNote { .. } => "add-note",
        ProjectOp::RemoveNote { .. } => "remove-note",
        ProjectOp::AddMidiSource { .. } => "add-midi-source",
        ProjectOp::RemoveMidiSource { .. } => "remove-midi-source",
        ProjectOp::AddMidiNote { .. } => "add-midi-note",
        ProjectOp::RemoveMidiNote { .. } => "remove-midi-note",
        ProjectOp::SetMidiNoteVelocity { .. } => "set-midi-note-velocity",
        ProjectOp::MoveMidiNote { .. } => "move-midi-note",
        ProjectOp::DetachClipSource { .. } => "detach-clip-source",
        ProjectOp::RegisterAsset { .. } => "register-asset",
        ProjectOp::RegisterSource { .. } => "register-source",
        ProjectOp::SetSourceState { .. } => "set-source-state",
        ProjectOp::RegisterClockEpoch { .. } => "register-clock-epoch",
        ProjectOp::Composite { .. } => "composite",
        ProjectOp::SplitClip { .. } => "split-clip",
        ProjectOp::RemoveClipsInRange { .. } => "remove-clips-in-range",
        ProjectOp::AddParameterTarget { .. } => "add-parameter-target",
        ProjectOp::RemoveParameterTarget { .. } => "remove-parameter-target",
        ProjectOp::SetParameterBase { .. } => "set-parameter-base",
        ProjectOp::AddAutomationLane { .. } => "add-automation-lane",
        ProjectOp::RemoveAutomationLane { .. } => "remove-automation-lane",
        ProjectOp::WriteAutomationRegion { .. } => "write-automation-region",
        ProjectOp::EraseAutomationRange { .. } => "erase-automation-range",
        ProjectOp::AddModulationSource { .. } => "add-modulation-source",
        ProjectOp::RemoveModulationSource { .. } => "remove-modulation-source",
        ProjectOp::AddModulationAssignment { .. } => "add-modulation-assignment",
        ProjectOp::SetModulationAssignment { .. } => "set-modulation-assignment",
        ProjectOp::RemoveModulationAssignment { .. } => "remove-modulation-assignment",
    }
}

// ── Semantic op facts ────────────────────────────────────────

/// What a lowered `ProjectOp` semantically consults: resources read
/// internally beyond its writes, the preconditions `validate`
/// enforces, and dimensions outside the observable model.
pub(crate) struct OpFacts {
    /// Resources the op reads internally (never disclosed).
    pub reads: BTreeSet<ResourceRef>,
    /// Resources sharing the written source — `source → linked clips`.
    /// A linked-MIDI write lands on the shared source; every clip
    /// reading it is *impacted* without its own attributes changing —
    /// this is the explicit shared-link edge GTC-005 requires the
    /// effect record to name (never a clip-local copy-on-write).
    pub impacted: BTreeMap<ResourceRef, BTreeSet<ResourceRef>>,
    /// Named preconditions mirrored from `Operation::validate`.
    pub preconditions: Vec<Precondition>,
    /// Unverifiable effect dimensions — non-empty fails closed.
    pub unsupported: Vec<String>,
}

fn cond(
    code: &'static str,
    resource: Option<ResourceRef>,
    detail: impl Into<String>,
) -> Precondition {
    Precondition {
        code,
        resource,
        detail: detail.into(),
    }
}

fn project_ref(project: &Project) -> ResourceRef {
    ResourceRef::Project(project.project_id)
}

/// The semantic facts of a `ProjectOp` against the pinned `project`.
///
/// `reads` name resources consulted internally beyond the op's own
/// writes — a linked-source read (detach copies S1's payload), a
/// creation parent, a clip's referenced source. Inside-source note
/// ids are *not* separate reads: notes are source payload, addressable
/// only through their source (SEC-05's sharing model).
#[allow(clippy::too_many_lines)] // flat semantic table — one arm per op
pub(crate) fn op_facts(op: &ProjectOp, project: &Project) -> OpFacts {
    let mut reads = BTreeSet::new();
    let mut impacted: BTreeMap<ResourceRef, BTreeSet<ResourceRef>> = BTreeMap::new();
    let mut preconditions = Vec::new();
    let mut unsupported = Vec::new();
    let mut read = |r: ResourceRef| {
        reads.insert(r);
    };
    // Every clip holding `source` — the shared-link set an edit to
    // that source impacts (SEC-05's explicit sharing model).
    let linked_clips = |source: SourceId| -> BTreeSet<ResourceRef> {
        project
            .clips
            .values()
            .filter(|c| c.source_id == source)
            .map(|c| ResourceRef::Clip(c.id))
            .collect()
    };
    match op {
        ProjectOp::AddTrack { .. } => {
            read(project_ref(project));
            preconditions.push(cond("project-exists", Some(project_ref(project)), ""));
        }
        ProjectOp::RemoveTrack { track_id } => {
            read(ResourceRef::Track(*track_id));
            preconditions.push(cond(
                "track-exists",
                Some(ResourceRef::Track(*track_id)),
                "",
            ));
            preconditions.push(cond("track-has-no-lanes", None, ""));
        }
        ProjectOp::AddLane { track_id } => {
            read(ResourceRef::Track(*track_id));
            preconditions.push(cond(
                "track-exists",
                Some(ResourceRef::Track(*track_id)),
                "",
            ));
        }
        ProjectOp::RemoveLane { lane_id } => {
            read(ResourceRef::Lane(*lane_id));
            preconditions.push(cond("lane-exists", Some(ResourceRef::Lane(*lane_id)), ""));
            preconditions.push(cond("lane-empty", None, ""));
            preconditions.push(cond("lane-not-capture-source", None, ""));
        }
        ProjectOp::AddClip {
            lane_id, source_id, ..
        } => {
            read(ResourceRef::Lane(*lane_id));
            read(ResourceRef::Source(*source_id));
            preconditions.push(cond("lane-exists", Some(ResourceRef::Lane(*lane_id)), ""));
            preconditions.push(cond(
                "source-exists",
                Some(ResourceRef::Source(*source_id)),
                "",
            ));
            preconditions.push(cond("placement-legal", None, ""));
        }
        ProjectOp::RemoveClip { clip_id } | ProjectOp::SetClipMuted { clip_id, .. } => {
            read(ResourceRef::Clip(*clip_id));
            preconditions.push(cond("clip-exists", Some(ResourceRef::Clip(*clip_id)), ""));
        }
        ProjectOp::MoveClip { clip_id, .. } | ProjectOp::SetClipPlayback { clip_id, .. } => {
            read(ResourceRef::Clip(*clip_id));
            preconditions.push(cond("clip-exists", Some(ResourceRef::Clip(*clip_id)), ""));
            preconditions.push(cond("clip-contract-valid", None, ""));
        }
        ProjectOp::SetClipExtent { clip_id, .. } => {
            read(ResourceRef::Clip(*clip_id));
            preconditions.push(cond("clip-exists", Some(ResourceRef::Clip(*clip_id)), ""));
            preconditions.push(cond("extent-fits-playback", None, ""));
            preconditions.push(cond("clip-contract-valid", None, ""));
        }
        ProjectOp::AddNote { clip, .. } => {
            read(ResourceRef::Clip(*clip));
            preconditions.push(cond("clip-exists", Some(ResourceRef::Clip(*clip)), ""));
            preconditions.push(cond("note-fits-clip", None, ""));
        }
        ProjectOp::RemoveNote { note } => {
            read(ResourceRef::Note(*note));
            preconditions.push(cond("note-exists", Some(ResourceRef::Note(*note)), ""));
        }
        ProjectOp::AddMidiSource { .. } => {
            read(project_ref(project));
            preconditions.push(cond("division-positive", None, ""));
        }
        ProjectOp::RemoveMidiSource { source_id } => {
            read(ResourceRef::Source(*source_id));
            preconditions.push(cond(
                "midi-source-exists",
                Some(ResourceRef::Source(*source_id)),
                "",
            ));
            preconditions.push(cond(
                "midi-source-unreferenced",
                None,
                "no clip may reference the source",
            ));
        }
        ProjectOp::AddMidiNote { source_id, .. } => {
            read(ResourceRef::Source(*source_id));
            impacted.insert(ResourceRef::Source(*source_id), linked_clips(*source_id));
            preconditions.push(cond(
                "midi-source-exists",
                Some(ResourceRef::Source(*source_id)),
                "",
            ));
            preconditions.push(cond("note-fields-valid", None, ""));
        }
        ProjectOp::SetMidiNoteVelocity { source_id, .. }
        | ProjectOp::MoveMidiNote { source_id, .. } => {
            read(ResourceRef::Source(*source_id));
            impacted.insert(ResourceRef::Source(*source_id), linked_clips(*source_id));
            preconditions.push(cond(
                "midi-source-exists",
                Some(ResourceRef::Source(*source_id)),
                "",
            ));
            preconditions.push(cond("note-exists", None, ""));
            preconditions.push(cond("note-fields-valid", None, ""));
        }
        ProjectOp::RemoveMidiNote { source_id, .. } => {
            read(ResourceRef::Source(*source_id));
            impacted.insert(ResourceRef::Source(*source_id), linked_clips(*source_id));
            preconditions.push(cond(
                "midi-source-exists",
                Some(ResourceRef::Source(*source_id)),
                "",
            ));
            preconditions.push(cond("note-exists", None, ""));
        }
        ProjectOp::DetachClipSource { clip_id } => {
            read(ResourceRef::Clip(*clip_id));
            if let Some(clip) = project.clips.get(clip_id) {
                read(ResourceRef::Source(clip.source_id));
            }
            preconditions.push(cond("clip-exists", Some(ResourceRef::Clip(*clip_id)), ""));
            preconditions.push(cond("clip-source-is-midi", None, ""));
        }
        ProjectOp::RegisterAsset { .. } => {
            read(project_ref(project));
        }
        ProjectOp::RegisterSource { asset_id, .. } => {
            read(ResourceRef::Asset(*asset_id));
            preconditions.push(cond(
                "asset-exists",
                Some(ResourceRef::Asset(*asset_id)),
                "",
            ));
        }
        ProjectOp::SetSourceState { source_id, .. } => {
            read(ResourceRef::Source(*source_id));
            preconditions.push(cond(
                "media-source-exists",
                Some(ResourceRef::Source(*source_id)),
                "",
            ));
            preconditions.push(cond("state-transition-forward", None, ""));
        }
        ProjectOp::RegisterClockEpoch { .. } => {
            read(project_ref(project));
            // Clock epochs live outside the gateway resource model —
            // no ResourceRef can name one, so the effect is honestly
            // unverifiable rather than "absent" (SEC-14).
            unsupported
                .push("clock-epoch registration has no observable resource effect".to_owned());
        }
        ProjectOp::Composite { ops } => {
            for member in ops {
                let facts = op_facts(member, project);
                reads.extend(facts.reads);
                for (source, clips) in facts.impacted {
                    impacted.entry(source).or_default().extend(clips);
                }
                preconditions.extend(facts.preconditions);
                unsupported.extend(facts.unsupported);
            }
            preconditions.push(cond("composite-non-empty", None, ""));
        }
        ProjectOp::SplitClip { clip_id, .. } => {
            read(ResourceRef::Clip(*clip_id));
            preconditions.push(cond("clip-exists", Some(ResourceRef::Clip(*clip_id)), ""));
            preconditions.push(cond("clip-musically-anchored", None, ""));
            preconditions.push(cond("split-inside-extent", None, ""));
            preconditions.push(cond("split-offset-not-before-source", None, ""));
            preconditions.push(cond("no-note-straddles-split", None, ""));
        }
        ProjectOp::RemoveClipsInRange { lanes, .. } => {
            for lane_id in lanes {
                read(ResourceRef::Lane(*lane_id));
                preconditions.push(cond("lane-exists", Some(ResourceRef::Lane(*lane_id)), ""));
            }
        }
        // Control-plane, media-state, and any unlowered variant: the
        // observable attribute diff cannot fully classify them —
        // fail closed rather than claim a partial effect set.
        _ => {
            unsupported.push("operation effects are outside the observable model".to_owned());
        }
    }
    OpFacts {
        reads,
        impacted,
        preconditions,
        unsupported,
    }
}

// ── Effect derivation ────────────────────────────────────────

/// One resource's full observable state: the GW-02 scope attributes
/// (owner/range/related — what scope coverage reads) plus the
/// entity's own serialized body (`muted`, `velocity`, `state`,
/// `name`… — the mutable fields `ResourceAttributes` deliberately
/// does not model). Both halves participate in the before/after diff:
/// a change to either is a `Write`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Observed {
    /// Scope-level attributes (GW-02's fact view).
    pub attrs: ResourceAttributes,
    /// The entity's serialized state — detail-level truth.
    pub detail: Value,
}

/// `serde_json::to_value` never fails on these aggregate members
/// (plain data, all `Serialize`).
fn to_detail<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("aggregate members serialize")
}

/// The kind-specific mutable body the scope attributes do not model.
fn detail_of(project: &Project, resource: &ResourceRef) -> Value {
    match resource {
        ResourceRef::Project(id) if *id == project.project_id => serde_json::json!({
            "name": project.name,
            "ppqn": project.ppqn,
            "tempo_map": project.tempo_map,
        }),
        ResourceRef::Track(id) => project.tracks.get(id).map_or(Value::Null, to_detail),
        ResourceRef::Lane(id) => project.lanes.get(id).map_or(Value::Null, to_detail),
        ResourceRef::Clip(id) => project.clips.get(id).map_or(Value::Null, to_detail),
        ResourceRef::Source(id) => project.sources.get(id).map_or_else(
            || project.midi_sources.get(id).map_or(Value::Null, to_detail),
            to_detail,
        ),
        ResourceRef::Asset(id) => project.assets.get(id).map_or(Value::Null, to_detail),
        ResourceRef::Note(id) => project.notes.get(id).map_or_else(
            || {
                project
                    .midi_sources
                    .values()
                    .find_map(|s| s.notes.get(id))
                    .map_or(Value::Null, to_detail)
            },
            to_detail,
        ),
        ResourceRef::TakeGroup(id) => project
            .comp
            .take_groups
            .get(id)
            .map_or(Value::Null, to_detail),
        ResourceRef::Comp(id) => project.comp.comps.get(id).map_or(Value::Null, to_detail),
        ResourceRef::ParameterTarget(id) => project
            .control
            .targets
            .get(id)
            .map_or(Value::Null, to_detail),
        ResourceRef::AutomationLane(id) => {
            project.control.lanes.get(id).map_or(Value::Null, to_detail)
        }
        ResourceRef::Project(_) => Value::Null,
    }
}

/// The `{resource → observed}` map of every resource the gateway
/// can name in `project` — the before/after observation basis.
pub(crate) fn resource_map(project: &Project) -> BTreeMap<ResourceRef, Observed> {
    let mut map = BTreeMap::new();
    map.insert(
        project_ref(project),
        Observed {
            attrs: ResourceAttributes {
                kind: ResourceKind::Project,
                owner: None,
                range: None,
                created_revision: None,
                related: Vec::new(),
            },
            detail: detail_of(project, &project_ref(project)),
        },
    );
    for kind in snapshot::ENUMERATED_KINDS {
        for r in snapshot::enumerate_kind(project, kind) {
            if let Some(attrs) = snapshot::attributes(project, &r) {
                map.insert(
                    r,
                    Observed {
                        attrs,
                        detail: detail_of(project, &r),
                    },
                );
            }
        }
    }
    map
}

/// Classifies the before/after diff plus declared internal reads into
/// [`DerivedEffect`] records.
///
/// - absent → present = `Create` (parent = the created attrs' owner)
/// - present → changed = `Write`
/// - present → absent = `Delete`
/// - declared reads = `Read` (internal visibility — validation reads
///   never disclose)
///
/// Deterministic emission order: reads first (`BTreeSet` order), then
/// creates, writes, deletes in `ResourceRef` order.
pub(crate) fn derive_effects(
    before: &BTreeMap<ResourceRef, Observed>,
    after: &BTreeMap<ResourceRef, Observed>,
    reads: &BTreeSet<ResourceRef>,
) -> Vec<DerivedEffect> {
    let mut effects = Vec::new();
    for resource in reads {
        let before_obs = before.get(resource);
        let after_obs = after.get(resource);
        effects.push(DerivedEffect {
            kind: EffectKind::Read,
            resource: *resource,
            visibility: EffectVisibility::Internal,
            before: before_obs.map(|o| o.attrs.clone()),
            after: after_obs.map(|o| o.attrs.clone()),
            before_detail: before_obs.map(|o| o.detail.clone()),
            after_detail: after_obs.map(|o| o.detail.clone()),
            references: before_obs
                .map(|a| a.attrs.related.clone())
                .unwrap_or_default(),
            data_class: None,
        });
    }
    for (resource, after_obs) in after {
        match before.get(resource) {
            None => effects.push(DerivedEffect {
                kind: EffectKind::Create,
                resource: *resource,
                visibility: EffectVisibility::Internal,
                before: None,
                after: Some(after_obs.attrs.clone()),
                before_detail: None,
                after_detail: Some(after_obs.detail.clone()),
                references: after_obs.attrs.related.clone(),
                data_class: None,
            }),
            Some(before_obs) if before_obs != after_obs => effects.push(DerivedEffect {
                kind: EffectKind::Write,
                resource: *resource,
                visibility: EffectVisibility::Internal,
                before: Some(before_obs.attrs.clone()),
                after: Some(after_obs.attrs.clone()),
                before_detail: Some(before_obs.detail.clone()),
                after_detail: Some(after_obs.detail.clone()),
                references: after_obs.attrs.related.clone(),
                data_class: None,
            }),
            _ => {}
        }
    }
    for (resource, before_obs) in before {
        if !after.contains_key(resource) {
            effects.push(DerivedEffect {
                kind: EffectKind::Delete,
                resource: *resource,
                visibility: EffectVisibility::Internal,
                before: Some(before_obs.attrs.clone()),
                after: None,
                before_detail: Some(before_obs.detail.clone()),
                after_detail: None,
                references: before_obs.attrs.related.clone(),
                data_class: None,
            });
        }
    }
    effects
}

/// Flattens [`DerivedEffect`]s into the GW-02 [`ScopedEffects`] view
/// plus the after-state records SEC-06's destination coverage needs.
pub(crate) fn prepared_effects(
    effects: &[DerivedEffect],
    capabilities: Vec<CapabilityRef>,
    unsupported: Vec<String>,
) -> PreparedEffects {
    let mut scoped = ScopedEffects {
        unsupported,
        ..ScopedEffects::default()
    };
    let mut after_states = Vec::new();
    for effect in effects {
        match effect.kind {
            EffectKind::Read => scoped.reads.push(effect.resource),
            EffectKind::Write => {
                scoped.writes.push(effect.resource);
                if let (Some(before), Some(after)) = (&effect.before, &effect.after)
                    && (before.range != after.range || before.owner != after.owner)
                {
                    after_states.push((effect.resource, after.clone()));
                }
            }
            EffectKind::Create => {
                let parent = effect
                    .after
                    .as_ref()
                    .and_then(|a| a.owner)
                    .expect("created resources carry an owner");
                scoped.creates.push(CreateEffect {
                    parent,
                    creates: effect.after.as_ref().expect("create has after").kind,
                    range: effect.after.as_ref().and_then(|a| a.range),
                });
            }
            EffectKind::Delete => scoped.deletes.push(effect.resource),
            EffectKind::Disclose => scoped
                .disclosures
                .push(effect.data_class.unwrap_or(DataAccessClass::MetadataRead)),
            EffectKind::ExternalAction => {
                scoped.external_transfers = scoped.external_transfers.saturating_add(1);
            }
        }
    }
    PreparedEffects {
        scoped,
        after_states,
        capabilities,
    }
}

/// The data-access classes a prepared result actually emits.
///
/// `MetadataRead` is the floor (ids, kinds, structure); any effect
/// touching `Note` resources or carrying a `SourceNative`-domain range
/// discloses structured content — note positions in source-native
/// ticks are content, not metadata (DATA-01).
pub(crate) fn emitted_classes(effects: &[DerivedEffect]) -> BTreeSet<DataAccessClass> {
    let mut classes = BTreeSet::from([DataAccessClass::MetadataRead]);
    for effect in effects {
        let content = effect.resource.kind() == ResourceKind::Note
            || [&effect.before, &effect.after]
                .into_iter()
                .flatten()
                .any(|a| {
                    a.range
                        .is_some_and(|r| r.domain == TimeDomain::SourceNative)
                })
            // A `Source`-kind detail can embed the whole `MidiSource`
            // notes map — structured content even though the resource
            // itself is not a `Note` and carries no native range.
            || [&effect.before_detail, &effect.after_detail]
                .into_iter()
                .flatten()
                .any(|d| d.get("notes").is_some());
        if content {
            classes.insert(DataAccessClass::StructuredContentRead);
        }
    }
    classes
}

// ── Authorization seam ───────────────────────────────────────

/// The fact source for prepare authorization: the pinned *base* first
/// (what every existing resource actually is), then the *candidate*
/// (so a `create` inside the same transaction can resolve a parent
/// that the base does not carry — e.g. a detached note's new source).
///
/// Order is load-bearing: base-first keeps writes/deletes evaluated
/// against the state the caller was granted over (the destination
/// re-check is `PreparedEffects::after_states`' job, not this one's).
pub(crate) struct BaseFirstOracle {
    base: snapshot::ProjectOracle,
    candidate: snapshot::ProjectOracle,
}

impl BaseFirstOracle {
    /// `base` serves facts for existing resources; `candidate` only
    /// resolves resources the base cannot (transaction-created ids).
    #[must_use]
    pub(crate) fn new(base: &Arc<Project>, candidate: &Arc<Project>) -> Self {
        Self {
            base: snapshot::ProjectOracle::new(base.clone()),
            candidate: snapshot::ProjectOracle::new(candidate.clone()),
        }
    }
}

impl ResourceOracle for BaseFirstOracle {
    fn facts(&self, project: ProjectId, resource: &ResourceRef) -> crate::grant::ResourceFacts {
        match self.base.facts(project, resource) {
            crate::grant::ResourceFacts::Known(attrs) => crate::grant::ResourceFacts::Known(attrs),
            crate::grant::ResourceFacts::Unresolved => self.candidate.facts(project, resource),
        }
    }
}

/// The GW-02 boundary a prepare call evaluates (GTC-003 middle stage).
///
/// Two checks, both on the same binding chain the evaluator owns:
///
/// - [`authorize_effects`][Self::authorize_effects] — the *derived*
///   effects against grant scope / `create_within` / `data_access`,
///   the inner capability floor, and SEC-06 after-state coverage.
/// - [`authorize_emission`][Self::authorize_emission] — the classes the
///   result actually emits, plus `ExternalDisclosure` for external
///   callers (the GW-03 disclosure boundary reused verbatim).
pub trait PrepareAuthorizer: Send + Sync {
    /// Effect-aware proposal authorization. `facts` is the pinned-base
    /// fact source — coverage is judged on the base the derivation ran
    /// against, never a newer live view.
    fn authorize_effects(
        &self,
        req: &OperationRequest,
        capability: &crate::capability::CapabilityDescriptor,
        prepared: &PreparedEffects,
        facts: Arc<dyn ResourceOracle>,
    ) -> AuthorizationDecision;

    /// Result-emission authorization — `classes` against the grant's
    /// `data_access`, plus `ExternalDisclosure` for external callers.
    fn authorize_emission(
        &self,
        req: &OperationRequest,
        capability: &crate::capability::CapabilityDescriptor,
        classes: &BTreeSet<DataAccessClass>,
        external: bool,
    ) -> AuthorizationDecision;
}

// ── Serialization helpers (internal result surface) ─────────

/// The canonical tagged id string of a `ResourceRef` (the inner
/// project id types own `to_tagged`).
pub(crate) fn ref_tagged(r: &ResourceRef) -> String {
    match r {
        ResourceRef::Project(id) => id.to_tagged(),
        ResourceRef::Track(id) => id.to_tagged(),
        ResourceRef::Lane(id) => id.to_tagged(),
        ResourceRef::Clip(id) => id.to_tagged(),
        ResourceRef::Source(id) => id.to_tagged(),
        ResourceRef::Asset(id) => id.to_tagged(),
        ResourceRef::Note(id) => id.to_tagged(),
        ResourceRef::TakeGroup(id) => id.to_tagged(),
        ResourceRef::Comp(id) => id.to_tagged(),
        ResourceRef::ParameterTarget(id) => id.to_tagged(),
        ResourceRef::AutomationLane(id) => id.to_tagged(),
    }
}

fn kind_json(kind: ResourceKind) -> Value {
    serde_json::to_value(kind).expect("resource kind serializes")
}

fn range_json(range: &TypedRange) -> Value {
    serde_json::json!({
        "domain": match range.domain {
            TimeDomain::ProjectMusical => "project-musical",
            TimeDomain::Media => "media",
            TimeDomain::SourceNative => "source-native",
        },
        "start": range.start,
        "len": range.len,
    })
}

pub(crate) fn attrs_json(attrs: &ResourceAttributes) -> Value {
    serde_json::json!({
        "kind": kind_json(attrs.kind),
        "owner": attrs.owner.map(|r| ref_tagged(&r)),
        "range": attrs.range.as_ref().map(range_json),
        "related": attrs.related.iter().map(ref_tagged).collect::<Vec<_>>(),
    })
}

pub(crate) fn target_json(target: &OperationTarget) -> Value {
    match target {
        OperationTarget::None => Value::Null,
        OperationTarget::Resource { resource } => serde_json::json!({
            "shape": "resource",
            "id": ref_tagged(resource),
        }),
        OperationTarget::ResourceSet { resources } => serde_json::json!({
            "shape": "resource-set",
            "ids": resources.iter().map(ref_tagged).collect::<Vec<_>>(),
        }),
        OperationTarget::Create { parent, creates } => serde_json::json!({
            "shape": "create",
            "parent": ref_tagged(parent),
            "creates": kind_json(*creates),
        }),
    }
}

pub(crate) fn effect_kind_name(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Read => "read",
        EffectKind::Write => "write",
        EffectKind::Create => "create",
        EffectKind::Delete => "delete",
        EffectKind::Disclose => "disclose",
        EffectKind::ExternalAction => "external-action",
    }
}

fn effect_json(effect: &DerivedEffect) -> Value {
    serde_json::json!({
        "kind": effect_kind_name(effect.kind),
        "visibility": match effect.visibility {
            EffectVisibility::Internal => "internal",
            EffectVisibility::External => "external",
        },
        "resource": ref_tagged(&effect.resource),
        "before": effect.before.as_ref().map(attrs_json),
        "after": effect.after.as_ref().map(attrs_json),
        "before_detail": effect.before_detail.clone(),
        "after_detail": effect.after_detail.clone(),
        "references": effect.references.iter().map(ref_tagged).collect::<Vec<_>>(),
    })
}

/// The result payload emitted for a completed prepare — the
/// internal-facing view of a [`PreparedRecord`].
pub(crate) fn result_json(proposal: &ProposalRef, prepared: &PreparedOperation) -> Value {
    serde_json::json!({
        "proposal": proposal.as_str(),
        "operation": prepared.operation.to_tagged(),
        "base": {
            "project": prepared.base.project.to_tagged(),
            "revision": prepared.base.revision.0,
            "snapshot": prepared.base.snapshot.as_str(),
            "tempo_map_revision": prepared.base.tempo_map_revision,
        },
        "commands": prepared.commands.iter().map(|c| serde_json::json!({
            "capability": serde_json::to_value(&c.capability).expect("capability ref serializes"),
            "op": c.op,
            "target": target_json(&c.target),
            "args": c.args,
        })).collect::<Vec<_>>(),
        "preconditions": prepared.preconditions.iter().map(|p| serde_json::json!({
            "code": p.code,
            "resource": p.resource.map(|r| ref_tagged(&r)),
            "detail": p.detail,
        })).collect::<Vec<_>>(),
        "effects": prepared.effects.iter().map(effect_json).collect::<Vec<_>>(),
        "unsupported": prepared.unsupported,
        "candidate": { "base_revision": prepared.base.revision.0 },
    })
}
