//! Issue #601 slice 2 — `GrantPolicy` property suite.
//!
//! Target contracts (product source of truth:
//! `crates/synvelis-gateway/src/policy.rs`, `src/authorization.rs`):
//!
//! - Every decision stays in the closed `AuthorizationDecision` set
//!   (`PermitForStage` / `ApprovalRequired` / `Deny`), and every deny
//!   reason stays in the documented contract string set
//!   (`DENY_REASONS` below — `pub(crate)` literals copied by value).
//! - An agent caller whose authority chain fails at any link —
//!   unresolved refs, principal/project mismatch, dead validity,
//!   dead/missing proactive permit, or binding mismatch — uniformly
//!   receives `Deny { reason: "no applicable authorization" }`:
//!   existence-blind, across all eight stages.
//! - A trusted human with a matching descriptor is `PermitForStage`
//!   at every stage (and under `evaluate_commit`/`evaluate_effects`);
//!   a descriptor mismatch denies *before* the human short-circuit.
//! - `DelegatedApply` + `Commit` consumes `apply_budget` monotonically:
//!   each `PermitForStage` spends exactly one unit, exhaustion denies
//!   `apply budget exhausted`, and `try_consume_apply` agrees.
//! - Preflight (`RequestApply`, `evaluate_effects` at non-Commit
//!   stages, and every `Deny`) never moves `consumed_applies`.
//! - `evaluate_commit` is the same evaluation as `decide(Commit)` —
//!   two identical rigs observe identical decisions.
//! - Scope coverage is invariant under repeated evaluation and
//!   resource overlap/deduplication; unresolved or uncovered
//!   resources deny `target outside grant scope`, create defects deny
//!   `creation outside grant constraints`, unclassifiable effects
//!   deny `unverifiable effects`, and disclosures/transfers outside
//!   `data_access` deny `data access outside grant`.
//!
//! Bounds: a fresh `InMemoryPolicyStore` + `ManualClock` +
//! `StaticOracle` + fixture `Gateway` per generated case (no
//! cross-case budget/state contamination); budgets ≤ 5; ≤ budget+2
//! commit iterations per case; case counts kept small since each
//! case builds a full rig. Requests enter through the real
//! `parse_wire` + `bind` path — `OperationRequest`'s constructor is
//! private, so trusted `FixtureAdapter` seals supply the caller.

use std::collections::BTreeSet;
use std::sync::Arc;

use proptest::prelude::*;
use serde_json::{Value, json};
use synvelis_gateway::authorization::{AuthorizationDecision, AuthorizationStage, PolicyEvaluator};
use synvelis_gateway::capability::{
    CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityVersion, DataAccessClass,
    DataAccessSet, PermissionKind, PermissionSet,
};
use synvelis_gateway::clock::{ManualClock, PolicyInstant};
use synvelis_gateway::fixture::{self, FixtureAdapter, StaticOracle};
use synvelis_gateway::gateway::Gateway;
use synvelis_gateway::grant::{
    ApprovalPolicy, CreateWithin, Grant, GrantBinding, GrantLimits, OwnerConstraint, PolicyEpoch,
    ResourceAttributes, ResourceScope, ScopedEffects, Validity,
};
use synvelis_gateway::id::{AdapterId, GrantRef, ProactivePermitRef, TaskRef};
use synvelis_gateway::policy::GrantPolicy;
use synvelis_gateway::request::{
    CallerIdentity, OperationRequest, Principal, PrincipalKind, ResourceKind, ResourceRef,
};
use synvelis_gateway::store::{
    Activation, AuthorizedTask, InMemoryPolicyStore, PolicyStore, ProactivePermit,
};
use synvelis_project::{ProjectId, ProjectRevision, SourceId};

use crate::strategies::arb_ref_text;

/// The documented deny-reason contract — `pub(crate)` literals in
/// `src/policy.rs`, copied by value so a rename drifts loudly.
const DENY_REASONS: &[&str] = &[
    "capability descriptor mismatch",
    "no applicable authorization",
    "target outside grant scope",
    "creation outside grant constraints",
    "grant revoked",
    "grant expired",
    "policy version superseded",
    "capability outside grant",
    "required permissions outside grant",
    "data access outside grant",
    "apply is not permitted under a suggest-mode grant",
    "apply budget exhausted",
    "unverifiable effects",
];

const NO_AUTH: &str = "no applicable authorization";
const OUTSIDE_SCOPE: &str = "target outside grant scope";
const OUTSIDE_CREATE: &str = "creation outside grant constraints";
const BUDGET_EXHAUSTED: &str = "apply budget exhausted";
const DESCRIPTOR_MISMATCH: &str = "capability descriptor mismatch";
const DATA_ACCESS_OUTSIDE: &str = "data access outside grant";
const UNVERIFIABLE: &str = "unverifiable effects";
const REVOKED: &str = "grant revoked";

const ALL_STAGES: [AuthorizationStage; 8] = [
    AuthorizationStage::DiscoverCapability,
    AuthorizationStage::ReadContext,
    AuthorizationStage::PrepareProposal,
    AuthorizationStage::RequestApply,
    AuthorizationStage::Commit,
    AuthorizationStage::StartRuntimeOperation,
    AuthorizationStage::StartAsyncOperation,
    AuthorizationStage::ReadOperationResult,
];

