//! GW-10 real-transport evidence — Issue #62, unit GW10-F / gate G4.
//!
//! These cases are the L4/Real half of the MCP evidence: the actual
//! `mcp-poc` binary over real stdio pipes + the authenticated
//! loopback control channel, against a real harness —
//! `DirectoryBundleStore` project, the GW-06 commit journal, and the
//! file-backed permit store. Every record carries `L4`/`real` +
//! backend/transport metadata so mock-layer results can never be
//! promoted into these slots.

mod common;

use common::*;
use serde_json::json;
use synvelis_gateway::{Evidence, Layer, RunMode};

fn suite() -> std::sync::Arc<Evidence> {
    Evidence::suite("gw10-transport")
}

const BACKEND: (&str, &str) = (
    "DirectoryBundleStore+FileJournal+FilePermitStore",
    "stdio+loopback",
);

// ── GTV-004 path 2 + REC-06: delegated apply over real stdio ─

#[test]
fn gw10f_delegated_apply_commits_and_survives_restart_real() {
    // Delegated-apply E2E plus restart reconciliation — real process,
    // real commit gate, real durable state.
    let ev = suite();
    let dir = fresh_dir("gw10f-delegated");
    let control_permit = issue_permit(&dir, "control", "human", "user-1", None, None);
    let client_permit = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("t1"),
        Some("g1"),
    );
    let mut server = Server::spawn(&dir);
    let mut control = server.control();
    connect(&mut control, control_permit["secret"].as_str().unwrap());
    provision_authority(&mut control, "t1", "g1", "delegated-apply", FULL_CAPS);
    bound_client(&mut server, client_permit["secret"].as_str().unwrap());

    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(720));
    let prepared = structured(&resp);
    let proposal = prepared["proposal"].as_str().unwrap().to_owned();
    let digest = prepared["digest"].as_str().unwrap().to_owned();
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(&proposal, &digest, None, 1),
    );
    let applied = structured(&resp);
    assert_eq!(applied["status"], "committed");
    let operation = applied["operation"].as_str().unwrap().to_owned();
    let journal_seq = applied["journal_seq"].as_u64().unwrap();

    // REC-06: the same apply reconciles to the same commit — the
    // disconnect/retry path cannot mint a duplicate.
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(&proposal, &digest, None, 1),
    );
    let retried = structured(&resp);
    assert_eq!(retried["journal_seq"], journal_seq);

    ev.case("E2E-2-delegated")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .log(&format!(
            "committed op={operation} seq={journal_seq}; retry replayed"
        ))
        .verify("committed", "committed");

    // SYS-09 / E2E-2-restart: kill the process; a fresh server
    // reconciles the committed operation from the durable journal,
    // and the project state survived.
    drop(server);
    let mut server = Server::spawn(&dir);
    bound_client(&mut server, client_permit["secret"].as_str().unwrap());
    let resp = tool_call(
        &mut server.stdio,
        "operation.get",
        &json!({"operation": operation}),
    );
    assert_eq!(structured(&resp)["status"], "committed");
    ev.case("E2E-2-restart")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .log("process kill + fresh serve: durable journal reconciles committed")
        .verify("committed", "committed");
    // SYS-09: durable operation identity survives the transport restart.
    ev.case("SYS-09")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .verify("committed", "committed");
}

// ── SEC-08 + GTV-004: the three modes over real transports ───

#[test]
fn gw10f_three_modes_over_real_transports() {
    let ev = suite();

    // Suggest: prepare permits; apply is denied at the real gate.
    let dir = fresh_dir("gw10f-suggest");
    let cp = issue_permit(&dir, "control", "human", "user-1", None, None);
    let client = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("ts"),
        Some("gs"),
    );
    let mut server = Server::spawn(&dir);
    let mut control = server.control();
    connect(&mut control, cp["secret"].as_str().unwrap());
    provision_authority(&mut control, "ts", "gs", "suggest", FULL_CAPS);
    bound_client(&mut server, client["secret"].as_str().unwrap());
    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(960));
    let prepared = structured(&resp);
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(
            prepared["proposal"].as_str().unwrap(),
            prepared["digest"].as_str().unwrap(),
            None,
            1,
        ),
    );
    assert_eq!(tool_error(&resp), "denied");
    ev.case("E2E-2-suggest")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .verify("denied", "denied");

    // Approve-to-apply: receipt minted on the control channel, spent
    // exactly once on the client channel.
    let dir = fresh_dir("gw10f-a2a");
    let cp = issue_permit(&dir, "control", "human", "user-1", None, None);
    let client = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("ta"),
        Some("ga"),
    );
    let mut server = Server::spawn(&dir);
    let mut control = server.control();
    connect(&mut control, cp["secret"].as_str().unwrap());
    provision_authority(&mut control, "ta", "ga", "approve-to-apply", FULL_CAPS);
    bound_client(&mut server, client["secret"].as_str().unwrap());
    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(1200));
    let prepared = structured(&resp);
    let proposal = prepared["proposal"].as_str().unwrap().to_owned();
    let digest = prepared["digest"].as_str().unwrap().to_owned();
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(&proposal, &digest, None, 1),
    );
    assert_eq!(structured(&resp)["status"], "approval-required");
    let resp = tool_call(
        &mut control,
        "proposal.approve",
        &json!({"proposal": proposal, "digest": digest, "expected_revision": 1}),
    );
    let receipt = structured(&resp)["approval"].as_str().unwrap().to_owned();
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(&proposal, &digest, Some(&receipt), 1),
    );
    assert_eq!(structured(&resp)["status"], "committed");
    ev.case("E2E-2-approve")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .log("control-channel receipt spent once on the client channel")
        .verify("committed", "committed");
}

