---
id: "SPEC-034XI73iNvfO3GRXXim8Ph"
type: "spec"
status: "approved"
title: "TIM-03 driver-side loop-wrap boundary derivation"
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
  implemented-by:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  depends-on:
    - "docs/specs/tim-03-loop-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-03-loop-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-03-driver-loop-boundary.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 driver-side loop-wrap boundary derivation

## What Should Be Built

`HeadlessDriver` の `ProcessBoundarySession::stage_window` 呼出 3 箇所
（activation/resume anchor、通常 transport staging、seek re-anchor）に、
node の位置から導出した `BoundaryPoint { kind: LoopWrap, offset }` を
供給する。driver 側ではこれまで `boundaries: Vec::new()` で、loop wrap
という実在の transport discontinuity が oracle の legality 評価から
不可視だった — Issue #127 の driver-side GAP を埋める slice。

routing 側は先行 slice（`docs/specs/tim-03-loop-boundary.md`）で
`WrapSim` + `boundary_loop_wraps` により解決済み。本 slice は同一の
`pub(crate)` `WrapSim` を `AttachedDispatch` に保持し、driver の
観測窓ごとに 1 回だけ進めることで、routing 側の実装を複製せずに
driver 経路へ同じ導出を適用する。

## Why

- `TransportMirror` は lazy-fold で現在の loop region 下の位置しか
  再構成できず、mid-stream `SetLoop`/`ClearLoop`/`Seek` を跨ぐ
  scheduler `pos_at` を忠実に再現できない。wrap 導出には
  wrap ごとに再 anchor する scheduler-faithful な scalar 状態が必要。
- `WrapSim`（`process_boundary.rs`）は upstream `TransportScheduler`
  の `preflight`/`run_callback` と同じ `(status, anchor, loop)` 規則で
  walk し、apply 優先・same-head cascade・window-end 除外を再現する。
- oracle は caller 供給 point と command 由来 point を併せて評価する。
  wrap を供給しないと、wrap で分割される segment の合法性が
  評価対象から漏れ、degraded consent なしの exact delivery 契約を
  oracle が検証できない。

## Acceptance Criteria (this slice)

1. `AttachedDispatch` が `WrapSim` を保持し、`prepare_attached` で
   node の `frame_base` を使い `WrapSim::new` 相当で初期化する。
2. `observe_attached_block` が各観測窓を staged-apply リスト
   （全 driver command は `due = 0` head apply）で 1 回だけ進め、
   node cursor ドメイン（`sim_window`）で管理する — seek quiesce
   pass による driver cursor との一時的乖離を吸収する。
3. 3 箇所の `stage_window` 呼出全てに導出 `LoopWrap` point を
   `boundaries` 引数として供給する。preview は read-only clone で、
   live sim を二重に進めない。
4. `tests/tim03_driver_loop_boundary.rs` で、導出 offset が fake
   plugin の観測 span cut（実 scheduler の fold 位置軌跡から再構成）
   と一致すること、oracle が point を評価することを検証する。
   activation/anchor・通常 run・seek/re-anchor・SetLoop 変更・
   ClearLoop・window-end/head wrap・non-playing を覆う。
5. sessioned node の失敗は typed `BoundarySessionError` のまま
   fail-closed。`boundary: None` node は既存 lenient 経路を維持する。
6. routing 側（`routing_control.rs`・`tests/tim03_loop_boundary.rs`・
   先行 doc chain）の挙動は変更しない。

## Out of Scope

- 実 CLAP/VST3/AU plugin・実 Host・GUI-L2/EXT-L3・実機・
  precision certification/CapabilityMatrix・consumer oracle
  （AudioLens f64/native PCM 照合）— 別 gate として NotRun。
- routing 側 `boundary_loop_wraps`・node 側 span 分割機構・
  `ProcessCapability::evaluate` の変更。
- `stage_window` を command 非 staging の window にも毎回呼ぶ
  per-window 評価経路（現行契約は stage 時評価のまま）。
- Issue #127 tracker 全体の auto-close — 本 slice は `Refs #127` のみ。
