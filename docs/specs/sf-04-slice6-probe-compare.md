---
id: "SPEC-9fQ2xLmNpR7vT4wY8zK1jH"
type: "spec"
status: "approved"
title: "SF-04 slice6 — 複数 Probe 整列比較 consumer（ProbeComparisonRequest → AcousticObservation）"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/specs/sf-04-slice4-app-wiring.md"
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
  defers: []
  deferred-by: []
---

# SF-04 slice6 — 複数 Probe 整列比較 consumer（ProbeComparisonRequest → AcousticObservation）

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）の SF-PROBE-07 headless
slice。Issue §4 の契約 `ProbeComparisonRequest →
AcousticObservation` を `synvelis-project` の contract 語彙で定義
し、player 層に **非 RT・bounded・fail-closed の free function**
比較 consumer を実装する。consumer は既存の公開 read model
（`RoutingEngine::probe_bindings()`/`probe_segments()`/
`probe_time_context()`/`probe_counts()`）と、probe trace drain
経路が保持する `CoverageEvent` 証拠から `ObservationSpan` を構築
し、判定本体は Ruviform #309 の `align_observation_points`
（`Aligned`/`Conditional`/`Incomparable`）へ委譲する。

具体的には:

- `crates/synvelis-project/src/probe.rs` に契約型を追加:
  `ProbeComparisonRequest`（before/after semantic point・
  `observed_range`・`ProbeAlignPolicy` mirror・`metric_set`・
  `max_bytes`）、`AcousticObservation`（quality・interval・
  `gap_mask`・provenance）、および `ProbeComparisonQuality`・
  `ProbeIncomparableReason`・`ProbeComparisonBasis`・
  `ProbeAlignCondition`・`ProbeComparisonGap`・
  `ProbeComparisonProvenance`・`ProbeGenerationRange`・
  `ProbeFrameInterval`・`ProbeComparisonSide`・`ProbeMetricKind`・
  `ObservationGapKind` 等の semantic mirror。runtime id
  （`ProbeId`/`NodeId`/`PortId`）は contract に現れない。
- `crates/synvelis-player/src/probe_compare.rs`（新規）に
  `ProbeCoverage`（probe ごとの bounded `CoverageEvent` 保持、
  `PROBE_COVERAGE_MAX_EVENTS = 2048`）と free function
  `probe_compare(request, bindings, segments, time_context,
  coverage, counts) -> AcousticObservation` を実装。semantic
  point → binding → runtime probe → retained coverage の解決、
  in-window record からの format/latency/generation 収集、
  `ObservationCoverage::reconstruct` による coverage 再構築、
  `ObservationSpan` 構築、runtime `Alignment` → contract
  `ProbeComparisonQuality` への写像を行う。
- `crates/synvelis-player/src/probe_trace.rs` の
  `ProbeTraceBridge::probe_drain` が drain した
  `CoverageEvent::{Data,Status}`（metric・status・captured
  chunk meta）を `ProbeCoverage` に記録し、live binding を
  失った probe の series を prune する。`compare_probes`
  convenience method を追加。
- `crates/synvelis-player/tests/sf04_s7_probe_compare.rs` で
  実 engine fixture の headless test を追加し、`Aligned`/
  `Conditional`/`Incomparable` の全到達分岐を固定する。

## Why Now

Issue #107 の SF-PROBE-07 は「複数 Probe の
Clock/Epoch/Latency/Format/Gaps 比較」と「Seek/Loop/Restart の
Unknown/Incomparable」を要求する。Ruviform #309 が
`align_observation_points`・`ObservationCoverage::reconstruct`・
`CoverageEvent`・`AlignPolicy`/`Alignment`/`IncomparableReason`/
`LatencyKnowledge`/`ObservationFormat`/`GenerationRange` を
提供済み（pin `fd1d4d5` に同梱、更新不要）で、consumer 側接続
が未実装のまま残っていた。本 slice がその headless 中核を固定
し、#108 UI・#109 実測 oracle がこの契約の上に乗る。

## Users and Value

- #108 UI/製品 session 実装者が、2 つの semantic probe point
  間の整列品質を `Aligned`/`Conditional`/`Incomparable` の
  3 値と明示 reason で取得できる。
