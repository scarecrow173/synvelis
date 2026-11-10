---
id: "SPEC-8qSDBn4gud5ayYyjIQalpX"
type: "spec"
status: "approved"
title: "SV-GUI-03 slice2: Arrangement read-only Table mount + Undo/Redo 露出"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
    - "docs/designs/sv-gui-03-02-arrangement-undo-slice2-design.md"
    - "docs/tasks/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/test-specs/sv-gui-03-02-arrangement-undo-slice2.md"
  refines:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
  refined-by:
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice2 仕様: Arrangement read-only Table + Undo/Redo

## What Should Be Built

Issue #112（SV-GUI-03 Core Workspace GUI）の第 2 スライス。slice 1
（PR #199）で確立した `MainScreenState` 共有 cell + `ui_pump`
capture + `update_view` push パターンを延長し、Arrangement region
の read-only mount と既存公開 undo/redo の UI 露出を行う。
domain op の新規追加・`synvelis-project`/`synvelis-player` の変更・
Cargo pin の変更は行わない。

1. **Arrangement rows read model**:
   `synvelis-workspace` に `arrangement_rows(&ProjectSession,
   media_rate) -> Vec<ArrangementRow>` を実装する。track→lane→
   clip の構造化 walk（`Project.tracks/lanes/clips` の順序付き
   BTreeMap 走査）で各 clip の track name・lane・clip・source
   tagged id・start/end（秒）・muted を返す。start/end の解決は
   `timeline_items` と同一の内部ロジック（`TimeAnchor::Musical`/
   `Absolute`・`ClipExtent`・`TickTimeMapping`/`resolve_span`）を
   再利用し、秒表示規約（#175 確定）を維持する。read-side のみで
   domain/ops の追加はしない。
2. **Arrangement tab**: `screens.rs` に "Arrangement" タブと
   `Table`（Track/Lane/Clip/Source/Start/End/Muted 列）を mount
   する。`selection: None` の read-only table で、row selection→
   Inspector 接続は後続 slice。`MainScreenState` に arrangement
   rows を追加し、project 由来の表示 state として service op 後
   （`publish_project_length` 経路）に更新する。
3. **Undo/Redo**: `ProjectSession::undo`/`redo`（既存 pub）を
   `ControlsUi::undo`/`redo` 経由で呼ぶ `op_project_undo`/
   `op_project_redo` を app 層に追加し、`syn.project.undo`/
   `syn.project.redo` op として `AppService` に登録する。
   `project_action_op`（`binding.rs`）に `"undo"`/`"redo"` を追加し、
   toolbar に Undo/Redo `Button`（`project` target）を mount する。
   押下は既存の `project` Button → `project_action_op` →
   `dispatch_window_intent` → service op の経路をそのまま通る。
4. **headless 検証**: `tests/view_mount.rs` を拡張し、Arrangement
   table が mount され project revision 変化で rows が push される
   こと、Undo/Redo Button が op 経路を通ることを検証する。
   `tests/ui_ops.rs` に `syn.project.undo`/`redo` の op テストを
   追加する。fixture は `session.apply(ProjectOp::AddTrack/
   AddLane/AddClip)` を使用する。
5. **実機検証**: `run-app-native`（`synvelis-native`、feature
   `native`）で実 GPUI window が起動し Arrangement tab と
   Undo/Redo button が表示されることを Windows 実機で確認し、
   証拠を記録する。

## Why Now

Issue #112 は Arrangement/Mixer/Transport/Inspector の 4 region
の GPUI 標準 driver 化を要求するエピックであり、slice 1 が
Transport/Inspector の垂直 slice を確立した。本 slice は残る
Arrangement region の read-only mount と、domain 層に既に存在する
undo/redo を UI に露出する最小の垂直 slice である。
`Project.tracks/lanes/clips`・`timeline_items`・
`TickTimeMapping`/`resolve_span`・`ProjectSession::undo/redo`・
`ControlsUi::undo/redo`・`project_action_op` の project-target
Button 経路は全て既存であり、新規 domain op 無しで実現できる。

## Users and Value

