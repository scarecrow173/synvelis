---
id: "IMPL-sv517kL3mN4oP5qR6sT7uV8"
type: "impl"
status: "completed"
title: "issue-517-render-branch-wiring"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/tasks/issue-517-render-branch-wiring.md"
    - "docs/test-specs/issue-517-render-branch-wiring.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-517-render-branch-wiring.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/issue-517-render-branch-wiring.md"
  derived-by: []
  verifies:
    - "docs/test-specs/issue-517-render-branch-wiring.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-517-render-branch-wiring.md"
    rejected: []
---

# issue-517-render-branch-wiring — implementation record

## 実装サマリ

### synvelis-project

- `src/variant/store.rs`: `DirectoryBranchContentStore` 追加
  （content-addressed file store、StagedFile + CreateNew +
  FileAndDirectorySync、idempotent re-put は readback digest 検証、
  corruption fail-closed）。
- `src/variant/runtime.rs`（新規）: `JournaledRuntimePublish` —
  request を `<dir>/<request-digest>.json` に durable journal、
  常に typed `Pending`（Published を返さない）、journal 失敗時
  typed `Failed`。
- `src/variant/mod.rs`: 両者を re-export。
- `src/restore/mod.rs`: `RestoreBranchContentSource` trait +
  `BranchContentStore` blanket impl、`RestorePorts.branch_content`、
  `materialize_branch_content`（Captured digest 収集→read→digest
  検証→staging publish）、`branch_content_report`（dest 実在で
  copied 再計算）、`RestoreReceipt.branch_content`、
  `RestoreBranchContentReport` 型。
- `src/lib.rs`: restore 型の re-export 追加。

### synvelis-gateway

- `src/variant.rs`: `RENDER_*_ID` 9 定数、`RenderAction`（9 variant、
  `ACTION_RENDER_*` と 1:1）、`RenderIntent`、`RenderDispatchState`
  （Dispatched/Pending/Unavailable/Failed）、`RenderDispatchOutcome`、
  `RenderDispatch` trait、`UnavailableRenderDispatch`、
  `render_descriptors()`、`VariantPorts::standard(Option<&Path>)`。
- `src/prepare_engine.rs`: `PrepareEngine.render_ops` + render
  descriptor 登録 + `lower_render` → `PreparedMutation::Render`。
- `src/prepare.rs`: `PreparedMutation::Render` arm（project state
  不変 — pinned base が candidate）。
- `src/commit.rs`: `GateDeps.render: Option<Arc<dyn RenderDispatch>>`、
  Render intent の revision fence + dispatch（Dispatched/Pending →
  committed provenance、Unavailable/Failed/port 不在 → determined
  rejection）。

### consumers

- `synvelis-cli/src/host.rs`: `install_engines`/`build_gateway` に
  `variant_root` 追加。durable/read-only project boot は
  `Some(&ctx.canonical)`、discovery は `None`。`GateDeps.render` =
  `UnavailableRenderDispatch`。
- `mcp-poc/src/main.rs`: `gateway_stack` に `variant_root` 追加、
  boot は `Some(&store_dir)`、render = `UnavailableRenderDispatch`。

### tests

- `synvelis-project/tests/sv517_render_wiring.rs`（新規 6 件）:
  store persistence/corruption、journaled publish pending/failed、
  restore fork-copy/unavailable。
- `synvelis-gateway/tests/sv517_render_actions.rs`（新規 6 件）:
  capability 登録、dispatch parity、typed failure、fail-closed、
  capability floor、idempotent replay。
- 既存 `GateDeps`/`RestorePorts` literal を全更新
  （commit_gate.rs, job.rs, rig/mod.rs, vcs05/vcs06 系）。

## 検証

`cargo test` 補助証拠（canonical = mise）:
- `synvelis-project --test sv517_render_wiring`: 6/6 pass
- `synvelis-gateway --test sv517_render_actions`: 6/6 pass

mise 正規検証・prek・review-protocol は PR 前に実施し
`evidence/sv517/` に記録。
