//! GW-08 MCP tools — the adapter's semantic surface (GIA-004).
//!
//! Tool names are the adapter's own contract — `project.inspect`,
//! `midi.notes.query`, `proposal.apply` — deliberately *not* a 1:1
//! mirror of internal capability ids (`synvelis.context.query`,
//! `synvelis.operation.prepare`, …). Each tool validates its own
//! argument shape, builds one internal [`WireRequest`], and hands it
//! to [`McpHost::call_gateway`], so every invocation crosses the same
//! parse → bind → invoke pipeline and the same per-call policy
//! evaluation as any other adapter.
//!
//! Catalog rules:
//!
//! - `proposal.approve` exists only on the control channel — a client
//!   session cannot even discover it (discovery is not permission,
//!   and the client surface keeps the trusted-user act unoffered).
//! - Everything else is offered on both channels; authority is
//!   decided per call by the bound identity + grants, never by the
//!   tool list.
//!
//! Bounded-transfer enforcement (range length, page size, argument
//! bytes) happens here *before* the wire is built — the adapter
//! bound is the outer guard under which the engine budgets apply.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use synvelis_project::ProjectRevision;

use crate::capability::{CapabilityId, CapabilityRef, CapabilityVersion};
use crate::commit::{CommitOutcome, RetryOutcome};
use crate::grant::{Grant, GrantBinding, GrantLimits, ResourceScope, Validity};
use crate::id::{GrantRef, OperationId, ProposalRef, TaskRef};
use crate::mcp::{AdapterFault, CallToolResult, McpChannel};
use crate::mcp_permit::PermitSpec;
use crate::mcp_server::{BoundIdentity, McpHost, OpStatus};
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::proposal_engine::{APPLY_ID, APPROVE_ID, PREVIEW_ID};
use crate::request::{Principal, PrincipalKind, ResourceKind, ResourceRef};
use crate::store::{Activation, AuthorizedTask};

/// The inner capabilities `midi.edit.prepare` will lower — the MIDI
/// note/source ops targeting a source resource. `midi.source.create`
/// is excluded (its `Create` target shape needs a different tool
/// contract); the clip-level ops live behind a future clip tool.
/// The allowlist is a *surface* decision, not the authorization —
/// GW-02 decides per call whether the bound grant covers the op.
const MIDI_EDIT_OPS: &[&str] = &[
    "synvelis.midi.note.add",
    "synvelis.midi.note.remove",
    "synvelis.midi.note.set-velocity",
    "synvelis.midi.note.move",
    "synvelis.midi.source.remove",
];

/// Field groups a client tool may never request — bytes, derived
/// data, and workspace selection are not MCP-disclosable surface in
/// GW-08 (they need their own authorized paths before any exposure).
const FORBIDDEN_FIELD_GROUPS: &[&str] = &["media", "derived", "selection"];

/// Field groups `project.inspect` accepts — metadata is the
/// structure view the resource template also serves.
const INSPECT_FIELD_GROUPS: &[&str] = &["metadata"];

type ToolResult = Result<CallToolResult, CallToolResult>;

fn err(code: GatewayError) -> CallToolResult {
    CallToolResult::from(&ErrorEnvelope::from(code))
}

fn fault(f: &AdapterFault) -> CallToolResult {
    CallToolResult::err(f.to_value())
}

fn args_fault(reason: impl Into<String>) -> CallToolResult {
    fault(&AdapterFault::InvalidArguments {
        reason: reason.into(),
    })
}

/// Rejects unknown argument keys and missing required ones — the
/// same deny-by-default discipline the wire layer applies.
fn check_keys(args: &Value, allowed: &[&str], required: &[&str]) -> Result<(), CallToolResult> {
    let Some(obj) = args.as_object() else {
        return Err(args_fault("expected an object"));
    };
    for key in obj.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(args_fault(format!("unknown field {key}")));
        }
    }
    for req in required {
        if !obj.contains_key(*req) {
            return Err(args_fault(format!("missing required field {req}")));
        }
    }
    Ok(())
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, CallToolResult> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| args_fault(format!("{key} must be a string")))
}

fn u64_arg(args: &Value, key: &str) -> Result<u64, CallToolResult> {
    args.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| args_fault(format!("{key} must be an integer")))
}

/// The `tools/list` catalog for `channel` — the adapter's semantic
/// contract, with JSON-Schema argument shapes for client-side
/// validation. `proposal.approve` is listed on the control channel
/// only.
pub(crate) fn catalog(channel: McpChannel) -> Value {
    let mut tools = vec![
        schema_project_inspect(),
        schema_notes_query(),
        schema_edit_prepare(),
        schema_proposal_preview(),
        schema_proposal_apply(),
        schema_operation_get(),
        schema_operation_cancel(),
    ];
    if channel == McpChannel::Control {
        tools.push(schema_proposal_approve());
    }
    json!({"tools": tools})
}

