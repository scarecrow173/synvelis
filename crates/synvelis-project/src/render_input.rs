//! Render-input read adapter (synvelis#290, RCP-01).
//!
//! This module is the *read side* of the canonical Render Recipe
//! contract (`docs/specs/render-checkpoint.md`): it resolves which
//! [`Project`] a render recipe is computed over and exposes the
//! canonical, name-free semantic entity values the recipe digests
//! are built from.
//!
//! Two responsibilities live here, both owned by the project crate
//! because they need `pub(crate)` apply/variant machinery:
//!
//! - [`resolve_render_project`] — produces the *effective* project a
//!   recipe resolves against. For a live (non-branch) request that is
//!   the project itself; for a [`BranchContentRef::Captured`] branch
//!   it composes the branch's scope-owned entities (decoded from the
//!   [`BranchContentStore`], digest/schema/reference verified) over the
//!   live shared context, reusing the adopt transplant plan
//!   ([`crate::variant::engine::plan_adopt_writes`] +
//!   [`crate::apply::entity::write_all`]) so the composed view is
//!   exactly what adopt would materialize — never a divergent
//!   hand-rolled merge. [`BranchContentRef::Live`] branches mirror the
//!   live scope by construction and resolve to the live project.
//! - Canonical entity extraction — [`all_semantic_objects`],
//!   [`canonical_entity_value`], [`canonical_entity_digest`] and
//!   [`canonical_fingerprint`] produce deterministic, display-name-free
//!   semantic values. Display `name` fields are stripped at the top
//!   level of every entity (UI rename must never move a recipe key,
//!   RCP-REC-01); non-finite floats are rejected during serialization
//!   rather than silently normalized to `null` (RCP-REC-02).
//!
//! The adapter never mutates the live project, never writes the
//! variant model, and treats every persisted pin (payload digest,
//! scope digest, schema version) as a claim to *re-verify*, not a
//! trusted cache: a mismatch is a typed [`RenderInputError::Corrupt`],
//! never a silent pass.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use crate::{
    Error, Project,
    apply::entity,
    diff::{ObjectKind, SemanticObjectRef},
    id::{
        AssetId, AutomationLaneId, BranchId, BusId, CaptureSegmentId, CaptureStreamId, ChainId,
        ClipId, ClockEpochId, CompId, CompRegionId, CompTransitionId, CounterIdGenerator, ImportId,
        LaneId, ModulationAssignmentId, ModulationSourceId, NoteId, ParameterTargetId, ProcessorId,
        ProjectId, RecordingPassId, RecordingSessionId, RoutingConnectionId, SemanticPortId,
        SendId, SourceId, StripId, TakeGroupId, TakeId, TrackId, VariantSetId,
    },
    snapshot::ContentDigest,
    variant::{
        BranchContentRef, BranchContentStore, BranchRevision, VariantError, decode_candidate,
    },
};

/// Schema version of this adapter's canonical-output contract. Bump
/// when the canonical entity projection changes meaning; recipe keys
/// derived from it must never collide across versions.
pub const RENDER_INPUT_SCHEMA_VERSION: u32 = 1;

/// Fail-closed error of the render-input read adapter.
#[derive(Clone, Debug)]
pub enum RenderInputError {
    /// A referenced variant set / branch does not exist.
    NotFound {
        /// Human-readable locator.
        detail: String,
    },
    /// The requested content cannot be resolved in this slice
    /// (e.g. adopt materialization of an apply-unsupported kind).
    Unsupported {
        /// Human-readable reason.
        detail: String,
    },
    /// A persisted pin failed re-verification (payload digest, scope
    /// digest, schema version, reference integrity).
    Corrupt {
        /// Human-readable reason.
        detail: String,
    },
    /// The composed view failed project reference validation.
    InvalidProject(Error),
    /// Canonical serialization failed (including non-finite floats).
    Canonical {
        /// Human-readable reason.
        detail: String,
    },
}

impl std::fmt::Display for RenderInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { detail } => write!(f, "render input not found: {detail}"),
            Self::Unsupported { detail } => write!(f, "render input unsupported: {detail}"),
            Self::Corrupt { detail } => write!(f, "render input corrupt: {detail}"),
            Self::InvalidProject(e) => write!(f, "render input project invalid: {e}"),
            Self::Canonical { detail } => write!(f, "render input canonicalization: {detail}"),
        }
    }
}

