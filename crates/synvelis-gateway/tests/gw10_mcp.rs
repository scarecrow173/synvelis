// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-10 MCP protocol/security evidence — Issue #62, unit GW10-F /
//! gate G4 (in-process layer).
//!
//! Every case drives the real `McpHost` + `Gateway` + `PrepareEngine`
//! + `ProposalEngine` stack; only the commit boundary is scripted —
//!   its real-transport counterpart is `mcp-poc`'s `gw10_transport`
//!   suite (L4/real). Session identity, permit lifecycle, channel
//!   separation, per-call reauthorization, and the three approval modes
//!   are evidenced here.

mod rig;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::mcp::*;
use synvelis_gateway::mcp_permit::check_channel_kind;
use synvelis_gateway::*;
use synvelis_project::ProjectRevision;

fn suite() -> Arc<Evidence> {
    Evidence::suite("gw10-mcp")
}

// ── Scripted seams (L2 doubles; L4 real wire is mcp-poc) ─────

#[derive(Default)]
struct ScriptedCommit {
    committed: Mutex<Vec<ProposalRef>>,
    outcomes: Mutex<BTreeMap<u128, RetryOutcome>>,
}

impl CommitPort for ScriptedCommit {
    fn commit(&self, proposal: &ProposalRef) -> Result<CommitReceipt, ErrorEnvelope> {
        self.committed
            .lock()
            .expect("committed")
            .push(proposal.clone());
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
}

#[derive(Default)]
struct ScriptedAdmin {
    revoked_grants: Mutex<Vec<GrantRef>>,
}

impl AdminPort for ScriptedAdmin {
    fn issue_task(&self, _task: AuthorizedTask) -> Result<(), ErrorEnvelope> {
        Ok(())
    }
    fn issue_grant(&self, _grant: Grant) -> Result<(), ErrorEnvelope> {
        Ok(())
    }
    fn revoke_grant(&self, id: &GrantRef) -> Result<(), ErrorEnvelope> {
        self.revoked_grants
            .lock()
            .expect("revoked_grants")
            .push(id.clone());
        Ok(())
    }
    fn revoke_task(&self, _id: &TaskRef) -> Result<(), ErrorEnvelope> {
        Ok(())
    }
    fn live_revision(&self, _project: synvelis_project::ProjectId) -> Option<ProjectRevision> {
        Some(ProjectRevision(1))
    }
}

// ── The GW10-F rig ───────────────────────────────────────────

struct McpRig {
    host: McpHost,
    store: Arc<InMemoryPolicyStore>,
    clock: Arc<ManualClock>,
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
    // fixture_rig already wires the context engine (`synvelis.context.query`).
    let rig = fixture_rig();
    let create_notes = Some(CreateWithin {
        parents: OwnerConstraint::OneOf {
            refs: BTreeSet::from([source10()]),
        },
        kinds: BTreeSet::from([ResourceKind::Note]),
        placement: None,
    });
    for (task, grant, policy) in [
        ("t1", "g1", ApprovalPolicy::DelegatedApply),
        ("t-suggest", "g-suggest", ApprovalPolicy::Suggest),
        ("t-approve", "g-approve", ApprovalPolicy::ApproveToApply),
    ] {
        issue_task(&rig.store, task);
        let mut g = apply_grant(grant, task, &["synvelis.midi.note.add"], policy);
        g.create_within.clone_from(&create_notes);
        rig.store.issue_grant(g);
    }

    let now = rig.clock.now();
    let mut permits = MemoryPermitStore::new();
    let mut secrets = BTreeMap::new();
    for (tag, spec) in [
        ("agent-1", client_spec("t1", "g1")),
        ("agent-suggest", client_spec("t-suggest", "g-suggest")),
        ("agent-approve", client_spec("t-approve", "g-approve")),
        ("control-1", control_spec()),
        (
            "foreign",
            PermitSpec {
                channel: McpChannel::Client,
                principal: Principal::new(PrincipalKind::Agent, "mcp-client-2").unwrap(),
                project: project(),
                task: None,
                grant: None,
                ttl_ms: None,
            },
        ),
        (
            "expired",
            PermitSpec {
                ttl_ms: Some(50),
                ..client_spec("t1", "g1")
            },
        ),
    ] {
        check_channel_kind(&spec).expect("spec is channel-kind clean");
        let issue = permits.issue(spec, now).expect("issue");
        secrets.insert(tag.to_owned(), issue);
    }
    let revoked = permits.issue(client_spec("t1", "g1"), now).expect("issue");
    permits.revoke(&revoked.record.permit).expect("revoke");
    secrets.insert("revoked".to_owned(), revoked);