fn schema_project_inspect() -> Value {
    tool_entry(
        "project.inspect",
        "Metadata inspection of the bound project (structure, names, placements).",
        &json!({
            "type": "object",
            "properties": {
                "fields": {"type": "array", "items": {"enum": ["metadata"]}},
                "at_revision": {"type": "integer", "minimum": 0},
                "page": {"type": "object", "properties": {"limit": {"type": "integer", "minimum": 1}}, "additionalProperties": false},
            },
            "additionalProperties": false,
        }),
    )
}

fn schema_notes_query() -> Value {
    tool_entry(
        "midi.notes.query",
        "Note content of one MIDI source over an explicit source-native range, paged.",
        &json!({
            "type": "object",
            "properties": {
                "source": {"type": "string"},
                "range": {
                    "type": "object",
                    "properties": {"start": {"type": "integer"}, "len": {"type": "integer", "minimum": 1}},
                    "required": ["start", "len"],
                    "additionalProperties": false,
                },
                "page": {
                    "type": "object",
                    "properties": {
                        "limit": {"type": "integer", "minimum": 1},
                        "cursor": {"type": "string"},
                        "snapshot": {"type": "string"},
                    },
                    "additionalProperties": false,
                },
                "at_revision": {"type": "integer", "minimum": 0},
            },
            "additionalProperties": false,
        }),
    )
}

fn schema_edit_prepare() -> Value {
    tool_entry(
        "midi.edit.prepare",
        "Prepares one MIDI edit as a reviewable proposal — no mutation until an apply is accepted.",
        &json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "object",
                    "properties": {
                        "capability": {"type": "string", "enum": MIDI_EDIT_OPS},
                        "target": {
                            "type": "object",
                            "properties": {"kind": {"const": "source"}, "id": {"type": "string"}},
                            "required": ["kind", "id"],
                            "additionalProperties": false,
                        },
                        "args": {"type": "object"},
                    },
                    "required": ["capability", "target"],
                    "additionalProperties": false,
                },
                "base_revision": {"type": "integer", "minimum": 0},
            },
            "required": ["operation", "base_revision"],
            "additionalProperties": false,
        }),
    )
}

fn schema_proposal_preview() -> Value {
    tool_entry(
        "proposal.preview",
        "The semantic diff of a prepared proposal — the review document an approval binds to.",
        &json!({
            "type": "object",
            "properties": {
                "proposal": {"type": "string"},
                "digest": {"type": "string"},
            },
            "required": ["proposal"],
            "additionalProperties": false,
        }),
    )
}

fn schema_proposal_apply() -> Value {
    tool_entry(
        "proposal.apply",
        "Requests the apply of a prepared proposal. Suggest-mode grants deny here; approve-to-apply needs a receipt; delegated applies commit within the grant's bounds.",
        &json!({
            "type": "object",
            "properties": {
                "proposal": {"type": "string"},
                "digest": {"type": "string"},
                "expected_revision": {"type": "integer", "minimum": 0},
                "approval": {"type": "string"},
                "idempotency_key": {"type": "string"},
            },
            "required": ["proposal", "digest", "expected_revision"],
            "additionalProperties": false,
        }),
    )
}

fn schema_operation_get() -> Value {
    tool_entry(
        "operation.get",
        "The settled verdict of an accepted operation — the reconciliation answer after a retry or reconnect.",
        &json!({
            "type": "object",
            "properties": {"operation": {"type": "string"}},
            "required": ["operation"],
            "additionalProperties": false,
        }),
    )
}

fn schema_operation_cancel() -> Value {
    tool_entry(
        "operation.cancel",
        "Settles a still-pending apply as rejected — the caller-side cancel for an accepted operation.",
        &json!({
            "type": "object",
            "properties": {"operation": {"type": "string"}},
            "required": ["operation"],
            "additionalProperties": false,
        }),
    )
}

fn schema_proposal_approve() -> Value {
    tool_entry(
        "proposal.approve",
        "Issues a Synvelis approval receipt bound to a proposal's digest and base revision (operator channel only).",
        &json!({
            "type": "object",
            "properties": {
                "proposal": {"type": "string"},
                "digest": {"type": "string"},
                "expected_revision": {"type": "integer", "minimum": 0},
            },
            "required": ["proposal", "digest", "expected_revision"],
            "additionalProperties": false,
        }),
    )
}

