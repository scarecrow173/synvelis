---
id: "DESIGN-sv518cD4eF6gH8iJ0kL3mN5"
type: "design"
status: "approved"
title: "Issue #518 設計 — production job system 配線"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/issue-518-job-system.md"
    - "https://github.com/scarecrow173/synvelis/issues/518"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/adr/issue-518-job-system.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-517-render-branch-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-518-job-system.md"
  derived-by:
    - "docs/plans/issue-518-job-system.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #518 設計 — production job system 配線

## 方式概要

`JobOrchestrator`（単一 writer 状態機械）を所有する worker thread を
`ruviform_exec::OfflineJob` で起動し、command channel
（`std::sync::mpsc::sync_channel`・bounded）経由で投入する
actor 型 scheduler を `synvelis-render::render_job` に追加する。
worker は command を drain した後、実行可能な job を round-robin で
`drive_step`（demand 1 件分の進行 quantum）ずつ駆動する。
`run()`/`drive()` は `drive_step` の loop に再構成し、同期等価物と
非同期 scheduler が同一の状態遷移を共有する。

executor primitive: **`OfflineJob` + consumer 側 bounded queue** を
採用（JobExecutor 直接採用は不採用 — discovery 記録どおり）。

## コンポーネント

### 1. `render_job/scheduler.rs`（新規・lease 内）

```rust
pub struct SchedulerPorts {
    pub store: Box<dyn ArtifactStore + Send>,
    pub executor: Box<dyn RenderExecutor + Send>,
    pub settings: Box<dyn RenderSettingsProbe + Send>,
    pub policies: Box<dyn PolicyResolver + Send>,
    pub demands: Box<dyn ScopeDemandResolver + Send>,
    pub plugin_states: Option<Box<dyn PluginStateProbe + Send>>,
    pub external_assets: Option<Box<dyn ExternalAssetProbe + Send>>,
    pub authorizer: Option<Box<dyn DemandAuthorizer + Send>>,
    pub calibration: Option<Box<dyn CostCalibration + Send>>,
}
```

- `JobScheduler` — `SyncSender<SchedulerMsg>` handle（Clone）。
  `spawn(ports, grants, queue_capacity)` で `OfflineJob` worker を
  起動。`SchedulerMsg` は HandleReport/SubmitIntent/Cancel/Retry/
  Resume/SetGrants/Query*/Preview/Shutdown。
- `queue_capacity` 満杯 → `try_send` 失敗 → `SchedulerError::
  QueueFull`（caller non-blocking・typed）。
- worker loop: `try_recv` で全 command を drain → 実行可能 job を
  `drive_step` 1 quantum → `recv` で block。command は demand render
  間に必ず処理される（cancel/retry latency ≤ 1 demand）。
- `resting: BTreeSet<RenderJobKey>` — `drive_step` が Pending を
  返した job を一時除外し、他 job の進行または任意 command で
  全解除（quota pending の sibling 終了→resume を保証・busy-spin
  防止）。
- `JobView` — `JobRecord` の Serialize 可能 snapshot DTO。
- `wait_terminal(key, timeout)` — poll helper（test/adapter 用）。
- fail-closed port 既定: `UnavailableExecutor`
  （`RenderExecutor` impl → 常に typed `WorkerFailure`）・
  `UnavailableSettings`（`resolve` → None）・`NoDeclaredPolicies`・
  `NoScopeDemands`・`NoCostCalibration` — headless host の最小
  honest port。`JobScheduler::headless()` は `InMemoryArtifactStore`
  + 上記既定 + `LocalGrants::cpu_only()` を束ねる。

### 2. `render_job/orchestrator.rs` 変更（lease 内）

- `drive_step(&mut self, key: &RenderJobKey) -> DriveOutcome` を
  `drive()` の 1 quantum 分として新設。`drive()`/`run()` は
  `drive_step` の loop に再構成（既存同期契約・test 不変）。
- `DriveOutcome::Yield` 追加? — いや、`drive_step` は
  `DriveOutcome` を返し、Pending=要再試行、Terminal=終了、
  Crashed/Unknown=異常。drive() loop は Pending 以外で継続/終了。
  （Pending は drive() では即 return — 現行契約と同一）
- `inspect_share` を `pub(crate)` 化（scheduler の ShareGate 経路用）。
- cost model 統合: `submit()` で `model_estimate` を算出し
  `effective = max(caller, model)`（要素別）→ `record.estimate`・
  `preflight_reject`・`pending_gate` は effective を使用。
  caller-declared は締める方向のみ有効。

### 3. `render_job/cost.rs`（新規・lease 内）

- `pub trait CostCalibration` — `observed(&self, recipe_key:
  &RecipeKey) -> Option<JobCostEstimate>`。実測由来の seam。
  既定 `NoCostCalibration`。
