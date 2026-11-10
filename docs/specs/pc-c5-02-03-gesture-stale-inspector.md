---
id: "SPEC-034UxXLvNqgbDGOypo7Uu4"
type: "spec"
status: "approved"
title: "PC-C5 slice2 (UI-C5-02/03): Gesture 固定・stale drop・Inspector Follow/Explore/Pinned（headless）"
created: "2026-11-09"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
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
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c5-02-03-gesture-stale-inspector-design.md"
    - "docs/plans/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/tasks/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/test-specs/pc-c5-02-03-gesture-stale-inspector.md"
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

# PC-C5 slice2 (UI-C5-02/03): Gesture 固定・stale drop・Inspector Follow/Explore/Pinned（headless）

## What Should Be Built

synvelis#102 の UI-C5-02 / UI-C5-03 を headless（fake 検証）で実証する slice。
`synvelis-workspace` に以下を追加・拡張する。

1. **Control Layer Edge gesture**: Control Layer 上の接続エッジ（modulation source → parameter）
   の drag を、Audio Routing（clip/lane の配線操作）とは型レベルで区別された gesture として
   Begin / Preview / Commit / Cancel の固定 semantics で扱う。commit は `ProjectOp` 経由で
   `ProjectSession` にのみ到達し、undo 境界は実 project 変更があった場合のみ発生する。
2. **Inspect click ≠ Edit**: Inspector/Signal Flow 上の inspect 意図（対象の観測・
   フォーカス移動）は editing selection・project・undo を一切変更しないことを fixture で証明する。
3. **Inspector mode semantics**: `InspectionMode` に `Explore` を追加し、
   Follow / Explore / Pinned の遷移と Inspector 所有の follow source・explore 履歴を
   `ViewState` 上にモデル化する。link 由来の追従は follow source が一致する view の
   Follow mode にのみ適用され、editing selection は変更しない。
4. **Stale drop / retarget 防止**: gesture fixation は Begin 時に target・project revision・
   view generation（window epoch）を固定し、2-window・dock transfer・focus 変更・
   selection 変更を挟んでも commit/cancel が別対象にすり替わらないこと、stale な
   generation/revision の結果が drop されることを fixture で証明する。

## Why Now

PC-C5-01（PR #211）で Relation Inspector の read model は main 入り済みだが、
Issue #102 の UI-C5-02/03（gesture 固定・stale drop・Inspector mode）は未実装。
Inspector semantics の正本は Issue #5 が定める Follow/Explore/Pinned であり、
現行 `InspectionMode` は Follow/Pinned のみで Explore と follow source の概念がない。
Control Layer の edge gesture は ControlsUi の parameter gesture（Begin/Preview/Commit/Cancel、
generation 照合済み）とは別経路であり、Audio Routing と混同されない型付き契約が必要。

## Users and Value

- ユーザー: Control Layer / Inspector を操作する DAW ユーザー。
- 価値: edge drag が routing と誤認されない、inspect click が編集を起こさない、
  undo が preview/cancel で汚れない、window/focus/dock 変化で gesture 対象が
  すり替わらない、という信頼性を headless fixture で保証する。

## Scope

### In Scope

- `synvelis-workspace`: `InspectionMode::Explore`、`FollowSource`、`InspectionFocus`
  の follow source / explore 履歴、link deliver の mode・source 照合、
  inspector navigation API（inspect/mode/follow source/explore back-forward）、
  Control Layer edge gesture（fixation/preview/commit/cancel・stale drop）。
- `synvelis-workspace/tests/`: UI-C5-02/03 を網羅する fixture。
- `docs/`: spec/design/plan/task/test-spec/impl の文書チェーン。
- `synvelis-app`: 変更なし（native 静的検査で非回帰のみ確認）。

### Out of Scope

- UI-C5-04（Recording Conflict/Retry/Rebase/Abort）— 後続 slice。
- UI-C5-05（実 OS GUI/Plugin window/high-DPI/IME/a11y）— NotRun。
- `synvelis-project` の実装変更 — 既存 read-only projection / command adapter の消費のみ。
- GPUI ViewSpec への inspector mode 配線（#112/native slice で実施）。
- Audio Routing 自体の gesture 変更 — 既存 `gesture.rs`/`edit.rs`/`tool.rs` は変更しない。
- ControlsUi（parameter gesture）の変更 — 既存 generation 機構をそのまま利用。

