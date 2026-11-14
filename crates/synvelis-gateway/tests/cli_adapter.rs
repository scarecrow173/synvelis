//! AUTX-02 fail-closed fixtures — Issue #261 acceptance criteria.
//!
//! Each test names the security invariant it proves:
//!
//! - `CallerIdentity`/`AdapterSeal` cannot be minted from wire data,
//!   CLI arguments, environment variables, TTY state, `--yes`, or
//!   script input — only the crate's three seal issuers produce one.
//! - Unattended local execution is always agent-equivalent
//!   (`UnattendedDelegated`) and never reaches the unconditional
//!   human authorization path; verified interactive sessions are the
//!   only local route to `Human`.
//! - Delegated authority is ordinary GW-02 grant/task authority:
//!   scope, validity, revocation, approval policy, and apply budget
//!   are re-evaluated on every call — nothing about the CLI origin
//!   weakens or strengthens them.
//! - `HeadlessSessionHost` composes the existing gateway/policy seams
//!   without GUI dependencies and without a second source of truth.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::fixture::{self, FixtureAdapter, StaticOracle};
use synvelis_gateway::mcp::CONNECT_METHOD;
use synvelis_gateway::*;
use synvelis_project::{ProjectId, ProjectRevision, SourceId};

// ── Fixture ids ──────────────────────────────────────────────

fn project() -> ProjectId {
    ProjectId::new(7)
}
fn source9() -> ResourceRef {
    ResourceRef::Source(SourceId::new(9))
}
fn source10() -> ResourceRef {
    ResourceRef::Source(SourceId::new(10))
}
fn task_ref(tag: &str) -> TaskRef {
    TaskRef::new(tag).unwrap()
}
fn grant_ref(tag: &str) -> GrantRef {
    GrantRef::new(tag).unwrap()
}
fn cap_ref(id: &str, major: u32, minor: u32) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id).unwrap(),
        version: CapabilityVersion::new(major, minor),
    }
}

// ── Principals / callers ─────────────────────────────────────

fn human_principal() -> Principal {
    Principal::new(PrincipalKind::Human, "user-1").unwrap()
}
fn mcp_agent_principal() -> Principal {
    Principal::new(PrincipalKind::Agent, "mcp-client-1").unwrap()
}
fn ci_principal() -> Principal {
    Principal::new(PrincipalKind::Agent, "ci-runner-1").unwrap()
}
fn human() -> CallerIdentity {
    FixtureAdapter::for_testing().human("user-1").unwrap()
}
fn fixture_agent() -> CallerIdentity {
    FixtureAdapter::for_testing().agent("mcp-client-1").unwrap()
}

// ── Policy rig ───────────────────────────────────────────────
//
// A gateway + evaluator sharing one authority store/clock/oracle —
// the same shape `tests/authorization.rs` uses. `host()` builds a
// `HeadlessSessionHost` over a *second* gateway wired to the same
// authority, proving the composition adds no second source of truth.

struct Rig {
    store: Arc<InMemoryPolicyStore>,
    clock: Arc<ManualClock>,
    oracle: Arc<StaticOracle>,
    policy: GrantPolicy,
    gw: Gateway,
}

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

fn standard_facts(oracle: &mut StaticOracle) {
    oracle.known(
        project(),
        ResourceRef::Project(project()),
        attrs(ResourceKind::Project, None, None, Some(0)),
    );
    for source in [source9(), source10()] {
        oracle.known(
            project(),
            source,
            attrs(
                ResourceKind::Source,
                Some(ResourceRef::Project(project())),
                None,
                Some(1),
            ),
        );
    }
}

fn wired_gateway(
    store: &Arc<InMemoryPolicyStore>,
    clock: &Arc<ManualClock>,
    oracle: &Arc<StaticOracle>,
) -> Gateway {
    let mut gw = Gateway::with_evaluator(Box::new(GrantPolicy::new(
        store.clone(),
        clock.clone(),
        oracle.clone(),
    )));
    fixture::install(&mut gw).unwrap();
    gw
}

