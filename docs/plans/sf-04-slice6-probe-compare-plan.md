---
id: "PLAN-6hJ4nVbC8xZ2qW5eRtYuIo"
type: "plan"
status: "approved"
title: "SF-04 slice6 実装計画 — 複数 Probe 整列比較 consumer（ProbeComparisonRequest → AcousticObservation）"
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
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice6 実装計画 — 複数 Probe 整列比較 consumer

## Goal

`ProbeComparisonRequest → AcousticObservation` 契約を
`synvelis_project::probe` に定義し、Ruviform #309
`align_observation_points` を消費する非 RT・bounded・
fail-closed の `probe_compare` free function と、drain 経路の
`CoverageEvent` 保持（`ProbeCoverage`）を player 層に実装する。
`RoutingEngine`・port 契約・app/session 層は変更しない。

## Scope

単一実装ストリーム。変更は allowed file 集合に限定
（`synvelis-project/src/probe.rs`、`synvelis-player/src/
{probe_compare.rs 新規, probe_trace.rs, lib.rs}`、
`synvelis-player/tests/sf04_s7_probe_compare.rs` 新規、
docs 6 件）。`synvelis-workspace`・`Cargo.lock`・README
index は coordinator 統合時処理のため触らない。

## Upstream Documents

- `docs/specs/sf-04-slice6-probe-compare.md`
- `docs/designs/sf-04-slice6-probe-compare-design.md`

## Design Inputs

- `docs/designs/sf-04-slice6-probe-compare-design.md`
  （approved）— 契約型・`ProbeCoverage`・`probe_compare`
  処理順・`build_side` 証拠規則・bridge 配線の正本。
- Ruviform `typed/probe/time.rs` @ `fd1d4d5` —
  `align_observation_points`/`ObservationCoverage::reconstruct`/
  `CoverageEvent`/`AlignPolicy`/`Alignment`/`IncomparableReason`
  の意味論正本（pin 更新不要を確認済み）。

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-project/src/probe.rs` | slice 6 契約型一式を追加（request/result/policy mirror/gap/provenance） |
| `crates/synvelis-player/src/probe_compare.rs` | 新規 — `ProbeCoverage` + `probe_compare` free function + 写像 helper |
| `crates/synvelis-player/src/probe_trace.rs` | `coverage` field、drain 時 `CoverageEvent` 記録、prune、`probe_coverage()`/`compare_probes()` |
| `crates/synvelis-player/src/lib.rs` | `pub mod probe_compare` + re-export |
| `crates/synvelis-player/tests/sf04_s7_probe_compare.rs` | 新規 — 10 件の headless integration test |
| `docs/` | spec/design/plan/task/test-spec/impl-ir 6 件 |

変更しない: `synvelis-project/src/lib.rs`（sv125 lease）、
`synvelis-project/src/probe_trace.rs`、`synvelis-player/src/
{routing_engine,host,routing_lower,probe}.rs`（sv106 lease —
`probe.rs` は変更不要と判断）、`synvelis-workspace/**`、
`synvelis-app/**`、`mise.toml`、`Cargo.lock`、docs index。

## Implementation Sequence

- [x] T1-1: `synvelis-project/src/probe.rs` に契約型を追加。
- [x] T1-2: `probe_compare.rs` 新規 — `ProbeCoverage`（bounded
  series + loss floor）と `probe_compare`（resolve_side →
  build_side → budget → ObservationSpan → align → 写像）。
- [x] T1-3: `probe_trace.rs` 配線 — drain 記録・prune・
  accessor、`lib.rs` export。
- [x] T1-4: `sf04_s7_probe_compare.rs` — test-spec F1–F10 を
  実 engine fixture で固定（Aligned/Conditional/Incomparable
  全到達分岐）。
- [x] T1-5: docs 6 件作成。
- [ ] T1-6: canonical 検証（`mise run fmt-check`/`check`/
  `lint`/`test`/`test-rt`/`check-app-native`/
  `lint-app-native`/`test-msrv`/`build`）・review・impl record
  最終化・commit/push/PR・Issue #107 コメント。

## Dependencies and Risks

- 依存: Ruviform `fd1d4d5` の #309 API（存在確認済み、
  pin 変更なし）。`ProbeTraceBridge`/`RoutingEngine` の
  既存公開 read model のみ。
- リスク: coverage は bridge drain 経路依存 — 直接
  `engine.probe_drain` を使う consumer は `NoObservationData`
  で正直に失敗（fail-closed、設計どおり）。
- リスク: `observed_range=None` の extent が segment
  `frame_range` で延びる場合の分類 — 未観測部は `Unknown`
  gap に落ちるため validity を偽装しない。
- リスク: generation 境界で semantic point が移動し probe
  id が変わると cross-generation 証拠は prune される —
  point 不変 republish で `GenerationSpan` を再現する
  test 設計で回避（test 内で probe id 不変を assert）。

## Task Breakdown

- `docs/tasks/sf-04-slice6-probe-compare-task.md` — T1
  （本計画の唯一の task。実装〜検証〜PR まで）。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `lint-app-native` /
  `test-msrv` / `build` が green。
- [ ] `sf04_s7_probe_compare.rs` 10 件が green で
  test-spec F1–F10 を網羅。
- [ ] 既存 `sf04_*` テストが無変更で green（drain 経路の
  外部挙動不変の証拠）。
- [ ] 外部 gate（実機/#109/#108/app op）は NotRun 記録。

## Open Questions / Gaps

- `syn.probe.compare` app op は対象外（`service.rs`/
  `engine.rs` 他 Worker 占有）— 後続 slice で消費。
- session 層（`synvelis-workspace`）への露出は不要と判断 —
  `ProbeTraceBridge::compare_probes` が player 層の公開面。
  必要になれば additive に追加可能。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 worker task）。
- Independent work streams: none。
- User approval required before dispatch: no（coordinator task
  spec が実装を直接指示）。

## Review Handoff

- reviewer focus: `resolve_side`/`build_side` の fail-closed
  網羅性、loss floor の保守方向、`gap_mask` が request 未観測
  領域を `Unknown` で正直に報告するか、runtime `Alignment`
  写像の将来 variant fail-closed、`RoutingEngine` 非依存性。
