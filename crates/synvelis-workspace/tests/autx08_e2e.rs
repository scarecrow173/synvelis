//! AUTX-08 (synvelis#267) — Leg B: one wire, three surfaces; Leg C:
//! the AI/live-adapter surface.
//!
//! The headless common-E2E slice freezes the shared contract that the
//! GUI automation surface, the local CLI session host, the MCP
//! adapter, and the production `GatewayStepLane` live adapter all
//! lower into the *same* gateway wire semantics: same
//! capability/schema/project/args/consistency/target/idempotency
//! dimensions, the same trusted-caller attestation boundary, the same
//! parse → bind → invoke → authorize pipeline, and the same typed
//! `GatewayOutcome`/`ErrorEnvelope` verdicts.
//!
//! Each surface wraps an independently [`compose`]d gateway stack —
//! identical fixture state and compose recipe, so per-instance
//! counters (operation ids, proposal refs, snapshot pins) align and
//! the gateway-produced payloads are directly comparable. The test
//! never duplicates authority logic: it compares what each surface's
//! own lowering/attestation produced against the real `Gateway`.
//!
//! Leg C exercises the production [`GatewayStepLane`] — the AI
//! surface's `StepCall` → `step_wire` → `Gateway::handle` path —
//! through the public [`StepLane`] seam (`invoke` /
//! `operation_result`). `step_wire` is adapter-internal, so wire
//! byte equality is not asserted; outcome/error/`DuplicateRequest`
//! parity on the public surface is the contract (AUTX-E2E-SAME-01).
//!
//! Scope honesty: this is the headless slice only. Passing here does
//! not satisfy real Windows GUI-L2 or EXT-L3 acceptance — those stay
//! with #114/#267's OS-evidence lane (see `docs/test-specs/
//! autx-08-common-e2e.md` Non-goals).

mod rig;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use synvelis_automation::coordinator::{OperationObservation, StepCall, StepLane};
use synvelis_automation::{GatewayStepLane, VersionedRef};
use synvelis_gateway::mcp::{CONNECT_METHOD, MCP_ADAPTER_RAW};
use synvelis_gateway::*;
use synvelis_workspace::{AutomationCatalog, AutomationViewAction};

use rig::*;

// ── the shared wire ──────────────────────────────────────────────────

/// The engine-registered prepare capability every surface lowers to —
/// never a fixture-only descriptor.
const PREPARE_CAPABILITY: &str = "synvelis.operation.prepare";

/// The shared mutation: `synvelis.midi.note.add` on `source10` — the
/// linked-MIDI demo source whose write propagates to clip12 and clip13.
/// The args shape is exactly what `midi.edit.prepare` lowers on the
/// MCP side (`capability`/`target`/`args` triple inside `operation`).
fn note_add_args() -> Value {
    json!({
        "operation": op(
            "synvelis.midi.note.add",
            res_target("source", &source10()),
            &json!({
                "key": 67,
                "velocity": 90,
                "range": {"domain": "source-native", "start": 720, "len": 240},
            }),
        ),
    })
}

/// The agent authority every delegated surface binds: task `t1` +
/// grant `g1` covering `midi.note.add` on the linked source, with the
/// note-create axis scoped inside `source10` — the same shape the GW-09
/// rig's `note_grant` uses.
fn issue_agent_authority(ports: &GatePorts) {
    ports.issue_task(task("t1")).expect("issue task");
    let mut grant = agent_grant(
        "g1",
        "t1",
        &["synvelis.midi.note.add"],
        ResourceScope::Exact {
            resources: BTreeSet::from([clip12(), clip13(), source10()]),
        },
        ApprovalPolicy::DelegatedApply,
    );
    grant.create_within = Some(CreateWithin {
        parents: OwnerConstraint::OneOf {
            refs: BTreeSet::from([source10()]),
        },
        kinds: BTreeSet::from([ResourceKind::Note]),
        placement: None,
    });
    ports.issue_grant(grant).expect("issue grant");
}

