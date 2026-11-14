---
id: "IMPL-034XiNO9SadjRbTGExtQtw"
type: "impl"
status: "in-progress"
title: "AUTX-06 Workflow Coordinator — Run/Step FSM・Journal・Resume・JobPort fixture"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/265"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-automation/src/run.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/eval.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/coordinator.rs"
      - type: "file"
        path: "crates/synvelis-automation/tests/coordinator.rs"
      - type: "file"
        path: "docs/impl/exp/autx-06-workflow-coordinator.md"
      - type: "file"
        path: "docs/test-specs/autx-06-workflow-coordinator.md"
      - type: "file"
        path: "evidence/sv265/autx-06-workflow-coordinator-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-automation/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-automation/src/lib.rs"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/automation/design-workflow-run.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
    - "docs/impl/ir/autx-04-atomic-macro.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/autx-06-workflow-coordinator.md"
    - "docs/test-specs/autx-06-workflow-coordinator.md"
    - "evidence/sv265/autx-06-workflow-coordinator-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-06-workflow-coordinator.md"
  references:
    - "docs/automation/design-workflow-run.md"
    - "docs/designs/execution-plan-design.md"
    - "crates/synvelis-gateway/src/job.rs"
    - "crates/synvelis-gateway/src/outcome.rs"
    - "crates/synvelis-gateway/src/id.rs"
    - "crates/synvelis-gateway/src/request.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-06-workflow-coordinator.md"
    rejected: []
---

# AUTX-06 Workflow Coordinator — Run/Step FSM・Journal・Resume・JobPort fixture

## Summary

Issue #265（AUTX-06）の headless 実装スライスとして、
`synvelis-automation` に Workflow Run 実行系を追加した。
AUTX-05 の `ExecutablePlan` + `WorkflowDefinition` を入力に、
`WorkflowRun`/`StepCheckpoint` の Replay 可能な有限状態機械、
step 境界 append-only の `RunJournal`（entry digest・dense seq・
plan digest 検証付き replay）、resume cursor、headless
`WorkflowCoordinator`（`StepLane`/`JobLane` port 経由で
Gateway/Job の正本型をそのまま駆動）、制限付き式/条件評価器、
および F-1〜F-10 fault fixture を `tests/coordinator.rs` に実装
した。`start_run` は admission（`Created→Ready` journal）のみを
行い、実行は `drive`/`cancel`/`reconcile` が担う。live
Gateway/Project/Job・GUI/MCP/CLI・実 process kill・実 OS は
統合スライス（#262/#266/#267）のスコープであり、本 slice では
Mock lane/JobPort fixture で契約を検証する。

## Context

`docs/automation/design-workflow-run.md` が本スライスの
canonical 契約であり、Run/Step 状態機械・journal/resume
cursor・冪等 3 層・F-1〜F-10 Fault/Recovery Matrix・
「Workflow が所有するもの/しないもの」境界表を固定する。
`crates/synvelis-gateway` の `job.rs`（`JobPhase`/
`CancelLink`/`CancelResponse`/`JobPort`）・`outcome.rs`
（`GatewayOutcome`/`GatewayError::Indeterminate`/
`RetryAdvice`）・`id.rs`/`request.rs`（`OperationId`/
`JobRef`/`Principal`/`IdempotencyKey`）が Job/Proposal/
Commit の一次正本であり、WorkflowRun は進行状態（run
journal・cursor・step verdict）のみを所有する。task spec の
許可に基づき `synvelis-automation → synvelis-gateway` の
path 依存を追加し、mirror 型を作らず正本型を直接利用した。
`docs/impl/ir/autx-05-workflow-dsl.md`（plan compiler）と
`docs/impl/ir/autx-04-atomic-macro.md`（macro registry）の
成果物を入力として再利用する。

## Implementation

### Changes