impl std::error::Error for RenderInputError {}

impl From<VariantError> for RenderInputError {
    fn from(e: VariantError) -> Self {
        match e {
            VariantError::CandidateUnavailable { detail } => Self::Corrupt { detail },
            VariantError::Unsupported { detail } => Self::Unsupported { detail },
            other => Self::Unsupported {
                detail: format!("{other}"),
            },
        }
    }
}

impl From<Error> for RenderInputError {
    fn from(e: Error) -> Self {
        Self::InvalidProject(e)
    }
}

// ---------------------------------------------------------------------------
// branch target / provenance
// ---------------------------------------------------------------------------

/// Selects which variant branch a render input resolves, plus the
/// content store needed to decode captured candidates.
#[derive(Clone, Copy)]
pub struct RenderBranchTarget<'a> {
    /// Variant set containing the branch.
    pub variant_set: VariantSetId,
    /// Branch whose content the render resolves.
    pub branch: BranchId,
    /// Store holding captured candidate payloads.
    pub store: &'a dyn BranchContentStore,
}

/// Provenance pin of a resolved branch render input. Every digest
/// here is *recomputed* during resolution — the persisted values are
/// claims that must match, never trusted shortcuts.
#[derive(Clone, Debug, PartialEq)]
pub struct BranchRenderPin {
    /// Variant set the branch belongs to.
    pub variant_set: VariantSetId,
    /// The resolved branch.
    pub branch: BranchId,
    /// Branch content revision observed (authority pin — recorded for
    /// provenance, deliberately *not* part of recipe identity).
    pub branch_revision: BranchRevision,
    /// Captured payload digest; `None` for [`BranchContentRef::Live`].
    pub payload_digest: Option<ContentDigest>,
    /// Canonical (display-name-free) scope-content fingerprint — the
    /// render-reuse identity RCP-01 hashes into the recipe. Recomputed
    /// against the composed view for captured branches, against live
    /// for `Live` branches; deliberately *not* the CB-01
    /// `BranchContentRef::Captured.scope_digest` claim, which includes
    /// display names and remains an apply-side integrity check only.
    pub scope_digest: ContentDigest,
    /// Digest of the resolved *shared context*: every live semantic
    /// object outside the set's scope, canonicalized name-free. An
    /// upstream (non-scoped) edit moves this digest for every branch
    /// in the set; a scoped edit never does. Conservative by design —
    /// it covers all non-scope objects, not only render-reachable
    /// ones (over-invalidation is safe; under-invalidation is not).
    pub shared_context_digest: ContentDigest,
}

/// The effective project a render recipe resolves against, plus the
/// provenance of how it was composed.
#[derive(Clone, Debug)]
pub struct ResolvedRenderProject {
    /// Effective project: for a captured branch, live context with the
    /// scope-owned entities transplanted from the verified candidate.
    pub project: Project,
    /// `None` for live resolution; `Some` for a branch resolution.
    pub branch: Option<BranchRenderPin>,
}

// ---------------------------------------------------------------------------
// resolution
// ---------------------------------------------------------------------------

