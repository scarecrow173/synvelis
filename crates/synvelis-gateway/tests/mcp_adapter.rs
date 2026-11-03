//! GW-08 MCP adapter tests — Issue #59 acceptance criteria (in-process layer).
//!
//! Each test names the contract it proves: permit-bound trusted
//! identity (never `clientInfo`/wire claims), channel separation,
//! per-call re-authorization, argument/bound hygiene at the protocol
//! edge, the three MIDI approval modes through the real gateway
//! pipeline, and scoped operation bookkeeping.
//!
//! These tests exercise the real `Gateway`/`PrepareEngine`/
//! `ProposalEngine`/`ContextProjectionEngine` stack; only the commit
//! boundary is scripted (a real `CommitGate` drives the
//! transport-level E2E in `mcp-poc`, so verdict honesty is covered
//! against durable truth there).

mod rig;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::mcp::*;
use synvelis_gateway::mcp_permit::check_channel_kind;
use synvelis_gateway::*;
use synvelis_project::ProjectRevision;

// ── Scripted seams ───────────────────────────────────────────

/// A scripted commit boundary — records what the tool layer asked
/// and answers what reconcile would have proven, so the apply
/// tail's verdict honesty is testable without a full `CommitGate`
/// (the real gate runs the transport-level E2E).
#[derive(Default)]
struct ScriptedCommit {
    committed: Mutex<Vec<ProposalRef>>,
    cancelled: Mutex<Vec<ProposalRef>>,
    fail_commit: Mutex<Option<ErrorEnvelope>>,
    outcomes: Mutex<BTreeMap<u128, RetryOutcome>>,
}

impl ScriptedCommit {
    fn report(&self, op: OperationId, outcome: RetryOutcome) {
        self.outcomes
            .lock()
            .expect("outcomes")
            .insert(op.raw(), outcome);
    }

    fn fail(&self, env: ErrorEnvelope) {
        *self.fail_commit.lock().expect("fail_commit") = Some(env);
    }

    fn calls(&self) -> usize {
        self.committed.lock().expect("committed").len()
    }

    fn cancelled(&self) -> Vec<ProposalRef> {
        self.cancelled.lock().expect("cancelled").clone()
    }
}

impl CommitPort for ScriptedCommit {
    fn commit(&self, proposal: &ProposalRef) -> Result<CommitReceipt, ErrorEnvelope> {
        self.committed
            .lock()
            .expect("committed")
            .push(proposal.clone());
        if let Some(env) = self.fail_commit.lock().expect("fail_commit").clone() {
            return Err(env);
        }
        Ok(CommitReceipt {
            commit: CommitId::new(9),
            operation: None,
            proposal: Some(proposal.clone()),
            principal: agent_principal(),
            project: project(),
            base_revision: ProjectRevision(1),
            revision: ProjectRevision(2),
            journal_seq: 9,
        })
    }

    fn reconcile(&self, operation: OperationId) -> RetryOutcome {
        self.outcomes
            .lock()
            .expect("outcomes")
            .get(&operation.raw())
            .cloned()
            .unwrap_or(RetryOutcome::Unknown)
    }

    fn cancel(&self, proposal: &ProposalRef, _reason: &str) -> Result<bool, ErrorEnvelope> {
        self.cancelled
            .lock()
            .expect("cancelled")
            .push(proposal.clone());
        Ok(true)
    }
}

/// A scripted admin seam — control methods reach here through
/// `synvelis.issue-*`/`revoke-*`.
#[derive(Default)]
struct ScriptedAdmin {
    tasks: Mutex<Vec<AuthorizedTask>>,
    grants: Mutex<Vec<Grant>>,
    revoked_grants: Mutex<Vec<GrantRef>>,
    revoked_tasks: Mutex<Vec<TaskRef>>,
}

impl AdminPort for ScriptedAdmin {
    fn issue_task(&self, task: AuthorizedTask) -> Result<(), ErrorEnvelope> {
        self.tasks.lock().expect("tasks").push(task);
        Ok(())
    }

    fn issue_grant(&self, grant: Grant) -> Result<(), ErrorEnvelope> {
        self.grants.lock().expect("grants").push(grant);
        Ok(())
    }

    fn revoke_grant(&self, id: &GrantRef) -> Result<(), ErrorEnvelope> {
        self.revoked_grants
            .lock()
            .expect("revoked_grants")
            .push(id.clone());
        Ok(())
    }

    fn revoke_task(&self, id: &TaskRef) -> Result<(), ErrorEnvelope> {
        self.revoked_tasks
            .lock()
            .expect("revoked_tasks")
            .push(id.clone());
        Ok(())
    }

    fn live_revision(&self, _project: synvelis_project::ProjectId) -> Option<ProjectRevision> {
        Some(ProjectRevision(1))
    }
}

// ── The adapter rig ──────────────────────────────────────────

