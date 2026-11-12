---
id: "DESIGN-3kM8vBnQ2wX7yT1zR5pL9d"
type: "design"
status: "approved"
title: "SF-04 slice6 設計 — 複数 Probe 整列比較 consumer（ProbeComparisonRequest → AcousticObservation）"
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
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
    - "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice6 設計 — 複数 Probe 整列比較 consumer

## Context

Ruviform #309（pin `fd1d4d5`）は `align_observation_points`・
`ObservationSpan`・`ObservationCoverage::reconstruct`・
`CoverageEvent`・`AlignPolicy`/`Alignment`/`IncomparableReason`/
`LatencyKnowledge`/`ObservationFormat`/`ProbePrecision`/
`GenerationRange`/`ProbeClockMark`/`ProbeTimeContext` を提供済み
だが、Synvelis 側 consumer は未接続だった。slice1–3 が確立した
`RoutingEngine` の probe read model（`probe_bindings()`/
`probe_segments()`/`probe_time_context()`/`probe_counts()`）と
`ProbeTraceBridge::probe_drain` の drain 経路に、coverage 証拠の
保持と `ProbeComparisonRequest → AcousticObservation` の写像を
additive に追加する。port 契約・`RoutingEngine`・project lib
export は変更しない。

## Scope

### In Scope

- `crates/synvelis-project/src/probe.rs`: slice 6 契約型。
- `crates/synvelis-player/src/probe_compare.rs`（新規）:
  `ProbeCoverage` + `probe_compare`。
- `crates/synvelis-player/src/probe_trace.rs`: coverage 記録・
  `probe_coverage()`/`compare_probes()`。
- `crates/synvelis-player/src/lib.rs`: module/re-export。
- `crates/synvelis-player/tests/sf04_s7_probe_compare.rs`。
- `docs/` 文書チェーン。

### Out of Scope

- `RoutingEngine`/`routing_engine.rs`/`host.rs`/
  `routing_lower.rs`（sv106 lease）、`synvelis-project/src/lib.rs`
  （sv125 lease）、`synvelis-project/src/probe_trace.rs`、
  `synvelis-app/**`、`mise.toml`、`Cargo.lock`、
  `synvelis-workspace`（session 露出は不要 — player 層の
  `compare_probes` が公開面）。
- `syn.probe.compare` app op、SF-PROBE-08/09、SF04-RCP-01、
  seek 以外の不連続宣言点、PCM 相関/再サンプル、実機・#109・
  #108 外部 gate。

## Components and Boundaries

### 契約型（`synvelis_project::probe`）

Issue §4 の `ProbeComparisonRequest → AcousticObservation` を
semantic 語彙で定義する。runtime `AlignPolicy`/`Alignment`/
`IncomparableReason`/`AlignmentBasis`/`AlignmentCondition` の
mirror を `ProbeAlignPolicy`/`ProbeComparisonQuality`/
`ProbeIncomparableReason`/`ProbeComparisonBasis`/
`ProbeAlignCondition` として持ち、consumer 側解決失敗
（`PointUnresolved`/`ProbeNotBound`/`NoObservationData`/
`InconsistentFormat`/`UnsupportedChannelLayout`/
`EvidenceBudgetExceeded`）を runtime 語彙に無い variant として
追加する。`ClockContextRef` と同じ規則で clock id を持つ
runtime evidence 型（`AcousticObservation`・
`ProbeComparisonProvenance` 等）は serde 対象外、
`ProbeComparisonRequest` は persistable intent として serde
可能。公開 enum は全て `#[non_exhaustive]`。

### `ProbeCoverage`（player `probe_compare.rs`）

`ProbeTraceBridge::probe_drain` が唯一の供給経路となる
bounded な per-probe `CoverageEvent` 保持:

```rust
pub const PROBE_COVERAGE_MAX_EVENTS: usize = 2048;

pub struct ProbeCoverage { /* raw probe id → VecDeque<RetainedEvent> */ }
```

- `record(event, loss_floor)`: `CoverageEvent::Data(meta)` と
  `CoverageEvent::Status(status)` のみ保持（`meta.probe`/
  `status.probe` で系列化）。`loss_floor` はその event を生んだ
  drain batch **前**の `ProbeCounts::lost_records`。
- `events(probe)`/`retained_len(probe)`/`probes()`/`retain(keep)`/
  `is_empty()`: read と prune。
- `reconstruct(probe, counts)`: 最新 `counts.lost_records` と
  最古 retained event の floor の差分を loss delta として
  `ObservationCoverage::reconstruct` へ渡す。陽性 delta は全
  interval を `Unknown` へ降格（runtime 規則 — loss の位置を
  推測しない）。

`ProbeId` は public constructor を持たないため、series は最初の
retained event から echo back する（`SeriesCoverage.probe`）。

### `probe_compare` free function

```rust
pub fn probe_compare(
    request: &ProbeComparisonRequest,
    bindings: &BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>,
    segments: &[ProbeCaptureSegment],
    time_context: &ProbeTimeContext,
    coverage: &ProbeCoverage,
    counts: ProbeCounts,
) -> AcousticObservation
```

