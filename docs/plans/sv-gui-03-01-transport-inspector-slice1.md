---
id: "PLAN-034Ul4TkjPiKYvapjdmBbt"
type: "plan"
status: "approved"
title: "SV-GUI-03 slice1 Transport/Inspector 実施 plan"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-01-transport-inspector-slice1-design.md"
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/designs/sv-gui-03-01-transport-inspector-slice1-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-01-transport-inspector-slice1.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice1 実施 plan

## Scope

Issue #112 slice 1: Transport 操作（Play/Stop）+ Inspector 表示を
既存 GPUI/ViewHost 経路に接続。domain/ops 層は不変。

## File Map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-app/src/screens.rs` | `MainScreenState`/`TransportScreenState`/`InspectorScreenState`/`InspectorRow` 追加。`main_screen_spec(&MainScreenState)` 化。`main_view_targets()` に `TRANSPORT_TARGET` 追加。Transport tab に status `Label` + Play/Stop `Button`、新規 Inspector tab に target `Label` + `Table`。`mount_main_view` を state 引数化。 |
| `crates/synvelis-app/src/app.rs` | `project_length: Rc<Cell<f64>>` → `main_screen: Rc<RefCell<MainScreenState>>`（`pub(crate)`）。`project_length_cell()` → `main_screen_cell()`。`publish_project_length()` は `main_screen.project_length_seconds` を更新（service.rs 無変更のため名前維持）。 |
| `crates/synvelis-app/src/ui_ops.rs` | `ui_pump` 末尾に `capture_screen_state()` 呼出を追加。`capture_screen_state(&self)` が `ui.subscriptions`（view=`main_view_id()`）経由で `controls.transport_row`/`strip_row` の `observed` を読み `TransportScreenState`/`InspectorScreenState` を `main_screen` cell に書込。 |
| `crates/synvelis-app/src/lib.rs` | `MainScreenState` 等 4 型を re-export。 |
| `crates/synvelis-app/src/bin/synvelis-native.rs` | `project_length_cell` → `main_screen_cell`。mount/update を `MainScreenState` 全体の `PartialEq` 比較で push。 |
| `crates/synvelis-app/tests/view_mount.rs` | fixture が `main_screen_cell` を保持。`push_screen_state()` ヘルパ追加。transport Button/Label/Table の検証 + `spec(120.0)` 呼出の state 化。 |
| `crates/synvelis-app/tests/`（新規 or 既存に追加） | `FakeGuiTest`/`SemanticGuiTest` scenario: `parity_gesture_scenario` on seek + `invoke` on transport.play + `post_event` receipt/projection。 |
| `docs/` | 本チェーン（spec/design/plan/task/test-spec）。 |

## Dependency Graph

```
screens.rs (state types + spec)
  ├─ app.rs (cell field, publish)
  ├─ ui_ops.rs (capture_screen_state in ui_pump)
  ├─ lib.rs (re-export)
  ├─ bin/synvelis-native.rs (update_view push)
  └─ tests/view_mount.rs + scenario test
```

## Steps

1. `screens.rs`: state 型 + spec 構築 + targets + mount 引数化。
2. `app.rs`/`ui_ops.rs`/`lib.rs`: cell + capture + export。
3. `bin/synvelis-native.rs`: state push。
4. `tests/view_mount.rs` 更新 + 新規 scenario test。
5. mise 検証 → native lane → run-app-native → review → PR。

## Risks

- `service.rs` は編集範囲外 → `publish_project_length` 名を維持し
  本体を `MainScreenState` 更新に変更（PR に判断を明記）。
- `MainScreenState` の float 比較で update 頻度 — `PartialEq` で
  dedup、表示上問題ない範囲。
- `RefCell` borrow 競合 — 単一スレッド、短い borrow のみ。

## Rollback

`git revert` で spec/state 型ごと戻る。domain/ops 層に変更がない
ため外部影響なし。
