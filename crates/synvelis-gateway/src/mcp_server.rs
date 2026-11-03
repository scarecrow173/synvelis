//! GW-08 MCP server adapter — the session host (GIA-004).
//!
//! [`McpHost`] is the transport-agnostic core of the adapter: a
//! transport (stdio lines, a loopback socket, …) hands one JSON-RPC
//! message in and carries the response value out. The host owns:
//!
//! - **Session binding** — `synvelis.connect` presents a connection
//!   permit ([`crate::mcp_permit`]); a verified permit mints the
//!   session's trusted [`CallerIdentity`] through the crate-internal
//!   seal. MCP `clientInfo`, tool arguments, and self-declared ids
//!   play no role in identity.
//! - **Method dispatch** — initialize / ping / tools / resources /
//!   prompts plus the control-channel host methods (`synvelis.*`).
//!   A client channel never sees the control methods exist.
//! - **The operation book** — the adapter-side record of accepted
//!   applies: operation id, owning principal/task/grant, tool, and
//!   the verdict the commit boundary settled. `operation.get`
//!   reconciles against [`CommitPort`] — the gateway ledger — so
//!   reconnects answer from durable truth, and foreign handles share
//!   one blind verdict.
//!
//! What the host does **not** own: transport framing, project state,
//! policy authority, and the commit fence. Commit/project access is
//! injected behind [`CommitPort`]/[`AdminPort`] so the in-crate tests
//! can run the full session layer with no commit boundary wired.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use synvelis_project::{ProjectId, ProjectRevision};

use crate::Gateway;
use crate::clock::{PolicyClock, PolicyInstant};
use crate::commit::{CommitOutcome, CommitReceipt, RetryOutcome};
use crate::grant::Grant;
use crate::id::{AdapterId, GrantRef, OperationId, ProposalRef, TaskRef};
use crate::mcp::{
    self, ADAPTER_DISABLED, CONNECT_METHOD, CallToolResult, FORBIDDEN, INTERNAL_ERROR,
    INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND, McpBounds, McpChannel, PERMIT_INVALID,
    PERMIT_REQUIRED, PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION, TOOL_CONTRACT,
    UNKNOWN_RESOURCE, rpc_error, rpc_result,
};
use crate::mcp_permit::{self, PermitIssue, PermitRecord, PermitSpec, PermitStore};
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::request::{CallerIdentity, Principal};
use crate::store::AuthorizedTask;

/// Host construction inputs the `PoC` wiring chooses explicitly.
#[derive(Clone, Debug)]
pub struct McpConfig {
    /// The adapter endpoint identity bound callers attest through.
    pub adapter: AdapterId,
    /// Bounded-transfer limits (see [`McpBounds`]).
    pub bounds: McpBounds,
    /// Whether the adapter serves at all — `AI/MCP OFF` must leave the
    /// DAW unaffected, so the default construction is off.
    pub enabled: bool,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            adapter: AdapterId::new(mcp::MCP_ADAPTER_RAW),
            bounds: McpBounds::default(),
            enabled: false,
        }
    }
}

/// The commit-boundary seam the adapter drives after `Accepted`.
///
/// Implemented by the `PoC`'s `CommitGate` wrapper; the in-crate host
/// runs with [`NoCommitPort`] so the adapter layer itself never
/// fabricates commit authority.
#[allow(clippy::result_large_err)] // typed envelopes are the contract
pub trait CommitPort: Send + Sync {
    /// Drives one accepted apply to its settled outcome.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] for request-level and non-commit outcomes —
    /// determined verdicts are recorded by the gate, so
    /// [`reconcile`][Self::reconcile] is the ground truth afterwards.
    fn commit(&self, proposal: &ProposalRef) -> Result<CommitReceipt, ErrorEnvelope>;

    /// The durable verdict for an operation — `Settled`,
    /// `InFlight`, or `Unknown`.
    fn reconcile(&self, operation: OperationId) -> RetryOutcome;

    /// Settles a still-pending apply as rejected (operator/caller
    /// cancellation). Returns `Ok(true)` when the settle landed,
    /// `Ok(false)` when the apply was already past cancellation —
    /// the caller then reports the real status instead of a false
    /// `cancelled`. Default: unsupported — a commit boundary that
    /// cannot settle reports the function unavailable rather than
    /// claiming a cancel it did not perform.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — no pending apply, illegal transition, or
    /// the port cannot settle at all.
    fn cancel(&self, proposal: &ProposalRef, _reason: &str) -> Result<bool, ErrorEnvelope> {
        let _ = proposal;
        Err(GatewayError::Internal {
            reason: "this adapter has no commit boundary to settle applies".to_owned(),
        }
        .into())
    }
}

