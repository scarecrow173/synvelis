//! Shared real-transport harness for `mcp-poc` integration tests —
//! the newline-delimited JSON-RPC endpoint, the spawned `serve`
//! child, and the request/result helpers every suite reuses. Nothing
//! here is a fixture: this drives the actual binary over real stdio
//! pipes and the authenticated loopback control channel.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

pub const BIN: &str = env!("CARGO_BIN_EXE_mcp-poc");
pub const PROJECT: &str = "project-00000000000000000000000000000007";
pub const SOURCE10: &str = "source-0000000000000000000000000000000a";
pub const CLIP11: &str = "clip-0000000000000000000000000000000b";
pub const CLIP12: &str = "clip-0000000000000000000000000000000c";
pub const CLIP13: &str = "clip-0000000000000000000000000000000d";

pub const PERMIT_INVALID: i64 = -32001;
pub const PERMIT_REQUIRED: i64 = -32002;

static HARNESS_SEQ: AtomicU64 = AtomicU64::new(0);

// ── Line-transport helpers ───────────────────────────────────

/// A bidirectional newline-delimited JSON-RPC endpoint — over a
/// child's stdio pair or a TCP stream; identical wire discipline.
pub struct LineIo {
    writer: Box<dyn Write + Send>,
    reader: BufReader<Box<dyn Read + Send>>,
    next_id: u64,
}

impl LineIo {
    /// Sends one request and reads its response line.
    pub fn call(&mut self, method: &str, params: &Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send_raw(&req.to_string());
        let resp = self.read_line();
        assert_eq!(resp["id"], id, "response id mismatch for {method}");
        resp
    }

    pub fn send_raw(&mut self, text: &str) {
        self.writer.write_all(text.as_bytes()).expect("wire write");
        self.writer.write_all(b"\n").expect("wire newline");
        self.writer.flush().expect("wire flush");
    }

    pub fn read_line(&mut self) -> Value {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).expect("wire read");
        assert!(n > 0, "server closed the transport");
        serde_json::from_str(&line).expect("response is valid JSON")
    }
}

pub fn result(resp: &Value) -> &Value {
    assert!(resp.get("error").is_none(), "rpc error: {resp}");
    &resp["result"]
}

pub fn error_code(resp: &Value) -> i64 {
    resp["error"]["code"].as_i64().expect("error.code")
}

pub fn structured(resp: &Value) -> &Value {
    &result(resp)["structuredContent"]
}

pub fn tool_error(resp: &Value) -> String {
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

pub fn fresh_dir(tag: &str) -> PathBuf {
    let seq = HARNESS_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mcp-e2e-{}-{tag}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `mcp-poc permit` — offline provisioning; returns the JSON the CLI
/// printed (which includes the one-time secret).
pub fn permit_cli(dir: &Path, args: &[&str]) -> Value {
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

pub fn issue_permit(
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
pub struct Server {
    pub child: Child,
    pub stdio: LineIo,
    pub port: u16,
}

impl Server {
    pub fn spawn(dir: &Path) -> Self {
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
    pub fn control(&self) -> LineIo {
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

pub fn connect(io: &mut LineIo, secret: &str) -> Value {
    io.call("synvelis.connect", &json!({"permit": secret}))
}

pub fn bound_client(server: &mut Server, secret: &str) {
    let resp = connect(&mut server.stdio, secret);
    assert_eq!(result(&resp)["status"], "bound");
}

pub fn tool_call(io: &mut LineIo, name: &str, arguments: &Value) -> Value {
    io.call("tools/call", &json!({"name": name, "arguments": arguments}))
}

/// `synvelis.issue-task` + `synvelis.issue-grant` over the control
/// channel — operator provisioning of agent authority.
pub fn provision_authority(
    control: &mut LineIo,
    task: &str,
    grant: &str,
    policy: &str,
    caps: &[&str],
) {
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
pub fn prepare_args(start: i64) -> Value {
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

pub fn apply_args(proposal: &str, digest: &str, approval: Option<&str>, expected: u64) -> Value {
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

pub const FULL_CAPS: &[&str] = &[
    "synvelis.context.query",
    "synvelis.operation.prepare",
    "synvelis.proposal.preview",
    "synvelis.proposal.apply",
    "synvelis.midi.note.add",
];