/// The authority-pushed [`AutomationCatalog`] for `gw`'s registered
/// capabilities — every verdict `satisfied` so projections expose
/// executable rows. Grant verdicts are display-only; the Gateway
/// re-checks authority per call.
fn automation_catalog(gw: &Gateway) -> AutomationCatalog {
    let registry = gw.registry();
    let capabilities: Vec<Value> = registry
        .list()
        .iter()
        .map(|cap| {
            let descriptor = registry.describe(cap).expect("listed capability describes");
            json!({ "descriptor": descriptor, "grant": { "state": "satisfied" } })
        })
        .collect();
    AutomationCatalog::from_json(&json!({
        "seq": 1,
        "capabilities": capabilities,
        "macros": [],
    }))
    .expect("catalog parses")
}

// ── surface legs ─────────────────────────────────────────────────────

/// The GUI leg: the pushed catalog lowers the palette `Submit` into a
/// wire request document (the `syn.autx.action` → `lower` → gateway
/// lane path), then the session gateway handles it under the bound
/// trusted human caller. Returns the lowered wire document and the
/// typed outcome.
fn gui_leg(
    consistency: &Value,
    idempotency_key: Option<&str>,
) -> (Value, Result<GatewayOutcome<Value>, ErrorEnvelope>) {
    let mut s = compose(None);
    let catalog = automation_catalog(&s.gw);
    let mut requests = catalog
        .lower(
            &AutomationViewAction::Submit {
                capability: format!("{PREPARE_CAPABILITY}@1.0"),
                target: None,
                args: note_add_args(),
                idempotency_key: idempotency_key.map(str::to_owned),
            },
            &project().to_tagged(),
            "autx08-gui",
            consistency,
        )
        .expect("gui lowering succeeds");
    assert_eq!(requests.len(), 1, "submit lowers to exactly one request");
    let wire = requests.remove(0);
    let caller = human();
    let outcome = s.gw.handle(&wire, caller);
    (wire, outcome)
}

/// The local-CLI leg: [`HeadlessSessionHost::handle_claim`] attests
/// the unattended claim to the delegated agent-equivalent caller, then
/// runs the wire document through `Gateway::handle`. Returns the wire
/// document and the typed outcome.
fn cli_leg(
    consistency: Value,
    with_authority: bool,
) -> (Value, Result<GatewayOutcome<Value>, ErrorEnvelope>) {
    let s = compose(None);
    if with_authority {
        issue_agent_authority(&s.ports);
    }
    let wire = wire(
        PREPARE_CAPABILITY,
        note_add_args(),
        with_authority.then_some("t1"),
        with_authority.then_some("g1"),
        consistency,
    );
    let host = HeadlessSessionHost::new(
        s.gw,
        Box::new(MemoryCliSessionStore::new()),
        s.ports.clone(),
        s.ports.clone(),
        s.clock.clone(),
        AdapterId::new(CLI_ADAPTER_RAW),
    );
    let outcome = host.handle_claim(
        &wire,
        &CliSessionClaim::Unattended {
            principal: "mcp-client-1".to_owned(),
        },
    );
    (wire, outcome)
}