/// A host with no commit boundary — applies are `Accepted` and
/// reported `commit-requested`; the operation book records them and
/// `operation.get` answers from the book alone.
pub struct NoCommitPort;

impl CommitPort for NoCommitPort {
    fn commit(&self, _proposal: &ProposalRef) -> Result<CommitReceipt, ErrorEnvelope> {
        Err(GatewayError::Internal {
            reason: "no commit boundary is wired to this adapter".to_owned(),
        }
        .into())
    }

    fn reconcile(&self, _operation: OperationId) -> RetryOutcome {
        RetryOutcome::Unknown
    }
}

/// The host-administration seam the control channel consumes —
/// policy issuance/revocation and the live revision probe. Implemented
/// by the `PoC` around `CommitGate`'s fenced admin surface; absent in
/// the in-crate host.
#[allow(clippy::result_large_err)] // typed envelopes are the contract
pub trait AdminPort: Send + Sync {
    /// Issues an authorized task through the durable admin surface.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] on admin failure.
    fn issue_task(&self, task: AuthorizedTask) -> Result<(), ErrorEnvelope>;
    /// Issues a grant.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] on admin failure.
    fn issue_grant(&self, grant: Grant) -> Result<(), ErrorEnvelope>;
    /// Records revocation of `id` (revocation is a state, not a delete).
    ///
    /// # Errors
    /// [`ErrorEnvelope`] on admin failure.
    fn revoke_grant(&self, id: &GrantRef) -> Result<(), ErrorEnvelope>;
    /// Records revocation of `id`.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] on admin failure.
    fn revoke_task(&self, id: &TaskRef) -> Result<(), ErrorEnvelope>;
    /// The live committed revision, when the host exposes one.
    fn live_revision(&self, project: ProjectId) -> Option<ProjectRevision>;
}

/// A host with no admin surface — control-channel host methods fail
/// closed as unavailable.
pub struct NoAdminPort;

impl AdminPort for NoAdminPort {
    fn issue_task(&self, _task: AuthorizedTask) -> Result<(), ErrorEnvelope> {
        Err(Self::err())
    }
    fn issue_grant(&self, _grant: Grant) -> Result<(), ErrorEnvelope> {
        Err(Self::err())
    }
    fn revoke_grant(&self, _id: &GrantRef) -> Result<(), ErrorEnvelope> {
        Err(Self::err())
    }
    fn revoke_task(&self, _id: &TaskRef) -> Result<(), ErrorEnvelope> {
        Err(Self::err())
    }
    fn live_revision(&self, _project: ProjectId) -> Option<ProjectRevision> {
        None
    }
}

impl NoAdminPort {
    fn err() -> ErrorEnvelope {
        GatewayError::Internal {
            reason: "no host administration surface is wired to this adapter".to_owned(),
        }
        .into()
    }
}

/// The settled/in-flight status the operation book tracks.
#[derive(Clone, Debug, PartialEq)]
pub enum OpStatus {
    /// The apply was accepted; the commit decision is not recorded
    /// (no commit port, or the port's reconcile is `Unknown`).
    CommitRequested,
    /// Committed — carries the durable receipt fields the client needs.
    Committed {
        /// The revision the commit produced.
        revision: u64,
        /// The durable commit id.
        commit: String,
        /// The journal sequence.
        journal_seq: u64,
    },
    /// The commit boundary rejected the apply.
    Rejected {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// Revision conflict at the commit boundary.
    Conflicted {
        /// The asserted expected revision.
        expected: u64,
        /// The live revision observed.
        actual: u64,
    },
    /// The outcome crossed a persistence boundary undecided —
    /// reconcile, never silently retry.
    Indeterminate {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// A still-pending apply was settled as rejected via
    /// `operation.cancel`.
    Cancelled {
        /// Safe-to-disclose reason.
        reason: String,
    },
}

impl OpStatus {
    /// The wire status name (`operation.get` results and apply verdicts).
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::CommitRequested => "commit-requested",
            Self::Committed { .. } => "committed",
            Self::Rejected { .. } => "rejected",
            Self::Conflicted { .. } => "conflicted",
            Self::Indeterminate { .. } => "indeterminate",
            Self::Cancelled { .. } => "cancelled",
        }
    }

    /// Whether the status is terminal.
    #[must_use]
    pub fn terminal(&self) -> bool {
        !matches!(self, Self::CommitRequested)
    }

