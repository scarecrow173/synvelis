---
id: "DESIGN-8o9zUv4L5ae0FBB0zOM293"
type: "design"
status: "approved"
title: "SV-GUI-03 slice2 Arrangement/Undo 設計 — workspace read model + syn.project.* op 露出"
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
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice2 設計: Arrangement read-only Table + Undo/Redo

## Context

slice 1（PR #199）で `MainScreenState` 共有 cell・`ui_pump`
`capture_screen_state`・native lane の `update_view` push が確立
済み。`ComponentSpec::Table` は resolved rows を spec 内に持ち
GPUI で描画される。`Project.tracks/lanes/clips`（pub BTreeMap）、
`viz_ui::timeline_items`（private）、`ProjectSession::undo/redo`、
`ControlsUi::undo/redo`、`project_action_op` の project-target
Button 経路は全て既存。

## Goals / Non-Goals

**Goals**

- Arrangement tab に track/lane/clip/source/start/end/muted の
  read-only `Table` を mount し、project 変化で rows が更新される。
- Undo/Redo `Button` が `syn.project.undo`/`redo` op を通り、
  `ProjectSession::undo`/`redo` が呼ばれる。
- 既存 seek/fader/transport/inspector の契約を壊さない。

**Non-Goals**

- row selection→Inspector、viz surface mount、domain op 追加、
  `synvelis-project`/`synvelis-player` 変更、pin 変更。

## Design

### `arrangement_ui.rs`（synvelis-workspace、新規）

```rust
/// Arrangement table の 1 行 — track→lane→clip walk の read model。
#[derive(Clone, Debug, PartialEq)]
pub struct ArrangementRow {
    /// Track 表示名。
    pub track: String,
    /// Lane tagged id（lane に name は無い）。
    pub lane: String,
    /// Clip tagged id。
    pub clip: String,
    /// Source tagged id。
    pub source: String,
    /// Clip 開始（秒。span 未解決なら None）。
    pub start_seconds: Option<f64>,
    /// Clip 終了（秒）。
    pub end_seconds: Option<f64>,
    /// muted flag。
    pub muted: bool,
}

/// `session.project()` の track→lane→clip を順序付きで walk し、
/// 各 clip の秒 span を `timeline_items`（Seconds axis）で解決する。
pub fn arrangement_rows(session: &ProjectSession, media_rate: u32) -> Vec<ArrangementRow>
```

- `viz_ui.rs` の `timeline_items` を `pub(crate)` に変更し、
  `TimelineAxis::Seconds` で呼んで `payload.clip`（tagged id）を
  key に `(start, end)` の map を構築。Musical/absolute anchor・
  `TickTimeMapping`/`resolve_span` の解決ロジックは一箇所のまま。
- walk は `project.tracks.values()` → `track.lane_ids` →
  `lane.clip_ids` の順（永続化された表示順）。参照切れの
  lane/clip は `get` で skip（load 時 `validate_references` 済み
  だが、read model が panic しないことを保証）。
- span 未解決の clip は `start_seconds`/`end_seconds` = `None`
  で行を残す（timeline surface の `unresolved` カウントと異なり、
  table は全 clip を表示する — 欠落より "—" 表示が正直）。

### `screens.rs`

- `MainScreenState` に `pub arrangement_rows: Vec<ArrangementRow>`
  を追加（`ArrangementRow` は `synvelis_workspace` から import、
  `lib.rs` で re-export）。
- tabs を `[Arrangement, Transport, Mixer, Inspector]` に拡張
  （Arrangement を先頭・`active: 0` の既定表示に）。
  Arrangement tab child panel に `arrangement_table(rows)`:
  `ComponentSpec::Table`、columns `["Track","Lane","Clip",
  "Source","Start","End","Muted"]`、各行を `CellValue::Text`
  にマップ（start/end は `format!("{:.2}")` 秒 or `"—"`、
  muted は `"yes"`/`""`）、`selection: None`。
- toolbar に `project_button("project.undo", "Undo", "undo")`・
  `project_button("project.redo", "Redo", "redo")` を追加。

### `app.rs`

- `SEEK_MEDIA_RATE` を `pub(crate)` 化（ui_ops からも参照する
  場合に備え、現状は app.rs 内のみ使用 — arrangement rows の
  media_rate も同一定数を使う）。
- `publish_project_length` を拡張: `project_length_seconds` に
  加え `arrangement_rows = arrangement_rows(session, SEEK_MEDIA_RATE)`
  を同じ cell に書き込む。service.rs が op 毎に呼ぶ既存経路を
  そのまま使うため、`syn.project.new/open/save/reload/info` を
  含む全 op で rows が最新化される。doc comment を「project 由来
  screen state の publish」に更新（呼出名は service.rs 契約で
  維持）。

### `ui_ops.rs`

```rust
/// `syn.project.undo` — `ControlsUi::undo`（session.undo +
/// broadcast_all）を呼び、`ui_pump` で receipt/projection を流す。
pub fn op_project_undo(&mut self) -> Result<Value, ErrorBody>
/// `syn.project.redo` — 同上。
pub fn op_project_redo(&mut self) -> Result<Value, ErrorBody>
```

- 戻り値 `json!({ "undone": bool })` / `json!({ "redone": bool })`
  （履歴空は `false` で ok — 未実行を偽装しない）。
- `ControlsUi::undo`/`redo` の `Err` は `internal()` で
  `service_error` に写す。

### `binding.rs`

`project_action_op` に `"undo" => Some("syn.project.undo")`、
`"redo" => Some("syn.project.redo")` を追加。shell.rs の
`dispatch_project_action` は無変更で動く。

### `service.rs`（Coordinator 承認が前提）

- `OPS` に `syn.project.undo`/`syn.project.redo` の `OpInfo` を
  追加。
- `dispatch` match に `"syn.project.undo" =>
  self.app.op_project_undo()`、`"syn.project.redo" =>
  self.app.op_project_redo()` を追加。`call` 末尾の
  `publish_project_length` が arrangement rows を更新する。

### `bin/synvelis-native.rs`

無変更 — `MainScreenState` 全体の `PartialEq` 比較で
`arrangement_rows` の変化も `update_view` push に乗る。

## Risks / Trade-offs

- **`publish_project_length` の責務拡大**: 名前は狭いが
  service.rs の呼出契約を変えられないため本体を拡張。doc で
  「project 由来 state publish」と明記し、次に service.rs を
  触れる機会に rename を検討。
- **undo/redo の wire 到達性**: 現行 wire には project を変更する
  op が無い（param commit は既定 Read mode で書き込まない）ため、
  実機での undo 対象は将来の編集 op 追加待ち。op 自体は履歴空で
  `undone:false` を正直に返す。
- **`timeline_items` の `pub(crate)` 化**: private → crate 内
  共有への最小変更。公開 API 面は増えない。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` / `build`
- `mise run check-app-native` / `lint-app-native` / `build-app-native`
- `mise run run-app-native` で実 GPUI window 起動・Arrangement
  tab・Undo/Redo button 表示確認
- `tests/view_mount.rs` + `tests/ui_ops.rs` + workspace
  `arrangement_rows` 単体テスト