## Requirements

- Control Layer edge gesture は `ModulationSource → ParameterTarget` の connect のみを
  表現し、Audio Routing（clip/lane 配線）とは別型・別 API とする。
- Gesture は Begin で target・`ProjectRevision`・view の window epoch を固定する。
  Preview は project を変更しない。Commit は固定 revision で `ProjectOp` を適用し、
  実変更があれば revision を返し undo 境界を 1 つ作る。Cancel は project を変更しない。
- Window 移動（dock transfer）・focus 変更・selection 変更は実行中 gesture の対象を
  すり替えない。Begin 後に window epoch が変わった commit は stale として drop する。
- Begin 後に project revision が進んだ commit は CAS 失敗として drop する
  （expected revision は fixation が保持する）。
- Inspect 操作は `InspectionFocus` のみを更新し、`object_selections`・project・
  undo・`revision_inbox` を変更しない。
- `InspectionMode::Explore` 中は link 由来の追従で target がすり替わらない。
  Explore の戻る/進むは Inspector 内履歴のみを辿り、editing selection に触れない。
- Follow mode でも link deliver は `follow_source` が一致する view にのみ適用する。
- Pinned target が削除された場合は unresolved を表示し、別対象へ暗黙に切り替えない。
- UI selection（`object_selections`）と Command Target（gesture fixation /
  `CommandContext.explicit_targets`）は型レベルで区別される。

## Acceptance Criteria

- [ ] control edge Begin→Preview→Commit で `AddModulationAssignment` が 1 回だけ
      project に適用され、undo 1 回で取り消せる。Preview/Cancel は project revision・
      undo を変えない。
- [ ] control edge gesture は Audio Routing gesture（`gesture.rs` の `Gesture`）と
      独立しており、互いの commit/cancel が混線しない。
- [ ] inspect click 相当の API 呼び出しで `object_selections`・project revision・
      undo 履歴が不変であることを fixture が証明する。
- [ ] Follow/Explore/Pinned の遷移、follow source 照合、explore back/forward が
      fixture で証明される。Explore/Pinned 中の link deliver は target を変更しない。
- [ ] multi-selection（per-domain selection）が inspect/mode 変更で失われない。
- [ ] 2-window 間の focus 移動・dock transfer・selection 変更を挟んでも実行中
      gesture の commit が fixation 対象にのみ適用される。
- [ ] detach/attach・window 移動後の stale commit/cancel・古い generation の
      receipt/projection が drop されることを fixture が証明する。
- [ ] Pinned target 削除後も target が別対象に置き換わらない（unresolved）。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` が pass。
      `mise run check-app-native` / `lint-app-native` が pass（synvelis-app 非回帰）。

## Deferred Design Concerns

- GPUI ViewSpec/Binding への inspector mode・control edge gesture 配線 | 実 OS GUI が
  必要で本 slice は headless 限定 | #114 / UI-C5-05 の native gate | 配線なしでは
  実画面で mode が見えない（headless fixture で semantics は保証済み）
- Audio Routing edge gesture 自体の Begin/Preview/Commit | 本 slice は Control Layer
  edge のみ対象。routing 側は既存 `gesture.rs` が担う | 別 slice | routing gesture の
  固定 semantics が未検証のまま残る
- ControlsUi parameter gesture と control edge gesture の統合 | 経路が異なる
  （LogicalTarget vs typed endpoints）ため本 slice では分離 | 将来の統合検討時 |
  二重 gesture 管理の複雑さ

## Sources and Relations

- synvelis#102（UI-C5-02/03 の受入条件の出典）
- synvelis#5（Context Inspector semantics: Follow/Explore/Pinned・follow source・
  editing selection 分離・late receipt drop の正本）
- synvelis#108（Signal Flow graph/port・Inspector 統合）
- PC-C5-01（`docs/specs/pc-c5-01-relation-inspector.md` — Relation Inspector read model、
  本 slice の前提）
- `crates/synvelis-workspace/src/{state,link,gesture,edit,command,session,controls_ui,relation_ui,workspace}.rs`
- `crates/synvelis-project/src/control.rs`（`ModulationSource`/`ParameterTarget`/
  `ModulationAssignment`/`ProjectOp::AddModulationAssignment` の read-only 消費）