struct McpRig {
    host: McpHost,
    store: Arc<InMemoryPolicyStore>,
    clock: Arc<ManualClock>,
    committer: Arc<ScriptedCommit>,
    admin: Arc<ScriptedAdmin>,
    /// tag → the issued secret each test presents.
    secrets: BTreeMap<String, PermitIssue>,
}

fn client_spec(task: &str, grant: &str) -> PermitSpec {
    PermitSpec {
        channel: McpChannel::Client,
        principal: agent_principal(),
        project: project(),
        task: Some(task_ref(task)),
        grant: Some(grant_ref(grant)),
        ttl_ms: None,
    }
}

fn agent2_spec() -> PermitSpec {
    PermitSpec {
        channel: McpChannel::Client,
        principal: Principal::new(PrincipalKind::Agent, "mcp-client-2").unwrap(),
        project: project(),
        task: None,
        grant: None,
        ttl_ms: None,
    }
}

fn control_spec() -> PermitSpec {
    PermitSpec {
        channel: McpChannel::Control,
        principal: human_principal(),
        project: project(),
        task: None,
        grant: None,
        ttl_ms: None,
    }
}

fn mcp_rig() -> McpRig {
    mcp_rig_enabled(true)
}

fn mcp_rig_enabled(enabled: bool) -> McpRig {
    let mut rig = fixture_rig();
    // The fixture rig registers prepare/proposal but not
    // `context.query` — the adapter's read tools need it.
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: rig.source.clone(),
    });
    let ctx = Arc::new(ContextProjectionEngine::new(
        rig.source.clone(),
        Arc::new(StaticSelection),
        Arc::new(GrantPolicy::new(
            rig.store.clone(),
            rig.clock.clone(),
            oracle,
        )),
        rig.clock.clone(),
        ContextBudget::default(),
    ));
    ctx.install(&mut rig.gw).expect("context engine installs");

    // Authority: three agents under three approval policies, plus a
    // second principal for foreign-handle tests. `midi.note.add`
    // derives a Create effect, so each grant carries the create axis
    // for notes under the MIDI source.
    let create_notes = Some(CreateWithin {
        parents: OwnerConstraint::OneOf {
            refs: BTreeSet::from([source10()]),
        },
        kinds: BTreeSet::from([ResourceKind::Note]),
        placement: None,
    });
    issue_task(&rig.store, "t1");
    let mut g1 = apply_grant(
        "g1",
        "t1",
        &["synvelis.midi.note.add"],
        ApprovalPolicy::DelegatedApply,
    );
    g1.create_within.clone_from(&create_notes);
    rig.store.issue_grant(g1);
    issue_task(&rig.store, "t-suggest");
    let mut gs = apply_grant(
        "g-suggest",
        "t-suggest",
        &["synvelis.midi.note.add"],
        ApprovalPolicy::Suggest,
    );
    gs.create_within.clone_from(&create_notes);
    rig.store.issue_grant(gs);
    issue_task(&rig.store, "t-approve");
    let mut ga = apply_grant(
        "g-approve",
        "t-approve",
        &["synvelis.midi.note.add"],
        ApprovalPolicy::ApproveToApply,
    );
    ga.create_within = create_notes;
    rig.store.issue_grant(ga);

    let now = rig.clock.now();
    let mut permits = MemoryPermitStore::new();
    let mut secrets = BTreeMap::new();
    for (tag, spec) in [
        ("agent-1", client_spec("t1", "g1")),
        ("agent-suggest", client_spec("t-suggest", "g-suggest")),
        ("agent-approve", client_spec("t-approve", "g-approve")),
        ("agent-2", agent2_spec()),
        ("control-1", control_spec()),
    ] {
        check_channel_kind(&spec).expect("spec is channel-kind clean");
        let issue = permits.issue(spec, now).expect("issue");
        secrets.insert(tag.to_owned(), issue);
    }
    // A revoked permit and an expired one — both must fail blind.
    let revoked = permits.issue(client_spec("t1", "g1"), now).expect("issue");
    permits.revoke(&revoked.record.permit).expect("revoke");
    secrets.insert("revoked".to_owned(), revoked);
    let expired = permits
        .issue(
            PermitSpec {
                ttl_ms: Some(50),
                ..client_spec("t1", "g1")
            },
            now,
        )
        .expect("issue");
    secrets.insert("expired".to_owned(), expired);

    let committer = Arc::new(ScriptedCommit::default());
    let admin = Arc::new(ScriptedAdmin::default());
    let clock: Arc<dyn PolicyClock> = rig.clock.clone();
    let host = McpHost::new(
        rig.gw,
        Box::new(permits),
        committer.clone(),
        admin.clone(),
        clock,
        McpConfig {
            adapter: AdapterId::new(MCP_ADAPTER_RAW),
            bounds: McpBounds::default(),
            enabled,
        },
    );
    McpRig {
        host,
        store: rig.store,
        clock: rig.clock,
        committer,
        admin,
        secrets,
    }
}

// ── RPC helpers ──────────────────────────────────────────────