fn tool_entry(name: &str, description: &str, input_schema: &Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

/// `tools/call` dispatch — `name` selects the tool; every tool is a
/// pure function of (host, bound identity, args).
pub(crate) fn call(
    host: &McpHost,
    bound: &BoundIdentity,
    channel: McpChannel,
    name: &str,
    args: &Value,
) -> CallToolResult {
    match name {
        "project.inspect" => run(tool_project_inspect(host, bound, args)),
        "midi.notes.query" => run(tool_notes_query(host, bound, args)),
        "midi.edit.prepare" => run(tool_edit_prepare(host, bound, args)),
        "proposal.preview" => run(tool_proposal_preview(host, bound, args)),
        "proposal.apply" => run(tool_proposal_apply(host, bound, args)),
        "operation.get" => run(tool_operation_get(host, bound, args)),
        "operation.cancel" => run(tool_operation_cancel(host, bound, args)),
        "proposal.approve" if channel == McpChannel::Control => {
            run(tool_proposal_approve(host, bound, args))
        }
        _ => fault(&AdapterFault::Unavailable {
            reason: format!("unknown tool: {name}"),
        }),
    }
}

fn run(r: ToolResult) -> CallToolResult {
    r.unwrap_or_else(|e| e)
}

/// Builds one wire request for the bound session — the only wire
/// producer in the adapter. `task`/`grant` flow from the permit
/// record on the client channel; a control session never carries
/// them (approval is human authority, not a grant reference).
pub(crate) fn context_wire(
    host: &McpHost,
    bound: &BoundIdentity,
    args: &Value,
    consistency: &Value,
) -> Value {
    wire_for(
        host,
        bound,
        crate::context_engine::CONTEXT_QUERY_ID,
        args,
        consistency,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn wire_for(
    host: &McpHost,
    bound: &BoundIdentity,
    capability: &str,
    args: &Value,
    consistency: &Value,
    idempotency_key: Option<&str>,
) -> Value {
    let mut wire = json!({
        "request_id": host.next_request_id(),
        "capability": {"id": capability, "version": {"major": 1, "minor": 0}},
        "schema": {"name": format!("{capability}.input"), "version": {"major": 1, "minor": 0}},
        "project": bound.record.project.to_tagged(),
        "args": args,
        "consistency": consistency,
    });
    if bound.record.channel == McpChannel::Client {
        if let Some(task) = &bound.record.task {
            wire["task"] = json!(task.as_str());
        }
        if let Some(grant) = &bound.record.grant {
            wire["grant"] = json!(grant.as_str());
        }
    }
    if let Some(key) = idempotency_key {
        wire["idempotency_key"] = json!(key);
    }
    wire
}

fn consistency_for(args: &Value, mutation: bool) -> Result<Value, CallToolResult> {
    if mutation {
        let revision = u64_arg(args, "expected_revision")?;
        return Ok(json!({"kind": "expected-revision", "revision": revision}));
    }
    match args.get("at_revision") {
        Some(v) => {
            let revision = v
                .as_u64()
                .ok_or_else(|| args_fault("at_revision must be an integer"))?;
            Ok(json!({"kind": "read-at", "revision": revision}))
        }
        None => Ok(json!({"kind": "latest"})),
    }
}

// ── project.inspect ──────────────────────────────────────────

fn tool_project_inspect(host: &McpHost, bound: &BoundIdentity, args: &Value) -> ToolResult {
    check_keys(args, &["fields", "at_revision", "page"], &[])?;
    let mut groups: BTreeSet<String> = BTreeSet::from(["metadata".to_owned()]);
    if let Some(fields) = args.get("fields") {
        let Some(arr) = fields.as_array() else {
            return Err(args_fault("fields must be an array"));
        };
        for (i, v) in arr.iter().enumerate() {
            let name = v
                .as_str()
                .ok_or_else(|| args_fault(format!("fields[{i}] must be a string")))?;
            if FORBIDDEN_FIELD_GROUPS.contains(&name) || !INSPECT_FIELD_GROUPS.contains(&name) {
                return Err(args_fault(format!(
                    "field group is not MCP-disclosable here: {name}"
                )));
            }
            groups.insert(name.to_owned());
        }
    }
    let page = page_args(host, args, /*allow_continuation*/ false)?;
    let mut query = json!({
        "selector": {"project": true},
        "fields": groups,
    });
    if let Some(page) = page {
        query["page"] = page;
    }
    let consistency = consistency_for(args, false)?;
    let wire = context_wire(host, bound, &query, &consistency);
    Ok(crate::mcp_server::translate_outcome(
        host.call_gateway(&wire, &bound.caller),
    ))
}

/// Page-argument handling shared by the query tools. A continuation
/// is `{cursor, snapshot}` and forbids every other request field —
/// the engine pins the query inside the cursor record.
fn page_args(
    host: &McpHost,
    args: &Value,
    allow_continuation: bool,
) -> Result<Option<Value>, CallToolResult> {
    let Some(page) = args.get("page") else {
        return Ok(None);
    };
    let Some(obj) = page.as_object() else {
        return Err(args_fault("page must be an object"));
    };
    for key in obj.keys() {
        if !matches!(key.as_str(), "limit" | "cursor" | "snapshot") {
            return Err(args_fault(format!("unknown field page.{key}")));
        }
    }
    if let Some(limit) = obj.get("limit") {
        let limit = limit
            .as_u64()
            .ok_or_else(|| args_fault("page.limit must be an integer"))?;
        if limit > u64::from(host.bounds().max_page_limit) {
            return Err(fault(&AdapterFault::LimitExceeded {
                limit: "page limit".to_owned(),
                max: u64::from(host.bounds().max_page_limit),
            }));
        }
    }
    if obj.contains_key("cursor") {
        if !allow_continuation {
            return Err(args_fault("this tool does not page through cursors"));
        }
        for key in ["cursor", "snapshot"] {
            if !obj.contains_key(key) {
                return Err(args_fault(format!("a continuation requires page.{key}")));
            }
        }
        return Ok(Some(json!({"cursor": obj["cursor"].clone()})));
    }
    let mut out = json!({});
    if let Some(limit) = obj.get("limit") {
        out["limit"] = limit.clone();
    }
    Ok(Some(out))
}

// ── midi.notes.query ─────────────────────────────────────────

fn tool_notes_query(host: &McpHost, bound: &BoundIdentity, args: &Value) -> ToolResult {
    check_keys(args, &["source", "range", "page", "at_revision"], &[])?;
    // A continuation carries only the cursor pair — the query was
    // pinned inside the record (DATA-03: never re-base to latest).
    let is_continuation = args.get("page").and_then(|p| p.get("cursor")).is_some();
    if is_continuation {
        if args.get("source").is_some()
            || args.get("range").is_some()
            || args.get("at_revision").is_some()
        {
            return Err(args_fault(
                "a cursor continuation carries no query parameters",
            ));
        }
        let page = page_args(host, args, true)?.expect("cursor implies page");
        let snapshot = args
            .get("page")
            .and_then(|p| p.get("snapshot"))
            .and_then(Value::as_str)
            .ok_or_else(|| args_fault("a continuation requires page.snapshot"))?;
        let wire = wire_for(
            host,
            bound,
            crate::context_engine::CONTEXT_QUERY_ID,
            &json!({"page": page}),
            &json!({"kind": "fixed-snapshot", "snapshot": snapshot}),
            None,
        );
        return Ok(crate::mcp_server::translate_outcome(
            host.call_gateway(&wire, &bound.caller),
        ));
    }
    check_keys(
        args,
        &["source", "range", "page", "at_revision"],
        &["source", "range"],
    )?;
    let source = str_arg(args, "source")?;
    // Typed early — the kind tag must be `source`, the id canonical.
    ResourceRef::expect(ResourceKind::Source, source).map_err(err)?;
    let range = args.get("range").expect("required");
    let Some(obj) = range.as_object() else {
        return Err(args_fault("range must be an object"));
    };
    for key in obj.keys() {
        if !matches!(key.as_str(), "start" | "len") {
            return Err(args_fault(format!("unknown field range.{key}")));
        }
    }
    let start = range
        .get("start")
        .and_then(Value::as_i64)
        .ok_or_else(|| args_fault("range.start must be an integer"))?;
    let len = range
        .get("len")
        .and_then(Value::as_i64)
        .ok_or_else(|| args_fault("range.len must be an integer"))?;
    if len <= 0 {
        return Err(args_fault("range.len must be positive"));
    }
    if len > host.bounds().max_range_len {
        return Err(fault(&AdapterFault::LimitExceeded {
            limit: "range length".to_owned(),
            max: u64::try_from(host.bounds().max_range_len).unwrap_or(0),
        }));
    }
    let page = page_args(host, args, true)?;
    let mut query = json!({
        "selector": {"resources": [source]},
        "fields": ["content"],
        "range": {"domain": "source-native", "start": start, "len": len},
    });
    if let Some(page) = page {
        query["page"] = page;
    }
    let consistency = consistency_for(args, false)?;
    let wire = context_wire(host, bound, &query, &consistency);
    Ok(crate::mcp_server::translate_outcome(
        host.call_gateway(&wire, &bound.caller),
    ))
}

// ── midi.edit.prepare ────────────────────────────────────────

fn tool_edit_prepare(host: &McpHost, bound: &BoundIdentity, args: &Value) -> ToolResult {
    check_keys(
        args,
        &["operation", "base_revision"],
        &["operation", "base_revision"],
    )?;
    let op = args.get("operation").expect("required");
    check_keys(
        op,
        &["capability", "target", "args"],
        &["capability", "target"],
    )?;
    let capability = str_arg(op, "capability")?;
    if !MIDI_EDIT_OPS.contains(&capability) {
        return Err(args_fault(format!(
            "capability not offered by midi.edit.prepare: {capability}"
        )));
    }
    let target = op.get("target").expect("required");
    check_keys(target, &["kind", "id"], &["kind", "id"])?;
    let kind = str_arg(target, "kind")?;
    if kind != "source" {
        return Err(args_fault("midi.edit.prepare targets must be kind source"));
    }
    let target_id = str_arg(target, "id")?;
    ResourceRef::expect(ResourceKind::Source, target_id).map_err(err)?;
    let op_args = match op.get("args") {
        Some(v) if v.is_object() => v.clone(),
        Some(_) => return Err(args_fault("operation.args must be an object")),
        None => json!({}),
    };
    let base = u64_arg(args, "base_revision")?;
    let wire = wire_for(
        host,
        bound,
        "synvelis.operation.prepare",
        &json!({
            "operation": {
                "capability": {"id": capability, "version": {"major": 1, "minor": 0}},
                "target": {"shape": "resource", "kind": "source", "id": target_id},
                "args": op_args,
            },
        }),
        &json!({"kind": "expected-revision", "revision": base}),
        None,
    );
    Ok(crate::mcp_server::translate_outcome(
        host.call_gateway(&wire, &bound.caller),
    ))
}

// ── proposal.preview / proposal.apply / proposal.approve ─────

fn tool_proposal_preview(host: &McpHost, bound: &BoundIdentity, args: &Value) -> ToolResult {
    check_keys(args, &["proposal", "digest"], &["proposal"])?;
    let proposal = str_arg(args, "proposal")?;
    let mut wire_args = json!({"proposal": proposal});
    if let Some(digest) = args.get("digest") {
        wire_args["digest"] = json!(
            digest
                .as_str()
                .ok_or_else(|| args_fault("digest must be a string"))?
        );
    }
    let wire = wire_for(
        host,
        bound,
        PREVIEW_ID,
        &wire_args,
        &json!({"kind": "latest"}),
        None,
    );
    Ok(crate::mcp_server::translate_outcome(
        host.call_gateway(&wire, &bound.caller),
    ))
}

fn tool_proposal_apply(host: &McpHost, bound: &BoundIdentity, args: &Value) -> ToolResult {
    check_keys(
        args,
        &[
            "proposal",
            "digest",
            "expected_revision",
            "approval",
            "idempotency_key",
        ],
        &["proposal", "digest", "expected_revision"],
    )?;
    let proposal = str_arg(args, "proposal")?;
    let digest = str_arg(args, "digest")?;
    let mut wire_args = json!({"proposal": proposal, "digest": digest});
    if let Some(approval) = args.get("approval") {
        wire_args["approval"] = json!(
            approval
                .as_str()
                .ok_or_else(|| args_fault("approval must be a string"))?
        );
    }
    let idem = match args.get("idempotency_key") {
        Some(v) => Some(
            v.as_str()
                .ok_or_else(|| args_fault("idempotency_key must be a string"))?,
        ),
        None => None,
    };
    let consistency = consistency_for(args, true)?;
    let wire = wire_for(host, bound, APPLY_ID, &wire_args, &consistency, idem);
    let outcome = host.call_gateway(&wire, &bound.caller);
    Ok(finish_apply(host, bound, proposal, outcome))
}

/// The apply tail — `Accepted` is never reported as a commit: the
/// book records it, the commit port drives it, and the durable
/// reconcile answers the real verdict (ACG-004 honesty: `committed`
/// only when the boundary recorded one).
fn finish_apply(
    host: &McpHost,
    bound: &BoundIdentity,
    proposal: &str,
    outcome: Result<GatewayOutcome<Value>, ErrorEnvelope>,
) -> CallToolResult {
    match outcome {
        Ok(GatewayOutcome::Accepted { operation }) => {
            let proposal_ref = ProposalRef::new(proposal).ok();
            host.record_op(
                bound,
                operation,
                "proposal.apply",
                proposal_ref.clone(),
                OpStatus::CommitRequested,
            );
            let Some(pref) = proposal_ref else {
                return CallToolResult::ok(json!({
                    "status": "commit-requested",
                    "operation": operation,
                }));
            };
            match host.committer().commit(&pref) {
                Ok(receipt) => {
                    let status = OpStatus::Committed {
                        revision: receipt.revision.0,
                        commit: receipt.commit.to_tagged(),
                        journal_seq: receipt.journal_seq,
                    };
                    host.settle_op(bound, operation, status.clone());
                    CallToolResult::ok(json!({
                        "status": "committed",
                        "operation": operation,
                        "revision": receipt.revision.0,
                        "commit": receipt.commit.to_tagged(),
                        "journal_seq": receipt.journal_seq,
                    }))
                }
                Err(_env) => settled_verdict(host, bound, operation),
            }
        }
        other => crate::mcp_server::translate_outcome(other),
    }
}

/// Reads the durable verdict after a commit attempt — the gate
/// records determined non-commit outcomes too, so reconcile is the
/// ground truth and the apply tool reports what it *proved*, never a
/// guess.
fn settled_verdict(
    host: &McpHost,
    bound: &BoundIdentity,
    operation: OperationId,
) -> CallToolResult {
    match host.committer().reconcile(operation) {
        RetryOutcome::Settled(outcome) => {
            let status = OpStatus::of_outcome(&outcome);
            host.settle_op(bound, operation, status.clone());
            CallToolResult::ok(json!({
                "status": status.name(),
                "operation": operation,
                "verdict": verdict_json(&status),
            }))
        }
        RetryOutcome::InFlight => CallToolResult::ok(json!({
            "status": "in-flight",
            "operation": operation,
        })),
        RetryOutcome::Unknown => {
            // No commit boundary wired — the apply is genuinely
            // accepted but undriven; report the honest state rather
            // than the port's internal error.
            CallToolResult::ok(json!({
                "status": "commit-requested",
                "operation": operation,
            }))
        }
    }
}

fn verdict_json(status: &OpStatus) -> Value {
    match status {
        OpStatus::Committed {
            revision,
            commit,
            journal_seq,
        } => json!({"revision": revision, "commit": commit, "journal_seq": journal_seq}),
        OpStatus::Rejected { reason } | OpStatus::Indeterminate { reason } => {
            json!({"reason": reason})
        }
        OpStatus::Conflicted { expected, actual } => {
            json!({"expected": expected, "actual": actual})
        }
        OpStatus::Cancelled { reason } => json!({"reason": reason}),
        OpStatus::CommitRequested => json!({}),
    }
}

fn tool_proposal_approve(host: &McpHost, bound: &BoundIdentity, args: &Value) -> ToolResult {
    check_keys(
        args,
        &["proposal", "digest", "expected_revision"],
        &["proposal", "digest", "expected_revision"],
    )?;
    let proposal = str_arg(args, "proposal")?;
    let digest = str_arg(args, "digest")?;
    let consistency = consistency_for(args, true)?;
    let wire = wire_for(
        host,
        bound,
        APPROVE_ID,
        &json!({"proposal": proposal, "digest": digest}),
        &consistency,
        None,
    );
    Ok(crate::mcp_server::translate_outcome(
        host.call_gateway(&wire, &bound.caller),
    ))
}

// ── operation.get / operation.cancel ─────────────────────────

fn parse_operation(text: &str) -> Result<OperationId, CallToolResult> {
    serde_json::from_value::<OperationId>(json!(text))
        .map_err(|_| args_fault("operation must be a tagged operation id"))
}

fn tool_operation_get(host: &McpHost, bound: &BoundIdentity, args: &Value) -> ToolResult {
    check_keys(args, &["operation"], &["operation"])?;
    let operation = parse_operation(str_arg(args, "operation")?)?;
    let book = host.scoped_op(bound, operation);
    match host.committer().reconcile(operation) {
        RetryOutcome::Settled(outcome) => {
            // Ownership: a committed receipt carries its principal —
            // that survives an adapter restart (the book is
            // volatile). Non-commit verdicts attribute only through
            // the scoped book entry; a foreign or unattributable
            // handle reports unknown, same as the cursor store.
            let owned = match &outcome {
                CommitOutcome::Committed(receipt) => {
                    receipt.operation == Some(operation)
                        && receipt.principal == bound.record.principal
                        && receipt.project == bound.record.project
                }
                _ => book.is_some(),
            };
            if !owned {
                return Err(fault(&AdapterFault::UnknownOperation));
            }
            let status = OpStatus::of_outcome(&outcome);
            host.settle_op(bound, operation, status.clone());
            Ok(CallToolResult::ok(json!({
                "operation": operation,
                "tool": book.map(|v| v.tool),
                "status": status.name(),
                "verdict": verdict_json(&status),
            })))
        }
        RetryOutcome::InFlight => {
            if book.is_none() {
                return Err(fault(&AdapterFault::UnknownOperation));
            }
            Ok(CallToolResult::ok(json!({
                "operation": operation,
                "status": "in-flight",
            })))
        }
        RetryOutcome::Unknown => match book {
            Some(view) => Ok(CallToolResult::ok(json!({
                "operation": operation,
                "tool": view.tool,
                "status": view.status.name(),
                "verdict": verdict_json(&view.status),
            }))),
            None => Err(fault(&AdapterFault::UnknownOperation)),
        },
    }
}

fn tool_operation_cancel(host: &McpHost, bound: &BoundIdentity, args: &Value) -> ToolResult {
    check_keys(args, &["operation"], &["operation"])?;
    let operation = parse_operation(str_arg(args, "operation")?)?;
    let Some(view) = host.scoped_op(bound, operation) else {
        return Err(fault(&AdapterFault::UnknownOperation));
    };
    if view.status.terminal() {
        return Err(fault(&AdapterFault::NotCancellable {
            status: view.status.name().to_owned(),
        }));
    }
    let Some(proposal) = view.proposal else {
        return Err(fault(&AdapterFault::Unavailable {
            reason: "this operation has no cancellable proposal".to_owned(),
        }));
    };
    let cancelled = host
        .committer()
        .cancel(&proposal, "cancelled through operation.cancel")
        .map_err(|env| CallToolResult::from(&env))?;
    if !cancelled {
        // The commit boundary found the apply already settled — the
        // honest answer is its real status, not a false `cancelled`.
        // Refresh the book from durable truth first.
        if let RetryOutcome::Settled(outcome) = host.committer().reconcile(operation) {
            let status = OpStatus::of_outcome(&outcome);
            host.settle_op(bound, operation, status.clone());
            return Err(fault(&AdapterFault::NotCancellable {
                status: status.name().to_owned(),
            }));
        }
        return Err(fault(&AdapterFault::NotCancellable {
            status: view.status.name().to_owned(),
        }));
    }
    host.settle_op(
        bound,
        operation,
        OpStatus::Cancelled {
            reason: "cancelled through operation.cancel".to_owned(),
        },
    );
    Ok(CallToolResult::ok(json!({
        "operation": operation,
        "status": "cancelled",
    })))
}

// ── Control-channel parameter shapes ─────────────────────────

fn params_obj(params: Option<&Value>) -> Result<&Value, CallToolResult> {
    params
        .filter(|p| p.is_object())
        .ok_or_else(|| args_fault("expected object params"))
}

fn param_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, CallToolResult> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| args_fault(format!("params.{key} must be a string")))
}

/// `synvelis.issue-permit` params → [`PermitSpec`].
///
/// # Errors
/// [`CallToolResult`] — malformed fields (channel/kind mismatches are
/// re-checked at issue).
pub(crate) fn permit_spec_from_params(
    params: Option<&Value>,
    bound: &BoundIdentity,
) -> Result<PermitSpec, CallToolResult> {
    let params = params_obj(params)?;
    let channel = match param_str(params, "channel")? {
        "client" => McpChannel::Client,
        "control" => McpChannel::Control,
        other => return Err(args_fault(format!("unknown channel: {other}"))),
    };
    let kind = match param_str(params, "principal_kind")? {
        "agent" => PrincipalKind::Agent,
        "human" => PrincipalKind::Human,
        other => return Err(args_fault(format!("unknown principal kind: {other}"))),
    };
    let principal = Principal::new(kind, param_str(params, "principal")?).map_err(err)?;
    let project = bound_project_arg(params, bound)?;
    let task = optional_ref(params, "task", TaskRef::new)?;
    let grant = optional_ref(params, "grant", GrantRef::new)?;
    let ttl_ms = match params.get("ttl_ms") {
        Some(v) => Some(
            v.as_u64()
                .ok_or_else(|| args_fault("params.ttl_ms must be an integer"))?,
        ),
        None => None,
    };
    Ok(PermitSpec {
        channel,
        principal,
        project,
        task,
        grant,
        ttl_ms,
    })
}

fn parse_project(tagged: &str) -> Result<synvelis_project::ProjectId, CallToolResult> {
    synvelis_project::ProjectId::from_tagged(tagged)
        .map_err(|_| args_fault("project must be a tagged project id"))
}

/// The `params.project` argument, scoped to the bound permit's
/// project — a control session administers its own project only.
fn bound_project_arg(
    params: &Value,
    bound: &BoundIdentity,
) -> Result<synvelis_project::ProjectId, CallToolResult> {
    let project = parse_project(param_str(params, "project")?)?;
    if project != bound.record.project {
        return Err(fault(&AdapterFault::Unavailable {
            reason: "control administration is scoped to the bound project".to_owned(),
        }));
    }
    Ok(project)
}

fn optional_ref<T>(
    params: &Value,
    key: &str,
    ctor: impl Fn(String) -> Result<T, GatewayError>,
) -> Result<Option<T>, CallToolResult> {
    match params.get(key) {
        Some(v) => {
            let s = v
                .as_str()
                .ok_or_else(|| args_fault(format!("params.{key} must be a string")))?;
            ctor(s.to_owned()).map(Some).map_err(err)
        }
        None => Ok(None),
    }
}

/// `synvelis.issue-task` params → [`AuthorizedTask`].
///
/// # Errors
/// [`CallToolResult`] — malformed fields.
pub(crate) fn task_from_params(
    params: Option<&Value>,
    bound: &BoundIdentity,
) -> Result<AuthorizedTask, CallToolResult> {
    let params = params_obj(params)?;
    let task = TaskRef::new(param_str(params, "task")?).map_err(err)?;
    let principal =
        Principal::new(PrincipalKind::Agent, param_str(params, "principal")?).map_err(err)?;
    let project = bound_project_arg(params, bound)?;
    Ok(AuthorizedTask {
        task,
        principal,
        project,
        activation: Activation::UserIssued,
        validity: Validity::default(),
    })
}

/// `synvelis.issue-grant` params → [`Grant`].
///
/// The `PoC`'s grant surface: exact-scope task-bound grants. Dynamic
/// scope, create axes, proactive permits, and session bindings are
/// not exposed over the control channel — they exist in the
/// internal model and keep their own issuance paths.
///
/// # Errors
/// [`CallToolResult`] — malformed fields or unknown enum names.
pub(crate) fn grant_from_params(
    params: Option<&Value>,
    bound: &BoundIdentity,
) -> Result<Grant, CallToolResult> {
    let params = params_obj(params)?;
    let id = GrantRef::new(param_str(params, "id")?).map_err(err)?;
    let task = TaskRef::new(param_str(params, "task")?).map_err(err)?;
    let principal =
        Principal::new(PrincipalKind::Agent, param_str(params, "principal")?).map_err(err)?;
    let project = bound_project_arg(params, bound)?;
    let capabilities = string_set(params, "capabilities")?
        .iter()
        .map(|id| {
            Ok(CapabilityRef {
                id: CapabilityId::new(id.as_str()).map_err(err)?,
                version: CapabilityVersion::new(1, 0),
            })
        })
        .collect::<Result<BTreeSet<_>, CallToolResult>>()?;
    let permissions = string_set(params, "permissions")?
        .iter()
        .map(|name| permission_kind(name))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let data_access = string_set(params, "data_access")?
        .iter()
        .map(|name| data_access_class(name))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let scope_resources = string_set(params, "scope_resources")?
        .iter()
        .map(|tagged| ResourceRef::parse_tagged(tagged).map_err(err))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let approval_policy = match param_str(params, "approval_policy")? {
        "suggest" => crate::grant::ApprovalPolicy::Suggest,
        "approve-to-apply" => crate::grant::ApprovalPolicy::ApproveToApply,
        "delegated-apply" => crate::grant::ApprovalPolicy::DelegatedApply,
        other => return Err(args_fault(format!("unknown approval policy: {other}"))),
    };
    let apply_budget = match params.get("apply_budget") {
        Some(v) => Some(
            v.as_u64()
                .ok_or_else(|| args_fault("params.apply_budget must be an integer"))?,
        ),
        None => None,
    };
    let issued_at_revision = match params.get("issued_at_revision") {
        Some(v) => Some(ProjectRevision(v.as_u64().ok_or_else(|| {
            args_fault("params.issued_at_revision must be an integer")
        })?)),
        None => None,
    };
    // The create axis is optional — present as a `create_parents` +
    // `create_kinds` pair (either alone is malformed). Placement
    // bounds stay an internal knob, not a PoC wire field.
    let create_parents = optional_string_set(params, "create_parents")?;
    let create_kinds = optional_string_set(params, "create_kinds")?;
    let create_within = match (create_parents, create_kinds) {
        (Some(parents), Some(kinds)) => {
            let parent_refs = parents
                .iter()
                .map(|tagged| ResourceRef::parse_tagged(tagged).map_err(err))
                .collect::<Result<BTreeSet<_>, _>>()?;
            let kind_set = kinds
                .iter()
                .map(|name| resource_kind(name))
                .collect::<Result<BTreeSet<_>, _>>()?;
            Some(crate::grant::CreateWithin {
                parents: crate::grant::OwnerConstraint::OneOf { refs: parent_refs },
                kinds: kind_set,
                placement: None,
            })
        }
        (None, None) => None,
        _ => {
            return Err(args_fault(
                "params.create_parents and params.create_kinds must appear together",
            ));
        }
    };
    Ok(Grant {
        id,
        issued_by: bound.record.principal.clone(),
        principal,
        project,
        binding: GrantBinding::Task { task },
        capabilities,
        permissions: crate::capability::PermissionSet::of(permissions),
        data_access: crate::capability::DataAccessSet::of(data_access),
        scope: ResourceScope::Exact {
            resources: scope_resources,
        },
        create_within,
        approval_policy,
        limits: GrantLimits { apply_budget },
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision,
    })
}

fn string_set(params: &Value, key: &str) -> Result<Vec<String>, CallToolResult> {
    let Some(arr) = params.get(key).and_then(Value::as_array) else {
        return Err(args_fault(format!("params.{key} must be an array")));
    };
    arr.iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| args_fault(format!("params.{key}[{i}] must be a string")))
        })
        .collect()
}

