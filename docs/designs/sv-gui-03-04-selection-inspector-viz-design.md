---
id: "DESIGN-Mx7pQr2wNz4kLs8tVb6yHd"
type: "design"
status: "approved"
title: "SV-GUI-03 slice4 設計 — row selection→syn.view.select→Inspector rebinding + syn.viz.* surface reconcile"
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
  implements:
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice4 設計: Arrangement selection → Inspector + Meter/Waveform viz mount

## Context

slice1-3 で確立した3層構造の上に載せる:

- **wire**: `rvf.ui.command.v1` intent → shell → `syn.*` op →
  `Result`/`Notify` → view binding の receipt/projection。
- **workspace**: `ViewState.object_selections`（`WorkspaceManager::
  select`）と `ViewState.inspection_focus`（`inspect`）は意図的に
  別 state。`SemanticTarget::{Track,Lane,Clip,Source}`、
  `EditingSelection::single` は既存 pub API。
- **viz**: `UiOpsState`/`VizUi` が `syn.viz.*` op と
  `WaveformRow`/`MeterRow`/`MeterFrame`/`WaveformView` を保持し、
  `waveform_clip_target`/`meter_master_target` の source id
  vocabulary が既存。`ViewHost::select_row` は optimistic
  selection で rejected/unsupported receipt 時に rollback する。

slice2 spec の Non-Goals にあった「row selection→
`ObjectSelections`→Inspector 接続」と、`syn.viz.*` 基盤の未 mount
だった Meter/Waveform の2件が本 slice の対象。

## Goals / Non-Goals

**Goals**

- table `select` action を既存 view-local 経路（`VIEW_STATE_TARGET`
  の params 許容）で `syn.view.select` op に届け、row→clip→
  `ObjectSelections`+`InspectionFocus` を authoritative に更新。
- Inspector 表示を `inspection_focus` の semantic entity readout
  に rebinding（param breakdown は fallback として維持）。
- `Waveform`/`Meter` を `syn.viz.*` surface に compare-and-bind で
  接続し、status/bins を bounded に投影。

**Non-Goals**

- 実 pixel/OS E2E、Dock/Theme/PluginDock、他 crate 変更、新規
  domain op、selection の link 伝播 UI、複数選択。

## Design

### `binding.rs`

`ViewCommand` に `action_params: Option<Value>` を追加。
`decode_view_command` の `action` arm で `w.params` を保持するが、
従来どおり `VIEW_STATE_TARGET` 以外の target に params があれば
`WireTranslateError`（domain target に params を持たせない契約は
不変）。gesture/cancel arm は `None`。

### `shell.rs`

`answer_view_state_action` を拡張: `TAB_SELECT_ACTION` は従来どおり
即 ack、`TABLE_SELECT_ACTION` は新設 `dispatch_view_select` が
`params.row`（u64→usize、欠落/不正は `invalid_params` で拒否 →
view の optimistic selection が rollback）を検証して
`Intent::new(command_id, "syn.view.select",
{"view_id": view, "row": row})` を `dispatch_window_intent` に流す。
結果は `observe_result`+`post_routed` で command_id 相関のまま
view に返る — `syn.view.select` は view→window 所有権を変えない
ため `ViewOp` watch 対象外でよい。

### `service.rs`

OPS 登録と dispatch に `"syn.view.select" =>
self.app.op_view_select(params)` を追加するのみ。

### `ui_ops.rs`（中核）

- `op_view_select(params)`: `view_id`/`row` を parse →
  `require_attached_view`（viz attach 済み = UI bind 済み）→
  `parse_view_instance`+`workspace.registry().contains` で登録
  view を確認 → `arrangement_rows().get(row)` で clip tagged id
  を解決（table が render する行順と同一 read model、行ズレなし）
  → `ClipId::from_tagged` → `workspace.select(inst,
  EditingSelection::single(SemanticTarget::Clip(clip)))` →
  `workspace.inspect(inst, target)` → `ui_pump()` → ack
  `{view_id,row,clip,selection,inspection}`。範囲外 row・未登録
  view・id 解決失敗は `invalid`/`internal` で拒否し部分適用しない。
- `reconcile_viz_surfaces`（`ui_pump` 内、`push_surface_frames`
  前）: main view が window attach 中のみ、
  - waveform: desired source = primary clip selection の
    `waveform_clip_target`、fallback = `arrangement_rows().first()`
    の clip、どちらも無ければ `"waveform:none"`（port 無し source
    id → `NoSource` を正直に表示）。`waveform_row(view,surface)`
    の `source` と比較し、不一致時のみ `bind_waveform`（再 bind は
    新規 bounded query）。
  - meter: `meter_master_target()` に同様に compare-and-bind。
    rebind 時は `meter_levels` を捨てて履歴を reset。
- `desired_waveform_source`: `workspace.state(main_view_instance())`
  の `object_selections.primary()` が `Clip` ならその source、
  非 Clip/無しなら先頭 arrangement clip。
