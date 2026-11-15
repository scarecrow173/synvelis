---
id: "IMPL-034XLar3aaWdSdwjp7eeMM"
type: "impl"
status: "in-progress"
title: "AUTX-06 live adapters — GatewayStepLane/GatewayJobLane + registry projection"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/265"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-automation/src/live.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/live_registry.rs"
      - type: "file"
        path: "crates/synvelis-automation/tests/live_adapters.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/autx06_live_adapters.rs"
      - type: "file"
        path: "docs/impl/exp/autx-06-live-adapters.md"
      - type: "file"
        path: "docs/test-specs/autx-06-live-adapters.md"
      - type: "file"
        path: "evidence/sv265/autx-06-live-adapters-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-automation/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/gateway.rs"
      - type: "file"
        path: "docs/automation/design-workflow-run.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
  implemented-by: []
  depends-on:
    - "docs/automation/design-workflow-run.md"
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
    - "docs/impl/ir/autx-04-atomic-macro.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/autx-06-live-adapters.md"
    - "docs/test-specs/autx-06-live-adapters.md"
    - "evidence/sv265/autx-06-live-adapters-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-06-live-adapters.md"
  references:
    - "crates/synvelis-automation/src/coordinator.rs"
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/live_registry.rs"
    - "crates/synvelis-gateway/src/gateway.rs"
    - "crates/synvelis-gateway/src/job_engine.rs"
    - "crates/synvelis-gateway/src/cli.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-06-live-adapters.md"
    rejected: []
---

# AUTX-06 live adapters — GatewayStepLane/GatewayJobLane + registry projection

## Summary

Issue #265（AUTX-06）の統合スライスとして、headless
`WorkflowCoordinator` の 2 つの authority seam — `StepLane`
（capability invoke + operation-result 照合）と `JobLane`
（job 観測 + cancel）— の production 実装を追加した。
`GatewayStepLane`/`GatewayJobLane` は `StepCall`/job handle を
wire request に写像し `Gateway::handle` の parse → bind →
invoke 経路へ通し、typed `GatewayOutcome`/`ErrorEnvelope` を
そのまま coordinator の verdict/journal に渡す。第二正本・
独自 policy 評価・"latest" 解決は存在しない。あわせて
`Gateway::operation_outcome`（`DuplicateRequest` 照合用の
最小 read surface）と `capture_registry_snapshot`（Gateway
`CapabilityRegistry`/`SchemaRegistry` → automation
`RegistrySnapshot` の厳密版投影）を追加し、unit test
（`tests/live_adapters.rs` 14 件）と実 rig 上の headless e2e
（`tests/autx06_live_adapters.rs` 5 件）で検証した。

## Context

`docs/automation/design-workflow-run.md` が canonical 契約で
あり、「Workflow が所有するもの/所有しないもの」境界表・
冪等 3 層・F-1〜F-10 Fault/Recovery Matrix を固定する。
前 slice（`docs/impl/ir/autx-06-workflow-coordinator.md`）が
coordinator 本体と Mock lane 契約を実装し、その ledger の
N-01「live Gateway/Project/Job adapter 接続」が本 slice の
スコープである。`synvelis-automation` は既に
`synvelis-gateway` path 依存を持ち（正本型の直接利用）、
`StepLane`/`JobLane` は意図的に狭い 2 seam として設計済み
だったため、adapter はその契約の実装に留まる。

## Implementation

### Changes

- `crates/synvelis-automation/src/live.rs`（新規）:
  - `GatewayStepLane`: `Arc<Mutex<Gateway>>` +
    `Arc<LocalCliAdapter>` + `Arc<dyn CommitPort>` +
    `Option<Arc<JobEngine>>` を保持し、`invoke` ごとに
    mutex を取り wire request を構成して `gateway.handle`
    へ渡す（lock は dispatch 中のみ、step 境界を跨がない）。
  - `attest`: `Agent` principal → `caller_delegated`、
    `Human` principal → `Authentication` で fail-closed。
  - `step_wire`: registry descriptor から厳密 capability/
    schema ref・domain 妥当な consistency（Query→latest、
    ProjectMutation→expected-revision 必須、Async→
    unconditional）・target contract 適合の wire target を
    構成。`synvelis.operation.batch` は
    `synvelis.operation.prepare` の `operation` 引数へ
    lowering（batch は executor を持たない named operation）。
  - `operation_result`: `Gateway::operation_outcome` →
    `CommitPort::reconcile` → `JobEngine::record` の順で
    正本照合し、`Settled`/`Pending`/`Unknown` を返す。
    記録なしは `Unknown` — 捏造しない。
  - `GatewayJobLane`: run の (principal, project, task,
    grant) scope で `synvelis.job.get`/`synvelis.job.cancel`
    を `Gateway::handle` 経由で dispatch。`job_observation`/
    `cancel_response` は wire payload を `JobObservation`/
    `CancelResponse` へ写像し、解釈不能は `Unknown`/
    `Unreachable` に落とす。
- `crates/synvelis-automation/src/live_registry.rs`（新規）:
  `capture(gateway)` — Gateway `CapabilityRegistry` 全件 +
  `SchemaRegistry` の全宣言 schema を automation
  `RegistrySnapshot` へ投影。厳密 version・domain・
  target/effect/permission/data-access/execution/availability
  ・入出力 `SchemaNode`（再帰変換）を保持し、workflow-local
  job schema の捏造なし。投影後 `validate()` を通す。
- `crates/synvelis-gateway/src/gateway.rs`: `operation_outcome`
  read surface を追加 — keyed outcome ledger の記録済み
  `GatewayOutcome` を `OperationId` で照合する最小 API。
  ledger evict・commit/job 正本への fallback は lane 側。
