---
id: "EXP-sv518hI9jK0lM5nO7pQ8"
type: "experiment"
status: "completed"
title: "issue-518-job-system — 実験記録"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/impl/ir/issue-518-job-system.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-518-job-system.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/exp/issue-518-job-system-exp.jsonl"
  defers: []
  deferred-by: []
---

# issue-518-job-system — experiment log

詳細イベントは `issue-518-job-system-exp.jsonl` を参照。

## E1: executor primitive の選定

`ruviform_exec::JobExecutor`（ruviform#744）の直接採用と
`OfflineJob` + bounded queue を比較。`JobOrchestrator` が
既に single-writer の scheduling authority であり、stage
遷移・budget/grant gate・receipt 記録を一箇所で所有する。
per-job executor primitive を被せると scheduling 責務が
二重化するため、worker thread 1 本 + bounded command queue
の actor 型 `JobScheduler` を採用。JobExecutor 非採用は
ADR に記録。

## E2: quantum 分割の設計

scheduler が `drive()` を呼ぶと 1 job の全 demand render が
終わるまで worker が専有され、cancel/retry/grant update が
飢餓する。`drive_step`（1 stage 遷移 or 1 demand render）を
quantum とし、round-robin で candidate を選ぶ構成に分割。
既存 sync API は `drive_step` の Progress loop として保存し、
呼出側の契約を壊さない。

## E3: crash job の自動再開欠陥（実装中に発見・修正）

初版では crashed job が `state == Rendering` +
`crashed == true` のまま残り、worker の candidate picker が
それを再選択して `drive_step` を回し続けた — 明示
`resume` を要求する契約違反。orchestrator 境界で修正:
`drive_step` が crashed job に `DriveOutcome::Crashed` を
返し、`pick_candidate` が crashed を除外、
`resume_crashed` が flag 解除 + `ReceiptKind::Resumed`
記録のみ行う（同期 drive しない）。async pin
`async_pol06_crash_requires_explicit_resume` で固定。

## E4: `call()` の reply ブロッキングと test の deadlock

`JobScheduler::call()` は `reply_rx.recv()` で同期待機する。
worker が gate 付き executor 内で park している間に送った
command は release まで reply できず、reply を await して
から gate を開く test は deadlock した。scheduler の欠陥
ではなく test 記述の問題 — worker が park し得る区間で発行
する command は helper thread から送り、gate release 後に
reply を await する形に修正。

## E5: `QueueFull` の transient 扱い

bounded queue の admission 拒否 `SchedulerError::QueueFull`
を `wait_terminal` が fatal 扱いしていた。queue-full は
worker 死ではなく backpressure — deadline まで retry する
よう修正。`QueueFull` 自体の typed rejection は
`async_scheduler_bounded_queue_reports_queue_full` で pin。

## E6: cost model の merge 意味論

`JobCostEstimate::default()` の placeholder を deterministic
`estimate_cost`（recipe・scope・measured inputs 由来）に
置換。caller-declared estimate との合成は要素ごと max
（model floor）— caller が過少申告しても floor を下回れず、
budget/grant gate の fail-closed 性を維持する。明示 estimate
を渡す既存 caller 経路は温存。

## E7: `OfflineJob` の `!Sync` と adapter 包装

`ruviform_exec::OfflineJob` は `mpsc::Receiver` を保持し
`Sync` でない。`RenderDispatch` は `Send + Sync` を要求
されるため、workspace adapter が `JobScheduler` 状態を
`Mutex` で包む構成にした。`JobSystem::headless()` が
composition root として default fail-closed ports +
scheduler を生成し、CLI / MCP PoC の `GateDeps.render`
に配線。

## E8: schema v14→v15 と leased test pin

`render_policy` 永続化に伴い `SCHEMA_VERSION` を 15 に
bump（serde-default field + identity migration、既存
pattern 準拠）。`crates/synvelis-project/tests` 内の
`SCHEMA_VERSION == 14` pin 3 件（cb01_variant.rs /
md_a.rs / tim_02_smf.rs）が fail したため、coordinator
承認（msg_3e7143e90682 / msg_77da6b863ccb）のもと pin 行
のみ 14→15 に更新。テスト名・snapshot provenance は不変 —
pin は schema bump のたびに意識的更新を強制する canary
として機能する。