/// One `midi.edit.prepare` tools/call on a fresh MCP client session
/// bound to permit `(t1, g1)` — or to no authority when
/// `with_authority` is false. Returns the `CallToolResult` JSON.
fn mcp_prepare_call(with_authority: bool, base_revision: u64) -> Value {
    let s = compose(None);
    if with_authority {
        issue_agent_authority(&s.ports);
    }
    let mut permits = MemoryPermitStore::new();
    let issue = permits
        .issue(
            PermitSpec {
                channel: McpChannel::Client,
                principal: agent_principal(),
                project: project(),
                task: with_authority.then(|| task_ref("t1")),
                grant: with_authority.then(|| grant_ref("g1")),
                ttl_ms: None,
            },
            s.clock.now(),
        )
        .expect("permit issue");
    let host = McpHost::new(
        s.gw,
        Box::new(permits),
        s.ports.clone(),
        s.ports.clone(),
        s.clock.clone(),
        McpConfig {
            adapter: AdapterId::new(MCP_ADAPTER_RAW),
            bounds: McpBounds::default(),
            enabled: true,
        },
    );
    let mut session = host.new_session(McpChannel::Client);
    let connected = host
        .handle(
            &mut session,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": CONNECT_METHOD,
                "params": {"permit": issue.secret},
            }),
        )
        .expect("connect responds");
    assert!(
        connected.get("error").is_none(),
        "connect failed: {connected}"
    );
    host.handle(
        &mut session,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "midi.edit.prepare",
                "arguments": {
                    "operation": {
                        "capability": "synvelis.midi.note.add",
                        "target": {"kind": "source", "id": tagged(&source10())},
                        "args": {
                            "key": 67,
                            "velocity": 90,
                            "range": {"domain": "source-native", "start": 720, "len": 240},
                        },
                    },
                    "base_revision": base_revision,
                },
            },
        }),
    )
    .expect("tools/call responds")
    .get("result")
    .expect("tools/call result")
    .clone()
}

/// The MCP leg's semantic payload — `structuredContent` on success,
/// the `{"error": …}` projection on failure.
fn mcp_structured(tool_result: &Value) -> &Value {
    &tool_result["structuredContent"]
}

// ── AUTX-E2E-SAME-01: same wire, same outcome ────────────────────────

/// GUI (catalog lowering + human caller), local CLI (unattended claim
/// → delegated agent caller), and MCP (`midi.edit.prepare` →
/// `synvelis.operation.prepare`) all carry the same wire semantics for
/// the same mutation and land the same gateway-produced outcome.
#[test]
fn autx08_same_wire_lands_same_outcome_across_surfaces() {
    let (gui_wire, gui_outcome) = gui_leg(&expected(1), None);
    let (cli_wire, cli_outcome) = cli_leg(expected(1), true);
    let mcp_tool = mcp_prepare_call(true, 1);

    // ── wire equivalence: the semantic fields are identical; only the
    // surface-scoped correlation/authority fields differ by design.
    for key in [
        "capability",
        "schema",
        "project",
        "args",
        "consistency",
        "target",
    ] {
        assert_eq!(
            gui_wire[key], cli_wire[key],
            "wire field {key} diverges between GUI lowering and CLI wire"
        );
    }
    assert_ne!(
        gui_wire["request_id"], cli_wire["request_id"],
        "request ids are surface-minted correlation ids"
    );
    // The GUI human caller carries no delegated authority refs; the
    // CLI agent wire binds task+grant explicitly.
    assert!(
        gui_wire.get("task").is_none_or(Value::is_null)
            && gui_wire.get("grant").is_none_or(Value::is_null),
        "human GUI wire must not carry task/grant: {gui_wire}"
    );
    assert_eq!(cli_wire["task"], json!("t1"));
    assert_eq!(cli_wire["grant"], json!("g1"));

    // ── outcome equivalence: identical `GatewayOutcome` payloads.
    let gui = gui_outcome.expect("gui prepare completes");
    let cli = cli_outcome.expect("cli prepare completes");
    assert_eq!(gui, cli, "GUI vs CLI gateway outcome diverges");
    assert_eq!(
        mcp_tool["isError"],
        json!(false),
        "mcp prepare failed: {mcp_tool}"
    );
    // `translate_outcome` surfaces `Completed{result}` verbatim as
    // `structuredContent` — the same payload the other surfaces saw.
    let gui_result = complete(gui);
    assert_eq!(
        &gui_result,
        mcp_structured(&mcp_tool),
        "MCP tool payload diverges from the gateway result"
    );
    assert_eq!(
        complete(cli),
        gui_result,
        "CLI result payload diverges from GUI"
    );
}