- `crates/synvelis-automation/src/run.rs`（新規）:
  - `RunState`/`StepState` の明示遷移表による有限状態機械。
    `Indeterminate` は証拠なしに終端へ遷移せず、reconcile 経路
    のみが抜ける。`CancelRequested→Completed` は「cancel が
    間に合わず正本が完了した」場合のみ。
  - `WorkflowRun`/`RunHeader`/`StepCheckpoint`/`StepFrame`/
    `StepPath`/`StepVerdict` — run header は plan digest・
    definition version・input digest・principal/task/grant
    参照・policy epoch・expected revision を保持し、Secret/
    Authority は型で表現不能。
  - `JournalEntry`/`JournalKind`/`JournalRefs`/`RunJournal`
    trait + `MemoryRunJournal` — step 境界 append-only、
    dense zero-based seq、entry digest（semantic payload の
    sha256）、`retry_advice` を verdict と共に journal。
  - `apply_entry`/`replay_entries` — cold replay と live
    session projection が同一コード経路を共有。plan digest
    一致・dense seq・entry digest・run/step 遷移合法性を
    検証し、torn/forged entry を `ReplayDivergence` で拒否。
    `StepBegan` は lane 呼出前に journal され、re-entry は
    必ず journaled `StepBegan` を経由する。
  - pending-job 会計: completed `async-submit` で job handle
    を追加し、`await-job` が terminal verdict に達した時のみ
    除去。`Indeterminate` は handle を保持する。
- `crates/synvelis-automation/src/eval.rs`（新規）:
  制限付き `Expr`/`Condition` 評価器。`EvalEnv` は
  input/step outputs/item/array/object/macro 定義のみを
  公開し、authority・shell・eval・script 参照は型で
  表現不能。比較は全順序の checked semantics。
- `crates/synvelis-automation/src/coordinator.rs`（新規）:
  headless `WorkflowCoordinator`。port は
  `StepLane::invoke(&StepCall) -> Result<GatewayOutcome<Value>,
  Box<ErrorEnvelope>>`・`StepLane::operation_result`・
  `JobLane::observe`・`JobLane::request_cancel` の 4 本のみで、
  coordinator は逐次 step 境界でのみ呼び、Gateway mutex/
  CommitFence/Project lock を跨いで保持しない（非 RT・
  再入なしを構造で担保）。
  - `start_run`: plan digest/入力検証 → header+journal
    `Created→Ready`。重複 `run_id` は `DuplicateRun`。
  - `drive`: `Ready→Running` 後、definition の step 列・
    `If`/`ForEachBounded` を cursor から再開して駆動。
    cursor 以降のみ実行し、確定済み step を再送しない。
  - `cancel`: run/該当 step を `CancelRequested` に journal
    し、pending job へ `request_cancel` を 1 回だけ送信
    （`cancel_issued` で再送抑止）。`CancelResponse` の
    Confirmed/Pending/Unreachable/Settled をそのまま journal
    し、settle race で完了済みの正本は completed 証拠として
    記録する。全 authoritative work が settle し pending が
    残らない場合のみ `Completed`、それ以外は `Failed`/
    `Indeterminate`。
  - `reconcile`: `Indeterminate` run のみ受理し、
    `operation_result`/`JobLane::observe` で正本を再照会。
    `Unknown` は Success に昇格しない。
  - `status`/`result`: read-only replay projection。
  - `GatewayError::DuplicateRequest` は `operation_result`
    で照合し再実行しない。`Indeterminate`/`ReconcileState`
    は run を `Indeterminate` に park。`RevisionConflict` は
    correction advice 付きで `Failed`。その他 error は
    `RetryAdvice` 付き step `Failed`。
  - step 冪等 key は deterministic step-occurrence key
    （run_id + step path + iteration）で、crash-retry の
    二重 commit/job submit を防ぐ。
  - `AwaitJob` は `JobPhase` を正本として扱い、
    Pending/Running→Waiting、Completed→結果（output schema
    検証付き）、Failed/provider 確定 Cancelled→Failed、
    Unknown→Indeterminate。
  - `ApprovalRequired`/`ProposalReady` は run を
    `Approval` に park し、self-approve/apply しない。
    re-drive での再提示は authoritative 側の状態を再照会
    する。
  - budget: max steps/max iterations/max runtime/max output
    size を fail-closed で強制し、超過は journaled `Failed`。
  - completed output が committed revision を露出する場合、
    以降の step の `expected_revision` を連鎖させる。