/// Resolves the effective project for a render input.
///
/// `live` is the project-authoritative state; `branch` optionally
/// selects a variant branch whose scope-owned entities replace the
/// live ones (shared context stays live — upstream edits propagate to
/// every branch, CB-01 semantics).
///
/// # Errors
/// [`RenderInputError::NotFound`] for unknown set/branch,
/// [`RenderInputError::Corrupt`] for digest/schema/reference
/// verification failures, [`RenderInputError::Unsupported`] for
/// content the apply surface cannot materialize, and
/// [`RenderInputError::InvalidProject`] when the composed view fails
/// `validate_references`.
pub fn resolve_render_project(
    live: &Project,
    branch: Option<RenderBranchTarget<'_>>,
) -> Result<ResolvedRenderProject, RenderInputError> {
    let Some(target) = branch else {
        // The live view is still a render input: it must satisfy the
        // same reference oracle a composed branch view does — a caller
        // holding an invalid in-memory project gets a typed error,
        // never a recipe over corrupt state.
        live.validate_references()?;
        return Ok(ResolvedRenderProject {
            project: live.clone(),
            branch: None,
        });
    };

    let set =
        live.variant_sets
            .get(&target.variant_set)
            .ok_or_else(|| RenderInputError::NotFound {
                detail: format!("variant set {}", target.variant_set),
            })?;
    let branch_ref =
        set.branches
            .get(&target.branch)
            .ok_or_else(|| RenderInputError::NotFound {
                detail: format!("branch {}", target.branch),
            })?;

    // Scope membership defines the branch/shared boundary. Both
    // fingerprints below use the *canonical* (display-name-free,
    // non-finite-rejecting) projection — the CB-01 `scope_digest`
    // stored on `BranchContentRef` is an apply-side integrity claim
    // and stays confined to payload verification.
    let scope_objects = crate::variant::scope::scope_objects(live, &set.scope)?;
    let shared_context_digest =
        canonical_fingerprint_except(live, &all_semantic_objects(live), &scope_objects)?;

    let pin =
        |scope_digest: ContentDigest, payload_digest: Option<ContentDigest>| BranchRenderPin {
            variant_set: target.variant_set,
            branch: target.branch,
            branch_revision: branch_ref.content.revision(),
            payload_digest,
            scope_digest,
            shared_context_digest,
        };

    match &branch_ref.content {
        BranchContentRef::Live { .. } => {
            // The adopted branch mirrors the live scope by construction;
            // recompute the canonical fingerprint rather than trusting
            // any persisted pin.
            let scope_digest = canonical_fingerprint(live, &scope_objects)?;
            Ok(ResolvedRenderProject {
                project: live.clone(),
                branch: Some(pin(scope_digest, None)),
            })
        }
        BranchContentRef::Captured {
            payload_digest,
            scope_digest: declared_scope_digest,
            ..
        } => {
            // decode_candidate re-verifies payload digest, schema
            // version and reference integrity — fail closed on drift.
            let candidate = decode_candidate(target.store, &branch_ref.content)?;
            // The persisted scope digest is a derived cache: recompute
            // and compare; a mismatch means a corrupt index, never a
            // valid shortcut.
            let scope_digest = crate::variant::scope::scope_digest(&candidate, &set.scope)?;
            if scope_digest != *declared_scope_digest {
                return Err(RenderInputError::Corrupt {
                    detail: format!(
                        "branch {} scope digest mismatch: declared {declared_scope_digest}, computed {scope_digest}",
                        target.branch
                    ),
                });
            }
            // Compose the effective view with the same transplant plan
            // adopt would run — scoped candidate entities over the live
            // shared context.
            let writes = crate::variant::engine::plan_adopt_writes(
                live, &candidate, set, branch_ref, &set.scope,
            )?;
            let mut composed = live.clone();
            entity::write_all(
                &mut composed,
                &candidate,
                &writes,
                &mut CounterIdGenerator::new(0),
            )?;
            composed.validate_references()?;
            // Canonical scope fingerprint over the composed view —
            // identical acoustic scope content yields identical
            // digests across branches regardless of display names.
            let composed_scope = crate::variant::scope::scope_objects(&composed, &set.scope)?;
            let scope_digest = canonical_fingerprint(&composed, &composed_scope)?;
            Ok(ResolvedRenderProject {
                project: composed,
                branch: Some(pin(scope_digest, Some(*payload_digest))),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// canonical entity projection
// ---------------------------------------------------------------------------

fn object(project_id: ProjectId, kind: ObjectKind, stable_id: String) -> SemanticObjectRef {
    SemanticObjectRef {
        project_id,
        kind,
        stable_id,
        lineage: None,
    }
}

macro_rules! ins_map {
    ($out:expr, $pid:expr, $kind:expr, $map:expr) => {
        for id in $map.keys() {
            $out.insert(object($pid, $kind, id.to_tagged()));
        }
    };
}

fn arrangement_objects(project: &Project, out: &mut BTreeSet<SemanticObjectRef>) {
    let pid = project.project_id;
    ins_map!(out, pid, ObjectKind::Track, project.tracks);
    ins_map!(out, pid, ObjectKind::Lane, project.lanes);
    ins_map!(out, pid, ObjectKind::Clip, project.clips);
    ins_map!(out, pid, ObjectKind::Note, project.notes);
    ins_map!(out, pid, ObjectKind::SmfDocument, project.smf_documents);
}

fn media_objects(project: &Project, out: &mut BTreeSet<SemanticObjectRef>) {
    let pid = project.project_id;
    ins_map!(out, pid, ObjectKind::Asset, project.assets);
    ins_map!(out, pid, ObjectKind::MediaSource, project.sources);
    ins_map!(out, pid, ObjectKind::MidiSource, project.midi_sources);
    for source in project.midi_sources.values() {
        ins_map!(out, pid, ObjectKind::MidiNote, source.notes);
    }
    ins_map!(out, pid, ObjectKind::ClockEpoch, project.clock_epochs);
}

fn recording_objects(project: &Project, out: &mut BTreeSet<SemanticObjectRef>) {
    let pid = project.project_id;
    ins_map!(
        out,
        pid,
        ObjectKind::RecordingSession,
        project.recording.sessions
    );
    ins_map!(
        out,
        pid,
        ObjectKind::RecordingPass,
        project.recording.passes
    );
    ins_map!(
        out,
        pid,
        ObjectKind::CaptureStream,
        project.recording.streams
    );
    ins_map!(
        out,
        pid,
        ObjectKind::CaptureSegment,
        project.recording.segments
    );
    ins_map!(out, pid, ObjectKind::Take, project.recording.takes);
    ins_map!(
        out,
        pid,
        ObjectKind::AppliedCommit,
        project.recording.applied_commits
    );
    ins_map!(out, pid, ObjectKind::AppliedImport, project.applied_imports);
}

fn comp_objects(project: &Project, out: &mut BTreeSet<SemanticObjectRef>) {
    let pid = project.project_id;
    ins_map!(out, pid, ObjectKind::TakeGroup, project.comp.take_groups);
    ins_map!(out, pid, ObjectKind::Comp, project.comp.comps);
    ins_map!(out, pid, ObjectKind::CompRegion, project.comp.regions);
    ins_map!(
        out,
        pid,
        ObjectKind::CompTransition,
        project.comp.transitions
    );
}

fn control_objects(project: &Project, out: &mut BTreeSet<SemanticObjectRef>) {
    let pid = project.project_id;
    ins_map!(
        out,
        pid,
        ObjectKind::ParameterTarget,
        project.control.targets
    );
    ins_map!(out, pid, ObjectKind::AutomationLane, project.control.lanes);
    ins_map!(
        out,
        pid,
        ObjectKind::ModulationSource,
        project.control.sources
    );
    ins_map!(
        out,
        pid,
        ObjectKind::ModulationAssignment,
        project.control.assignments
    );
}

fn routing_objects(project: &Project, out: &mut BTreeSet<SemanticObjectRef>) {
    let pid = project.project_id;
    ins_map!(out, pid, ObjectKind::Bus, project.routing.buses);
    ins_map!(out, pid, ObjectKind::Strip, project.routing.strips);
    ins_map!(out, pid, ObjectKind::Processor, project.routing.processors);
    ins_map!(out, pid, ObjectKind::ProcessorChain, project.routing.chains);
    ins_map!(out, pid, ObjectKind::SemanticPort, project.routing.ports);
    ins_map!(
        out,
        pid,
        ObjectKind::RoutingConnection,
        project.routing.connections
    );
    ins_map!(out, pid, ObjectKind::SendConfig, project.routing.sends);
}

fn misc_objects(project: &Project, out: &mut BTreeSet<SemanticObjectRef>) {
    let pid = project.project_id;
    ins_map!(
        out,
        pid,
        ObjectKind::TrackMonitorDecl,
        project.monitoring.tracks
    );
    ins_map!(
        out,
        pid,
        ObjectKind::PluginInstance,
        project.plugin_instances.instances
    );
}

/// Every semantic object in `project`, in deterministic
/// `(kind, stable_id)` order — the same identity conventions the VCS-02
/// diff engine uses (singletons keyed by the project tag, keyed-
/// synthetic collections by their borrowed ID space).
#[must_use]
pub fn all_semantic_objects(project: &Project) -> BTreeSet<SemanticObjectRef> {
    let pid = project.project_id;
    let mut out = BTreeSet::new();
    macro_rules! ins {
        ($kind:expr, $id:expr) => {
            out.insert(object(pid, $kind, $id.to_tagged()))
        };
    }
    ins!(ObjectKind::Project, pid);
    ins!(ObjectKind::RoutingModel, pid);
    if project.monitoring.punch.punch_in.is_some() || project.monitoring.punch.punch_out.is_some() {
        ins!(ObjectKind::PunchMarks, pid);
    }
    arrangement_objects(project, &mut out);
    media_objects(project, &mut out);
    recording_objects(project, &mut out);
    comp_objects(project, &mut out);
    control_objects(project, &mut out);
    routing_objects(project, &mut out);
    misc_objects(project, &mut out);
    out
}

/// `serde_json::Formatter` that rejects non-finite floats instead of
/// letting `serde_json` silently emit `null` (which would normalize
/// NaN/±∞ into a valid-looking value — forbidden by the RCP-01
/// contract). All other output is the compact default.
struct FiniteFormatter;

impl serde_json::ser::Formatter for FiniteFormatter {
    fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if value.is_finite() {
            serde_json::ser::CompactFormatter.write_f64(writer, value)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "non-finite f64 in canonical entity value",
            ))
        }
    }

    fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if value.is_finite() {
            serde_json::ser::CompactFormatter.write_f32(writer, value)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "non-finite f32 in canonical entity value",
            ))
        }
    }
}

