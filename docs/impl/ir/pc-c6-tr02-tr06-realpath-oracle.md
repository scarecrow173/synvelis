---
id: "IMPL-034WxwExc8oWsdZ2zey3N1"
type: "impl"
status: "completed"
title: "PC-C6 TR02/TR06 realpath oracle 実装記録"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
      - type: "file"
        path: "docs/adr/pc-c6-tr02-tr06-realpath-oracle.md"
      - type: "file"
        path: "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
      - type: "file"
        path: "docs/plans/pc-c6-tr02-tr06-realpath-oracle-plan.md"
      - type: "file"
        path: "docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md"
      - type: "file"
        path: "docs/tasks/pc-c6-tr02-tr06-realpath-oracle-task.md"
      - type: "file"
        path: "docs/impl/ir/pc-c6-tr02-tr06-realpath-oracle.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/pc-c6-tr02-tr06-realpath-oracle-task.md"
    - "docs/plans/pc-c6-tr02-tr06-realpath-oracle-plan.md"
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
    - "docs/adr/pc-c6-tr02-tr06-realpath-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/pc-c6-tr02-tr06-realpath-oracle-task.md"
    - "docs/plans/pc-c6-tr02-tr06-realpath-oracle-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-c6-tr-ledger-oracle.md"
    - "docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/impl/ir/pc-c3-05-rt-trace-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/adr/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md"
  verified-by: []
  references:
    - "docs/impl/ir/pc-c6-tr-ledger-oracle.md"
    - "docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C6 TR02/TR06 realpath oracle 実装記録

## Summary

`crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`（既存
file、6→8 tests）に TR02・TR06 の 2 test を追加し、
`DispatchTraceAdapter` + `ParamDispatch` + in-process fake plugin +
`GraphRuntime` の実経路で TR02/TR06 の oracle 分類規則を再証拠化
した。`src/`・他 crate・manifest・mise・CI・他 TR 行は無変更。
本 slice により TR 系 8 行全てが dispatch-ledger 実経路で証拠化
された。

## Context

Issue #103 PC-C6 の PC-C3-TR02/TR06 行の残件。先行 slice
（`docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md`）の
project 層証拠（`pc_c6_oracle_tr.rs`）は分類規則を証拠化したが
dispatch-ledger 実経路を `NotRun` と記録し、直前 slice
（`docs/impl/ir/pc-c6-tr-ledger-oracle.md`、PR #354）は TR01/03/
04/05/07/08 を実経路で再証拠化しつつ TR02/TR06 を「project 層で
証拠化済み、必要なら別 slice」として Deferred に記録した。本
slice がその残件を閉じる。

## Implementation

### Changes

- `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`:
  - `tr02_context_mismatch_dimensions_real_ledger`（新規）: target
    701 を bind し実 plan を dispatch → `record_reported` +
    `GraphRuntime` 駆動 + `observe_cycle` + trusted carried probe
    で live evidence を構成。key の `project_revision`(5)・
    `graph_generation`(11)・`control_plan_generation`(実 dispatch
    採番) が実経路生成物であることを assert。7 次元それぞれを
    1 次元ずつ動かした context が対応 `StaleReason` の `Stale` に
    分類されることを独立 oracle（`oracle_freshness`）と実装
    （`trace.freshness`/`observed_freshness`）の双方で field 照合。
    全次元同時不一致が documented check order 先頭の
    `StaleReason::ProjectRevision` に解決されること、全ケースで
    `is_coherent()` が true のままであることを検証。
  - `tr06_silence_vs_unobserved_real_ledger`（新規）: sibling
    target 801/802 を同一実 plan で dispatch。801 に
    `MeasuredValue::Silence` + carried stamp を `record_probe` で
    記録し、stored `ProbeObservation::Measuring` が `Silence`・
    timestamp・provenance を field 単位で保持すること、
    observed/combined freshness が `Current` であることを検証。
    未 touch の 802 が `Unavailable{NoEvidence}`、続けて
    `Missing`→`ProbeMissing`・`Disabled`→`ProbeDisabled`・
    `Lost{count:4}`→`DataLoss{4}` に分類されること、各段階で
    801 が `Current` のまま（probe 状態が target 間で漏れない）
    ことを検証。
  - file 冒頭 doc comment を TR01〜TR08 全行対象の記述に更新
    （TR02/TR06 を明示追加）。
  - `#[allow(clippy::too_many_lines)]` を新規 2 test に付与
    （既存長大 scenario test と同じ用途・同じ attribute、
    lint 隠蔽ではない）。
- docs チェーン 7 file（spec/ADR/design/plan/test-spec/task/impl-ir）
  を新規作成。触れた 7 dir の管理 index（`docs/*/README.md`、
  `docs/impl/ir/README.md`）を doc-maintenance `index apply` で
  再生成し `index plan` で in-sync を確認。

### Decisions During Implementation

- **既存 file への test 追加を採用**（ADR
  `pc-c6-tr02-tr06-realpath-oracle`）: rig・fixture・独立 oracle
  （`oracle_*`/`assert_trace_agrees`/`live_ctx`）を再利用し、
  ~700 行の基盤複製を避ける。file は TR01〜TR08 全行をカバーする
  「TR 分類規則の実経路再証拠化」file となった。
- **TR02 の実経路 evidence 構成**: `record_reported`（plugin
  channel）+ `record_probe(Measuring, Carried)`（trusted probe
  channel）の両 channel に live evidence を持たせ、context
  mismatch が channel 評価に先立つこと（`context_stale` が
  `freshness_for` の先頭）も `observed_freshness` の一致で固定。