fn optional_string_set(params: &Value, key: &str) -> Result<Option<Vec<String>>, CallToolResult> {
    match params.get(key) {
        Some(_) => string_set(params, key).map(Some),
        None => Ok(None),
    }
}

fn resource_kind(name: &str) -> Result<ResourceKind, CallToolResult> {
    ResourceKind::parse(name).map_err(err)
}

fn permission_kind(name: &str) -> Result<crate::capability::PermissionKind, CallToolResult> {
    use crate::capability::PermissionKind;
    Ok(match name {
        "read-context" => PermissionKind::ReadContext,
        "disclose-context" => PermissionKind::DiscloseContext,
        "mutate-project" => PermissionKind::MutateProject,
        "control-transport" => PermissionKind::ControlTransport,
        "run-job" => PermissionKind::RunJob,
        "external-action" => PermissionKind::ExternalAction,
        other => return Err(args_fault(format!("unknown permission kind: {other}"))),
    })
}

fn data_access_class(name: &str) -> Result<crate::capability::DataAccessClass, CallToolResult> {
    use crate::capability::DataAccessClass;
    Ok(match name {
        "metadata-read" => DataAccessClass::MetadataRead,
        "media-bytes-read" => DataAccessClass::MediaBytesRead,
        "structured-content-read" => DataAccessClass::StructuredContentRead,
        "derived-data-read" => DataAccessClass::DerivedDataRead,
        "workspace-selection-read" => DataAccessClass::WorkspaceSelectionRead,
        "external-disclosure" => DataAccessClass::ExternalDisclosure,
        "provider-transfer" => DataAccessClass::ProviderTransfer,
        other => return Err(args_fault(format!("unknown data access class: {other}"))),
    })
}
