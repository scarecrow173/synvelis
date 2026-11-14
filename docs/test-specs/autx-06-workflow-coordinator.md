---
id: "TSPEC-Mx7rT2vQ8wN4kP6yB1dF3h"
type: "test-spec"
status: "approved"
title: "AUTX-06 Workflow Coordinator — Run/Step FSM・journal replay・cancel/resume fault matrix"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/265"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/automation/design-workflow-run.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
    - "docs/impl/exp/autx-06-workflow-coordinator.md"
    - "evidence/sv265/autx-06-workflow-coordinator-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/designs/execution-plan-design.md"
  verified-by: []
  references:
    - "evidence/sv265/autx-06-workflow-coordinator-ledger.md"
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
    - "crates/synvelis-automation/tests/coordinator.rs"
  defers: []
---

# AUTX-06 Workflow Coordinator — Run/Step FSM・journal replay・cancel/resume fault matrix

## Feature: Run/Step 有限状態機械と Replay 可能 journal

### Rule: 遷移は明示表のみ許可

Run/Step の状態遷移は `can_transition_to` の明示 (from,to)
ペア表のみ許可し、表にない遷移は `IllegalTransition` で
拒否する。`Indeterminate` は証拠なしに `Completed`/`Failed`
へ遷移せず、reconcile 経路のみが抜ける。

#### Example: 不正 run 遷移は replay divergence

- Given: journal に `Ready→Completed`（表にない遷移）の
  `RunTransition` entry がある
- When: `replay_entries` で projection を再構成する
- Then: `RunError::ReplayDivergence` で fail-closed
- Verified by: `illegal_run_transition_is_replay_divergence`

### Rule: journaled されない settle は divergence

`StepBegan` なしに `StepObserved`/`StepSettled` が現れる
journal は replay divergence。parked/indeterminate からの
settle は journaled `StepBegan` readmit を必須とする。

#### Example: begin なし settle の拒否

- Given: step checkpoint が存在しない状態で `StepSettled`
  entry のみが続く journal
- When: replay する
- Then: `ReplayDivergence`
- Verified by: `settle_without_begin_is_replay_divergence`

### Rule: torn/forged entry は digest/dense seq で検出

各 entry は semantic payload の sha256 digest を持ち、seq は
dense zero-based。digest 不一致・seq 欠落/重複は
`ReplayDivergence`。

#### Example: digest 改竄の検出

- Given: `StepBegan` entry の `detail` を改竄し digest を
  再計算しない journal
- When: replay する
- Then: `ReplayDivergence`
- Verified by: `journal_digest_mismatch_is_replay_divergence`

## Feature: plan identity と冪等 admission

### Rule: plan digest 不一致は resume を拒否

`start_run`/`drive`/`status`/`cancel`/`reconcile` はいずれも
`RunContext` の plan digest が header の `plan_digest` と
一致することを要求し、不一致は `PlanMismatch`。

#### Example: resume 時の plan 改竄拒否

- Given: `start_run` 済み run に対し digest の異なる plan で
  `status` を呼ぶ
- When: session を構成する
- Then: `RunError::PlanMismatch`
- Verified by: `plan_identity_mismatch_rejected_on_resume`

### Rule: 重複 run_id は idempotent reject

同一 `run_id` の `start_run` 再呼出は `DuplicateRun` を返し、
二重の run record を作らない。

#### Example: duplicate start

- Given: `start_run` 済みの `run_id`
- When: 同じ `run_id` で再度 `start_run` する
- Then: `RunError::DuplicateRun`、journal は不変
- Verified by: `duplicate_start_is_idempotent_rejected`

## Feature: headless coordinator の lane port 駆動

### Rule: `start_run` は admission のみ、実行は `drive`

`start_run` は `Created→Ready` の durable record のみを
journal し、step を実行しない。実行は `drive`（または
`cancel`/`reconcile`）が担う。

#### Example: basic sequential workflow

- Given: Query→Invoke→EmitResult の `ExecutablePlan`
- When: `start_run` → `drive` → `status`
- Then: run は `Completed`、journal に各 step の
  `StepBegan`/`StepSettled` 境界が dense seq で記録され、
  `result` が emit 出力を返す
- Verified by: `basic_workflow_completes_with_journaled_boundaries`

### Rule: `DuplicateRequest` は正本照合で再実行しない

lane が `GatewayError::DuplicateRequest` を返した場合、
`operation_result` で authoritative outcome を照合し、
side effect を再実行しない。

#### Example: indeterminate error 後の同一 key reconcile

- Given: lane が `Indeterminate` error を返す script
- When: `drive` → run `Indeterminate` → `reconcile` →
  `drive`
- Then: 再呼出は同一 deterministic step-occurrence
  idempotency key を使い、run は `Completed` へ収束
- Verified by: `lane_indeterminate_error_parks_then_
  reconciles_same_key`

## Feature: async submit/await と Job 正本

### Rule: `JobPhase` が await の唯一の正本

`AwaitJob` は `JobLane::observe` の `JobPhase` のみで判定
する。Pending/Running→`Waiting`、Completed→結果
（output schema 検証付き）、Failed/provider 確定
Cancelled→`Failed`、Unknown→`Indeterminate`。Unknown を
Success に昇格しない。

#### Example: async submit→await→pending retire

- Given: AsyncSubmit→AwaitJob→EmitResult の plan と
  Pending→Completed の Job fixture
