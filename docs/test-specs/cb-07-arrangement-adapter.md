---
id: "TSPEC-034XqB7wNvKtM2xYpL4d"
type: "test-spec"
status: "draft"
title: "CB-07 Arrangement 候補 Adapter — range 導出 ownership・mask・境界 inventory・adopt"
created: "2026-11-15"
updated: "2026-11-15"
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
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/test-specs/cb-03-midi-clip-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CB-07 Arrangement 候補 Adapter — range 導出 ownership・mask・境界 inventory・adopt

## Purpose

`VariantScope::Arrangement { tracks, time: MusicalRange, fields:
ArrangementDomainMask }` の候補が、宣言した Track 集合の lane
上で `time`（半開区間）と overlap する clip とその clip-local
note のみを編集・比較・採用できること、範囲をまたぐ配置や
absolute anchor が黙って scope に混入しないこと、削除を含む
候補が live を部分変更しないことを保証する。この spec は
Issue #284 の `CB-ARR-01`〜`CB-ARR-06` と `CB-ARR-RCP-01` の
headless 検証意図を固定する。実 OS/Plugin/RT/PCM/GUI
timeline の受入は対象外であり、それらの保証が必要になった
時点で別 test-spec が必要。

## Feature

Arrangement 候補 Adapter の range 導出 confinement・mask 適合
編集・境界依存 inventory・原子 adopt

## Rules

- Rule: scope 所有は `scope_objects` が正本 — scoped track の
  lane 上で `time` と overlap する musical 配置の clip、
  `fields.notes` 宣言時はその clip-local note と各 clip の
  control closure。absolute anchor/extent の clip・範囲外
  clip・非 scoped track の clip・source/asset/track/lane 構造は
  全て共有 context であり候補が書き換えない（CB-ARR-01/04）。
- Rule: 候補編集は宣言 mask の family に対応する op のみ受理
  する。clip 構造 op（`AddClip`/`RemoveClip`/`SplitClip`/
  `MoveClip`/`SetClipExtent`/`SetClipPlayback`/`SetClipMuted`/
  `RemoveClipsInRange`）は `fields.clips`、clip-local note op
  （`AddNote`/`RemoveNote`）は `fields.notes` を要求する。
  mask 未宣言 family は target の所在に関わらず `Unsupported`
  （capability report と enforcement の一致）、宣言済み
  family の scope 外 target は `ScopeViolation`、その他の
  `ProjectOp`/`CompOp` は `Unsupported` の型付き拒否であり、
  silent normalize や scope 拡大はしない（CB-ARR-01/02）。
- Rule: `SplitClip` は split 点以降の clip-local note を新
  clip へ再親付けする — `fields.notes` 未宣言の scope では
  その note が scope object でないため、移動を起こす split
  を `Unsupported` で拒否する（candidate と adopt の静かな
  乖離を防ぐ）。split 点より前の note のみを持つ clip の
  split は受理する。
- Rule: `AddClip` は scoped track の lane のみ、
  `RemoveClipsInRange` は scoped lane + `time` に内包される
  sweep range のみ受理し、生成/削除結果が scope 内に収まる
  ことを post-edit で検証する。より広い sweep は object-set
  diff が観測できない範囲外 clip を消し得るため拒否する
  （CB-ARR-04）。
- Rule: scoped clip が実体を保ったまま `time` を外れる編集
  （`MoveClip`/`SetClipExtent`/`SplitClip` の半分外れ）は
  scope-removal rule で `Unsupported` — range の再解釈による
  暗黙の scope 変更を許さない。in-range の実体削除
  （`RemoveClip`/`RemoveClipsInRange`/note 削除・cascade）のみ
  arrangement removal policy が受理する（CB-ARR-01/04）。
- Rule: scoped lane 上の非所有 clip は
  `boundary_dependencies` が `OutsideRange`（musical 非重複）
  または `AbsoluteAnchor`（absolute 配置 — tempo-map 変換
  policy 不在のため所有も編集もしない）として typed
  inventory する。境界を跨ぐ変更は shared-context digest を
  動かし比較 context を stale にする（CB-ARR-04/RCP-01）。
- Rule: `ArrangementCapabilities` の報告は enforcement と
  一致する — `Supported` は受理経路を持ち、`Unsupported`
  （absolute anchor・MIDI source・control closure 編集・
  mask 外 family）は型付き拒否される。削除系 capability の
  `Supported` は「候補として表現可能」を意味し、adopt
  materialization は slice 横断の no-removal 不変条件により
  型付き `Unsupported` で deferred される（CB-ARR-01/06）。
- Rule: adopt は宣言範囲の変更を 1 Project CAS/Undo で
  materialize し、対象外 track/clip/asset・共有 context を
  変更しない。stale scope digest・scoped lane の recording
  active・scope-removal candidate は typed reject で部分
  変更なし。branch-local で mint された clip/note ID は
  live-space で一意であり lineage が追跡される
  （CB-ARR-03/06）。
- Rule: adapter は自分の scope kind に対してのみ編集面を
  開く — engine が adapter と scope を独立に受け取るため、
  非 Arrangement scope への `edit_candidate` は
  `InvalidScope` で拒否し、arrangement 専用 removal policy
  が foreign scope へ漏れない。`boundary_dependencies` も
  `validate_scope` と同じ構造検証を先行し、malformed
  scope を typed error にする。
