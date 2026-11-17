---
id: "IMPL-034Z9b1GPiML5a5vQBbZef"
type: "impl"
status: "in-progress"
title: "issue-518-job-system — production render job system 配線"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/tasks/issue-518-job-system.md"
    - "docs/test-specs/issue-518-job-system.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-render/src/render_job/scheduler.rs"
      - type: "file"
        path: "crates/synvelis-render/src/render_job/cost.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/render_dispatch.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/job_system.rs"
      - type: "file"
        path: "docs/discovery/issue-518-job-system.md"
      - type: "file"
        path: "docs/specs/issue-518-job-system.md"
      - type: "file"
        path: "docs/designs/issue-518-job-system.md"
      - type: "file"
        path: "docs/plans/issue-518-job-system.md"
      - type: "file"
        path: "docs/tasks/issue-518-job-system.md"
      - type: "file"
        path: "docs/test-specs/issue-518-job-system.md"
      - type: "file"
        path: "docs/adr/issue-518-job-system.md"
      - type: "file"
        path: "docs/impl/ir/issue-518-job-system.md"
      - type: "file"
        path: "docs/impl/exp/issue-518-job-system.md"
      - type: "file"
        path: "docs/impl/exp/issue-518-job-system-exp.jsonl"
      - type: "file"
        path: "evidence/sv518/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-render/src/render_job/orchestrator.rs"
      - type: "file"
        path: "crates/synvelis-render/src/render_job/model.rs"
      - type: "file"
        path: "crates/synvelis-render/src/render_job/mod.rs"
      - type: "file"
        path: "crates/synvelis-render/src/policy/trigger.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/rcp03_render_policy.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb01_variant.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/host.rs"
      - type: "file"
        path: "crates/synvelis-cli/Cargo.toml"
      - type: "file"
        path: "crates/mcp-poc/src/main.rs"
      - type: "file"
        path: "crates/mcp-poc/Cargo.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-518-job-system.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/issue-518-job-system.md"
  derived-by: []
  verifies:
    - "docs/test-specs/issue-518-job-system.md"
  verified-by: []
  references:
    - "docs/impl/exp/issue-518-job-system-exp.jsonl"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-518-job-system.md"
    rejected: []
---

# issue-518-job-system — implementation record

## Summary

synvelis#518 の production render-job system 配線を実装した。
`synvelis-render` に bounded async non-RT scheduler
（`ruviform_exec::OfflineJob` worker + bounded `sync_channel`、
`JobOrchestrator` を single-writer として内包）を追加し、
orchestrator に scheduler quantum（`drive_step` /
`DriveOutcome::Progress` / `resume_crashed`）を導入。
deterministic cost model で `JobCostEstimate::default()` stub を
置換し、project render preference を schema v15 として永続化。
`synvelis-workspace` に `RenderDispatch` adapter と
`JobSystem` composition root を追加し、CLI / MCP PoC の
`GateDeps.render` を headless job system に配線した。
fail-closed が既定: executor/settings/policy/demand mapping が
無い場合は typed failure/rejection/inert であり、render 成功を
捏造しない。

## Context

- Issue: `scarecrow173/synvelis#518`（production job system 配線:
  async scheduler・preference 永続化・cost model）
- Task: `docs/tasks/issue-518-job-system.md`
- Test spec: `docs/test-specs/issue-518-job-system.md`
- ADR: `docs/adr/issue-518-job-system.md`（executor = OfflineJob +
  bounded queue、JobExecutor 非採用の決定を記録）
- 前提: synvelis#517（render/branch pipeline 本番配線、
  `RenderDispatch` port と `UnavailableRenderDispatch` が済）

## Implementation

### Changes

#### synvelis-render

- `src/render_job/orchestrator.rs`:
  - `DriveOutcome::Progress` 追加。`drive`/`run` は `drive_step`
    の loop として保存。
  - `drive_step` — scheduler quantum。1 stage 遷移または
    1 demand render のみ進める。`crashed` job は即座に
    `DriveOutcome::Crashed` を返し前進しない。
  - `resume_crashed` — crash flag を解除し
    `ReceiptKind::Resumed` を記録するが、job 全体を同期駆動
    しない（async resume 契約）。
  - cost model 統合: submit 時に caller-declared estimate と
    model floor を merge してから budget/grant gate へ。
- `src/render_job/scheduler.rs`（新規）: `JobScheduler` —
  `OfflineJob` worker thread が `JobOrchestrator` を所有。
  command は bounded `sync_channel`（既定 cap 64）経由で
  `SchedulerError::QueueFull` typed rejection。
  round-robin candidate 選択で quantum を回し、cancel/retry/
  grant update が demand 間に interleave する。
  fail-closed default ports（executor 無し → typed permanent
  unsupported、settings 無し → typed unknown dependency、
  policy 未宣言 → built-in 解決、scope→demand mapping 無し →
  inert）。crash した job は自動再開せず明示 `resume` のみ。
  `wait_terminal` は `QueueFull` を transient として retry。
- `src/render_job/cost.rs`（新規）: deterministic
  `estimate_cost`（recipe・scope・measured/derived inputs）と
  `merge_estimate`（要素ごと max で floor 合成）。
- `src/render_job/model.rs`, `src/render_job/mod.rs`,
  `src/policy/trigger.rs`: dispatch args / trigger / event
  contract の serde derive 追加（外部タグ・kebab-case）。

#### synvelis-project

- `src/project.rs`: `render_policy` optional field
  （serde default）+ inline `#[cfg(test)]` round-trip test。