- Synvelis 開発者/利用者: 実窓で Arrangement の track/lane/clip
  一覧（source・start/end 秒・muted）を確認でき、Undo/Redo
  button で project 履歴を操作できる。
- 後続 slice（row selection→Inspector、Meter/Waveform viz、
  Dock/Theme 等）: 本 slice が確立する arrangement rows の
  `MainScreenState` push と `syn.project.*` op 露出パターンを
  再利用できる。

## Scope

- `crates/synvelis-workspace/src/arrangement_ui.rs`（新規）:
  `ArrangementRow` 型と `arrangement_rows` read model。
  `crates/synvelis-workspace/src/viz_ui.rs`: `timeline_items` を
  `pub(crate)` 化（内部ロジック再利用のため）。
  `crates/synvelis-workspace/src/lib.rs`: export。
- `crates/synvelis-app/src/screens.rs`: "Arrangement" tab +
  `Table`、Undo/Redo `Button`、`MainScreenState` に
  `arrangement_rows` 追加。
- `crates/synvelis-app/src/app.rs`: `publish_project_length` を
  project 由来 state（length + arrangement rows）の publish に
  拡張（service.rs の呼出名維持）。
- `crates/synvelis-app/src/ui_ops.rs`: `op_project_undo`/
  `op_project_redo`（`ControlsUi::undo`/`redo` + `ui_pump`）。
- `crates/synvelis-app/src/binding.rs`: `project_action_op` に
  `"undo"`/`"redo"` 追加。
- `crates/synvelis-app/src/service.rs`: `syn.project.undo`/
  `syn.project.redo` の `OPS` 登録と dispatch arm 追加
  （Coordinator 確認済みの場合のみ — 未承認なら Undo/Redo は
  次 slice に切り分け PR 本文に明記）。
- `crates/synvelis-app/src/lib.rs`: 新規型の re-export。
- `crates/synvelis-app/tests/`: `view_mount.rs` 拡張 +
  `ui_ops.rs` op テスト。
- `crates/synvelis-workspace/tests/`: `arrangement_rows` 単体
  テスト。
- `docs/`: 本 slice の文書チェーン。

## Non-Goals

- row selection→`ObjectSelections`→Inspector 接続、Meter/Waveform
  viz mount、RF#464 scenario 接続（ruviform#464 契約側の整備中）。
- Dock/2 window（#113）、Theme（#177）、PluginDock（#176）。
- 実 OS E2E/IME/pixel 検証（#114/RF#313）。
- domain op の新規追加、`synvelis-project`/`synvelis-player` の
  変更、Cargo.toml/ruviform pin の変更。
- Arrangement table の編集操作（clip の追加/移動/削除 UI）—
  read-only mount のみ。

## Acceptance Criteria

- Arrangement 行生成: `arrangement_rows` 相当を synvelis-workspace
  に実装 — track→lane→clip の構造化 walk（name・source・
  start/end・muted を含む）。`timeline_items` の内部ロジックを
  再利用し、domain/ops の新規追加はしない
  （`ProjectSession::undo/redo` 呼出は既存 pub API の利用）。
- screens.rs に "Arrangement" タブ + `Table`（track/lane/clip/
  source/start/end/muted）を mount。`selection: None`。
- ui_ops.rs + MainScreenState で pump/revision 変化時に rows を
  更新し `update_view` push（slice 1 パターン踏襲）。
- Undo/Redo: `syn.project.undo`/`redo` op を
  `project_action_op` 経路で露出し、toolbar Button を追加
  （service 層の変更が必要な場合は Coordinator 確認。追加が
  大きければ Undo は次 slice に切り分け PR 本文に明記）。
- テスト: `view_mount.rs` 拡張（rows が mount され revision 変化
  で push される）+ `ui_ops.rs` テスト。fixture は
  `session.apply(ProjectOp::AddTrack/AddLane/AddClip)` を使用。
- 実 GPUI window で起動・表示を確認し実行証拠を PR に記録。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec）。

## Open Questions

- `syn.project.undo`/`syn.project.redo` の op 登録に
  `crates/synvelis-app/src/service.rs` の編集が必要（OPS +
  dispatch match）。Coordinator に確認中 — 未承認の場合は
  Undo/Redo を次 slice に切り分ける。
