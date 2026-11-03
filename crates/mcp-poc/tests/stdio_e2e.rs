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
//! real wire.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_mcp-poc");
const PROJECT: &str = "project-00000000000000000000000000000007";
const SOURCE10: &str = "source-0000000000000000000000000000000a";
const CLIP11: &str = "clip-0000000000000000000000000000000b";
const CLIP12: &str = "clip-0000000000000000000000000000000c";
const CLIP13: &str = "clip-0000000000000000000000000000000d";

const PERMIT_INVALID: i64 = -32001;
const PERMIT_REQUIRED: i64 = -32002;

static HARNESS_SEQ: AtomicU64 = AtomicU64::new(0);

// ── Line-transport helpers ───────────────────────────────────

/// A bidirectional newline-delimited JSON-RPC endpoint — over a
/// child's stdio pair or a TCP stream; identical wire discipline.
struct LineIo {
    writer: Box<dyn Write + Send>,
    reader: BufReader<Box<dyn Read + Send>>,
    next_id: u64,
}

impl LineIo {
    /// Sends one request and reads its response line.
    fn call(&mut self, method: &str, params: &Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send_raw(&req.to_string());
        let resp = self.read_line();
        assert_eq!(resp["id"], id, "response id mismatch for {method}");
        resp
    }

    fn send_raw(&mut self, text: &str) {
        self.writer.write_all(text.as_bytes()).expect("wire write");
        self.writer.write_all(b"\n").expect("wire newline");
        self.writer.flush().expect("wire flush");
    }

    fn read_line(&mut self) -> Value {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).expect("wire read");
        assert!(n > 0, "server closed the transport");
        serde_json::from_str(&line).expect("response is valid JSON")
    }
}

fn result(resp: &Value) -> &Value {
    assert!(resp.get("error").is_none(), "rpc error: {resp}");
    &resp["result"]
}

fn error_code(resp: &Value) -> i64 {
    resp["error"]["code"].as_i64().expect("error.code")
}

fn structured(resp: &Value) -> &Value {
    &result(resp)["structuredContent"]
}

fn tool_error(resp: &Value) -> String {
    assert_eq!(
        result(resp)["isError"],
        true,
        "expected isError tool result: {resp}"
    );
    structured(resp)["error"]["code"]
        .as_str()
        .expect("tool error code")
        .to_owned()
}

// ── Harness + server processes ───────────────────────────────

fn fresh_dir(tag: &str) -> PathBuf {
    let seq = HARNESS_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mcp-e2e-{}-{tag}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `mcp-poc permit` — offline provisioning; returns the JSON the CLI
/// printed (which includes the one-time secret).
fn permit_cli(dir: &Path, args: &[&str]) -> Value {
    let output = Command::new(BIN)
        .arg("permit")
        .arg("--dir")
        .arg(dir)
        .args(args)
        .output()
        .expect("mcp-poc permit runs");
    assert!(
        output.status.success(),
        "permit {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("permit prints JSON")
}

fn issue_permit(
    dir: &Path,
    channel: &str,
    kind: &str,
    principal: &str,
    task: Option<&str>,
    grant: Option<&str>,
) -> Value {
    let mut args = vec![
        "issue",
        "--channel",
        channel,
        "--principal-kind",
        kind,
        "--principal",
        principal,
        "--project",
        PROJECT,
    ];
    if let Some(t) = task {
        args.extend(["--task", t]);
    }
    if let Some(g) = grant {
        args.extend(["--grant", g]);
    }
    permit_cli(dir, &args)
}

/// A running `mcp-poc serve` — stdio client channel + loopback
/// control socket. Dropping it kills the child.
struct Server {
    child: Child,
    stdio: LineIo,
    port: u16,
}

impl Server {
    fn spawn(dir: &Path) -> Self {
        let port = free_port();
        let mut child = Command::new(BIN)
            .arg("serve")
            .arg("--dir")
            .arg(dir)
            .arg("--control-port")
            .arg(port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("mcp-poc serve spawns");
        let stdin: ChildStdin = child.stdin.take().expect("stdin pipe");
        let stdout: ChildStdout = child.stdout.take().expect("stdout pipe");
        Self {
            stdio: LineIo {
                writer: Box::new(stdin),
                reader: BufReader::new(Box::new(stdout)),
                next_id: 0,
            },
            child,
            port,
        }
    }

    /// A fresh control-channel connection (one session per socket).
    fn control(&self) -> LineIo {
        // The listener binds lazily in its thread — retry briefly.
        let mut last = None;
        for _ in 0..50 {
            match TcpStream::connect(("127.0.0.1", self.port)) {
                Ok(stream) => {
                    let read = stream.try_clone().expect("clone");
                    return LineIo {
                        writer: Box::new(stream),
                        reader: BufReader::new(Box::new(read)),
                        next_id: 0,
                    };
                }
                Err(e) => last = Some(e),
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("control listener never bound: {last:?}");
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

fn connect(io: &mut LineIo, secret: &str) -> Value {
    io.call("synvelis.connect", &json!({"permit": secret}))
}

fn bound_client(server: &mut Server, secret: &str) {
    let resp = connect(&mut server.stdio, secret);
    assert_eq!(result(&resp)["status"], "bound");
}

fn tool_call(io: &mut LineIo, name: &str, arguments: &Value) -> Value {
    io.call("tools/call", &json!({"name": name, "arguments": arguments}))
}

/// `synvelis.issue-task` + `synvelis.issue-grant` over the control
/// channel — operator provisioning of agent authority.
fn provision_authority(control: &mut LineIo, task: &str, grant: &str, policy: &str, caps: &[&str]) {
    let resp = control.call(
        "synvelis.issue-task",
        &json!({"task": task, "principal": "mcp-client-1", "project": PROJECT}),
    );
    result(&resp);
    let resp = control.call(
        "synvelis.issue-grant",
        &json!({
            "id": grant,
            "task": task,
            "principal": "mcp-client-1",
            "project": PROJECT,
            "capabilities": caps,
            "permissions": ["mutate-project", "read-context", "disclose-context"],
            "data_access": ["metadata-read", "structured-content-read", "external-disclosure"],
            "scope_resources": [CLIP11, CLIP12, CLIP13, SOURCE10],
            "create_parents": [SOURCE10],
            "create_kinds": ["note"],
            "approval_policy": policy,
            "issued_at_revision": 1,
        }),
    );
    result(&resp);
}

/// A `midi.edit.prepare` for a note.add on source10 at base rev 1.
fn prepare_args(start: i64) -> Value {
    json!({
        "operation": {
            "capability": "synvelis.midi.note.add",
            "target": {"kind": "source", "id": SOURCE10},
            "args": {"key": 60, "velocity": 90,
                     "range": {"domain": "source-native", "start": start, "len": 240}},
        },
        "base_revision": 1,
    })
}

fn apply_args(proposal: &str, digest: &str, approval: Option<&str>, expected: u64) -> Value {
    let mut args = json!({
        "proposal": proposal,
        "digest": digest,
        "expected_revision": expected,
    });
    if let Some(a) = approval {
        args["approval"] = json!(a);
    }
    args
}

const FULL_CAPS: &[&str] = &[
    "synvelis.context.query",
    "synvelis.operation.prepare",
    "synvelis.proposal.preview",
    "synvelis.proposal.apply",
    "synvelis.midi.note.add",
];

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