fn rpc(rig: &McpRig, session: &mut McpSession, method: &str, params: &Value) -> Value {
    rig.host
        .handle(
            session,
            &json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}),
        )
        .expect("a request always produces a response")
}

fn rpc_error_code(v: &Value) -> i64 {
    v["error"]["code"].as_i64().expect("error.code")
}

fn rpc_result(v: &Value) -> &Value {
    assert!(
        v.get("error").is_none(),
        "expected a result, got {}",
        v["error"]
    );
    &v["result"]
}

fn connect(rig: &McpRig, session: &mut McpSession, secret: &str) -> Value {
    rpc(rig, session, CONNECT_METHOD, &json!({"permit": secret}))
}

/// A bound client session for `tag`; asserts the bind succeeded.
fn client_session(rig: &McpRig, tag: &str) -> McpSession {
    let mut session = rig.host.new_session(McpChannel::Client);
    let secret = rig.secrets[tag].secret.clone();
    let resp = connect(rig, &mut session, &secret);
    rpc_result(&resp);
    session
}

fn control_session(rig: &McpRig) -> McpSession {
    let mut session = rig.host.new_session(McpChannel::Control);
    let secret = rig.secrets["control-1"].secret.clone();
    let resp = connect(rig, &mut session, &secret);
    rpc_result(&resp);
    session
}

fn call_tool(rig: &McpRig, session: &mut McpSession, name: &str, arguments: &Value) -> Value {
    let resp = rpc(
        rig,
        session,
        "tools/call",
        &json!({"name": name, "arguments": arguments}),
    );
    rpc_result(&resp).clone()
}

fn tool_error_code(v: &Value) -> String {
    assert_eq!(v["isError"], true, "expected an error result: {v}");
    v["structuredContent"]["error"]["code"]
        .as_str()
        .expect("error.code")
        .to_owned()
}

fn tool_ok(v: &Value) -> &Value {
    assert!(v["isError"] == false, "expected a success result: {v}");
    &v["structuredContent"]
}

/// A valid `midi.edit.prepare` argument object for note.add on
/// source10 (the rig's MIDI source), asserting base revision 1.
fn prepare_args() -> Value {
    json!({
        "operation": {
            "capability": "synvelis.midi.note.add",
            "target": {"kind": "source", "id": tagged(&source10())},
            "args": {"key": 60, "velocity": 90, "range": {"domain": "source-native", "start": 720, "len": 240}},
        },
        "base_revision": 1,
    })
}

/// Prepares the standard note.add under a bound session; returns
/// (proposal, digest) from the real engine result.
fn prepare_note(rig: &McpRig, session: &mut McpSession) -> (String, String) {
    let result = call_tool(rig, session, "midi.edit.prepare", &prepare_args());
    let out = tool_ok(&result);
    (
        out["proposal"].as_str().expect("proposal").to_owned(),
        out["digest"].as_str().expect("digest").to_owned(),
    )
}

fn apply_args(proposal: &str, digest: &str, approval: Option<&str>) -> Value {
    let mut args = json!({
        "proposal": proposal,
        "digest": digest,
        "expected_revision": 1,
    });
    if let Some(a) = approval {
        args["approval"] = json!(a);
    }
    args
}

// ── Trusted identity: permit-bound, never client-claimed ─────

#[test]
fn connect_binds_record_identity_not_client_claims() {
    // GIA-004 / GT-002: the session's trusted identity is minted from
    // the permit *record* — `clientInfo`, tool arguments, and wire
    // fields can never inject a principal.
    let rig = mcp_rig();
    let mut session = rig.host.new_session(McpChannel::Client);
    rpc(
        &rig,
        &mut session,
        "initialize",
        &json!({
            "protocolVersion": PROTOCOL_VERSION,
            "clientInfo": {"name": "user-1", "version": "9.9.9"},
            "capabilities": {},
        }),
    );
    let resp = connect(&rig, &mut session, &rig.secrets["agent-1"].secret);
    let result = rpc_result(&resp);
    assert_eq!(result["status"], "bound");
    assert_eq!(
        result["principal"],
        json!({"kind": "agent", "id": "mcp-client-1"}),
        "identity is the permit record's principal — clientInfo's 'user-1' claim is ignored",
    );
}

#[test]
fn connect_rejects_invalid_secrets_with_one_blind_verdict() {
    // Unknown, revoked, expired, and wrong-channel secrets must be
    // indistinguishable — a probe learns nothing about which permits
    // exist (GIA-004 trust boundary).
    let rig = mcp_rig();
    rig.clock.advance(2_000); // age the ttl=50 permit past expiry

    let cases: Vec<(String, &str)> = vec![
        ("syv-".to_owned() + &"0".repeat(64), "unknown secret"),
        (rig.secrets["revoked"].secret.clone(), "revoked permit"),
        (rig.secrets["expired"].secret.clone(), "expired permit"),
        (
            rig.secrets["control-1"].secret.clone(),
            "control permit on client channel",
        ),
        ("not-a-permit".to_owned(), "malformed secret"),
    ];
    let mut messages = Vec::new();
    for (secret, case) in cases {
        let mut session = rig.host.new_session(McpChannel::Client);
        let resp = connect(&rig, &mut session, &secret);
        assert_eq!(rpc_error_code(&resp), PERMIT_INVALID, "{case}");
        messages.push(resp["error"]["message"].as_str().unwrap().to_owned());
    }
    assert!(
        messages.iter().all(|m| m == &messages[0]),
        "every failure shares one blind verdict: {messages:?}"
    );
}

