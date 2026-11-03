//! GW-02 authorization tests — Issue #54 acceptance criteria.
//!
//! Each test names the security/data invariant it proves
//! (SEC-01..14 / DATA classes / AHG-006..010 / ACG-003 seam).
//! Rejections assert the exact deny reason — the authorization
//! contract only exists if every failure is classifiable and
//! existence-blind where required.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::fixture::{self, FixtureAdapter, StaticOracle};
use synvelis_gateway::*;
use synvelis_project::{ClipId, ProjectId, ProjectRevision, SourceId, TrackId};

// ── Fixture ids ──────────────────────────────────────────────

fn project() -> ProjectId {
    ProjectId::new(7)
}
fn other_project() -> ProjectId {
    ProjectId::new(8)
}
fn source9() -> ResourceRef {
    ResourceRef::Source(SourceId::new(9))
}
fn source10() -> ResourceRef {
    ResourceRef::Source(SourceId::new(10))
}
fn track3() -> ResourceRef {
    ResourceRef::Track(TrackId::new(3))
}
fn track5() -> ResourceRef {
    ResourceRef::Track(TrackId::new(5))
}
fn clip11() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(11))
}
fn clip12() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(12))
}
fn clip13() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(13))
}
fn project_ref() -> ResourceRef {
    ResourceRef::Project(project())
}

fn task_ref(tag: &str) -> TaskRef {
    TaskRef::new(tag).unwrap()
}
fn grant_ref(tag: &str) -> GrantRef {
    GrantRef::new(tag).unwrap()
}
fn permit_ref(tag: &str) -> ProactivePermitRef {
    ProactivePermitRef::new(tag).unwrap()
}
fn cap_ref(id: &str, major: u32, minor: u32) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id).unwrap(),
        version: CapabilityVersion::new(major, minor),
    }
}

// ── Callers / principals ─────────────────────────────────────

fn adapter() -> FixtureAdapter {
    FixtureAdapter::for_testing()
}
fn agent_principal() -> Principal {
    Principal::new(PrincipalKind::Agent, "mcp-client-1").unwrap()
}
fn other_principal() -> Principal {
    Principal::new(PrincipalKind::Agent, "mcp-client-2").unwrap()
}
fn human_principal() -> Principal {
    Principal::new(PrincipalKind::Human, "user-1").unwrap()
}
fn agent() -> CallerIdentity {
    adapter().agent("mcp-client-1").unwrap()
}
fn other_agent() -> CallerIdentity {
    adapter().agent("mcp-client-2").unwrap()
}
fn human() -> CallerIdentity {
    adapter().human("user-1").unwrap()
}

// ── Policy rig ───────────────────────────────────────────────

struct Rig {
    store: Arc<InMemoryPolicyStore>,
    clock: Arc<ManualClock>,
    /// A policy evaluator over the rig's store/clock/oracle — used for
    /// direct `decide`/`evaluate_*` calls.
    policy: GrantPolicy,
    /// Fixture gateway sharing the same store/clock/oracle — used for
    /// `bind` (validation) and `invoke` (stage-mapped evaluate+run).
    gw: Gateway,
}

/// A rig whose oracle knows the standard resource set: the project,
/// sources 9/10, and clips 11/12/13 owned by track3 inside 0..960.
fn rig() -> Rig {
    rig_with(standard_facts)
}

/// A rig whose oracle is populated by `build` — lets a test choose
/// exactly which facts exist.
fn rig_with(build: impl FnOnce(&mut StaticOracle)) -> Rig {
    let store = Arc::new(InMemoryPolicyStore::new());
    let clock = Arc::new(ManualClock::new(PolicyInstant::new(1_000)));
    let mut oracle = StaticOracle::new();
    build(&mut oracle);
    let oracle = Arc::new(oracle);
    let policy = GrantPolicy::new(store.clone(), clock.clone(), oracle.clone());
    let mut gw = Gateway::with_evaluator(Box::new(GrantPolicy::new(
        store.clone(),
        clock.clone(),
        oracle,
    )));
    fixture::install(&mut gw).unwrap();
    install_test_capabilities(&mut gw);
    Rig {
        store,
        clock,
        policy,
        gw,
    }
}

fn standard_facts(oracle: &mut StaticOracle) {
    oracle.known(
        project(),
        project_ref(),
        attrs(ResourceKind::Project, None, None, Some(0)),
    );
    for s in [source9(), source10()] {
        oracle.known(
            project(),
            s,
            attrs(ResourceKind::Source, Some(project_ref()), None, Some(1)),
        );
    }
    for clip in [clip11(), clip12(), clip13()] {
        oracle.known(
            project(),
            clip,
            attrs(
                ResourceKind::Clip,
                Some(track3()),
                Some(musical(120, 240)),
                Some(5),
            ),
        );
    }
}

/// Registers the test-only capabilities the fixture registry lacks:
/// a clip-targeted mutating op, a clip-set op, a read-only clip query,
/// and a second version of `synvelis.midi.add-note`, sharing one input
/// schema that carries move/reparent destination fields.
fn install_test_capabilities(gw: &mut Gateway) {
    register_clip_ops_schemas(gw);
    register_clip_ops_capabilities(gw);
}

/// The shared clip-ops input/output schemas — `dest_parent` /
/// `dest_range` carry typed move destinations, `dest_raw` lets a test
/// put an unparseable value into a declared field.
fn register_clip_ops_schemas(gw: &mut Gateway) {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "dest_parent".to_owned(),
        FieldSchema::optional(ValueSchema::Id {
            kind: ResourceKind::Track,
        }),
    );
    fields.insert(
        "dest_range".to_owned(),
        FieldSchema::optional(ValueSchema::TimeRange {
            domain: TimeDomain::ProjectMusical,
        }),
    );
    fields.insert(
        "dest_raw".to_owned(),
        FieldSchema::optional(ValueSchema::String { max_len: None }),
    );
    gw.schemas_mut()
        .register(
            "test.clip-ops.input",
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields,
                deny_unknown: true,
            },
        )
        .unwrap();
    gw.schemas_mut()
        .register(
            "test.clip-ops.output",
            SchemaVersion::new(1, 0),
            ValueSchema::Object {
                fields: std::collections::BTreeMap::new(),
                deny_unknown: false,
            },
        )
        .unwrap();
}

