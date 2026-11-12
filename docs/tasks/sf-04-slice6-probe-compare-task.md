---
id: "TASK-1pO9iU7yT5rE3wQ6aSdFgH"
type: "task"
status: "done"
title: "SF-04 slice6 T1 — 複数 Probe 整列比較 consumer（ProbeComparisonRequest → AcousticObservation）"
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
    - "docs/plans/sf-04-slice6-probe-compare-plan.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-04-slice6-probe-compare-plan.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice6 T1 — 複数 Probe 整列比較 consumer

## Work Items

- [x] T1-1: `crates/synvelis-project/src/probe.rs` に
  `ProbeComparisonRequest`/`AcousticObservation` 契約型一式を
  追加（`#[non_exhaustive]`・runtime id 非含有・clock id を
  持つ evidence 型は serde 対象外）。
- [x] T1-2: `crates/synvelis-player/src/probe_compare.rs` 新規
  — `ProbeCoverage`（bounded per-probe `CoverageEvent` 系列、
  `PROBE_COVERAGE_MAX_EVENTS = 2048`、loss floor 付き）と
  `probe_compare` free function（resolve → build side →
  budget → `ObservationSpan` → `align_observation_points` →
  contract 写像）。
- [x] T1-3: `probe_trace.rs` 配線 — `ProbeTraceBridge::
  probe_drain` が `CoverageEvent::{Data,Status}` を記録、
  live binding で prune、`probe_coverage()`/
  `compare_probes()` accessor 追加。`lib.rs` export。
- [x] T1-4: `tests/sf04_s7_probe_compare.rs` 10 件 —
  Aligned/Conditional/Incomparable 全到達分岐を実 engine
  fixture で固定。
- [x] T1-5: docs チェーン 6 件。
- [ ] T1-6: canonical 検証・commit/push/PR・Issue #107
  コメント。

## Classification

- 実装タスク（headless consumer + headless test + docs）。
  runtime/RT 経路の変更なし。

## Upstream

- Issue #107 SF-PROBE-07、Issue §4 契約
  `ProbeComparisonRequest → AcousticObservation`。
- Ruviform #309 `typed/probe/time.rs` @ `fd1d4d5`（pin 更新
  不要を確認済み）。
- `docs/specs|designs|plans/sf-04-slice6-probe-compare*`。

## Affected Files

- `crates/synvelis-project/src/probe.rs`（契約型追加のみ、
  lib.rs 非変更 — `synvelis_project::probe::*` で到達）。
- `crates/synvelis-player/src/probe_compare.rs`（新規）。
- `crates/synvelis-player/src/probe_trace.rs`（coverage 記録・
  accessor）。
- `crates/synvelis-player/src/lib.rs`（module 宣言）。
- `crates/synvelis-player/tests/sf04_s7_probe_compare.rs`
  （新規）。
- `docs/` 6 件。

非対象: `routing_engine.rs`/`host.rs`/`routing_lower.rs`/
`synvelis-project/src/lib.rs`/`probe_trace.rs`（project）/
`synvelis-app/**`/`synvelis-workspace/**`/`mise.toml`/
`Cargo.lock`/docs index。

## Done-When

- [x] 契約型が `synvelis_project::probe` で到達可能・
  runtime id 非含有。
- [x] `probe_compare`/`compare_probes` 実装・`sf04_s7_*`
  10 test green。
- [x] seek/epoch 不連続・generation 境界・coverage gap が
  `Unknown`/`Incomparable`/`Conditional` として正直に
  報告されることを test で証明。
- [x] `cargo test -p synvelis-player --test
  sf04_s7_probe_compare` green（10/10）。
- [ ] canonical `mise run` 検証 9 件 green。
- [ ] commit/push/PR/Issue コメント/worker_done。

## Verification

実行済み: `cargo test -p synvelis-player --test
sf04_s7_probe_compare`（10 pass / 0 fail）、
`cargo check -p synvelis-player` 系の途中 build。
予定: `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
`check-app-native`/`lint-app-native`/`test-msrv`/`build`。
外部 gate（実機・#109 Oracle・#108 UI・app op・consumer
照合）: NotRun — 後続 slice/外部受け入れへ委譲。
