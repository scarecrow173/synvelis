# AUTX-08 Headless Common E2E Case Definitions (synvelis#267)

Date: 2026-11-15（Leg A+B freeze）/ 2026-11-16（Leg C
live-adapter lane 追記）。Status: **frozen at definition
level for the headless slice — no real-OS, real-GUI,
real-provider, or multi-process lane was executed by this
slice.** Every Real OS / Native Plugin row below is
`NotRun`; the Headless rows point at
`crates/synvelis-workspace/tests/autx08_e2e.rs` executed
under `mise run test synvelis-workspace`. Leg C adds the
AI/live-adapter surface — production `GatewayStepLane`
over `rig::compose()` — proven on the public `StepLane`
surface (`invoke`/`operation_result`); `step_wire` is
adapter-internal private so wire byte equality is not
asserted (same convention as the MCP leg's internal
`wire_for`).

Source of truth for case identity: synvelis Issue #267 and
`docs/automation/tasks.md` §AUTX-08 (T-08-1〜T-08-3). Contract
freeze: `docs/test-specs/autx-08-common-e2e.md` (Rules
AUTX-E2E-SAME-01 / FAILCLOSED / IDEM / NOAUTHORITY). Spec anchor:
`docs/specs/command-gateway.md`, `docs/designs/
command-gateway-design.md`, `docs/automation/
design-capability-action.md`, `docs/automation/
design-trust-boundary.md`.

## Evidence schema (frozen)

Each executed case row MUST carry:

| Field | Meaning |
| --- | --- |
| Case | `AUTX-E2E-*` id |
| Lane | `headless` / `real-os` / `native-plugin` / `live-adapter` |
| Surface(s) | GUI catalog lowering / local CLI claim / MCP tool |
| Expected | observable expectation before the run |
| Observed | what was actually observed (empty while NotRun) |
| Result | `Pass` / `Fail` / `NotRun` / `Unsupported` / `Deferred` (POLICIES.md §3 vocabulary) |
| Artifact | test name / log / receipt paths |
| Owner | accepting owner (synvelis issue / CI lane) |

Evaluation lanes are recorded separately and never merged:

- **Headless** — in-process `Gateway`/`HeadlessSessionHost`/
  `McpHost` over the shared `compose()` recipe (OS-L1 semantic
  layer, real engines + real `CommitGate`/journal/durable store,
  fixture caller seals).
- **Real OS (GUI-L2)** — real GPUI window, real OS user input,
  real windowing/session lifetime (Windows-first per POLICIES.md;
  macOS/Linux deferred until capability is required).
- **EXT-L3** — real provider/plugin/OS-API/credential-store
  dependencies (real CLI binary, real session credential verification,
  real MCP transport process).

Headless Pass never promotes to GUI-L2/EXT-L3 Pass.

## Lane status summary (freeze snapshot)

| Case | Headless | Real OS (Windows first) | EXT-L3 / live adapter | Primary Owner |
| --- | --- | --- | --- | --- |
| AUTX-E2E-SAME-01 | `autx08_e2e.rs` (8 tests — Leg B 3 surface + Leg C AI lane) | NotRun | NotRun | synvelis#267 (headless slice) / #114 (GUI-L2) |
| AUTX-E2E-ORACLE-01 | same file — 4 independent `compose()` stacks (Leg C 含む) | NotRun | NotRun | synvelis#267 |
| AUTX-E2E-OS-EVID | n/a (definition only) | NotRun | NotRun | synvelis#267 acceptance lane |

## Case definitions

### AUTX-E2E-SAME-01 — same wire, same outcome (T-08-1)

- **Lane**: headless.
- **Surfaces**: GUI = `AutomationCatalog::lower(Submit)` on
  engine-registered `synvelis.operation.prepare@1.0` → wire JSON →
  `Gateway::handle` under the fixture human caller (the
  `syn.autx.action` → `lower` → gateway-lane path).
  CLI = `HeadlessSessionHost::handle_claim(CliSessionClaim::
  Unattended)` → `LocalCliAdapter::attest` → delegated agent
  caller (`Agent`/`LocalCli`)。wire document は rig helper で
  構築する — `synvelis-cli` の `build_request`/`claim_for`
  lowering は本 crate の依存境界外であり、binary envelope は
  `cli_e2e.rs` が、field 一致は deferred cross-check が担う。
  MCP = `McpHost` `synvelis.connect` (client permit `(t1,g1)`)
  → `tools/call midi.edit.prepare` → `wire_for` →
  `synvelis.operation.prepare` under the permit-bound agent caller
  (`Agent`/`ExternalAdapter`).
- **Shared mutation**: prepare of `synvelis.midi.note.add` on
  `source10` (linked-MIDI demo source; write propagates to clip12
  and clip13), `consistency = expected-revision:1`.