// ── GTV-006: the boundary battery over real transports ───────

#[test]
fn gw10f_boundary_attacks_fail_closed_real() {
    let ev = suite();
    let dir = fresh_dir("gw10f-sec");
    let cp = issue_permit(&dir, "control", "human", "user-1", None, None);
    let client = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("t1"),
        Some("g1"),
    );
    let foreign = issue_permit(&dir, "client", "agent", "mcp-client-2", None, None);
    let mut server = Server::spawn(&dir);
    let mut control = server.control();
    connect(&mut control, cp["secret"].as_str().unwrap());
    provision_authority(&mut control, "t1", "g1", "delegated-apply", FULL_CAPS);

    // DATA-09: discovery is not permission — unbound calls refuse.
    let resp = server.stdio.call("tools/list", &json!({}));
    assert_eq!(error_code(&resp), PERMIT_REQUIRED);
    ev.case("DATA-09")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .verify("denied", "denied");

    // Channel separation: wrong-channel and garbage secrets refuse.
    let mut wrong = server.control();
    assert_eq!(
        error_code(&connect(&mut wrong, client["secret"].as_str().unwrap())),
        PERMIT_INVALID
    );
    assert_eq!(
        error_code(&connect(
            &mut server.stdio,
            "syv-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )),
        PERMIT_INVALID
    );
    ev.case("GTV-006")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .log("wrong-channel and garbage permits refuse identically")
        .verify("denied", "denied");

    bound_client(&mut server, client["secret"].as_str().unwrap());
    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(720));
    let prepared = structured(&resp);
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(
            prepared["proposal"].as_str().unwrap(),
            prepared["digest"].as_str().unwrap(),
            None,
            1,
        ),
    );
    let operation = structured(&resp)["operation"].as_str().unwrap().to_owned();

    // JOB-08 surface: a foreign principal cannot even see the handle.
    let mut foreign_server = Server::spawn(&dir);
    connect(
        &mut foreign_server.stdio,
        foreign["secret"].as_str().unwrap(),
    );
    let resp = tool_call(
        &mut foreign_server.stdio,
        "operation.get",
        &json!({"operation": operation}),
    );
    assert_eq!(tool_error(&resp), "unknown-operation");
    ev.case("JOB-08")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .log("foreign principal: operation handle is existence-blind")
        .verify("denied", "denied");

    // Revocation denies the very next call on the live channel.
    result(&control.call("synvelis.revoke-grant", &json!({"grant": "g1"})));
    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(960));
    assert_eq!(tool_error(&resp), "denied");
    ev.case("GTV-006")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .verify("denied", "denied");

    // DATA-08 at the transport edge: oversized inbound is refused
    // before parse — the response carries a null id.
    let big = "x".repeat(300_000);
    server.stdio.send_raw(
        &json!({
            "jsonrpc": "2.0", "id": 99, "method": "tools/call",
            "params": {"name": "project.inspect", "arguments": {"pad": big}},
        })
        .to_string(),
    );
    let resp = server.stdio.read_line();
    assert!(resp.get("error").is_some());
    ev.case("DATA-08")
        .meta(Layer::L4, RunMode::Real)
        .backend(BACKEND.0, BACKEND.1)
        .fixture(&dir.display().to_string())
        .log("300KB inbound refused before parse")
        .verify("denied", "denied");
}

// ── Permit lifecycle over the real credential file ───────────

#[test]
fn gw10f_permit_cli_lifecycle_real() {
    let ev = suite();
    let dir = fresh_dir("gw10f-permit");
    let issued = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("t1"),
        Some("g1"),
    );
    let permit_id = issued["permit"].as_str().unwrap().to_owned();
    let listed = permit_cli(&dir, &["list"]);
    assert_eq!(listed["permits"].as_array().unwrap().len(), 1);
    let raw = std::fs::read_to_string(dir.join("permits.json")).unwrap();
    assert!(
        !raw.contains(issued["secret"].as_str().unwrap()),
        "the credential file never stores a usable secret"
    );
    let revoked = permit_cli(&dir, &["revoke", "--permit", &permit_id]);
    assert_eq!(revoked["revoked"], true);
    let mut server = Server::spawn(&dir);
    let resp = connect(&mut server.stdio, issued["secret"].as_str().unwrap());
    assert_eq!(error_code(&resp), PERMIT_INVALID);
    ev.case("GTV-006")
        .meta(Layer::L4, RunMode::Real)
        .backend("FilePermitStore", "offline-cli+stdio")
        .fixture(&dir.display().to_string())
        .log("provision → list without secrets → revoke → refused at serve")
        .verify("denied", "denied");
}
