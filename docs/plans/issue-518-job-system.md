---
id: "PLAN-sv518dE5fG7hI9jK0lM6"
type: "plan"
status: "approved"
title: "Issue #518 plan — production job system 配線"
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
    - "docs/tasks/issue-518-job-system.md"
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
  derived-by:
    - "docs/tasks/issue-518-job-system.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #518 plan — production job system 配線

## ファイルマップ

### synvelis-render（lease 内）

| ファイル | 変更 |
| --- | --- |
| `src/render_job/scheduler.rs` | 新規 — `SchedulerPorts`/`JobScheduler`/`JobView`/`SchedulerError`/fail-closed 既定 |
| `src/render_job/cost.rs` | 新規 — `CostCalibration` trait・`model_estimate`・`NoCostCalibration` |
| `src/render_job/orchestrator.rs` | `drive_step` 追加・`drive()`/`run()` 再構成・`inspect_share` pub(crate)・cost model 統合・`UnavailableExecutor` 等 |
| `src/render_job/model.rs` | `PinnedSnapshot`/`RenderDemand` serde 追加 |
| `src/render_job/mod.rs` | `pub mod scheduler; pub mod cost;` + re-export |
| `src/policy/trigger.rs` | wire 契約型 serde 追加 |
| `tests/sv518_job_scheduler.rs` | 新規 — 非同期 pin suite |

### synvelis-project（承認済み4ファイル）

| ファイル | 変更 |
| --- | --- |
| `src/project.rs` | `render_policy: Option<serde_json::Value>` field + accessor + #[cfg(test)] |
| `src/migration.rs` | `migrate_v14_render_policy` identity step + STEPS 登録 |
| `src/store.rs` | `SCHEMA_VERSION` 14→15 |
| `src/lib.rs` | 必要なら re-export |

### synvelis-workspace（lease 内）

| ファイル | 変更 |
| --- | --- |
| `src/job_system.rs` | 新規 — `JobSystem` 組立 + `headless()` |
| `src/render_dispatch.rs` | 新規 — `WorkspaceRenderDispatch`（gateway port impl） |
| `src/lib.rs` | `pub mod render_dispatch; pub mod job_system;` のみ |
| `Cargo.toml` | 必要 dep 確認/追加 |

### host wiring（承認済み）

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-cli/Cargo.toml` | `synvelis-workspace` path dep |
| `crates/synvelis-cli/src/host.rs` | `UnavailableRenderDispatch` → `JobSystem::headless().dispatch()` |
| `crates/mcp-poc/Cargo.toml` | `synvelis-workspace` path dep |
| `crates/mcp-poc/src/main.rs` | 同上 |
| `Cargo.lock` | 再生成例外 |

### docs/evidence

- `docs/{discovery,specs,designs,plans,tasks,test-specs,adr,impl/ir,impl/exp}/issue-518-job-system.md` + `issue-518-job-system-exp.jsonl`
- `evidence/sv518/` — lane 別 Pass/NotRun 簿記

## 順序

1. render: model/trigger serde → orchestrator drive_step+cost → cost.rs → scheduler.rs
2. project: field+migration+store+test
3. workspace: job_system + render_dispatch + lib.rs
4. host: cli/mcp-poc wiring + Cargo.tomls + lock
5. tests: sv518 suite
6. docs impl chain + evidence
7. verify + review + PR