/// The adapter id `FixtureAdapter::for_testing()` seals callers with.
const TEST_ADAPTER_RAW: u128 = 0xF1C7_0001;

/// A project distinct from `spine.project` for mismatch defects.
fn other_project(spine: &Spine) -> ProjectId {
    let other = ProjectId::new(0);
    if spine.project == other {
        ProjectId::new(1)
    } else {
        other
    }
}

/// Asserts the decision is a member of the closed set and any deny
/// reason belongs to the contract string set.
fn assert_closed(decision: &AuthorizationDecision) {
    match decision {
        AuthorizationDecision::PermitForStage | AuthorizationDecision::ApprovalRequired => {}
        AuthorizationDecision::Deny { reason } => assert!(
            DENY_REASONS.contains(&reason.as_str()),
            "deny reason outside contract set: {reason:?}"
        ),
    }
}

/// Decision equality that does not rely on `PartialEq` derives.
fn same_decision(a: &AuthorizationDecision, b: &AuthorizationDecision) -> bool {
    match (a, b) {
        (AuthorizationDecision::PermitForStage, AuthorizationDecision::PermitForStage)
        | (AuthorizationDecision::ApprovalRequired, AuthorizationDecision::ApprovalRequired) => {
            true
        }
        (
            AuthorizationDecision::Deny { reason: ra },
            AuthorizationDecision::Deny { reason: rb },
        ) => ra == rb,
        _ => false,
    }
}

// ── Rig ─────────────────────────────────────────────────────

struct Rig {
    store: Arc<InMemoryPolicyStore>,
    policy: GrantPolicy,
    gw: Gateway,
}

/// Fresh store/clock/oracle/gateway per case — no cross-case state.
fn rig(now: u64, build: impl FnOnce(&mut StaticOracle)) -> Rig {
    let store = Arc::new(InMemoryPolicyStore::new());
    let clock = Arc::new(ManualClock::new(PolicyInstant::new(now)));
    let mut oracle = StaticOracle::new();
    build(&mut oracle);
    let oracle = Arc::new(oracle);
    let policy = GrantPolicy::new(store.clone(), clock.clone(), oracle.clone());
    let mut gw = Gateway::with_evaluator(Box::new(GrantPolicy::new(store.clone(), clock, oracle)));
    fixture::install(&mut gw).expect("fixture install");
    Rig { store, policy, gw }
}

/// The generated identity spine a case is built around.
#[derive(Debug)]
struct Spine {
    project: ProjectId,
    principal: String,
    task: String,
    grant: String,
    now: u64,
}

fn arb_spine() -> impl Strategy<Value = Spine> {
    (
        any::<u128>(),
        arb_ref_text(),
        arb_ref_text(),
        arb_ref_text(),
        1u64..10_000,
    )
        .prop_map(|(project, principal, task, grant, now)| Spine {
            project: ProjectId::new(project),
            principal,
            task,
            grant,
            now,
        })
}

fn agent_caller(spine: &Spine) -> CallerIdentity {
    FixtureAdapter::for_testing()
        .agent(spine.principal.clone())
        .expect("generated principal id is valid")
}

fn human_caller(id: &str) -> CallerIdentity {
    FixtureAdapter::for_testing()
        .human(id.to_owned())
        .expect("generated principal id is valid")
}

fn agent_principal(spine: &Spine) -> Principal {
    Principal::new(PrincipalKind::Agent, spine.principal.clone())
        .expect("generated principal id is valid")
}

/// A principal id guaranteed to differ from the spine's.
fn other_principal(spine: &Spine) -> Principal {
    let id = if spine.principal == "other-agent" {
        "other-2"
    } else {
        "other-agent"
    };
    Principal::new(PrincipalKind::Agent, id).expect("literal id")
}

/// A task ref guaranteed to differ from the spine's.
fn other_task_ref(spine: &Spine) -> TaskRef {
    let tag = if spine.task == "task-other" {
        "task-other2"
    } else {
        "task-other"
    };
    TaskRef::new(tag).expect("literal ref")
}

fn task_ref(spine: &Spine) -> TaskRef {
    TaskRef::new(spine.task.clone()).expect("generated ref is valid")
}

fn grant_ref(spine: &Spine) -> GrantRef {
    GrantRef::new(spine.grant.clone()).expect("generated ref is valid")
}

fn cap_ref(id: &str, minor: u32) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id.to_owned()).expect("fixture capability ids are valid"),
        version: CapabilityVersion::new(1, minor),
    }
}

