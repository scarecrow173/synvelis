---
id: "IMPL-034W5m2YJNbKaaqcO5j3m5"
type: "impl"
status: "completed"
title: "PC-C6 TR ledger oracle 実装記録"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs"
      - type: "file"
        path: "docs/specs/pc-c6-tr-ledger-oracle.md"
      - type: "file"
        path: "docs/adr/pc-c6-tr-ledger-oracle.md"
      - type: "file"
        path: "docs/designs/pc-c6-tr-ledger-oracle-design.md"
      - type: "file"
        path: "docs/plans/pc-c6-tr-ledger-oracle-plan.md"
      - type: "file"
        path: "docs/test-specs/pc-c6-tr-ledger-oracle.md"
      - type: "file"
        path: "docs/tasks/pc-c6-tr-ledger-oracle-task.md"
      - type: "file"
        path: "docs/impl/ir/pc-c6-tr-ledger-oracle.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/pc-c6-tr-ledger-oracle-task.md"
    - "docs/plans/pc-c6-tr-ledger-oracle-plan.md"
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/pc-c6-tr-ledger-oracle-task.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/impl/ir/pc-c3-05-rt-trace-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/adr/pc-c6-tr-ledger-oracle.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
  verified-by: []
  references:
    - "docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/impl/ir/pc-c3-05-rt-trace-adapter.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C6 TR ledger oracle 実装記録

## Summary

`crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`（新規、
6 tests）を追加し、TR01/03/04/05/07/08 を
`DispatchTraceAdapter` + `ParamDispatch` + in-process fake plugin +
`GraphRuntime` の実経路で再証拠化した。期待値は fixture 宣言から
test 内独立 oracle が導出し、`ControlTrace`/`TraceMetrics` の観測と
field 単位で照合する。`src/`・他 crate・manifest・mise・CI は無変更。

## Context