- `pub(crate) fn model_estimate(...) -> JobCostEstimate`:
  - **storage_bytes**: 各 demand の `frames = interval.end -
    start`。resolved boundary は `recipe.output_contract.ports` の
    channels×format bytes で port 毎に算出、unresolved は
    demand.boundary.ports.len() × quality.channels ×
    format_bytes（保守的同サイズ仮定）。payload × 2（stage+
    publish）+ manifest overhead（demand 毎 4096B）。
  - **cpu_ms/wall_ms**: `Σ frames × total_channels / 1024`（初期
   係数・calibration で置換可能）。wall=cpu（並列性仮定なし・
    保守的）。
  - saturation arithmetic（u64 overflow 防止）。
- `format_bytes`: F32=4,F64=8,I16=2,I24=3,I32=4。

### 4. `render_job/model.rs` 変更

- `PinnedSnapshot`・`RenderDemand` に `Serialize, Deserialize` 追加
  （dispatch args 契約で `TriggerReport` 木を serde 化するため）。

### 5. `policy/trigger.rs` 変更

- wire 契約型に serde 追加: `SaveReceipt`/`SaveEvent`/`CommitCapability`/
  `CommitReceipt`/`CommitEvent`/`ShareRequest`/`DemandProposal`/
  `TriggerEvent`/`TriggerReport`。enum は
  `#[serde(tag = "kind", rename_all = "kebab-case")]`。
  内部型（`RoutedTrigger`/`JobIntent`/`TriggerOutcome`/
  `InertReason` 等）は serde 不付与。

### 6. `synvelis-project` 永続化（AC2・承認済み4ファイル）

- `project.rs`: `pub render_policy: Option<serde_json::Value>`
  field 追加（`#[serde(default)]`・`skip_serializing_if`）。
  値は render 側 `PolicyPatch` の JSON — project は render 型を
  名指せない（循環依存）ため opaque Value で保持し、解釈は
  render の `PolicyPatch` serde が担う。project 側 accessor
  `render_policy_patch()` は生 `&Value` を返すのみ。
- `store.rs`: `SCHEMA_VERSION` 14→15。
- `migration.rs`: `migrate_v14_render_policy`（identity step —
  `#[serde(default)]` で旧 body は None に deserialize）。
- `lib.rs`: 必要なら `render_policy` 関連の re-export。
- 検証: `project.rs` 内 `#[cfg(test)]`（serde round-trip・旧
  version body → migrate → render_policy=None）。

### 7. `synvelis-workspace`（lease 内 3ファイル + Cargo.toml）

- `job_system.rs`（新規）: `JobSystem` — `SchedulerPorts` 組立 +
  `JobScheduler` 所有 + `dispatch() -> Arc<dyn RenderDispatch +
  Send + Sync>`。`JobSystem::headless()` = fail-closed 既定 ports +
  `InMemoryArtifactStore` + `cpu_only` grants。
- `render_dispatch.rs`（新規）: `WorkspaceRenderDispatch` —
  `synvelis_gateway::variant::RenderDispatch` 実装。
  - `synvelis.render.request`: args → `TriggerReport` decode →
    `scheduler.handle_report` → outcome 写像。
  - `synvelis.render.cancel`/`retry`: key decode → 制御 →
    Dispatched/Pending。
  - `synvelis.render.inspect`: job key → `JobView` → JSON。
  - `synvelis.render.plan`: args → route preview（副作用最小の
    `Preview` command）。
  - その他 action → `Unavailable`（typed・理由明記）。
  - `Dispatched`/`Pending`/`Unavailable`/`Failed` の determined
    rejection 維持 — render success 捏造なし。
- `lib.rs`: `pub mod render_dispatch; pub mod job_system;` のみ。
- `Cargo.toml`: `synvelis-render`・`serde_json` dep 確認/追加。

### 8. host wiring（cli/mcp-poc・承認済み dep）

- `synvelis-cli/Cargo.toml`・`mcp-poc/Cargo.toml`:
  `synvelis-workspace = { path = "../synvelis-workspace" }` 追加。
- `host.rs` L618・`main.rs` L378: `UnavailableRenderDispatch` →
  `synvelis_workspace::job_system::JobSystem::headless().dispatch()`。
- `Cargo.lock` は再生成例外として許容（PR 本文明記）。

## 非同期化後の契約保存

- `JobOrchestrator` の全状態遷移・receipt・失敗行列は不変。
  scheduler は command を orchestrator に橋渡しするだけ。
- pol01 coalesce: 同一 `RenderJobKey` の 2 回目 submit は
  `Coalesced` receipt — 非同期でも同一経路。
- pol06: quota pending→SetGrants で resume、cancel→Cancelled、
  retryable failure→retry、crash→resume — 全て command 経路。
- stale completion: `PinnedSnapshot`/`RenderJobKey` は不変 —
  scheduler は新 revision の project を resolve しない限り古い
  job を新 revision に紐付けない（orchestrator の既存契約）。

## fail-closed 境界

- headless host の port 既定は全て typed reject/failure —
  settings 文書不在 → `UnknownDependency` reject、executor 不在
  → `WorkerFailure` typed failure。render success を捏造しない。
- 実 settings/policy/executor の供給は各 owning gate
  （#296 render 実行・settings 文書・project policy 消費側）に
  委ねる — 本 slice は配線 + 契約 pin。