#[test]
fn connect_channel_kind_and_duplicate_bind() {
    let rig = mcp_rig();
    // A client-kind permit presented on the control channel fails —
    // the channel binding is part of the credential.
    let mut control = rig.host.new_session(McpChannel::Control);
    let resp = connect(&rig, &mut control, &rig.secrets["agent-1"].secret);
    assert_eq!(rpc_error_code(&resp), PERMIT_INVALID);
    // A bound session cannot re-bind.
    let mut session = client_session(&rig, "agent-1");
    let resp = connect(&rig, &mut session, &rig.secrets["agent-1"].secret);
    assert_eq!(rpc_error_code(&resp), INVALID_REQUEST);
}

#[test]
fn issuance_rejects_channel_kind_mismatch() {
    // `check_channel_kind` is the fail-closed guard at issuance: a
    // Client permit can only bind an agent, a Control permit a human.
    let bad = PermitSpec {
        principal: human_principal(),
        ..client_spec("t1", "g1")
    };
    assert!(check_channel_kind(&bad).is_err());
}

// ── Discovery is not permission ──────────────────────────────

#[test]
fn discovery_requires_a_bound_session() {
    let rig = mcp_rig();
    let mut session = rig.host.new_session(McpChannel::Client);
    for (method, params) in [
        ("tools/list", &json!({})),
        (
            "tools/call",
            &json!({"name": "project.inspect", "arguments": {}}),
        ),
        ("resources/list", &json!({})),
        (
            "resources/read",
            &json!({"uri": "synvelis://project/x/inspect"}),
        ),
    ] {
        let resp = rpc(&rig, &mut session, method, params);
        assert_eq!(rpc_error_code(&resp), PERMIT_REQUIRED, "{method}");
    }
}

#[test]
fn catalog_is_channel_scoped() {
    // `proposal.approve` — the trusted-user act — is not even
    // *discoverable* on the client channel; invoking it by name is
    // refused, and a client can never mint an approval.
    let rig = mcp_rig();
    let mut client = client_session(&rig, "agent-1");
    let resp = rpc(&rig, &mut client, "tools/list", &json!({}));
    let catalog = rpc_result(&resp);
    let names: Vec<&str> = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(!names.contains(&"proposal.approve"));
    assert!(names.contains(&"midi.edit.prepare"));

    let result = call_tool(
        &rig,
        &mut client,
        "proposal.approve",
        &json!({"proposal": "p-x", "digest": "d-x", "expected_revision": 1}),
    );
    assert_eq!(tool_error_code(&result), "unavailable");

    let mut control = control_session(&rig);
    let resp = rpc(&rig, &mut control, "tools/list", &json!({}));
    let catalog = rpc_result(&resp);
    let names: Vec<&str> = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"proposal.approve"));
}

#[test]
fn control_methods_are_blind_on_the_client_channel() {
    // Host administration (permit/policy issuance) does not exist for
    // a client session — not even as a permission error.
    let rig = mcp_rig();
    let mut client = client_session(&rig, "agent-1");
    for method in [
        "synvelis.issue-permit",
        "synvelis.issue-grant",
        "synvelis.host-status",
    ] {
        let resp = rpc(&rig, &mut client, method, &json!({}));
        assert_eq!(rpc_error_code(&resp), METHOD_NOT_FOUND, "{method}");
    }
}

#[test]
fn disabled_adapter_denies_the_surface() {
    // `enabled: false` is the default-off posture: connect, tools,
    // and control methods all refuse — normal DAW operation is
    // untouched because nothing binds.
    let rig = mcp_rig_enabled(false);
    let mut session = rig.host.new_session(McpChannel::Client);
    let resp = connect(&rig, &mut session, &rig.secrets["agent-1"].secret);
    assert_eq!(rpc_error_code(&resp), ADAPTER_DISABLED);
}

// ── Protocol hygiene ─────────────────────────────────────────

#[test]
fn unsupported_protocol_version_is_rejected() {
    let rig = mcp_rig();
    let mut session = rig.host.new_session(McpChannel::Client);
    let resp = rpc(
        &rig,
        &mut session,
        "initialize",
        &json!({"protocolVersion": "1999-01-01", "capabilities": {}}),
    );
    assert_eq!(rpc_error_code(&resp), INVALID_PARAMS);
    let resp = rpc(
        &rig,
        &mut session,
        "initialize",
        &json!({"protocolVersion": PROTOCOL_VERSION, "capabilities": {}}),
    );
    assert_eq!(rpc_result(&resp)["protocolVersion"], PROTOCOL_VERSION);
}

