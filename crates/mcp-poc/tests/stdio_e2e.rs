//! GW-08 real-transport E2E — Issue #59 acceptance criteria.
//!
//! These tests drive the *actual* `mcp-poc` binary: newline-delimited
//! JSON-RPC over real stdio pipes (the local stdio helper transport)
//! plus the authenticated loopback control channel, against a real
//! harness — `DirectoryBundleStore` project state, the GW-06
//! `CommitGate` journal, and the file-backed permit store. Nothing
//! here is a fixture: a committed apply mutates the durable project
//! and survives a server restart, and `operation.get` reconciles
//! against the real journal.
//!
//! Coverage: the three MIDI modes (Suggest / Approve-to-Apply /
//! Delegated Apply), idempotent retry (no duplicate commit), restart
//! reconciliation, and the boundary attack battery — all through the
//! real wire. The shared transport harness lives in `tests/common`;
//! GW-10's evidence suite (`gw10_transport`) drives the same wire.

mod common;

use common::*;
use serde_json::json;

// ── Delegated apply + idempotent retry + restart reconcile ───

#[test]
fn delegated_apply_commits_retries_without_duplicates_and_survives_restart() {
    // The full delegated-apply E2E over real transports: prepare →
    // apply → real CommitGate commit → durable mutation → reconnect
    // reconcile → no duplicate commit on retry.
    let dir = fresh_dir("delegated");
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

    // Operator provisioning on the authenticated control channel.
    let mut control = server.control();
    let resp = connect(&mut control, control_permit["secret"].as_str().unwrap());
    assert_eq!(result(&resp)["principal"]["id"], "user-1");
    provision_authority(&mut control, "t1", "g1", "delegated-apply", FULL_CAPS);

    // The client session over real stdio.
    let resp = server.stdio.call(
        "initialize",
        &json!({"protocolVersion": "2025-06-18",
               "clientInfo": {"name": "hostile-name", "version": "0"},
               "capabilities": {}}),
    );
    assert_eq!(result(&resp)["protocolVersion"], "2025-06-18");
    bound_client(&mut server, client_permit["secret"].as_str().unwrap());
    let resp = server.stdio.call("tools/list", &json!({}));
    let names: Vec<&str> = result(&resp)["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"midi.edit.prepare"));
    assert!(!names.contains(&"proposal.approve"));

    // Prepare → apply → committed through the real CommitGate.
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
    assert_eq!(applied["revision"], 2);
    let operation = applied["operation"].as_str().unwrap().to_owned();
    let journal_seq = applied["journal_seq"].as_u64().unwrap();

    // Retry the identical apply — the gateway's existing-result
    // reference answers, the gate returns the *same* durable receipt,
    // and no second commit is minted.
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(&proposal, &digest, None, 1),
    );
    let retried = structured(&resp);
    assert_eq!(retried["status"], "committed");
    assert_eq!(retried["operation"], operation);
    assert_eq!(
        retried["journal_seq"], journal_seq,
        "a retry must reconcile to the same commit, never mint a second"
    );

    // The durable project really mutated — the note is visible
    // through the real snapshot seam.
    let resp = tool_call(
        &mut server.stdio,
        "midi.notes.query",
        &json!({"source": SOURCE10, "range": {"start": 0, "len": 960}}),
    );
    let notes = structured(&resp).to_string();
    assert!(
        notes.contains("720"),
        "the committed note is readable: {notes}"
    );

    // ── Disconnect/restart: a fresh server recovers the journal ──
    drop(server);
    let mut server = Server::spawn(&dir);
    bound_client(&mut server, client_permit["secret"].as_str().unwrap());
    let resp = tool_call(
        &mut server.stdio,
        "operation.get",
        &json!({"operation": operation}),
    );
    let status = structured(&resp);
    assert_eq!(
        status["status"], "committed",
        "restart reconcile reads durable truth: {status}"
    );
    // And the mutation itself survived — real ProjectController +
    // DirectoryBundleStore, not a fixture.
    let resp = tool_call(
        &mut server.stdio,
        "midi.notes.query",
        &json!({"source": SOURCE10, "range": {"start": 0, "len": 960}}),
    );
    let notes = structured(&resp).to_string();
    assert!(
        notes.contains("720"),
        "the committed note survives restart: {notes}"
    );
}

