---
id: "ADR-cyMsJVD4HiJceSTYXBJCJD"
type: "adr"
status: "accepted"
title: "MD-A — Clip 正規化 Placement/Playback スキーマ・Comp 採用モデル・Effective Comp 投影の境界"
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
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md"
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
  derives-from:
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# MD-A ADR — Clip 正規化 Placement/Playback・Comp 採用モデル・Effective Comp 投影境界

## Status

Accepted (2026-11-03)

## Context and Problem Statement

Issue #42 は #10 §3.2 (正規化 Clip モデル) と #12 §13/§15 (TakeGroup/
Comp Region/Transition・Effective Comp 投影) の合意済み・未実装スキーマを
headless model PoC として実装する。現行 `Clip` は F0 の簡略スキーマ
(`anchor: TimeAnchor` = 位置付き区間 `Musical|Media` + `source_offset`)で、
合意モデルの「配置 Anchor(点) + Extent(長さ) + Playback Mapping」の
分離がなく、`PlaybackMapping` の Natural/Loop/Stretch 区別も表せない。
Comp 採用系統 (TakeGroup/Comp/Region/Transition) は Project データに
一切存在しない。

制約: 単一 Writer (`ProjectController`)・単一 Store・全変更 undo 対象・
`deny_unknown_fields` 厳格スキーマ・Recording Commit 経路は再実装しない
(VS-D の消費側として機械的に適合させる)・DAW 固有スキーマを Ruviform に
持ち込まない。

## Decision Drivers

- 合意スキーマ (#10 §3.2・#12 §13.3・§15.2) をそのまま正本にする。
- 同一ドメインルールの重複実装を避ける (旧 anchor モデルと新 placement
  モデルの併存は認めない)。
- Source Native Frame 不変・Raw Source 保全を型で強制する。
- 検証を Project Controller の commit 前 validate に集約する。
- PoC で未確定な要素 (take の project placement・per-source rate) を
  明示的な近似として隔離する。

## Considered Options

1. **Clip を正規化モデルへ移行** (`placement+playback` へ置換・
   `TimeAnchor` 再定義) + CompOp/CompRegistry/投影を `synvelis-project`
   に実装。
2. 旧 `anchor`/`source_offset` を維持し、新契約型を併置する追加のみの
   実装 — 二重モデル・正本の分散を招く。
3. Clip 移行は見送り、Comp 系のみ実装 — 時間契約が実体を持たず
   MD-A-03 の受入が空転する。

## Decision Outcome

Option 1 を採用。

- `Clip { id, lane_id, source_id, placement: ClipPlacement,
  playback: ClipPlayback, muted }` に移行。旧 `TimeAnchor` (位置付き
  区間 enum) は `ProjectRange` に rename し Comp の project 区間型
  として再利用。新 `TimeAnchor = Musical(MusicalTick) |
  Absolute(MediaTimeValue)` は点 anchor。`ClipExtent = Musical |
  Absolute`、`PlaybackMapping = Natural | Loop | SimpleStretch
  (StretchRatio)` を追加。`SCHEMA_VERSION` を 2 に bump (旧 v1
  snapshot は version mismatch で明示拒否)。
- Comp 系 POD (`TakeGroup`/`Comp`/`CompRegion`/`CompTransition`/
  `CompRegistry`) と `CompOp` を `synvelis-project` に配置。Comp 編集は
  recording ではなく authoring 操作であり、`synvelis-recording` へは
  置かない (依存方向 `recording → project` を維持)。
- `CompCandidateRef = Take { take_id, source_offset } |
  ExistingSource { source_id, source_range } | Silence`。Take 候補は
  region 先頭→`source_offset` の明示対応 (take の project placement が
  F0 に無いための PoC 契約; §13.6 で「実型名は PoC」と明示済み)。
- `TakeGroup.source_rate` を media 時間→source frame 変換の基準とする
  単一レート近似 (Source が native format を持たないため)。
- Effective Comp 投影は `synvelis-project` の純粋導出関数
  (`project_comp`) として実装。単一 Source 正本に偽装せず provenance
  付き segment 列を返す。
- Transition の `boundary` は保存せず `left.end == right.start` から
  導出 (不変条件を構造的に保証)。Overlap は境界を内部に含む範囲のみ許可。

### Consequences

- 永続スキーマが変わる (v1 snapshot は読めない; `UnsupportedSchema` で
  明示失敗)。PoC 段階で migration 対象データなし。
- `synvelis-recording::commit` の clip 生成/検査・`synvelis-player::
  lowering` の anchor 解釈・各 crate テストの Clip 構築が機械的に
  書き換わる。manifest (`ClipEntry`) の wire 形式は変更しない。
- lowering は `PlaybackMapping::Natural` のみ実行し Loop/Stretch を
  明示 skip する (DSP 未実装を暗黙に天然再生しない)。

### Confirmation

- `md_a.rs` の MD-A-01..06 と既存スイート全 green、`mise run
  fmt-check`/`check`/`lint`/`test` で確認。

## Implementation Plan

task1: 正規化 placement 型 + Clip 移行 + consumer 適合 → task2: Comp
スキーマ/ops → task3: 投影 + editor target → task4: 受入/検証/記録。
詳細は `docs/plans/mda-comp-model-poc.md`。

## Verification

- `cargo test -p synvelis-project --test md_a` が受入契約を固定。
- `vs_d.rs`/`l1_e2e.rs`/`l2_e2e.rs`/`x_e2e.rs` が従来挙動を維持
  (Natural 写像で従来 anchor と同一解決結果)。

## Pros and Cons of the Options

### Option 1 (採用)

良い: 合意スキーマが唯一の正本。再導出契約が `Clip` 実体で検証可能。
悪い: consumer 適合コスト・スキーマ破壊。

### Option 2

良い: diff が小さい。悪い: 二重モデルが恒常化し正本が曖昧になる
(AGENTS.md「重複補正を認めない」に反する)。

### Option 3

良い: Clip 移行コスト回避。悪い: #10 §3.2 の時間契約が Clip に
接続せず、Issue 受入を満たせない。

## More Information

- #10 §3.2 (正規化 Clip モデル) / #12 §13.3 (Region/Transition 分離) /
  §15 (Effective Comp・Editor target) / §13.6・§15.9 (PoC 留保項目)
