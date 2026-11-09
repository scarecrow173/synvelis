---
id: "SPEC-034Ul4godUxplIT6whAIkO"
type: "spec"
status: "approved"
title: "SV-GUI-03 slice1: Transport 操作 + Inspector 表示を既存 GPUI 経路に接続"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/175"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
    - "docs/designs/sv-gui-03-01-transport-inspector-slice1-design.md"
    - "docs/tasks/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/test-specs/sv-gui-03-01-transport-inspector-slice1.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice1 仕様: Transport 操作 + Inspector 表示

## What Should Be Built

Issue #112（SV-GUI-03 Core Workspace GUI）の第 1 スライス。1 Window の
既存 driver-neutral GPUI 経路（`ViewSpec → ViewHost → ViewBinding →
rvf.ui.command.v1 → NativeShell → UiRuntime/AppService → ControlsUi →
syn.control.* projection/receipt → NativeShell outbox → ViewHost`）に、
Transport 操作と Inspector 表示を接続する。domain/ops 層
（`synvelis-workspace`・`synvelis-player`・`engine.rs`/`session.rs`/
`service.rs`）と新規 `syn.*` op は変更しない。

1. **Transport 操作**: Transport tab に `transport` target の
   Play/Stop `Button` を mount する。押下は既存の `Action` intent →
   `syn.control.dispatch` → `ControlsUi::route_action`
   （`play`/`stop` → `transport_play`/`transport_stop` → `Applied`
   receipt → `broadcast_all`）の経路をそのまま通る。`transport`
   target を `main_view_targets()` の購読対象に加え、
   `TransportProjection`（status/position/loop/record flags）の
   projection で status readout（`Label`）を表示する。seek の秒
   表示規約（#175 確定: 表示・書き込みは秒、beat 変換は
   commit/adapter 境界のみ）は維持する。
2. **Inspector 表示**: 選択 target の `InspectorBreakdown` を `Table`
   で表示する。slice 1 では view が購読する param target（現行は
   mixer gain fader の bootstrap target 1 件）が対象であり、
   target 選択 UI は後続 slice の範囲。表示値は既存の
   `syn.control.projection` param row が運ぶ
   `ParamProjection.breakdown`（evaluation + observation + mode +
   arm + writer_state + capability）を app 層で捕捉し、既存の
   `update_view` push パターン（project-length 更新と同じ
   `MainScreenState` 共有セル → spec 再構築 → `ViewHost::update`）
   で spec を更新する。新しい domain op・projection channel は
   追加しない。
3. **headless 検証**: `tests/view_mount.rs` を拡張し、実
   `FakeDriver`/`ViewHost`/`NativeShell`/`AppService` 経路で
   gesture→command→receipt→projection と Button Action →
   `Applied` receipt → projection → spec 更新を検証する。
   `ruviform-ui-test` の `SemanticGuiTest` contract
   （`parity_gesture_scenario`）でも同経路を実行する。
4. **実機検証**: `run-app-native`（`synvelis-native`、feature
   `native`）で実 GPUI window が起動し Transport/Inspector が
   表示・操作できることを Windows 実機で確認し、証拠を記録する。

## Why Now

Issue #112 は Arrangement/Mixer/Transport/Inspector の 4 region の
GPUI 標準 driver 化を要求するエピックであり、Issue 自身が第 1
スライスとして「Transport 操作 + Inspector 表示を既存 GPUI 経路に
接続」を指定している。SV-GPUI-01（native shell）・SV175-02（view
mount）・SV185-01（mixer gain projection）で command/projection/
receipt の wire 契約と `update_view` push の前例が確立済みであり、
本 slice はその上に product 領域を広げる最小の垂直 slice である。

## Users and Value

- Synvelis 開発者/利用者: 実窓で Transport の Play/Stop と
  position/loop readout、選択パラメータの Inspector breakdown を
  操作・確認できる。
- 後続 slice（Arrangement・Dock・Theme 等）: 本 slice が確立する
  `MainScreenState` push と region 配置の先例を再利用できる。

## Scope

- `crates/synvelis-app/src/screens.rs`: `main_screen_spec` に
  Transport Button/Label・Inspector tab/Table を追加し、
  `MainScreenState` 型と `main_view_targets()` の `transport`
  追加、`mount_main_view` の state 引数化。
- `crates/synvelis-app/src/app.rs`: `project_length` cell を
  `MainScreenState` 共有セルに拡張（Coordinator 承認済みの最小配線）。
- `crates/synvelis-app/src/ui_ops.rs`: service op 後の publish で
  購読 binding の `last_observed`（transport/param projection）を
  `MainScreenState` に捕捉。
- `crates/synvelis-app/src/lib.rs`: 新規型の re-export。
- `crates/synvelis-app/src/bin/synvelis-native.rs`: state 変化時の
  `update_view` push を `MainScreenState` 全体へ拡張。
- `crates/synvelis-app/tests/`: `view_mount.rs` 拡張 +
  `SemanticGuiTest` scenario。
- `docs/`: 本 slice の文書チェーン。

## Non-Goals

- Arrangement region の本実装、Undo/Redo（`syn.*` op 未公開）。
- Dock/2 window（#113）、Theme（#177）、Plugin Dock（#176）。
- `synvelis-workspace`/`synvelis-player`/`engine.rs`/`session.rs`/
  `service.rs` の domain/ops 変更、新規 `syn.*` op 追加。
- `Cargo.toml` 依存追加、`.github/`・`.devin/` の変更。
- 実 OS E2E/IME/pixel 検証（#114・ruviform #313/#314 が owner）。
- `SemanticGuiTest` の `RealOs` driver・Pixel Golden。
- Transport pause/resume button、record enable、loop 編集 UI —
  domain op は存在するが本 slice の acceptance は Play/Stop に限定。

## Acceptance（Issue #112 slice 1 転記）

- Transport region: `transport` target に Play/Stop の
  `Button`/`Toggle` を mount し、既存の command 経路で transport
  を操作できる。`TransportProjection`（status/position/loop）の
  購読で状態表示（status readout）を行う。seek の秒表示規約を維持。
- Inspector region: 選択 target の `InspectorBreakdown` を `Table`
  または `Label` で表示し、既存 `update_view` push パターンで
  spec を更新する（新しい domain op は追加しない。表示経路のみ）。
- テスト: `view_mount.rs` 拡張 + `SemanticGuiTest`/`FakeGuiTest`
  （ruviform-ui-test の Scenario contract）で
  gesture→command→receipt→projection が headless で検証される。
- 実 GPUI window（`run-app-native` 等の manual lane）で起動・操作
  できることを確認し、実行証拠（コマンド・OS・結果）を PR に記録。
- `docs/` の文書チェーン（spec→design→plan→task→test-spec）を作成。

## Open Questions

- なし（Coordinator 確認済み: app crate 内の最小配線
  `app.rs`/`ui_ops.rs`/`lib.rs`/`bin/synvelis-native.rs` への変更は
  許可。domain/ops 層は不変）。
