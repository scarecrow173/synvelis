---
id: "IMPL-5zX3cV7bN1mK9jH4gFtDsA"
type: "ir"
status: "completed"
title: "SF-04 slice6 実装記録 — 複数 Probe 整列比較 consumer（ProbeComparisonRequest → AcousticObservation）"
created: "2026-11-12"
updated: "2026-11-12"
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
    - "docs/tasks/sf-04-slice6-probe-compare-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/sf-04-slice6-probe-compare-task.md"
    - "docs/designs/sf-04-slice6-probe-compare-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/sf-04-slice6-probe-compare.md"
  defers: []
  deferred-by: []
---

# SF-04 slice6 実装記録 — 複数 Probe 整列比較 consumer

## Summary

Issue #107 SF-PROBE-07 の headless consumer を実装した。
`ProbeComparisonRequest → AcousticObservation` 契約を
`synvelis_project::probe` に定義し、Ruviform #309 の
`align_observation_points` を消費する非 RT・bounded・
fail-closed の `probe_compare` free function と、drain 経路の
`CoverageEvent` 保持（`ProbeCoverage`）を player 層に追加した。
`RoutingEngine`・port 契約・app/session 層・`Cargo.lock`・
docs index は変更していない。Ruviform pin は `fd1d4d5` のまま
（#309 API が同梱済みのため更新不要）。

## Implemented

### `crates/synvelis-project/src/probe.rs`

契約型一式を追加（全公開 enum `#[non_exhaustive]`）:

- `ProbeComparisonSide`/`ProbeMetricKind`/`ProbeAlignPolicy`
  （default: same generation 要求・known latency 必須・暗黙
  rate 変換なし・error budget 0・conditional 許容・shared
  cursor 優先）
- `ProbeComparisonRequest`（serde 可 — persistable intent）
- `AcousticObservation`/`ProbeComparisonQuality`/
  `ProbeIncomparableReason`（consumer 側解決失敗 variant を
  runtime 語彙へ追加）/`ProbeComparisonBasis`/
  `ProbeAlignCondition`/`ProbeComparisonGap`/
  `ProbeComparisonProvenance`/`ProbeGenerationRange`/
  `ProbeFrameInterval`/`ObservationGapKind`（serde 不可 —
  clock id を含む runtime evidence）。

### `crates/synvelis-player/src/probe_compare.rs`（新規）

- `ProbeCoverage`: probe ごとの bounded `CoverageEvent`
  系列（`PROBE_COVERAGE_MAX_EVENTS = 2048`）。`record` は
  `Data`/`Status` のみ保持し loss floor（drain 前の
  `lost_records`）を付与。`reconstruct` は最新 counts との差分
  を loss delta として `ObservationCoverage::reconstruct` へ
  渡し、陽性 delta は interval を `Unknown` へ降格。
- `probe_compare(request, bindings, segments, time_context,
  coverage, counts) -> AcousticObservation`: `RoutingEngine`
  非依存の pure function。`observed_range` 空/逆転 →
  `CoverageEmpty`、semantic point 解決失敗 →
  `PointUnresolved`/`ProbeNotBound`、window 内 data 無し →
  `NoObservationData`、format 不一致 → `InconsistentFormat`、
  非 canonical channel → `UnsupportedChannelLayout`、
  `max_bytes` 超過 → `EvidenceBudgetExceeded`（alignment 前）。
  `ObservationSpan` 構築 → `align_observation_points` →
  `Alignment` を contract `ProbeComparisonQuality` へ写像
  （将来 variant は `_ => Unknown` で fail-closed）。
- `gap_mask`: request が覆うが coverage が無い領域を
  `ObservationGapKind::Unknown` で列挙し `PartialCoverage`
  conditional へ降格 — `Aligned` への切り詰めなし。

### `crates/synvelis-player/src/probe_trace.rs`

`ProbeTraceBridge` に `coverage: ProbeCoverage` を追加。
`probe_drain` で `ProbeEvent::Metric` → `CoverageEvent::Data`、
`Status` → `CoverageEvent::Status`、`Captured` chunk meta →
`Data` を loss floor 付きで記録し、drain 後に live binding の
probe 集合で prune。`probe_coverage()`/`compare_probes()`
accessor を追加（既存 drain→update 経路は不変）。

### `crates/synvelis-player/src/lib.rs`

`pub mod probe_compare` + re-export。

### `crates/synvelis-player/tests/sf04_s7_probe_compare.rs`（新規）

10 件の headless integration test（実 engine fixture、
adapter map 空）。F1–F10 全て pass — 詳細は
`docs/test-specs/sf-04-slice6-probe-compare.md`。

## Decisions

- **Free function + read model 値受け**: `RoutingEngine` の
  method signature・generation/publish 意味に依存しない設計
  （sv106 lease 対応）。coverage は `ProbeTraceBridge` が drain
  時に記録する唯一の供給経路 — bridge を通さない consumer は
  `NoObservationData` で正直に失敗する（fail-closed）。
- **`observed_range=None` の extent**: reconstructed coverage
  interval ∪ capture segment `frame_range` の union。
  未観測部は `Unknown` gap に分類され validity を偽装しない。
- **generation 境界の再現**: 同一 project republish は
  generation を増やし probe id は不変（semantic point 不変の
  明示 rebuild）— cross-generation retained coverage が残り
  `GenerationSpan` を再現可能。point 移動時は rebind-drop で
  prune されるため cross-generation 証拠は残らない（設計どおり）。
- **単一 engine 到達不能 reason**: `DomainMismatch`/
  `FormatMismatch`/`RateMismatch`/`LatencyUnknown`/
  `UnknownClock` は series 共有 format/domain・pair clock の
  ため e2e 再現不可 — 写像網羅性は match + fail-closed で担保。

## Verification

- `cargo test -p synvelis-player --test sf04_s7_probe_compare`:
  10 passed / 0 failed（2026-11-12）。
- canonical `mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`check-app-native`/`lint-app-native`/`test-msrv`/
  `build`: 実施結果は PR/worker 報告を参照。
- 外部 gate: 実機・#109 Oracle・#108 UI・`syn.probe.compare`
  app op・SF-PROBE-08/09・consumer 照合は **NotRun** —
  後続 slice/外部受け入れへ委譲。Pass 扱いしない。

## Remaining / Out of Scope

- `syn.probe.compare` app op（`service.rs`/`engine.rs` 他
  Worker 占有）。
- `synvelis-workspace` session 露出（不要と判断 —
  `compare_probes` が player 層公開面）。
- seek 以外の不連続宣言点（loop/restart — routing_engine
  変更は sv106 完了後）。
- signal payload 比較（peak/rms・PCM difference —
  SF-PROBE-08/09）。
- doc index README 更新は coordinator 統合時処理。