/// Serializes `entity` to canonical JSON *verifying* every float is
/// finite, then parses back to a [`Value`] so display-name stripping
/// and re-serialization stay in one deterministic representation.
fn canonical_json<T: Serialize>(entity: &T) -> Result<Value, RenderInputError> {
    let mut buf = Vec::new();
    {
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, FiniteFormatter);
        entity
            .serialize(&mut ser)
            .map_err(|e| RenderInputError::Canonical {
                detail: e.to_string(),
            })?;
    }
    serde_json::from_slice(&buf).map_err(|e| RenderInputError::Canonical {
        detail: e.to_string(),
    })
}

/// Removes the top-level display `name` field — the only name class
/// the RCP-01 contract excludes (UI rename must not move a recipe
/// key). Nested names (e.g. SMF meta) are content and stay.
fn strip_display_name(value: &mut Value) {
    if let Value::Object(map) = value {
        map.remove("name");
    }
}

macro_rules! keyed_value {
    ($map:expr, $ty:ty, $stable_id:expr) => {{
        let id =
            <$ty>::from_tagged(&$stable_id.stable_id).map_err(|e| RenderInputError::Canonical {
                detail: format!("{}: {e}", $stable_id.stable_id),
            })?;
        $map.get(&id).map(canonical_json).transpose()?
    }};
}

