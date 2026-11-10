---
id: "TSPEC-034UL2Z7qW8xN4pLm5Rs6"
type: "test-spec"
status: "approved"
title: "SF-04 slice3 受入テスト仕様 — clock epoch discontinuity・segment cut・fault/DSP invariance"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice3 受入テスト仕様 — clock epoch discontinuity・segment cut・fault/DSP invariance

## Purpose

SF-04 slice3（Issue #107 の clock epoch discontinuity・capture
segmentation・fault/DSP invariance）が、cursor-series の clock epoch
不連続に対して fail-closed に振る舞い、capture segment が epoch
境界を跨がず、probe lifecycle が DSP 出力を変えず、bounded fault
が原子性を保つことを保証する受入仕様。#108 UI・#109 実測 slice
でも同じ契約を検証に使う。

## Feature

Probe clock epoch discontinuity・capture segment cut・bounded
fault・DSP invariance（headless、`synvelis-player` fixture）。

## Rules

- Rule: `RoutingEngine::seek` は probe clock epoch を bump し、
  以後に drain される `MetricRecord`/`StatusRecord` の
  `ClockContextRef.epoch` が新 epoch を運ぶ。RT frame cursor の
  `sample_position` は seek を跨いでも単調増加のままであり、
  epoch のみが不連続の証拠である。
- Rule: capture 中の epoch 変化で open segment は
  `ended_by == ClockEpoch` で閉じ、`clock_context` は旧 epoch を
  保持し、`frame_range`/`completeness`/`discontinuities` は
  旧 epoch の evidence のみを反映する。
- Rule: epoch 変化を示す最初の record/status は旧 segment を
  閉じる引き金であり、その record 自体は旧 segment に帰属しない。
  新 epoch の chunk/status は open segment が無い限り捨てられ、
  segment ledger に暗黙の新 segment は現れない — 新規 capture は
  明示的な `probe_capture` のみが開始する。
- Rule: segment の `clock` が `None`（clock 未配線）なら epoch
  照合を行わず segment を切らない — Unknown provenance を推測で
  塗り潰さない。
- Rule: closing status kind（`CaptureStopped`/`Removed`/
  `RebindDropped`）は自身の end reason を優先し、epoch cut の
  対象外とする。
- Rule: probe lifecycle（request/set_mode/capture/stop/remove・
  Monitor/Capture mode）が master PCM・graph 動作・CRC を変えない
  （SF-PROBE-02）。
- Rule: bounded fault は fail-closed — probe table 上限
  （`max_probes`）超過は `CapacityExceeded` で拒否され、rebind
  中の command channel full は atomic に失敗して subscription を
  `QueuedForBinding` に留め、RT install を `BoundActive` と
  偽装しない。次の publish が全体を再試行して回復する。

## Examples

- Example: clip-bearing project で strip Output に Monitor probe を
  install し run → metric record が epoch0 を運ぶ。`seek` 後の
  run で record が epoch1 を運び、`sample_position` は単調増加の
  まま（`sf04_s3_seek_bumps_cursor_clock_epoch_on_records`）。
- Example: capture budget 内で `seek` → open segment が
  `ended_by=ClockEpoch`・旧 epoch `clock_context`・`Complete`
  completeness で閉じ、以後の run でも segment 数は 1 のまま。
  明示 `probe_capture` で新 `capture_session_id`・新 epoch の
  第 2 segment が `BudgetExhausted` で閉じる
  （`sf04_s3_capture_segment_cuts_at_clock_epoch`）。
- Example: 同一 project を probe 無し engine と Monitor+Capture
  probe 付き engine で run → master PCM・CRC が bit 一致し、
  fixture PCM は非 silence
  （`sf04_s3_probe_modes_leave_master_output_identical`）。
- Example: 32 subscription install 後の 33 個目の `probe_request`
  が `Err(ProbeRequestError::Broker(CapacityExceeded))`
  （`sf04_s3_probe_table_capacity_is_bounded`）。
- Example: probe command channel を満杯にして publish → rebind の
  `Begin+Stage+Commit` が atomic に失敗し RT binding は空のまま、
  全 subscription が `QueuedForBinding`・`rt_status().
  stale_commands > 0`。次 publish で全件 `BoundActive{generation=3}`
  （`sf04_s3_rebind_channel_full_stays_atomic_and_recovers`）。

## Guarantees

- `CaptureSegmentEnd::ClockEpoch` は generation 境界
  （`GenerationBoundary`）と区別される独立した end reason であり、
  epoch 由来の切断と plan 由来の切断を後から識別できる。
- epoch cut は drain 側（非 RT）のみで起き、`ProbeDeck`/
  `ProbeCollector` の RT 契約（no-alloc/no-lock/no-I/O）を変更
  しない。
- `bump_clock_epoch` の `Err(CommandChannelFull)` は broker epoch
  が既に進んだ状態を残す — 失敗時に epoch を巻き戻す補償は行わず、
  broker/deck の乖離は `probe_time_context`/record epoch で
  観測可能なままにする。
- headless fixture は実 `RoutingEngine`+`ProbeDeck`+`ProbeCollector`
  の in-process 経路を通し、fake/mocked telemetry に依存しない。

## Non-goals

- UI 表示・実 plugin/OS/RT 観測・実測値の正しさ（#108/#109/#114）。
- loop point・device restart・transport stop/start 等 seek 以外の
  不連続宣言点の epoch bump（`bump_clock_epoch` API は再利用可能
  だが宣言点の網羅は transport 契約の別 slice）。
- `SetClockMark` 配送失敗時の deck/broker epoch 乖離の能動修復。
- Ruviform generic clock alignment・`ProbeTimeContext` の align
  分類自体の再検証（Ruviform #309 の契約）。
- `ControlTrace`/`ProbeTraceBridge` の翻訳契約の変更（slice 2 の
  契約をそのまま消費）。

## Risk

- What is lost or breaks if these guarantees are dropped: seek を
  跨ぐ PCM/status が単一の連続 capture span として記録され、
  `frame_range`/`discontinuities` が実在しない連続区間を主張する
  — SF-PROBE-04 の provenance 契約と「Unknown/不連続を silence/
  zero に置き換えない」honesty 原則の直接違反。probe mode が
  master PCM を変えるなら SF-PROBE-02 違反、bounded fault が
  原子性を失うなら fail-closed 契約違反。