// ── fail-closed parity ───────────────────────────────────────────────

/// An `expected-revision` the snapshot seam cannot serve (revision 99
/// was never committed → `SnapshotExpired`) fails identically on every
/// surface — consistency is wire semantics, not adapter policy.
#[test]
fn autx08_unavailable_revision_fails_identically_across_surfaces() {
    let (_gui_wire, gui_outcome) = gui_leg(&expected(99), None);
    let (_cli_wire, cli_outcome) = cli_leg(expected(99), true);
    let mcp_tool = mcp_prepare_call(true, 99);

    let gui_err = gui_outcome.expect_err("unservable revision must fail");
    let cli_err = cli_outcome.expect_err("unservable revision must fail");
    assert_eq!(
        gui_err, cli_err,
        "GUI vs CLI error envelope diverges on the same wire failure"
    );
    assert_eq!(
        mcp_tool["isError"],
        json!(true),
        "mcp unservable revision must fail: {mcp_tool}"
    );
    // The MCP boundary projects the same typed envelope through
    // `ExternalError` (diagnostics stripped) — compare the wire-safe
    // projection, never the internal diagnostic.
    let external = serde_json::to_value(gui_err.to_external()).expect("external error serializes");
    assert_eq!(
        mcp_structured(&mcp_tool)
            .get("error")
            .expect("error payload"),
        &external,
        "MCP error projection diverges from the gateway envelope"
    );
}

/// Delegated surfaces without task/grant authority fail closed with the
/// same verdict; the human GUI caller on the same wire still completes
/// — the human/agent authority boundary is preserved per surface.
#[test]
fn autx08_agent_without_authority_fails_closed_gui_human_permits() {
    let (_gui_wire, gui_outcome) = gui_leg(&expected(1), None);
    let (_cli_wire, cli_outcome) = cli_leg(expected(1), false);
    let mcp_tool = mcp_prepare_call(false, 1);

    assert!(
        matches!(gui_outcome, Ok(GatewayOutcome::Completed { .. })),
        "human GUI caller must complete without delegated refs: {gui_outcome:?}"
    );

    let cli_err = cli_outcome.expect_err("agent without task/grant must fail");
    assert_eq!(
        mcp_tool["isError"],
        json!(true),
        "mcp agent without authority must fail: {mcp_tool}"
    );
    let external = serde_json::to_value(cli_err.to_external()).expect("external error serializes");
    assert_eq!(
        mcp_structured(&mcp_tool)
            .get("error")
            .expect("error payload"),
        &external,
        "CLI vs MCP denial diverges for the same unauthorized wire"
    );
}

// ── idempotency parity ───────────────────────────────────────────────