/// Canonical value for the kinds that are not plain keyed-map
/// lookups: project/routing singletons, conditional `PunchMarks`,
/// and `MidiNote` (nested inside `midi_sources`, not a top-level
/// map). Callers must only pass these kinds.
fn canonical_special_entity_value(
    project: &Project,
    object: &SemanticObjectRef,
) -> Result<Option<Value>, RenderInputError> {
    Ok(match object.kind {
        ObjectKind::Project => Some(canonical_json(&serde_json::json!({
            "project_id": project.project_id,
            "ppqn": project.ppqn,
            "tempo_map": project.tempo_map,
        }))?),
        ObjectKind::RoutingModel => Some(canonical_json(&serde_json::json!({
            "master": project.routing.master,
        }))?),
        ObjectKind::PunchMarks => {
            let marks = &project.monitoring.punch;
            if marks.punch_in.is_some() || marks.punch_out.is_some() {
                Some(canonical_json(marks)?)
            } else {
                None
            }
        }
        ObjectKind::MidiNote => {
            let id = NoteId::from_tagged(&object.stable_id).map_err(|e| {
                RenderInputError::Canonical {
                    detail: format!("{}: {e}", object.stable_id),
                }
            })?;
            project
                .midi_sources
                .values()
                .find_map(|s| s.notes.get(&id))
                .map(canonical_json)
                .transpose()?
        }
        _ => unreachable!("canonical_special_entity_value: non-special kind"),
    })
}