- #109 実測/domain oracle が、比較結果の provenance
  （clock identity・generation range・runtime instance・
  sample rate）と gap mask を独立検証の入力に使える。
- session/app 層の後続 slice が `syn.probe.compare` op を
  実装する際の player 側受け口（`ProbeTraceBridge::
  compare_probes`）を得る。

## Scope

### In Scope

- `crates/synvelis-project/src/probe.rs`: slice 6 契約型の追加
  （`#[non_exhaustive]` 慣例・serde は persistable intent のみ、
  clock id を持つ runtime evidence 型は serde 対象外）。
- `crates/synvelis-player/src/probe_compare.rs`（新規）:
  `ProbeCoverage` + `probe_compare` free function。
- `crates/synvelis-player/src/probe_trace.rs`:
  `ProbeCoverage` の保持・drain 時記録・prune、
  `probe_coverage()`/`compare_probes()` accessor。
- `crates/synvelis-player/src/lib.rs`: module 宣言と re-export。
- `crates/synvelis-player/tests/sf04_s7_probe_compare.rs`:
  headless integration test。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/impl
  record）。

### Out of Scope

- `syn.probe.compare` app op（`service.rs`/`engine.rs` は他
  Worker 占有 — 後続 slice）。
- `synvelis-workspace`/`synvelis-app`/`ui` の変更（session
  露出は不要と判断 — `ProbeTraceBridge::compare_probes` が
  player 層の公開面として十分）。
- SF-PROBE-08（診断解釈）・SF-PROBE-09（実測）・
  SF04-RCP-01（RCP 実装待ち）・「seek 以外の不連続宣言点」
  （routing_engine 変更・sv106 完了後）。
- PCM 相関・再サンプル・signal payload（peak/rms delta・
  bounded PCM difference は契約上の将来領域で本 slice は
  alignment envelope のみ）。
- `RoutingEngine`/`routing_engine.rs`/`host.rs`/
  `routing_lower.rs`・`synvelis-project/src/lib.rs`・
  `synvelis-project/src/probe_trace.rs`・`synvelis-app/**`・
  `mise.toml`・`Cargo.lock`（lease/統合境界 — 変更禁止）。
- 実機・#109 独立 Oracle・#108 UI・consumer 照合（外部 gate —
  NotRun）。

## Requirements

- `ProbeComparisonRequest → AcousticObservation` 契約が
  `synvelis_project::probe` モジュールパスで到達可能で、
  semantic point のみを持ち runtime id を含まない。
- 比較は非 RT・bounded: `ProbeCoverage` は probe ごと
  `PROBE_COVERAGE_MAX_EVENTS`（2048）で頭打ち、request の
  `max_bytes` を超える証拠は alignment 前に
  `EvidenceBudgetExceeded` で fail-closed。
- 判定は Ruviform #309 `align_observation_points` に委譲し、
  `Aligned`/`Conditional`/`Incomparable` の 3 値を contract
  語彙へ写像する。runtime `AlignmentBasis`/
  `AlignmentCondition`/`IncomparableReason` の将来 variant は
  `Unknown` へ写像し昇格しない。
- consumer 側 fail-closed: semantic point に binding が無い →
  `PointUnresolved`、binding に runtime probe が無い →
  `ProbeNotBound`、window 内に data record が無い →
  `NoObservationData`、window 内 record の format 不一致 →
  `InconsistentFormat`、非 canonical channel 数 →
  `UnsupportedChannelLayout`、空/逆転 `observed_range` →
  `CoverageEmpty`、証拠超過 → `EvidenceBudgetExceeded`。
- `observed_range` は比較を絶対に規定する: request が覆うが
  観測の無い領域は `gap_mask` に `Unknown` として現れ、
  `Conditional{PartialCoverage}` へ降格する — `Aligned` への
  沈黙の切り詰めはしない。
- Seek/Loop/Restart 等の不連続は `Unknown`/`Incomparable`:
  epoch が進んだ record を跨ぐ window は `DiscontinuityInWindow`、
  異なる epoch の span 同士は `EpochMismatch`、generation を
  跨ぐ window は `GenerationSpan`（既定 policy）として決して
  valid alignment を名乗らない。
