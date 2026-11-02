---
id: "TSPEC-034QAmjdN5ihez8HzDIvIs"
type: "test-spec"
status: "approved"
title: "ED-A 検証仕様 — Editing Model 合意5項目 headless 受入"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/ed-a-editing-model-design.md"
  derived-by: []
  verifies:
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ED-A 検証仕様 — Editing Model 合意5項目 headless 受入

## Purpose

Issue #11 合意 5 項目が `synvelis-workspace` + `synvelis-project` 上で
不変条件として守られることを固定する。検証本体:
`crates/synvelis-workspace/tests/ed_a.rs`。Fixture:
`CounterIdGenerator` + `FakeWindowHost` + `HitProbe` (意味的 probe) で
完全決定論。

## Feature: ED-A-01 Selection Model 分離

- 2 View の object selection / time selection / lane scope が互いに独立。
- `ObjectSelections` が domain 別に selection を保持: Clips domain の選択を
  残したまま Tracks domain 選択に切替え、戻ると Clip 選択が残る。
- active domain と command の required_domain が不一致なら `Unresolved`
  (Tracks domain が active で `clip.remove` は clip を消さない)。
- 混在 domain の一括 selection は `MixedDomains` で拒否。
- Primary Target は選択最後尾 = gesture anchor として明示保持。
- Time Selection のみの view で `clip.remove` は `Unresolved` (object
  selection がない)。object + time 並存時も remove は object のみ対象。
- `clips.delete-range` は origin view の musical time selection + lane
  scope から対象を導出する独立 command — 対象 lane の overlapping musical
  clip のみ除去、scope 外 lane の clip・media anchor clip は残る。

## Feature: ED-A-02 Tool 解決 + Gesture 固定

- Smart tool: Handle > Body > LaneEmpty > Background の優先順で解決
  (Body probe より Handle probe が優先される等)。
- Dedicated tool は明示固定 (Trim tool + handle hit → Trim、Select tool →
  Select)。
- Gesture 開始後、pointer が handle zone 上を通っても fixation の
  operation/targets は不変 (Move の途中で Trim に切替わらない)。
- 同順位候補 2 つの hit → `Ambiguous` で候補・suggested tool を返し
  gesture は開始しない。
- Slip/Stretch/Fade は解決できるが commit は `UnsupportedOperation`。

## Feature: ED-A-03 Snap / Grid / Time Domain

- `grid.visible = false` でも snap enabled は動作。
- `SnapUnit::Explicit` は zoom を変えても snap 位置が不変、`Auto` のみ
  zoom 追従。
- 複数 clip move: primary のみ snap され、他 clip には同一 delta → 相対
  位置保持。
- Musical + Media anchor 混在の move は gesture 開始時に
  `MixedTimeDomains` で拒否。
- Snap 採用候補は domain・targets で絞られ、winner の source+position が
  label で報告される。hysteresis 内の微移動で winner が入れ替わらない。

## Feature: ED-A-04 Gesture / Atomic Transaction

- Preview 中 (drag_update 複数回) は project revision・undo 履歴・
  `EngineStub.graph_builds` が不変。
- commit: 複数 clip move は 1 `apply` = 1 revision 進行、1 undo で全件
  復旧 (相対位置含む)。
- 1 対象の検証失敗 (存在しない clip 含む) → 全対象が commit されず
  project 不変 + 理由通知 (all-or-nothing)。
- Gesture 中に他経路で project が変更されると commit は `StaleRevision`
  由来で拒否、対象はすり替えられない。
- `cancel_gesture` (Esc/PointerCancel/WindowClosed/TargetDeleted/
  InputScopeLost) で project 不変・gesture 消去。
- `edit.undo`/`edit.redo` は InputContext::View で `session.undo/redo`
  (engine hook → `graph_builds` + revision 配信)、TextInput/Ime では
  text lane stub 計数で project 不変、Modal/PluginGui で Suppressed。

## Feature: ED-A-05 Cross-View / Linked Context 分離

- `open_in_editor` が `edit_binding {clip, source}` を設定するが
  両 view の object selection・linked_context は不変。
- `edit_binding` は command target に解決されない (binding だけの view で
  `clip.remove` は `Unresolved`)。
- Gesture 進行中に他 view が link event を publish しても gesture の
  fixed target は不変。
- read-only view (Inspector/SignalFlow) への link 配送は
  linked_context/inspection_focus のみ更新し project に影響しない。

## Guarantees

- Object Selection / Time Selection / Lane Scope は常に view-local で
  意味別の型として分離。
- Gesture は開始時の operation/targets/revision を必ず固定。
- Preview は project を変更しない; commit は 1 回の atomic apply。
- 失敗時は project 正本が不変で、理由が呼び出し側へ返る。
- Undo/redo は必ず `ProjectSession` の engine hook 経由。

## Non-goals

- px hit test・OS keybinding・pointer device・Magnetic mode・混合 domain
  演算・Note/Automation entity・Take/Comp・remote asset・Quantize。
- Slip/Stretch/Fade の実 commit 意味。
- ripple (gap-closing) 削除。

## Risk

- `EditingSelection` → `ObjectSelections` 移行で WS-A snapshot と非互換 —
  `WORKSPACE_SCHEMA` 2 で明示し、回帰は ws_a.rs の capture/restore test で
  担保する。