Issue #103 PC-C6 の TR 系行のうち、`crates/synvelis-project/tests/
pc_c6_oracle_tr.rs:11-13` が `synvelis-player` dispatch-ledger path
と bounded history retention (TRACE-C3-05) を `NotRun` と記録していた
残件を閉じる slice。先行 slice
（`docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md`）の project
層証拠を実経路で再証拠化するものであり、adapter 単体契約は
`trace_c3_05.rs`（#100）の責務として重複させない。

## Implementation

### Changes

- `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`（新規）:
  - fixture 基盤: media time/span/call window/write/timestamp/live
    context helpers、fixture 値（project_id 7・revision 5・
    graph_generation 11・params 21..=25・targets 101/201/301/302/
    401/501/502/601..605）は `trace_c3_05.rs`・`pc_c6_oracle_tr.rs`
    と意図的に別系統。
  - rig: `host_plugin`（PluginHost lifecycle + attach）、
    `fabricated_adapter`（GraphRuntime 無し・宣言 span/call
    window のみ）、`ledger_rig`（Source→plugin→Sink graph を
    compile/publish し `RtRuntime` を駆動、実 scheduler receipt 経路）。
  - 独立 oracle: `oracle_context_stale`/`oracle_stamp_freshness`/
    `oracle_channel_freshness`/`oracle_freshness`/`oracle_is_coherent`/
    `assert_trace_agrees` — `ControlTrace` メソッドを呼ばず
    `pc_c6_oracle_tr.rs` と同一規則を再実装。
  - tests: `tr01_stage_independence_real_ledger`、
    `tr03_context_change_stale_real_ledger`、
    `tr04_plugin_unavailable_vs_probe_states`、
    `tr05_plugin_reported_stamp_provenance`、
    `tr07_shared_probe_two_contexts_real_ledger`、
    `tr08_bounded_retention_metrics_real_ledger`。
- docs チェーン 7 file（spec/design/plan/task/test-spec/ADR/impl-ir）
  を新規作成。docs index（`docs/*/README.md`）は Coordinator が
  統合時に処理するため本 slice では変更していない。

### Decisions During Implementation

- **TR08 は fabricated adapter 経路を採用**: bounded retention の
  metrics 会計（`events_recorded`/`history_lost`/`targets_lost`/
  `expired_correlations`/`dropped_observations`）は receipt を持た
  ない宣言 span で厳密に手計算できる。ledger 経路は receipt attribution
  が event 数を左右するため、TR08 の会計照合には fabricated が適切。
  TR01/03/04/05/07 は receipt 依存の証拠（scheduled/applied slot・
  re-key）が必要なため ledger rig を使う。
- **`call_window` は span 全体を 1 call**: `call_start` は
  `EngineFrame::new(span_epoch(), 0)`、`call_frames` は
  `span.frames()`。dispatch の `position`/`frame_offset` は宣言値と
  field 照合する。
- **TR03 の re-key 検証**: 2 回目の実 dispatch で
  `key.control_plan_generation` が gen2 に更新され、gen1 context が
  `Stale{ControlPlan}` になることを確認。trace key を合成編集せず、
  実 dispatch 経路のみで re-key を起こす。
- **TR08 会計値の手計算根拠**（宣言操作列: dispatch 5・observe 2・
  configured 1・probe 3、capacity 4・max_targets 3・
  plan_gen_window 2）:
  - `events_recorded = 14`: dispatch 5（DispatchPlanned 各 1）+
    observe 2（ReceiptRecorded は fabricated では 0、ただし
    DispatchPlanned/ProbeRecorded 等）+ configured 1 + probe 3 +
    TargetEvicted 3 — adapter の `push_event` 会計に一致。
  - `history_lost = events_recorded - history_capacity = 10`。
  - `targets_lost = 2`: max_targets 3 に対し 5 target を dispatch
    したため LRU eviction が 2 回発生。
  - `expired_correlations = 1`: plan_gen_window 2 を超えた古い
    generation の相関が 1 件 expiry。
  - `dropped_observations = 1`: PlanReplaced drop が 1 件。
  - `latency_samples = 0`: fabricated 経路は receipt を持たず
    latency sample を生成しない。
  - retained history kind 列 `[ConfiguredRecorded, ProbeRecorded×3]`
    は ring buffer が最古から eviction する性質と一致。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（2026-11-12、origin/main `c04a198` merge 済み HEAD
で全 lane 再実行。初回 merge `7b348d2` 時点でも全 lane pass、
その後 upstream の synvelis-player/project 変更を取り込むため
再 merge・再検証した）:

- `mise run fmt-check` → exit 0
- `mise run check` → exit 0
- `mise run lint` → exit 0
- `mise run test` → exit 0（workspace 全 test binary `test result:
  ok`、失敗 0。`pc_c6_oracle_tr_dispatch` 6 tests 全 pass:
  tr01/tr03/tr04/tr05/tr07/tr08）
- `mise run test-rt` → exit 0（`rt_` filter、全 pass）
- `mise run check-app-native` → exit 0
- `mise run lint-app-native` → exit 0
- `mise run test-msrv` → exit 0（rust 1.89.0 toolchain）
- `mise run build` → exit 0

補助証拠: 直接 cargo 実行（`cargo test -p synvelis-player --test
pc_c6_oracle_tr_dispatch`）6 tests 全 pass。

## Risks

- TR08 の会計値は adapter 内部の event 生成順に依存する。内部変更は
  本 test の fail として顕在化させ、会計の再導出を要求する
  （bounded retention の性質上不可避）。
- `GraphRuntime` 経路は決定的 headless であり、実 scheduler の
  timing/RT 性は証明しない。`test-rt` canonical と #114 gate が
  その境界を担う。
- oracle 自体の誤実装リスクは先行 slice と同じ — spec/design/
  test-spec の規則を一次情報とし、差異時は契約解釈を再確認する。

## Follow-ups

- TR02/TR06 の実経路再証拠: project 層で証拠化済み。必要なら別 slice。
- 実 GUI/plugin/OS/RT/consumer 照合: #114 gate・Ruviform consumer
  gate に委譲（`NotRun` 記録）。
- GUI-PC-01〜04 scenario 対応表: #114・sv112 着地後の別作業。
- PC-C1-R01〜08・PC-C4-PL01〜08・PC-C5-UI01〜07・
  PC-C6-COMP01〜05: Issue の残件、別 slice。
- docs index（`docs/*/README.md`）: Coordinator が統合時に処理。
