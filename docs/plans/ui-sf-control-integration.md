---
id: "PLAN-102UiCfInt003CcDdEeFfG"
type: "plan"
status: "approved"
title: "UI-C5 製品 GUI 統合 実施計画 — syn.control.relation / syn.control.edge / syn.recording.*"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ui-sf-control-integration.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ui-sf-control-integration-design.md"
    - "docs/tasks/ui-sf-control-integration.md"
    - "docs/test-specs/ui-sf-control-integration.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ui-sf-control-integration.md"
    - "docs/designs/ui-sf-control-integration-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ui-sf-control-integration.md"
  references: []
  defers: []
  deferred-by: []
---

# UI-C5 製品 GUI 統合 実施計画

## スコープ

`synvelis-workspace` の Control Relation inspection focus 追加と、
`synvelis-app` への 4 op（`syn.control.relation`/`syn.control.edge`/
`syn.recording.status`/`syn.recording.action`）の wire surface 実装、
および headless テスト。編集範囲は task spec の「編集してよい範囲」に限定。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-workspace/src/relation_ui.rs` | `control_entity_tag`/`control_entity_parse` 公開、`ControlRelationInspectionFocus`/`ControlRelationViewState` 新設 |
| `crates/synvelis-workspace/src/state.rs` | `ViewState.control_relation` 追加（tolerant decode） |
| `crates/synvelis-workspace/src/workspace.rs` | `control_relation_state/inspect/set_inspection_mode/set_follow_source/explore_back/explore_forward` 追加 |
| `crates/synvelis-workspace/src/lib.rs` | 新規型・関数の re-export |
| `crates/synvelis-workspace/tests/uic5.rs` | control_relation focus の契約テスト追加 |
| `crates/synvelis-app/src/ui_ops.rs` | 4 op + wire helper（`control_entity`/`relation_view_json`/`control_edge_*_json`/`recording_*_json`） |
| `crates/synvelis-app/src/service.rs` | OpInfo 登録 + dispatch arm |
| `crates/synvelis-app/tests/uic5_app.rs` | 新設 — op surface の headless テスト |

## リスクと対策

| リスク | 対策 |
| --- | --- |
| `ControlEntityRef` に serde がない | tagged 文字列で永続化し、`control_entity_parse` で fail-closed に復元 |
| Explore 履歴の誤永続化 | `#[serde(skip)]` + snapshot JSON 往復テストで担保 |
| malformed payload で snapshot 全体が死ぬ | `deny_unknown_fields` + tolerant decode で `None` 劣化、テストで固定 |
| gesture outcome を error と混同 | `outcome` field の typed 値として返し、stale は error にしない |
| app 層での domain 再実装 | workspace/session API へ委譲のみ。commit ロジックは `control_record_*`/`control_edge_*` に一元化 |
| headless 成功を GUI 合格と誤記 | PR 本文・テスト doc で lane を明記。実 GUI は Deferred/NotRun |

## 検証マトリクス

| 受入条件 | 検証 |
| --- | --- |
| UI-C5-01 双方向探索 + 復帰 | `uic5_app.rs::relation_explore_bidirectional_and_back_to_origin`、`uic5.rs::uic5_control_relation_inspect_explore_back_and_modes` |
| UI-C5-02 edge≠routing / inspect≠edit / undo | `uic5_app.rs::control_edge_*` 3 件、`relation_inspect_never_touches_edit_or_undo`、既存 `uic5_02_*` |
| UI-C5-03 modes/stability/stale | `uic5_app.rs::relation_*`、`uic5.rs` snapshot/mode テスト、既存 `uic5_03_*` |
| UI-C5-04 recording states/actions | `uic5_app.rs::recording_*` 3 件、既存 `uic5_04_*` |
| UI-C5-05 headless honesty | テスト doc コメント + PR 本文の NotRun/Deferred 記録 |

## ロールバック

全変更は additive（新 op・新 Option field・新テスト）。revert で
wire surface と per-view state が除去され、既存 `signal_flow`/
`control_edge`/`recording_status` の workspace 契約は無変更のまま残る。
