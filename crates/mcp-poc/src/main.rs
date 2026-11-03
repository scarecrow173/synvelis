//! `mcp-poc` — the GW-08 proof-of-concept transport binary (GIA-004).
//!
//! Two surfaces, one [`synvelis_gateway::McpHost`]:
//!
//! - **`serve --dir <harness>`** runs the MCP session host over
//!   newline-delimited JSON-RPC on **stdio** — the local stdio helper
//!   an MCP host spawns — and, when `--control-port` is given, over an
//!   authenticated `127.0.0.1` line socket that stands in for the
//!   local IPC candidate. The control channel is where Synvelis-side
//!   operator acts enter (permit/policy administration, the trusted
//!   approval GW-09's UI will own) — it is *not* an agent-reachable
//!   surface and exists only on loopback.
//! - **`permit --dir <harness> …`** provisions/revokes connection
//!   permits *offline* against the harness's credential file — the
//!   out-of-band provisioning step the first credential requires
//!   (the same relationship `sshd` has to `authorized_keys`).
//!
//! The harness directory carries real durable state — a
//! `DirectoryBundleStore` project, the GW-06 `commit.journal`, and the
//! permit file — so disconnect/reconnect/restart behavior is honest:
//! a fresh `serve` recovers the commit ledger from the journal, and
//! `operation.get` reconciles against durable truth.
//!
//! This binary is a `PoC` transport, not the product: Streamable-HTTP
//! comparison and remote exposure are explicitly out of scope.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use serde_json::{Value, json};
use synvelis_gateway::commit::GateDeps;
use synvelis_gateway::context::{SelectionFacts, SelectionSource};
use synvelis_gateway::id::{GrantRef, OperationId, ProposalRef, SelectionViewRef, TaskRef};
use synvelis_gateway::mcp::McpBounds;
use synvelis_gateway::snapshot::{self, MemorySnapshotSource, SnapshotSource};
use synvelis_gateway::store::InMemoryPolicyStore;
use synvelis_gateway::{
    APPLY_ID, AdminPort, CommitGate, CommitPort, CommitReceipt, ContextBudget,
    ContextProjectionEngine, ErrorEnvelope, FilePermitStore, Gateway, GrantPolicy, McpChannel,
    McpConfig, McpHost, MutexFence, PermitSpec, PermitStore, PolicyClock, PrepareBudget,
    PrepareEngine, PreparePolicy, Principal, PrincipalKind, ProposalEngine, ProposalGate,
    ProposalStore, ResourceFacts, ResourceOracle, ResourceRef, RetryOutcome, SystemClock,
};
use synvelis_project::{
    Asset, AssetId, Clip, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    CounterIdGenerator, DirectoryBundleStore, Lane, LaneId, MidiDivision, MidiNote, MidiSource,
    MidiTick, MusicalDuration, MusicalTick, NoteId, PlaybackMapping, Project, ProjectId,
    ProjectRevision, ProjectStore, Source, SourceFrame, SourceId, SourceRange, SourceState,
    TimeAnchor, Track, TrackId,
};

// ── The linked-MIDI demo project ─────────────────────────────
//
// The PoC serves a fixed demo project — the same linked-MIDI shape
// the GW-04..06 test rig uses (S1 linked to two clips), so the
// MCP-level evidence exercises the real propagation model:
//
//   track3 ─ lane4 ─ clip11 → media source9 (asset5)
//                 └ clip12 → midi source10 (notes 20/21, PPQN 480)
//           lane5 ─ clip13 → midi source10   ← linked: same S1

