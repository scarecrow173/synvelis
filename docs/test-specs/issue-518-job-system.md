---
id: "TSPEC-sv518fG7hI9jK0lM5nO7"
type: "test-spec"
status: "approved"
title: "Issue #518 test spec — 非同期 scheduler・cost model・永続化の検証保証"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/issue-518-job-system.md"
    - "docs/designs/issue-518-job-system.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/issue-518-job-system.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-518-job-system.md"
    - "docs/designs/issue-518-job-system.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv518/verification.md"
  defers: []
  deferred-by: []
---

# Issue #518 test spec — 非同期 scheduler・cost model・永続化

## Feature: async scheduler（`tests/sv518_job_scheduler.rs`）

### Rule: pol01 coalesce が非同期でも保持される

- Given: 同一 `RenderSelection`+purpose+policy_intent の Manual
  TriggerReport を 2 回 submit
- When: scheduler が非同期処理
- Then: 2 回目は `Coalesced` receipt・render は 1 回のみ
- Example: `async_coalesce_identical_manual_reports`

### Rule: job が caller を block しない（AC-1）

- Given: executor が gate で render を block する fake
- When: `handle_report`/`submit_intent` を呼ぶ
- Then: submit は即 return（job は Queue/Rendering）— RT thread
  非 block の構造的 pin
- Example: `submit_returns_before_render_completes`

### Rule: bounded queue（capacity 満杯 → typed）

- Given: `queue_capacity = 1`・worker が command 処理中
- When: 2 件目の submit
- Then: `SchedulerError::QueueFull`（caller non-blocking）
- Example: `bounded_queue_full_returns_typed_error`

### Rule: pol06 cancel が非同期で効く

- Given: render 中の job（gate で block）
- When: `cancel(key)` command
- Then: job は `Cancelled` terminal・executor の cancellation
  token が set される
- Example: `async_cancel_mid_render`

### Rule: pol06 retry/crash-resume が非_async で保持

- Given: retryable failure / crash の job
- When: `retry(key)` / `resume(key)` command
- Then: orchestrator の既存 typed outcome がそのまま返る
- Example: `async_retry_and_resume`

### Rule: quota pending → SetGrants で resume（pol06）

- Given: `max_concurrent_jobs` cap で pending の job
- When: `set_grants`/他 job 終了で quota 解放
- Then: pending job が `Resumed` receipt で drive 再開
- Example: `async_quota_pending_resumes`

## Feature: cost model（`tests/sv518_job_scheduler.rs` + unit）

### Rule: model floor が caller-declared を下回らない（AC-3）

- Given: caller `estimate` が全 0・model が storage_bytes>0
- When: `storage_quota_bytes` が model 未満
- Then: `pending_gate` が `InsufficientStorage` pending
  （caller-declared のみに依存しない）
- Example: `cost_model_floor_blocks_storage_quota`

### Rule: caller-declared は締める方向に効く

- Given: caller estimate が model より大きい cpu_ms
- When: `cpu_render_budget_ms` が caller 値未満だが model 値以上
- Then: `BudgetExceeded` reject（caller の保守宣言を尊重）
- Example: `caller_estimate_tightens_budget`

### Rule: resolved/unresolved boundary で estimate が変わる

- Given: 同一 scope で resolved（port contract 既知）vs
  unresolved（port contract 不明）
- When: model_estimate 算出
- Then: unresolved は保守的同サイズ仮定で >= resolved
- Example: `cost_model_unresolved_is_conservative`

## Feature: project 永続化（`project.rs` #[cfg(test)]）

### Rule: render_policy が serde round-trip で保持される

- Given: `render_policy = Some(PolicyPatch JSON)` の project
- When: serialize → deserialize
- Then: 値が同一
- Example: `render_policy_round_trip`

### Rule: v14 body が v15 へ migrate で render_policy=None

- Given: `schema_version=14` の snapshot body（render_policy なし）
- When: `migrate_to_current`
- Then: `render_policy=None`・他 field 不変
- Example: `v14_body_migrates_render_policy_none`

## Feature: RenderDispatch adapter（workspace unit test）

### Rule: request → Dispatched/Pending・失敗は determined

- Given: `synvelis.render.request` + 有効 args
- When: adapter dispatch
- Then: `Dispatched`/`Pending`（scheduler 受理）・不正 args →
  `Failed`/`Unavailable` typed
- Example: `dispatch_request_maps_outcome`