/// An idempotency-keyed replay names the same original operation on
/// every surface that carries the key — the ledger semantics are wire
/// semantics, not adapter bookkeeping. (MCP `midi.edit.prepare` does
/// not expose an idempotency field; `proposal.apply` does — covered by
/// the gateway's own ledger tests.)
#[test]
fn autx08_idempotent_replay_names_same_operation() {
    // GUI leg with an idempotency key, then an identical replay on the
    // same composed gateway.
    let mut s = compose(None);
    let catalog = automation_catalog(&s.gw);
    let action = AutomationViewAction::Submit {
        capability: format!("{PREPARE_CAPABILITY}@1.0"),
        target: None,
        args: note_add_args(),
        idempotency_key: Some("autx08-idem".to_string()),
    };
    let gui_wire = catalog
        .lower(&action, &project().to_tagged(), "autx08-gui", &expected(1))
        .expect("gui lowering")
        .remove(0);
    let caller = human();
    assert!(matches!(
        s.gw.handle(&gui_wire, caller.clone()),
        Ok(GatewayOutcome::Completed { .. })
    ));
    // Replay with a fresh `request_id` — production retries always
    // mint a new correlation id, so dedup must key on
    // `idempotency_key` + fingerprint, never on `request_id`.
    let mut gui_replay_wire = gui_wire.clone();
    gui_replay_wire["request_id"] = json!("autx08-gui-replay");
    let gui_replay =
        s.gw.handle(&gui_replay_wire, caller)
            .expect_err("keyed replay must not re-execute");

    // CLI leg — same key on the wire document, same replay.
    let s_cli = compose(None);
    issue_agent_authority(&s_cli.ports);
    let cli_wire = {
        let mut w = wire(
            PREPARE_CAPABILITY,
            note_add_args(),
            Some("t1"),
            Some("g1"),
            expected(1),
        );
        w["idempotency_key"] = json!("autx08-idem");
        w
    };
    let host = HeadlessSessionHost::new(
        s_cli.gw,
        Box::new(MemoryCliSessionStore::new()),
        s_cli.ports.clone(),
        s_cli.ports.clone(),
        s_cli.clock.clone(),
        AdapterId::new(CLI_ADAPTER_RAW),
    );
    let claim = CliSessionClaim::Unattended {
        principal: "mcp-client-1".to_owned(),
    };
    assert!(matches!(
        host.handle_claim(&cli_wire, &claim),
        Ok(GatewayOutcome::Completed { .. })
    ));
    // Same fresh-`request_id` replay shape as the GUI leg.
    let mut cli_replay_wire = cli_wire.clone();
    cli_replay_wire["request_id"] = json!("autx08-cli-replay");
    let cli_replay = host
        .handle_claim(&cli_replay_wire, &claim)
        .expect_err("keyed replay must not re-execute");

    // Both replays are `DuplicateRequest` naming the operation that
    // actually ran — and because both stacks composed identically,
    // the named operation ids are the same.
    let (
        GatewayError::DuplicateRequest {
            original: gui_original,
        },
        GatewayError::DuplicateRequest {
            original: cli_original,
        },
    ) = (gui_replay.code, cli_replay.code)
    else {
        panic!("expected DuplicateRequest on both surfaces");
    };
    assert_eq!(gui_original, cli_original);
}

// ── Leg C: the AI / live-adapter surface ─────────────────────────────
//
// The production `GatewayStepLane` (`synvelis-automation` live
// adapters, AUTX-06) lowers each `StepCall` through its own
// adapter-internal `step_wire` producer and dispatches it through the
// same `Gateway::handle` parse → bind → invoke → authorize pipeline
// under a `LocalCliAdapter`-attested delegated caller. Parity is
// asserted on the public `StepLane` surface (`invoke` /
// `operation_result`), never on wire bytes — the same convention the
// MCP leg uses for its adapter-internal wire.

/// The production live step lane over a fresh [`compose`] stack —
/// the same recipe every Leg B surface wraps. The gateway moves
/// behind the `Arc<Mutex<..>>` the lane locks per dispatch,
/// [`GatePorts`] supplies the `CommitPort` reconcile seam, and a
/// `LocalCliAdapter` attests agent principals through
/// `caller_delegated` — the same delegated-agent identity the CLI
/// leg's unattended claim produces.
fn live_lane(with_authority: bool) -> GatewayStepLane {
    let s = compose(None);
    if with_authority {
        issue_agent_authority(&s.ports);
    }
    let gateway = Arc::new(Mutex::new(s.gw));
    let cli = Arc::new(LocalCliAdapter::new(
        AdapterId::new(CLI_ADAPTER_RAW),
        Box::new(MemoryCliSessionStore::new()),
        s.clock.clone(),
    ));
    let committer: Arc<dyn CommitPort> = s.ports.clone();
    GatewayStepLane::new(gateway, cli, committer, s.jobs)
}