fn demo_project() -> Project {
    let project = ProjectId::new(7);
    let mut p = Project::new(project, "demo");
    p.id_watermark = 1000;
    let track = TrackId::new(3);
    let lane = LaneId::new(4);
    let lane5 = LaneId::new(5);
    let src9 = SourceId::new(9);
    let src10 = SourceId::new(10);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t3".to_owned(),
            lane_ids: vec![lane, lane5],
        },
    );
    p.lanes.insert(
        lane,
        Lane {
            id: lane,
            track_id: track,
            clip_ids: vec![ClipId::new(11), ClipId::new(12)],
        },
    );
    p.lanes.insert(
        lane5,
        Lane {
            id: lane5,
            track_id: track,
            clip_ids: vec![ClipId::new(13)],
        },
    );
    p.assets.insert(
        AssetId::new(5),
        Asset {
            id: AssetId::new(5),
            digest: "sha256:asset5".to_owned(),
        },
    );
    p.sources.insert(
        src9,
        Source {
            id: src9,
            asset_id: AssetId::new(5),
            content_digest: "sha256:media9".to_owned(),
            state: SourceState::Verified,
        },
    );
    let mut notes = BTreeMap::new();
    for (id, key, start, len) in [(20u128, 60u8, 0i64, 240i64), (21, 64, 480, 240)] {
        notes.insert(
            NoteId::new(id),
            MidiNote {
                id: NoteId::new(id),
                key,
                velocity: 100,
                start: MidiTick::new(start).unwrap(),
                len: MidiTick::new(len).unwrap(),
            },
        );
    }
    p.midi_sources.insert(
        src10,
        MidiSource {
            id: src10,
            division: MidiDivision::ppqn(480).unwrap(),
            notes,
        },
    );
    for (clip, lane, source) in [
        (ClipId::new(11), lane, src9),
        (ClipId::new(12), lane, src10),
        (ClipId::new(13), lane5, src10),
    ] {
        p.clips.insert(
            clip,
            Clip {
                id: clip,
                lane_id: lane,
                source_id: source,
                placement: ClipPlacement {
                    anchor: TimeAnchor::Musical(MusicalTick::new(120)),
                    extent: ClipExtent::Musical(MusicalDuration::new(240).unwrap()),
                },
                playback: ClipPlayback {
                    source_region: SourceRange::try_new(
                        SourceFrame::new(0).unwrap(),
                        SourceFrame::new(48_000).unwrap(),
                    )
                    .unwrap(),
                    mapping: PlaybackMapping::Natural,
                },
                muted: false,
            },
        );
    }
    p
}

// ── Host seams (same shapes as the test rig) ─────────────────

struct LatestOracle {
    source: Arc<MemorySnapshotSource>,
}

impl ResourceOracle for LatestOracle {
    fn facts(&self, project: ProjectId, resource: &ResourceRef) -> ResourceFacts {
        let Some(rev) = self.source.latest_revision(project) else {
            return ResourceFacts::Unresolved;
        };
        let Some(state) = self.source.pin(project, rev) else {
            return ResourceFacts::Unresolved;
        };
        match snapshot::attributes(&state, resource) {
            Some(attrs) => ResourceFacts::Known(attrs),
            None => ResourceFacts::Unresolved,
        }
    }
}

struct StaticSelection;

impl SelectionSource for StaticSelection {
    fn selection(&self, _project: ProjectId, _view: &SelectionViewRef) -> SelectionFacts {
        SelectionFacts::Unknown
    }
}

/// The commit-boundary and host-administration seams, backed by the
/// real `CommitGate` — an `Accepted` apply drives a real fenced
/// commit, and the journal is the durable verdict reconcile reads.
struct GatePorts {
    gate: Arc<CommitGate>,
    source: Arc<MemorySnapshotSource>,
    project: ProjectId,
}

impl CommitPort for GatePorts {
    fn commit(&self, proposal: &ProposalRef) -> Result<CommitReceipt, ErrorEnvelope> {
        let receipt = self.gate.commit(proposal)?;
        // A committed mutation advances the snapshot seam so the next
        // context query / SEC-11 fence sees the new live revision.
        let live = self.gate.inspect(|c| c.live().clone());
        self.source.commit(self.project, receipt.revision, live);
        Ok(receipt)
    }

    fn reconcile(&self, operation: OperationId) -> RetryOutcome {
        self.gate.reconcile(operation)
    }

    fn cancel(&self, proposal: &ProposalRef, _reason: &str) -> Result<bool, ErrorEnvelope> {
        // The gate's own fenced cancel — serialized with commits, so
        // a cancel racing a commit sees the true apply state.
        self.gate.cancel_apply(proposal)
    }
}

impl AdminPort for GatePorts {
    fn issue_task(&self, task: synvelis_gateway::AuthorizedTask) -> Result<(), ErrorEnvelope> {
        self.gate.issue_task(&task)
    }

    fn issue_grant(&self, grant: synvelis_gateway::Grant) -> Result<(), ErrorEnvelope> {
        self.gate.issue_grant(&grant)
    }

    fn revoke_grant(&self, id: &GrantRef) -> Result<(), ErrorEnvelope> {
        self.gate.revoke_grant(id)
    }

    fn revoke_task(&self, id: &TaskRef) -> Result<(), ErrorEnvelope> {
        self.gate.revoke_task(id)
    }

    fn live_revision(&self, project: ProjectId) -> Option<ProjectRevision> {
        (project == self.project).then(|| self.gate.live_revision())
    }
}

// ── Harness boot ─────────────────────────────────────────────