// ── Suggest mode ─────────────────────────────────────────────

#[test]
fn suggest_mode_prepares_but_apply_is_denied_over_real_transport() {
    let dir = fresh_dir("suggest");
    let control_permit = issue_permit(&dir, "control", "human", "user-1", None, None);
    let client_permit = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("ts"),
        Some("gs"),
    );
    let mut server = Server::spawn(&dir);
    let mut control = server.control();
    connect(&mut control, control_permit["secret"].as_str().unwrap());
    provision_authority(&mut control, "ts", "gs", "suggest", FULL_CAPS);
    bound_client(&mut server, client_permit["secret"].as_str().unwrap());

    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(960));
    let prepared = structured(&resp);
    let proposal = prepared["proposal"].as_str().unwrap();
    let digest = prepared["digest"].as_str().unwrap();
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(proposal, digest, None, 1),
    );
    assert_eq!(
        tool_error(&resp),
        "denied",
        "a Suggest-mode agent apply is denied at the gateway"
    );
}

// ── Approve-to-apply mode ────────────────────────────────────

#[test]
fn approve_to_apply_needs_the_control_receipt_over_real_transport() {
    let dir = fresh_dir("a2a");
    let control_permit = issue_permit(&dir, "control", "human", "user-1", None, None);
    let client_permit = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("ta"),
        Some("ga"),
    );
    let mut server = Server::spawn(&dir);
    let mut control = server.control();
    connect(&mut control, control_permit["secret"].as_str().unwrap());
    provision_authority(&mut control, "ta", "ga", "approve-to-apply", FULL_CAPS);
    bound_client(&mut server, client_permit["secret"].as_str().unwrap());

    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(1200));
    let prepared = structured(&resp);
    let proposal = prepared["proposal"].as_str().unwrap().to_owned();
    let digest = prepared["digest"].as_str().unwrap().to_owned();

    // Without a receipt: approval-required, nothing commits.
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(&proposal, &digest, None, 1),
    );
    assert_eq!(structured(&resp)["status"], "approval-required");

    // The operator approves on the control channel — the receipt is
    // bound to proposal + digest + principal + expected revision.
    let resp = tool_call(
        &mut control,
        "proposal.approve",
        &json!({"proposal": proposal, "digest": digest, "expected_revision": 1}),
    );
    let receipt = structured(&resp)["approval"]
        .as_str()
        .expect("approval ref")
        .to_owned();

    // Apply with the receipt — committed.
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(&proposal, &digest, Some(&receipt), 1),
    );
    assert_eq!(structured(&resp)["status"], "committed");

    // The receipt was consumed — a second proposal can't use it.
    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(1440));
    let p2 = structured(&resp);
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(
            p2["proposal"].as_str().unwrap(),
            p2["digest"].as_str().unwrap(),
            Some(&receipt),
            2,
        ),
    );
    assert!(
        result(&resp)["isError"].as_bool().unwrap_or(false),
        "a consumed receipt authorizes nothing: {resp}"
    );
}

// ── Boundary attack battery over the real transport ──────────