/// Live task bound to the spine's principal/project.
fn issue_task(store: &InMemoryPolicyStore, spine: &Spine) {
    store.issue_task(AuthorizedTask {
        task: task_ref(spine),
        principal: agent_principal(spine),
        project: spine.project,
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
}

/// A grant covering every fixture capability's declared dimensions —
/// wide enough that a bound case permits unless a specific dimension
/// is defected away.
fn wide_grant(
    spine: &Spine,
    capability: CapabilityRef,
    scope: ResourceScope,
    approval: ApprovalPolicy,
    budget: Option<u64>,
) -> Grant {
    Grant {
        id: grant_ref(spine),
        issued_by: Principal::new(PrincipalKind::Human, "issuer").expect("literal id"),
        principal: agent_principal(spine),
        project: spine.project,
        binding: GrantBinding::Task {
            task: task_ref(spine),
        },
        capabilities: BTreeSet::from([capability]),
        permissions: PermissionSet::of([
            PermissionKind::ReadContext,
            PermissionKind::DiscloseContext,
            PermissionKind::MutateProject,
            PermissionKind::ControlTransport,
            PermissionKind::RunJob,
            PermissionKind::ExternalAction,
        ]),
        data_access: DataAccessSet::of([
            DataAccessClass::MetadataRead,
            DataAccessClass::MediaBytesRead,
            DataAccessClass::StructuredContentRead,
            DataAccessClass::DerivedDataRead,
            DataAccessClass::WorkspaceSelectionRead,
            DataAccessClass::ExternalDisclosure,
            DataAccessClass::ProviderTransfer,
        ]),
        scope,
        create_within: None,
        approval_policy: approval,
        limits: GrantLimits {
            apply_budget: budget,
            job_budget: None,
        },
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision: Some(ProjectRevision(10)),
    }
}

/// Issues the aligned task+grant pair for a bound agent case.
fn issue_bound(
    store: &InMemoryPolicyStore,
    spine: &Spine,
    capability: CapabilityRef,
    scope: ResourceScope,
    approval: ApprovalPolicy,
    budget: Option<u64>,
) {
    issue_task(store, spine);
    store.issue_grant(wide_grant(spine, capability, scope, approval, budget));
}

fn source_attrs() -> ResourceAttributes {
    ResourceAttributes {
        kind: ResourceKind::Source,
        owner: None,
        range: None,
        created_revision: Some(ProjectRevision(1)),
        related: vec![],
    }
}

// ── Wire builders (all bind-legal for their capability) ──────

fn transport_wire(spine: &Spine, task: bool, grant: bool) -> Value {
    let mut req = json!({
        "request_id": "req-t1",
        "capability": {"id": "synvelis.transport.set-position", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.transport.set-position.input", "version": {"major": 1, "minor": 0}},
        "project": spine.project.to_tagged(),
        "args": {"action": "play"},
        "consistency": {"kind": "unconditional"},
    });
    let obj = req.as_object_mut().expect("wire object");
    if task {
        obj.insert("task".into(), Value::String(spine.task.clone()));
    }
    if grant {
        obj.insert("grant".into(), Value::String(spine.grant.clone()));
    }
    req
}

fn add_note_wire(spine: &Spine, target: &ResourceRef) -> Value {
    let ResourceRef::Source(source) = target else {
        unreachable!("add-note wires target sources");
    };
    json!({
        "request_id": "req-a1",
        "capability": {"id": "synvelis.midi.add-note", "version": {"major": 1, "minor": 1}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": spine.project.to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": source.to_tagged()},
        "args": {"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 240, "len": 480}},
        "consistency": {"kind": "expected-revision", "revision": 10},
        "task": spine.task,
        "grant": spine.grant,
    })
}

fn create_source_wire(spine: &Spine) -> Value {
    json!({
        "request_id": "req-c1",
        "capability": {"id": "synvelis.midi.create-source", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.midi.create-source.input", "version": {"major": 1, "minor": 0}},
        "project": spine.project.to_tagged(),
        "target": {"shape": "create", "parent_kind": "project", "parent": spine.project.to_tagged(), "creates": "source"},
        "args": {"division": 480},
        "consistency": {"kind": "expected-revision", "revision": 10},
        "task": spine.task,
        "grant": spine.grant,
    })
}

fn bind_ok(rig: &mut Rig, wire: &Value, caller: CallerIdentity) -> OperationRequest {
    let parsed = Gateway::parse_wire(wire).expect("generated wire must parse");
    rig.gw
        .bind(parsed, caller)
        .expect("generated request must bind")
}

fn descriptor_of(rig: &Rig, id: &str, minor: u32) -> CapabilityDescriptor {
    rig.gw
        .registry()
        .describe(&cap_ref(id, minor))
        .expect("fixture capability is registered")
        .clone()
}

fn deny_reason(decision: &AuthorizationDecision) -> &str {
    match decision {
        AuthorizationDecision::Deny { reason } => reason,
        other => panic!("expected deny, got {other:?}"),
    }
}

// ── Properties ──────────────────────────────────────────────

/// Every link-failure mode that must collapse to the uniform
/// existence-blind deny (AHG-007 / GT-004).
#[derive(Clone, Copy, Debug)]
enum UnboundDefect {
    NoTaskRef,
    NoGrantRef,
    TaskPrincipalMismatch,
    TaskProjectMismatch,
    TaskRevoked,
    TaskExpired,
    PermitMissing,
    PermitDead,
    GrantMissing,
    GrantPrincipalMismatch,
    GrantProjectMismatch,
    GrantBoundOtherTask,
    GrantBoundOtherSession,
}

const UNBOUND_DEFECTS: [UnboundDefect; 13] = [
    UnboundDefect::NoTaskRef,
    UnboundDefect::NoGrantRef,
    UnboundDefect::TaskPrincipalMismatch,
    UnboundDefect::TaskProjectMismatch,
    UnboundDefect::TaskRevoked,
    UnboundDefect::TaskExpired,
    UnboundDefect::PermitMissing,
    UnboundDefect::PermitDead,
    UnboundDefect::GrantMissing,
    UnboundDefect::GrantPrincipalMismatch,
    UnboundDefect::GrantProjectMismatch,
    UnboundDefect::GrantBoundOtherTask,
    UnboundDefect::GrantBoundOtherSession,
];

/// A validity that is dead at every generated `now` (`now >= 1`).
fn dead_validity() -> Validity {
    Validity {
        expires_at: Some(PolicyInstant::new(0)),
        revoked: false,
    }
}

/// The aligned grant used by unbound-agent defect cases.
fn aligned_grant(spine: &Spine) -> Grant {
    wide_grant(
        spine,
        cap_ref("synvelis.transport.set-position", 0),
        ResourceScope::Exact {
            resources: BTreeSet::new(),
        },
        ApprovalPolicy::DelegatedApply,
        None,
    )
}

/// Issues the task record for one unbound-agent defect; falls back to
/// the aligned task for defects on the grant side.
fn issue_defect_task(store: &InMemoryPolicyStore, spine: &Spine, defect: UnboundDefect) {
    match defect {
        UnboundDefect::TaskPrincipalMismatch => store.issue_task(AuthorizedTask {
            task: task_ref(spine),
            principal: other_principal(spine),
            project: spine.project,
            activation: Activation::UserIssued,
            validity: Validity::default(),
        }),
        UnboundDefect::TaskProjectMismatch => store.issue_task(AuthorizedTask {
            task: task_ref(spine),
            principal: agent_principal(spine),
            project: other_project(spine),
            activation: Activation::UserIssued,
            validity: Validity::default(),
        }),
        UnboundDefect::TaskRevoked => store.issue_task(AuthorizedTask {
            task: task_ref(spine),
            principal: agent_principal(spine),
            project: spine.project,
            activation: Activation::UserIssued,
            validity: Validity {
                expires_at: None,
                revoked: true,
            },
        }),
        UnboundDefect::TaskExpired => store.issue_task(AuthorizedTask {
            task: task_ref(spine),
            principal: agent_principal(spine),
            project: spine.project,
            activation: Activation::UserIssued,
            validity: dead_validity(),
        }),
        UnboundDefect::PermitMissing => store.issue_task(AuthorizedTask {
            task: task_ref(spine),
            principal: agent_principal(spine),
            project: spine.project,
            activation: Activation::Proactive {
                permit: ProactivePermitRef::new("p-missing").expect("literal ref"),
            },
            validity: Validity::default(),
        }),
        UnboundDefect::PermitDead => {
            let permit = ProactivePermitRef::new("p-dead").expect("literal ref");
            store.issue_task(AuthorizedTask {
                task: task_ref(spine),
                principal: agent_principal(spine),
                project: spine.project,
                activation: Activation::Proactive {
                    permit: permit.clone(),
                },
                validity: Validity::default(),
            });
            store.issue_proactive_permit(ProactivePermit {
                id: permit,
                principal: agent_principal(spine),
                project: spine.project,
                condition: "n/a".to_owned(),
                validity: dead_validity(),
            });
        }
        _ => issue_task(store, spine),
    }
}

/// Issues the grant record for one unbound-agent defect; falls back
/// to the aligned grant for task-side defects so exactly one link
/// fails.
fn issue_defect_grant(store: &InMemoryPolicyStore, spine: &Spine, defect: UnboundDefect) {
    match defect {
        UnboundDefect::NoGrantRef | UnboundDefect::GrantMissing => {}
        UnboundDefect::GrantPrincipalMismatch => {
            let mut grant = aligned_grant(spine);
            grant.principal = other_principal(spine);
            store.issue_grant(grant);
        }
        UnboundDefect::GrantProjectMismatch => {
            let mut grant = aligned_grant(spine);
            grant.project = other_project(spine);
            store.issue_grant(grant);
        }
        UnboundDefect::GrantBoundOtherTask => {
            let mut grant = aligned_grant(spine);
            grant.binding = GrantBinding::Task {
                task: other_task_ref(spine),
            };
            store.issue_grant(grant);
        }
        UnboundDefect::GrantBoundOtherSession => {
            let mut grant = aligned_grant(spine);
            grant.binding = GrantBinding::Session {
                adapter: AdapterId::new(TEST_ADAPTER_RAW.wrapping_add(1)),
            };
            store.issue_grant(grant);
        }
        _ => store.issue_grant(aligned_grant(spine)),
    }
}

/// Builds the store state for one unbound-agent defect. The task/
/// grant records always exist under the spine's refs unless the
/// defect removes them, so resolution reaches exactly the injected
/// failing link.
fn issue_unbound(store: &InMemoryPolicyStore, spine: &Spine, defect: UnboundDefect) {
    issue_defect_task(store, spine, defect);
    issue_defect_grant(store, spine, defect);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// (b) Every authority-chain failure collapses to the uniform
    /// existence-blind deny, at every stage.
    #[test]
    fn unbound_agent_gets_uniform_no_auth(
        spine in arb_spine(),
        defect in prop::sample::select(UNBOUND_DEFECTS.to_vec()),
    ) {
        let mut rig = rig(spine.now, |_| {});
        issue_unbound(&rig.store, &spine, defect);
        let wire = transport_wire(
            &spine,
            !matches!(defect, UnboundDefect::NoTaskRef),
            !matches!(defect, UnboundDefect::NoGrantRef),
        );
        let req = bind_ok(&mut rig, &wire, agent_caller(&spine));
        let desc = descriptor_of(&rig, "synvelis.transport.set-position", 0);
        for stage in ALL_STAGES {
            let decision = rig.policy.decide(&req, stage, &desc);
            assert_closed(&decision);
            prop_assert_eq!(deny_reason(&decision), NO_AUTH);
        }
    }

    /// (c) A trusted human is permitted at every stage — including
    /// the commit and effect-aware entry points — with no budget
    /// consumption ever possible.
    #[test]
    fn human_permitted_at_every_stage(spine in arb_spine()) {
        let mut rig = rig(spine.now, |_| {});
        let req = bind_ok(&mut rig, &transport_wire(&spine, true, true), human_caller("user-1"));
        let desc = descriptor_of(&rig, "synvelis.transport.set-position", 0);
        for stage in ALL_STAGES {
            let decision = rig.policy.decide(&req, stage, &desc);
            assert_closed(&decision);
            prop_assert!(matches!(decision, AuthorizationDecision::PermitForStage));
            let with_effects = rig.policy.evaluate_effects(
                &req,
                stage,
                &desc,
                &ScopedEffects {
                    unsupported: vec!["opaque".to_owned()],
                    ..ScopedEffects::default()
                },
            );
            assert_closed(&with_effects);
            prop_assert!(matches!(with_effects, AuthorizationDecision::PermitForStage));
        }
        prop_assert!(matches!(
            rig.policy.evaluate_commit(&req, &desc),
            AuthorizationDecision::PermitForStage
        ));
    }

    /// (pin) Descriptor mismatch denies before the human
    /// short-circuit — for agent and human callers alike.
    #[test]
    fn descriptor_mismatch_denies_first(spine in arb_spine(), human in any::<bool>()) {
        let mut rig = rig(spine.now, |_| {});
        issue_bound(
            &rig.store,
            &spine,
            cap_ref("synvelis.transport.set-position", 0),
            ResourceScope::Exact {
                resources: BTreeSet::new(),
            },
            ApprovalPolicy::DelegatedApply,
            None,
        );
        let caller = if human {
            human_caller("user-1")
        } else {
            agent_caller(&spine)
        };
        let req = bind_ok(&mut rig, &transport_wire(&spine, true, true), caller);
        // A descriptor naming a different registered capability —
        // id mismatch, not just version drift.
        let wrong = descriptor_of(&rig, "synvelis.midi.add-note", 1);
        let mut drifted = descriptor_of(&rig, "synvelis.transport.set-position", 0);
        drifted.version = CapabilityVersion::new(1, 7);
        for desc in [&wrong, &drifted] {
            for stage in ALL_STAGES {
                let decision = rig.policy.decide(&req, stage, desc);
                assert_closed(&decision);
                prop_assert_eq!(deny_reason(&decision), DESCRIPTOR_MISMATCH);
            }
        }
    }

    /// (d) DelegatedApply commit consumes budget monotonically:
    /// each permit spends exactly one unit, exhaustion denies with
    /// the contract reason, and the store's spend check agrees.
    #[test]
    fn delegated_apply_commit_spend_is_monotone(
        spine in arb_spine(),
        budget in prop::option::weighted(0.7, 0u64..=5),
    ) {
        let mut rig = rig(spine.now, |_| {});
        issue_bound(
            &rig.store,
            &spine,
            cap_ref("synvelis.transport.set-position", 0),
            ResourceScope::Exact {
                resources: BTreeSet::new(),
            },
            ApprovalPolicy::DelegatedApply,
            budget,
        );
        let req = bind_ok(&mut rig, &transport_wire(&spine, true, true), agent_caller(&spine));
        let desc = descriptor_of(&rig, "synvelis.transport.set-position", 0);
        let commits = budget.map_or(6, |b| usize::try_from(b).unwrap_or(8) + 2).min(8);
        let mut permits = 0u64;
        for _ in 0..commits {
            let decision = rig.policy.evaluate_commit(&req, &desc);
            assert_closed(&decision);
            let consumed = rig
                .store
                .grant(&grant_ref(&spine))
                .expect("grant issued")
                .consumed_applies;
            match &decision {
                AuthorizationDecision::PermitForStage => {
                    permits += 1;
                    prop_assert_eq!(consumed, permits, "each permit spends exactly one unit");
                }
                AuthorizationDecision::Deny { reason } => {
                    prop_assert_eq!(reason.as_str(), BUDGET_EXHAUSTED);
                    prop_assert_eq!(consumed, permits, "deny must not move consumption");
                }
                AuthorizationDecision::ApprovalRequired => {
                    prop_assert!(false, "delegated apply never asks for approval");
                }
            }
            if let Some(b) = budget {
                prop_assert!(consumed <= b, "consumption never exceeds budget");
            }
        }
        if let Some(b) = budget {
            prop_assert_eq!(permits, b, "permits saturate the budget exactly");
            // The store-level spend check agrees the budget is gone.
            prop_assert!(!rig
                .store
                .try_consume_apply(&grant_ref(&spine), PolicyInstant::new(spine.now)));
        }
    }

    /// (e) Preflight never spends: non-Commit stages and
    /// `evaluate_effects` leave `consumed_applies` at zero, and a
    /// deny decision (post-revoke) spends nothing either.
    #[test]
    fn preflight_and_deny_never_spend(spine in arb_spine()) {
        let source = ResourceRef::Source(SourceId::new(9));
        let mut rig = rig(spine.now, |oracle| {
            oracle.known(spine.project, source, source_attrs());
        });
        issue_bound(
            &rig.store,
            &spine,
            cap_ref("synvelis.transport.set-position", 0),
            ResourceScope::Exact {
                resources: BTreeSet::from([source]),
            },
            ApprovalPolicy::DelegatedApply,
            Some(1),
        );
        let req = bind_ok(&mut rig, &transport_wire(&spine, true, true), agent_caller(&spine));
        let desc = descriptor_of(&rig, "synvelis.transport.set-position", 0);
        let covered = ScopedEffects {
            reads: vec![source],
            ..ScopedEffects::default()
        };
        for stage in ALL_STAGES {
            if stage == AuthorizationStage::Commit {
                continue;
            }
            assert_closed(&rig.policy.decide(&req, stage, &desc));
            assert_closed(&rig.policy.evaluate_effects(&req, stage, &desc, &covered));
        }
        let consumed = || {
            rig.store
                .grant(&grant_ref(&spine))
                .expect("grant issued")
                .consumed_applies
        };
        prop_assert_eq!(consumed(), 0, "preflight moved the apply budget");
        // A deny decision — after the grant is revoked — spends nothing.
        rig.store.revoke_grant(&grant_ref(&spine));
        let denied = rig.policy.evaluate_commit(&req, &desc);
        assert_closed(&denied);
        prop_assert_eq!(deny_reason(&denied), REVOKED);
        prop_assert_eq!(consumed(), 0, "deny decision consumed budget");
    }

    /// (f) `evaluate_commit` is the same evaluation as
    /// `decide(Commit)`: two identical rigs observe identical
    /// decisions and identical spend.
    #[test]
    fn commit_entry_points_agree(
        spine in arb_spine(),
        budget in prop::option::of(0u64..=4),
    ) {
        let mut rigs = (
            rig(spine.now, |_| {}),
            rig(spine.now, |_| {}),
        );
        for r in [&mut rigs.0, &mut rigs.1] {
            issue_bound(
                &r.store,
                &spine,
                cap_ref("synvelis.transport.set-position", 0),
                ResourceScope::Exact {
                    resources: BTreeSet::new(),
                },
                ApprovalPolicy::DelegatedApply,
                budget,
            );
        }
        let caller = agent_caller(&spine);
        let req_a = bind_ok(&mut rigs.0, &transport_wire(&spine, true, true), caller.clone());
        let req_b = bind_ok(&mut rigs.1, &transport_wire(&spine, true, true), caller);
        let desc_a = descriptor_of(&rigs.0, "synvelis.transport.set-position", 0);
        let desc_b = descriptor_of(&rigs.1, "synvelis.transport.set-position", 0);
        let via_commit = rigs.0.policy.evaluate_commit(&req_a, &desc_a);
        let via_decide = rigs.1.policy.decide(&req_b, AuthorizationStage::Commit, &desc_b);
        assert_closed(&via_commit);
        assert_closed(&via_decide);
        prop_assert!(same_decision(&via_commit, &via_decide));
        let spent = |r: &Rig| {
            r.store
                .grant(&grant_ref(&spine))
                .expect("grant issued")
                .consumed_applies
        };
        prop_assert_eq!(spent(&rigs.0), spent(&rigs.1));
    }

    /// (a) Decision set stays closed across every approval policy —
    /// each stage produces Permit/ApprovalRequired/Deny with a
    /// contract reason.
    #[test]
    fn decisions_stay_in_closed_set(
        spine in arb_spine(),
        approval in prop::sample::select(vec![
            ApprovalPolicy::Suggest,
            ApprovalPolicy::ApproveToApply,
            ApprovalPolicy::DelegatedApply,
        ]),
    ) {
        let mut rig = rig(spine.now, |_| {});
        issue_bound(
            &rig.store,
            &spine,
            cap_ref("synvelis.transport.set-position", 0),
            ResourceScope::Exact {
                resources: BTreeSet::new(),
            },
            approval,
            Some(0),
        );
        let req = bind_ok(&mut rig, &transport_wire(&spine, true, true), agent_caller(&spine));
        let desc = descriptor_of(&rig, "synvelis.transport.set-position", 0);
        for stage in ALL_STAGES {
            assert_closed(&rig.policy.decide(&req, stage, &desc));
        }
    }

    /// (g-1) An unresolved or scope-uncovered target uniformly denies
    /// `target outside grant scope` — existence-blind.
    #[test]
    fn unresolved_or_uncovered_target_denies(
        spine in arb_spine(),
        oracle_knows in any::<bool>(),
    ) {
        let target = ResourceRef::Source(SourceId::new(9));
        let other = ResourceRef::Source(SourceId::new(10));
        let mut rig = rig(spine.now, |oracle| {
            if oracle_knows {
                oracle.known(spine.project, target, source_attrs());
            }
        });
        // Scope covers `other` — never the target — so coverage fails
        // whether or not the oracle resolves the target.
        issue_bound(
            &rig.store,
            &spine,
            cap_ref("synvelis.midi.add-note", 1),
            ResourceScope::Exact {
                resources: BTreeSet::from([other]),
            },
            ApprovalPolicy::DelegatedApply,
            Some(2),
        );
        let req = bind_ok(&mut rig, &add_note_wire(&spine, &target), agent_caller(&spine));
        let desc = descriptor_of(&rig, "synvelis.midi.add-note", 1);
        for stage in ALL_STAGES {
            let decision = rig.policy.decide(&req, stage, &desc);
            assert_closed(&decision);
            prop_assert_eq!(deny_reason(&decision), OUTSIDE_SCOPE);
        }
    }

    /// (g-2) Create-target defects deny `creation outside grant
    /// constraints`: no `create_within`, unresolvable parent,
    /// disallowed parent, or disallowed created kind.
    #[test]
    fn create_target_defects_deny(spine in arb_spine(), defect in 0u8..4) {
        let parent = ResourceRef::Project(spine.project);
        let mut rig = rig(spine.now, |oracle| {
            if defect != 1 {
                oracle.known(
                    spine.project,
                    parent,
                    ResourceAttributes {
                        kind: ResourceKind::Project,
                        owner: None,
                        range: None,
                        created_revision: Some(ProjectRevision(0)),
                        related: vec![],
                    },
                );
            }
        });
        let capability = cap_ref("synvelis.midi.create-source", 0);
        issue_task(&rig.store, &spine);
        let mut grant = wide_grant(
            &spine,
            capability,
            ResourceScope::Exact {
                resources: BTreeSet::new(),
            },
            ApprovalPolicy::DelegatedApply,
            None,
        );
        grant.create_within = match defect {
            0 => None,
            1 => Some(CreateWithin {
                parents: OwnerConstraint::Any,
                kinds: BTreeSet::from([ResourceKind::Source]),
                placement: None,
            }),
            2 => Some(CreateWithin {
                parents: OwnerConstraint::OneOf {
                    refs: BTreeSet::from([ResourceRef::Source(SourceId::new(77))]),
                },
                kinds: BTreeSet::from([ResourceKind::Source]),
                placement: None,
            }),
            _ => Some(CreateWithin {
                parents: OwnerConstraint::Any,
                kinds: BTreeSet::from([ResourceKind::Clip]),
                placement: None,
            }),
        };
        rig.store.issue_grant(grant);
        let req = bind_ok(&mut rig, &create_source_wire(&spine), agent_caller(&spine));
        let desc = descriptor_of(&rig, "synvelis.midi.create-source", 0);
        let decision = rig.policy.decide(&req, AuthorizationStage::RequestApply, &desc);
        assert_closed(&decision);
        prop_assert_eq!(deny_reason(&decision), OUTSIDE_CREATE);
    }

    /// (g-3) Unclassifiable effects never auto-permit; disclosures
    /// and provider transfers outside `data_access` deny; an
    /// uncovered effect read denies scope — the effect pass rechecks
    /// what the operation actually touches.
    #[test]
    fn effects_recheck_actual_resources_and_classes(
        spine in arb_spine(),
        defect in prop::sample::select(vec!["unsupported", "disclosure", "transfer", "uncovered"]),
    ) {
        let covered_source = ResourceRef::Source(SourceId::new(9));
        let stray = ResourceRef::Source(SourceId::new(11));
        let mut rig = rig(spine.now, |oracle| {
            oracle.known(spine.project, covered_source, source_attrs());
            // `stray` exists in the project but outside the grant.
            oracle.known(spine.project, stray, source_attrs());
        });
        let mut grant = wide_grant(
            &spine,
            cap_ref("synvelis.transport.set-position", 0),
            ResourceScope::Exact {
                resources: BTreeSet::from([covered_source]),
            },
            ApprovalPolicy::DelegatedApply,
            None,
        );
        // Narrow data access so a disclosure/transfer can exceed it.
        grant.data_access = DataAccessSet::of([DataAccessClass::MetadataRead]);
        issue_task(&rig.store, &spine);
        rig.store.issue_grant(grant);
        let req = bind_ok(&mut rig, &transport_wire(&spine, true, true), agent_caller(&spine));
        let desc = descriptor_of(&rig, "synvelis.transport.set-position", 0);
        let effects = match defect {
            "unsupported" => ScopedEffects {
                unsupported: vec!["opaque".to_owned()],
                ..ScopedEffects::default()
            },
            "disclosure" => ScopedEffects {
                disclosures: vec![DataAccessClass::ExternalDisclosure],
                ..ScopedEffects::default()
            },
            "transfer" => ScopedEffects {
                external_transfers: 1,
                ..ScopedEffects::default()
            },
            _ => ScopedEffects {
                reads: vec![covered_source, stray],
                ..ScopedEffects::default()
            },
        };
        let expected = match defect {
            "unsupported" => UNVERIFIABLE,
            "uncovered" => OUTSIDE_SCOPE,
            _ => DATA_ACCESS_OUTSIDE,
        };
        let decision = rig.policy.evaluate_effects(
            &req,
            AuthorizationStage::RequestApply,
            &desc,
            &effects,
        );
        assert_closed(&decision);
        prop_assert_eq!(deny_reason(&decision), expected);
    }

    /// (g-4) Scope coverage is invariant under repeated evaluation
    /// and resource overlap/deduplication: duplicated effect members
    /// decide identically to the deduplicated set, and repeated
    /// non-commit evaluations are stable.
    #[test]
    fn coverage_stable_under_repeat_and_dedup(spine in arb_spine()) {
        let source = ResourceRef::Source(SourceId::new(9));
        let mut rig = rig(spine.now, |oracle| {
            oracle.known(spine.project, source, source_attrs());
        });
        issue_bound(
            &rig.store,
            &spine,
            cap_ref("synvelis.midi.add-note", 1),
            ResourceScope::Exact {
                resources: BTreeSet::from([source]),
            },
            ApprovalPolicy::DelegatedApply,
            Some(4),
        );
        let req = bind_ok(&mut rig, &add_note_wire(&spine, &source), agent_caller(&spine));
        let desc = descriptor_of(&rig, "synvelis.midi.add-note", 1);
        let duplicated = ScopedEffects {
            reads: vec![source, source, source],
            writes: vec![source],
            ..ScopedEffects::default()
        };
        let deduplicated = ScopedEffects {
            reads: vec![source],
            writes: vec![source],
            ..ScopedEffects::default()
        };
        let stage = AuthorizationStage::PrepareProposal;
        let d1 = rig.policy.evaluate_effects(&req, stage, &desc, &duplicated);
        let d2 = rig.policy.evaluate_effects(&req, stage, &desc, &deduplicated);
        assert_closed(&d1);
        assert_closed(&d2);
        prop_assert!(same_decision(&d1, &d2), "overlap changed the verdict");
        let first = rig.policy.decide(&req, stage, &desc);
        let second = rig.policy.decide(&req, stage, &desc);
        assert_closed(&first);
        prop_assert!(same_decision(&first, &second), "re-evaluation drifted");
    }

    /// Grant-dimension denies stay inside the contract set and name
    /// the dimension — revoked, expired, superseded, capability,
    /// permission, or data-access outside the grant.
    #[test]
    fn grant_dimension_denies_stay_in_contract(
        spine in arb_spine(),
        defect in prop::sample::select(vec![
            "revoked", "expired", "superseded", "capability", "permissions", "data_access",
        ]),
    ) {
        let mut rig = rig(spine.now, |_| {});
        issue_task(&rig.store, &spine);
        let mut grant = wide_grant(
            &spine,
            cap_ref("synvelis.midi.add-note", 1),
            ResourceScope::Exact {
                resources: BTreeSet::new(),
            },
            ApprovalPolicy::DelegatedApply,
            None,
        );
        let expected = match defect {
            "revoked" => {
                grant.validity.revoked = true;
                "grant revoked"
            }
            "expired" => {
                grant.validity.expires_at = Some(PolicyInstant::new(0));
                "grant expired"
            }
            "superseded" => {
                rig.store.update_policy();
                grant.epoch_ceiling = Some(PolicyEpoch(0));
                "policy version superseded"
            }
            "capability" => {
                grant.capabilities = BTreeSet::from([cap_ref("synvelis.transport.set-position", 0)]);
                "capability outside grant"
            }
            "permissions" => {
                grant.permissions = PermissionSet::of([PermissionKind::ReadContext]);
                "required permissions outside grant"
            }
            _ => {
                grant.data_access = DataAccessSet::of([DataAccessClass::DerivedDataRead]);
                "data access outside grant"
            }
        };
        rig.store.issue_grant(grant);
        let req = bind_ok(
            &mut rig,
            &add_note_wire(&spine, &ResourceRef::Source(SourceId::new(9))),
            agent_caller(&spine),
        );
        let desc = descriptor_of(&rig, "synvelis.midi.add-note", 1);
        let decision = rig.policy.decide(&req, AuthorizationStage::RequestApply, &desc);
        assert_closed(&decision);
        prop_assert_eq!(deny_reason(&decision), expected);
    }
}