/// Canonical value for every keyed-map [`ObjectKind`] — resolved via
/// [`keyed_value!`]. Callers must only pass non-special kinds.
fn canonical_keyed_entity_value(
    project: &Project,
    object: &SemanticObjectRef,
) -> Result<Option<Value>, RenderInputError> {
    Ok(match object.kind {
        ObjectKind::Track => keyed_value!(project.tracks, TrackId, object),
        ObjectKind::Lane => keyed_value!(project.lanes, LaneId, object),
        ObjectKind::Clip => keyed_value!(project.clips, ClipId, object),
        ObjectKind::Note => keyed_value!(project.notes, NoteId, object),
        ObjectKind::Asset => keyed_value!(project.assets, AssetId, object),
        ObjectKind::MediaSource => keyed_value!(project.sources, SourceId, object),
        ObjectKind::MidiSource => keyed_value!(project.midi_sources, SourceId, object),
        ObjectKind::SmfDocument => keyed_value!(project.smf_documents, SourceId, object),
        ObjectKind::ClockEpoch => keyed_value!(project.clock_epochs, ClockEpochId, object),
        ObjectKind::RecordingSession => {
            keyed_value!(project.recording.sessions, RecordingSessionId, object)
        }
        ObjectKind::RecordingPass => {
            keyed_value!(project.recording.passes, RecordingPassId, object)
        }
        ObjectKind::CaptureStream => {
            keyed_value!(project.recording.streams, CaptureStreamId, object)
        }
        ObjectKind::CaptureSegment => {
            keyed_value!(project.recording.segments, CaptureSegmentId, object)
        }
        ObjectKind::Take => keyed_value!(project.recording.takes, TakeId, object),
        ObjectKind::AppliedCommit => {
            keyed_value!(
                project.recording.applied_commits,
                RecordingSessionId,
                object
            )
        }
        ObjectKind::AppliedImport => {
            keyed_value!(project.applied_imports, ImportId, object)
        }
        ObjectKind::TakeGroup => keyed_value!(project.comp.take_groups, TakeGroupId, object),
        ObjectKind::Comp => keyed_value!(project.comp.comps, CompId, object),
        ObjectKind::CompRegion => keyed_value!(project.comp.regions, CompRegionId, object),
        ObjectKind::CompTransition => {
            keyed_value!(project.comp.transitions, CompTransitionId, object)
        }
        ObjectKind::ParameterTarget => {
            keyed_value!(project.control.targets, ParameterTargetId, object)
        }
        ObjectKind::AutomationLane => {
            keyed_value!(project.control.lanes, AutomationLaneId, object)
        }
        ObjectKind::ModulationSource => {
            keyed_value!(project.control.sources, ModulationSourceId, object)
        }
        ObjectKind::ModulationAssignment => {
            keyed_value!(project.control.assignments, ModulationAssignmentId, object)
        }
        ObjectKind::Bus => keyed_value!(project.routing.buses, BusId, object),
        ObjectKind::Strip => keyed_value!(project.routing.strips, StripId, object),
        ObjectKind::Processor => keyed_value!(project.routing.processors, ProcessorId, object),
        ObjectKind::ProcessorChain => {
            keyed_value!(project.routing.chains, ChainId, object)
        }
        ObjectKind::SemanticPort => {
            keyed_value!(project.routing.ports, SemanticPortId, object)
        }
        ObjectKind::RoutingConnection => {
            keyed_value!(project.routing.connections, RoutingConnectionId, object)
        }
        ObjectKind::SendConfig => keyed_value!(project.routing.sends, SendId, object),
        ObjectKind::TrackMonitorDecl => {
            keyed_value!(project.monitoring.tracks, TrackId, object)
        }
        ObjectKind::PluginInstance => {
            keyed_value!(project.plugin_instances.instances, ProcessorId, object)
        }
        ObjectKind::Project
        | ObjectKind::RoutingModel
        | ObjectKind::PunchMarks
        | ObjectKind::MidiNote => {
            unreachable!("canonical_keyed_entity_value: special kind")
        }
    })
}

