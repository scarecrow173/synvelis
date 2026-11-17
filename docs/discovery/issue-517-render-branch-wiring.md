---
id: "DISC-sv517aB2cD4eF6gH8iJ0kL1"
type: "discovery"
status: "resolved"
title: "Issue #517 discovery — render/branch pipeline 本番配線の残件調査"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/517"
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
    - "docs/adr/rcp-00-render-recipe.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-517-render-branch-wiring.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #517 discovery — render/branch pipeline 本番配線

## Issue framing

`VariantPorts::standard()` が `InMemoryBranchContentStore` +
`NoopRuntimePublish` を配線したままで、`synvelis.render.*` capability
が gateway pipeline に未登録、VCS-RESTORE-10 の fork-copy が未実装
という 3 つの残件を、本番配線として確定する。

## 調査結果（実測）

### (a) BranchContentStore

- `crates/synvelis-project/src/variant/store.rs` は
  `InMemoryBranchContentStore` のみ。file-backed 先例は
  `src/store.rs` の DirectoryBundleStore（immutable-name + staged
  publish）。
- `BranchContentRef::Captured` は payload digest・scope digest・
  schema version・branch revision を保持し、payload bytes は
  snapshot に埋め込まれない — store が唯一の bytes 所有者。

### (b) RuntimePublish

- `types.rs:423-473` に `RuntimePublish`/`RuntimePublishRequest`/
  `RuntimePublishEvidence`/`RuntimePublishState`（Published/Pending/
  Failed/NotRequested）が既存。実装は `NoopRuntimePublish`/
  `FakeRuntimePublish` のみ（engine.rs:71-111）。
- publish 呼出点は `engine.rs:1310` — project CAS 成功後にのみ呼ば
  れ、project commit と runtime activation は別証跡（RCP-BR-04
  Last-good/Pending 契約）。
- `synvelis-project` 内に live engine は存在しない — engine 側 rebuild
  は session 層（sv#514 領域）の責務。本 crate が正直にできるのは
  durable journal + typed Pending/Failed の報告まで。

### (c) synvelis.render.*

- `crates/synvelis-gateway` に `synvelis.render` 0 件。語彙は
  `crates/synvelis-workspace/src/render_ui.rs:1534-1550` の
  `ACTION_RENDER_{PLAN,INSPECT,REQUEST,CANCEL,RETRY,VERIFY,ATTACH,
  PUBLISH,REMOTE}` 9 件が正本。
- `synvelis.branch.*` の三層登録パターン（variant.rs ID+specs →
  prepare_engine.rs ops 登録 → commit.rs CommitGate 実行）が準拠先。
- gateway は synvelis-render に依存しない — VariantPorts 式の host
  注入 port（`RenderDispatch`）で閉じるのが既定方針。

### (d) VCS-RESTORE-10

- `restore/mod.rs` の `fork_project` は project_id 再採番のみで
  payload bytes を複製しない（D6「store 移送設計が必要」）。
- `RestorePorts` に branch-content read port を追加し、
  `Captured` digest を fork 先 `branch-content/` へ digest 検証付き
  で複製。port 不在・confirmed-absent は receipt の `unavailable`
  に正直記録（`CandidateUnavailable` verdict）。

## 判定

全残件とも lease 内で完結可能。dispatch 可能。