    /// Maps the durable commit verdict into the book's vocabulary.
    #[must_use]
    pub fn of_outcome(outcome: &CommitOutcome) -> Self {
        match outcome {
            CommitOutcome::Committed(receipt) => Self::Committed {
                revision: receipt.revision.0,
                commit: receipt.commit.to_tagged(),
                journal_seq: receipt.journal_seq,
            },
            CommitOutcome::Rejected { reason } => Self::Rejected {
                reason: reason.clone(),
            },
            CommitOutcome::Conflicted { expected, actual } => Self::Conflicted {
                expected: expected.0,
                actual: actual.0,
            },
            CommitOutcome::Indeterminate { reason } => Self::Indeterminate {
                reason: reason.clone(),
            },
        }
    }
}

/// One adapter-side operation record — the book never holds
/// authority, only the correlation a reconnect needs.
struct OpRecord {
    /// The tool that produced the accept.
    tool: String,
    /// The proposal the apply names (needed by `operation.cancel`).
    proposal: Option<ProposalRef>,
    /// Owning principal — scoping, not authority.
    principal: Principal,
    /// Owning project.
    project: ProjectId,
    /// Owning task ref — a foreign session's op is invisible.
    task: Option<TaskRef>,
    /// Owning grant ref.
    grant: Option<GrantRef>,
    /// The tracked status.
    status: OpStatus,
}

impl OpRecord {
    /// The ownership scope — one session's records are invisible to
    /// a session bound on a different scope.
    fn same_scope(&self, bound: &BoundIdentity) -> bool {
        self.principal == bound.record.principal
            && self.project == bound.record.project
            && self.task == bound.record.task
            && self.grant == bound.record.grant
    }
}

struct OpBook {
    map: BTreeMap<OperationId, OpRecord>,
    order: VecDeque<OperationId>,
    cap: usize,
}

impl OpBook {
    /// Inserts the record, evicting the oldest *same-scope* entry
    /// when the book is full — a flooding session can never evict
    /// another session's correlation data. With no same-scope victim
    /// the insert is declined rather than spilling across scopes.
    fn insert(&mut self, operation: OperationId, record: OpRecord, bound: &BoundIdentity) {
        if self.map.contains_key(&operation) {
            return;
        }
        while self.map.len() >= self.cap {
            let victim = self
                .order
                .iter()
                .position(|op| self.map.get(op).is_some_and(|r| r.same_scope(bound)))
                .map(|pos| self.order.remove(pos).expect("position found"));
            match victim {
                Some(oldest) => {
                    self.map.remove(&oldest);
                }
                None => return,
            }
        }
        self.order.push_back(operation);
        self.map.insert(operation, record);
    }

    /// The record `operation` names iff it belongs to this session's
    /// (principal, project, task, grant) scope — foreign and unknown
    /// share one `None`, the same existence blinding the cursor
    /// store uses.
    fn scoped(&self, operation: OperationId, bound: &BoundIdentity) -> Option<&OpRecord> {
        let record = self.map.get(&operation)?;
        record.same_scope(bound).then_some(record)
    }

    fn scoped_mut(
        &mut self,
        operation: OperationId,
        bound: &BoundIdentity,
    ) -> Option<&mut OpRecord> {
        let record = self.map.get_mut(&operation)?;
        record.same_scope(bound).then_some(record)
    }
}

/// The verified identity a session bound through `synvelis.connect`.
///
/// `caller` is minted from the permit record through the
/// crate-internal seal — `clientInfo` and self-reported identity
/// never reach it. `task`/`grant` are the permit's opaque
/// references: they scope the session and flow into every wire
/// request, where GW-02 evaluates them per call.
pub struct BoundIdentity {
    /// The permit record this session verified against.
    pub record: PermitRecord,
    /// The attested caller injected into every wire request.
    pub caller: CallerIdentity,
}

/// A read view of one scoped operation-book entry.
#[derive(Clone, Debug)]
pub struct OpView {
    /// The tool that produced the accept.
    pub tool: String,
    /// The proposal the apply named (cancel path).
    pub proposal: Option<ProposalRef>,
    /// The tracked status.
    pub status: OpStatus,
}

/// Per-connection session state — created by [`McpHost::new_session`],
/// owned by the transport.
pub struct McpSession {
    channel: McpChannel,
    bound: Option<BoundIdentity>,
}

impl McpSession {
    /// Which channel this session arrived on.
    #[must_use]
    pub const fn channel(&self) -> McpChannel {
        self.channel
    }

    /// The bound identity, when `synvelis.connect` succeeded.
    #[must_use]
    pub fn bound(&self) -> Option<&BoundIdentity> {
        self.bound.as_ref()
    }
}