`RoutingEngine` 非依存の pure function — read model を値で
受け取るため sv106 の routing 変更に追従可能。処理順:

1. `observed_range` が空/逆転 → 即 `Incomparable{CoverageEmpty}`
   （証拠処理の前に fail-closed）。
2. `resolve_side` を before/after に適用。semantic point に
   一致する binding を走査し、`runtime_probe_id` を持つ最初の
   binding で retained coverage を再構築 → `build_side` で
   `SideEvidence` を構築。全候補が失敗した場合は最初の失敗
   reason を返す（`!matched` → `PointUnresolved`、
   `!any_bound` → `ProbeNotBound`、その他は build 失敗 reason）。
3. 片側でも `Err` → `Incomparable{reason}`（request 順で before
   優先）。`interval`/`gap_mask` は解決済み側のみ埋める
   （未解決側の provenance は `None`）。
4. byte budget: retained event 数×`size_of::<CoverageEvent>` +
   interval 数×`size_of::<CoverageInterval>` が `max_bytes` 超過
   → `Incomparable{EvidenceBudgetExceeded}`（alignment 前）。
5. `ObservationSpan` を両側に構築（`probe`/`point`/`clock_mark`/
   `interval`/`generation_range`/`format`/`latency`/
   `coverage`/`context: Some(time_context)`/
   `precision: F32Rt`）→ `align_observation_points` に
   `runtime_policy(&request.alignment_policy)` を渡す。
6. `Alignment` → `ProbeComparisonQuality` 写像
   （`_ =>` で将来 variant は `Unknown` reason へ fail-closed）。

### `build_side` の証拠規則

- window: `observed_range` verbatim（未観測部は gap 化、切り詰め
  ない）、`None` なら evidence extent（reconstructed coverage
  interval ∪ capture segment `frame_range` の union）。extent が
  無ければ `CoverageEmpty`。
- in-window event（`event_extent` が window と overlap）のみが
  format/latency/generation/point を供給。`Data` record が 1 件も
  無ければ `NoObservationData{side}`。channels/sample_rate が
  window 内で一致しなければ `InconsistentFormat{side}`、channel
  数が canonical layout（1/2/4/6/8）に写像できなければ
  `UnsupportedChannelLayout{side, channels}`（record は speaker
  mask を持たず layout を捏造しない）。
- latency: window 内 `effective_latency` が単一値 →
  `LatencyKnowledge::Known{frames}`、複数値 → `Unknown`
  （window 内で latency が変化した事実を単一値に偽装しない）。
- `clock_mark`: window と overlap する最初の coverage interval
  の mark（混在 mark は runtime の `DiscontinuityInWindow`
  証拠）。overlap が無ければ最寄り interval の mark、interval
  が皆無なら `ProbeClockMark::UNKNOWN`（fail-closed）。
- `generation_range`: window 内 event（Data/Status 両方）の
  generation の min/max。

### `side_gaps` / provenance 写像

`gap_mask` は side ごとに window 内の非 `Valid` interval
（`CoverageClass` → `ObservationGapKind` 写像、将来 variant は
`Unknown`）と、window 内で coverage interval が全く覆わない
領域（`Unknown` gap — request が観測を要求したのに証拠が無い
領域を黙って除外しない）を列挙する。`provenance` は解決済み
side の `clock_mark`（`clock_ref` 経由、UNKNOWN → `None`）・
generation range・`time_context` の `runtime_instance`/
`sample_rate` を運ぶ。

### `ProbeTraceBridge` 配線

- field `coverage: ProbeCoverage` を追加。
- `probe_drain` 内で `ProbeEvent::Metric` → `CoverageEvent::Data
  (record.meta)`、`Status` → `CoverageEvent::Status(record)`、
  `Captured(chunk)` → `chunk.meta` があれば `CoverageEvent::
  Data(meta)` を `counts.lost_records`（drain 前 snapshot）を
  floor として記録。stale-generation record も coverage には
  残す（観測履歴の証拠であり update 昇格とは別経路）。
- drain 後に live binding の `runtime_probe_id` 集合で
  `coverage.retain` — 除去/rebind で probe が消えた series の
  陳腐化を防ぐ。
- `probe_coverage()` read accessor と `compare_probes(request)`
  convenience（engine read model + coverage を束ねて
  `probe_compare` を呼ぶ）を追加。

## Data and Control Flow

```
RT probe events ──► RoutingEngine::probe_drain
                      │ ProbeTraceBridge::probe_drain
                      ├─► contract updates（既存 slice2 経路）
                      └─► ProbeCoverage.record(event, loss_floor)
                              │ bounded per-probe series
                              ▼
ProbeComparisonRequest ──► probe_compare / bridge.compare_probes
   semantic points ──► bindings ──► runtime_probe_id
                        │            │
                        │            ▼
                        │      ProbeCoverage.reconstruct
                        │      (loss delta → Unknown 降格)
                        ▼
                   SideEvidence（window/format/latency/
                   generation/clock_mark/coverage）
                        │
                        ▼
              ObservationSpan ×2 ──► align_observation_points
                        │                (Ruviform #309)
                        ▼
              AcousticObservation（quality/gap_mask/
              interval/provenance — contract 語彙）
```