#[test]
fn notifications_never_produce_responses() {
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    for method in [
        "notifications/initialized",
        "notifications/cancelled",
        "notifications/progress",
    ] {
        let resp = rig.host.handle(
            &mut session,
            &json!({"jsonrpc": "2.0", "method": method, "params": {}}),
        );
        assert!(resp.is_none(), "{method}");
    }
}

#[test]
fn malformed_messages_get_typed_protocol_errors() {
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    // Unknown method.
    let resp = rpc(&rig, &mut session, "frobnicate/now", &json!({}));
    assert_eq!(rpc_error_code(&resp), METHOD_NOT_FOUND);
    // Non-object message.
    let resp = rig
        .host
        .handle(&mut session, &json!(["not", "an", "object"]))
        .unwrap();
    assert_eq!(rpc_error_code(&resp), INVALID_REQUEST);
    // tools/call without a name.
    let resp = rpc(&rig, &mut session, "tools/call", &json!({"arguments": {}}));
    assert_eq!(rpc_error_code(&resp), INVALID_PARAMS);
}

// ── Edge validation: arguments, typed ids, bounds ────────────

#[test]
fn rejects_capability_outside_the_tool_allowlist() {
    // The tool surface is a *shape* allowlist — a capability the tool
    // does not offer is refused before any wire exists (and the
    // allowlist is never the authorization itself).
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    for (capability, case) in [
        ("synvelis.clip.mute", "unoffered internal capability"),
        ("synvelis.midi.source.create", "create-shape op"),
        ("synvelis.proposal.apply", "internal apply capability"),
    ] {
        let result = call_tool(
            &rig,
            &mut session,
            "midi.edit.prepare",
            &json!({
                "operation": {
                    "capability": capability,
                    "target": {"kind": "source", "id": tagged(&source10())},
                    "args": {},
                },
                "base_revision": 1,
            }),
        );
        assert_eq!(tool_error_code(&result), "invalid-arguments", "{case}");
    }
}

#[test]
fn rejects_wrong_kinds_and_malformed_typed_ids() {
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    // Wrong target kind for the tool contract.
    let result = call_tool(
        &rig,
        &mut session,
        "midi.edit.prepare",
        &json!({
            "operation": {
                "capability": "synvelis.midi.note.add",
                "target": {"kind": "clip", "id": tagged(&clip11())},
                "args": {},
            },
            "base_revision": 1,
        }),
    );
    assert_eq!(tool_error_code(&result), "invalid-arguments");
    // A tagged id of the wrong kind for a `source` parameter.
    let result = call_tool(
        &rig,
        &mut session,
        "midi.notes.query",
        &json!({"source": tagged(&clip11()), "range": {"start": 0, "len": 10}}),
    );
    assert!(v_is_error(&result));
    // Operation handle of the wrong tagged kind — existence-blind
    // parse (never "I found a proposal by that name").
    let result = call_tool(
        &rig,
        &mut session,
        "operation.get",
        &json!({"operation": "commit-00000000000000000000000000000009"}),
    );
    assert_eq!(tool_error_code(&result), "invalid-arguments");
}

fn v_is_error(v: &Value) -> bool {
    v["isError"].as_bool().unwrap_or(false)
}

#[test]
fn rejects_unknown_missing_and_malformed_fields() {
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    // Unknown argument key.
    let result = call_tool(
        &rig,
        &mut session,
        "midi.notes.query",
        &json!({
            "source": tagged(&source10()),
            "range": {"start": 0, "len": 10},
            "bogus": 1,
        }),
    );
    assert_eq!(tool_error_code(&result), "invalid-arguments");
    // Missing required key.
    let result = call_tool(
        &rig,
        &mut session,
        "midi.notes.query",
        &json!({"source": tagged(&source10())}),
    );
    assert_eq!(tool_error_code(&result), "invalid-arguments");
    // Non-positive range length.
    let result = call_tool(
        &rig,
        &mut session,
        "midi.notes.query",
        &json!({"source": tagged(&source10()), "range": {"start": 0, "len": 0}}),
    );
    assert_eq!(tool_error_code(&result), "invalid-arguments");
    // A forbidden disclosure class on the inspect tool.
    for fields in [json!(["media"]), json!(["derived"]), json!(["selection"])] {
        let result = call_tool(
            &rig,
            &mut session,
            "project.inspect",
            &json!({"fields": fields}),
        );
        assert_eq!(tool_error_code(&result), "invalid-arguments");
    }
}