struct Harness {
    host: Arc<McpHost>,
    #[allow(dead_code)] // the gate lives behind the ports; the field pins ownership
    gate: Arc<CommitGate>,
}

/// The gateway plus the proposal store `boot` still needs for the
/// commit gate.
type GwStack = (Gateway, Arc<ProposalStore>);

/// The GW-01..05 engine stack and its gateway — capabilities
/// registered, proposals wired, policy evaluated by `ProposalGate`.
fn gateway_stack(
    policy: &Arc<InMemoryPolicyStore>,
    clock: &Arc<SystemClock>,
    source: &Arc<MemorySnapshotSource>,
    oracle: Arc<dyn ResourceOracle>,
) -> Result<GwStack, Box<dyn std::error::Error>> {
    let ctx = Arc::new(ContextProjectionEngine::new(
        source.clone(),
        Arc::new(StaticSelection),
        Arc::new(GrantPolicy::new(
            policy.clone(),
            clock.clone(),
            oracle.clone(),
        )),
        clock.clone(),
        ContextBudget::default(),
    ));
    let proposals = Arc::new(ProposalStore::new(
        clock.clone(),
        3_600_000,
        1_800_000,
        PrepareBudget::default().max_proposals,
    ));
    let prepare = Arc::new(PrepareEngine::new(
        ctx.clone(),
        Arc::new(PreparePolicy::new(policy.clone(), clock.clone())),
        PrepareBudget::default(),
        proposals.clone(),
    ));
    let proposal_engine = Arc::new(ProposalEngine::new(
        proposals.clone(),
        source.clone(),
        policy.clone(),
        Arc::new(PreparePolicy::new(policy.clone(), clock.clone())),
    ));
    let mut gw = Gateway::with_evaluator(Box::new(ProposalGate::new(
        proposals.clone(),
        Box::new(GrantPolicy::new(policy.clone(), clock.clone(), oracle)),
    )));
    ctx.install(&mut gw)?;
    prepare.install(&mut gw)?;
    proposal_engine.install(&mut gw)?;
    Ok((gw, proposals))
}

/// Builds the full GW-01..06 stack + MCP host over `dir`'s durable
/// state (created empty on first boot).
fn boot(dir: &Path) -> Result<Harness, Box<dyn std::error::Error>> {
    let store_dir = dir.join("project");
    let mut store = DirectoryBundleStore::create(&store_dir)?;
    if store.head()?.is_none() {
        let mut controller = synvelis_project::ProjectController::new(
            demo_project(),
            Box::new(CounterIdGenerator::new(9_000)),
        );
        controller.apply(Command {
            expected_revision: controller.live_revision(),
            op: synvelis_project::ProjectOp::SetProjectName {
                name: "demo".to_owned(),
            },
        })?;
        controller.save(&mut store)?;
    }

    let project_id = ProjectId::new(7);
    let policy = Arc::new(InMemoryPolicyStore::new());
    let clock: Arc<SystemClock> = Arc::new(SystemClock);
    let source = Arc::new(MemorySnapshotSource::new());
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: source.clone(),
    });
    let (gw, proposals) = gateway_stack(&policy, &clock, &source, oracle)?;
    let apply_capability = gw
        .registry()
        .describe(&synvelis_gateway::capability::CapabilityRef {
            id: synvelis_gateway::CapabilityId::new(APPLY_ID)?,
            version: synvelis_gateway::CapabilityVersion::new(1, 0),
        })
        .expect("apply capability registered")
        .clone();

    let (gate, _report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        GateDeps {
            policy: policy.clone(),
            admin: policy.clone(),
            evaluator: GrantPolicy::new(policy.clone(), clock.clone(), oracle_gate(source.clone())),
            proposals: proposals.clone(),
            apply_capability,
        },
        MutexFence::new(),
    )?;
    let gate = Arc::new(gate);
    // The snapshot seam serves the recovered live tip — restart
    // reconcile and SEC-11 both read this view.
    let live = gate.inspect(|c| c.live().clone());
    source.commit(project_id, gate.live_revision(), live);

    let ports = Arc::new(GatePorts {
        gate: gate.clone(),
        source: source.clone(),
        project: project_id,
    });
    let permits = FilePermitStore::open(dir.join("permits.json"))?;
    let host = Arc::new(McpHost::new(
        gw,
        Box::new(permits),
        ports.clone(),
        ports,
        clock,
        McpConfig {
            adapter: synvelis_gateway::AdapterId::new(synvelis_gateway::mcp::MCP_ADAPTER_RAW),
            bounds: McpBounds::default(),
            enabled: true,
        },
    ));
    Ok(Harness { host, gate })
}

