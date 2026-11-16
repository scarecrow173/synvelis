---
id: "DESIGN-034YqseBG2a0IpRjmMQrwF"
type: "design"
status: "approved"
title: "AUTX-08 Leg C live-adapter leg — 設計"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/267"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx08_e2e.rs"
      - type: "file"
        path: "evidence/sv267/live-adapter-leg.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/test-specs/autx-08-common-e2e.md"
      - type: "file"
        path: "evidence/sv267/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv267/case-definitions.md"
      - type: "file"
        path: "evidence/sv267/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/autx-08-live-adapter-leg.md"
  implemented-by: []
  depends-on:
    - "docs/test-specs/autx-08-common-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/autx-08-live-adapter-leg.md"
    - "docs/test-specs/autx-08-live-adapter-leg.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/autx-08-live-adapter-leg.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-08-live-adapter-leg.md"
  references:
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/coordinator.rs"
    - "crates/synvelis-workspace/tests/autx08_e2e.rs"
    - "crates/synvelis-workspace/tests/rig/mod.rs"
  defers: []
  deferred-by: []
---

# AUTX-08 Leg C live-adapter leg — 設計

## Context

Issue #267 AUTX-08 T-08-1 の AI lane。Leg B
（`crates/synvelis-workspace/tests/autx08_e2e.rs`）は GUI
automation surface・local CLI session host・MCP adapter の
3 surface が同一 wire semantics で同一 `Gateway` pipeline を
通ることを `rig::compose()` の独立 stack 上で固定済み。
本 slice は第 4 surface — production `GatewayStepLane`
（`synvelis-automation` live adapters、AUTX-06 merge 済み）
— を同じ契約に加える。

制約: `step_wire`（`StepCall` → wire JSON producer）は
adapter-internal private fn（`live.rs` L227）で公開 surface
から wire byte を観測できない。adapter src は READ-ONLY
（lease 外・変更禁止）。したがって等価の証明面は公開
`StepLane` trait（`invoke`/`operation_result`）が産出する
`GatewayOutcome`/`ErrorEnvelope`/`GatewayError::
DuplicateRequest{original}`/`OperationObservation` に限られる
— これは `docs/test-specs/autx-08-common-e2e.md` Rule
AUTX-E2E-SAME-01 が MCP leg に適用した convention
（adapter-internal wire は outcome/error parity で証明）と
同型である。

## Scope

### In Scope

- `crates/synvelis-workspace/tests/autx08_e2e.rs` への Leg C
  section 追加: `live_lane(with_authority)` /
  `live_step_call()` helper と 4 test
  （`autx08_live_adapter_*`）。
- `crates/synvelis-workspace/Cargo.toml`
  `[dev-dependencies]` への `synvelis-automation` path
  dev-dep（承認済み）+ `Cargo.lock` 差分。
- evidence/ledger/case-definitions/verification への Leg C
  記録と `docs/test-specs/autx-08-common-e2e.md` への
  note 追記。

### Out of Scope

- `crates/synvelis-automation/**` の src/test/manifest 変更、
  `GatewayJobLane` job 観測 leg、live catalog producer、
  dispatch-hop consumer、CLI binary cross-check、MCP
  `proposal.apply` idempotency、real-OS/EXT-L3、docs index、
  `*-msrv`/`*-full`/3OS lane。

## Components and Boundaries

- `live_lane(with_authority: bool) -> GatewayStepLane`:
  `compose(None)` で fresh `GatewayStack` を得て、
  `s.gw` を `Arc::new(Mutex::new(..))` に包み（lane は
  dispatch 毎に lock する設計 — coordinator 呼出し間に
  gateway lock を保持しない pull-based 契約と整合）、
  `LocalCliAdapter::new(AdapterId::new(CLI_ADAPTER_RAW),
  MemoryCliSessionStore, s.clock)` を `Arc` 化、
  `s.ports.clone()` を `Arc<dyn CommitPort>` へ unsize
  coerce（`GatePorts` は `CommitPort` impl 済み —
  `operation_result` の reconcile seam）、`s.jobs`
  （`compose(None)` では `None`）をそのまま渡して
  `GatewayStepLane::new` を呼ぶ。`with_authority` が true
  のときのみ `issue_agent_authority(&s.ports)` で `t1`/`g1`
  を発行する — fail-closed 検証では発行しない。