#[test]
fn rejects_oversized_ranges_pages_and_payloads() {
    // DATA-08 / bounded-transfer: the adapter bound refuses before
    // the engine budget is even consulted.
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    let result = call_tool(
        &rig,
        &mut session,
        "midi.notes.query",
        &json!({
            "source": tagged(&source10()),
            "range": {"start": 0, "len": rig.host.bounds().max_range_len + 1},
        }),
    );
    assert_eq!(tool_error_code(&result), "limit-exceeded");
    let result = call_tool(
        &rig,
        &mut session,
        "midi.notes.query",
        &json!({
            "source": tagged(&source10()),
            "range": {"start": 0, "len": 10},
            "page": {"limit": u64::from(rig.host.bounds().max_page_limit) + 1},
        }),
    );
    assert_eq!(tool_error_code(&result), "limit-exceeded");
    // Oversized argument blob is a JSON-RPC-level refusal.
    let big = "x".repeat(rig.host.bounds().max_tool_args_bytes + 8);
    let resp = rpc(
        &rig,
        &mut session,
        "tools/call",
        &json!({"name": "project.inspect", "arguments": {"padding": big}}),
    );
    assert_eq!(rpc_error_code(&resp), INVALID_PARAMS);
}

// ── Permit-bound authority and per-call re-evaluation ────────

#[test]
fn wire_authority_flows_from_the_permit_record() {
    // The bound task+grant ride inside the wire the *adapter* builds —
    // never inside tool arguments. A delegated grant over the MIDI
    // scope prepares; the same session cannot exceed that grant.
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    let (proposal, digest) = prepare_note(&rig, &mut session);
    assert!(!proposal.is_empty() && !digest.is_empty());
    // Scope violation: source9 is outside the grant's resource scope.
    let result = call_tool(
        &rig,
        &mut session,
        "midi.edit.prepare",
        &json!({
            "operation": {
                "capability": "synvelis.midi.note.add",
                "target": {"kind": "source", "id": tagged(&source9())},
                "args": {"key": 60, "velocity": 90, "range": {"domain": "source-native", "start": 0, "len": 240}},
            },
            "base_revision": 1,
        }),
    );
    assert!(v_is_error(&result), "out-of-scope prepare must deny");
}

#[test]
fn revoked_grant_denies_the_very_next_call() {
    // Re-evaluation is per invocation: a grant revoked between two
    // calls kills the second — tool discovery is not permission.
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    prepare_note(&rig, &mut session);
    rig.store.revoke_grant(&grant_ref("g1"));
    let result = call_tool(&rig, &mut session, "midi.edit.prepare", &prepare_args());
    assert_eq!(tool_error_code(&result), "denied");
}

// ── The three MIDI modes ─────────────────────────────────────

#[test]
fn suggest_mode_prepares_but_denies_apply() {
    // Suggest: the agent may propose; applying is for Synvelis alone.
    // The denial arrives from GW-02's per-call evaluation — and the
    // commit seam is never touched.
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-suggest");
    let (proposal, digest) = prepare_note(&rig, &mut session);
    let result = call_tool(
        &rig,
        &mut session,
        "proposal.apply",
        &apply_args(&proposal, &digest, None),
    );
    assert_eq!(tool_error_code(&result), "denied");
    assert_eq!(rig.committer.calls(), 0, "a denied apply never commits");
}

#[test]
fn delegated_apply_commits_through_the_seam() {
    // Delegated: no per-change approval, but the real apply pipeline
    // still runs and the commit boundary reports the durable verdict.
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    let (proposal, digest) = prepare_note(&rig, &mut session);
    let result = call_tool(
        &rig,
        &mut session,
        "proposal.apply",
        &apply_args(&proposal, &digest, None),
    );
    let out = tool_ok(&result);
    assert_eq!(out["status"], "committed");
    assert_eq!(out["revision"], 2);
    assert_eq!(rig.committer.calls(), 1);
    let operation = out["operation"].as_str().unwrap().to_owned();
    // The operation handle reads back its settled verdict.
    let status = call_tool(
        &rig,
        &mut session,
        "operation.get",
        &json!({"operation": operation}),
    );
    assert_eq!(tool_ok(&status)["status"], "committed");
}

#[test]
fn approve_to_apply_requires_and_consumes_the_receipt() {
    // Approve-to-apply: the agent's apply needs a Synvelis-issued
    // receipt (issued on the control channel — never by the agent),
    // and consumption is single-use.
    let rig = mcp_rig();
    let mut agent = client_session(&rig, "agent-approve");
    let (proposal, digest) = prepare_note(&rig, &mut agent);

    // Without a receipt the apply cannot complete — the gateway
    // answers with a pending approval *request*, and the proposal
    // stays unapplied.
    let result = call_tool(
        &rig,
        &mut agent,
        "proposal.apply",
        &apply_args(&proposal, &digest, None),
    );
    let out = tool_ok(&result);
    assert_eq!(out["status"], "approval-required");
    assert_eq!(rig.committer.calls(), 0);

    // A bogus receipt is denied too.
    let result = call_tool(
        &rig,
        &mut agent,
        "proposal.apply",
        &apply_args(&proposal, &digest, Some("approval-forged")),
    );
    assert!(v_is_error(&result));

    // The trusted approval enters on the control channel.
    let mut control = control_session(&rig);
    let approved = call_tool(
        &rig,
        &mut control,
        "proposal.approve",
        &json!({"proposal": proposal, "digest": digest, "expected_revision": 1}),
    );
    let receipt = tool_ok(&approved)["approval"]
        .as_str()
        .expect("approval ref")
        .to_owned();

    // With the receipt, the apply is accepted and commits.
    let result = call_tool(
        &rig,
        &mut agent,
        "proposal.apply",
        &apply_args(&proposal, &digest, Some(&receipt)),
    );
    assert_eq!(tool_ok(&result)["status"], "committed");
    assert_eq!(rig.committer.calls(), 1);

    // A second proposal under the same receipt cannot apply — the
    // receipt was consumed.
    let (proposal2, digest2) = prepare_note(&rig, &mut agent);
    let result = call_tool(
        &rig,
        &mut agent,
        "proposal.apply",
        &apply_args(&proposal2, &digest2, Some(&receipt)),
    );
    assert!(v_is_error(&result), "a consumed receipt authorizes nothing");
    assert_eq!(rig.committer.calls(), 1);
}

