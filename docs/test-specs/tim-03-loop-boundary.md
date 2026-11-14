---
id: "TSPEC-034XzNZjVRkpX8SwoPnSjs"
type: "test-spec"
status: "approved"
title: "TIM-03 Loop/Tempo boundary 自動導出 — テスト仕様"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
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
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/tim-03-loop-boundary.md"
    - "docs/designs/tim-03-loop-boundary.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  defers: []
  deferred-by: []
---

# TIM-03 Loop/Tempo boundary 自動導出 — テスト仕様

## Purpose

`boundary_loop_wraps` が `TransportMirror` + scheduled/
staged commands から導出する loop wrap `BoundaryPoint`
列が、実 `TransportScheduler` が観測する wrap frame と
一致することを保証する。spec/design の受入条件を
`tim03_loop_boundary.rs` で検証可能な形に固定する。

## Feature

Loop wrap boundary auto-derivation for process-boundary
preflight — upcoming window 内の loop wrap frame を
mirror/scheduler と同一 algebra で導出し、oracle の
`boundaries` 引数へ供給する。

## Rules

- **R1 wrap 導出**: Playing + loop 設定時、window
  `[call_start, call_start+call_frames)` 内で raw
  position が loop end に到達する frame を
  `BoundaryPoint{offset, LoopWrap}` として返す。
  offset は window-relative、`offset < call_frames`。
- **R2 apply 先行**: 同 frame の transport command
  apply は wrap より先に効く（upstream scheduler tie
  規則）。`SetLoop`/`Seek`/`ClearLoop`/`Play`/`Pause`/
  `Stop` の apply 後 state で wrap を再評価する。
- **R3 validity**: `Seek` は非負 `media_time_ticks`、
  `SetLoop` は `end > start` かつ start 非負のみ有効。
  無効 command は wrap 計算に影響しない。
- **R4 empty 条件**: loop 未設定・`Paused`/`Stopped`・
  loop region 不正（`end <= start`）で導出 empty。
- **R5 window 端**: offset `call_frames - 1`（最終
  interior frame）の wrap は導出、offset `call_frames`
  （window end ちょうど）は導出しない（次 window）。
- **R6 plan/tempo 不変**: plan install（tempo
  revision 変更を含む）は transport scalar state
  （status/anchor/loop）を変えないため wrap 位置は
  不変 — frame-linear position domain 前提を fixture
  で確認。
- **R7 合成**: 導出結果を `boundary_plan_window`/
  `boundary_stage_window` の `boundaries` へ渡すと
  oracle が LoopWrap を含む window を legality 検査
  する（command 由来 point と offset dedup）。
- **R8 provenance gating**: 未 attach `NoSurface`、
  pin 不整合 `StaleSurface`、control node 不在
  `NoSession` が typed error として返る。
- **R9 実 scheduler 一致**: 導出 offset 列が、同一
  command 列を流した実 `RoutingEngine`/
  `TransportScheduler` が plugin `TransportInfo` で
  観測する wrap frame と一致する。

## Examples

- **E1 steady loop**: loop `[100, 200)`、window
  `call_start=0, call_frames=256`、position 95 から
  Playing — wrap at engine frame 200 → offset 200。
  2 つ目の wrap は frame 300 → window 外。
- **E2 block-interior wrap**: loop `[90, 150)`、
  window `call_start=64, call_frames=128` — wrap at
  150 → offset 86（block interior、境界 0/1/100/last
  でない位置）。
- **E3 window-end wrap**: wrap がちょうど
  `call_start + call_frames` → offset `call_frames`
  は導出されない。
- **E4 last-frame wrap**: wrap が
  `call_start + call_frames - 1` → offset
  `call_frames - 1` で導出される。
- **E5 SetLoop mid-window**: window 中盤 due の
  `SetLoop` で旧 region wrap が消え新 region wrap
  が現れる（apply 後 state で再評価）。
- **E6 Seek mid-window**: window 中盤 due の `Seek`
  で wrap offset が再計算される。
- **E7 plan/tempo install 跨ぎ**: plan install
  （tempo revision 変更）を挟んでも wrap frame が
  不変（R6）。
- **E8 paused/stopped/no-loop**: 非 Playing または
  loop 未設定で empty（R4）。
- **E9 provenance**: 未 attach / pin 不整合 /
  node 不在で typed error（R8）。

## Guarantees

- 導出関数は `&self` pure read で wrap_sim clone のみ
  walk し、mirror/scheduled/staged/sim の実 state を
  変更しない。
- offset 昇順・同一 offset coalesce（cascade 含む）。
- `boundary_loop_wraps` は boundary session を要求
  しない（wrap sim は全 vended node で維持）。
- `LoopWrap` point は oracle の deferral scan を
  bound しない既存規則を維持（要求 frame のみ導出、
  実適用 frame は oracle/scheduler 責務）。

## Non-goals

- consumer 照合（AL f64/native PCM 出力）。
- 実 CLAP/VST3/AU・実 OS・実 GUI・L2/L3 受入。
- degraded deferral の実適用 frame 予測。
- `HeadlessDriver` 側の wrap 導出配線。
- plan install の scheduled tracking（現行は head
  apply で scalar state 不変のため不要）。
- precision certification/CapabilityMatrix 数値
  閾値。

## Risk

- mirror approximation（scheduler reject・out-of-band
  push を見ない）を継承 — receipts が generation
  軸を補正する契約は変更しない。
- staged due の絶対解決が window head 仮定に依存 —
  `call_start` が observe_block head と一致しない
  場合の誤導出リスクを doc comment で明示。
- fixture が実 scheduler と乖離した場合、oracle の
  legality 検査が誤った offset で実行される —
  R9 の一致検証がこのリスクを塞ぐ。