/// The `StepCall` the live lane lowers — the same logical operation
/// every Leg B surface sends: `synvelis.operation.prepare@1.0`
/// wrapping `synvelis.midi.note.add` on `source10` (the args shape is
/// [`note_add_args`]'s `{operation: …}` verbatim), CAS-pinned at
/// `expected_revision: 1`, under the delegated agent's `t1`/`g1`
/// authority on `project()`.
fn live_step_call() -> StepCall {
    StepCall {
        capability: VersionedRef::new(PREPARE_CAPABILITY, 1, 0),
        args: note_add_args(),
        target: None,
        expected_revision: Some(1),
        idempotency_key: IdempotencyKey::new("autx08-live-1").expect("idempotency key"),
        policy_epoch: None,
        principal: agent_principal(),
        project_id: project().to_tagged(),
        task: Some(task_ref("t1")),
        grant: Some(grant_ref("g1")),
    }
}

/// AUTX-E2E-SAME-01-AI (AI leg): the live lane's `invoke` lands the
/// same `GatewayOutcome` payload as every Leg B surface — the
/// delegated agent caller, the `t1`/`g1` binding, the CAS pin, and
/// the prepare result are wire semantics the lane must not diverge
/// on. Per-instance counters align because each leg composes the
/// same recipe and issues exactly one gateway call.
#[test]
fn autx08_live_adapter_lands_same_outcome() {
    let lane = live_lane(true);
    let live = lane
        .invoke(&live_step_call())
        .expect("live prepare completes");

    let (_gui_wire, gui_outcome) = gui_leg(&expected(1), None);
    let (_cli_wire, cli_outcome) = cli_leg(expected(1), true);
    let mcp_tool = mcp_prepare_call(true, 1);

    let cli = cli_outcome.expect("cli prepare completes");
    assert_eq!(live, cli, "live lane vs CLI surface outcome diverges");
    let gui = gui_outcome.expect("gui prepare completes");
    assert_eq!(live, gui, "live lane vs GUI surface outcome diverges");
    assert_eq!(
        mcp_tool["isError"],
        json!(false),
        "mcp prepare failed: {mcp_tool}"
    );
    assert_eq!(
        &complete(live),
        mcp_structured(&mcp_tool),
        "live lane result payload diverges from the MCP surface"
    );
}

/// AUTX-E2E-FAILCLOSED-AI-01 (AI leg): an unservable
/// `expected-revision` fails with the same typed `ErrorEnvelope` —
/// `SnapshotExpired` is snapshot-seam semantics carried on the wire,
/// not adapter policy. The MCP projection parity matches the Leg B
/// convention (`ExternalError`, diagnostics stripped).
#[test]
fn autx08_live_adapter_unavailable_revision_fails_identically() {
    let lane = live_lane(true);
    let mut call = live_step_call();
    call.expected_revision = Some(99);
    let live_err = lane
        .invoke(&call)
        .expect_err("unservable revision must fail");
    assert!(
        matches!(live_err.code, GatewayError::SnapshotExpired),
        "expected SnapshotExpired, got {:?}",
        live_err.code
    );

    let (_gui_wire, gui_outcome) = gui_leg(&expected(99), None);
    let (_cli_wire, cli_outcome) = cli_leg(expected(99), true);
    let mcp_tool = mcp_prepare_call(true, 99);

    let gui_err = gui_outcome.expect_err("unservable revision must fail");
    let cli_err = cli_outcome.expect_err("unservable revision must fail");
    assert_eq!(
        *live_err, cli_err,
        "live lane vs CLI error envelope diverges"
    );
    assert_eq!(gui_err, cli_err, "GUI vs CLI error envelope diverges");
    assert_eq!(
        mcp_tool["isError"],
        json!(true),
        "mcp unservable revision must fail: {mcp_tool}"
    );
    let external = serde_json::to_value(live_err.to_external()).expect("external error serializes");
    assert_eq!(
        mcp_structured(&mcp_tool)
            .get("error")
            .expect("error payload"),
        &external,
        "MCP error projection diverges from the live lane envelope"
    );
}