fn oracle_gate(source: Arc<MemorySnapshotSource>) -> Arc<dyn ResourceOracle> {
    Arc::new(LatestOracle { source })
}

// ── Transports ───────────────────────────────────────────────

/// Reads one newline-delimited frame within `max` bytes — the bound
/// caps the allocation itself, so an unterminated or over-long frame
/// can never grow `buf` past `max`. The remainder of an oversized
/// frame is drained (never buffered) so the stream stays framed.
/// Returns `Some(true)` for an in-bounds frame, `Some(false)` for an
/// oversized one (`buf` holds the truncated head), `None` on EOF.
fn read_frame<R: BufRead>(
    reader: &mut R,
    max: usize,
    buf: &mut Vec<u8>,
) -> io::Result<Option<bool>> {
    buf.clear();
    let mut oversized = false;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            return Ok(if buf.is_empty() && !oversized {
                None
            } else {
                Some(!oversized)
            });
        }
        let nl = chunk.iter().position(|b| *b == b'\n');
        let end = nl.map_or(chunk.len(), |p| p + 1);
        if !oversized {
            let room = max - buf.len();
            oversized = end > room;
            buf.extend_from_slice(&chunk[..end.min(room)]);
        }
        reader.consume(end);
        if nl.is_some() {
            return Ok(Some(!oversized));
        }
    }
}

/// One newline-delimited JSON-RPC session on `stream` — the loop the
/// stdio helper and every control connection run.
fn serve_stream<R: BufRead, W: Write>(
    host: &Arc<McpHost>,
    channel: McpChannel,
    mut reader: R,
    mut writer: W,
) {
    let mut session = host.new_session(channel);
    let max = host.bounds().max_message_bytes;
    let mut buf = Vec::new();
    while let Ok(Some(in_bounds)) = read_frame(&mut reader, max, &mut buf) {
        let response = if in_bounds {
            let line = std::str::from_utf8(&buf)
                .unwrap_or("")
                .trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(line) {
                Ok(msg) => host.handle(&mut session, &msg),
                Err(_) => Some(synvelis_gateway::mcp::rpc_error(
                    &Value::Null,
                    synvelis_gateway::mcp::PARSE_ERROR,
                    "invalid JSON-RPC payload",
                    None,
                )),
            }
        } else {
            Some(synvelis_gateway::mcp::rpc_error(
                &Value::Null,
                synvelis_gateway::mcp::INVALID_PARAMS,
                "message exceeds the adapter bound",
                Some(json!({"max": max})),
            ))
        };
        if let Some(resp) = response
            && (writeln!(writer, "{resp}").is_err() || writer.flush().is_err())
        {
            break;
        }
    }
}

// ── CLI ──────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("serve") => cmd_serve(&args[1..]),
        Some("permit") => cmd_permit(&args[1..]),
        _ => usage(),
    };
    std::process::exit(code);
}

fn usage() -> i32 {
    eprintln!(
        "usage:\n  \
         mcp-poc serve --dir <harness> [--control-port <port>]\n  \
         mcp-poc permit --dir <harness> issue --channel <client|control> \\\n    \
           --principal-kind <agent|human> --principal <id> --project <tagged> \\\n    \
           [--task <ref>] [--grant <ref>] [--ttl-ms <ms>]\n  \
         mcp-poc permit --dir <harness> revoke --permit <id>\n  \
         mcp-poc permit --dir <harness> list"
    );
    2
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].as_str())
}

fn require<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    flag(args, name).ok_or_else(|| format!("missing {name}"))
}

fn cmd_serve(args: &[String]) -> i32 {
    let dir = match require(args, "--dir") {
        Ok(d) => PathBuf::from(d),
        Err(e) => return fail(&e),
    };
    let control_port = flag(args, "--control-port").map(str::parse::<u16>);
    let harness = match boot(&dir) {
        Ok(h) => h,
        Err(e) => return fail(&format!("harness boot failed: {e}")),
    };
    if let Some(port) = control_port {
        let Ok(port) = port else {
            return fail("--control-port must be a port number");
        };
        let host = harness.host.clone();
        thread::spawn(move || control_listener(&host, port));
    }
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    serve_stream(
        &harness.host,
        McpChannel::Client,
        stdin.lock(),
        stdout.lock(),
    );
    0
}