fn rig() -> Rig {
    let store = Arc::new(InMemoryPolicyStore::new());
    let clock = Arc::new(ManualClock::new(PolicyInstant::new(1_000)));
    let mut oracle = StaticOracle::new();
    standard_facts(&mut oracle);
    let oracle = Arc::new(oracle);
    let policy = GrantPolicy::new(store.clone(), clock.clone(), oracle.clone());
    let gw = wired_gateway(&store, &clock, &oracle);
    Rig {
        store,
        clock,
        oracle,
        policy,
        gw,
    }
}

impl Rig {
    /// A headless host over a fresh gateway sharing this rig's
    /// authority — the host adds composition, not policy.
    fn host(&self) -> HeadlessSessionHost {
        HeadlessSessionHost::without_commit_boundary(
            wired_gateway(&self.store, &self.clock, &self.oracle),
            Box::new(MemoryCliSessionStore::new()),
            self.clock.clone(),
        )
    }
}

// ── CLI adapter / session helpers ────────────────────────────

fn cli_adapter(clock: &Arc<ManualClock>) -> LocalCliAdapter {
    LocalCliAdapter::new(
        AdapterId::new(CLI_ADAPTER_RAW),
        Box::new(MemoryCliSessionStore::new()),
        clock.clone(),
    )
}

fn human_session(adapter: &LocalCliAdapter) -> CliSessionIssue {
    adapter
        .issue_session(CliSessionSpec {
            principal: human_principal(),
            ttl_ms: None,
        })
        .expect("human session issues")
}

// ── Authority builders ───────────────────────────────────────

