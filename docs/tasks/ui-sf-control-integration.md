---
id: "TASK-102UiCfInt004DdEeFfGgH"
type: "task"
status: "done"
title: "UI-C5 製品 GUI 統合タスク — syn.control.relation / syn.control.edge / syn.recording.*"
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
    - "docs/plans/ui-sf-control-integration.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ui-sf-control-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ui-sf-control-integration.md"
    - "docs/designs/ui-sf-control-integration-design.md"
    - "docs/test-specs/ui-sf-control-integration.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ui-sf-control-integration.md"
  references: []
  defers: []
  deferred-by: []
---

# UI-C5 製品 GUI 統合タスク

| # | タスク | 完了条件 |
| --- | --- | --- |
| 1 | `relation_ui.rs` entity tag/parse + focus 型 | `control_entity_tag`/`control_entity_parse` が 4 種 `ControlEntityRef` を tagged 文字列で round-trip。`ControlRelationInspectionFocus`/`ControlRelationViewState` が `deny_unknown_fields` + session-only explore stacks で定義される |
| 2 | `state.rs` per-view state | `ViewState.control_relation` が tolerant decode 付き `Option` として存在し、snapshot の malformed payload が `None` に劣化する |
| 3 | `workspace.rs` control_relation API | inspect/mode/follow_source/back/forward が editing selection・`inspection_focus`・project・undo に触れず、Explore 履歴規則が signal_flow と一致する |
| 4 | `lib.rs` export | 新規型・関数が `synvelis_workspace` root から到達可能 |
| 5 | `ui_ops.rs` op + wire helper | `syn.control.relation`/`syn.control.edge`/`syn.recording.status`/`syn.recording.action` が typed parse → workspace 委譲 → JSON 応答を行い、stale/outcome を error と区別する |
| 6 | `service.rs` 登録 | 4 op が `OpInfo` と dispatch arm に登録される |
| 7 | workspace テスト | `uic5.rs` に inspect/explore/mode/snapshot 契約テストが追加され pass する |
| 8 | app テスト | `uic5_app.rs` が relation 双方向探索・edge gesture lifecycle/stale・recording 5 state/actions を headless で検証し pass する |
| 9 | 検証 | `fmt-check`/`check`/`lint`/`test`（synvelis-app・synvelis-workspace）が pass。実 GUI レーンは NotRun/Deferred として記録 |