- When: `drive` を 2 回呼ぶ
- Then: 1 回目は `Waiting`（pending job 1 件保持）、
  2 回目は `Completed` で pending job が除去される
- Verified by: `async_submit_await_completes_and_retires_pending_job`

#### Example: Unknown observation は証拠まで Indeterminate

- Given: await 対象 job の observe が `Unknown` を返す
- When: `drive` する
- Then: run/step は `Indeterminate` に park し、
  `reconcile` で `Completed` 証拠が得られるまで終端しない
- Verified by: `unknown_job_observation_stays_indeterminate_until_evidence`

## Feature: cancel の honest 記録

### Rule: cancel request は pending operation ごとに 1 回

`cancel` は run/step を `CancelRequested` に journal し、
pending job へ `request_cancel` を `cancel_issued` で
抑止して 1 回だけ送る。`CancelResponse` の
Confirmed/Pending/Unreachable/Settled をそのまま journal
する。

#### Example: cancel once + provider truth settle

- Given: `Waiting` run（pending job 1 件）
- When: `cancel` を 2 回呼ぶ
- Then: `request_cancel` は 1 回のみ、run は
  `CancelRequested`、provider が `Cancelled` を返せば
  step/run は `Failed` で settle
- Verified by: `cancel_requests_once_and_provider_truth_settles`

### Rule: settle race は実正本を記録する

cancel request と authoritative settle が競合した場合、
完了済みの正本は completed 証拠として記録し「cancelled」
とは記録しない。全 authoritative work が settle し pending
が残らない場合のみ `CancelRequested→Completed`。

#### Example: cancel/settle race

- Given: `Waiting` run で job が `Completed` に変化した後
  `cancel` する
- When: `cancel` → `drive`
- Then: journal は実 phase（`Settled{Completed}`/completed
  証拠）を記録し、run は `Completed` または `Failed` の
  実正本どおりに settle する
- Verified by: `cancel_settled_race_records_actual_phase`

## Feature: approval/proposal park

### Rule: `ApprovalRequired`/`ProposalReady` は park のみ

coordinator は approval/proposal outcome を受けると run を
`Approval` に park し、self-approve・apply・grant 取得を
行わない。re-drive は authoritative 側の状態を再照会する。

#### Example: approval park

- Given: lane が `ApprovalRequired` を返し続ける script
- When: `drive` を 2 回呼ぶ
- Then: run は `Approval` に留まり、lane は再提示として
  再呼出されるが self-approve しない
- Verified by: `approval_required_parks_run_without_self_approving`

#### Example: proposal park

- Given: lane が `ProposalReady` を返す
- When: `drive` する
- Then: run は `Approval` に park する
- Verified by: `proposal_ready_parks_run`

## Feature: revision conflict と budget

### Rule: `RevisionConflict` は correction advice 付き Failed

`RevisionConflict` error は retry 不能な correction が必要
な失敗として、`RetryAdvice::AfterCorrection` を journal し
run を `Failed` にする。replay（`status`）経由でも
`retry_policy` が保持される。

#### Example: revision conflict

- Given: lane が `RevisionConflict` を返す
- When: `drive` → `status`
- Then: run `Failed`、該当 checkpoint の
  `retry_policy == AfterCorrection`（replay でも保持）
- Verified by: `revision_conflict_fails_with_correction_advice`

### Rule: budget 超過は fail-closed

max steps/max iterations/max runtime/max output size の
超過は journaled `Failed` で fail-closed し、部分実行を
成功扱いしない。

#### Example: runtime budget

- Given: `max_runtime_ms` を超える経過を注入できる plan/
  fixture
- When: `drive` する
- Then: run は `Failed`、budget exceedance が journal に
  記録される
- Verified by: `runtime_budget_exceedance_fails_closed`

### Rule: bounded loop は上限内で各 item を実行

`ForEachBounded` は plan の `max_iterations` 内で各 item を
逐次実行し、cursor/iteration を journal で追跡する。

#### Example: bounded loop

- Given: 3 item の `ForEachBounded` plan
- When: `drive` する
- Then: run `Completed`、loop body step が item ごとに
  `StepBegan`/`StepSettled` を持つ
- Verified by: `bounded_loop_executes_each_item`

## Feature: 式/条件評価の scope

### Rule: eval は input/step/item/array/object/macro のみ

`Expr`/`Condition` 評価は `EvalEnv` が公開する参照のみを
解決し、authority・shell・eval・script 参照は型で
表現不能。未定義参照・型不一致は error。

#### Example: nested path 解決と scope 外 reject

- Given: input map と step output を持つ `EvalEnv`
- When: nested path 式を評価し、scope 外参照を評価する
- Then: 前者は値を返し、後者は error
- Verified by: `eval_resolves_nested_paths_and_rejects_out_of_scope_refs`

## NotRun / Deferred（本 slice 範囲外）

- live Gateway/Project/Job adapter・実 process kill・実 OS・
  RT lane・実 Plugin Host・GUI/MCP/CLI 実経路・Crash Safety
  受入 — 統合スライス（#262/#266/#267）の Gate。
  `evidence/sv265/` ledger に Case ID 付きで記録。
- YAML/Luau/Rhai 混在 Definition での実行 — Script Builder
  は #273。Source 非依存 `ExecutablePlan` が同一 FSM を
  駆動することは型構造で担保（VM stack/coroutine 非存在）。
