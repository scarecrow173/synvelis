---
id: "TSPEC-FqOD6GcN3rNgRT08cdU9qO"
type: "test-spec"
status: "approved"
title: "SF-04 loop-discontinuity テスト仕様 — OpName wire + loop→probe 不連続宣言"
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
  implements:
    - "docs/specs/sf-04-loop-discontinuity.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-loop-discontinuity.md"
    - "docs/designs/sf-04-loop-discontinuity.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-04-loop-discontinuity.md"
    - "docs/designs/sf-04-loop-discontinuity.md"
  verified-by: []
  references:
    - "docs/test-specs/tim-03-loop-boundary.md"
    - "docs/test-specs/sf-04-slice8-probe-faults-declare.md"
    - "docs/test-specs/sf-04-slice10-probe-compare-op.md"
  defers: []
  deferred-by: []
---

# SF-04 loop-discontinuity テスト仕様 — OpName wire + loop→probe 不連続宣言

## Purpose

SF-04 loop-discontinuity slice（Issue #107 の
`syn.probe.compare` UI wire OpName + SF-PROBE-07 Loop 脚）が、
loop wrap・`SetLoop`/`ClearLoop` apply を probe clock epoch
の不連続として block 単位で宣言し、跨ぎ compare を
fail-closed に保ち、宣言が transport cursor/PCM/seek を
動かさないことを player 層 headless fixture で保証する。

## Feature/Rule/Example

### F1 — `OpName` union の wire 到達性

- Rule: `ui/src/ipc.ts` の `OpName` union に
  `"syn.probe.compare"` が含まれ、service 側 op
  （`service.rs` `OpInfo`/dispatch・`ui_ops.rs`
  `op_probe_compare` — slice10 実装済み）と綴りが一致
  する。`check-ui`/`test-ui` が green。
- Example: `mise run check-ui` / `mise run test-ui` の
  green をもって wire 一致を確認（TS 側に専用 unit
  test は追加しない — union 追加のみのため）。

### F2 — wrap 検出 → 宣言 → 跨ぎ compare fail-closed

- Rule: `SetLoop` + `Play` が適用された node の
  `observe_block` window に loop wrap が含まれるとき、
  `observed_loop_discontinuity()` が true を返し、
  宣言（`declare_probe_discontinuity`）で probe clock
  epoch が進む。宣言前後を跨ぐ compare window は
  `Aligned` のまま残らず `EpochMismatch`/
  `DiscontinuityInWindow`/`Incomparable` で
  fail-closed する。
- Example: `sf04_loop_discontinuity.rs` で 2 probe
  point を monitor し、`boundary_stage_window` で
  `SetLoop` を stage → `run_observed` + `observe_block`
  の block loop で wrap を観測・宣言 → 追加 block
  実行後の `compare_probes` が `Incomparable` を返す
  ことを確認。baseline（宣言前）は `Aligned`。

### F3 — `SetLoop`/`ClearLoop` apply でも宣言が起きる

- Rule: wrap が発生しなくても、`observe_block` が
  fold した `SetLoop`/`ClearLoop` command があれば
  accessor が true を返し宣言が起きる。検証を通らない
  `SetLoop`（空 region 等 upstream が拒否するもの）は
  apply されず宣言も起きない。
- Example: `SetLoop` のみ（wrap 未発生の window）・
  `ClearLoop` apply の各 block で accessor=true →
  epoch 進行を確認。非該当 block（Play/Pause/Stop・
  command 無し）では accessor=false・epoch 不変。

### F4 — 宣言は cursor・PCM・seek を動かさない

- Rule: `declare_probe_discontinuity` は engine
  cursor・master PCM・seek 状態を変更しない
  （sf04_s9 先例の契約を loop 経路でも維持）。
  `stop`/`run` は宣言点ではない。
- Example: 宣言直前直後で `cursor()` が不変、
  同一 project・同一 block 数の宣言 engine と非宣言
  engine の `PlayReport.pcm`/`crc` が bit-identical、
  wrap/apply を含まない block 列では epoch が進まない。

### F5 — `CommandChannelFull` でも fail-closed

- Rule: probe command channel が満杯で宣言の wire
  push が失敗しても、broker epoch は進み
  `probe_time_context().epoch` と deck の mark が
  観測可能に乖離する（records は旧 epoch を名乗り
  続ける — seek/sf04_s9 先例）。宣言経路はこの
  乖離を隠さない。
- Example: sf04_s9 と同じ channel 満杯 fixture で
  wrap/apply block を流し、`declare_probe_discontinuity`
  が `Broker(CommandChannelFull)` を返しつつ epoch が
  進み、drain 後の record epoch が旧 epoch のまま・
  `time_context` と乖離することを確認。

### F6 — `SessionRoutingEngine::run` の外部契約不変

- Rule: per-block `run_observed(1)` 再構成後も
  `PlayReport`（pcm/crc/frames）・pump cadence
  （`Every(n)` の `index % n` 意味）・error 伝播・
  `pending_observe_cycles` の drain 意味が変更前と
  等価。既存 `synvelis-app`/`synvelis-player` test が
  無変更で green。
- Example: `mise run test synvelis-player` /
  `check`/`lint`/`test-rt`・`check`/`lint
  synvelis-app` の green をもって確認（app/tests は
  sv#117 占有のため本 slice では追加しない）。

## Out of Scope

- 実 GUI・#108 UI consumer・#109 oracle・実機計測
  （NotRun、外部 gate へ委譲）。
- SF-PROBE-08/09・SF04-RCP-01・signal payload 比較。
- node transport `Seek` command の probe 宣言
  （session transport seek は `RoutingEngine::seek`
  経路で宣言済み — spec Deferred）。
- `SessionRoutingEngine::run` を app 層から直接
  駆動する統合 test（`crates/synvelis-app/tests/` は
  sv#117 占有 — player 層 fixture が検出→宣言の
  契約を pin し、engine.rs の結線は code review +
  compile で担保）。
- `*-msrv` 系検証（Daily Full CI 委譲）。
