---
id: "DESIGN-sv517eF6gH8iJ0kL1mN3oP5"
type: "design"
status: "approved"
title: "Issue #517 設計 — render/branch pipeline 本番配線"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/517"
    - "docs/specs/issue-517-render-branch-wiring.md"
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
    - "docs/designs/variant-sets-vcs-restore.md"
    - "docs/designs/media-render-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-517-render-branch-wiring.md"
  derived-by:
    - "docs/plans/issue-517-render-branch-wiring.md"
    - "docs/adr/issue-517-render-branch-wiring.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-517-render-branch-wiring.md"
  references:
    - "docs/designs/variant-sets-vcs-restore.md"
  defers: []
  deferred-by: []
---

# Issue #517 設計 — render/branch pipeline 本番配線

## 全体構造

```
synvelis-project                synvelis-gateway              consumers
─────────────────               ─────────────────             ─────────
DirectoryBranchContentStore ──► VariantPorts::standard(root) ──► host.rs /
  <root>/branch-content/<hex>    content + JournaledRuntime    mcp-poc
                                 Publish(<root>/runtime-
                                 publish)

JournaledRuntimePublish ──────► adopt の runtime evidence
  <dir>/<request-digest>.json    Pending/Failed typed — never
                                 Published without runtime

PreparedMutation::Render ──────► prepare_engine render_ops ────► synvelis.render.*
  RenderIntent{action,args}      capability floor parity          capability ids =
                                 (capability/data-access)         render_ui
                                       │                          ACTION_RENDER_*
                                       ▼
                              CommitGate render arm ────────────► RenderDispatch port
                                revision fence + typed           (host-injected;
                                Dispatched/Pending→commit        CLI/MCP =
                                Unavailable/Failed→reject        UnavailableRender)

restore_as_copy ───────────────► RestorePorts.branch_content ──► Captured digest fork-
  snapshot variant_sets            read port → staging            copy → dest
  Captured digests                 branch-content/<hex>           branch-content/
                                   + receipt.branch_content       + receipt verdict
```

## 設計判断

### D1: DirectoryBranchContentStore（AC-1）

- `crates/synvelis-project/src/variant/store.rs` に追加。
  `<root>/<sha256-hex>` に 1 payload 1 file。
- `put`: digest 検証 → `StagedFile` + `PublishMode::CreateNew` +
  `Durability::FileAndDirectorySync`。既存ファイルは readback で
  digest 再検証（fail-closed）。`CreateNew` 競合時は readback で
  digest 一致のみ受理（冪等）。
- `get`: 不在は `None`、I/O 失敗は `VariantError::Store`。
  bytes の digest 検証は `decode_candidate` が読み取り毎に実施
  （store は bytes の正直な保管、整合性は参照側契約）。
- GC/削除なし — orphan payload は無害（CAS 参照のみ authoritative）。
  保持期間は host の project lifecycle 責務。
- `VariantPorts::standard(Some(root))` → `root/branch-content` +
  `root/runtime-publish`。`None` → 従来の in-memory + Noop
  （project-less discovery/test 経路を維持）。

### D2: JournaledRuntimePublish（AC-2）

- `crates/synvelis-project/src/variant/runtime.rs`。request を
  JSON record `<journal_dir>/<request-digest>.json` に staged
  publish（`PublishMode::Replace`、FileAndDirectorySync）。
- 常に typed `Pending` を返す — `synvelis-project` 内に live
  runtime は存在せず、activation は session engine の責務
  （RCP-BR-04: project commit ≠ runtime activation）。journal 書込
  失敗時のみ typed `Failed`。`Published` は決して返さない。
- engine/session 到達境界は sv#514 の SessionRoutingEngine/
  RoutingEngine 契約と照合 — 本 slice は project 側の正直な
  journal + typed evidence まで（`crates/synvelis-app/` 非接触）。

### D3: synvelis.render.* 登録（AC-3）

- `variant.rs`: `RENDER_*_ID` 9 定数 + `RenderAction`（
  `ACTION_RENDER_*` 語彙と 1:1）+ `RenderIntent` +
  `RenderDispatchState`（Dispatched/Pending/Unavailable/Failed）+
  `RenderDispatch` trait + `UnavailableRenderDispatch` +
  `render_descriptors()`。
- Target は `TargetContract::None` — render job は本 slice で
  project resource ではなく、job identity は validated args 内。
  project state を mutate しない（candidate = pinned base 不変）。
- `prepare_engine.rs`: `render_ops` テーブルに 9 descriptor 登録、
  `lower_render` で `PreparedMutation::Render` へ lowering。
  batch member としては拒否（project ops のみ batch 可 —
  branch ops と同じ posture）。
- `commit.rs`: `GateDeps.render: Option<Arc<dyn RenderDispatch>>`。
  CommitGate が fence 内で dispatch。revision fence あり。
  Dispatched/Pending → committed provenance（typed evidence）。
  Unavailable/Failed → determined non-commit rejection。
  port 不在 → fail-closed reject。
- 認可 parity: descriptor の `required_permissions`/
  `data_access` は宣言的 metadata（publish/remote は
  ExternalAction + ExternalDisclosure/ProviderTransfer を宣言）。
  実効 floor は capability list + scoped effects — branch ops と
  同一機構（per-op permission の個別強制は本 codebase の既存
  契約どおり descriptor metadata に留まる）。

### D4: VCS-RESTORE-10 fork-copy（AC-4）

- `RestorePorts.branch_content: Option<&dyn
  RestoreBranchContentSource>` 追加。`BranchContentStore` への
  blanket impl で production store をそのまま渡せる。
- `materialize_branch_content`: snapshot の `variant_sets` が参照
  する全 `Captured.payload_digest` を収集 → source から read →
  digest 再検証 → staging `branch-content/<hex>` へ publish。
- port 不在・confirmed-absent → skip。receipt の
  `branch_content` report が `referenced/copied/unavailable` を
  記録 — unavailable は `CandidateUnavailable` の正直 verdict。
- transport read 失敗・digest mismatch → restore 失敗
  （absent への矯正禁止）。publish 後に dest 実在で copied を
  再計算（receipt は published reality を反映）。
- 復元 project は自身の `branch-content/` store を持つため、
  fork-copy 成功時に candidate 操作は `CandidateUnavailable` に
  ならない（unavailable 分のみ限定的に degrade）。

### D5: consumer 配線

- `VariantPorts::standard()` シグネチャを
  `standard(content_root: Option<&Path>)` に変更。
  `host.rs`: durable/read-only project boot は `Some(&ctx.canonical)`、
  discovery は `None`。`mcp-poc`: `Some(&store_dir)`。
- `GateDeps.render`: CLI/MCP は `UnavailableRenderDispatch`（
  headless の正直な wiring — render backend 未接続を明示）。

## 代替案と却下

- gateway→synvelis-render 依存: Cargo.toml が lease 外 + 依存方向
  違反（gateway は project/automation/script のみ）。host 注入
  port で閉じる方が VariantPorts と対称で lease 内完結。
- JournaledRuntimePublish が `Published` を返す設計: runtime
  activation 未確認で成功を名乗るのは last-good 契約違反。
  Pending が正直。
- restore で unavailable を restore 失敗にする設計: payload 不在は
  bundle 整合性の問題ではなく per-reference verdict — receipt に
  記録して restore 自体は成功させるのが VCS-RESTORE-10 の意図。