- `crates/synvelis-automation/Cargo.toml`: task spec 許可の
  `synvelis-gateway` path 依存を追加（正本型利用・mirror
  排除）。`Cargo.lock` は cargo 再解決で最小差分。
- `crates/synvelis-automation/tests/coordinator.rs`（新規）:
  Mock `StepLane`/`JobLane`・`MemoryRunJournal` fixture と
  17 test。F-1〜F-10 の fault matrix、idempotent
  start/drive、resume cursor、cancel once/settle race、
  indeterminate→reconcile、digest/遷移 replay divergence、
  plan identity mismatch、approval/proposal park、revision
  conflict、budget fail-closed、bounded loop、eval
  scope/reject を網羅。
- `docs/impl/exp/autx-06-workflow-coordinator.md`、
  `docs/test-specs/autx-06-workflow-coordinator.md`、
  `evidence/sv265/autx-06-workflow-coordinator-ledger.md`。

### Decisions During Implementation

- **`start_run` は admission のみ**: `Created→Ready` の
  durable record を作り、実行は `drive` が担う設計とした。
  crash-safe admission（run 存在の先行記録）と execution の
  分離は `design-workflow-run.md` の journal-first 契約に
  沿う。初期 test が `start_run` 直後に実行を期待して
  `Ready` で stall したのはこの契約どおりの挙動であり、
  test 側を `drive` 呼出に修正した。
- **`retry_advice` を journal に追加**: 当初
  `StepCheckpoint.retry_policy` は post-journal の in-memory
  代入のみで replay が失う設計だった。replay 可能性の
  invariant に反するため `JournalEntry.retry_advice` を追加し、
  digest payload に含めて `apply_step_verdict` で復元する
  よう修正した（journal schema は本 slice で新規のため
  互換性問題なし）。
- **parked/indeterminate からの settle は journaled
  `StepBegan` readmit を必須化**: `Waiting→Completed` 等の
  違法直接遷移を `settle_step` の funnel で排除し、FSM の
  正当性を journal replay と同一経路で保証する。
- **`StepLane::invoke` は `Box<ErrorEnvelope>` を返す**:
  `ErrorEnvelope` が clippy の large-Err 閾値を超えるため、
  trait 境界で box 化した。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-06-workflow-coordinator.md` —
  headless coordinator の port 設計・FSM/journal 構造・
  fault fixture 構成の試行記録。採用根拠は test-spec の
  全 case が決定的 fixture で Pass したこと。

### Rejected

- なし。

## Validation

`docs/test-specs/autx-06-workflow-coordinator.md` と
`evidence/sv265/autx-06-workflow-coordinator-ledger.md` に
記録。`mise run test synvelis-automation` で coordinator
integration 17 test を含む全 test が Pass（詳細は ledger）。

## Risks

- journal は `RunJournal` trait 抽象のため、durable
  filesystem/DB 実装は統合スライスで接続する。本 slice の
  `MemoryRunJournal` は契約検証用 fixture であり、実永続化の
  Crash Safety 受入は #267 の Gate。
- `synvelis-automation → synvelis-gateway` 依存は正本型利用の
  ために task spec が許可した最小差分。依存方向の恒久的な
  整理（adapter 層の要否）は統合スライスで再検討の余地あり。
- live Gateway/Project/Job・GUI/MCP/CLI 実経路・実 OS・
  RT lane・実 Plugin Host は NotRun/Deferred（ledger 参照）。

## Follow-ups

- 統合スライス: live `StepLane`/`JobLane` adapter を Gateway/
  JobPort へ接続し、#262/#266/#267 の GUI/MCP/CLI 経路と
  Crash Safety 受入を実施。
- durable `RunJournal` 実装（filesystem/DB）と process kill
  を跨ぐ resume 検証。
- `synvelis-render` 既存 JobPort との実結合（本 slice は
  JobPort FSM fixture で代替）。