/// The clip-ops descriptors plus a second `add-note` version (for
/// exact-version checks) — all share `test.clip-ops.input`.
fn register_clip_ops_capabilities(gw: &mut Gateway) {
    let descriptor = |id: &str,
                      domain: OperationDomain,
                      target: TargetContract,
                      bound: EffectSet,
                      permissions: PermissionSet| CapabilityDescriptor {
        id: CapabilityId::new(id).unwrap(),
        version: CapabilityVersion::new(1, 0),
        domain,
        input_schema: SchemaRef::new("test.clip-ops.input", 1, 0),
        output_schema: SchemaRef::new("test.clip-ops.output", 1, 0),
        target,
        effect_bound: bound,
        required_permissions: permissions,
        data_access: DataAccessSet::of([DataAccessClass::MetadataRead]),
        execution: ExecutionContract::Sync,
        availability: Availability::Available,
    };
    gw.register_capability(
        descriptor(
            "test.clip.edit",
            OperationDomain::ProjectMutation,
            TargetContract::Resource {
                kind: ResourceKind::Clip,
            },
            EffectSet::of([EffectKind::Read, EffectKind::Write]),
            PermissionSet::of([PermissionKind::MutateProject]),
        ),
        Some(Arc::new(fixture::EchoHandler)),
    )
    .unwrap();
    gw.register_capability(
        descriptor(
            "test.clip.edit-set",
            OperationDomain::ProjectMutation,
            TargetContract::ResourceSet {
                kind: ResourceKind::Clip,
            },
            EffectSet::of([EffectKind::Read, EffectKind::Write]),
            PermissionSet::of([PermissionKind::MutateProject]),
        ),
        Some(Arc::new(fixture::EchoHandler)),
    )
    .unwrap();
    // A read-only clip query — no mutating effect in its bound, so
    // preflight scope evaluation does not follow related resources.
    gw.register_capability(
        descriptor(
            "test.clip.read",
            OperationDomain::Query,
            TargetContract::Resource {
                kind: ResourceKind::Clip,
            },
            EffectSet::of([EffectKind::Read]),
            PermissionSet::of([PermissionKind::ReadContext]),
        ),
        Some(Arc::new(fixture::EchoHandler)),
    )
    .unwrap();
    // A second registered version of add-note — lets a bound request
    // name a capability ref outside a grant's exact version set.
    gw.register_capability(
        CapabilityDescriptor {
            id: CapabilityId::new("synvelis.midi.add-note").unwrap(),
            version: CapabilityVersion::new(1, 0),
            domain: OperationDomain::ProjectMutation,
            input_schema: SchemaRef::new("synvelis.midi.add-note.input", 1, 0),
            output_schema: SchemaRef::new("synvelis.midi.add-note.output", 1, 0),
            target: TargetContract::Resource {
                kind: ResourceKind::Source,
            },
            effect_bound: EffectSet::of([EffectKind::Read, EffectKind::Write]),
            required_permissions: PermissionSet::of([PermissionKind::MutateProject]),
            data_access: DataAccessSet::of([DataAccessClass::MetadataRead]),
            execution: ExecutionContract::Sync,
            availability: Availability::Available,
        },
        Some(Arc::new(fixture::EchoHandler)),
    )
    .unwrap();
}

// ── Store builders ───────────────────────────────────────────

