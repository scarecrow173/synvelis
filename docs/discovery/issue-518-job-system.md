---
id: "DISC-sv518aB2cD4eF6gH8iJ0kL2"
type: "discovery"
status: "resolved"
title: "Issue #518 discovery — production job system 配線の残件調査"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/518"
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
  related:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/issue-517-render-branch-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-518-job-system.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #518 discovery — production job system 配線

## Issue framing

RCP-03 の render job 基盤（`JobOrchestrator`・`RenderJobKey`・
receipt・失敗行列）は headless 同期等価物として存在するが、本番の
非同期 scheduler/executor 経路・project への declared preference
永続化・実 cost モデル・`GateDeps.render` への本番 adapter 注入が
未配線。本 slice でその最小生産経路を配線する。

## 調査結果（実測）

### (a) scheduler/executor 経路

- `crates/synvelis-render/src/render_job/orchestrator.rs`:
  `drive()`（L1124）・`run()`（L1192）は「the headless equivalent of
  an async scheduler tick」と自記する同期駆動。`handle()` は
  route→submit→drive を 1 呼出で完結する。
- `JobOrchestrator<'a>` は全 port を `&'a`/`&'a mut` 借用で保持 —
  worker thread 上で `'static` port 群を所有し orchestrator を
  ローカル構築する actor 型が必要（port は `Box<dyn Trait + Send>`）。
- `JobOrchestrator`/`RenderExecutor` の使用は test のみ
  （`tests/rcp03_render_policy.rs`・`tests/rcp07_gate.rs`）— 本番
  配線なし。
- `ruviform-exec` pin `84a68f25` に `OfflineJob`/`JobScope`/
  `JobState`/`JobError` が存在（`crates/ruviform-exec/src/job.rs`）。
  synvelis 全体で `OfflineJob`/`OfflineDriver` の使用実績なし —
  本 slice が初採用。
- 本番 `RenderExecutor`/`RenderSettingsProbe`/`PolicyResolver`/
  `ScopeDemandResolver`/`DemandAuthorizer` の実装は存在しない
  （test fake のみ）。`ArtifactStore` は `InMemoryArtifactStore`
  のみ。→ headless host は fail-closed port 既定で配線し、render
  実行本体・settings 文書・宣言 policy の実供給は各 owning gate
  （#296/#294/AC2 消費側）に委ねる。

### (b) project 永続化

- `crates/synvelis-project` に render preference/PolicyPatch の
  永続化 field は存在しない（`PolicyPatch`/`PolicyLayer::
  ProjectDeclared` は render 側 typed model のみ —
  `crates/synvelis-render/src/policy/model.rs`）。
- synvelis-project → synvelis-render の依存は不可（render が
  project に依存 — 逆方向は循環）。project 側には render 型を
  名指せない serde 可能な declared-policy 表現が必要。
- migration ladder: `migration.rs` STEPS v5→v14、
  `store.rs` `SCHEMA_VERSION = 14`。serde `#[serde(default)]`
  field 追加 + identity step + `SCHEMA_VERSION = 15` が既定形
  （v11_monitoring・v13_variant_sets と同型）。
- Coordinator 承認（msg_5a6053a8ed60）: `project.rs`・
  `migration.rs`・`store.rs`・`lib.rs` の拡張を許可。
  `crates/synvelis-project/tests` は sv#511 占有のため不可 —
  検証は src 内 `#[cfg(test)]` または render tests で行う。

### (c) cost estimate stub

- trigger routing は `JobCostEstimate::default()`（全 0）を積む
  stub（orchestrator.rs L542/631/706/744/1614）。
- `preflight_reject`（L1000-1041）・`pending_gate`（L1046-1069）は
  caller-declared `intent.estimate` のみを budget/storage_quota と
  比較。
- 既存契約 test が caller estimate 依存:
  `rcp03_render_policy.rs` L1281-1310（cpu_ms=1000 vs cap=1 →
  BudgetExceeded）・L1506（storage_bytes=4096 vs quota=1 →
  InsufficientStorage pending）。→ caller-declared を下回らない
  model floor（要素別 max）で契約を保存する。

### (d) RenderDispatch 配線点

- `crates/synvelis-gateway/src/variant.rs` L1555 `RenderDispatch`
  port・`RenderIntent{action,args}`・`RenderDispatchOutcome` は
  sv#517 で main 供給済み。commit.rs L1437-1499 で fence 内
  dispatch → `Dispatched`/`Pending` は provenance commit、
  `Unavailable`/`Failed` は determined non-commit rejection。
- host 注入は `UnavailableRenderDispatch` のみ:
  `crates/synvelis-cli/src/host.rs` L618-619・
  `crates/mcp-poc/src/main.rs` L378-379。
- `synvelis-cli`/`mcp-poc` は synvelis-render/workspace に
  非依存 — adapter は gateway+render 両方を名指せる
  `synvelis-workspace`（唯一両 dep 保持・headless-clean）に置く。
  Coordinator 承認（msg_5a6053a8ed60）: 両 crate へ
  `synvelis-workspace` path dep 各1本を許可。
- `synvelis-workspace/src/lib.rs` への `pub mod render_dispatch;`
  `pub mod job_system;` 2行追加を承認（mod 宣言のみ）。

## executor primitive の採否（slice 冒頭の設計記録義務）

- **採用: `ruviform-exec::OfflineJob` + consumer 側 bounded
  queue。** JobOrchestrator は単一 writer で全 job 状態を共有する
  ため、per-job executor primitive ではなく「orchestrator を所有
  する 1 worker thread + bounded command channel」が正しい粒度。
  `OfflineJob` は worker thread の lifecycle/cancel/events を
  提供し、command queue は `sync_channel`（満杯 → typed
  QueueFull/Pending、caller を block しない）。
- **不採用: `ruviform-exec` JobExecutor 直接採用**（ruviform#744・
  OPEN・pin bump 必須・承認範囲外）。AC1 は OfflineJob+quota 経路で
  契約どおり実現可能（Issue コメント 2026-11-17 の探索結論どおり）。

## 判定

全残件とも lease + 承認済み拡張で完結可能。dispatch 可能。
