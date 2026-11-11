---
id: "TSPEC-Zx5wCv8nBm2qKs6yHt4pLr"
type: "test-spec"
status: "approved"
title: "SV-GUI-03 slice4 テスト仕様 — Arrangement selection→Inspector + Meter/Waveform viz mount"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
    - "docs/designs/sv-gui-03-04-selection-inspector-viz-design.md"
    - "docs/tasks/sv-gui-03-04-selection-inspector-viz.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice4 テスト仕様

## 範囲

`crates/synvelis-app` の headless 検証。実 GPUI 描画/pixel・
IME/Focus/DPI・実 Window E2E は NotRun（#114 Acceptance、後続
検証）。`FakeDriver` + `ViewHost` の mount→wire→receipt 経路と
`MainScreenState` 投影を対象とする。

## ケース

### spec 形状（`view_mount.rs` / spec unit）

- `arrangement.table` が `selection: Some(TableSelectionSpec)` を
  持ち、target が `VIEW_STATE_TARGET`（`"view"`）。
- `main_screen_spec` が Arrangement タブに `Waveform`、Mixer タブに
  `Meter` の `ComponentSpec` と対応 status `Label` を含む。
  unbound 状態でも mount され、status は `"no source"`。

### selection → ObjectSelections → Inspector（`view_mount.rs`）

- clip-bearing project を `syn.project.open` で load →
  `push_screen_state` → `host.select_row(gen, "arrangement.table",
  0)` が `{"row":0}` params 付き `select` action を mint。
- wire 経路で `syn.view.select` op が `ok` Result を返し、
  `syn.view.list` の view entry が `selection`/`inspection` に
  clip tagged id を持つ。
- `MainScreenState.inspector.target` が clip tagged id、rows が
  entity readout（`kind=clip`/`id`/`source`/`lane`/`muted`/`track`/
  `start_s`/`end_s`）になる。
- `workspace.state(view-1)` の `object_selections.primary()` が
  `SemanticTarget::Clip`、`inspection_focus.target` が同 clip
  （op ack/`syn.view.list` 経由で観測 — workspace 直読みは
  app 内部なので wire 面で証明）。
- inspection が無い状態（attach 直後・project 空）では
  inspector が従来の param breakdown（`plain`/`normalized`/…）
  を維持する — fallback 非回帰。
- `select_row` の optimistic selection が reject で rollback
  する: 範囲外 row は `ViewHost` 側で emit されない、
  malformed `params.row` は `invalid_params` Result で view に
  返る（`dispatch_view_select` の防御経路は op 未到達 — wire
  上の直接 `syn.view.select` 不正 params で `invalid_params`/
  `internal` を確認）。

### viz mount と wire 契約（`view_mount.rs` / `ui_ops.rs`）

- attach 後の pump で `reconcile_viz_surfaces` が waveform を
  先頭 clip（または `waveform:none`）に、meter を `meter:master`
  に bind し、`syn.viz.meta` で surface row が観測できる。
- port 未登録では両 surface の status が `"no source"`、
  `WaveformScreenState.channel_bins`/`MeterScreenState.
  channel_levels` が空（捏造なし）。
- `syn.viz.register`（waveform: `rate`+`channels`、meter:
  `channels`）で port を登録 → `notify_source_changed` 経由で
  meta が再解決され `"awaiting data"` → `syn.viz.push_frame`
  （meter）/ bounded query 解決（waveform）で `"ok"` へ遷移し、
  `WaveformScreenState`/`MeterScreenState` に bounded bins が
  投影される（`<= MAX_*_BINS`、`channel_bins.len()==channels`）。
- `syn.viz.push_frame` の meter frame が `syn.viz.frame` notify
  と `MeterScreenState.channel_levels` の level bin に反映される
  （history は landed frame のみ）。
- clip 選択で waveform desired source が `waveform:clip-<tag>`
  に切り替わり rebind される（`syn.viz.meta` の `source` で観測）。
- detach/close で surface・meter history が破棄され、再 attach で
  再 bind される（既存 detach/close 契約の非回帰）。

### 非回帰

- 既存の selection 非依存ケース（transport/inspector param
  breakdown、arrangement rows、undo/redo、strip toggles、
  dock/window ops）が全て維持される — `mise run test` の
  既存 suite で担保。

## Non-Goals（NotRun）

- 実 GPUI `Waveform`/`Meter` 描画・pixel 比較、実 Window での
  selection/IME/Focus/DPI（#114 Acceptance）。
- 実 engine/audio port からの frame 駆動（headless は
  `ManualMeter`/`MemoryWaveform` fixture のみ）。
- Dock/2 window・Theme・PluginDock・RF#464 scenario。
- selection の link propagation が他 view の selection に与える
  観測（`ObjectSelections` の link 配信は workspace 既存契約で、
  本 slice は発行側のみ）。
