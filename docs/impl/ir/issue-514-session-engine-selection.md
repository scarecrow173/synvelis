---
id: "IMPL-bJVGmSy1j9h2Vvv5gDkQVO"
type: "impl"
status: "completed"
title: "Issue #514 impl record — 判定記録 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
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
        path: "evidence/sv514"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-514-session-engine-selection.md"
    - "docs/tasks/issue-514-session-engine-selection.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-514-session-engine-selection.md"
    - "docs/tasks/issue-514-session-engine-selection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-514-session-engine-selection.md"
    - "evidence/sv514/decision-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-514-session-engine-selection.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #514 impl record

## 実施内容

判定分岐 B（実装不要）を採用し、以下を実施した。

1. **root-cause 調査**: rebuild 経路（`engine.rs`）・`SelectionInputs`
   供給可能性（`rcp_selection.rs`・`PluginHost`・`ProcessorBinding`）・
   pin 所在（`routing_engine.rs`・`rcp/verify.rs`・`rcp/model.rs`・
   `rcp_selection_integration.rs`・`sf03_session_engine.rs`）・
   consumer（`render.rs`・`project_selection`）を確認。
2. **判定記録**: doc chain 全 9 文書 + `evidence/sv514/` を作成。
3. **`engine.rs` module doc 追記**: selection 非消費が意図的である旨と
   将来の配線 trigger を明記（挙動変更なし）。
4. **AC4 帰属**: stateless `PluginStateDecl` は player 契約変更が
   必要と確定し、別 Issue を起票して `evidence/sv514/
   decision-ledger.md` に帰属を記録。
5. **検証**: `mise` 必須 lane を実行し `evidence/sv514/
   verification.md` に実測を記録。

## 変更ファイル

- `crates/synvelis-app/src/engine.rs` — module doc comment のみ
- `docs/{discovery,specs,designs,plans,tasks,test-specs,adr,impl}/
  issue-514-session-engine-selection.md` — 新規
- `docs/impl/exp/issue-514-session-engine-selection-exp.jsonl` — 新規
- `evidence/sv514/` — 新規

## 非変更（意図的）

- rebuild 経路の挙動・`EnginePort` 契約・`SessionEngineConfig`/
  composition 経路 — 変更なし
- `synvelis-app` の dep — 追加なし（synvelis-render 非依存を維持）
- `synvelis-player`/`synvelis-project`/`synvelis-workspace`（
  `rcp_selection.rs` 以外） — 変更なし
- `Cargo.toml`/`Cargo.lock` — 変更なし