- **Expected**: wire semantic fields (capability, schema, project,
  args, consistency, target) identical across GUI-lowered and
  CLI-constructed wire documents (GUI↔CLI direct field compare;
  the MCP wire is adapter-internal so MCP equivalence is proven by
  outcome/error parity — see spec Rule SAME-01); `GatewayOutcome::
  Completed` result payloads identical across all three surfaces
  (per-instance counters align because every stack is composed by
  the same recipe and sees exactly one gateway call); MCP
  `structuredContent` equals the gateway `result` verbatim.
- **Failure variants**: unservable `expected-revision:99`
  (`SnapshotExpired`) → identical typed `ErrorEnvelope` on all
  three surfaces (MCP observed as `ExternalError` projection);
  agent wire without task/grant → identical `Denied` on CLI and
  MCP while the human GUI caller on the same wire completes.
- **Result**: **Pass (headless)** — `autx08_same_wire_lands_
  same_outcome_across_surfaces`, `autx08_unavailable_revision_
  fails_identically_across_surfaces`, `autx08_agent_without_
  authority_fails_closed_gui_human_permits`.
- **AI leg (Leg C, 2026-11-16)**: production
  `GatewayStepLane`（`synvelis_automation::live`）を
  `rig::compose()` の fresh `GatewayStack` 上に
  `GatewayStepLane::new` で組み立て、`StepLane::invoke`
  で同一論理操作（`synvelis.operation.prepare@1.0` +
  `midi.note.add` on `source10`・`expected_revision:1`・
  `t1`/`g1`・agent principal）を投入。`step_wire` は
  adapter-internal private のため wire byte ではなく、
  公開 `StepLane` surface が産出する
  `GatewayOutcome`/`ErrorEnvelope`/`DuplicateRequest`
  parity で証明する。**Result: Pass (headless)** —
  `autx08_live_adapter_lands_same_outcome`、
  `autx08_live_adapter_unavailable_revision_fails_
  identically`、`autx08_live_adapter_without_authority_
  fails_closed`（delegated agent 経路のみ — Human
  principal の `attest` fail-closed は adapter contract
  の別経路）。live catalog 実供給・dispatch-hop
  consumer・`GatewayJobLane` job 観測は依然 Deferred。

### AUTX-E2E-ORACLE-01 — independent policy/commit oracle (T-08-2)

- **Lane**: headless.
- **Expected**: each surface's outcome is produced by its own
  independently composed stack — own `InMemoryPolicyStore`, own
  `CommitGate` + journal + durable `DirectoryBundleStore`, own
  `ProposalStore`, own idempotency ledger. No shared mutable
  authority state exists between surface runs; identical verdicts
  therefore demonstrate that the contract lives in the wire +
  gateway pipeline, not in adapter-local state. Idempotent replay
  on GUI and CLI stacks names the same `DuplicateRequest.original`
  operation id, proving ledger semantics are wire semantics.
- **Result**: **Pass (headless)** — same test file; every leg
  calls `compose(None)` fresh (own temp dir, own journal, own
  store).

### AUTX-E2E-IDEM-01 — idempotency parity (subset of T-08-1)

- **Lane**: headless.
- **Expected**: `idempotency_key` carried on GUI `Submit` and CLI
  wire replays — with a fresh `request_id`, matching the
  production retry shape — to `DuplicateRequest{original}` naming
  the first operation; MCP `midi.edit.prepare` exposes no
  idempotency field (adapter contract boundary —
  `proposal.apply` carries it, covered by gateway ledger tests).
- **Result**: **Pass (headless)** —
  `autx08_idempotent_replay_names_same_operation`。
- **AI leg (Leg C, 2026-11-16)**: 同一 `idempotency_key`
  の `StepCall` 再 `invoke`（lane が mint する新
  `wf-req-*` request_id — production retry と同じ形）が
  `GatewayError::DuplicateRequest{original}` を返し
  Leg B replay と同一 `original` operation を名指し、
  `StepLane::operation_result(&original)` が recorded
  outcome を `OperationObservation::Settled` として返す。
  **Result: Pass (headless)** —
  `autx08_live_adapter_duplicate_reconciles_original`。
  journal replay oracle は Leg B AUTX-E2E-ORACLE-01
  責務のまま。

### AUTX-E2E-OS-EVID — real OS evidence separation (T-08-3)

- **Lane**: real-os / ext-l3 — **definition only**.
- **Expected**: the #267 acceptance lane records Required /
  Deferred / NotRun per POLICIES.md §2-3 with OS, toolchain, exit
  code, and artifact pointers; headless Pass is never reported as
  real-machine Pass.
- **Result**: **NotRun** — this slice produces the headless half of
  the ledger only; the real-OS rows stay open for the #267
  acceptance lane (see `acceptance-ledger.md` §NotRun/Deferred).
