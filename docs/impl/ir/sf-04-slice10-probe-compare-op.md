---
id: "IMPL-8jFdP2wErT5yU7iO1aQ4sG"
type: "ir"
status: "completed"
title: "SF-04 slice10 実装記録 — syn.probe.compare app op（ProbeComparisonRequest → AcousticObservation の wire 露出）"
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
    - "docs/tasks/sf-04-slice10-probe-compare-op-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/sf-04-slice10-probe-compare-op-task.md"
    - "docs/designs/sf-04-slice10-probe-compare-op-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/sf-04-slice10-probe-compare-op.md"
  defers: []
  deferred-by: []
---

# SF-04 slice10 実装記録 — syn.probe.compare app op

## Summary

Issue #107 SF-PROBE-07 の app-facing 露出を実装した。
slice 6 の `ProbeTraceBridge::compare_probes` を
`syn.probe.compare` canonical op として `AppService`
dispatch 面へ追加し、`AcousticObservation`（非 serde・
clock id 含有）を手動 JSON mapper で wire 投影した。
比較は `&self` read-only — 最後の `ui_pump` が保持した
retained coverage のみを読み、drain も mutation もしない。
player/project/workspace/ui・port trait は変更していない。

## Implemented

### `crates/synvelis-app/src/service.rs`

- `OPS` に `syn.probe.compare` `OpInfo` を追加。
- `dispatch` に `"syn.probe.compare" =>
  self.app.op_probe_compare(params)` arm を追加。

### `crates/synvelis-app/src/ui_ops.rs`

- `synvelis_project::probe` contract 型の import
  （`AcousticObservation`/`ClockContextRef`/
  `ProbeAlignCondition`/`ProbeComparisonBasis`/
  `ProbeComparisonGap`/`ProbeComparisonProvenance`/
  `ProbeComparisonQuality`/`ProbeComparisonRequest`/
  `ProbeIncomparableReason`）。
- mapper helpers: `probe_clock_context_json`（clock id を
  `{domain,epoch}` で明示投影）・
  `probe_comparison_basis_json`・`probe_align_condition_json`・
  `probe_incomparable_reason_json`（全 variant を stable
  `kind` + payload で投影）・`probe_comparison_quality_json`・
  `probe_comparison_gap_json`・`probe_comparison_provenance_json`・
  `probe_observation_json`。`#[non_exhaustive]` の `_` arm は
  `{"kind":"unknown"}` で fail-closed。
- `op_probe_compare(&self, params)`: `json_param` で
  `ProbeComparisonRequest` を parse（engine check より先 —
  `INVALID_PARAMS`）→ engine 無しは
  `{engine_attached:false, observation:null}` を Ok →
  `engine.with_probe_bridge(|port| port.compare_probes(&request))`
  → `probe_observation_json` で `{engine_attached:true,
  observation:{...}}` を返す。

### `crates/synvelis-app/tests/sf04_probe_compare_op.rs`（新規）

6 件の headless integration test（`sf04_probe_wiring.rs`
fixture 先例・`UiRuntime`/`AppService` dispatch 経由）:

- `compare_fails_closed_without_engine` — engine 未接続で
  `engine_attached:false` + `observation:null`、malformed
  request は `INVALID_PARAMS`。
- `compare_aligned_two_monitored_points` — master strip
  `PostFader`/`PreFader` の monitor subscribe → run+pump →
  `aligned` + `shared-cursor` basis + provenance 検証。
- `compare_conditional_partial_coverage` — `observed_range`
  拡張で `partial-coverage` condition + `unknown` gap 降格。
- `compare_incomparable_resolution_failures` —
  `point-unresolved`/`probe-not-bound`。
- `compare_incomparable_window_and_budget_failures` —
  `no-observation-data`/`coverage-empty`/
  `evidence-budget-exceeded`。
- `compare_is_read_only_and_never_drains` — drain/bindings/
  project info 不変・同一 evidence → 同一 observation。

## Decisions

- **`&self` read-only（pump しない）**: `ui_pump` 末尾の
  `drain_probe` が session 唯一の drain owner（slice 4 設計）。
  query op が暗黙に drain すると read op が drain report・
  coverage・trace slot を mutate し、`syn.probe.state` との
  観測順序を壊す。`&mut self` + `ui_pump()` 先行案は不採用
  — `syn.probe.state` と同じ「保持証拠の投影」分類。
  stale/欠落証拠は `incomparable`/`conditional` に正直に
  劣化し `aligned` を偽装しない。
- **engine 未接続は Ok + `engine_attached:false`**:
  `syn.probe.state`（read op → 空投影で 200）先例。
  `SERVICE_ERROR`（`op_probe_control` 先例）は lifecycle/
  action op の契約であり、query op が error を返すと
  consumer が「比較不能」と「エラー」を区別する追加
  ロジックを要する。`observation:null` が「観測なし」を
  明示し捏造を防ぐ。
- **malformed request は engine check より先に
  `INVALID_PARAMS`**: `op_probe_control` の検証順序先例 —
  param fault は環境状態に依存しない決定的エラー。
- **手動 JSON mapper**: `AcousticObservation` は clock id
  を含む runtime evidence で serde 対象外（slice 6 設計 —
  `ClockContextRef` 規則）。`before`/`after`/`interval` は
  serde contract 型で verbatim、`quality`/`basis`/`gap_mask`/
  `provenance` は field-by-field で wire shape を明示決定。
  clock id は `{domain,epoch}` raw id で投影（推測しない）。

## Verification

- `cargo test -p synvelis-app --test sf04_probe_compare_op`:
  6 passed / 0 failed（2026-11-12）。
- canonical `mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`check-app-native`/`lint-app-native`/`test-msrv`/
  `build`: 実施結果は PR/worker 報告を参照。
- 外部 gate: 実機・#109 Oracle・#108 UI・SF-PROBE-08/09・
  SF04-RCP-01・`ui/src/ipc.ts` OpName 登録は **NotRun** —
  後続 slice/外部受け入れへ委譲。Pass 扱いしない。

## Remaining / Out of Scope

- `ui/src/ipc.ts` `OpName` union・per-node adapter 登録
  （#108/SF-05）。
- signal payload 比較（`metric_set` は intent のみ —
  SF-PROBE-08/09）。
- `syn.probe.compare` の streaming/push 露出 — pull 型
  query op のみ。
- 実機計測・#109 oracle・#108 UI — 外部 gate。