/// The loopback control socket — one JSON-RPC session per
/// connection, all attesting `McpChannel::Control`. Connections past
/// the cap are dropped rather than queued — the loopback `PoC` never
/// needs more than a handful of operator sessions.
fn control_listener(host: &Arc<McpHost>, port: u16) {
    const MAX_CONTROL_CONNS: usize = 8;
    let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else {
        return;
    };
    let live = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for conn in listener.incoming().flatten() {
        let host = host.clone();
        let live = live.clone();
        if live.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= MAX_CONTROL_CONNS {
            live.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            continue;
        }
        thread::spawn(move || {
            if let Ok(read) = conn.try_clone() {
                serve_stream(&host, McpChannel::Control, BufReader::new(read), conn);
            }
            live.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        });
    }
}

fn cmd_permit(args: &[String]) -> i32 {
    let dir = match require(args, "--dir") {
        Ok(d) => PathBuf::from(d),
        Err(e) => return fail(&e),
    };
    let verb = args
        .iter()
        .find(|a| !a.starts_with("--") && **a != dir.to_string_lossy());
    let path = dir.join("permits.json");
    match verb.map(String::as_str) {
        Some("issue") => permit_issue(args, &path),
        Some("revoke") => permit_revoke(args, &path),
        Some("list") => permit_list(&path),
        _ => usage(),
    }
}

fn permit_issue(args: &[String], path: &Path) -> i32 {
    let parsed = (|| -> Result<PermitSpec, String> {
        let channel = match require(args, "--channel")? {
            "client" => McpChannel::Client,
            "control" => McpChannel::Control,
            other => return Err(format!("unknown --channel: {other}")),
        };
        let kind = match require(args, "--principal-kind")? {
            "agent" => PrincipalKind::Agent,
            "human" => PrincipalKind::Human,
            other => return Err(format!("unknown --principal-kind: {other}")),
        };
        let principal =
            Principal::new(kind, require(args, "--principal")?).map_err(|e| e.to_string())?;
        let project = ProjectId::from_tagged(require(args, "--project")?)
            .map_err(|_| "--project must be a tagged project id".to_owned())?;
        let task = flag(args, "--task").map(|t| TaskRef::new(t.to_owned()));
        let grant = flag(args, "--grant").map(|g| GrantRef::new(g.to_owned()));
        Ok(PermitSpec {
            channel,
            principal,
            project,
            task: task.transpose().map_err(|e| e.to_string())?,
            grant: grant.transpose().map_err(|e| e.to_string())?,
            ttl_ms: flag(args, "--ttl-ms")
                .map(str::parse::<u64>)
                .transpose()
                .map_err(|_| "--ttl-ms must be an integer".to_owned())?,
        })
    })();
    let spec = match parsed {
        Ok(s) => s,
        Err(e) => return fail(&e),
    };
    if let Err(e) = synvelis_gateway::mcp_permit::check_channel_kind(&spec) {
        return fail(&e.to_string());
    }
    let mut store = match FilePermitStore::open(path) {
        Ok(s) => s,
        Err(e) => return fail(&format!("permit store: {e}")),
    };
    match store.issue(spec, SystemClock.now()) {
        Ok(issue) => {
            println!(
                "{}",
                json!({
                    "permit": issue.record.permit,
                    "secret": issue.secret,
                    "channel": issue.record.channel,
                    "principal": issue.record.principal.id(),
                    "expires_at": issue.record.expires_at.map(|e| e.0),
                })
            );
            0
        }
        Err(e) => fail(&format!("permit issue failed: {e}")),
    }
}

fn permit_revoke(args: &[String], path: &Path) -> i32 {
    let permit = match require(args, "--permit") {
        Ok(p) => p,
        Err(e) => return fail(&e),
    };
    let mut store = match FilePermitStore::open(path) {
        Ok(s) => s,
        Err(e) => return fail(&format!("permit store: {e}")),
    };
    match store.revoke(permit) {
        Ok(true) => {
            println!("{}", json!({"permit": permit, "revoked": true}));
            0
        }
        Ok(false) => fail("unknown permit"),
        Err(e) => fail(&format!("permit revoke failed: {e}")),
    }
}

fn permit_list(path: &Path) -> i32 {
    let store = match FilePermitStore::open(path) {
        Ok(s) => s,
        Err(e) => return fail(&format!("permit store: {e}")),
    };
    // `record` needs an id — enumerate is not part of the trait;
    // the CLI reads the file directly for operator inspection.
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| "{}".to_owned());
    let file: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({"permits": []}));
    let permits: Vec<Value> = file
        .get("permits")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            json!({
                "permit": r.get("permit"),
                "channel": r.get("channel"),
                "principal": r.pointer("/principal/id"),
                "revoked": r.get("revoked"),
            })
        })
        .collect();
    let _ = store;
    println!("{}", json!({"permits": permits, "count": permits.len()}));
    0
}

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    1
}
