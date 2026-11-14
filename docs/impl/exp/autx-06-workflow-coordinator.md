---
id: "EXP-Kd4mQ9wX2zB7nR5tY8uJ1c"
type: "experiment"
status: "completed"
title: "AUTX-06 Workflow Coordinator — headless coordinator・Run/Step FSM・fault fixture の試行"
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
  implemented-by:
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
  depends-on:
    - "docs/automation/design-workflow-run.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-06-workflow-coordinator.md"
    - "evidence/sv265/autx-06-workflow-coordinator-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-automation/src/coordinator.rs"
    - "crates/synvelis-automation/src/run.rs"
    - "crates/synvelis-automation/tests/coordinator.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-06 Workflow Coordinator — headless coordinator・Run/Step FSM・fault fixture の試行

## 問題設定

Issue #265 は `design-workflow-run.md` の canonical 契約に沿った
headless Workflow Coordinator を要求する。設計上の不確実点は
(a) `ExecutablePlan` が step AST を保持しないため run 駆動に
`WorkflowDefinition` を併用する構成、(b) journal/replay を
live session と cold replay でどう単一経路にするか、
(c) Gateway/Job の正本型をどう coordinator port に写すか
（mirror 型を作らない制約）、(d) cancel/indeterminate の
honest 記録をどう journal に落とすか、の 4 点。

## 試行と結果

### E1: Run/Step FSM + journal replay の単一経路化

候補は (i) live 遷移と replay 適用を別実装にする、
(ii) `apply_entry` を共有し live session も journal 経由でのみ
状態を進める。replay が live と乖離すると resume cursor の
正当性が崩れるため (ii) を採用。`StepBegan` を lane 呼出前に
journal し、parked/indeterminate からの settle は必ず
journaled `StepBegan` readmit を経由させることで、
`Waiting→Completed` のような違法直接遷移を構造的に排除した。

**結果**: 採用。`tests/coordinator.rs` の
`settle_without_begin_is_replay_divergence`・
`illegal_run_transition_is_replay_divergence` で
replay divergence を fail-closed に確認。

### E2: Gateway 正本型の port 写像

`GatewayOutcome<Value>`/`GatewayError`/`JobPhase`/
`CancelResponse`/`RetryAdvice` をそのまま port の型に使う
（task spec が許可した path 依存）。`StepLane::invoke` は
`Box<ErrorEnvelope>` を返し large-Err lint を回避。
`DuplicateRequest` は `operation_result` 照合で再実行せず、
`Indeterminate`/`ReconcileState` は run を park、
`RevisionConflict` は correction advice 付き `Failed`。

**結果**: 採用。`lane_indeterminate_error_parks_then_
reconciles_same_key`・`revision_conflict_fails_with_
correction_advice` で確認。

### E3: cancel の honest 記録と settle race

`CancelResponse` 4 variant をそのまま journal し、
`request_cancel` は pending operation ごとに 1 回だけ送信
（`cancel_issued`）。cancel が間に合わず正本が完了した race
では completed 証拠を記録し「cancelled」と偽らない。
全 authoritative work が settle し pending が残らない場合
のみ `Completed`。

**結果**: 採用。`cancel_requests_once_and_provider_truth_
settles`・`cancel_settled_race_records_actual_phase` で確認。

### E4: `start_run` の admission/execution 分離

当初 test が `start_run` 直後の実行を期待したが、契約上
`start_run` は `Created→Ready` の durable admission のみで
あり実行は `drive` が担う。test を `drive` 呼出に修正して
解消（実装は契約どおりで変更なし）。

**結果**: 採用（契約確認）。`basic_workflow_completes_with_
journaled_boundaries` 等で `drive` 経路を確認。

### E5: `retry_policy` の replay 喪失

`StepCheckpoint.retry_policy` が post-journal の in-memory
代入のみで cold replay が失う欠陥を revision-conflict test
で検出。`JournalEntry.retry_advice` を追加して digest・
replay 復元を実装。

**結果**: 採用。`revision_conflict_fails_with_correction_
advice` で `status()`（replay）経由の retry_policy 保持を
確認。

## 結論

headless coordinator の port/FSM/journal 構造は決定的 fixture
で検証可能であり、`docs/test-specs/autx-06-workflow-coordinator.md`
の全 case が Pass。live adapter・durable journal・実 OS/RT/
GUI 経路は統合スライスの Gate として残る。