- Rule: `SetProjectPpqn` の musical-tick 再射影は persisted
  scope 宣言も対象 — `Arrangement.time` は
  `arrangement_scope` site として clip と同一 grid へ
  変換され、scope ownership・boundary inventory が PPQN
  変更を跨いで安定する（candidate payload の grid 整合は
  engine/apply 側の別課題 — Non-goals 参照）。
- Rule: tempo-map 変換を要する時間対応・section mapping
  不整合の Unaligned/Unknown diff verdict・Automation/
  Tail/Loop/Tempo の完全な影響域伝播解析は本 slice の対象外
  — 後続 slice/Issue の責務であり、headless PASS を実機・
  比較 orchestration の PASS に昇格しない（CB-ARR-05/RCP-01）。

## Examples

- Example: tracks={A,B}・time=[1000,20000) の候補で、A の
  lane 上の `[0,7680)`・`[15000,22680)` clip と B の lane 上の
  `[9600,17280)` clip が scope 所有となり、`[40000,47680)`・
  absolute 配置・foreign track の clip は共有 context のまま。
- Example: `fields={clips}` の候補で `SetClipMuted`・
  `MoveClip`（範囲内）・`SetClipExtent`・`AddClip`（scoped
  lane・範囲内）が受理され、`AddNote`/`RemoveNote`・
  `AddTrack`・`AddMidiNote`・`CompOp` は全て型付き拒否。
- Example: `MoveClip` で scoped clip を `time` の外へ運ぶと
  `Unsupported`、scoped lane へ `time` 外の `AddClip` は
  `ScopeViolation`、`time` をはみ出す `RemoveClipsInRange`
  は `ScopeViolation`、foreign lane を含む sweep も
  `ScopeViolation`。
- Example: `RemoveClip`+`RemoveNote` の削除候補は fork/edit/
  semantic diff/save-reload 可能だが、adopt と render 解決は
  `Unsupported` で拒否され live・revision・branch 状態は
  不変 — 部分 commit は発生しない。
- Example: scoped track の lane に recording session が open
  の間は adopt が `RecordingActive` で拒否され、foreign
  track の lane の session は gate しない。
- Example: `{clips}` と `{notes}` の mask variant は同一
  window で scope-conflict し、独立 track の disjoint scope
  同士は互いの編集・adopt に干渉しない。
- Example: `{clips}`-only scope で split 点以降に note を
  持つ clip の `SplitClip` は `Unsupported`、同じ split を
  full mask で行うと受理され adopt 後に note が新 clip
  へ再親付けされる。`{clips}`-only での `RemoveNote` は
  target の所在に関わらず `Unsupported`。
- Example: fork 済み arrangement scope を持つ project で
  `SetProjectPpqn`（960→480, Exact）を適用すると scope の
  `time` が `[500,10000)` へ変換され、boundary inventory
  の分類（`OutsideRange`/`AbsoluteAnchor`）が変わらない。
- Example: `ArrangementAdapter::edit_candidate` に
  `AudioClip` scope を渡すと `InvalidScope`、未知 track を
  指す scope の `boundary_dependencies` は `NotFound`。

## Guarantees

- range 導出 ownership と confinement・fingerprint・conflict・
  render pin・adopt plan が `scope_objects` 単一の真実を共有
  する。
- mask 外 family・foreign target・range 外配置・sweep 超過が
  型付き拒否され、silent widen/normalize がない。
- 非所有 clip が `OutsideRange`/`AbsoluteAnchor` で typed
  inventory され、false equivalence を作らない。
- capability 報告と enforcement が一致する。
- 削除候補を含む全 candidate で部分 commit が構造的に
  発生しない。

## Non-goals

- scope entity removal の adopt/render materialization
  （`Removed` write）— engine/apply 側の後続 slice。
- PPQN 変更と variant candidate payload の grid 整合 —
  content store 内 payload は `SetProjectPpqn` の walk 対象外
  であり、variant set 存在時の拒否または adopt 時再射影は
  engine/CB-01 側の設計判断（全 domain 共通）。
- tempo-map 変換を伴う absolute 配置の musical scope 写像、
  Automation/Tail/Loop/Tempo の完全な影響域伝播解析。
- Unaligned/Unknown を伴う section/tempo mapping diff —
  CB-08 比較 orchestration（#285）・cross-gate（#288）。
- 実 OS/Plugin/RT/PCM/render・実 timeline GUI（#114）。
- 3-way Semantic Merge・自動 Blend・set 内 N>2 branch の
  literal 同時 candidate。

## Risk

- mask/target 判定を `scope_objects` 以外で再実装すると
  range drift で mask 外波及・range 外所有を見逃す。
- removal policy が clip/note 以外の scope-owned kind を
  見落とすと scope 脱退が黙って通る — 現在は fail-closed で、
  `scope.rs` の kind 追加時に policy の再検討が必要。
- boundary inventory が clip のみを対象とするため、clip 以外
  の shared context（automation lane・send/connection 等）の
  境界は CB-STRIP 系 inventory の責務と重複しない範囲で
  別途検討が必要。