- telemetry loss は drain batch の `lost_records` floor で局所化
  し、陽性 delta は coverage を `Unknown` へ降格する
  （runtime 規則）— loss を validity と見なさない。
- PCM を捏造・相関・再サンプルしない。測定された無音
  （`CoverageClass::Valid`）と未観測/喪失/非対応の区別を
  保つ。root-cause の推測結論を出さない。
- `RoutingEngine` の公開 read model のみを消費し、method
  signature・generation/publish 意味に依存しない
  （sv106 lease — 将来の routing 変更に追従可能）。

## Acceptance Criteria

- [ ] `ProbeComparisonRequest`/`AcousticObservation` と関連
  contract 型が `synvelis_project::probe` で定義され、runtime
  id を含まず `#[non_exhaustive]` 慣例に従う。
- [ ] `probe_compare` free function と `ProbeTraceBridge::
  compare_probes` が実装され、`crates/synvelis-player/tests/
  sf04_s7_probe_compare.rs` が `Aligned`（shared cursor
  basis）・`Conditional`（partial coverage）・`Incomparable`
  （`PointUnresolved`/`ProbeNotBound`/`NoObservationData`/
  `CoverageEmpty`/`EvidenceBudgetExceeded`/`EpochMismatch`/
  `DiscontinuityInWindow`/`GenerationSpan`）を実 engine で
  固定する。
- [ ] seek（epoch bump）を跨ぐ比較が `Incomparable` であり
  `Aligned` を名乗らないこと、republish（generation 境界）
  を跨ぐ比較が `GenerationSpan` であることが test で証明
  される。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build`
  が green。
- [ ] 外部 gate（実機・#109 Oracle・#108 UI・app op・consumer
  照合）は NotRun として記録され、Pass 扱いしない。

## Deferred Design Concerns

- `syn.probe.compare` app op・session 層露出 | `service.rs`/
  `engine.rs`/`synvelis-app` は他 Worker 占有 | 後続 slice
  （#108 系）| player 層の `compare_probes` が受け口として
  十分かは app 実装時に検証。
- `DomainMismatch`/`FormatMismatch`/`RateMismatch`/
  `LatencyUnknown`/`UnknownClock` の e2e 再現 | 単一 engine の
  cursor series は format・domain を共有し、clock は pair で
  常時 wired — これらは runtime 層の fail-closed 意味論で
  あり consumer は写像のみ | Ruviform #309 の runtime test
  が正本 | contract mirror の存在と写像の網羅性は本 slice
  で固定済み。
- 「seek 以外の不連続宣言点」（loop/restart の epoch/世代
  宣言）| routing_engine 変更は sv106 lease | sv106 完了後
  の別 slice | 現行は seek の `bump_clock_epoch` のみが
  宣言済み不連続。
- signal payload 比較（peak/rms delta・bounded PCM
  difference・`metric_set` の消費）| SF-PROBE-08/09 範囲 |
  診断・実測 slice | `metric_set` は宣言 intent のみで
  本 slice は alignment envelope のみ報告。
- 実機 latency/format の多様性 | #109 実測 oracle gate |
  gate 通過後 | headless fixture は単一 format のみ。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/107>
  （SF-PROBE-07、契約 §4 `ProbeComparisonRequest →
  AcousticObservation`）。
- Ruviform #309（`crates/ruviform-audio/src/typed/probe/
  time.rs`）: `align_observation_points`・`ObservationSpan`・
  `ObservationCoverage::reconstruct`・`CoverageEvent`・
  `AlignPolicy`/`Alignment`/`IncomparableReason`/
  `LatencyKnowledge`/`ObservationFormat`/`ProbePrecision`/
  `GenerationRange`。pin `fd1d4d5` に同梱済み — pin 更新不要。
- 前提: SF-04 slice1（`ProbeSession`/semantic point 解決）、
  slice2（`ProbeTraceBridge`/`ProbeTracePort` drain）、
  slice3（seek → `bump_clock_epoch`・capture segment の
  `ClockEpoch` cut）、slice5（subscriber lease — 本 slice は
  port 契約を変えず消費のみ）。
- 本 spec は Issue #107 のうち SF-PROBE-07 の headless
  consumer 部分のみを対象とし、Issue は Open 継続が前提。