Seek/republish は RT 側の既存機構（`bump_clock_epoch`・
generation-scoped rebind）が record の clock mark/generation に
刻印し、consumer はそれを coverage と generation_range 経由で
受け取る — consumer は不連続を自前で検出・宣言しない。

## Non-Functional Constraints

- 非 RT・control plane のみ: allocation は `Vec`/`BTreeMap`
  ベースで bounded（coverage は probe あたり 2048 event、
  gap_mask/interval は window 内 interval 数に比例）。
- `unsafe` なし、workspace lint 準拠、公開 enum は
  `#[non_exhaustive]`。
- `RoutingEngine` の公開 read model のみ消費 — sv106 が routing
  系を変更中でも signature/publish 意味に依存しない。
- fail-closed が既定: unknown clock/latency/epoch/generation/
  coverage は aligned と見なさない。将来 variant は `Unknown`
  へ写像。

## Risks and Trade-offs

- **coverage は drain 経路依存**: `RoutingEngine::probe_drain`
  を bridge 経由せず直接呼ぶ consumer は coverage を貯めない
  → `NoObservationData` で正直に失敗する（捏造しない）。
  drain の単一 owner は slice2 の `ProbeTracePort` 契約どおり。
- **loss floor の保守性**: floor は drain batch 前の
  `lost_records` で、最古 retained event 起点の差分は retained
  window 全体の損失を over-approximate し得る → coverage が
  `Unknown` に倒れ得るが、valid を偽装する方向には倒れない
  （fail-closed 側の誤差）。
- **`observed_range=None` の extent は coverage ∪ segment**:
  capture segment の `frame_range` は coverage eviction 後も
  extent を延ばし得るが、覆われない部分は `Unknown` gap に
  分類されるため validity を捏造しない。
- **単一 engine では到達不能な reason**（`DomainMismatch`/
  `FormatMismatch`/`RateMismatch`/`LatencyUnknown`/
  `UnknownClock`）: format/domain/clock は series 共有のため
  e2e 再現不可。写像の網羅性はコード上の match で担保し、
  runtime 意味論の正本は Ruviform #309 の test — test-spec に
  明記。
- **generation crossing で probe id が変わる場合**: semantic
  point が移動して rebind が remove+install になると旧
  coverage は prune され cross-generation 証拠は残らない
  → `GenerationSpan` ではなく新世代のみの比較になる。
  point 不変の republish（同じ topology の明示 rebuild）で
  cross-generation window を再現する設計とした。

## Deferred Design Concerns

- `syn.probe.compare` app op・workspace session 露出 |
  `service.rs`/`engine.rs`/`synvelis-app` 占有 | 後続 slice |
  `compare_probes` の wire 適合は app 実装時に検証。
- signal payload（peak/rms・PCM difference）| SF-PROBE-08/09 |
  診断・実測 slice | `metric_set` は宣言 intent のみ。
- loop/restart の不連続宣言点 | routing_engine 変更は
  sv106 完了後 | 別 slice | seek の epoch bump のみ現行。
- 実機 format/latency 多様性 | #109 oracle gate | gate 後 |
  headless は単一 format。

## Verification Notes

- `crates/synvelis-player/tests/sf04_s7_probe_compare.rs`
  （実 `RoutingEngine` + `ProbeTraceBridge`、adapter map 空 —
  comparison は trace routing を必要としない）:
  - `sf04_s7_monitored_probes_align_on_shared_cursor`:
    2 strip Output point の Monitor → `Aligned` +
    `SharedCursor` basis + error budget 0 + clock provenance
    一致 + gap_mask 空。
  - `sf04_s7_partial_coverage_is_conditional_never_aligned`:
    `observed_range` を証拠 extent より拡大 → `Conditional`
    + `PartialCoverage` + `excluded_intervals` 非空 + 両側
    `Unknown` gap。
  - `sf04_s7_unsubscribed_point_is_point_unresolved` /
    `sf04_s7_binding_without_runtime_probe_is_not_bound` /
    `sf04_s7_bound_probe_without_records_has_no_observation_data`:
    consumer 側 3 分岐。
  - `sf04_s7_empty_observed_range_is_coverage_empty` /
    `sf04_s7_evidence_budget_is_bounded`: 境界 fail-closed。
  - `sf04_s7_window_spanning_seek_is_discontinuity_in_window`:
    seek を跨ぐ window → `DiscontinuityInWindow`。
  - `sf04_s7_sides_on_different_epochs_are_epoch_mismatch`:
    pre-seek 観測 vs post-seek bind → `EpochMismatch`。
  - `sf04_s7_window_spanning_republish_is_generation_span`:
    同一 project republish（generation 増・probe id 不変を
    assert）→ `GenerationSpan`。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build`。
