---
id: "DESIGN-5n53fkGhYaUtJlMHOu5P1p"
type: "design"
status: "approved"
title: "SV-GUI-03 slice1 Transport/Inspector 設計 — MainScreenState push + 既存 syn.control.* 経路"
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
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice1 設計: Transport 操作 + Inspector 表示

## Context

`main_screen_spec`（`crates/synvelis-app/src/screens.rs`）は現在
`Panel > [Toolbar, Tabs]` で、Transport tab は `transport/position`
の `Seek` のみ、Mixer tab は master fader の `Fader` のみを持つ。
`transport` target（`synvelis_workspace::controls_ui::TRANSPORT_TARGET`
= `"transport"`）は `ControlsUi::route_action` が `play`/`stop`/
`pause`/`resume` を解釈する既存の command target であり、
`TransportProjection`（status/position/loop/record flags）は
`TransportRow` 経由で既に投影されている。`InspectorBreakdown` は
param target の `ParamProjection.breakdown` として既に
`syn.control.projection` の param row に載っている。

不足しているのは view 側の配線だけである:

1. `transport` target を `main_view_targets()` に追加して購読を
   有効化する。
2. Transport tab に Play/Stop `Button`（`target="transport"`,
   `action="play"|"stop"`）と status `Label` を追加する。
3. Inspector tab に `Table` を追加し、購読 param target の
   `InspectorBreakdown` を表示する。
4. projection 由来の表示値を `update_view` push で spec に反映する
   ための app 層 state（`MainScreenState`）を導入する。

## Goals / Non-Goals

**Goals**

- Transport Play/Stop が既存 `Action` intent → `syn.control.dispatch`
  → `Applied` receipt → `broadcast_all` の経路で動作する。
- `TransportProjection` が status readout（`Label`）に反映される。
- `InspectorBreakdown` が `Table` に反映される。
- 既存 `transport/position` seek・mixer fader の gesture/projection/
  receipt セマンティクスを壊さない。
- headless テストで wire 全体（gesture→command→receipt→
  projection→spec update）を検証する。

**Non-Goals**

- 新規 `syn.*` op、domain/ops 層の変更、新規 projection channel。
- Inspector target 選択 UI（slice 1 は購読 param target のみ）。
- Pause/Resume/Record button、loop 編集、Arrangement region。

## Design

### `MainScreenState`（`screens.rs`）

表示専用の不変スナップショット。`Rc<Cell<MainScreenState>>` で共有
するため `Clone + PartialEq` が必要（`Cell` は `Copy` 不要だが
`set`/`get` に `Clone` が要る）。

```rust
/// Transport status/position/loop の readout 用スナップショット。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransportScreenState {
    /// 再生状態の表示文字列（例: "stopped"/"playing"/"paused"）。
    pub status: Option<String>,
    /// 現在位置（秒）。
    pub position_seconds: Option<f64>,
    /// ループ有効。
    pub loop_enabled: bool,
}

/// Inspector 表示用の 1 行分の breakdown。
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorRow {
    pub field: String,
    pub value: String,
}

/// Inspector tab の表示内容。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InspectorScreenState {
    /// 表示対象 param target の tagged id（無ければ None）。
    pub target: Option<String>,
    /// breakdown の行（field/value の組）。
    pub rows: Vec<InspectorRow>,
}

/// `main_screen_spec` が必要とする全表示 state。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MainScreenState {
    pub project_length_seconds: f64,
    pub transport: TransportScreenState,
    pub inspector: InspectorScreenState,
}
```

`main_screen_spec` のシグネチャを
`main_screen_spec(state: &MainScreenState) -> ViewSpec` に変更し、
project length・transport status・inspector rows を spec 内の
`Label`/`Table` content に焼き込む。`Cell` の `PartialEq` で
「前回と同じなら `update_view` を呼ばない」最適化が可能。

### `screens.rs` の spec 変更

- `main_view_targets()` に `TRANSPORT_TARGET.to_string()` を追加
  → `vec![TRANSPORT_POSITION_TARGET, mixer_gain_target(),
  TRANSPORT_TARGET]`。
- Transport tab の panel children を
  `[transport_status_label, transport_buttons, transport_seek]` に
  拡張:
  - `transport_status_label`: `Label`（key=`"transport.status"`,
    content=`state.transport` から生成した文字列）。`control_id`
    は付けない（projection 対象外の静的表示）。
  - `transport_buttons`: `Button` × 2
    （key=`"transport.play"`/`"transport.stop"`,
    target=`TRANSPORT_TARGET`, action=`"play"`/`"stop"`）。
    `ButtonSpec` に `control_id` は無い（discrete Action intent
    で `control_id` は button key になる — RF410 の shared
    `control_id` 問題は projection を持つ control のみ関係）。
  - `transport_seek`: 既存そのまま。
- 新しい `TabSpec`（key=`"tab.inspector"`, title=`"Inspector"`）を
  tabs に追加。child panel に `Table`（key=`"inspector.table"`,
  columns=[`"Field"`,`"Value"`], rows=`state.inspector.rows` を
  `CellValue::Text` にマップ）。`control_id` 無し。

