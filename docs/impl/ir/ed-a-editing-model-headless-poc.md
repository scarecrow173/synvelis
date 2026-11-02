---
id: "IMPL-034QAWvNQYPna11TqH7YTn"
type: "impl"
status: "completed"
title: "ED-A editing model headless PoC"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/38"
    - "https://github.com/scarecrow173/synvelis/issues/11"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/selection.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/tool.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/snap.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/gesture.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/edit.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/ed_a.rs"
      - type: "file"
        path: "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
      - type: "file"
        path: "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
      - type: "file"
        path: "docs/designs/ed-a-editing-model-design.md"
      - type: "file"
        path: "docs/plans/ed-a-editing-model.md"
      - type: "file"
        path: "docs/tasks/ed-a-task1-selection-model-project-ops.md"
      - type: "file"
        path: "docs/tasks/ed-a-task2-tool-gesture-model.md"
      - type: "file"
        path: "docs/tasks/ed-a-task3-snap-edit-commit.md"
      - type: "file"
        path: "docs/tasks/ed-a-task4-commands-workspace-api.md"
      - type: "file"
        path: "docs/tasks/ed-a-task5-acceptance-tests.md"
      - type: "file"
        path: "docs/tasks/ed-a-task6-verification-impl-record.md"
      - type: "file"
        path: "docs/test-specs/ed-a-e2e-editing-model.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/command.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/error.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/persist.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/ws_a.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ed-a-editing-model.md"
    - "docs/tasks/ed-a-task1-selection-model-project-ops.md"
    - "docs/tasks/ed-a-task2-tool-gesture-model.md"
    - "docs/tasks/ed-a-task3-snap-edit-commit.md"
    - "docs/tasks/ed-a-task4-commands-workspace-api.md"
    - "docs/tasks/ed-a-task5-acceptance-tests.md"
    - "docs/tasks/ed-a-task6-verification-impl-record.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/designs/ed-a-editing-model-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ed-a-e2e-editing-model.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ED-A editing model headless PoC

## Summary

Issue #38 — issue #11 で合意された editing model の headless PoC —
を WS-A 基盤 (`crates/synvelis-workspace` + `crates/synvelis-project`)
上に実装した。5 契約 (ED-A-01..05) を受入 test 6 件
(`tests/ed_a.rs`) で検証。`EditingSelection` 単一 vector から
domain 分離 `ObjectSelections` + `TimeSelection` + `LaneScope`
への移行を含み、`WORKSPACE_SCHEMA` は 1 → 2。

## Context

Spec: docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md
ADR: docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md
Design: docs/designs/ed-a-editing-model-design.md
Plan: docs/plans/ed-a-editing-model.md / Test spec:
docs/test-specs/ed-a-e2e-editing-model.md。

## Implementation

### Changes

- `synvelis-project/src/ops.rs`: `ProjectOp::Composite` (空・入れ子
  拒否、scratch 上で逐次 validate→apply、中途失敗は scratch ごと
  破棄されるため atomic)、`SplitClip` (musical anchor 限定、
  `clip_b_source_offset` は caller 供給 + 単調性検証)、
  `RemoveClipsInRange` (lane 集合 × `MusicalRange` overlap 除去、
  media-anchored clip は別 domain として不変)。
- `synvelis-workspace/src/selection.rs`: `SelectionDomain`
  (Tracks/Lanes/Clips/Sources)、`ObjectSelections` (active domain +
  domain 別 map、混在 selection を `MixedDomains` で拒否、
  `activate` は domain 切替のみ)、`TimeSelection` (Musical|Media)、
  `LaneScope`、`EditTimeDomain`、`EditBinding` (display binding、
  command target ではない)。
- `tool.rs`: `Tool` (Smart + 9 dedicated)、`HitZone`
  (Handle > Body > LaneEmpty > Background の明示 priority)、
  `HitProbe`/`HitCandidate` (意味 probe)、`EditOperation`、
  `resolve_tool` — Smart は最良 zone の候候補を採用、同順位複数は
  `Ambiguous` で候補+ suggested dedicated tool を返す。dedicated
  は smart ladder を通らず互換 candidate がなければ `NoGesture`。
- `snap.rs`: `GridConfig` (表示のみ) と `SnapConfig` (独立) を
  分離。`SnapUnit::Explicit` は zoom 非追従、`Auto` のみ
  caller 供給 `auto_step` に追従。`resolve_snap` は domain
  filter → hysteresis → threshold → 距離 → `SnapSource` priority
  (`ClipBounds` > `Playhead` > `Grid`) の決定的順序で winner を
  選び、`label()` で source+position を報告。
- `gesture.rs`: `Gesture` = `GestureFixation` (operation/targets/
  revision/domain/origin、不変) + `GesturePreview` (position/
  delta/snap、mutable)。`#[serde(skip)]` で非永続。
- `edit.rs`: `gesture_domain` (clip anchor 由来の共有
  `EditTimeDomain`、混在は `MixedTimeDomains` 拒否)、
  `build_commit` (Select/Marquee → selection 更新、Move/Trim/
  Erase → `ProjectOp` 単一または `Composite`、Split/Draw/Slip/
  Stretch/Fade → `UnsupportedOperation`)。primary のみが snap し、
  delta は全 clip に同値適用 (相対 offset 保持)。
- `state.rs`: `ViewState.editing_selection` → `object_selections`
  + `time_selection`/`lane_scope`/`active_tool`/`grid`/`snap`/
  `edit_binding`/`gesture` 追加。
- `command.rs`: `CommandSpec.required_domain` (selection 由来
  target の domain gate)、新 command `edit.undo`/`edit.redo`
  (`CommandKind::Undo`)、`clips.delete-range`、`tool.activate`。