- `capture_screen_state`: 末尾で `inspection_focus.target` を読み、
  `Some` なら `inspector.target = semantic_target_tag(target)` +
  `semantic_inspector_rows(project, target, &arrangement_rows())`
  （entity 不在は `state` 行で正直に報告）。`None` なら従来の
  param breakdown（strip toggle 除外込み）を維持。
  `screen.viz = capture_viz_state()` を追加。
- `waveform_screen_state`/`meter_screen_state`: `WaveformRow`/
  `MeterRow` を spec payload に投影。
  - axis 変換: `WaveformAxis::{Frames,Samples}` はそのまま、
    `Seconds` は `meta.sample_rate` 必須（未宣言なら payload 空 +
    status のみ — rate を仮定して frame 位置を捏造しない）。
    `Bins` は bin edge を frame に写し、`Samples` は
    `MAX_WAVEFORM_BINS/channels` 上限で min/max/rms/count を集約
    （`NaN` sample は gap = `count` から除外）。
  - `channel_bins`/`channel_levels` は `channels` に揃える
    （不足は空 vec、超過は truncate — spec の
    `len()==channels` 契約）。
  - status は `viz_status_text`（`view_stale` 優先、`detail` を保持）。
- meter level history: `push_surface_frames` が新 seq の meter
  frame を `syn.viz.frame` notify と同時に
  `ui.meter_levels[(view,surface)]`（`VecDeque`、
  `METER_HISTORY_CAP=256`）へ蓄積。`meter_screen_state` が
  `MAX_METER_BINS/channels` に収まる最新分を `MeterBin`
  （1 frame = 1 bin、`start=i,end=i+1`）に射影。
  detach/close/rebind で `meter_levels` を retain/remove。
- `op_view_list`: 各 view の `selection`/`inspection` に
  `semantic_target_tag` の tagged id（未設定/未登録は `null`）。
- free fn: `semantic_target_tag`、`semantic_inspector_rows`、
  `viz_status_text`。

### `screens.rs`

- `arrangement_table`: `selection: Some(TableSelectionSpec {
  target: VIEW_STATE_TARGET })`（従来 `None` = read-only から変更）。
- `WaveformScreenState`/`MeterScreenState`/`VizScreenState` を追加し
  `MainScreenState.viz` に格納。`Default` は空 bins +
  `status="no source"`（display convention の `sample_rate=48_000`・
  `channels=1` のみ、payload は捏造しない）。
- `waveform_panel`/`meter_panel`: status `Label` +
  `ComponentSpec::Waveform`/`Meter`。Arrangement タブに
  waveform、Mixer タブに meter を追加配置。

### `app.rs` / `lib.rs`

- `app.rs`: `arrangement_rows()`・`main_screen_cell()` は既存のまま
  利用（変更不要の見込み）。
- `lib.rs`: `VizScreenState` 等の新規 pub 型を export
  （`MainScreenState` の pub field 型のため必須）。

### 変更しないもの

- `synvelis-project`/`synvelis-workspace`/`synvelis-player` 他全
  crate、`Cargo.toml`、`ui/`、`.github/`、`mise.toml`。
- `syn.viz.*` op 契約、`ControlsUi`/`VizUi` の内部、
  `main_view_targets` 静的一覧、Undo/RT 経路。

## Risks / Trade-offs

- **row index ↔ clip tag の対応**: `arrangement_rows` が track/lane
  の persisted order で skip 規則（lane/clip lookup 失敗を skip）を
  持つため、table rows と `get(row)` は同一 Vec を見る限り常に一致。
  project open/undo で行数が変わると view 側の optimistic
  selection は receipt 前に stale になり得るが、`select_row` は
  現在の spec rows で bounds 検証するため範囲外は emit されない。
- **inspection_focus の持続**: clip 削除後も `inspection_focus` は
  残り、entity readout は `state` 行で正直に「not in project」を
  表示する（stale target を消す責務は workspace の inspection
  契約側 — 本 slice では表示の正直さのみ保証）。
- **meter history の表示専用性**: `meter_levels` は
  `MainScreenState` ではなく app 内部の表示キャッシュで、
  project state・undo・RT に影響しない。frame 喪失は `dropped`
  bookkeeping 側の契約であり history は latest-wins で良い。
- **waveform rebind の頻度**: desired source は selection/先頭
  clip のみで決まり、compare-and-bind なので通常は不変。clip
  増減で先頭が変わる場合のみ rebind（新規 bounded query、
  `syn.viz.bind` 契約の範囲）。

## Verification

- `tests/view_mount.rs`: selectable spec 形状、select_row→wire→
  `syn.view.select`→`ObjectSelections`/`inspection_focus`→
  `inspector.target`/rows、`syn.view.list` fields、param
  breakdown fallback、waveform/meter mount + status、
  `syn.viz.register`+`push_frame` での data 経路、reject rollback、
  範囲外 row 拒否。
- `tests/ui_ops.rs` または unit: `op_view_select` の param 検証・
  未登録 view 拒否、screen state 投影。
- mise canonical 検証一式（fmt-check/check/lint/test/test-rt/
  test-msrv/check-app-native/lint-app-native/build-app-native）。
