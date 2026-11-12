---
id: "TSPEC-8uY6tR4eW2qA5sD9fGhJk"
type: "test-spec"
status: "approved"
title: "SF-04 slice6 テスト仕様 — 複数 Probe 整列比較 consumer"
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
    - "docs/specs/sf-04-slice6-probe-compare.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice6-probe-compare.md"
    - "docs/designs/sf-04-slice6-probe-compare-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/sf-04-slice5-probe-subscriber-ownership.md"
  defers: []
  deferred-by: []
---

# SF-04 slice6 テスト仕様 — 複数 Probe 整列比較 consumer

## 対象

`crates/synvelis-player/tests/sf04_s7_probe_compare.rs`
（headless、実 `RoutingEngine` + `ProbeTraceBridge`、
adapter map は空 — comparison は trace routing を必要と
しない）。

## 検証方針

- fixture は実 engine の `probe_request` → `run` →
  `probe_drain` 経路を通し、`probe_compare`/
  `bridge.compare_probes` の公開面のみで結果を検証する。
- `Aligned`/`Conditional`/`Incomparable` の 3 分岐と、
  spec の fail-closed 要件（解決失敗・coverage 空・
  budget 超過・epoch 不連続・generation 境界・gap）を
  到達可能な全分岐で固定する。
- 単一 engine では到達不能な runtime reason
  （`DomainMismatch`/`FormatMismatch`/`RateMismatch`/
  `LatencyUnknown`/`UnknownClock`）は本 test-spec の対象外
  — format/domain/clock は series 共有・clock は pair で
  常時 wired のため。写像の網羅性は `map_reason`/
  `map_basis`/`map_condition` の match 網羅 + `_ => Unknown`
  fail-closed で担保し、runtime 意味論の正本は Ruviform
  #309 の test に委譲する。
- PCM 相関・signal payload は対象外（契約は alignment
  envelope のみ）。

## Test Cases

| # | Test | 期待 |
| --- | --- | --- |
| F1 | `sf04_s7_monitored_probes_align_on_shared_cursor` | 同一 shared cursor で観測された 2 probe の比較が `Aligned`、basis = `SharedCursor`、error budget = 0、両側 provenance の clock mark/generation が一致、gap_mask 空 |
| F2 | `sf04_s7_partial_coverage_is_conditional_never_aligned` | `observed_range` を証拠 extent より拡大 → `Conditional` + `PartialCoverage`、`excluded_intervals` 非空、gap_mask に `Unknown` gap |
| F3 | `sf04_s7_unsubscribed_point_is_point_unresolved` | 未 subscribe の semantic point → `Incomparable{PointUnresolved}` |
| F4 | `sf04_s7_binding_without_runtime_probe_is_not_bound` | binding はあるが runtime probe 未 install → `Incomparable{ProbeNotBound}` |
| F5 | `sf04_s7_bound_probe_without_records_has_no_observation_data` | probe は bound だが window 内 data record 無し → `Incomparable{NoObservationData}` |
| F6 | `sf04_s7_empty_observed_range_is_coverage_empty` | 空/逆転 `observed_range` → `Incomparable{CoverageEmpty}` |
| F7 | `sf04_s7_evidence_budget_is_bounded` | `max_bytes` 超過 → `Incomparable{EvidenceBudgetExceeded}`（alignment 前 fail-closed） |
| F8 | `sf04_s7_window_spanning_seek_is_discontinuity_in_window` | seek（`bump_clock_epoch`）を跨ぐ window → `Incomparable{DiscontinuityInWindow}` |
| F9 | `sf04_s7_sides_on_different_epochs_are_epoch_mismatch` | pre-seek 観測 vs post-seek bind の比較 → `Incomparable{EpochMismatch}` |
| F10 | `sf04_s7_window_spanning_republish_is_generation_span` | 同一 project republish（generation 増・probe id 不変を assert）跨ぎ window → `Incomparable{GenerationSpan}`（default policy は same-generation 要求） |

## 結果記録

- 実行: `cargo test -p synvelis-player --test
  sf04_s7_probe_compare` → 10 passed / 0 failed / 0 ignored
  （2026-11-12）。
- 既存 `sf04_*` test 群は drain 経路の外部挙動を変えない
  additive 変更のため無変更で green を期待（canonical
  `mise run test` で確認）。

## 外部 Gate（NotRun — Pass 扱いしない）

- 実機/実 latency・format 多様性（#109 oracle gate）。
- #108 UI/session 統合・`syn.probe.compare` app op。
- consumer 照合・診断解釈（SF-PROBE-08/09）。
