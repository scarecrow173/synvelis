---
id: "IMPL-IAG5was6XCP1GXzwMhcv"
type: "impl"
status: "completed"
title: "Issue #530 impl record — stateless PluginStateDecl 契約 slice"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
  changes:
    added:
      - type: "dir"
        path: "docs/discovery"
      - type: "dir"
        path: "docs/specs"
      - type: "dir"
        path: "docs/designs"
      - type: "dir"
        path: "docs/plans"
      - type: "dir"
        path: "docs/tasks"
      - type: "dir"
        path: "docs/test-specs"
      - type: "dir"
        path: "docs/adr"
      - type: "dir"
        path: "docs/impl"
      - type: "dir"
        path: "evidence/sv530"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/rcp/model.rs"
      - type: "file"
        path: "crates/synvelis-player/src/rcp/verify.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/rcp_04.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/rcp_selection.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/rcp_selection_integration.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-530-plugin-state-decl.md"
    - "docs/tasks/issue-530-plugin-state-decl.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-530-plugin-state-decl.md"
    - "docs/tasks/issue-530-plugin-state-decl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-530-plugin-state-decl.md"
    - "evidence/sv530/decision-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-530-plugin-state-decl.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-530-plugin-state-decl.md"
    rejected: []
---

# Issue #530 impl record

## 実施内容

設計判断（既存語彙再利用 — ADR 参照）に基づき、以下を実施した。

1. **`model.rs`**: `PluginStateDecl::stateless()` predicate を追加。
   stateless-by-design profile を `digest: None` +
   `quality: StateCaptureQuality::Unsupported` +
   `status != OpaqueStateStatus::Known` として定義。
   `PluginStateDecl` と `StateRestoreSupport::Unsupported` の doc を
   新契約に合わせて更新。
2. **`verify.rs`**: `verify_plugin` の
   `StateRestoreSupport::Unsupported` arm に
   `decl.state.stateless()` → `Live` の経路を追加。それ以外は
   `StateUnsupported` を維持。`PluginVerdict::{Live,
   StateUnsupported}` の doc を更新。
3. **`rcp_selection.rs`**: `ProcessorPluginEvidence.state` の doc に
   stateless profile の宣言方法を明記（コード変更なし — 証拠は
   そのまま `PluginDeclaration` に写される）。
4. **oracle 追加**: `rcp_04.rs` に profile 単体・verdict matrix・
   selection-level Live/Blocked の 4 case、`rcp_selection_integration.
   rs` に stateless evidence 経路 + 未 capture fail-closed の
   1 case を追加。
5. **doc chain + evidence**: 全 9 文書 + `evidence/sv530/` を作成。
6. **検証**: `mise` 必須 lane を実行し `evidence/sv530/
   verification.md` に実測を記録。

## 変更ファイル

- `crates/synvelis-player/src/rcp/model.rs` — `stateless()` 追加 + doc
- `crates/synvelis-player/src/rcp/verify.rs` — `Unsupported` arm + doc
- `crates/synvelis-player/tests/rcp_04.rs` — oracle 4 case 追加
- `crates/synvelis-workspace/src/rcp_selection.rs` — doc のみ
- `crates/synvelis-workspace/tests/rcp_selection_integration.rs` —
  oracle 1 case 追加
- `docs/{discovery,specs,designs,plans,tasks,test-specs,adr,impl}/
  issue-530-plugin-state-decl.md` + `-exp.jsonl` — 新規
- `evidence/sv530/` — 新規

## 非変更（意図的）

- `PluginStateDecl`/`StateRestoreSupport`/`PluginVerdict` の
  variant/field 数 — 変更なし（lease 外波及を回避）
- `crates/synvelis-render/tests/rcp07_gate.rs`・
  `crates/synvelis-workspace/src/render_ui.rs`・
  `crates/synvelis-workspace/tests/rcp06_render_ui.rs` — 無変更
- `synvelis-project` の `StateCaptureQuality`/`OpaqueStateStatus` —
  read-only consume、変更なし
- `SessionRoutingEngine` 配線（sv#514 で実装不要確定済み）
- `Cargo.toml`/`Cargo.lock` — 変更なし（依存追加なし）
- docs index（`docs/**/README.md`）— Coordinator が統合時に再生成
