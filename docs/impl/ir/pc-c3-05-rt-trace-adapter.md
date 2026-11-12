---
id: "IMPL-nZ0nDD3aP4qQxwynUrbE0g"
type: "impl"
kind: "ir"
status: "completed"
title: "PC-C3-05 実装記録 — synvelis-player RT trace adapter（TRACE-C3-05）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/trace.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/trace_c3_05.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/trace_c3_05_rt.rs"
      - type: "file"
        path: "docs/specs/pc-c3-05-rt-trace-adapter.md"
      - type: "file"
        path: "docs/adr/pc-c3-05-trace-adapter-drain-history.md"
      - type: "file"
        path: "docs/designs/pc-c3-05-rt-trace-adapter-design.md"
      - type: "file"
        path: "docs/plans/pc-c3-05-rt-trace-adapter.md"
      - type: "file"
        path: "docs/test-specs/pc-c3-05-rt-trace-adapter.md"
      - type: "file"
        path: "docs/tasks/pc-c3-task2-rt-trace-adapter.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/dispatch.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/*/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/*/README.md"
  implements:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/plans/pc-c3-05-rt-trace-adapter.md"
    - "docs/tasks/pc-c3-task2-rt-trace-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/pc-c3-task2-rt-trace-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-c3-control-trace-projection.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/pc-c3-task2-rt-trace-adapter.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c3-05-rt-trace-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C3-05 実装記録 — synvelis-player RT trace adapter

## Summary

`crates/synvelis-player` に `DispatchTraceAdapter` を実装。`ParamDispatch` を所有権ごと受け取り、report channel の drain を `observe_cycle` 単一経路に限定（single drain owner の型レベル表現）。ledger observation・scheduler receipt・外部 ingest を `synvelis_project::trace::ControlTrace` の stage slot へ写像し、bounded history・metrics・generation 相関 window を備える。project/workspace/app/recording/Ruviform は無変更。

## Context

Issue #100 (PC-C3 / TRACE-C3) の player 側 slice (TRACE-C3-05)。
project 層の `ControlTrace` projection は
`docs/impl/ir/pc-c3-control-trace-projection.md`
(TRACE-C3-01..04) で実装済みであり、本 slice はその RT 側
adapter を `synvelis-player` に追加する。

Spec: docs/specs/pc-c3-05-rt-trace-adapter.md
ADR: docs/adr/pc-c3-05-trace-adapter-drain-history.md
Design: docs/designs/pc-c3-05-rt-trace-adapter-design.md
Plan: docs/plans/pc-c3-05-rt-trace-adapter.md
Test spec: docs/test-specs/pc-c3-05-rt-trace-adapter.md
Task: docs/tasks/pc-c3-task2-rt-trace-adapter.md

変更は `synvelis-player` 内に閉じ、project/workspace/app/
recording/Ruviform は無変更。

## Implementation

- `src/trace.rs`: `DispatchTraceAdapter`/`TraceAdapterConfig`/`TraceContext`/`TraceMetrics`/`TraceEvent`/`TraceEventKind`。写像規則は design「写像規則」表どおり。`DispatchStage`/`DropReason` → project `DispatchEvidence`/`DispatchDropReason` の mirror は `map_observation`/`map_drop_reason` に集約。
- `src/dispatch.rs`: `DispatchCycle { observations, receipts }` + `observe_cycle`（`observe` は wrapper として不変維持）+ `record_reported`/`mark_reporting_unavailable` passthrough + `in_range` の `pub(crate)` 化。
- `src/lib.rs`: `pub mod trace` + additive export。`Cargo.toml`: dev-dep `stats_alloc`（synvelis-project と同 0.1 系）。
- tests: `trace_c3_05.rs`（5 件: stage 記入・receipt 相関・Reported/Unavailable・PlanReplaced drop・bounded/Expired）+ `trace_c3_05_rt.rs`（`rt_` alloc 計測 1 件）。

### Key Decisions (vs design)

- ledger revision ↔ dispatch ordinal の 1:1 前提を `plan_gens` window で実装（upstream `record_plan` 規約依存 — ADR 記載どおり）。
- `plugin_reported` stamp は drain 時点 span 位置を `Estimated` provenance で付与 — `Current` にならないことを test で assert。
- key の transport/clock gens は receipt 帰属時に receipt 値を優先、無帰属時は span ctx 値。project 系 field は新規証拠記録時のみ更新（旧証拠を新 revision と偽装しない）。

## Related Experiments

`metadata.experiments.adopted` / `rejected` はともに空。
`docs/impl/exp/` にも PC-C3-05 RT trace adapter に対応する実験記録
（`.jsonl` / `.md`）は存在しない。

## Validation

- 補助証拠（canonical = mise）: `cargo test -p synvelis-player --test trace_c3_05 --test trace_c3_05_rt` → 6 passed（rt alloc 0/0 実測）。
- canonical: `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` — 実行結果は PR 本文に記録。

## Risks

- ledger revision ↔ dispatch ordinal の 1:1 前提は upstream
  `record_plan` 規約に依存する（ADR 記載）— upstream 側の記録
  規約が変わると generation 相関 window の帰属が崩れ得る。
- `plugin_reported` stamp は drain 時点 span 位置を `Estimated`
  provenance で付与し `Current` にならない — freshness の過小
  評価は意図的だが、`Current` を期待する consumer 設計とは
  整合の確認が必要。
- bounded history の overflow/drop は `Dropped`/`Expired` slot
  として記録される設計 — 長時間運転で evidence が失われることを
  consumer が認識する必要がある。
- single drain owner は adapter が `ParamDispatch` の所有権を
  持つことで型レベルに表現されるが、adapter 外で dispatch を
  直接保持・drain する経路は型では排除されない
  （`observe` wrapper は不変維持）。

## Follow-ups

- workspace `ControlSession` 接続・UI・2 Inspector 防止・driver 搭載 — 後続 integration slice。
- 実 plugin→host param report channel・実 OS/RT 観測 — upstream/環境依存 NotRun。
- #101/#103/#114 との Revision/Case ID 統合 — 各 Issue gate。
- doc graph router は main 既存の broken relation（本 slice 無関係）で blocked — 文書チェーンは手動で spec→adr→design→plan→test-spec→task→impl を作成し、index を再生成済み。