- `src/migration.rs`: v14→v15 identity migration step。
- `src/store.rs`: `SCHEMA_VERSION` 15。
- `src/lib.rs`: re-export 調整。
- `tests/cb01_variant.rs`, `tests/md_a.rs`,
  `tests/tim_02_smf.rs`: `SCHEMA_VERSION` pin `14`→`15`
  （coordinator 承認 msg_3e7143e90682 / msg_77da6b863ccb。
  pin 行のみ、テスト名・provenance 不変）。

#### synvelis-workspace

- `src/render_dispatch.rs`（新規）: `JobScheduler` を
  `Mutex` で包み `RenderDispatch` を実装する adapter
  （`OfflineJob` は `!Sync` のため）。dispatch args を
  scheduler command に変換し、typed outcome を返す。
- `src/job_system.rs`（新規）: `JobSystem::headless()`
  composition root — default ports + scheduler 生成。
- `src/lib.rs`: 承認された 2 行
  （`pub mod job_system;` / `pub mod render_dispatch;`）のみ追加。

#### consumers

- `synvelis-cli/src/host.rs`: headless `GateDeps.render` を
  `JobSystem::headless().dispatch()` に置換。
- `synvelis-cli/Cargo.toml`, `crates/mcp-poc/Cargo.toml`:
  `synvelis-workspace` path dependency 追加（coordinator 承認、
  PR 本文に記録）。
- `mcp-poc/src/main.rs`: 同様に headless dispatch を配線。

#### tests

- `crates/synvelis-render/tests/rcp03_render_policy.rs`:
  async regression pin 9 件追加（pol01 mixed-trigger
  coalescing・duplicate intent idempotency、pol06
  concurrency quota・grant-update wake・retryable retry・
  explicit crash resume・queued cancel typed receipt、
  bounded queue `QueueFull`、headless fail-closed）。
  `GateExecutor`（gate 付き executor）と `Arc<AtomicUsize>`
  counter を追加。

### Decisions During Implementation

- **Executor**: `JobExecutor`（ruviform#744）ではなく
  `OfflineJob` + bounded queue を採用。`JobOrchestrator` が
  既に single-writer scheduling authority であり、per-job
  executor primitive は責務を重複させる（ADR 記録）。
- **Quantum 分割**: `drive` を `drive_step` に分解し scheduler
  が 1 quantum ずつ駆動。既存 sync API（`drive`/`run`）の
  挙動は Progress loop で保存。
- **Crash 契約**: crashed job を worker が自動再選択しない
  よう orchestrator 境界（`drive_step` の `Crashed` 返却 +
  `pick_candidate` 除外）と `resume_crashed` で修正。
  scheduler `Resume` は `resume_crashed` → `Progress` で
  通常 quantum に戻す。
- **Cost model**: placeholder `JobCostEstimate::default()` を
  deterministic model floor で置換。caller estimate との
  merge は要素ごと max — caller が低く申告しても model floor
  を下回れない（budget gate の fail-closed 性を維持）。
- **Thread-safety**: `OfflineJob` の `Receiver` 保持により
  `!Sync` → workspace adapter で `Mutex` 包装。
- **Schema bump**: render preference 追加に伴い v14→v15。
  既存 migration pattern（serde-default field + identity
  step + version increment）に従う。

## Related Experiments

### Adopted

- `docs/impl/exp/issue-518-job-system.md`
  （`issue-518-job-system-exp.jsonl` のイベントログ付き）:
  executor 選択・quantum 分割・crash-resume 欠陥修正・
  `call()` ブロッキングと test helper-thread pattern・
  `QueueFull` transient 扱い・cost model merge・
  `!Sync` Mutex 包装・schema v15 pin 更新の各決定を記録。

### Rejected

- なし。

## Validation

補助証拠（canonical = mise、`evidence/sv518/verification.md`
に記録）:

- `cargo test -p synvelis-render`: 全 suite pass
  （rcp03_render_policy 30 件 — async pin 9 件含む、
  l2_e2e 17、recipe 11、artifact 20、gate 7、sf03 2、
  plugin-state 5、e2e 5、doc test）。
- `cargo test -p synvelis-project`: 全 suite pass
  （inline render_policy round-trip 含む、v15 pin 3 件更新後）。
- `cargo test -p synvelis-workspace`: 全 suite pass。
- `cargo check` workspace: synvelis-render / synvelis-workspace /
  synvelis-cli / mcp-poc 全て clean。

mise 正規検証（fmt-check / check / lint / test × 対象 package、
prek run --all-files）と review-protocol は PR 前に実施。

## Risks

- 実 render backend（`RenderExecutor`・settings・demand
  resolver）の production 実装は未存在 — headless host は
  fail-closed typed rejection を返すのみで、render 成功を
  主張しない（仕様どおり）。backend 接続は別 Issue 領域。
- scheduler は単一 worker thread — 大量 job の throughput は
  quantum 設計次第。現状の bounded queue + round-robin で
  要件（非 RT・cooperative）は満たす。
- `synvelis-project/tests` の schema pin 3 件は coordinator
  承認済みだが、将来の schema bump でも同 pin 更新が必要
  （意図された fail-closed canary）。

## Follow-ups

- 実 `RenderExecutor` / settings / demand mapping の
  production 実装と `JobSystem` への注入（別 slice）。
- docs index（`docs/**/README.md`、`docs/impl/*/README.md`）
  の再生成は Coordinator 統合時に実施。
- `JobExecutor`（ruviform#744）の将来採用検討は orchestrator
  の scheduling authority 設計が変わる場合に限る。