- `workspace.rs`: editing API (`pointer_down`/`drag_update`/
  `commit_gesture`/`cancel_gesture`/`set_tool`/`set_time_selection`/
  `set_lane_scope`/`open_in_editor`/`set_input_context`)、undo lane
  配送 (View→session.undo/redo + revision 配信、TextInput/Ime→
  `text_lane_undos` stub、Modal/PluginGui→Suppressed)、
  `select` の `ObjectSelections` 対応。
- `persist.rs`: `WORKSPACE_SCHEMA` = 2。
- `tests/ed_a.rs`: ED-A-01..05 受入 test 6 件。
- `tests/ws_a.rs`: `object_selections.assign`/`primary` API へ追従。

### Decisions During Implementation

- `ObjectSelections` は domain 別 map + active domain とし、
  `ws_a.rs` は `assign` (domain 推論) で最小差分に留めた。
- `Composite` 内では各 op を scratch 上で validate→apply する。
  controller が `Composite.validate` を先に呼ぶため二重検証だが、
  逐次依存 (後続 op が前段結果に依存) を許す構造にした。
- `SplitClip` の `clip_b_source_offset` は tick→source-frame の
  rate mapping が未 model のため caller 供給とした。workspace の
  Split gesture はこの mapping を持たないため commit を
  `UnsupportedOperation` で返す (op 自体は project API として検証済)。
- `command.rs` の required_domain check は selection 由来
  resolution のみに適用し、explicit targets は既存の
  kind-mismatch→Unresolved 経路に委ねた (context menu 対象指定を
  阻害しない)。
- `edit.undo`/`edit.redo` は `CommandKind::Undo` として suppress
  判定より先に input context で分岐 — TextInput/Ime は project
  history ではなく text lane stub に落とす。

## Related Experiments

### Adopted

なし — 承認済み設計どおりに実装。

### Rejected

- `SplitClip` の source offset を tick delta で近似する案 —
  意味的に誤りのため `UnsupportedOperation` を返す側を採用。

## Validation

| 項目 | 結果 |
| --- | --- |
| `mise run fmt-check` | PASS |
| `mise run check` | PASS (workspace --all-targets) |
| `mise run lint` | PASS (`cargo clippy --workspace --all-targets -- -D warnings`) |
| `mise run test` | PASS (`cargo test --workspace --locked` 全 crate green。ed_a 6 件 + ws_a 8 件含む) |
| ruviform 依存変更 | なし (`Cargo.lock` ruviform rev `94c5bf8` のまま) |

受入対応: ED-A-01 `ed_a_01*` (per-view/per-domain 独立性、
primary、混在拒否、domain gate→Unresolved、time selection 非
破壊、`clips.delete-range` の scope/domain 限定)、ED-A-02
`ed_a_02_*` (Handle>Body>LaneEmpty priority、dedicated 固定、
Ambiguous+suggested tool、fixation 不変、Slip commit 拒否)、
ED-A-03 `ed_a_03_*` (grid 非表示でも snap、Explicit 非 zoom 追従
/Auto 追従、primary-only snap + relative offset、mixed domain
拒否、winner label、hysteresis)、ED-A-04 `ed_a_04_*` (preview が
revision/graph_builds/undo を不変、1 apply=1 undo、
all-or-nothing、stale 拒否、cancel 各種、undo lane 配送)、
ED-A-05 `ed_a_05_*` (edit_binding 非 command target、gesture 中
link event 非破壊、linked_context 分離)。

### 既知の repo 健全性(既存・本スライス非因果)

- WS-A と同じく `doc_status audit` は architecture/vs-c 系の
  既存 findings を含む可能性がある。ED-A 成果物についての
  新規 blocker は self-review で解消済み。

### 未検証(正直な記録)

- 実 hit-test 形状・px threshold・drag 閾値・hover・pen/touch・
  OS keybinding・実 GUI: 対象外 (`HitProbe` 意味 probe のみ)。
- Slip/Stretch/Fade/Split/Draw の commit 意味: resolve まで、
  commit は `UnsupportedOperation` (rate mapping / source binding
  未 model)。`SplitClip` op 自体は project API として実装済。
- Media anchor clip の move/trim: domain 分離のため Musical のみ
  実装 (media-only gesture は Unsupported)。混合 domain の共通
  演算契約は issue #11 の deferred 項目。
- Magnetic mode / 実 hysteresis の device 挙動: 型と deterministic
  keep-window のみ。
- `mise run test-msrv`: rust 1.89 toolchain 未導入のため未実施
  (pinned 1.99.0 で全検証)。
- `clips.delete-range` は gap-closing shift (ripple) なしの範囲除去。

## Risks

- `WORKSPACE_SCHEMA` 2 で schema 1 snapshot は読めない —
  PoC 間の意図的破壊的変更であり移行経路は提供しない。
- `remove_view`/window `ReleaseViews` は gesture を state ごと
  破棄する — cancel hook としては十分だが、gesture 中の view
  destroy に `WindowClosed` の記録は残らない (観測ポイント)。
- `revision_inbox` は引き続き無制限ログ (WS-A 既知)。

## Follow-ups

- Marquee の domain: 現在は Musical 固定。Media domain の range
  select は domain 導出の一般化が必要。
- `ClipBounds` snap candidate の収集 (他 clip の edge 列挙) は
  caller 提供 `SnapEnvironment` の責務 — view 実装層で供給を組む。
- Note/Automation entity、Take/Comp、remote asset、実 GUI lane
  (ruviform #243) は継続対象外。