/// AUTX-E2E-FAILCLOSED-AI-02 (AI leg): a delegated agent `StepCall`
/// without task/grant authority fails closed with the same typed
/// denial the CLI/MCP legs produce — `attest` mints the delegated
/// agent caller but the lane never fabricates authority the wire
/// does not carry.
#[test]
fn autx08_live_adapter_without_authority_fails_closed() {
    let lane = live_lane(false);
    let mut call = live_step_call();
    call.task = None;
    call.grant = None;
    let live_err = lane
        .invoke(&call)
        .expect_err("agent without task/grant must fail");

    let (_cli_wire, cli_outcome) = cli_leg(expected(1), false);
    let cli_err = cli_outcome.expect_err("agent without task/grant must fail");
    assert_eq!(*live_err, cli_err, "live lane vs CLI denial diverges");

    let mcp_tool = mcp_prepare_call(false, 1);
    assert_eq!(
        mcp_tool["isError"],
        json!(true),
        "mcp agent without authority must fail: {mcp_tool}"
    );
    let external = serde_json::to_value(cli_err.to_external()).expect("external error serializes");
    assert_eq!(
        mcp_structured(&mcp_tool)
            .get("error")
            .expect("error payload"),
        &external,
        "MCP denial diverges from the live lane envelope"
    );
}

/// AUTX-E2E-IDEM-AI (AI leg): replaying the same `StepCall` — the lane
/// mints a fresh `wf-req-*` request id per invoke, the production
/// retry shape — surfaces `DuplicateRequest{original}` naming the
/// same operation the Leg B CLI replay names on an identically
/// composed stack, and `operation_result` reconciles the original
/// to the recorded `Settled` outcome.
#[test]
fn autx08_live_adapter_duplicate_reconciles_original() {
    let lane = live_lane(true);
    let call = live_step_call();
    let first = lane.invoke(&call).expect("first invoke completes");
    let err = lane
        .invoke(&call)
        .expect_err("keyed replay must not re-execute");
    let GatewayError::DuplicateRequest { original } = err.code else {
        panic!("expected DuplicateRequest, got {:?}", err.code);
    };

    // The lane's reconcile read returns the recorded keyed outcome —
    // the same `Completed` payload the first invoke produced.
    match lane.operation_result(&original) {
        OperationObservation::Settled(GatewayOutcome::Completed { result }) => {
            assert_eq!(result, complete(first));
        }
        other => panic!("expected Settled(Completed), got {other:?}"),
    }

    // The Leg B CLI surface replaying the same idempotency key on an
    // identically-composed stack names the same original operation —
    // ledger semantics are wire semantics on the live lane too.
    let s_cli = compose(None);
    issue_agent_authority(&s_cli.ports);
    let cli_wire = {
        let mut w = wire(
            PREPARE_CAPABILITY,
            note_add_args(),
            Some("t1"),
            Some("g1"),
            expected(1),
        );
        w["idempotency_key"] = json!("autx08-live-1");
        w
    };
    let host = HeadlessSessionHost::new(
        s_cli.gw,
        Box::new(MemoryCliSessionStore::new()),
        s_cli.ports.clone(),
        s_cli.ports.clone(),
        s_cli.clock.clone(),
        AdapterId::new(CLI_ADAPTER_RAW),
    );
    let claim = CliSessionClaim::Unattended {
        principal: "mcp-client-1".to_owned(),
    };
    assert!(matches!(
        host.handle_claim(&cli_wire, &claim),
        Ok(GatewayOutcome::Completed { .. })
    ));
    let mut cli_replay_wire = cli_wire.clone();
    cli_replay_wire["request_id"] = json!("autx08-cli-replay");
    let cli_replay = host
        .handle_claim(&cli_replay_wire, &claim)
        .expect_err("keyed replay must not re-execute");
    let GatewayError::DuplicateRequest {
        original: cli_original,
    } = cli_replay.code
    else {
        panic!("expected DuplicateRequest on the CLI leg");
    };
    assert_eq!(original, cli_original);
}