    let host = McpHost::new(
        rig.gw,
        Box::new(permits),
        Arc::new(ScriptedCommit::default()),
        Arc::new(ScriptedAdmin::default()),
        rig.clock.clone(),
        McpConfig {
            adapter: AdapterId::new(MCP_ADAPTER_RAW),
            bounds: McpBounds::default(),
            enabled: true,
        },
    );
    McpRig {
        host,
        store: rig.store,
        clock: rig.clock,
        secrets,
    }
}

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

fn client_session(rig: &McpRig, tag: &str) -> McpSession {
    let mut session = rig.host.new_session(McpChannel::Client);
    let resp = connect(rig, &mut session, &rig.secrets[tag].secret);
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

// ── Identity + permit lifecycle (GT-002 / GTV-006) ───────────

#[test]
fn gw10f_session_binds_permit_record_never_client_claims() {
    // GT-002 at the transport edge: `clientInfo` is decoration —
    // the bound identity comes from the permit record.
    let ev = suite();
    let rig = mcp_rig();
    let mut session = rig.host.new_session(McpChannel::Client);
    rpc(
        &rig,
        &mut session,
        "initialize",
        &json!({
            "protocolVersion": PROTOCOL_VERSION,
            "clientInfo": {"name": "root", "version": "9.9.9"},
            "capabilities": {},
        }),
    );
    let resp = connect(&rig, &mut session, &rig.secrets["agent-1"].secret);
    let result = rpc_result(&resp);
    assert_eq!(result["principal"]["kind"], "agent");
    assert_eq!(result["principal"]["id"], "mcp-client-1");
    ev.case("GTV-006")
        .meta(Layer::L3, RunMode::Mock)
        .backend("MemoryPermitStore", "in-process")
        .fixture("mcp rig")
        .log("clientInfo 'root' cannot claim the session identity")
        .verify("mcp-client-1", "mcp-client-1");
}

#[test]
fn gw10f_dead_permits_fail_blind() {
    // Revoked, expired, and wrong-channel permits refuse identically —
    // no existence disclosure between the failure modes.
    let ev = suite();
    let rig = mcp_rig();
    let mut dead = Vec::new();
    {
        let mut s = rig.host.new_session(McpChannel::Client);
        let resp = connect(&rig, &mut s, &rig.secrets["revoked"].secret);
        dead.push(rpc_error_code(&resp));
    }
    // Expire the second permit.
    rig.clock.set(PolicyInstant::new(1_000 + 51));
    let mut s = rig.host.new_session(McpChannel::Client);
    dead.push(rpc_error_code(&connect(
        &rig,
        &mut s,
        &rig.secrets["expired"].secret,
    )));
    // A client secret on the control channel.
    let mut s = rig.host.new_session(McpChannel::Control);
    dead.push(rpc_error_code(&connect(
        &rig,
        &mut s,
        &rig.secrets["agent-1"].secret,
    )));
    // Garbage secret.
    let mut s = rig.host.new_session(McpChannel::Client);
    dead.push(rpc_error_code(&connect(&rig, &mut s, "syv-garbage")));
    assert!(
        dead.iter().all(|c| *c == dead[0]),
        "all dead-permit failures must be indistinguishable: {dead:?}"
    );
    ev.case("GTV-006")
        .meta(Layer::L3, RunMode::Mock)
        .backend("MemoryPermitStore", "in-process")
        .fixture("mcp rig")
        .log(&format!("uniform refusal code {}", dead[0]))
        .verify("uniform-deny", "uniform-deny");
}

// ── Per-call reauthorization (DATA-09 at the tool layer) ─────

#[test]
fn gw10f_revocation_denies_the_next_call() {
    // A bound session is not cached authority — the gateway
    // re-evaluates the live grant on every tool call.
    let ev = suite();
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    let ok = call_tool(&rig, &mut session, "midi.edit.prepare", &prepare_args());
    assert_eq!(ok["isError"], false);

    rig.store.revoke_grant(&grant_ref("g1"));
    let denied = call_tool(&rig, &mut session, "midi.edit.prepare", &prepare_args());
    assert_eq!(tool_error_code(&denied), "denied");
    ev.case("DATA-09")
        .meta(Layer::L3, RunMode::Mock)
        .backend("MemoryPermitStore", "in-process")
        .fixture("mcp rig")
        .log("revoke between calls → next call denies on the live grant")
        .verify("Deny", "Deny");
}

// ── The three approval modes through tools/call ──────────────

#[test]
fn gw10f_suggest_mode_denies_apply() {
    let ev = suite();
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-suggest");
    let prepared = call_tool(&rig, &mut session, "midi.edit.prepare", &prepare_args());
    let out = tool_ok(&prepared);
    let resp = call_tool(
        &rig,
        &mut session,
        "proposal.apply",
        &apply_args(
            out["proposal"].as_str().unwrap(),
            out["digest"].as_str().unwrap(),
            None,
        ),
    );
    assert_eq!(tool_error_code(&resp), "denied");
    ev.case("SEC-08")
        .meta(Layer::L3, RunMode::Mock)
        .backend("MemoryPermitStore", "in-process")
        .fixture("mcp rig")
        .verify("Deny", "Deny");
}

#[test]
fn gw10f_delegated_apply_commits_through_the_host() {
    let ev = suite();
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    let prepared = call_tool(&rig, &mut session, "midi.edit.prepare", &prepare_args());
    let out = tool_ok(&prepared);
    let applied = call_tool(
        &rig,
        &mut session,
        "proposal.apply",
        &apply_args(
            out["proposal"].as_str().unwrap(),
            out["digest"].as_str().unwrap(),
            None,
        ),
    );
    assert_eq!(tool_ok(&applied)["status"], "committed");
    ev.case("GTV-004")
        .meta(Layer::L3, RunMode::Mock)
        .backend("MemoryPermitStore", "in-process")
        .fixture("mcp rig")
        .log("L4/real counterpart: mcp-poc gw10_transport")
        .verify("committed", "committed");
}

// ── Boundary hygiene at the protocol edge ────────────────────

#[test]
fn gw10f_unknown_tool_and_malformed_call_refuse() {
    let ev = suite();
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    let resp = rpc(
        &rig,
        &mut session,
        "tools/call",
        &json!({"name": "synvelis.drop_table", "arguments": {}}),
    );
    assert!(
        matches!(
            tool_error_code(rpc_result(&resp)).as_str(),
            "unavailable" | "unknown-tool" | "denied"
        ),
        "unknown tool must refuse closed: {resp}"
    );
    // tools/call with no name at all.
    let resp = rpc(&rig, &mut session, "tools/call", &json!({"arguments": {}}));
    assert!(resp.get("error").is_some() || rpc_result(&resp)["isError"] == true);
    ev.case("GTV-006")
        .meta(Layer::L3, RunMode::Mock)
        .backend("MemoryPermitStore", "in-process")
        .fixture("mcp rig")
        .log("unknown tool name and malformed call refuse closed")
        .verify("denied", "denied");
}

#[test]
fn gw10f_foreign_principal_cannot_read_the_operation() {
    // JOB-08 in-process: a second agent's `operation.get` on a
    // committed operation is existence-blind.
    let ev = suite();
    let rig = mcp_rig();
    let mut session = client_session(&rig, "agent-1");
    let prepared = call_tool(&rig, &mut session, "midi.edit.prepare", &prepare_args());
    let out = tool_ok(&prepared);
    let applied = call_tool(
        &rig,
        &mut session,
        "proposal.apply",
        &apply_args(
            out["proposal"].as_str().unwrap(),
            out["digest"].as_str().unwrap(),
            None,
        ),
    );
    let operation = tool_ok(&applied)["operation"].as_str().unwrap().to_owned();

    let mut foreign = client_session(&rig, "foreign");
    let resp = call_tool(
        &rig,
        &mut foreign,
        "operation.get",
        &json!({"operation": operation}),
    );
    assert_eq!(tool_error_code(&resp), "unknown-operation");
    ev.case("JOB-08")
        .meta(Layer::L3, RunMode::Mock)
        .backend("MemoryPermitStore", "in-process")
        .fixture("mcp rig")
        .verify("denied", "denied");
}