- `live_step_call() -> StepCall`: Leg B の論理操作を
  `StepCall` として構築 — `capability:
  VersionedRef::new(PREPARE_CAPABILITY, 1, 0)`、`args:
  note_add_args()`（`{"operation": {"capability":
  "synvelis.midi.note.add@1.0", ...}}` shape を verbatim
  再利用 — `step_wire` は target なし・descriptor 由来
  schema・`expected_revision`→CAS consistency を wire へ
  写す）、`target: None`、`expected_revision:
  Some(1)`、`idempotency_key: IdempotencyKey::new(
  "autx08-live-1")`、`policy_epoch: None`、`principal:
  agent_principal()`、`project_id: project().to_tagged()`、
  `task: Some(task_ref("t1"))`、`grant: Some(grant_ref(
  "g1"))`。
- 4 test は全て `live_lane`/`live_step_call` と既存 Leg B
  leg helper（`gui_leg`/`cli_leg`/`mcp_prepare_call`/
  `wire`/`expected`/`complete`/`mcp_structured`）の組合せ
  のみで構成し、authority ロジックを複製しない（Rule
  AUTX-E2E-NOAUTORITY）。

## Data and Control Flow

1. `live_lane(true)`: `compose(None)` → `issue_agent_authority`
   → `GatewayStepLane::new(Arc<Mutex<Gateway>>, cli,
   ports-as-CommitPort, None)`。
2. `lane.invoke(&live_step_call())`: `attest`（agent
   principal → `cli.caller_delegated` の delegated agent
   caller mint）→ `request_id`（`wf-req-{n:016x}`）→
   `step_wire`（private — `StepCall` を wire JSON へ lowering、
   `synvelis.operation.batch` のみ prepare wrap 変換）→
   `gateway.handle`（parse → bind → invoke → authorize →
   commit — Leg B と同一 pipeline）。
3. assert: 返り値の `GatewayOutcome<Value>`/`ErrorEnvelope`/
   `DuplicateRequest{original}`/`OperationObservation` を
   Leg B surface の同一観測値と比較。

## Non-Functional Constraints

- fake adapter・scripted gateway・fixture descriptor で
  契約を代替しない — production `GatewayStepLane` と実
  `Gateway`/`CommitGate`/journal/durable store のみ。
- `Human` principal の `attest` fail-closed
  （`GatewayError::Authentication`）は adapter contract
  どおり別経路 — AI leg は delegated agent 経路のみを
  検証し、human principal の振る舞いを本 test で主張しない。
- request_id 系の surface 由来メタは同一性の対象外
  （Leg B convention — `wf-req-*` は lane が invoke 毎に
  mint する production retry shape）。
- headless Pass は real-OS/EXT-L3 Pass に昇格しない。

## Risks and Trade-offs

- `autx08_e2e.rs` への追加で file が ~730 行に肥大化する
  リスク → sibling file 案は helper 複製・`pub(crate)` 化・
  lease 外変更リスクが大きく、parity の「同一論理操作」
  保証を 1 file に集約する方を優先（discovery Alternatives
  参照）。
- `operation_result` の reconcile は stage-1
  （`gateway.operation_outcome`）で prepare outcome が
  settle するため `jobs: None` で影響しない — job lane
  parity は本 slice 対象外として Deferred に記録。
- `DuplicateRequest.original` の operation id 一致は
  per-instance counter が compose recipe 同一なら揃う
  Leg B の根拠に依存 — live lane の `invoke` は gateway
  call を正確に 1 回だけ産む（prepare のみ、job submit
  なし）ため counter 系列は Leg B CLI leg と一致する。

## Deferred Design Concerns

- `GatewayJobLane` job 観測 parity・`CancelRequested`
  非昇格の honest-cancel semantics → #265 coordinator
  接続後の live workflow leg。
- `step_wire` ↔ `synvelis-cli build_request`/`claim_for`
  ↔ `lower` の 3-producer wire field cross-check →
  #264 mount wiring + CLI binary E2E lane。
- live-registry → `syn.autx.catalog` producer・
  `lower` → gateway dispatch-hop consumer → #264。

## Verification Notes

- [x] `cargo test -p synvelis-workspace --locked --test
  autx08_e2e`: 8/8 pass（新規 4 test 全て green、Leg B
  4 test 非回帰）
- [x] `mise run fmt-check` / `check synvelis-workspace` /
  `lint synvelis-workspace` / `test synvelis-workspace` /
  `check-headless-deps` / `test-rt` /
  `prek run --all-files`: 全て exit 0（`evidence/sv267/
  verification.md` に実測値）
- [ ] merge 後 main SHA の必須 CI 証拠・real-OS
  GUI-L2/EXT-L3・live-catalog/dispatch-hop consumer:
  Coordinator/後続 slice（NotRun/Deferred のまま）
