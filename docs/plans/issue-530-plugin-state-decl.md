---
id: "PLAN-yBpwmwoB5470oC0kf0tb"
type: "plan"
status: "approved"
title: "Issue #530 plan — stateless PluginStateDecl 契約 slice"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "docs/adr/issue-530-plugin-state-decl.md"
  changes:
    added: []
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
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "docs/adr/issue-530-plugin-state-decl.md"
  implemented-by:
    - "docs/tasks/issue-530-plugin-state-decl.md"
  depends-on:
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "docs/adr/issue-530-plugin-state-decl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-530-plugin-state-decl.md"
    - "evidence/sv530/decision-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-530-plugin-state-decl.md"
  derived-by:
    - "docs/tasks/issue-530-plugin-state-decl.md"
    - "docs/test-specs/issue-530-plugin-state-decl.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #530 plan — stateless PluginStateDecl 契約 slice

## スコープ

`PluginStateDecl` で stateless plugin を表現し、`verify_plugin` が
env=Unsupported + decl=stateless-consistent で `Live` に到達する
契約を実装する。既存語彙再利用設計（design/ADR 参照）のため、
variant/field 追加は行わず lease 外ファイルへの波及を持たない。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-player/src/rcp/model.rs` | `PluginStateDecl::stateless()` predicate 追加 + `PluginStateDecl`/`StateRestoreSupport::Unsupported` の doc 更新 |
| `crates/synvelis-player/src/rcp/verify.rs` | `verify_plugin` の `Unsupported` arm に stateless→Live 経路追加 + `PluginVerdict::{Live,StateUnsupported}` doc 更新 |
| `crates/synvelis-player/tests/rcp_04.rs` | stateless profile 単体 oracle・env/decl verdict matrix・selection-level Live/Blocked case 追加 |
| `crates/synvelis-workspace/src/rcp_selection.rs` | `ProcessorPluginEvidence.state` doc に stateless profile の宣言方法を明記（コード変更なし） |
| `crates/synvelis-workspace/tests/rcp_selection_integration.rs` | stateless evidence 経路 + 未 capture fail-closed の integration case 追加 |
| `docs/{discovery,specs,designs,plans,tasks,test-specs,adr,impl/ir,impl/exp}/issue-530-plugin-state-decl.md` + `-exp.jsonl` | 新規 — doc chain |
| `evidence/sv530/` | 新規 — decision-ledger・verification |

## リスクと rollback

- リスク: `Unsupported` arm の意味拡大が既存呼出の期待を変える →
  verdict matrix oracle で全組合せを pin。`StateUnsupported` を
  期待する既存 test は decl が stateless profile でない限り不変。
- リスク: stateless profile の `status != Known` 条件が広すぎる →
  `Known` は検証済み state 主張であり stateless と矛盾するため
  除外が正しい。oracle で pin。
- rollback: 変更は `verify_plugin` 1 arm + predicate + test/doc のみ。
  revert で完全に戻る。

## 検証マトリクス

| 検証 | コマンド | 期待 |
| --- | --- | --- |
| fmt | `mise run fmt-check` | pass（既知 flake 時は per-package fmt を補助証拠） |
| player | `mise run check/lint/test synvelis-player` | pass（`rcp_04` 新規 oracle 含む） |
| workspace | `mise run check/lint/test synvelis-workspace` | pass（`rcp_selection_integration` 新規 case 含む） |
| render 波及確認 | `mise run check synvelis-render`（補助） | pass（`rcp07_gate.rs` literal 非破壊の確認） |
| 実機/RT | T3/T4・実 plugin/OS/PCM/RT | NotRun（#109/#296 cross-gate） |