/// The transport-agnostic MCP adapter host.
pub struct McpHost {
    gateway: Mutex<Gateway>,
    permits: Mutex<Box<dyn PermitStore>>,
    committer: Arc<dyn CommitPort>,
    admin: Arc<dyn AdminPort>,
    clock: Arc<dyn PolicyClock>,
    ops: Mutex<OpBook>,
    adapter: AdapterId,
    seal: crate::request::AdapterSeal,
    bounds: McpBounds,
    enabled: bool,
    request_seq: AtomicU64,
}

#[allow(clippy::result_large_err)] // typed envelopes are the contract
impl McpHost {
    /// The adapter host over its seams. `config.enabled` defaults off —
    /// an MCP boundary that is never turned on cannot affect the DAW.
    #[must_use]
    pub fn new(
        gateway: Gateway,
        permits: Box<dyn PermitStore>,
        committer: Arc<dyn CommitPort>,
        admin: Arc<dyn AdminPort>,
        clock: Arc<dyn PolicyClock>,
        config: McpConfig,
    ) -> Self {
        Self {
            gateway: Mutex::new(gateway),
            permits: Mutex::new(permits),
            committer,
            admin,
            clock,
            ops: Mutex::new(OpBook {
                map: BTreeMap::new(),
                order: VecDeque::new(),
                cap: config.bounds.max_ops,
            }),
            adapter: config.adapter,
            seal: crate::request::AdapterSeal::issue(config.adapter),
            bounds: config.bounds,
            enabled: config.enabled,
            request_seq: AtomicU64::new(0),
        }
    }

    /// The adapter endpoint identity.
    #[must_use]
    pub const fn adapter(&self) -> AdapterId {
        self.adapter
    }

    /// The configured transfer bounds.
    #[must_use]
    pub const fn bounds(&self) -> &McpBounds {
        &self.bounds
    }

    /// A fresh session on `channel` — unbound until `synvelis.connect`.
    #[must_use]
    pub fn new_session(&self, channel: McpChannel) -> McpSession {
        McpSession {
            channel,
            bound: None,
        }
    }

    /// The policy clock.
    #[must_use]
    pub fn clock(&self) -> PolicyInstant {
        self.clock.now()
    }

    /// Mints a connection permit (control-channel host method and
    /// offline provisioning share this entry point).
    ///
    /// # Errors
    /// [`io::Error`] on durable-store failure;
    /// [`GatewayError::MalformedRequest`] on a channel/kind mismatch.
    ///
    /// # Panics
    /// Only if the permit-store lock is poisoned by a panic elsewhere.
    pub fn issue_permit(&self, spec: PermitSpec) -> Result<PermitIssue, ErrorEnvelope> {
        mcp_permit::check_channel_kind(&spec).map_err(ErrorEnvelope::from)?;
        self.permits
            .lock()
            .expect("permit store poisoned")
            .issue(spec, self.clock.now())
            .map_err(|e| {
                GatewayError::Internal {
                    reason: format!("permit store failure: {e}"),
                }
                .into()
            })
    }

    /// Records permit revocation.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — unknown id or durable-store failure.
    ///
    /// # Panics
    /// Only if the permit-store lock is poisoned by a panic elsewhere.
    pub fn revoke_permit(&self, permit: &str) -> Result<bool, ErrorEnvelope> {
        self.permits
            .lock()
            .expect("permit store poisoned")
            .revoke(permit)
            .map_err(|e| {
                GatewayError::Internal {
                    reason: format!("permit store failure: {e}"),
                }
                .into()
            })
    }

    /// A stored permit record by id (never carries a usable secret).
    ///
    /// # Panics
    /// Only if the permit-store lock is poisoned by a panic elsewhere.
    #[must_use]
    pub fn permit_record(&self, permit: &str) -> Option<PermitRecord> {
        self.permits
            .lock()
            .expect("permit store poisoned")
            .record(permit)
    }

    /// The next adapter-side request id.
    pub(crate) fn next_request_id(&self) -> String {
        format!("mcp-{}", self.request_seq.fetch_add(1, Ordering::Relaxed))
    }