#[test]
fn boundary_attacks_fail_closed_over_real_transports() {
    let dir = fresh_dir("security");
    let control_permit = issue_permit(&dir, "control", "human", "user-1", None, None);
    let client_permit = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("t1"),
        Some("g1"),
    );
    let foreign_permit = issue_permit(&dir, "client", "agent", "mcp-client-2", None, None);
    let mut server = Server::spawn(&dir);
    let mut control = server.control();
    connect(&mut control, control_permit["secret"].as_str().unwrap());
    provision_authority(&mut control, "t1", "g1", "delegated-apply", FULL_CAPS);

    // Discovery is not permission — unbound calls refuse.
    let resp = server.stdio.call("tools/list", &json!({}));
    assert_eq!(error_code(&resp), PERMIT_REQUIRED);

    // A client permit on the control channel fails closed.
    let mut wrong = server.control();
    let resp = connect(&mut wrong, client_permit["secret"].as_str().unwrap());
    assert_eq!(error_code(&resp), PERMIT_INVALID);
    // A control permit on the client channel fails closed.
    let resp = connect(
        &mut server.stdio,
        control_permit["secret"].as_str().unwrap(),
    );
    assert_eq!(error_code(&resp), PERMIT_INVALID);
    // A garbage secret fails closed.
    let resp = connect(
        &mut server.stdio,
        "syv-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    assert_eq!(error_code(&resp), PERMIT_INVALID);

    bound_client(&mut server, client_permit["secret"].as_str().unwrap());

    // A committed operation from agent-1 — the foreign probe target.
    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(720));
    let prepared = structured(&resp);
    let proposal = prepared["proposal"].as_str().unwrap().to_owned();
    let digest = prepared["digest"].as_str().unwrap().to_owned();
    let resp = tool_call(
        &mut server.stdio,
        "proposal.apply",
        &apply_args(&proposal, &digest, None, 1),
    );
    let operation = structured(&resp)["operation"].as_str().unwrap().to_owned();

    // A second agent (no grant) cannot even see the handle — a
    // second stdio session attests the Client channel for it.
    let mut foreign_server = Server::spawn(&dir); // NOTE: second host instance
    let resp = connect(
        &mut foreign_server.stdio,
        foreign_permit["secret"].as_str().unwrap(),
    );
    assert_eq!(result(&resp)["status"], "bound");
    let resp = tool_call(
        &mut foreign_server.stdio,
        "operation.get",
        &json!({"operation": operation}),
    );
    assert_eq!(tool_error(&resp), "unknown-operation");

    // Grant revocation denies the very next call.
    let resp = control.call("synvelis.revoke-grant", &json!({"grant": "g1"}));
    result(&resp);
    let resp = tool_call(&mut server.stdio, "midi.edit.prepare", &prepare_args(960));
    assert_eq!(tool_error(&resp), "denied");

    // An oversized inbound message is refused at the transport edge —
    // rejected before JSON parse, so the error carries a null id.
    let big = "x".repeat(300_000);
    server.stdio.send_raw(
        &json!({
            "jsonrpc": "2.0", "id": 99, "method": "tools/call",
            "params": {"name": "project.inspect", "arguments": {"pad": big}},
        })
        .to_string(),
    );
    let resp = server.stdio.read_line();
    assert!(
        resp.get("error").is_some(),
        "oversized message refused: {resp}"
    );
}

// ── Permit CLI roundtrip ─────────────────────────────────────

#[test]
fn permit_cli_provisions_lists_and_revokes() {
    let dir = fresh_dir("permit-cli");
    let issued = issue_permit(
        &dir,
        "client",
        "agent",
        "mcp-client-1",
        Some("t1"),
        Some("g1"),
    );
    assert!(issued["secret"].as_str().unwrap().starts_with("syv-"));
    assert_eq!(issued["channel"], "client");
    let permit_id = issued["permit"].as_str().unwrap().to_owned();

    // `list` exposes records without secrets.
    let listed = permit_cli(&dir, &["list"]);
    let records = listed["permits"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["permit"], permit_id);
    let raw = std::fs::read_to_string(dir.join("permits.json")).unwrap();
    assert!(
        !raw.contains(issued["secret"].as_str().unwrap()),
        "the credential file never stores a usable secret"
    );

    // `revoke` kills the credential — the next serve refuses it.
    let revoked = permit_cli(&dir, &["revoke", "--permit", &permit_id]);
    assert_eq!(revoked["revoked"], true);
    let mut server = Server::spawn(&dir);
    let resp = connect(&mut server.stdio, issued["secret"].as_str().unwrap());
    assert_eq!(error_code(&resp), PERMIT_INVALID);
}