`TabsSpec.active` は `0` のまま（Transport が先頭）。`tabs` の
順序は `[Transport, Mixer, Inspector]`。

### `app.rs` の state cell

`project_length: Rc<Cell<f64>>` を `main_screen:
Rc<RefCell<MainScreenState>>` に置き換える（`Cell` ではなく
`RefCell` — `MainScreenState` は `Copy` でないため）。
`project_length_cell()` は削除し、`main_screen_state()` /
`main_screen_cell()` を追加する。

`publish_project_length()` を `publish_main_screen()` に改名し、
`project_length_seconds()` を `MainScreenState.project_length_
seconds` に書き込む。transport/inspector の値は `ui_ops` が別途
`MainScreenState` に書き込む（`ui` フィールド経由）。

`SynvelisApp` に `main_screen: Rc<RefCell<MainScreenState>>` を追加。
`main_screen_state()` で `Rc` clone を返し、`publish_main_screen()`
で project length を更新する。transport/inspector は `ui_ops.rs`
の `capture_screen_state()` が `ControlsUi` の binding
`last_observed` から抽出して `main_screen.borrow_mut()` に書き込む。

### `ui_ops.rs` の capture

`ControlsUi` が公開する binding snapshot（`TransportRow`/
`StripRow`/`BindingSnapshot`）から以下を抽出:

- `TransportRow.last_observed` → `TransportProjection` →
  `status`/`position`/`loop_enabled` → `TransportScreenState`。
- `StripRow`（param binding）の `last_observed` →
  `ParamProjection.breakdown`（`InspectorBreakdown`）→
  `InspectorScreenState.rows`。

`AppService::call` が op 後に `publish_main_screen()` を呼ぶ
既存パターンに倣い、`ui_pump`（または相当の publish 関数）内で
`capture_screen_state()` を呼んで `MainScreenState` を更新する。

### `lib.rs` の re-export

`MainScreenState`, `TransportScreenState`, `InspectorScreenState`,
`InspectorRow` を `pub use screens::{...}` で re-export。
`main_screen_spec` のシグネチャ変更に伴い `mount_main_view` も
`&MainScreenState` を取るように変更。

### `bin/synvelis-native.rs` の update_view push

現在:
```rust
if length_cell.get() != last_length {
    view_driver.update_view(window, main_screen_spec(length_cell.get()));
}
```

変更後:
```rust
let state = screen_cell.borrow().clone();
if state != last_state {
    view_driver.update_view(window, main_screen_spec(&state));
    last_state = state;
}
```

`Rc<RefCell<MainScreenState>>` を `main_screen_cell()` で取得し、
`borrow()` で snapshot を取り `PartialEq` で変化検出する。
`update_view` は spec の契約が同じなら mounted identity を保持する
ため、seek/fader の gesture 状態は壊れない。

### `tests/view_mount.rs` の拡張

既存 fixture（`FakeDriver`/`ViewHost`/`NativeShell`/`AppService`）を
使い、以下を検証:

1. `transport.play` Button の `invoke` → `Action` intent
   （target=`"transport"`, action=`"play"`）→ `syn.control.dispatch`
   → `Applied` receipt → `broadcast_all` → `TransportProjection`
   更新。
2. `transport.stop` Button 同様。
3. `TransportProjection` の status/position が `transport.status`
   Label の表示に反映される（`MainScreenState` → `update_view` →
   `ComponentSnapshot.display`）。
4. `InspectorBreakdown` が `inspector.table` の rows に反映される。
5. 既存 seek/fader の gesture→command→receipt→projection が
   regression 無しで動作する。

`SemanticGuiTest`/`FakeGuiTest`（`ruviform-ui-test`）で
`parity_gesture_scenario` を使い、Button invoke → command →
receipt → projection の semantic 経路も検証する。

## Risks / Trade-offs

- **`MainScreenState` の更新頻度**: `ui_pump` は service op 毎に
  呼ばれる。transport position は連続的に変わるため、`PartialEq`
  で「実質同じ」場合は `update_view` をスキップする。ただし
  position は float なので毎回変わる可能性がある — 必要なら
  表示精度（小数 2 桁）で丸めて比較する。
- **`RefCell` の借用競合**: `ui_ops` が `borrow_mut` で書き込み、
  native bin が `borrow` で読む。単一スレッドなので問題ないが、
  `borrow_mut` 中に `borrow` が来ると panic する — 書き込みは
  短時間で完結させる。
- **Inspector target の限定**: slice 1 は購読 param target（mixer
  gain fader）のみ。選択 UI は後続 slice。
- **`Button` の `control_id`**: `ButtonSpec` に `control_id` フィールド
  は無い。`ViewHost::invoke` は button key を `control_id` として
  mint するため、RF410 の shared `control_id` 問題は projection を
  持つ control のみ関係し、Button には影響しない。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` / `build`
- `mise run check-app-native` / `lint-app-native` / `build-app-native`
- `mise run run-app-native` で実 GPUI window 起動・操作確認
- `tests/view_mount.rs` + `SemanticGuiTest` scenario