    /// Runs one wire request through the typed boundary — the single
    /// path every tool takes into the gateway (parse → bind → invoke
    /// under the session's trusted caller).
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — the typed failure the gateway produced.
    pub(crate) fn call_gateway(
        &self,
        wire: &Value,
        caller: &CallerIdentity,
    ) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        self.gateway
            .lock()
            .expect("gateway poisoned")
            .handle(wire, caller.clone())
    }

    /// Records an accepted operation in the book under the session's
    /// ownership scope.
    pub(crate) fn record_op(
        &self,
        bound: &BoundIdentity,
        operation: OperationId,
        tool: &str,
        proposal: Option<ProposalRef>,
        status: OpStatus,
    ) {
        self.ops.lock().expect("op book poisoned").insert(
            operation,
            OpRecord {
                tool: tool.to_owned(),
                proposal,
                principal: bound.record.principal.clone(),
                project: bound.record.project,
                task: bound.record.task.clone(),
                grant: bound.record.grant.clone(),
                status,
            },
            bound,
        );
    }

    /// Updates the status of a scoped record (no-op for foreign/unknown).
    pub(crate) fn settle_op(
        &self,
        bound: &BoundIdentity,
        operation: OperationId,
        status: OpStatus,
    ) {
        let mut ops = self.ops.lock().expect("op book poisoned");
        if let Some(record) = ops.scoped_mut(operation, bound) {
            record.status = status;
        }
    }

    /// The scoped book entry (foreign and unknown share `None`).
    pub(crate) fn scoped_op(
        &self,
        bound: &BoundIdentity,
        operation: OperationId,
    ) -> Option<OpView> {
        self.ops
            .lock()
            .expect("op book poisoned")
            .scoped(operation, bound)
            .map(|r| OpView {
                tool: r.tool.clone(),
                proposal: r.proposal.clone(),
                status: r.status.clone(),
            })
    }

    /// The commit seam (tool layer reconciles through it).
    pub(crate) fn committer(&self) -> &Arc<dyn CommitPort> {
        &self.committer
    }

    /// The admin seam (control methods only).
    pub(crate) fn admin(&self) -> &Arc<dyn AdminPort> {
        &self.admin
    }

    /// One JSON-RPC message in → zero-or-one response out.
    ///
    /// Returns `None` for notifications (no `id`) — a transport never
    /// writes an answer to one. Every other message produces exactly
    /// one response value.
    pub fn handle(&self, session: &mut McpSession, msg: &Value) -> Option<Value> {
        let Some(obj) = msg.as_object() else {
            return Some(rpc_error(
                &Value::Null,
                INVALID_REQUEST,
                "request must be a JSON-RPC object",
                None,
            ));
        };
        let id = obj.get("id").cloned();
        let Some(method) = obj.get("method").and_then(Value::as_str) else {
            return Some(rpc_error(
                &id.unwrap_or(Value::Null),
                INVALID_REQUEST,
                "missing method",
                None,
            ));
        };
        // Notifications never produce a response — including the
        // protocol ones the adapter does not track (cancelled,
        // progress, roots changes).
        let id = id?;
        Some(self.dispatch(session, method, obj.get("params"), &id))
    }

    /// Routes a request method to its handler — every path returns
    /// one full JSON-RPC response value.
    fn dispatch(
        &self,
        session: &mut McpSession,
        method: &str,
        params: Option<&Value>,
        id: &Value,
    ) -> Value {
        match method {
            "initialize" => self.initialize(params, id),
            "ping" => rpc_result(id, json!({})),
            CONNECT_METHOD => self.connect(session, params, id),
            _ => self.dispatch_bound(session, method, params, id),
        }
    }

    /// `initialize` — version negotiation plus the server's protocol
    /// identity and the adapter contract revision under `_meta`. A
    /// client that requests a protocol version the adapter does not
    /// speak is refused outright — silent acceptance would bind a
    /// session to a contract it cannot honor.
    fn initialize(&self, params: Option<&Value>, id: &Value) -> Value {
        if let Some(requested) = params
            .and_then(|p| p.get("protocolVersion"))
            .and_then(Value::as_str)
            && requested != PROTOCOL_VERSION
        {
            return rpc_error(
                id,
                INVALID_PARAMS,
                "unsupported protocol version",
                Some(json!({"supported": PROTOCOL_VERSION, "requested": requested})),
            );
        }
        rpc_result(id, self.initialize_result())
    }

    /// The `initialize` result — the server's protocol identity plus
    /// the adapter contract revision under `_meta`.
    fn initialize_result(&self) -> Value {
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {"listChanged": false},
                "resources": {"subscribe": false, "listChanged": false},
                "prompts": {"listChanged": false},
                "logging": {},
            },
            "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
            "instructions": "Present a Synvelis connection permit via synvelis.connect before calling tools or reading resources. Tool discovery is not execution permission.",
            "_meta": {
                "synvelis": {
                    "adapter": self.adapter.to_tagged(),
                    "toolContract": TOOL_CONTRACT,
                    "enabled": self.enabled,
                },
            },
        })
    }

    /// `synvelis.connect` — the only credential-bearing method.
    ///
    /// The verdict is one blind `permit-invalid` for unknown,
    /// wrong-channel, expired, and revoked secrets — the response
    /// never discloses which permits exist. On success the session's
    /// trusted identity is minted from the *record*, not from
    /// anything the client claimed.
    fn connect(&self, session: &mut McpSession, params: Option<&Value>, id: &Value) -> Value {
        if !self.enabled {
            return rpc_error(
                id,
                ADAPTER_DISABLED,
                "the Synvelis MCP adapter is disabled",
                None,
            );
        }
        if session.bound.is_some() {
            return rpc_error(id, INVALID_REQUEST, "session already bound", None);
        }
        let Some(permit) = params.and_then(|p| p.get("permit")).and_then(Value::as_str) else {
            return rpc_error(
                id,
                INVALID_PARAMS,
                "synvelis.connect requires params.permit",
                None,
            );
        };
        let verified = self.permits.lock().expect("permit store poisoned").verify(
            permit,
            session.channel,
            self.clock.now(),
        );
        let Some(record) = verified else {
            return rpc_error(id, PERMIT_INVALID, "invalid connection permit", None);
        };
        let caller = mcp_permit::caller_for(&record, &self.seal);
        let result = json!({
            "status": "bound",
            "channel": record.channel,
            "principal": {"kind": record.principal.kind(), "id": record.principal.id()},
            "project": record.project.to_tagged(),
            "task": record.task.as_ref().map(TaskRef::as_str),
            "grant": record.grant.as_ref().map(GrantRef::as_str),
            "adapter": self.adapter.to_tagged(),
            "permit": record.permit,
        });
        session.bound = Some(BoundIdentity { record, caller });
        rpc_result(id, result)
    }

    /// Methods that require a bound session plus the control-channel
    /// host methods (existence-blind on the client channel).
    fn dispatch_bound(
        &self,
        session: &mut McpSession,
        method: &str,
        params: Option<&Value>,
        id: &Value,
    ) -> Value {
        // Control-channel host methods are checked first so an
        // unbound control session cannot even probe them — the same
        // blind `method not found` an unbound client sees.
        if method.starts_with("synvelis.") {
            return self.dispatch_control(session, method, params, id);
        }
        let Some(bound) = session.bound.as_ref() else {
            return rpc_error(
                id,
                PERMIT_REQUIRED,
                "a bound session requires synvelis.connect first",
                None,
            );
        };
        if !self.enabled {
            return rpc_error(
                id,
                ADAPTER_DISABLED,
                "the Synvelis MCP adapter is disabled",
                None,
            );
        }
        match method {
            "tools/list" => rpc_result(id, crate::mcp_tools::catalog(session.channel)),
            "tools/call" => self.call_tool(bound, session.channel, params, id),
            "resources/list" => rpc_result(id, Self::resources_list(bound)),
            "resources/templates/list" => rpc_result(id, Self::resource_templates()),
            "resources/read" => self.read_resource(bound, params, id),
            "prompts/list" => rpc_result(id, json!({"prompts": []})),
            "logging/setLevel" => rpc_result(id, json!({})),
            "completion/complete" => rpc_result(
                id,
                json!({"completion": {"values": [], "total": 0, "hasMore": false}}),
            ),
            _ => rpc_error(id, METHOD_NOT_FOUND, "method not found", None),
        }
    }

    /// `tools/call` — parameter shape check, then the tool layer.
    fn call_tool(
        &self,
        bound: &BoundIdentity,
        channel: McpChannel,
        params: Option<&Value>,
        id: &Value,
    ) -> Value {
        let Some(obj) = params.and_then(Value::as_object) else {
            return rpc_error(
                id,
                INVALID_PARAMS,
                "tools/call requires object params",
                None,
            );
        };
        let Some(name) = obj.get("name").and_then(Value::as_str) else {
            return rpc_error(id, INVALID_PARAMS, "tools/call requires params.name", None);
        };
        let args = obj.get("arguments").cloned().unwrap_or_else(|| json!({}));
        if !args.is_object() {
            return rpc_error(
                id,
                INVALID_PARAMS,
                "tools/call arguments must be an object",
                None,
            );
        }
        // Bounded transfer at the edge — an oversized argument blob
        // is rejected before any typed contract runs (DATA-08).
        if args.to_string().len() > self.bounds.max_tool_args_bytes {
            return rpc_error(
                id,
                INVALID_PARAMS,
                "tool arguments exceed the adapter bound",
                Some(json!({"max": self.bounds.max_tool_args_bytes})),
            );
        }
        let result = crate::mcp_tools::call(self, bound, channel, name, &args);
        rpc_result(id, result.to_value())
    }

    /// The one static resource template — the bound project's
    /// inspection projection. Real file paths never appear; the URI
    /// names the project by tagged id only.
    fn resources_list(bound: &BoundIdentity) -> Value {
        json!({
            "resources": [{
                "uri": format!("synvelis://project/{}/inspect", bound.record.project.to_tagged()),
                "name": "project inspection",
                "description": "Metadata projection of the bound project",
                "mimeType": "application/json",
            }],
        })
    }

    fn resource_templates() -> Value {
        json!({
            "resourceTemplates": [{
                "uriTemplate": "synvelis://project/{projectId}/inspect",
                "name": "project inspection",
                "description": "Metadata projection of a bound project",
                "mimeType": "application/json",
            }],
        })
    }

    /// `resources/read` — the inspection projection, scoped to the
    /// bound project only (foreign URIs share the blind verdict).
    fn read_resource(&self, bound: &BoundIdentity, params: Option<&Value>, id: &Value) -> Value {
        let Some(uri) = params.and_then(|p| p.get("uri")).and_then(Value::as_str) else {
            return rpc_error(
                id,
                INVALID_PARAMS,
                "resources/read requires params.uri",
                None,
            );
        };
        let expected = format!(
            "synvelis://project/{}/inspect",
            bound.record.project.to_tagged()
        );
        if uri != expected {
            return rpc_error(id, UNKNOWN_RESOURCE, "unknown resource", None);
        }
        let wire = crate::mcp_tools::context_wire(
            self,
            bound,
            &json!({"selector": {"project": true}, "fields": ["metadata"]}),
            &json!({"kind": "latest"}),
        );
        match self.call_gateway(&wire, &bound.caller) {
            Ok(GatewayOutcome::Completed { result }) => rpc_result(
                id,
                json!({
                    "contents": [{
                        "uri": expected,
                        "mimeType": "application/json",
                        "text": result.to_string(),
                    }],
                }),
            ),
            Ok(_) => rpc_error(id, INTERNAL_ERROR, "unexpected outcome", None),
            Err(env) => {
                let code = match env.code {
                    GatewayError::Denied { .. } => FORBIDDEN,
                    _ => INTERNAL_ERROR,
                };
                rpc_error(
                    id,
                    code,
                    &env.message,
                    Some(
                        serde_json::to_value(env.to_external()).expect("external error serializes"),
                    ),
                )
            }
        }
    }

    /// Control-channel host methods — a bound *operator* session is
    /// required, and on the client channel these names do not exist.
    fn dispatch_control(
        &self,
        session: &mut McpSession,
        method: &str,
        params: Option<&Value>,
        id: &Value,
    ) -> Value {
        const CONTROL_METHODS: &[&str] = &[
            "synvelis.issue-permit",
            "synvelis.revoke-permit",
            "synvelis.issue-task",
            "synvelis.revoke-task",
            "synvelis.issue-grant",
            "synvelis.revoke-grant",
            "synvelis.host-status",
        ];
        if session.channel != McpChannel::Control {
            return rpc_error(id, METHOD_NOT_FOUND, "method not found", None);
        }
        // Bound before the method-name check — an unbound control
        // session gets one uniform PERMIT_REQUIRED for every name,
        // never a vocabulary oracle.
        let Some(bound) = session.bound.as_ref() else {
            return rpc_error(
                id,
                PERMIT_REQUIRED,
                "a bound session requires synvelis.connect first",
                None,
            );
        };
        if !self.enabled {
            return rpc_error(
                id,
                ADAPTER_DISABLED,
                "the Synvelis MCP adapter is disabled",
                None,
            );
        }
        if !CONTROL_METHODS.contains(&method) {
            return rpc_error(id, METHOD_NOT_FOUND, "method not found", None);
        }
        match method {
            "synvelis.issue-permit" => self.op_issue_permit(bound, params, id),
            "synvelis.revoke-permit" => self.op_revoke_permit(params, id),
            "synvelis.issue-task" => self.op_issue_task(bound, params, id),
            "synvelis.revoke-task" => self.op_revoke_task(params, id),
            "synvelis.issue-grant" => self.op_issue_grant(bound, params, id),
            "synvelis.revoke-grant" => self.op_revoke_grant(params, id),
            "synvelis.host-status" => rpc_result(
                id,
                json!({
                    "adapter": self.adapter.to_tagged(),
                    "enabled": self.enabled,
                    "live_revision": self
                        .admin()
                        .live_revision(bound.record.project)
                        .map(|r| r.0),
                }),
            ),
            _ => rpc_error(id, METHOD_NOT_FOUND, "method not found", None),
        }
    }

    fn op_issue_permit(&self, bound: &BoundIdentity, params: Option<&Value>, id: &Value) -> Value {
        match crate::mcp_tools::permit_spec_from_params(params, bound) {
            Ok(spec) => match self.issue_permit(spec) {
                Ok(issue) => rpc_result(
                    id,
                    json!({
                        "permit": issue.record.permit,
                        "secret": issue.secret,
                        "channel": issue.record.channel,
                        "principal": issue.record.principal.id(),
                        "expires_at": issue.record.expires_at.map(|e| e.0),
                    }),
                ),
                Err(env) => rpc_error(
                    id,
                    INVALID_PARAMS,
                    &env.message,
                    Some(serde_json::to_value(env.to_external()).expect("serializes")),
                ),
            },
            Err(fault) => rpc_error(id, INVALID_PARAMS, &fault.message(), fault.error_value()),
        }
    }

    fn op_revoke_permit(&self, params: Option<&Value>, id: &Value) -> Value {
        let Some(permit) = params.and_then(|p| p.get("permit")).and_then(Value::as_str) else {
            return rpc_error(id, INVALID_PARAMS, "params.permit is required", None);
        };
        match self.revoke_permit(permit) {
            Ok(true) => rpc_result(id, json!({"permit": permit, "revoked": true})),
            // Unknown ids are still a successful-looking no-op? No —
            // the operator asked for a specific record; unknown is a
            // reportable result, not a secret to keep.
            Ok(false) => rpc_error(id, INVALID_PARAMS, "unknown permit", None),
            Err(env) => rpc_error(id, INTERNAL_ERROR, &env.message, None),
        }
    }

    fn op_issue_task(&self, bound: &BoundIdentity, params: Option<&Value>, id: &Value) -> Value {
        match crate::mcp_tools::task_from_params(params, bound) {
            Ok(task) => match self.admin().issue_task(task) {
                Ok(()) => rpc_result(id, json!({"issued": true})),
                Err(env) => rpc_error(id, INTERNAL_ERROR, &env.message, None),
            },
            Err(fault) => rpc_error(id, INVALID_PARAMS, &fault.message(), fault.error_value()),
        }
    }

    fn op_revoke_task(&self, params: Option<&Value>, id: &Value) -> Value {
        let Some(task) = params.and_then(|p| p.get("task")).and_then(Value::as_str) else {
            return rpc_error(id, INVALID_PARAMS, "params.task is required", None);
        };
        let Ok(task_ref) = TaskRef::new(task) else {
            return rpc_error(id, INVALID_PARAMS, "malformed task ref", None);
        };
        match self.admin().revoke_task(&task_ref) {
            Ok(()) => rpc_result(id, json!({"task": task, "revoked": true})),
            Err(env) => rpc_error(id, INTERNAL_ERROR, &env.message, None),
        }
    }

    fn op_issue_grant(&self, bound: &BoundIdentity, params: Option<&Value>, id: &Value) -> Value {
        match crate::mcp_tools::grant_from_params(params, bound) {
            Ok(grant) => match self.admin().issue_grant(grant) {
                Ok(()) => rpc_result(id, json!({"issued": true})),
                Err(env) => rpc_error(id, INTERNAL_ERROR, &env.message, None),
            },
            Err(fault) => rpc_error(id, INVALID_PARAMS, &fault.message(), fault.error_value()),
        }
    }

    fn op_revoke_grant(&self, params: Option<&Value>, id: &Value) -> Value {
        let Some(grant) = params.and_then(|p| p.get("grant")).and_then(Value::as_str) else {
            return rpc_error(id, INVALID_PARAMS, "params.grant is required", None);
        };
        let Ok(grant_ref) = GrantRef::new(grant) else {
            return rpc_error(id, INVALID_PARAMS, "malformed grant ref", None);
        };
        match self.admin().revoke_grant(&grant_ref) {
            Ok(()) => rpc_result(id, json!({"grant": grant, "revoked": true})),
            Err(env) => rpc_error(id, INTERNAL_ERROR, &env.message, None),
        }
    }
}

/// Translates a gateway outcome into the tool-layer result — the
/// single place `GatewayOutcome` and `ErrorEnvelope` become MCP
/// vocabulary (`ErrorEnvelope::to_external` strips diagnostics).
pub(crate) fn translate_outcome(
    outcome: Result<GatewayOutcome<Value>, ErrorEnvelope>,
) -> CallToolResult {
    match outcome {
        Ok(GatewayOutcome::Completed { result }) => CallToolResult::ok(result),
        Ok(GatewayOutcome::ProposalReady { proposal }) => {
            CallToolResult::ok(json!({"status": "proposal-ready", "proposal": proposal}))
        }
        Ok(GatewayOutcome::ApprovalRequired { approval }) => {
            CallToolResult::ok(json!({"status": "approval-required", "approval": approval}))
        }
        Ok(GatewayOutcome::Accepted { operation }) => {
            CallToolResult::ok(json!({"status": "accepted", "operation": operation}))
        }
        Err(env) => CallToolResult::from(&env),
    }
}