- **TR02 の composite 順序**: 全 7 次元同時不一致の期待値を
  `StaleReason::ProjectRevision` に固定 — `trace.rs` の
  `context_stale` の documented check order（project_revision →
  control_plan_generation → graph_generation →
  transport_generation → clock_epoch → loop_occurrence →
  evaluation_position）と一致。実装が順序を変えれば test が fail
  で顕在化する。
- **TR06 の sibling 構成**: 1 つの実 plan が 2 target を含み、
  両者が実 receipt 経路の `Dispatched` evidence を持つ。probe
  記録は target 単位で `record_probe` を呼び分け、801 の
  `Silence` が 802 の channel state に影響しないことを各段階で
  再 assert する。
- **`observed` slot の `Expired`/`Dropped`/`Unsupported` variant は
  要求しない**: 実経路（`observe_cycle` + `record_probe`）が
  生成しない状態であり、それらの写像は `trace_c3_05.rs`・
  project 層 oracle の責務。行の証拠範囲を実経路証拠の境界に
  正直に合わせた。

## Related Experiments

### Adopted

<!-- なし — 実験 log は未使用。 -->

### Rejected

<!-- なし。 -->

## Validation

canonical 検証（2026-11-13、`origin/main` `323d434` merge 済み
HEAD で全 lane 実行。OS: Windows、toolchain: rust stable
（`mise run` 経由）+ `test-msrv` は rust 1.89.0）:

- `mise run fmt-check` → exit 0（`mise run fmt` で新規 test の
  整形を適用後に確認）
- `mise run check` → exit 0
- `mise run lint` → exit 0
- `mise run test` → exit 0（workspace 全 test binary `test result:
  ok`、失敗 0。`pc_c6_oracle_tr_dispatch` 8 tests 全 pass:
  tr01..tr08、tr02/tr06 新規含む）
- `mise run test-rt` → exit 0（`rt_` filter、全 pass）
- `mise run check-app-native` → exit 0
- `mise run lint-app-native` → exit 0
- `mise run test-msrv` → exit 0（rust 1.89.0 toolchain、
  `pc_c6_oracle_tr_dispatch` 8 tests 全 pass）
- `mise run build` → exit 0

補助証拠: `cargo test -p synvelis-player --test
pc_c6_oracle_tr_dispatch` → 8 tests 全 pass（2026-11-13、
`mise run test` 内の同 binary 結果と一致）。

fixture matrix ledger（PC-C6-R4、headless + in-process fake
plugin、Synvelis commit = 本 slice HEAD、Ruviform SHA =
`Cargo.lock` resolved rev `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`、
OS = Windows、Backend = headless `GraphRuntime`/`RtRuntime`、
Fixture = `pc_c6_oracle_tr_dispatch.rs`）:

| Case | Expected | Observed | Result |
| --- | --- | --- | --- |
| PC-C3-TR02 | 7 次元単独 mismatch が対応 `StaleReason` の `Stale`、composite が `ProjectRevision`、coherence 維持 | `tr02_context_mismatch_dimensions_real_ledger` pass — oracle/実装 field 一致 | Pass（headless/fake plugin） |
| PC-C3-TR06 | `Silence`→`Current`、`Missing`/`Disabled`/`Lost`/未 touch が別の正直な分類、target 間漏洩なし | `tr06_silence_vs_unobserved_real_ledger` pass — field 一致 | Pass（headless/fake plugin） |

review-protocol: cooperative（Reviewer A）+ adversarial
（Reviewer B）の独立 2 pass で docs+code をレビュー。Reviewer B
は blocker なしを確認（oracle 独立性・check order・TR02/TR06
契約網羅・doc 整合を `trace.rs` 実装と照合）。Reviewer A の
指摘は P2×1（impl record の検証欄が未記入 — 本記録で解消）と
P3×2（ADR の行数表記・test-spec TR06-r1 の対象表現 — 修正済み）。
CONFIRMED blocker なし。

## Risks

- TR02 の composite 順序は `trace.rs` `context_stale` の check
  order に依存 — 実装が順序を変更した場合本 test が fail で
  顕在化し、契約変更として spec/design/test-spec の再確認を
  要求する。
- TR06 の sibling 分離は probe channel が target 単位で独立で
  あることを前提 — adapter が target key で分離しなくなった場合
  fail で顕在化する。
- `GraphRuntime` 経路は決定的 headless であり、実 scheduler の
  timing/RT 性は証明しない。`test-rt` canonical と #114 gate が
  その境界を担う。
- oracle 自体の誤実装リスクは先行 slice と同じ — 規則の一次情報は
  `docs/test-specs/pc-c6-e2e-oracle-rel-tr.md` Rule(TR02)/
  Rule(TR06) と spec/design であり、差異時は契約解釈を再確認する。

## Follow-ups

- 実 GUI/実 VST3/実 CLAP/実 OS/実 RT/consumer 照合: #114・#101・
  #102・Ruviform consumer gate に委譲（`NotRun` 継続、headless
  結果を実機成功に繰り上げない）。
- GUI-PC-01〜04 scenario 対応表: #114・sv112 着地後の別作業。
- PC-C1-R01〜08・PC-C4-PL01〜08・PC-C5-UI01〜07・
  PC-C6-COMP01〜05: Issue の残件、別 slice。
- 本 slice の merge により PC-C3-TR01〜08 の実経路再証拠は完結。
  Issue #103 は Open 継続（`Refs` のみ、Close しない）。
