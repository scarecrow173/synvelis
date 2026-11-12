---
id: "TSPEC-5nCxL8dSwV2yH4jK6fG1bN"
type: "test-spec"
status: "approved"
title: "SF-04 slice10 テスト仕様 — syn.probe.compare app op"
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
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
    - "docs/designs/sf-04-slice10-probe-compare-op-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
    - "docs/designs/sf-04-slice10-probe-compare-op-design.md"
  verified-by: []
  references:
    - "docs/test-specs/sf-04-slice4-app-wiring.md"
    - "docs/test-specs/sf-04-slice6-probe-compare.md"
  defers: []
  deferred-by: []
---

# SF-04 slice10 テスト仕様 — syn.probe.compare app op

## Purpose

SF-04 slice10（Issue #107 の `syn.probe.compare` app op）が、
`AppService` dispatch 経由で `ProbeComparisonRequest →
AcousticObservation` を正しく wire JSON へ投影し、
`Aligned`/`Conditional`/`Incomparable` の 3 quality と
consumer 側 fail-closed reason を誠実に返し、read-only
不変条件（drain しない・revision 不変）を守ることを
headless fixture で保証する。

## Feature/Rule/Example

### F1 — `syn.probe.compare` dispatch 登録と request parse

- Rule: `OPS` テーブルに `syn.probe.compare` が登録され、
  `dispatch` が `op_probe_compare` へ振る。`params.request`
  は serde `ProbeComparisonRequest` verbatim で parse され、
  malformed request（`request` 欠落・shape 不正）は
  `INVALID_PARAMS`。
- Example: `tests/sf04_probe_compare_op.rs` で
  `UiRuntime::dispatch_op("syn.probe.compare", params)` を
  呼び、`{}` params と `{request:{before:{},after:{}}}` が
  `INVALID_PARAMS` を返すことを確認。

### F2 — `Aligned` quality の wire 投影

- Rule: 同一 cursor series 上の 2 monitored point（例:
  master strip の `PostFader`/`PreFader`）を subscribe →
  `engine.run` + pump → compare すると
  `quality.kind=="aligned"`、`basis.kind=="shared-cursor"`、
  `offset_frames`/`error_budget_frames` が数値、
  `gap_mask==[]`、provenance の clock domain が一致・
  `sample_rate`/`runtime_instance`/generation range が
  投影される。`before`/`after` は request の echo。
- Example: `compare_aligned_two_monitored_points` —
  `AppService` dispatch で `engine_attached:true` +
  `observation.quality.kind=="aligned"` を確認。

### F3 — `Conditional` quality（partial coverage）

- Rule: `observed_range` を retained coverage の extent より
  拡張すると、未観測 tail が `Unknown` gap +
  `partial-coverage` condition + `excluded_intervals` で
  `conditional` に降格する — `aligned` への切り詰めなし。
- Example: `compare_conditional_partial_coverage` —
  まず extent を取得し `end+4096` の `observed_range` で
  再 compare、`conditions` に `partial-coverage`、
  両側 `gap_mask` に `kind=="unknown"` を確認。

### F4 — `Incomparable` consumer 側 fail-closed reason

- Rule: consumer 側解決失敗がそのまま投影される:
  `point-unresolved`（binding 無し）・`probe-not-bound`
  （binding あるが runtime probe 未解決）・
  `no-observation-data`（binding あるが coverage 未 drain）・
  `coverage-empty`（退化 window）・
  `evidence-budget-exceeded`（`max_bytes` 超過）。
  `interval` は `null`（解決失敗時）または window
  （budget 超過時）。`aligned` に格上げされない。
- Example: `compare_incomparable_resolution_failures`・
  `compare_incomparable_window_and_budget_failures` —
  各 reason の `kind`/`side`/`interval` を確認。

### F5 — engine 未接続の fail-closed

- Rule: `routing_engine` が `None` の app で有効な request
  は `{engine_attached:false, observation:null}` を Ok で
  返す — `SERVICE_ERROR` でも捏造 observation でもない。
  malformed request は engine 判定より先に
  `INVALID_PARAMS`。
- Example: `compare_fails_closed_without_engine`。

### F6 — read-only 不変条件

- Rule: compare は drain を起こさず、`syn.probe.state` の
  `drain`/`bindings`・`syn.project.info` を変えない。
  同一 retained evidence → 同一 observation（決定的）。
- Example: `compare_is_read_only_and_never_drains` —
  compare 前後で state drain/bindings・project info が
  一致、2 回の compare が同一結果。

## Out of Scope

- 実 GUI・#108 UI consumer・#109 oracle・実機計測
  （NotRun、外部 gate へ委譲）。
- signal payload 比較（SF-PROBE-08/09）、`metric_set` の
  実消費（intent のみ）。
- `ui/src/ipc.ts` OpName・per-node adapter 登録（#108/SF-05）。
- `#[non_exhaustive]` 将来 variant の具体写像 — `"unknown"`
  fallback の存在確認のみ。