#[test]
fn commit_failure_reports_the_reconciled_verdict_not_success() {
    // ACG-004 honesty: a failed commit attempt reports what reconcile
    // *proved* — never a fabricated success.
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    rig.committer.fail(
        GatewayError::Indeterminate {
            reason: "journal write failed".to_owned(),
        }
        .into(),
    );
    let (proposal, digest) = prepare_note(&rig, &mut session);
    let result = call_tool(
        &rig,
        &mut session,
        "proposal.apply",
        &apply_args(&proposal, &digest, None),
    );
    let out = tool_ok(&result);
    // Nothing settled: the honest report is commit-requested/in-flight,
    // never "committed".
    let status = out["status"].as_str().unwrap();
    assert!(
        status == "commit-requested" || status == "in-flight",
        "an undetermined commit must never report committed: {out}"
    );
}

#[test]
fn settle_op_reports_committed_after_restart_scoped_reconcile() {
    // After an adapter restart the book is empty — reconcile is the
    // durable truth. A committed receipt attributes its operation to
    // its principal, so the surviving principal still resolves it;
    // a foreign principal sees the blind verdict.
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    let op = OperationId::new(0xBEEF);
    rig.committer.report(
        op,
        RetryOutcome::Settled(CommitOutcome::Committed(CommitReceipt {
            commit: CommitId::new(3),
            operation: Some(op),
            proposal: None,
            principal: agent_principal(),
            project: project(),
            base_revision: ProjectRevision(1),
            revision: ProjectRevision(2),
            journal_seq: 3,
        })),
    );
    let result = call_tool(
        &rig,
        &mut session,
        "operation.get",
        &json!({"operation": op.to_tagged()}),
    );
    assert_eq!(tool_ok(&result)["status"], "committed");

    // A foreign principal gets the blind verdict for the same handle.
    let mut foreign = client_session(&rig, "agent-2");
    let result = call_tool(
        &rig,
        &mut foreign,
        "operation.get",
        &json!({"operation": op.to_tagged()}),
    );
    assert_eq!(tool_error_code(&result), "unknown-operation");
}

#[test]
fn operation_handles_are_scoped_and_blind() {
    // Unknown and foreign operation handles share one verdict — a
    // probe never learns whether an operation exists.
    let rig = mcp_rig();
    let mut a = client_session(&rig, "agent-1");
    let mut b = client_session(&rig, "agent-2");
    // A genuinely unknown handle.
    let result = call_tool(
        &rig,
        &mut a,
        "operation.get",
        &json!({"operation": OperationId::new(0xDEAD).to_tagged()}),
    );
    assert_eq!(tool_error_code(&result), "unknown-operation");
    // A records an op in the book (unknown to reconcile).
    let (proposal, digest) = prepare_note(&rig, &mut a);
    // A cancel on an unknown-to-B operation is blind.
    let result = call_tool(
        &rig,
        &mut b,
        "operation.cancel",
        &json!({"operation": OperationId::new(0xBEEF).to_tagged()}),
    );
    assert_eq!(tool_error_code(&result), "unknown-operation");
    // Cancelling A's real op requires a book entry — apply it first.
    rig.committer.fail(
        GatewayError::Indeterminate {
            reason: "offline".to_owned(),
        }
        .into(),
    );
    let applied = call_tool(
        &rig,
        &mut a,
        "proposal.apply",
        &apply_args(&proposal, &digest, None),
    );
    let op = tool_ok(&applied)["operation"].as_str().unwrap().to_owned();
    // B cannot cancel A's operation.
    let result = call_tool(&rig, &mut b, "operation.cancel", &json!({"operation": op}));
    assert_eq!(tool_error_code(&result), "unknown-operation");
    // A can — a pending apply settles as rejected through the port.
    let result = call_tool(&rig, &mut a, "operation.cancel", &json!({"operation": op}));
    assert_eq!(tool_ok(&result)["status"], "cancelled");
    assert_eq!(rig.committer.cancelled().len(), 1);
    // A settled operation is not cancellable again.
    let result = call_tool(&rig, &mut a, "operation.cancel", &json!({"operation": op}));
    assert_eq!(tool_error_code(&result), "not-cancellable");
}