- `crates/synvelis-automation/src/lib.rs`:
  `GatewayStepLane`/`GatewayJobLane`/`capture_registry_snapshot`
  を re-export。
- `crates/synvelis-automation/tests/live_adapters.rs`（新規、
  14 test）: 実 `Gateway` rig 上で wire dispatch・duplicate
  reconcile・unknown operation・job observe/cancel 全 verdict・
  human fail-closed・registry converter を検証。
- `crates/synvelis-gateway/tests/autx06_live_adapters.rs`
  （新規、5 test）: `mod rig` の実 rig（prepare/job/context/
  commit gate 全 install）上で headless coordinator を駆動。
  macro→prepare→submit→await→emit の完了 + trusted
  apply/commit、query domain、grant revoke deny、cancel
  confirmed/unreachable の honest settle、re-drive 非重複を
  実測。
- `docs/automation/design-workflow-run.md`: live adapter の
  配線契約（wire 経路・attestation・reconcile 順序・
  commit 非所有）を contract notes として追記。
- `docs/test-specs/autx-06-live-adapters.md`、
  `docs/impl/exp/autx-06-live-adapters.md`、
  `evidence/sv265/autx-06-live-adapters-ledger.md`。

### Decisions During Implementation

- **human principal は fail-closed**: `StepCall` は principal
  のみを運び verified-session credential を持たない。human
  run を agent-equivalent caller へ silent downgrade する
  代わりに `Authentication` で拒否する — AUTX-02 の
  「unattended delegated は常に Agent」契約と整合。
- **job lane は capability 経路のみ**: `JobEngine::record`
  直読みは existence-blind scope を迂回するため禁止。
  observe/cancel は `synvelis.job.get`/`job.cancel` の
  wire 経路に限定し、scope・disclosure・honest verdict を
  権威の回答に委ねる。
- **`operation_result` は 3 段照合**: keyed outcome ledger
  （成功 invoke の記録）→ `CommitPort::reconcile`
  （commit-bound apply）→ `JobEngine::record`（admitted
  submit の book 証拠）の順。ledger evict 後も commit/job
  正本が残るため `Unknown` に落ちない範囲を最大化しつつ、
  いずれにも無い operation は `Unknown` のまま。
- **commit は workflow の外**: prepare が生成する
  proposal/digest を trusted caller が apply + `CommitGate`
  で commit する e2e とし、workflow が commit 権威を持た
  ないことを実測で固定した。
- **gateway は `Arc<Mutex<Gateway>>` で共有**: e2e では rig
  所有の `Gateway` を cell へ移し、lane の per-dispatch
  lock と test 側の trusted apply が同一 authority を共有
  する。placeholder gateway は誰も invoke しない。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-06-live-adapters.md` — attestation
  fail-closed・job lane capability 限定・3 段 reconcile・
  registry 投影の試行記録。採用根拠は unit/e2e test が
  実 authority 経路で全て green だったこと。

### Rejected

- なし。

## Validation

`docs/test-specs/autx-06-live-adapters.md` と
`evidence/sv265/autx-06-live-adapters-ledger.md` に記録。
`mise run test synvelis-automation`（live_adapters 15 件含む）
と `mise run test synvelis-gateway`（autx06_live_adapters
5 件含む）が Pass。詳細は ledger の計測環境・case 対応表を
参照。

## Risks

- `Gateway::operation_outcome` は keyed outcome ledger の
  read surface であり、ledger eviction semantics や key 形式
  の変更は reconcile 正しさに影響する。gateway 側変更時は
  `live_adapters.rs` の reconcile test が先に落ちる。
- job.get/job.cancel の wire result 形状変更は lane parser
  を silent に `Unknown` 化しうる — phase/cancel 網羅
  mapping test で検出する配置。
- human principal の verified-session 委譲は現契約に無く、
  human workflow run は fail-closed のまま。credential を
  運ぶ step call 契約の拡張は別 Issue のスコープ。
- durable journal・実 OS・GUI/MCP/CLI 実経路・RT lane は
  前 slice から継続して NotRun/Deferred（ledger 参照）。
- idempotency ledger は bounded in-memory（`LEDGER_CAP`
  4096・FIFO eviction、durable 化は GW-06 の scope）。
  eviction 後の keyed op は `DuplicateRequest` を返さず
  再実行されうる — stage 1/2/3 の reconcile は post-eviction
  の honest `Unknown` までを保証し、再実行抑止は ledger
  保持期間内の best-effort。job submit は `JobEngine::record`
  の book（cap 512・same-scope eviction）が post-eviction の
  admission evidence になるが、book 側も溢れれば duplicate
  submit になりうる。review で確認済みの継承境界であり、
  本 slice では durable idempotency を新設しない。
- `operation_result` stage 2/3（`CommitPort::reconcile`・
  `JobEngine::record` fallback）は ledger eviction 後にのみ
  到達する経路で、`LEDGER_CAP` が private const のため
  runtime test では post-eviction を再現できない。stage 2
  の `Settled→Accepted` 写像は unit test で pin 済み、
  stage 3 は code review + 契約で担保（coverage gap として
  記録）。

## Follow-ups

- durable `RunJournal` + 実 process kill を跨ぐ resume
  検証（#267 Crash Safety Gate）。
- human principal の verified-session credential 委譲設計
  （別 Issue 起票候補）。
- `synvelis-render` 実 JobPort・実 provider との結合
  （stub fixture は wire/authority 契約のみを検証）。