fn issue_task_for(store: &InMemoryPolicyStore, tag: &str, principal: &Principal) {
    store.issue_task(AuthorizedTask {
        task: task_ref(tag),
        principal: principal.clone(),
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
}

/// A delegated-apply grant bound to `principal` under `task_tag` —
/// the unattended-CI authority shape: agent-equivalent principal,
/// one capability, exact source-9 scope.
fn delegated_grant(tag: &str, task_tag: &str, principal: &Principal) -> Grant {
    Grant {
        id: grant_ref(tag),
        issued_by: human_principal(),
        principal: principal.clone(),
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

fn issue_delegated_authority(
    store: &InMemoryPolicyStore,
    task_tag: &str,
    grant_tag: &str,
    principal: &Principal,
) {
    issue_task_for(store, task_tag, principal);
    store.issue_grant(delegated_grant(grant_tag, task_tag, principal));
}

// ── Request builders ─────────────────────────────────────────

fn add_note_req(task: Option<&str>, grant: Option<&str>, source: ResourceRef) -> Value {
    let mut req = json!({
        "request_id": "req-cli-1",
        "capability": {"id": "synvelis.midi.add-note", "version": {"major": 1, "minor": 1}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": project().to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": tagged(source)},
        "args": {"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 240, "len": 480}},
        "consistency": {"kind": "expected-revision", "revision": 10},
    });
    if let Some(t) = task {
        req["task"] = json!(t);
    }
    if let Some(g) = grant {
        req["grant"] = json!(g);
    }
    req
}

fn tagged(r: ResourceRef) -> String {
    match r {
        ResourceRef::Project(id) => id.to_tagged(),
        ResourceRef::Source(id) => id.to_tagged(),
        _ => unimplemented!("fixture kinds"),
    }
}

fn bind(rig: &mut Rig, req: &Value, caller: CallerIdentity) -> OperationRequest {
    let wire = Gateway::parse_wire(req).unwrap();
    rig.gw.bind(wire, caller).unwrap()
}

fn deny_reason(env: &ErrorEnvelope) -> &str {
    match &env.code {
        GatewayError::Denied { reason, .. } => reason.as_deref().unwrap_or(""),
        other => panic!("expected denied, got {other:?}"),
    }
}

const NO_AUTH: &str = "no applicable authorization";
const OUTSIDE_SCOPE: &str = "target outside grant scope";

// ════════════════════════════════════════════════════════════
// Caller classification — the five independent lanes (AC: fixture
// classification of Verified Human / Delegated CI / Agent / External
// MCP / Test Fixture)
// ════════════════════════════════════════════════════════════

#[test]
fn class_fixture_origins_classify_by_kind_and_origin() {
    // Test Fixture lane: the fixture adapter's human is verified-human
    // authority; its agent is the fixture agent lane.
    let human = human();
    assert_eq!(human.origin(), OriginKind::TestFixture);
    assert_eq!(human.authority_class(), CallerClass::VerifiedHuman);

    let agent = fixture_agent();
    assert_eq!(agent.origin(), OriginKind::TestFixture);
    assert_eq!(agent.authority_class(), CallerClass::FixtureAgent);
}

#[test]
fn class_delegated_local_cli_is_unattended_delegated() {
    let rig = rig();
    let adapter = cli_adapter(&rig.clock);
    let caller = adapter.caller_delegated("ci-runner-1").unwrap();
    assert_eq!(caller.principal().kind(), PrincipalKind::Agent);
    assert_eq!(caller.principal().id(), "ci-runner-1");
    assert_eq!(caller.origin(), OriginKind::LocalCli);
    assert_eq!(caller.authority_class(), CallerClass::UnattendedDelegated);
}

#[test]
fn class_verified_session_is_verified_human_on_local_cli() {
    let rig = rig();
    let adapter = cli_adapter(&rig.clock);
    let issue = human_session(&adapter);
    let caller = adapter
        .caller_verified(&issue.secret)
        .expect("live credential verifies");
    assert_eq!(caller.principal().kind(), PrincipalKind::Human);
    assert_eq!(caller.principal().id(), "user-1");
    assert_eq!(caller.origin(), OriginKind::LocalCli);
    assert_eq!(caller.authority_class(), CallerClass::VerifiedHuman);
}

#[test]
fn class_mcp_channels_classify_client_agent_and_control_human() {
    // External MCP lane — a minimal McpHost proves the permit-bound
    // callers land in the expected classes.
    let clock = Arc::new(ManualClock::new(PolicyInstant::new(1_000)));
    let host = McpHost::new(
        Gateway::new(),
        Box::new(MemoryPermitStore::new()),
        Arc::new(NoCommitPort),
        Arc::new(NoAdminPort),
        clock.clone(),
        McpConfig {
            enabled: true,
            ..McpConfig::default()
        },
    );

    // Client attests ExternalAdapter; Control is the PoC stand-in for
    // the GUI and attests ManualUi — both minted through McpHost's seal.
    for (channel, principal, want_kind, want_origin, want_class) in [
        (
            McpChannel::Client,
            mcp_agent_principal(),
            PrincipalKind::Agent,
            OriginKind::ExternalAdapter,
            CallerClass::ExternalAgent,
        ),
        (
            McpChannel::Control,
            human_principal(),
            PrincipalKind::Human,
            OriginKind::ManualUi,
            CallerClass::VerifiedHuman,
        ),
    ] {
        let issue = host
            .issue_permit(PermitSpec {
                channel,
                principal,
                project: project(),
                task: None,
                grant: None,
                ttl_ms: None,
            })
            .expect("permit issues");
        let mut session = host.new_session(channel);
        let resp = host
            .handle(
                &mut session,
                &json!({
                    "jsonrpc": "2.0", "id": 1, "method": CONNECT_METHOD,
                    "params": {"permit": issue.secret},
                }),
            )
            .expect("connect responds");
        assert!(resp.get("error").is_none(), "connect failed: {resp}");
        let bound = session.bound().expect("session bound");
        assert_eq!(bound.caller.principal().kind(), want_kind);
        assert_eq!(bound.caller.origin(), want_origin);
        assert_eq!(bound.caller.authority_class(), want_class);
    }
}

// ════════════════════════════════════════════════════════════
// Fail-closed minting — nothing caller-controlled reaches Human
// ════════════════════════════════════════════════════════════

#[test]
fn unattended_claim_cannot_mint_human_regardless_of_principal_id() {
    // The delegated path takes only a principal *id string* — even an
    // id that looks like a human account stays Agent. There is no
    // parameter that selects PrincipalKind.
    let rig = rig();
    let adapter = cli_adapter(&rig.clock);
    for id in ["user-1", "human", "admin", "ci-runner-1"] {
        let claim = CliSessionClaim::Unattended {
            principal: id.to_owned(),
        };
        let caller = adapter.attest(&claim).unwrap();
        assert_eq!(caller.principal().kind(), PrincipalKind::Agent, "{id}");
        assert_eq!(caller.authority_class(), CallerClass::UnattendedDelegated);
    }
    // Direct path agrees.
    let caller = adapter.caller_delegated("user-1").unwrap();
    assert_eq!(caller.principal().kind(), PrincipalKind::Agent);
}

#[test]
fn interactive_claim_requires_a_verified_credential() {
    let rig = rig();
    let adapter = cli_adapter(&rig.clock);
    let issue = human_session(&adapter);

    // Unknown, malformed, and wrong-prefix credentials share the one
    // blind Authentication verdict — a probe learns nothing.
    for bad in [
        "syv-cli-0000000000000000000000000000000000000000000000000000000000000000",
        "not-a-credential",
        "syv-0000000000000000000000000000000000000000000000000000000000000000",
        "",
    ] {
        let err = adapter
            .caller_verified(bad)
            .expect_err("bad credential must fail");
        assert_eq!(err, GatewayError::Authentication, "{bad}");
        let claim = CliSessionClaim::Interactive {
            credential: bad.to_owned(),
        };
        assert_eq!(
            adapter.attest(&claim).expect_err("claim must fail"),
            GatewayError::Authentication,
            "{bad}"
        );
    }
    // The real credential verifies — and only through the store.
    assert!(adapter.caller_verified(&issue.secret).is_ok());
}

#[test]
fn session_credentials_fail_closed_on_expiry_and_revocation() {
    let rig = rig();
    let adapter = cli_adapter(&rig.clock);
    let issue = adapter
        .issue_session(CliSessionSpec {
            principal: human_principal(),
            ttl_ms: Some(500),
        })
        .expect("session issues");
    assert!(adapter.caller_verified(&issue.secret).is_ok());

    // TTL expiry — the policy clock is the only clock.
    rig.clock.set(PolicyInstant::new(1_600));
    assert_eq!(
        adapter
            .caller_verified(&issue.secret)
            .expect_err("expired credential must fail"),
        GatewayError::Authentication
    );

    // Revocation is a recorded state — a live credential dies too.
    rig.clock.set(PolicyInstant::new(1_000));
    let issue2 = adapter
        .issue_session(CliSessionSpec {
            principal: human_principal(),
            ttl_ms: None,
        })
        .expect("second session issues");
    assert!(adapter.revoke_session(&issue2.record.session).unwrap());
    assert_eq!(
        adapter
            .caller_verified(&issue2.secret)
            .expect_err("revoked credential must fail"),
        GatewayError::Authentication
    );
    // The record stays — revocation is not a delete.
    let record = adapter
        .session_record(&issue2.record.session)
        .expect("record retained");
    assert!(record.revoked);
    assert!(!record.live(rig.clock.now()));
}

#[test]
fn sessions_bind_human_principals_only() {
    // The session credential is the *only* local path to Human — an
    // agent-bound session would be a confused identity, so issuance
    // rejects it outright (and verify would re-check regardless).
    let rig = rig();
    let adapter = cli_adapter(&rig.clock);
    for principal in [mcp_agent_principal(), ci_principal()] {
        let err = adapter
            .issue_session(CliSessionSpec {
                principal,
                ttl_ms: None,
            })
            .expect_err("non-human session must not issue");
        assert!(matches!(err.code, GatewayError::MalformedRequest { .. }));
    }
    // check_session_kind agrees at the contract surface.
    assert!(
        check_session_kind(&CliSessionSpec {
            principal: human_principal(),
            ttl_ms: None,
        })
        .is_ok()
    );
    assert!(
        check_session_kind(&CliSessionSpec {
            principal: ci_principal(),
            ttl_ms: None,
        })
        .is_err()
    );
}

#[test]
fn wire_payloads_cannot_assert_authority_or_identity() {
    // GT-002/AUTX-02: every authority-asserting field is rejected at
    // parse — a CLI argument vector or script payload smuggling
    // "principal"/"--as-human"/"yes" fields never reaches bind.
    // `parse_wire` is caller-independent by construction: the trusted
    // caller is always supplied separately to `bind`.
    let base = add_note_req(None, None, source9());
    for field in [
        "principal",
        "caller",
        "authority",
        "auth",
        "authenticated",
        "token",
        "approved",
        "approval",
        "impersonate",
        "as_user",
        "role",
    ] {
        let mut req = base.clone();
        req[field] = json!("user-1");
        let err = Gateway::parse_wire(&req).expect_err("authority field must fail");
        assert!(
            matches!(err.code, GatewayError::SelfDeclaredAuthority { .. }),
            "{field}: {err:?}"
        );
    }
    // Unknown fields (e.g. a doctored "--yes"/"tty" flag smuggled into
    // JSON) fail closed at parse too — the wire shape is closed.
    for field in ["yes", "tty", "as_human", "human", "interactive"] {
        let mut req = base.clone();
        req[field] = json!(true);
        assert!(Gateway::parse_wire(&req).is_err(), "{field} must not parse");
    }
}

#[test]
fn session_records_never_contain_the_usable_secret() {
    let rig = rig();
    let adapter = cli_adapter(&rig.clock);
    let issue = human_session(&adapter);

    // The record serializes for durable stores — and carries only the
    // digest, never the credential.
    let value = serde_json::to_value(&issue.record).unwrap();
    let text = value.to_string();
    assert!(!text.contains(&issue.secret), "secret leaked: {text}");
    let hash = value["secret_hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
    assert_ne!(hash, issue.secret);

    // Introspection returns the same digest-only record.
    let record = adapter.session_record(&issue.record.session).unwrap();
    assert_eq!(record.principal.id(), "user-1");
    assert!(record.live(rig.clock.now()));

    // Two sessions get distinct secrets — no reuse, no derivation.
    let other = human_session(&adapter);
    assert_ne!(issue.secret, other.secret);
    assert_ne!(issue.record.session, other.record.session);
}

#[test]
fn claims_deserialize_fail_closed() {
    // A claim arriving as JSON can name a lane and a principal/credential
    // — nothing else. Unknown fields and unknown kinds are rejected.
    let unattended: CliSessionClaim =
        serde_json::from_str(r#"{"kind":"unattended","principal":"ci-9"}"#).unwrap();
    assert_eq!(
        unattended,
        CliSessionClaim::Unattended {
            principal: "ci-9".to_owned()
        }
    );
    assert!(
        serde_json::from_str::<CliSessionClaim>(r#"{"kind":"human","principal":"user-1"}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<CliSessionClaim>(
            r#"{"kind":"unattended","principal":"ci-9","as_human":true}"#
        )
        .is_err()
    );
}

// ════════════════════════════════════════════════════════════
// Delegated authorization — ordinary GW-02 evaluation per call
// ════════════════════════════════════════════════════════════

#[test]
fn delegated_without_authority_never_reaches_the_human_path() {
    // The core AUTX-02 invariant: an unattended local caller with no
    // task/grant is denied exactly like any other agent — the
    // unconditional human path is unreachable from this origin.
    let mut rig = rig();
    let host = rig.host();
    let claim = CliSessionClaim::Unattended {
        principal: "ci-runner-1".to_owned(),
    };
    let req = add_note_req(None, None, source9());
    let err = host
        .handle_claim(&req, &claim)
        .expect_err("unauthorized delegated call must fail");
    assert_eq!(deny_reason(&err), NO_AUTH);

    // Same verdict at the decision layer for a directly-bound request.
    let caller = cli_adapter(&rig.clock)
        .caller_delegated("ci-runner-1")
        .unwrap();
    let bound = bind(&mut rig, &req, caller);
    let d = rig.policy.decide(
        &bound,
        AuthorizationStage::RequestApply,
        rig.gw
            .registry()
            .describe(&cap_ref("synvelis.midi.add-note", 1, 1))
            .unwrap(),
    );
    assert_eq!(
        match d {
            AuthorizationDecision::Deny { reason } => reason,
            other => panic!("expected deny, got {other:?}"),
        },
        NO_AUTH
    );
}

#[test]
fn delegated_with_task_and_grant_is_permitted_like_any_agent() {
    // Delegated execution is not *denied by origin* — it is authorized
    // by the same task+grant contract every agent uses. This is the
    // legitimate unattended path: explicit, scoped, revocable.
    let rig = rig();
    issue_delegated_authority(&rig.store, "task-ci", "grant-ci", &ci_principal());
    let host = rig.host();
    let claim = CliSessionClaim::Unattended {
        principal: "ci-runner-1".to_owned(),
    };
    let req = add_note_req(Some("task-ci"), Some("grant-ci"), source9());
    let outcome = host.handle_claim(&req, &claim).expect("delegated call");
    assert!(matches!(outcome, GatewayOutcome::Completed { .. }));
}

#[test]
fn delegated_authority_is_bound_to_its_principal() {
    // A grant issued for one agent id does not authorize another —
    // claiming a different principal string on the claim gains nothing.
    let rig = rig();
    issue_delegated_authority(&rig.store, "task-ci", "grant-ci", &ci_principal());
    let host = rig.host();
    for impostor in ["ci-runner-2", "mcp-client-1", "user-1"] {
        let claim = CliSessionClaim::Unattended {
            principal: impostor.to_owned(),
        };
        let req = add_note_req(Some("task-ci"), Some("grant-ci"), source9());
        let err = host
            .handle_claim(&req, &claim)
            .expect_err("foreign principal must fail");
        assert_eq!(deny_reason(&err), NO_AUTH, "{impostor}");
    }
}

#[test]
fn delegated_scope_is_evaluated_per_call() {
    // The grant's exact scope (source-9) is checked on every request —
    // a delegated call outside scope is denied, not silently widened.
    let rig = rig();
    issue_delegated_authority(&rig.store, "task-ci", "grant-ci", &ci_principal());
    let host = rig.host();
    let claim = CliSessionClaim::Unattended {
        principal: "ci-runner-1".to_owned(),
    };
    let req = add_note_req(Some("task-ci"), Some("grant-ci"), source10());
    let err = host
        .handle_claim(&req, &claim)
        .expect_err("out-of-scope call must fail");
    assert_eq!(deny_reason(&err), OUTSIDE_SCOPE);
}

#[test]
fn delegated_grant_revocation_stops_the_next_call() {
    // Revocation is evaluated per call — a grant revoked between two
    // identical requests changes the second verdict. No caching, no
    // session-pinned authority.
    let rig = rig();
    issue_delegated_authority(&rig.store, "task-ci", "grant-ci", &ci_principal());
    let host = rig.host();
    let claim = CliSessionClaim::Unattended {
        principal: "ci-runner-1".to_owned(),
    };
    let req = add_note_req(Some("task-ci"), Some("grant-ci"), source9());
    assert!(matches!(
        host.handle_claim(&req, &claim).unwrap(),
        GatewayOutcome::Completed { .. }
    ));

    rig.store.revoke_grant(&grant_ref("grant-ci"));
    let err = host
        .handle_claim(&req, &claim)
        .expect_err("revoked grant must deny");
    assert_eq!(deny_reason(&err), "grant revoked");
}

#[test]
fn delegated_task_revocation_stops_the_next_call() {
    let rig = rig();
    issue_delegated_authority(&rig.store, "task-ci", "grant-ci", &ci_principal());
    let host = rig.host();
    let claim = CliSessionClaim::Unattended {
        principal: "ci-runner-1".to_owned(),
    };
    let req = add_note_req(Some("task-ci"), Some("grant-ci"), source9());
    assert!(matches!(
        host.handle_claim(&req, &claim).unwrap(),
        GatewayOutcome::Completed { .. }
    ));

    rig.store.revoke_task(&task_ref("task-ci"));
    let err = host
        .handle_claim(&req, &claim)
        .expect_err("revoked task must deny");
    assert_eq!(deny_reason(&err), NO_AUTH);
}

#[test]
fn delegated_grant_expiry_is_enforced_by_the_policy_clock() {
    let rig = rig();
    issue_task_for(&rig.store, "task-ci", &ci_principal());
    let mut grant = delegated_grant("grant-ci", "task-ci", &ci_principal());
    grant.validity.expires_at = Some(PolicyInstant::new(1_500));
    rig.store.issue_grant(grant);
    let host = rig.host();
    let claim = CliSessionClaim::Unattended {
        principal: "ci-runner-1".to_owned(),
    };
    let req = add_note_req(Some("task-ci"), Some("grant-ci"), source9());
    assert!(matches!(
        host.handle_claim(&req, &claim).unwrap(),
        GatewayOutcome::Completed { .. }
    ));

    rig.clock.set(PolicyInstant::new(1_600));
    let err = host
        .handle_claim(&req, &claim)
        .expect_err("expired grant must deny");
    assert_eq!(deny_reason(&err), "grant expired");
}

#[test]
fn delegated_apply_budget_is_not_bypassed() {
    // Budget: apply_budget is consumed at commit authorization —
    // evaluate_commit proves the delegated caller spends the same
    // budget any agent does.
    let mut rig = rig();
    issue_delegated_authority(&rig.store, "task-ci", "grant-ci", &ci_principal());
    let mut grant = delegated_grant("grant-budget", "task-ci", &ci_principal());
    grant.limits.apply_budget = Some(1);
    rig.store.issue_grant(grant);

    let caller = cli_adapter(&rig.clock)
        .caller_delegated("ci-runner-1")
        .unwrap();
    let bound = bind(
        &mut rig,
        &add_note_req(Some("task-ci"), Some("grant-budget"), source9()),
        caller,
    );
    let desc = rig
        .gw
        .registry()
        .describe(&cap_ref("synvelis.midi.add-note", 1, 1))
        .unwrap()
        .clone();
    assert_eq!(
        rig.policy.evaluate_commit(&bound, &desc),
        AuthorizationDecision::PermitForStage
    );
    assert_eq!(
        match rig.policy.evaluate_commit(&bound, &desc) {
            AuthorizationDecision::Deny { reason } => reason,
            other => panic!("expected budget deny, got {other:?}"),
        },
        "apply budget exhausted"
    );
}

#[test]
fn delegated_approval_policy_is_not_bypassed() {
    // Approval: an ApproveToApply grant cannot complete an apply
    // inline — the outcome is ApprovalRequired, never silent success.
    let rig = rig();
    issue_task_for(&rig.store, "task-ap", &ci_principal());
    let mut grant = delegated_grant("grant-ap", "task-ap", &ci_principal());
    grant.approval_policy = ApprovalPolicy::ApproveToApply;
    rig.store.issue_grant(grant);
    let host = rig.host();
    let claim = CliSessionClaim::Unattended {
        principal: "ci-runner-1".to_owned(),
    };
    let req = add_note_req(Some("task-ap"), Some("grant-ap"), source9());
    let outcome = host
        .handle_claim(&req, &claim)
        .expect("approval-gated call");
    assert!(
        matches!(outcome, GatewayOutcome::ApprovalRequired { .. }),
        "expected ApprovalRequired, got {outcome:?}"
    );
}

// ════════════════════════════════════════════════════════════
// Verified interactive path — the only local Human route
// ════════════════════════════════════════════════════════════

#[test]
fn verified_interactive_session_reaches_human_authority() {
    // The legitimate interactive path: a session issued after
    // verified provenance yields Human + LocalCli, which the policy
    // treats as manual authority — no grant needed.
    let rig = rig();
    let host = rig.host();
    let issue = host
        .cli()
        .issue_session(CliSessionSpec {
            principal: human_principal(),
            ttl_ms: None,
        })
        .expect("session issues");
    let claim = CliSessionClaim::Interactive {
        credential: issue.secret.clone(),
    };
    let req = add_note_req(None, None, source9());
    let outcome = host.handle_claim(&req, &claim).expect("human call");
    assert!(matches!(outcome, GatewayOutcome::Completed { .. }));

    // Revoking the session ends the authority — interaction
    // cancellation never preserves it.
    host.cli()
        .revoke_session(&issue.record.session)
        .expect("revoke");
    let err = host
        .handle_claim(&req, &claim)
        .expect_err("revoked session must fail");
    assert_eq!(err.code, GatewayError::Authentication);
}

#[test]
fn host_handle_rejects_unattended_human_shaped_requests() {
    // Even a request that *looks* human-initiated (no task/grant,
    // interactive-sounding args) is denied for the delegated caller —
    // authority comes from the attested caller, never the payload.
    let rig = rig();
    let host = rig.host();
    let claim = CliSessionClaim::Unattended {
        principal: "ci-runner-1".to_owned(),
    };
    let req = add_note_req(None, None, source9());
    let err = host
        .handle_claim(&req, &claim)
        .expect_err("delegated call must fail");
    assert_eq!(deny_reason(&err), NO_AUTH);
}

// ════════════════════════════════════════════════════════════
// Composition seams — ports and the CommitGate impls
// ════════════════════════════════════════════════════════════

#[test]
fn headless_host_exposes_the_wired_commit_and_admin_ports() {
    // The host's ports are the ones it was built with — a CommitGate
    // satisfies both, and the fail-closed defaults answer honestly.
    struct RevisionAdmin;
    impl AdminPort for RevisionAdmin {
        fn issue_task(&self, _task: AuthorizedTask) -> Result<(), ErrorEnvelope> {
            Err(GatewayError::Internal {
                reason: "not wired".to_owned(),
            }
            .into())
        }
        fn issue_grant(&self, _grant: Grant) -> Result<(), ErrorEnvelope> {
            Err(GatewayError::Internal {
                reason: "not wired".to_owned(),
            }
            .into())
        }
        fn revoke_grant(&self, _id: &GrantRef) -> Result<(), ErrorEnvelope> {
            Err(GatewayError::Internal {
                reason: "not wired".to_owned(),
            }
            .into())
        }
        fn revoke_task(&self, _id: &TaskRef) -> Result<(), ErrorEnvelope> {
            Err(GatewayError::Internal {
                reason: "not wired".to_owned(),
            }
            .into())
        }
        fn live_revision(&self, project: ProjectId) -> Option<ProjectRevision> {
            (project == ProjectId::new(7)).then_some(ProjectRevision(42))
        }
    }

    let rig = rig();
    let gw = wired_gateway(&rig.store, &rig.clock, &rig.oracle);
    let host = HeadlessSessionHost::new(
        gw,
        Box::new(MemoryCliSessionStore::new()),
        Arc::new(NoCommitPort),
        Arc::new(RevisionAdmin),
        rig.clock.clone(),
        AdapterId::new(CLI_ADAPTER_RAW),
    );
    assert_eq!(
        host.admin().live_revision(project()),
        Some(ProjectRevision(42))
    );
    assert_eq!(host.admin().live_revision(ProjectId::new(8)), None);
    // NoCommitPort answers honestly — no fabricated commit authority.
    let err = host.committer().commit(&ProposalRef::new("p-1").unwrap());
    assert!(err.is_err());
}

#[test]
fn commit_gate_satisfies_both_adapter_ports() {
    // Compile-time proof the real commit boundary plugs into the port
    // seams directly — no divergent wrapper.
    fn assert_ports<T: CommitPort + AdminPort>() {}
    assert_ports::<CommitGate<MutexFence>>();
}