fn issue_task(store: &InMemoryPolicyStore, tag: &str, principal: &Principal) {
    store.issue_task(AuthorizedTask {
        task: task_ref(tag),
        principal: principal.clone(),
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
}

fn issue_task_for_project(
    store: &InMemoryPolicyStore,
    tag: &str,
    principal: &Principal,
    p: ProjectId,
) {
    store.issue_task(AuthorizedTask {
        task: task_ref(tag),
        principal: principal.clone(),
        project: p,
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
}

/// A baseline delegated-apply grant bound to `task_tag`: agent
/// principal, this project, add-note capability, source-9 exact scope.
fn base_grant(tag: &str, task_tag: &str) -> Grant {
    Grant {
        id: grant_ref(tag),
        issued_by: human_principal(),
        principal: agent_principal(),
        project: project(),
        binding: GrantBinding::Task {
            task: task_ref(task_tag),
        },
        capabilities: BTreeSet::from([cap_ref("synvelis.midi.add-note", 1, 1)]),
        permissions: PermissionSet::of([PermissionKind::MutateProject]),
        data_access: DataAccessSet::of([DataAccessClass::MetadataRead]),
        scope: ResourceScope::Exact {
            resources: BTreeSet::from([source9()]),
        },
        create_within: None,
        approval_policy: ApprovalPolicy::DelegatedApply,
        limits: GrantLimits::default(),
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision: Some(ProjectRevision(10)),
    }
}

/// A task + matching delegated-apply grant for the agent caller.
fn issue_agent_authority(store: &InMemoryPolicyStore, task_tag: &str, grant_tag: &str) {
    issue_task(store, task_tag, &agent_principal());
    store.issue_grant(base_grant(grant_tag, task_tag));
}

// ── Request builders ─────────────────────────────────────────

fn add_note_req(task: &str, grant: &str) -> Value {
    json!({
        "request_id": "req-a1",
        "capability": {"id": "synvelis.midi.add-note", "version": {"major": 1, "minor": 1}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": project().to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 240, "len": 480}},
        "consistency": {"kind": "expected-revision", "revision": 10},
        "task": task,
        "grant": grant,
    })
}

fn add_note_req_on(task: &str, grant: &str, source: ResourceRef) -> Value {
    let mut req = add_note_req(task, grant);
    req["target"]["id"] = json!(tagged(source));
    req
}

fn context_req(task: &str, grant: &str) -> Value {
    json!({
        "request_id": "req-q1",
        "capability": {"id": "synvelis.context.source-notes", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.context.source-notes.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {"range": {"domain": "source-native", "start": 0, "len": 960}},
        "consistency": {"kind": "latest"},
        "task": task,
        "grant": grant,
    })
}

fn transport_req(task: &str, grant: &str) -> Value {
    json!({
        "request_id": "req-t1",
        "capability": {"id": "synvelis.transport.set-position", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.transport.set-position.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "args": {"action": "play"},
        "consistency": {"kind": "unconditional"},
        "task": task,
        "grant": grant,
    })
}

fn create_source_req(task: &str, grant: &str) -> Value {
    json!({
        "request_id": "req-c1",
        "capability": {"id": "synvelis.midi.create-source", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.midi.create-source.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "target": {"shape": "create", "parent_kind": "project", "parent": project().to_tagged(), "creates": "source"},
        "args": {"division": 480},
        "consistency": {"kind": "expected-revision", "revision": 10},
        "task": task,
        "grant": grant,
    })
}

fn clip_edit_req(task: &str, grant: &str, clip: ResourceRef, args: Value) -> Value {
    let mut req = json!({
        "request_id": "req-m1",
        "capability": {"id": "test.clip.edit", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "test.clip-ops.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "target": {"shape": "resource", "kind": "clip", "id": tagged(clip)},
        "consistency": {"kind": "expected-revision", "revision": 10},
        "task": task,
        "grant": grant,
    });
    req["args"] = args;
    req
}

fn clip_set_req(task: &str, grant: &str, clips: &[ResourceRef]) -> Value {
    json!({
        "request_id": "req-s1",
        "capability": {"id": "test.clip.edit-set", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "test.clip-ops.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "target": {"shape": "resource-set", "kind": "clip", "ids": clips.iter().map(|r| json!(tagged(*r))).collect::<Vec<_>>()},
        "args": {},
        "consistency": {"kind": "expected-revision", "revision": 10},
        "task": task,
        "grant": grant,
    })
}

fn tagged(r: ResourceRef) -> String {
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

/// Binds a wire request as the given caller.
#[allow(clippy::result_large_err)]
fn bind(
    rig: &mut Rig,
    req: &Value,
    caller: CallerIdentity,
) -> Result<OperationRequest, ErrorEnvelope> {
    let wire = Gateway::parse_wire(req).unwrap();
    rig.gw.bind(wire, caller)
}

fn bind_req(rig: &mut Rig, req: &Value, caller: CallerIdentity) -> OperationRequest {
    bind(rig, req, caller).unwrap()
}

fn descriptor_of(rig: &Rig, id: &str, major: u32, minor: u32) -> CapabilityDescriptor {
    rig.gw
        .registry()
        .describe(&cap_ref(id, major, minor))
        .unwrap()
        .clone()
}

fn decide(
    rig: &Rig,
    req: &OperationRequest,
    stage: AuthorizationStage,
    id: &str,
    major: u32,
    minor: u32,
) -> AuthorizationDecision {
    rig.policy
        .decide(req, stage, &descriptor_of(rig, id, major, minor))
}

fn deny_reason(decision: &AuthorizationDecision) -> &str {
    match decision {
        AuthorizationDecision::Deny { reason } => reason,
        other => panic!("expected deny, got {other:?}"),
    }
}

// ── Oracle attributes ────────────────────────────────────────

fn attrs(
    kind: ResourceKind,
    owner: Option<ResourceRef>,
    range: Option<TypedRange>,
    created: Option<u64>,
) -> ResourceAttributes {
    ResourceAttributes {
        kind,
        owner,
        range,
        created_revision: created.map(ProjectRevision),
        related: vec![],
    }
}

fn musical(start: i64, len: i64) -> TypedRange {
    TypedRange {
        domain: TimeDomain::ProjectMusical,
        start,
        len,
    }
}

const NO_AUTH: &str = "no applicable authorization";
const OUTSIDE_SCOPE: &str = "target outside grant scope";

// ════════════════════════════════════════════════════════════
// SEC-01: unauthenticated callers — default deny
// ════════════════════════════════════════════════════════════

#[test]
fn sec01_agent_without_any_authority_denied() {
    let mut rig = rig();
    let mut req = add_note_req("task-a", "grant-a");
    // Strip both references: an unauthenticated-shaped agent call.
    req.as_object_mut().unwrap().remove("task");
    req.as_object_mut().unwrap().remove("grant");
    let bound = bind_req(&mut rig, &req, agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn sec01_human_needs_no_grant() {
    // Manual authority is distinct from agent grant authority
    // (AHG-007): a trusted human never carries task/grant refs.
    let mut rig = rig();
    let mut req = add_note_req("x", "y");
    req.as_object_mut().unwrap().remove("task");
    req.as_object_mut().unwrap().remove("grant");
    let bound = bind_req(&mut rig, &req, human());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn sec01_grant_ref_alone_is_not_proof() {
    // A grant string on the wire is a lookup key, not authority:
    // no task → no authorization.
    let mut rig = rig();
    rig.store.issue_grant(base_grant("grant-a", "task-a"));
    let mut req = add_note_req("task-a", "grant-a");
    req.as_object_mut().unwrap().remove("task");
    let bound = bind_req(&mut rig, &req, agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

// ════════════════════════════════════════════════════════════
// SEC-02/03/04: binding — principal, project, existence
// ════════════════════════════════════════════════════════════

#[test]
fn sec02_authority_issued_for_another_principal_denied() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), other_agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn sec03_authority_for_another_project_denied() {
    let mut rig = rig();
    issue_task_for_project(&rig.store, "task-a", &agent_principal(), other_project());
    let mut grant = base_grant("grant-a", "task-a");
    grant.project = other_project();
    rig.store.issue_grant(grant);
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn sec04_unknown_grant_ref_denied() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    // "grant-ghost" is never issued.
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-ghost"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn sec04_unknown_task_ref_denied() {
    let mut rig = rig();
    rig.store.issue_grant(base_grant("grant-a", "task-ghost"));
    let bound = bind_req(&mut rig, &add_note_req("task-ghost", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn sec04_no_existence_leak_between_deny_reasons() {
    // Probing cannot distinguish "grant exists but belongs to someone
    // else" from "grant does not exist" — one reason for both (AC:
    // unauthorized existence is not leaked).
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(base_grant("grant-theirs", "task-a"));

    let missing = bind_req(&mut rig, &add_note_req("task-a", "grant-ghost"), agent());
    let not_mine = bind_req(
        &mut rig,
        &add_note_req("task-a", "grant-theirs"),
        other_agent(),
    );
    let d_missing = decide(
        &rig,
        &missing,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    let d_not_mine = decide(
        &rig,
        &not_mine,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d_missing, d_not_mine);
    assert_eq!(deny_reason(&d_missing), NO_AUTH);
}

#[test]
fn sec04_binding_mismatch_denied() {
    // The grant is the agent's own but bound to a different task —
    // no silent widening across tasks (AHG-010).
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    issue_task(&rig.store, "task-b", &agent_principal());
    rig.store.issue_grant(base_grant("grant-a", "task-b"));
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

// ════════════════════════════════════════════════════════════
// SEC-07/08: expiry, revocation, epoch
// ════════════════════════════════════════════════════════════

#[test]
fn sec07_expired_grant_denied() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    // Grant expires at 1000; clock is at 1000 → still live (expiry
    // instant is inclusive). Advance past it → expired.
    let mut grant = base_grant("grant-b", "task-a");
    grant.validity.expires_at = Some(PolicyInstant::new(1_000));
    rig.store.issue_grant(grant);

    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-b"), agent());
    let at_expiry = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(
        at_expiry,
        AuthorizationDecision::PermitForStage,
        "expiry instant is inclusive"
    );

    rig.clock.set(PolicyInstant::new(1_001));
    let expired = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&expired), "grant expired");
}

#[test]
fn sec08_revoked_grant_denied() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    rig.store.revoke_grant(&grant_ref("grant-a"));
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), "grant revoked");
}

#[test]
fn sec08_revoked_task_denied() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    rig.store.revoke_task(&task_ref("task-a"));
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn sec08_revoked_task_denies_like_missing_task() {
    // A revoked task denies uniformly with an unknown task — task
    // state is not disclosed before grant binding.
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    rig.store.revoke_task(&task_ref("task-a"));

    let revoked = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let unknown = bind_req(&mut rig, &add_note_req("task-ghost", "grant-a"), agent());
    let d_revoked = decide(
        &rig,
        &revoked,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    let d_unknown = decide(
        &rig,
        &unknown,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d_revoked, d_unknown);
}

#[test]
fn sec07_expired_task_denied() {
    let mut rig = rig();
    // The task's expiry already passed at the evaluation instant.
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("task-a"),
        principal: agent_principal(),
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity {
            expires_at: Some(PolicyInstant::new(500)),
            revoked: false,
        },
    });
    rig.store.issue_grant(base_grant("grant-a", "task-a"));
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn sec12_policy_epoch_supersedes_grant() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let mut grant = base_grant("grant-e", "task-a");
    grant.epoch_ceiling = Some(PolicyEpoch(0));
    rig.store.issue_grant(grant);

    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-e"), agent());
    let ok = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(
        ok,
        AuthorizationDecision::PermitForStage,
        "epoch 0 grant at epoch 0 is live"
    );

    rig.store.update_policy();
    let stale = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&stale), "policy version superseded");
}

#[test]
fn sec12_grant_without_ceiling_survives_policy_update() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    rig.store.update_policy();
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

// ════════════════════════════════════════════════════════════
// SEC-13: capability / permission / data-access dimensions
// ════════════════════════════════════════════════════════════

#[test]
fn sec13_capability_outside_grant_set_denied() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.capabilities = BTreeSet::from([cap_ref("synvelis.context.source-notes", 1, 0)]);
    rig.store.issue_grant(grant);
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), "capability outside grant");
}

#[test]
fn sec13_capability_version_is_exact() {
    // The grant names add-note@1.1 exactly; the registered 1.0 ref —
    // same id, different version — is outside the set.
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let mut req = add_note_req("task-a", "grant-a");
    req["capability"]["version"] = json!({"major": 1, "minor": 0});
    req["schema"]["version"] = json!({"major": 1, "minor": 0});
    let bound = bind_req(&mut rig, &req, agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), "capability outside grant");
}

#[test]
fn sec13_capability_descriptor_mismatch_denied() {
    // The evaluator verifies the passed descriptor matches the
    // request's capability ref — a caller cannot soften the declared
    // bounds by handing in a descriptor for a different version.
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    // The request names add-note@1.1; the 1.0 descriptor is for a
    // different capability ref.
    let wrong = descriptor_of(&rig, "synvelis.midi.add-note", 1, 0);
    let d = rig
        .policy
        .decide(&bound, AuthorizationStage::RequestApply, &wrong);
    assert_eq!(deny_reason(&d), "capability descriptor mismatch");
}

#[test]
fn sec13_required_permissions_outside_grant_denied() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.permissions = PermissionSet::of([PermissionKind::ReadContext]);
    rig.store.issue_grant(grant);
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), "required permissions outside grant");
}

#[test]
fn data_access_class_subset_enforced() {
    // The context query declares metadata+media+disclosure; a grant
    // covering only metadata denies — classes do not collapse.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.capabilities = BTreeSet::from([cap_ref("synvelis.context.source-notes", 1, 0)]);
    grant.permissions =
        PermissionSet::of([PermissionKind::ReadContext, PermissionKind::DiscloseContext]);
    grant.data_access = DataAccessSet::of([DataAccessClass::MetadataRead]);
    rig.store.issue_grant(grant);
    let bound = bind_req(&mut rig, &context_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::ReadContext,
        "synvelis.context.source-notes",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), "data access outside grant");
}

#[test]
fn data_access_full_set_permits() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.capabilities = BTreeSet::from([cap_ref("synvelis.context.source-notes", 1, 0)]);
    grant.permissions =
        PermissionSet::of([PermissionKind::ReadContext, PermissionKind::DiscloseContext]);
    grant.data_access = DataAccessSet::of([
        DataAccessClass::MetadataRead,
        DataAccessClass::MediaBytesRead,
        DataAccessClass::ExternalDisclosure,
    ]);
    rig.store.issue_grant(grant);
    let bound = bind_req(&mut rig, &context_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::ReadContext,
        "synvelis.context.source-notes",
        1,
        0,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

// ════════════════════════════════════════════════════════════
// SEC-09: fixed scope — exact/snapshot
// ════════════════════════════════════════════════════════════

#[test]
fn sec09_exact_scope_permits_listed_member() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn sec09_exact_scope_denies_unlisted_and_unresolvable_alike() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");

    // source-10 is oracle-known but not in the scope set.
    let unlisted = bind_req(
        &mut rig,
        &add_note_req_on("task-a", "grant-a", source10()),
        agent(),
    );
    let d_unlisted = decide(
        &rig,
        &unlisted,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d_unlisted), OUTSIDE_SCOPE);

    // source-99 does not resolve at all: same verdict — the deny
    // reason never reveals which case applied (no existence leak).
    let ghost = bind_req(
        &mut rig,
        &add_note_req_on("task-a", "grant-a", ResourceRef::Source(SourceId::new(99))),
        agent(),
    );
    let d_ghost = decide(
        &rig,
        &ghost,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d_unlisted, d_ghost);
}

#[test]
fn sec09_snapshot_scope_behaves_like_exact() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.scope = ResourceScope::Snapshot {
        resources: BTreeSet::from([source9()]),
        captured_revision: ProjectRevision(10),
    };
    rig.store.issue_grant(grant);
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

// ════════════════════════════════════════════════════════════
// SEC-10: dynamic scope — constrained membership
// ════════════════════════════════════════════════════════════

fn dynamic_grant(tag: &str, task_tag: &str, scope: DynamicScope) -> Grant {
    let mut grant = base_grant(tag, task_tag);
    grant.capabilities = BTreeSet::from([
        cap_ref("test.clip.edit", 1, 0),
        cap_ref("test.clip.edit-set", 1, 0),
        cap_ref("test.clip.read", 1, 0),
    ]);
    grant.permissions =
        PermissionSet::of([PermissionKind::MutateProject, PermissionKind::ReadContext]);
    grant.scope = ResourceScope::Dynamic(scope);
    grant
}

fn dyn_scope() -> DynamicScope {
    DynamicScope {
        owners: OwnerConstraint::OneOf {
            refs: BTreeSet::from([track3()]),
        },
        kinds: BTreeSet::from([ResourceKind::Clip]),
        range: Some(musical(0, 960)),
        max_members: None,
        include_future: false,
        move_args: None,
    }
}

#[test]
fn sec10_dynamic_member_covered() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn sec10_dynamic_wrong_kind_denied() {
    // The oracle reports clip11 as a Source — kind comes from facts,
    // not the ref tag. A member of the wrong kind is outside.
    let mut rig = rig_with(|o| {
        o.known(
            project(),
            clip11(),
            attrs(
                ResourceKind::Source,
                Some(track3()),
                Some(musical(120, 240)),
                Some(5),
            ),
        );
    });
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

#[test]
fn sec10_dynamic_wrong_owner_denied() {
    // clip11 owned by track5 — outside the OneOf{track3} constraint.
    let mut rig = rig_with(|o| {
        o.known(
            project(),
            clip11(),
            attrs(
                ResourceKind::Clip,
                Some(track5()),
                Some(musical(120, 240)),
                Some(5),
            ),
        );
    });
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

#[test]
fn sec10_dynamic_ownerless_denied() {
    // An ownerless resource under an owner-constrained scope cannot
    // satisfy the constraint.
    let mut rig = rig_with(|o| {
        o.known(
            project(),
            clip11(),
            attrs(ResourceKind::Clip, None, Some(musical(120, 240)), Some(5)),
        );
    });
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

#[test]
fn sec10_dynamic_range_outside_bound_denied() {
    // Clip placed at 900..1140 — extends past the 0..960 bound.
    let mut rig = rig_with(|o| {
        o.known(
            project(),
            clip11(),
            attrs(
                ResourceKind::Clip,
                Some(track3()),
                Some(musical(900, 240)),
                Some(5),
            ),
        );
    });
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

#[test]
fn sec10_dynamic_wrong_time_domain_denied() {
    // A member range in a different domain can never be contained —
    // no implicit unit conversion across domains (AHG-008).
    let mut rig = rig_with(|o| {
        o.known(
            project(),
            clip11(),
            attrs(
                ResourceKind::Clip,
                Some(track3()),
                Some(TypedRange {
                    domain: TimeDomain::SourceNative,
                    start: 120,
                    len: 240,
                }),
                Some(5),
            ),
        );
    });
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

#[test]
fn sec10_dynamic_future_member_requires_inclusion() {
    // clip11 created at revision 20 — after the grant's issuance
    // revision 10.
    let mut rig = rig_with(|o| {
        o.known(
            project(),
            clip11(),
            attrs(
                ResourceKind::Clip,
                Some(track3()),
                Some(musical(120, 240)),
                Some(20),
            ),
        );
    });
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(
        deny_reason(&d),
        OUTSIDE_SCOPE,
        "a post-issuance member is never silently covered (AHG-008)"
    );

    // include_future=true admits it.
    let mut scope = dyn_scope();
    scope.include_future = true;
    rig.store
        .issue_grant(dynamic_grant("grant-f", "task-a", scope));
    let bound_f = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-f", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound_f,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn sec10_dynamic_max_members_caps() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut scope = dyn_scope();
    scope.max_members = Some(2);
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", scope));
    let bound = bind_req(
        &mut rig,
        &clip_set_req("task-a", "grant-a", &[clip11(), clip12(), clip13()]),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit-set",
        1,
        0,
    );
    assert_eq!(
        deny_reason(&d),
        OUTSIDE_SCOPE,
        "third member exceeds max_members"
    );

    // Two members fit.
    let bound_two = bind_req(
        &mut rig,
        &clip_set_req("task-a", "grant-a", &[clip11(), clip12()]),
        agent(),
    );
    let d = decide(
        &rig,
        &bound_two,
        AuthorizationStage::RequestApply,
        "test.clip.edit-set",
        1,
        0,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn sec10_dynamic_max_members_spans_target_and_effects() {
    // The cap is per request: a member covered at preflight plus a
    // distinct member in the derived effects draw from one budget —
    // coverage is not reset between the two axes.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut scope = dyn_scope();
    scope.max_members = Some(1);
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", scope));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let desc = descriptor_of(&rig, "test.clip.edit", 1, 0);

    // clip11 counted at preflight; clip12 is a second distinct member
    // in the effects — the request exceeds the cap of 1.
    let effects = ScopedEffects {
        writes: vec![clip11(), clip12()],
        ..ScopedEffects::default()
    };
    let d = rig
        .policy
        .evaluate_effects(&bound, AuthorizationStage::RequestApply, &desc, &effects);
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);

    // The same member on both axes counts once — an effects set for
    // the target itself stays inside the cap.
    let effects = ScopedEffects {
        writes: vec![clip11()],
        ..ScopedEffects::default()
    };
    let d = rig
        .policy
        .evaluate_effects(&bound, AuthorizationStage::RequestApply, &desc, &effects);
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn sec10_dynamic_unknown_revision_treated_as_future() {
    // A member with no creation provenance cannot prove it predates
    // issuance — it is treated as future and needs include_future.
    let mut rig = rig_with(|o| {
        o.known(
            project(),
            clip11(),
            attrs(
                ResourceKind::Clip,
                Some(track3()),
                Some(musical(120, 240)),
                None,
            ),
        );
    });
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);

    // include_future=true covers the same unprovenanced member.
    let mut scope = dyn_scope();
    scope.include_future = true;
    rig.store
        .issue_grant(dynamic_grant("grant-f", "task-a", scope));
    let bound_f = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-f", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound_f,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

// ════════════════════════════════════════════════════════════
// SEC-05: shared source — no implicit authority through relations
// ════════════════════════════════════════════════════════════

fn clip_with_related_source_facts(oracle: &mut StaticOracle) {
    // clip11 (a covered clip member) structurally depends on source9.
    let mut clip_attrs = attrs(
        ResourceKind::Clip,
        Some(track3()),
        Some(musical(120, 240)),
        Some(5),
    );
    clip_attrs.related = vec![source9()];
    oracle.known(project(), clip11(), clip_attrs);
    // source9 is resolvable but outside the clip/kind/owner scope.
    oracle.known(
        project(),
        source9(),
        attrs(ResourceKind::Source, Some(project_ref()), None, Some(1)),
    );
}

#[test]
fn sec05_shared_source_not_inherited_through_clip() {
    // A clip in scope that *uses* a source does not carry source
    // permission: the mutating capability follows the clip's related
    // resources and each must be covered independently (AHG-008).
    let mut rig = rig_with(clip_with_related_source_facts);
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));

    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(
        deny_reason(&d),
        OUTSIDE_SCOPE,
        "clip permission never implies related-source permission"
    );
}

#[test]
fn sec05_read_only_capability_does_not_follow_related() {
    // The related traversal is bound to mutating effect bounds — a
    // read of the clip alone does not require source coverage.
    let mut rig = rig_with(clip_with_related_source_facts);
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", dyn_scope()));

    let req = json!({
        "request_id": "req-r1",
        "capability": {"id": "test.clip.read", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "test.clip-ops.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "target": {"shape": "resource", "kind": "clip", "id": tagged(clip11())},
        "args": {},
        "consistency": {"kind": "latest"},
        "task": "task-a",
        "grant": "grant-a",
    });
    let bound = bind_req(&mut rig, &req, agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::ReadContext,
        "test.clip.read",
        1,
        0,
    );
    assert_eq!(
        d,
        AuthorizationDecision::PermitForStage,
        "a read of the covered clip does not drag in its source"
    );
}

// ════════════════════════════════════════════════════════════
// SEC-06: move / reparent — after-state must stay inside scope
// ════════════════════════════════════════════════════════════

#[test]
fn sec06_move_inside_scope_permitted() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut scope = dyn_scope();
    scope.move_args = Some(MoveArgs {
        parent_arg: Some("dest_parent".to_owned()),
        range_arg: None,
    });
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", scope));

    let bound = bind_req(
        &mut rig,
        &clip_edit_req(
            "task-a",
            "grant-a",
            clip11(),
            json!({"dest_parent": TrackId::new(3).to_tagged()}),
        ),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn sec06_move_outside_scope_denied() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut scope = dyn_scope();
    scope.move_args = Some(MoveArgs {
        parent_arg: Some("dest_parent".to_owned()),
        range_arg: None,
    });
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", scope));

    // Reparenting onto track5 escapes the OneOf{track3} constraint —
    // the before-state match does not save it.
    let bound = bind_req(
        &mut rig,
        &clip_edit_req(
            "task-a",
            "grant-a",
            clip11(),
            json!({"dest_parent": TrackId::new(5).to_tagged()}),
        ),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

#[test]
fn sec06_move_range_outside_bound_denied() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut scope = dyn_scope();
    scope.move_args = Some(MoveArgs {
        parent_arg: None,
        range_arg: Some("dest_range".to_owned()),
    });
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", scope));

    // Moving to 900..1140 escapes the 0..960 placement bound.
    let bound = bind_req(
        &mut rig,
        &clip_edit_req(
            "task-a",
            "grant-a",
            clip11(),
            json!({"dest_range": {"domain": "project-musical", "start": 900, "len": 240}}),
        ),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

#[test]
fn sec06_move_args_unverifiable_after_state_denied() {
    // A declared destination field the evaluator cannot verify denies
    // — an unverifiable after-state is never assumed safe. `dest_raw`
    // is a schema-valid plain string, but it is not a tagged id, so
    // the after-state cannot be derived.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut scope = dyn_scope();
    scope.move_args = Some(MoveArgs {
        parent_arg: Some("dest_raw".to_owned()),
        range_arg: None,
    });
    rig.store
        .issue_grant(dynamic_grant("grant-a", "task-a", scope));

    let bound = bind_req(
        &mut rig,
        &clip_edit_req(
            "task-a",
            "grant-a",
            clip11(),
            json!({"dest_raw": "not-a-tagged-id"}),
        ),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

// ════════════════════════════════════════════════════════════
// SEC-11: create axis — create_within constraints
// ════════════════════════════════════════════════════════════

fn create_grant(tag: &str, task_tag: &str, within: Option<CreateWithin>) -> Grant {
    let mut grant = base_grant(tag, task_tag);
    grant.capabilities = BTreeSet::from([cap_ref("synvelis.midi.create-source", 1, 0)]);
    grant.scope = ResourceScope::Exact {
        resources: BTreeSet::new(),
    };
    grant.data_access = DataAccessSet::new();
    grant.create_within = within;
    grant
}

#[test]
fn sec11_create_without_axis_denied() {
    // create_within: None — the grant covers existing resources only;
    // the create axis is off by default.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(create_grant("grant-a", "task-a", None));
    let bound = bind_req(&mut rig, &create_source_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.create-source",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), "creation outside grant constraints");
}

#[test]
fn sec11_create_permitted_parent_and_kind() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(create_grant(
        "grant-a",
        "task-a",
        Some(CreateWithin {
            parents: OwnerConstraint::OneOf {
                refs: BTreeSet::from([project_ref()]),
            },
            kinds: BTreeSet::from([ResourceKind::Source]),
            placement: None,
        }),
    ));
    let bound = bind_req(&mut rig, &create_source_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.create-source",
        1,
        0,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn sec11_create_wrong_parent_denied() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(create_grant(
        "grant-a",
        "task-a",
        Some(CreateWithin {
            parents: OwnerConstraint::OneOf {
                refs: BTreeSet::from([track3()]),
            },
            kinds: BTreeSet::from([ResourceKind::Source]),
            placement: None,
        }),
    ));
    let bound = bind_req(&mut rig, &create_source_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.create-source",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), "creation outside grant constraints");
}

#[test]
fn sec11_create_wrong_kind_denied() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(create_grant(
        "grant-a",
        "task-a",
        Some(CreateWithin {
            parents: OwnerConstraint::OneOf {
                refs: BTreeSet::from([project_ref()]),
            },
            kinds: BTreeSet::from([ResourceKind::Clip]),
            placement: None,
        }),
    ));
    let bound = bind_req(&mut rig, &create_source_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.create-source",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), "creation outside grant constraints");
}

#[test]
fn sec11_create_under_unresolvable_parent_denied() {
    // An unverifiable parent cannot be proven in-bounds — even under
    // `OwnerConstraint::Any` the parent must resolve.
    let mut rig = rig_with(|_| {});
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(create_grant(
        "grant-a",
        "task-a",
        Some(CreateWithin {
            parents: OwnerConstraint::Any,
            kinds: BTreeSet::from([ResourceKind::Source]),
            placement: None,
        }),
    ));
    let bound = bind_req(&mut rig, &create_source_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.create-source",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), "creation outside grant constraints");
}

// ════════════════════════════════════════════════════════════
// AHG-009: three apply modes — derived from the grant, not the wire
// ════════════════════════════════════════════════════════════

fn mode_grant(tag: &str, task_tag: &str, policy: ApprovalPolicy) -> Grant {
    let mut grant = base_grant(tag, task_tag);
    grant.approval_policy = policy;
    grant
}

#[test]
fn ahg009_suggest_mode_denies_apply_but_permits_prestages() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(mode_grant("grant-a", "task-a", ApprovalPolicy::Suggest));
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());

    // Pre-apply stages still permit — a suggest grant is real
    // authority for read/prepare work.
    for stage in [
        AuthorizationStage::DiscoverCapability,
        AuthorizationStage::ReadContext,
        AuthorizationStage::PrepareProposal,
    ] {
        let d = decide(&rig, &bound, stage, "synvelis.midi.add-note", 1, 1);
        assert_eq!(
            d,
            AuthorizationDecision::PermitForStage,
            "stage {stage:?} must permit"
        );
    }
    let apply = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(
        deny_reason(&apply),
        "apply is not permitted under a suggest-mode grant"
    );
    let commit = decide(
        &rig,
        &bound,
        AuthorizationStage::Commit,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(
        deny_reason(&commit),
        "apply is not permitted under a suggest-mode grant"
    );
}

#[test]
fn ahg009_approve_to_apply_requires_approval() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(mode_grant(
        "grant-a",
        "task-a",
        ApprovalPolicy::ApproveToApply,
    ));
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());

    let apply = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(apply, AuthorizationDecision::ApprovalRequired);
    let commit = decide(
        &rig,
        &bound,
        AuthorizationStage::Commit,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(commit, AuthorizationDecision::ApprovalRequired);
}

#[test]
fn ahg009_delegated_apply_permits_inside_budget() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
    // Commit is a fresh evaluation; with no budget bound its unit
    // spend always succeeds.
    let commit = decide(
        &rig,
        &bound,
        AuthorizationStage::Commit,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(commit, AuthorizationDecision::PermitForStage);
}

#[test]
fn ahg009_apply_mode_comes_from_grant_not_request() {
    // The same request shape under different grant policies yields
    // the policy's mode — nothing the request carries selects it.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(mode_grant("grant-s", "task-a", ApprovalPolicy::Suggest));
    rig.store.issue_grant(mode_grant(
        "grant-d",
        "task-a",
        ApprovalPolicy::DelegatedApply,
    ));
    rig.store.issue_grant(mode_grant(
        "grant-p",
        "task-a",
        ApprovalPolicy::ApproveToApply,
    ));

    let req = add_note_req("task-a", "grant-s");
    let bound = bind_req(&mut rig, &req, agent());
    assert_eq!(
        deny_reason(&decide(
            &rig,
            &bound,
            AuthorizationStage::RequestApply,
            "synvelis.midi.add-note",
            1,
            1
        )),
        "apply is not permitted under a suggest-mode grant"
    );

    let bound_d = bind_req(&mut rig, &add_note_req("task-a", "grant-d"), agent());
    assert_eq!(
        decide(
            &rig,
            &bound_d,
            AuthorizationStage::RequestApply,
            "synvelis.midi.add-note",
            1,
            1
        ),
        AuthorizationDecision::PermitForStage
    );

    let bound_p = bind_req(&mut rig, &add_note_req("task-a", "grant-p"), agent());
    assert_eq!(
        decide(
            &rig,
            &bound_p,
            AuthorizationStage::RequestApply,
            "synvelis.midi.add-note",
            1,
            1
        ),
        AuthorizationDecision::ApprovalRequired
    );
}

// ════════════════════════════════════════════════════════════
// AHG-010: limits — delegated apply budget
// ════════════════════════════════════════════════════════════

#[test]
fn ahg010_apply_budget_exhausts() {
    // The budget caps commit authorizations — one unit per commit
    // decision. Preflight reports exhaustion but never spends.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.limits.apply_budget = Some(2);
    rig.store.issue_grant(grant);

    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    // Apply requests are free — repeated preflight spends nothing.
    for _ in 0..3 {
        assert_eq!(
            decide(
                &rig,
                &bound,
                AuthorizationStage::RequestApply,
                "synvelis.midi.add-note",
                1,
                1
            ),
            AuthorizationDecision::PermitForStage
        );
    }
    let c1 = decide(
        &rig,
        &bound,
        AuthorizationStage::Commit,
        "synvelis.midi.add-note",
        1,
        1,
    );
    let c2 = decide(
        &rig,
        &bound,
        AuthorizationStage::Commit,
        "synvelis.midi.add-note",
        1,
        1,
    );
    let c3 = decide(
        &rig,
        &bound,
        AuthorizationStage::Commit,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(c1, AuthorizationDecision::PermitForStage);
    assert_eq!(c2, AuthorizationDecision::PermitForStage);
    assert_eq!(deny_reason(&c3), "apply budget exhausted");
    // An exhausted budget also shows at apply preflight.
    assert_eq!(
        deny_reason(&decide(
            &rig,
            &bound,
            AuthorizationStage::RequestApply,
            "synvelis.midi.add-note",
            1,
            1
        )),
        "apply budget exhausted"
    );
}

#[test]
fn ahg010_budget_binds_at_commit() {
    // One unit per commit authorization — evaluating RequestApply
    // repeatedly (preflight, then effect-aware) spends nothing.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.limits.apply_budget = Some(1);
    rig.store.issue_grant(grant);

    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let desc = descriptor_of(&rig, "synvelis.midi.add-note", 1, 1);
    assert_eq!(
        decide(
            &rig,
            &bound,
            AuthorizationStage::RequestApply,
            "synvelis.midi.add-note",
            1,
            1
        ),
        AuthorizationDecision::PermitForStage
    );
    // The same stage through the effect-aware path — still no spend.
    let d = rig.policy.evaluate_effects(
        &bound,
        AuthorizationStage::RequestApply,
        &desc,
        &ScopedEffects::default(),
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
    // The first commit authorization spends the single unit.
    assert_eq!(
        rig.policy.evaluate_commit(&bound, &desc),
        AuthorizationDecision::PermitForStage
    );
    // A second commit finds the budget gone.
    assert_eq!(
        deny_reason(&rig.policy.evaluate_commit(&bound, &desc)),
        "apply budget exhausted"
    );
}

#[test]
fn ahg010_commit_without_prior_apply_enforces_budget() {
    // A commit authorization reached without a prior apply preflight
    // still enforces the budget — zero budget denies outright.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.limits.apply_budget = Some(0);
    rig.store.issue_grant(grant);

    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let desc = descriptor_of(&rig, "synvelis.midi.add-note", 1, 1);
    assert_eq!(
        deny_reason(&rig.policy.evaluate_commit(&bound, &desc)),
        "apply budget exhausted"
    );
}

#[test]
fn ahg010_reissue_preserves_revocation_and_spend() {
    // Single-issuance: reissuing under an existing id keeps the
    // record's revocation and consumption state — a confirmed revoke
    // never resurrects and a spent budget never refills.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.limits.apply_budget = Some(1);
    rig.store.issue_grant(grant);
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let desc = descriptor_of(&rig, "synvelis.midi.add-note", 1, 1);
    assert_eq!(
        rig.policy.evaluate_commit(&bound, &desc),
        AuthorizationDecision::PermitForStage
    );

    rig.store.revoke_grant(&grant_ref("grant-a"));
    // Reissue under the same id — the record keeps its state.
    rig.store.issue_grant(base_grant("grant-a", "task-a"));
    let record = rig.store.grant(&grant_ref("grant-a")).unwrap();
    assert!(record.grant.validity.revoked);
    assert_eq!(record.consumed_applies, 1);
}

// ════════════════════════════════════════════════════════════
// AHG-007: activation vs grant — proactive permit is separate
// ════════════════════════════════════════════════════════════

#[test]
fn ahg007_delegated_grant_is_not_proactive_authority() {
    // A proactively-started task requires a live proactive permit —
    // holding a delegated-apply grant is not activation authority.
    let mut rig = rig();
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("task-p"),
        principal: agent_principal(),
        project: project(),
        activation: Activation::Proactive {
            permit: permit_ref("permit-missing"),
        },
        validity: Validity::default(),
    });
    rig.store.issue_grant(base_grant("grant-a", "task-p"));

    let bound = bind_req(&mut rig, &add_note_req("task-p", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn ahg007_live_proactive_permit_activates() {
    let mut rig = rig();
    rig.store.issue_proactive_permit(ProactivePermit {
        id: permit_ref("permit-1"),
        principal: agent_principal(),
        project: project(),
        condition: "user permitted proactive clip cleanup".to_owned(),
        validity: Validity::default(),
    });
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("task-p"),
        principal: agent_principal(),
        project: project(),
        activation: Activation::Proactive {
            permit: permit_ref("permit-1"),
        },
        validity: Validity::default(),
    });
    rig.store.issue_grant(base_grant("grant-a", "task-p"));

    let bound = bind_req(&mut rig, &add_note_req("task-p", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn ahg007_revoked_proactive_permit_denies() {
    let mut rig = rig();
    rig.store.issue_proactive_permit(ProactivePermit {
        id: permit_ref("permit-1"),
        principal: agent_principal(),
        project: project(),
        condition: "user permitted proactive cleanup".to_owned(),
        validity: Validity::default(),
    });
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("task-p"),
        principal: agent_principal(),
        project: project(),
        activation: Activation::Proactive {
            permit: permit_ref("permit-1"),
        },
        validity: Validity::default(),
    });
    rig.store.issue_grant(base_grant("grant-a", "task-p"));
    rig.store.revoke_proactive_permit(&permit_ref("permit-1"));

    let bound = bind_req(&mut rig, &add_note_req("task-p", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn ahg007_expired_proactive_permit_denies() {
    // A permit that lapsed before the evaluation instant does not
    // activate — same uniform deny as a missing one.
    let mut rig = rig();
    rig.store.issue_proactive_permit(ProactivePermit {
        id: permit_ref("permit-1"),
        principal: agent_principal(),
        project: project(),
        condition: "user permitted proactive cleanup".to_owned(),
        validity: Validity {
            expires_at: Some(PolicyInstant::new(500)),
            revoked: false,
        },
    });
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("task-p"),
        principal: agent_principal(),
        project: project(),
        activation: Activation::Proactive {
            permit: permit_ref("permit-1"),
        },
        validity: Validity::default(),
    });
    rig.store.issue_grant(base_grant("grant-a", "task-p"));

    let bound = bind_req(&mut rig, &add_note_req("task-p", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn ahg007_permit_for_other_principal_denies() {
    let mut rig = rig();
    rig.store.issue_proactive_permit(ProactivePermit {
        id: permit_ref("permit-1"),
        principal: other_principal(),
        project: project(),
        condition: "permit belongs to another agent".to_owned(),
        validity: Validity::default(),
    });
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("task-p"),
        principal: agent_principal(),
        project: project(),
        activation: Activation::Proactive {
            permit: permit_ref("permit-1"),
        },
        validity: Validity::default(),
    });
    rig.store.issue_grant(base_grant("grant-a", "task-p"));

    let bound = bind_req(&mut rig, &add_note_req("task-p", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn ahg007_session_binding_matches_adapter() {
    // A session-bound grant keys on the attesting adapter — the task
    // is still required for activation.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.binding = GrantBinding::Session {
        adapter: agent().adapter(),
    };
    rig.store.issue_grant(grant);

    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn ahg007_session_binding_rejects_other_adapter() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.binding = GrantBinding::Session {
        adapter: AdapterId::new(0xDEAD_BEEF),
    };
    rig.store.issue_grant(grant);

    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(deny_reason(&d), NO_AUTH);
}

#[test]
fn ahg007_persistent_project_binding_permits() {
    // A persistent-project grant binds principal+project only — the
    // task is still required for activation, but the grant needs no
    // task or adapter tie of its own.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.binding = GrantBinding::PersistentProject;
    rig.store.issue_grant(grant);

    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

// ════════════════════════════════════════════════════════════
// SEC-14 / GTC-003: effect-aware evaluation — ScopedEffects
// ════════════════════════════════════════════════════════════

#[test]
fn sec14_unverifiable_effects_deny() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let mut effects = ScopedEffects::default();
    effects.unsupported.push("opaque-plugin-state".to_owned());
    let d = rig.policy.evaluate_effects(
        &bound,
        AuthorizationStage::RequestApply,
        &descriptor_of(&rig, "synvelis.midi.add-note", 1, 1),
        &effects,
    );
    assert_eq!(deny_reason(&d), "unverifiable effects");
}

#[test]
fn sec14_write_outside_scope_denied() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let mut effects = ScopedEffects::default();
    effects.writes.push(source10());
    let d = rig.policy.evaluate_effects(
        &bound,
        AuthorizationStage::RequestApply,
        &descriptor_of(&rig, "synvelis.midi.add-note", 1, 1),
        &effects,
    );
    assert_eq!(deny_reason(&d), OUTSIDE_SCOPE);
}

#[test]
fn sec14_create_effect_checks_axis() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(create_grant(
        "grant-a",
        "task-a",
        Some(CreateWithin {
            parents: OwnerConstraint::OneOf {
                refs: BTreeSet::from([project_ref()]),
            },
            kinds: BTreeSet::from([ResourceKind::Source]),
            placement: None,
        }),
    ));
    let bound = bind_req(&mut rig, &create_source_req("task-a", "grant-a"), agent());

    let mut effects = ScopedEffects::default();
    effects.creates.push(CreateEffect {
        parent: track3(), // not a permitted parent
        creates: ResourceKind::Source,
        range: None,
    });
    let d = rig.policy.evaluate_effects(
        &bound,
        AuthorizationStage::RequestApply,
        &descriptor_of(&rig, "synvelis.midi.create-source", 1, 0),
        &effects,
    );
    assert_eq!(deny_reason(&d), "creation outside grant constraints");

    let mut effects = ScopedEffects::default();
    effects.creates.push(CreateEffect {
        parent: project_ref(),
        creates: ResourceKind::Source,
        range: None,
    });
    let d = rig.policy.evaluate_effects(
        &bound,
        AuthorizationStage::RequestApply,
        &descriptor_of(&rig, "synvelis.midi.create-source", 1, 0),
        &effects,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

#[test]
fn data_effect_disclosure_and_transfer_checked() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    // Grant covers MetadataRead only — an actual disclosure or
    // provider transfer denies even when the capability's bound
    // passed the descriptor-level check.
    let mut grant = base_grant("grant-a", "task-a");
    grant.capabilities = BTreeSet::from([cap_ref("synvelis.midi.add-note", 1, 1)]);
    rig.store.issue_grant(grant);
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());

    let mut effects = ScopedEffects::default();
    effects
        .disclosures
        .push(DataAccessClass::ExternalDisclosure);
    let d = rig.policy.evaluate_effects(
        &bound,
        AuthorizationStage::RequestApply,
        &descriptor_of(&rig, "synvelis.midi.add-note", 1, 1),
        &effects,
    );
    assert_eq!(deny_reason(&d), "data access outside grant");

    let effects = ScopedEffects {
        external_transfers: 1,
        ..ScopedEffects::default()
    };
    let d = rig.policy.evaluate_effects(
        &bound,
        AuthorizationStage::RequestApply,
        &descriptor_of(&rig, "synvelis.midi.add-note", 1, 1),
        &effects,
    );
    assert_eq!(deny_reason(&d), "data access outside grant");

    // With ProviderTransfer granted, a transfer is permitted.
    let mut grant = base_grant("grant-b", "task-a");
    grant.data_access = DataAccessSet::of([
        DataAccessClass::MetadataRead,
        DataAccessClass::ProviderTransfer,
    ]);
    rig.store.issue_grant(grant);
    let bound_b = bind_req(&mut rig, &add_note_req("task-a", "grant-b"), agent());
    let d = rig.policy.evaluate_effects(
        &bound_b,
        AuthorizationStage::RequestApply,
        &descriptor_of(&rig, "synvelis.midi.add-note", 1, 1),
        &effects,
    );
    assert_eq!(d, AuthorizationDecision::PermitForStage);
}

// ════════════════════════════════════════════════════════════
// ACG-003 seam: commit revalidation / revoke linearization
// ════════════════════════════════════════════════════════════

#[test]
fn acg003_commit_revalidates_fresh_state() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let desc = descriptor_of(&rig, "synvelis.midi.add-note", 1, 1);

    // Preflight permit…
    let pre = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "synvelis.midi.add-note",
        1,
        1,
    );
    assert_eq!(pre, AuthorizationDecision::PermitForStage);
    // …then a revoke lands before the commit read → deny.
    rig.store.revoke_grant(&grant_ref("grant-a"));
    let commit = rig.policy.evaluate_commit(&bound, &desc);
    assert_eq!(deny_reason(&commit), "grant revoked");
}

#[test]
fn acg003_commit_before_revoke_stands() {
    // The mirror side of the linearization: a commit authorized
    // before the revoke is confirmed stands — the earlier decision is
    // a value already returned, never retracted.
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let bound = bind_req(&mut rig, &add_note_req("task-a", "grant-a"), agent());
    let desc = descriptor_of(&rig, "synvelis.midi.add-note", 1, 1);

    let commit = rig.policy.evaluate_commit(&bound, &desc);
    assert_eq!(commit, AuthorizationDecision::PermitForStage);
    rig.store.revoke_grant(&grant_ref("grant-a"));
    // The pre-revoke decision remains a PermitForStage value — GW-06's
    // fence decides which side of the serialization the commit was on.
    assert_eq!(commit, AuthorizationDecision::PermitForStage);
}

// ════════════════════════════════════════════════════════════
// End-to-end through Gateway::invoke — stage mapping + envelopes
// ════════════════════════════════════════════════════════════

#[test]
fn invoke_maps_mutation_to_request_apply() {
    // Gateway::invoke derives the stage from the descriptor domain —
    // a suggest-mode grant denies a mutation invoke with the
    // typed Denied envelope at RequestApply.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store
        .issue_grant(mode_grant("grant-a", "task-a", ApprovalPolicy::Suggest));
    let err = rig
        .gw
        .handle(&add_note_req("task-a", "grant-a"), agent())
        .expect_err("suggest-mode invoke must deny");
    assert!(
        matches!(
            err.code,
            GatewayError::Denied {
                stage: AuthorizationStage::RequestApply,
                ..
            }
        ),
        "denial must carry the mapped stage: {err:?}"
    );
    assert_eq!(err.category, ErrorCategory::Authorization);
}

#[test]
fn invoke_delegated_apply_runs_handler() {
    let mut rig = rig();
    issue_agent_authority(&rig.store, "task-a", "grant-a");
    let outcome = rig
        .gw
        .handle(&add_note_req("task-a", "grant-a"), agent())
        .expect("delegated apply should execute");
    assert!(matches!(outcome, GatewayOutcome::Completed { .. }));
}

#[test]
fn invoke_approve_to_apply_returns_approval_outcome() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(mode_grant(
        "grant-a",
        "task-a",
        ApprovalPolicy::ApproveToApply,
    ));
    let outcome = rig
        .gw
        .handle(&add_note_req("task-a", "grant-a"), agent())
        .expect("approve-to-apply yields an approval outcome");
    assert!(matches!(outcome, GatewayOutcome::ApprovalRequired { .. }));
}

#[test]
fn invoke_query_maps_to_read_context() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.capabilities = BTreeSet::from([cap_ref("synvelis.context.source-notes", 1, 0)]);
    grant.permissions =
        PermissionSet::of([PermissionKind::ReadContext, PermissionKind::DiscloseContext]);
    grant.data_access = DataAccessSet::of([
        DataAccessClass::MetadataRead,
        DataAccessClass::MediaBytesRead,
        DataAccessClass::ExternalDisclosure,
    ]);
    rig.store.issue_grant(grant);
    let outcome = rig
        .gw
        .handle(&context_req("task-a", "grant-a"), agent())
        .expect("query should complete");
    assert!(matches!(outcome, GatewayOutcome::Completed { .. }));
}

#[test]
fn invoke_transport_under_grant() {
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    let mut grant = base_grant("grant-a", "task-a");
    grant.capabilities = BTreeSet::from([cap_ref("synvelis.transport.set-position", 1, 0)]);
    grant.permissions = PermissionSet::of([PermissionKind::ControlTransport]);
    grant.data_access = DataAccessSet::new();
    grant.scope = ResourceScope::Exact {
        resources: BTreeSet::new(),
    };
    rig.store.issue_grant(grant);
    let outcome = rig
        .gw
        .handle(&transport_req("task-a", "grant-a"), agent())
        .expect("transport should run");
    assert!(matches!(outcome, GatewayOutcome::Completed { .. }));
}

// ════════════════════════════════════════════════════════════
// GT-003 for GW-02: registration never widens a grant
// ════════════════════════════════════════════════════════════

#[test]
fn gt003_new_capability_not_implicitly_granted() {
    // A capability registered after the grant was issued is outside
    // its explicit set — existence in the registry grants nothing.
    let mut rig = rig();
    issue_task(&rig.store, "task-a", &agent_principal());
    rig.store.issue_grant(base_grant("grant-a", "task-a"));
    // The grant covers add-note only; test.clip.edit was registered
    // after issuance and is outside the set.
    let bound = bind_req(
        &mut rig,
        &clip_edit_req("task-a", "grant-a", clip11(), json!({})),
        agent(),
    );
    let d = decide(
        &rig,
        &bound,
        AuthorizationStage::RequestApply,
        "test.clip.edit",
        1,
        0,
    );
    assert_eq!(deny_reason(&d), "capability outside grant");
}
