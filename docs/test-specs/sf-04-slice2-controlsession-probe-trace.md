---
id: "TSPEC-034UupvzrMsHjf6LBZma9J"
type: "test-spec"
status: "approved"
title: "SF-04 slice2 受入テスト仕様 — probe observation → trace 記録・freshness・fail-closed"
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
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice2 受入テスト仕様 — probe observation → trace 記録・freshness・fail-closed

## Purpose

SF-04 slice2（Issue #107 の workspace ControlSession 配線）が、probe
observation を session 経由で `ControlTrace.observed` slot に記録し、
freshness を正直に分類し、fail-closed/stale reject を守ることを保証
する受入仕様。#108 UI・#109 実測 slice でも同じ契約を検証に使う。

## Feature

Probe observation の session→trace 記録経路（headless）。

## Rules

- Rule: `probe_drain` は session 唯一の drain 経路であり、port が返す
  `ProbeObservationUpdate` のうち session 相関
  （`ProbeSubscriptionKey` → `ParameterTargetId`）を持つものだけが
  `record_probe` される。相関の無い update は記録されず
  `ProbeDrainReport.uncorrelated` に計上される。
- Rule: `record_probe` が false（trace route 無し）を返した呼出は
  `unrouted` に計上され、silent drop にならない。
- Rule: stage 分離 — probe wiring は `observed` slot のみを書き、
  configured/prepared/scheduled/applied/dispatched/plugin_reported
  を合成・上書きしない。
- Rule: missing は偽装しない — trace 不在 target の freshness は
  `Unavailable{NoEvidence}`、`ProbeObservation::Missing` は
  `Unavailable{ProbeMissing}`、証拠の無い slot は `Missing` のまま。
- Rule: player bridge の Metric translation は
  `meta.generation == binding.active_plan_generation` の record のみ
  `Measuring` にする。stale generation の record は trace に記録
  しない。
- Rule: clock 未配線（`ProbeClockMark::is_unknown`）の record の
  stamp provenance は `Estimated` — `Current` に分類されない。
- Rule: `probe_remove` 成功後、相関 target の observed slot は
  `Missing` になる（RT `Removed` status の再記録は冪等）。

## Examples

- Example: fake port が `Measuring{stamp(Derived), Signal(0.5)}` の
  update を返し `probe_drain` → 対象 target の `observed` slot が
  `Present(Measuring)` になり、`observed_freshness(ctx)` が
  `Current`（ctx 一致時）。
- Example: session が相関を持たない key の update は drain report
  で `uncorrelated` に数えられ、いかなる trace にも現れない。
- Example: `probe_request` が `UnknownTarget`/port error を返した
  場合、相関は登録されず後続の update も記録されない。
- Example: player bridge で旧 generation の `MetricRecord` が drain
  されても update を生成しない（stale reject）。
- Example: `LostRecords` status → `ProbeObservation::Lost{count}` →
  freshness `DataLoss{count}`。`Removed` status → `Missing` →
  `Unavailable{ProbeMissing}`。
- Example: 1 つの subscription key が 2 target を養う場合、drain
  1 update が両 target の observed slot に記録される（fan-out）。

## Guarantees

- `ControlSession::probe_drain` が port の `probe_drain` を呼ぶ
  唯一の session 経路である。
- `ProbeTraceBridge::probe_drain` は contract 型のみを返し、
  `ProbeId`/ruviform 型を port 境界の外に出さない。
- 全 stage slot の `is_coherent` 判定は既存 `ControlTrace` 契約の
  まま — probe 記録は `observed` slot と `evaluation_position`
  （Measuring 時）だけを更新する。
- `ProbeDrainReport` の 4 カウンタが実際の呼出結果を反映する
  （記録・未相関・拒否を正直に数える）。

## Non-goals

- UI 表示・実 plugin/OS/RT 観測・実測値の正しさ（#108/#109/#114）。
- probe capture PCM の trace 記録（`MeasuredValue` は PCM を運ば
  ない — segment ledger の責務）。
- `DispatchTraceAdapter`/`ControlTrace` 自体の slot/freshness 契約
  の再検証（PC-C3/PC-C3-05 の test suite が担保）。
- app composition root の port 実装の検証（別 dispatch）。

## Risk

- What is lost or breaks if these guarantees are dropped: 観測の無い
  target が「測定中」を装う、旧 generation の record が新しい観測に
  見える、plugin report と probe 観測が混同される、drain が二重に
  走って event を取りこぼす/二重記録する — いずれも SF-PROBE-03/04
  と TRACE-C3 の honesty 契約の直接違反。