// ── Control-channel administration ───────────────────────────

#[test]
fn control_administration_routes_through_the_admin_seam() {
    // `synvelis.issue-*`/`revoke-*` on a bound control session reach
    // the host's AdminPort — never direct state mutation — and are
    // blind on the client channel.
    let rig = mcp_rig();
    let mut control = control_session(&rig);
    let resp = rpc(
        &rig,
        &mut control,
        "synvelis.issue-task",
        &json!({
            "task": "t-new",
            "principal": "mcp-client-1",
            "project": project().to_tagged(),
        }),
    );
    rpc_result(&resp);
    assert_eq!(
        rig.admin.tasks.lock().expect("tasks")[0].task,
        task_ref("t-new")
    );
    let resp = rpc(
        &rig,
        &mut control,
        "synvelis.revoke-task",
        &json!({"task": "t-new"}),
    );
    rpc_result(&resp);
    assert_eq!(
        rig.admin.revoked_tasks.lock().expect("revoked")[0],
        task_ref("t-new")
    );
    // Malformed control params get typed param errors.
    let resp = rpc(
        &rig,
        &mut control,
        "synvelis.issue-permit",
        &json!({"channel": "orbit", "principal_kind": "agent",
               "principal": "x", "project": project().to_tagged()}),
    );
    assert_eq!(rpc_error_code(&resp), INVALID_PARAMS);
}

#[test]
fn issue_permit_on_control_mints_a_working_credential() {
    // The control-issued permit actually binds a session — end-to-end
    // provisioning without touching the store directly.
    let rig = mcp_rig();
    let mut control = control_session(&rig);
    let resp = rpc(
        &rig,
        &mut control,
        "synvelis.issue-permit",
        &json!({
            "channel": "client",
            "principal_kind": "agent",
            "principal": "mcp-client-1",
            "project": project().to_tagged(),
            "task": "t1",
            "grant": "g1",
        }),
    );
    let result = rpc_result(&resp);
    let secret = result["secret"].as_str().expect("issued secret").to_owned();
    assert_eq!(result["channel"], "client");
    let mut client = rig.host.new_session(McpChannel::Client);
    let resp = connect(&rig, &mut client, &secret);
    assert_eq!(rpc_result(&resp)["principal"]["id"], "mcp-client-1");
}

// ── Permit store persistence ─────────────────────────────────

#[test]
fn file_store_survives_restart_and_never_leaks_secrets() {
    // The durable credential file stores digests only — a leak of the
    // file discloses no usable credential.
    let dir = std::env::temp_dir().join(format!("mcp-permits-{}", std::process::id()));
    let path = dir.join("permits.json");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let secret;
    let permit_id;
    {
        let mut store = FilePermitStore::open(&path).unwrap();
        let issue = store
            .issue(client_spec("t1", "g1"), PolicyInstant::new(1_000))
            .unwrap();
        secret = issue.secret.clone();
        permit_id = issue.record.permit.clone();
    }
    // Reopen — the permit verifies (restart honesty).
    let mut store = FilePermitStore::open(&path).unwrap();
    assert!(
        store
            .verify(&secret, McpChannel::Client, PolicyInstant::new(1_001))
            .is_some()
    );
    // The file on disk carries no usable credential.
    let disk = std::fs::read_to_string(&path).unwrap();
    assert!(!disk.contains(&secret), "the file never stores secrets");
    // Wrong channel is blind.
    assert!(
        store
            .verify(&secret, McpChannel::Control, PolicyInstant::new(1_001))
            .is_none()
    );
    // Revocation persists.
    assert!(store.revoke(&permit_id).unwrap());
    drop(store);
    let store = FilePermitStore::open(&path).unwrap();
    assert!(
        store
            .verify(&secret, McpChannel::Client, PolicyInstant::new(1_001))
            .is_none()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tampered_secret_hash_fails_closed() {
    // A credential file edited to point at a forged hash verifies
    // nothing — tampering breaks the credential, never grants it.
    let dir = std::env::temp_dir().join(format!("mcp-tamper-{}", std::process::id()));
    let path = dir.join("permits.json");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut store = FilePermitStore::open(&path).unwrap();
    let issue = store
        .issue(client_spec("t1", "g1"), PolicyInstant::new(1_000))
        .unwrap();
    drop(store);
    let text = std::fs::read_to_string(&path).unwrap();
    // The on-disk record holds only the *hash* — swap that for a
    // forged digest and no presented secret matches.
    let mut file: Value = serde_json::from_str(&text).unwrap();
    file["permits"][0]["secret_hash"] = json!("f".repeat(64));
    std::fs::write(&path, serde_json::to_string_pretty(&file).unwrap()).unwrap();
    let store = FilePermitStore::open(&path).unwrap();
    assert!(
        store
            .verify(&issue.secret, McpChannel::Client, PolicyInstant::new(1_001))
            .is_none()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
