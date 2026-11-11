---
id: "SPEC-Kd4nQx8vRw2mLp9sTf3hJc"
type: "spec"
status: "approved"
title: "SV-GUI-03 slice4: Arrangement row selection → Inspector target + Meter/Waveform viz mount"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
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
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-04-selection-inspector-viz-design.md"
    - "docs/tasks/sv-gui-03-04-selection-inspector-viz.md"
    - "docs/test-specs/sv-gui-03-04-selection-inspector-viz.md"
  refines:
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice4 仕様: Arrangement row selection → Inspector target + Meter/Waveform viz mount

## What Should Be Built

Issue #112 の最終 headless slice として、(A) Arrangement `Table` の
row selection を view-local `ObjectSelections` と Inspector の
inspection focus に接続し、(B) `syn.viz.*` op 基盤経由で
`Waveform`/`Meter` component を実際に mount する。slice1-3 で確立
した driver-neutral 契約（RF387 View/Component・RF386 Host/Binding・
app-state push）の上に載せ、headless に検証可能な範囲で実装する。

- Arrangement table を selectable にし、row 選択は Ruviform 標準の
  view-local `Action` intent（`action = "select"`,
  `params = {"row": n}`、`VIEW_STATE_TARGET`）として発行する。
  domain target に params を持たせる経路は作らない。
- view-local action は新設 `syn.view.select` op に routing し、
  op が row index を `arrangement_rows` read model（table が
  render するのと同一の行順）で clip tagged id に解決して、
  `WorkspaceManager::select`（`ObjectSelections`、Clips domain）
  と `WorkspaceManager::inspect`（`InspectionFocus`）を更新する。
  Project/Undo/RT は触らない — view-local workspace state のみ。
- Inspector の表示 target は view の `inspection_focus.target`
  があればその semantic entity（Track/Lane/Clip/Source）の readout
  に rebinding し、無ければ従来の購読 param target
  （`InspectorBreakdown`）表示を維持する。selection と inspection
  は workspace 上で別 state のまま — 表示側だけが inspection に
  追随する。
- Arrangement タブに `Waveform`、Mixer タブに `Meter` の
  `ComponentSpec` を mount し、`syn.viz.bind`/`meta`/`query`/
  `frame`/`event` の既存契約で配線する。source id vocabulary は
  `synvelis-workspace::viz_ui` の既存 `waveform:clip-<tag>` /
  `meter:master` を読み取り利用する。
- viz payload は `WaveformRow`/`MeterRow` の status
  （`no source`/`awaiting data`/`stale`/source error/unsupported）
  をそのまま表示し、source 不在を empty success で偽装しない。
  bin 数は `MAX_WAVEFORM_BINS`/`MAX_METER_BINS` で bounded。

## Why Now

- Issue #112 の受入は Arrangement/Mixer/Transport/Inspector の実
  画面を求めており、slice2 spec の Non-Goals に「row
  selection→`ObjectSelections`→Inspector 接続」として明示的に
  切り分けられた最後の headless 範囲。これで GUI slice の
  headless 部分は完結する。
- `syn.viz.*` op 基盤（bind/meta/query/viewport/select/snapshot/
  register/push_frame/source_changed）と `CHANNEL_VIZ_FRAME`/
  `CHANNEL_VIZ_EVENT` 配線は既に存在するが、Meter/Waveform の
  ViewSpec mount は未実施だった — 残るのは mount と投影のみ。
- Ruviform pinned rev は `ComponentSpec::Waveform`/`Meter`・
  `TableSpec.selection`・`ViewHost::select_row`（optimistic
  selection + receipt settle/rollback）を実装済み。

## Users and Value

- ユーザーは Arrangement の clip 行を選択すると Inspector がその
  entity の readout に切り替わり、clip 波形と master meter が実
  `syn.viz.*` source の status 付きで表示される。fake rendering・
  第二の Project 状態・独自 gesture protocol は導入しない。

## Scope

- `crates/synvelis-app/` のみ:
  - `screens.rs`: selectable `TableSelectionSpec`、`Waveform`/
    `Meter` component + status label、`VizScreenState`/
    `WaveformScreenState`/`MeterScreenState`、`MainScreenState.viz`。
  - `binding.rs`: `ViewCommand.action_params`（view-local target の
    params のみ通す）。
  - `shell.rs`: `TABLE_SELECT_ACTION` → `syn.view.select` op への
    routing。
  - `service.rs`: `syn.view.select` op 登録・dispatch。
  - `ui_ops.rs`: `op_view_select`、`reconcile_viz_surfaces`、
    `capture_viz_state`、inspector entity readout、meter level
    history、`syn.view.list` の selection/inspection field。
  - `tests/`: headless 検証の追加。
- docs チェーン（本 spec・design・plan・task・test-spec・ir）。

## Non-Goals

- 実 GPUI 描画/pixel・IME・Focus・DPI・実 Window E2E（#114）。
- Dock UX・2 window・Theme（#113/#177 系）・PluginDock・
  RF#464 scenario。
- `synvelis-project`/`synvelis-workspace`/`synvelis-player` 等の
  他 crate の変更（workspace 側 API が不足する場合は domain op を
  追加せず範囲拡張を相談する）。
- selection の link propagation 先 view への波及 UI、複数選択、
  keyboard navigation、context menu。
- `syn.viz.*` op 自体の新設・既存 op 契約の変更。

## Acceptance Criteria

- Arrangement table が `selection: Some(TableSelectionSpec)` を持ち、
  `ViewHost::select_row` が `{"row": n}` params 付き `select` action
  を `VIEW_STATE_TARGET` に mint する。
- `select` action が shell → `syn.view.select` op に届き、op が
  row を `arrangement_rows` の clip tagged id に解決して
  `ObjectSelections`（Clips domain）と `InspectionFocus` を更新
  する。範囲外 row・未 attach/未登録 view・不正 params は
  `invalid_params`/`internal` で拒否し、部分適用しない。
- `syn.view.list` が view の `selection`/`inspection` tagged id を
  返す。
- 選択後の `MainScreenState.inspector.target` が選択 clip の
  tagged id になり、rows が entity readout（kind/id/source/lane/
  muted/track/start_s/end_s）になる。inspection が無い間は
  従来の param breakdown が維持される。
- `main_screen_spec` が Arrangement タブに `Waveform`、Mixer タブに
  `Meter` を含み、status label が `VizStatus` の表示文字列を運ぶ。
- `reconcile_viz_surfaces` が main view attach 中に waveform を
  選択 clip（fallback: 先頭 clip、無ければ unbound source）へ、
  meter を `meter:master` へ bind し、source 変更時に再 bind
  する。port 未登録時は `no source`、data 未着時は
  `awaiting data` を正直に表示する。
- meter level history が `syn.viz.frame` の landed frame から
  bounded に蓄積され、`Meter` spec payload（`<= MAX_METER_BINS`）
  に投影される。view detach/close・rebind で破棄される。
- 既存の View 契約（state key/TargetID/Revision/Generation、
  unmount/close/late receipt/Cancel）と Undo/RT 非回帰を
  回帰 test で維持する。
- `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `check-app-native` /
  `lint-app-native` / `build-app-native` が成功する。

## Open Questions

- なし（review-protocol で blocker なしを確認済み）。