/// The canonical, display-name-free JSON view of one semantic object
/// inside `project`. `Ok(None)` = the kind is known but the entity is
/// absent (or, for `PunchMarks`, undeclared). Every variant of
/// [`ObjectKind`] is handled — an unknown future kind is a compile
/// error here, never a silent skip.
///
/// # Errors
/// [`RenderInputError::Canonical`] on unparseable stable IDs,
/// serialization failure, or non-finite floats.
pub fn canonical_entity_value(
    project: &Project,
    object: &SemanticObjectRef,
) -> Result<Option<Value>, RenderInputError> {
    let special = matches!(
        object.kind,
        ObjectKind::Project
            | ObjectKind::RoutingModel
            | ObjectKind::PunchMarks
            | ObjectKind::MidiNote
    );
    let mut value = if special {
        canonical_special_entity_value(project, object)?
    } else {
        canonical_keyed_entity_value(project, object)?
    };
    if let Some(v) = &mut value {
        strip_display_name(v);
    }
    Ok(value)
}

/// Canonical bytes of one entity: `serde_json` of the name-stripped
/// value. Deterministic — `Value` objects are `BTreeMap`-ordered and
/// floats round-trip through ryu shortest form (including `-0.0`).
///
/// # Errors
/// [`RenderInputError::Canonical`] when the entity is absent.
pub fn canonical_entity_bytes(
    project: &Project,
    object: &SemanticObjectRef,
) -> Result<Vec<u8>, RenderInputError> {
    let value =
        canonical_entity_value(project, object)?.ok_or_else(|| RenderInputError::Canonical {
            detail: format!("entity {} absent", object.stable_id),
        })?;
    serde_json::to_vec(&value).map_err(|e| RenderInputError::Canonical {
        detail: e.to_string(),
    })
}

/// SHA-256 content digest of one entity's canonical bytes.
///
/// # Errors
/// [`RenderInputError::Canonical`] when the entity is absent.
pub fn canonical_entity_digest(
    project: &Project,
    object: &SemanticObjectRef,
) -> Result<ContentDigest, RenderInputError> {
    Ok(ContentDigest::sha256(&canonical_entity_bytes(
        project, object,
    )?))
}

/// Canonical fingerprint over `objects` — the same
/// `"kind|stable_id|json"` line format as
/// [`crate::apply::entity::fingerprint`], but over the name-stripped
/// canonical values so display renames cannot move the digest.
///
/// `excluded` objects are skipped (used for the shared-context
/// complement of a variant scope).
///
/// # Errors
/// [`RenderInputError::Canonical`] on extraction failure.
pub fn canonical_fingerprint(
    project: &Project,
    objects: &BTreeSet<SemanticObjectRef>,
) -> Result<ContentDigest, RenderInputError> {
    canonical_fingerprint_except(project, objects, &BTreeSet::new())
}

/// [`canonical_fingerprint`] skipping `excluded` objects.
///
/// # Errors
/// [`RenderInputError::Canonical`] on extraction failure.
pub fn canonical_fingerprint_except(
    project: &Project,
    objects: &BTreeSet<SemanticObjectRef>,
    excluded: &BTreeSet<SemanticObjectRef>,
) -> Result<ContentDigest, RenderInputError> {
    let mut parts = Vec::with_capacity(objects.len());
    for object in objects {
        if excluded.contains(object) {
            continue;
        }
        let value = canonical_entity_value(project, object)?;
        let kind =
            serde_json::to_string(&object.kind).map_err(|e| RenderInputError::Canonical {
                detail: e.to_string(),
            })?;
        let text = match &value {
            Some(v) => serde_json::to_string(v).map_err(|e| RenderInputError::Canonical {
                detail: e.to_string(),
            })?,
            None => "null".to_owned(),
        };
        parts.push(format!("{kind}|{}|{text}", object.stable_id));
    }
    Ok(ContentDigest::sha256(parts.join("\n").as_bytes()))
}
