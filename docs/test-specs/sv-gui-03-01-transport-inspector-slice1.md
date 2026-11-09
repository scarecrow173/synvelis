---
id: "TSPEC-034Ul4TWD6Zf708dAHB1OW"
type: "test-spec"
status: "approved"
title: "SV-GUI-03 slice1 検証仕様: Transport 操作 + Inspector 表示"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/designs/sv-gui-03-01-transport-inspector-slice1-design.md"
    - "docs/tasks/sv-gui-03-01-transport-inspector-slice1.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice1 test-spec: Transport 操作 + Inspector 表示

## Feature: Transport Button → command → receipt → projection → spec

### Rule T1: Play/Stop Button が `transport` target の Action を mint する

`transport.play`/`transport.stop` Button の `invoke` は
`CommandIntent::Action{target:"transport", action:"play"|"stop"}` を
mint する。`control_id` は button key（`"transport.play"` 等）。

- Evidence: `tests/view_mount.rs` で `f.press("transport.play")` 後に
  `syn.control.receipt` notify（target=`"transport"`,
  status=`"applied"`）が観測される。`FakeGuiTest` scenario で
  `drain_intents` が `Action{target:"transport",action:"play"}` を返す。

### Rule T2: `TransportProjection` が `MainScreenState` → spec Label に届く

`transport` target 購読後、`route_action` の `broadcast_all` で
`TransportProjection`（status/position/loop）が binding に land し、
`ui_pump` の `capture_screen_state` が `MainScreenState.transport`
を更新する。native lane の `update_view` 相当（`host.update(
main_screen_spec(&state))`）で `transport.status` Label の text が
status+秒+loop を反映する。

- Evidence: play 押下後 `screen.borrow().transport.status ==
  Some("Playing")`、spec 内 Label text が `"Playing"` を含む。
  stop 押下後 `"Stopped"`。

### Rule T3: seek の秒表示規約を維持する

`transport/position` の display_value は従来通り tempo map 経由の
秒。`MainScreenState` の `position_seconds` も同じ
`TickTimeMapping::beat_to_seconds` 変換を使う。beat 変換は
capture/spec 構築の境界のみ。

- Evidence: 既存 `seek_display_follows_the_authoritative_projection_
  in_seconds` が回帰なく pass。status Label の秒表記は
  `position_seconds`（秒）から生成。

## Feature: Inspector Table ← ParamProjection.breakdown

### Rule I1: 購読 param target の breakdown が Table rows に出る

`ui.subscriptions` 内の main view の param-kind target（transport
2 target 以外、slice 1 では `mixer_gain_target()`）について
`strip_row.observed`（`ParamProjection.breakdown`:
evaluation+observation+mode+arm+writer_state+capability+read_only）
を `InspectorScreenState.rows` に展開し、`inspector.table` の
`TableSpec.rows` に `CellValue::Text` で載せる。`inspector.target`
Label に target tagged id を表示。

- Evidence: fixture で attach+subscribe 後 `screen.borrow().
  inspector.target == Some(mixer_gain_target())`、rows 非空、
  spec の `inspector.table` に breakdown 行（mode/arm/observation/
  writer/read_only/value）が存在。

### Rule I2: spec 更新は mounted identity を壊さない

`MainScreenState` 変化時の `update_view`（`ViewHost::update`）は
`ProjectionOp::Updated` のみを返し、seek/fader/新規 Button/Label/
Table の key は再 mount されない。gesture 中の control は
`contract_changed` を起こさない（control spec 不変）。

- Evidence: `seek_range_follows_project_length_in_place` 相当の
  assert を state 更新版で行い、`ops` が全て `Updated`、
  `host.generation()` 不変、seek/fader gesture→Applied receipt が
  継続。

## Feature: SemanticGuiTest contract

### Rule S1: `FakeGuiTest`/`parity_gesture_scenario` が同 spec で走る

`main_screen_spec` を `FakeGuiTest::mount_view` に mount し、
`parity_gesture_scenario`（Begin→Preview→Commit）で seek gesture
の minted intents を取得、`invoke` で `transport.play` の
`Action` intent を取得、`post_event` で receipt `Result` と
projection `Notify` を届けて `observe` が phase/observed を反映
する。driver=`Fake`、capabilities=`{semantic-action, multi-window}`
の semantic evidence のみを主張する（pixel/OS は対象外）。

## Feature: 実機 lane

### Rule R1: `run-app-native` で実 GPUI window が起動・表示・操作できる

`mise run run-app-native`（feature `native`、`synvelis-native`）で
window が開き、Transport tab に Play/Stop・status、Inspector tab
に breakdown table が描画される。Play/Stop が transport を操作
する。証拠（コマンド・OS・結果）を PR に記録。pixel/IME/E2E は
#114/ruviform #313/#314 の owner で本 slice 対象外。

## Non-Goals（検証しないもの）

- Arrangement region、Undo/Redo、Dock/2 window、Theme、Plugin Dock。
- `RealOs` driver、Pixel Golden、実 OS E2E/IME。
- `syn.transport.set`（legacy op）経路の projection refresh —
  ControlsUi broadcast を伴わない既存 semantics を変更しない。
