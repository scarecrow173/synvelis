---
id: "SPEC-rBmfzOP1H86N1KAXlFpX5N"
type: "spec"
status: "approved"
title: "MD-A — TakeGroup/Comp・Effective Comp 投影・ClipPlacement/Playback 時間契約 (headless model PoC)"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  depends-on:
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
---

# MD-A — TakeGroup/Comp・Effective Comp 投影・ClipPlacement/Playback 時間契約

## What Should Be Built

Issue #42: #10(Track/Lane/Clip/Source 統合 Project Model)と #12 §13/§15
(Recording Pass/TakeGroup/Comp Region/Transition・Effective Comp 投影)の
**合意済み設計**に基づく headless model PoC。合意済みだが未実装の
スキーマ領域を `synvelis-project` に実装する:

1. **TakeGroup / Comp Region / Comp Transition スキーマ** (#12 §13)
   — Take は Lane の録音候補、Comp は採用区間の組合せ。Comp Region
   (基本採用区間・順序付き・非重複) と Comp Transition (境界重複再生)
   を分離した独立 Stable ID オブジェクトとして実装する。
2. **Effective Comp 投影** (#12 §15) — 複数 Source から導出される
   Projection (単一 Source 正本ではない) としての再生表現と、
   `EditorDisplayTarget` (Effective Comp / Take Source / Compare) と
   `EditorEditingTarget` (Comp 構成 / Editable Source Version) の型区別。
3. **ClipPlacement / ClipPlayback 時間契約** (#10 §3.2) —
   `ClipPlacement { anchor: TimeAnchor, extent: ClipExtent }` の
   混合ドメイン正規化モデル。TempoMap 編集後に保存 Anchor を正本として
   位置を再導出し、元録音 Source の Sample Offset を変えない。
   `PlaybackMapping: Natural / Loop / SimpleStretch` の区別と検証。
   半開区間 `[start, end)` の契約を固定する。

## Why Now

VS-A..VS-D で Track/Lane/Clip/Source/Asset・Stable ID・Session/Pass/
Stream/Segment/Take・単一 Writer Command・Save/Reload が実装済み。
Take→TakeGroup→Comp→Effective Comp の採用系統と、#10 §3.2 の正規化
Placement/Playback 契約だけが未実装であり、Editor 統合 (§15) と
Lowering (#18) の前提となる契約を先に固定する。

## Users and Value

- DAW ユーザー: Punch/Loop 録音でも Raw Source が壊れず、採用区間の
  組替え・境界 Crossfade が非破壊で再現できる。Tempo 変更後も配置が
  意図どおり再解釈される。
- 開発者: Comp/Placement の正本・投影・検証境界が型と ops で固定され、
  後続の GUI/Lowering 実装が推測に依存しない。

## Scope

### In Scope

- `synvelis-project` への追加: `TakeGroupId`/`CompId`/`CompRegionId`/
  `CompTransitionId` Stable ID、TakeGroup/Comp/CompRegion/
  CompTransition POD、`CompRegistry`、参照整合検証。
- Comp 編集 ops (単一 Writer 経路・atomic・undo 対象):
  TakeGroup 登録/解除、Comp 作成/削除/active 切替、Region 追加/採用候補
  変更/範囲変更/削除、Transition 追加/調整/削除。
- Effective Comp 投影関数: 順序付き非重複 Region → Source Binding +
  Transition 重複再生 zone の導出。投影は純粋導出で Source を不変に保つ。
- Editor target 型: `EditorDisplayTarget` / `EditorEditingTarget` /
  `SourceVersionRef` (Raw Capture version 0 は編集対象に束縛不可)。
- 正規化 Placement/Playback: `ClipPlacement { anchor, extent }`・
  `ClipPlayback { source_region, mapping }` への `Clip` スキーマ移行、
  TempoMap 再導出関数、`SCHEMA_VERSION` bump、既存 consumer
  (commit 経路・lowering) の機械的適合。
- `crates/synvelis-project/tests/md_a.rs` に MD-A-* 受入テスト。

### Out of Scope

- 実 GUI の Comp 表示/編集、Take Lane UX、Audition。
- Comp MIDI Phrase の詳細・Note-level Comp・Controller Chase。
- Warp (marker 非線形 mapping) ・Time Stretch DSP・実再生 lowering の
  Loop/Stretch 対応 (契約と検証のみ; lowering は明示拒否)。
- Recording Commit 経路の再実装 (既存 VS-D を消費する側の機械的適合のみ)。
- `synvelis-workspace` 連携 (本 workspace には未存在; Editor target は
  project-model 側の型契約として実装)。
- Grouped Comp (複数 Track 一括採用) の一括 Command — 単一 Lane の
  Comp のみ。Take project placement (session→project 位置写像) の本格実装。

## Requirements

- R-1: `TakeGroup { owner_lane_id, target_range, take_ids, active_comp_id,
  source_rate }` が Lane の comp 編集単位として永続化できる。
- R-2: `CompRegion` は Project 半開区間・採用候補 (Take/既存 Source/
  Silence)・再生 Mapping を持ち、同一 Comp 内で順序付き・非重複を強制
  する。`CompTransition` は隣接左右 Region・境界・Overlap 区間・両
  Fade 形状を持つ独立 Stable ID オブジェクト。
- R-3: Comp/TakeGroup/Region/Transition/Clip の全変更が
  `ProjectController::apply(Command)` 経路で検証→atomic 適用され、
  undo で元 ID/状態を復元する。
- R-4: Effective Comp 投影が同じ Project 状態から常に同一結果を導出し、
  Segment の provenance (Take/既存 Source/Silence) と Source Frame 対応を
  保持する。投影生成は Source/Asset/Take を変更しない。
- R-5: Transition の Overlap は境界を内部に含み、左右 Region の範囲内に
  収まり、必要な Source Handle (左右それぞれの延長読み出し) が
  有効範囲内にあることを commit 前に検証する。不足時は拒否。
- R-6: `ClipPlacement` が `TimeAnchor = Musical | Absolute` と
  `ClipExtent = Musical | Absolute` の混合ドメインを保持し、
  `resolve` が TempoMap を介して Project media span と Source 読み出し
  対応を導出する。
- R-7: TempoMap 編集後、保存 Anchor を正本として Clip 位置が再導出され、
  `source_region` (Source Native Frame) は不変。
- R-8: `PlaybackMapping` が `Natural`/`Loop`/`SimpleStretch(ratio)` を
  区別し、必要な Source 消費長が window 内に収まることを検証する
  (Loop は window を繰り返し単位とする)。
- R-9: 全ての区間・境界が半開区間 `[start, end)` で評価され、
  Comp Region 境界で欠落も二重発火 (基本採用区間の重複) も起きない。
- R-10: `EditorEditingTarget` が `Comp(CompId)` と
  `EditableSource(SourceVersionRef)` を型で区別し、Raw Capture
  (version 0) を EditableSource として束縛できない。

## Acceptance Criteria

- MD-A-01: TakeGroup→Comp→Region/Transition が ops 経路で構築でき、
  投影が順序付き・非重複・provenance 付きの Effective Comp を再現可能に
  導出する。投影・save/reload・undo/redo で同一結果。
- MD-A-02: Region 非重複違反・Comp 跨ぎ参照・未隣接 Transition・
  Overlap が境界を跨がない/Region 外へはみ出す/Source Handle 不足の
  各ケースが validate で拒否され、部分適用が残らない。
- MD-A-03: Musical+Natural / Absolute+Natural / Musical+Stretch の
  Placement が TempoMap 変更を跨いで Anchor 正本に従い一貫して再導出
  され、Source Native Frame (source_region.start) が不変。
- MD-A-04: Comp 境界位置が常に右 Region の `[start, end)` に一意帰属し、
  基本採用区間の欠落/二重帰属がない。Overlap zone は Overlap 内に限定
  された重複再生として導出される。
- MD-A-05: Comp/Placement の全変更が undo で元 ID/状態を復元し、
  save/reload で同一スキーマが往復する。
- MD-A-06: `EditorEditingTarget` が Raw Source (version 0) の束縛を
  拒否し、Derived Version / Comp への明示 Binding を受理する。
- 既存スイート (VS-A..D・L1・L2・X) が全 green であること。
  `mise run fmt-check`/`check`/`lint`/`test` 全 green。

## Deferred Design Concerns

- Take の project placement (session→project 写像) — Comp Region の
  Take 候補は `source_offset` で対応区間を明示する PoC 契約。
- Source native format (rate/channel) の Source への保持 — MD-A は
  TakeGroup の `source_rate` を変換基準とする単一レート近似。
- Grouped Comp 一括 Command、Comp MIDI Phrase、Warp、Transition 実 DSP
  パラメータ (fade 長・形状係数) — 後続 Issue。

## Sources and Relations

- Issue #42 (本スライス) / #10 §3.2・§7 / #12 §13・§15 (設計合意)
- `docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md` (前提 Project Model)
- `docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md` (Take/Clip 生成元)
