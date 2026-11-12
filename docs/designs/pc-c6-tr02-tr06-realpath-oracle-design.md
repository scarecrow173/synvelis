---
id: "DESIGN-034WxmyGWBzk3HppFdEDXE"
type: "design"
status: "approved"
title: "PC-C6 TR02/TR06 realpath oracle 設計 — 既存 file への test 追加と行対応表"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/adr/pc-c6-tr02-tr06-realpath-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/adr/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
    - "docs/designs/pc-c3-05-rt-trace-adapter-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/adr/pc-c6-tr02-tr06-realpath-oracle.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 TR02/TR06 realpath oracle 設計 — 既存 file への test 追加と行対応表

## Context

直前 slice（PR#354）が確立した実経路 oracle fixture
`crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` は、
`DispatchTraceAdapter` + `ParamDispatch` + in-process fake plugin +
`GraphRuntime` の実経路で TR01/03/04/05/07/08 を再証拠化した。
TR02/TR06 は project 層で分類規則が証拠化済みだが、実経路の
dispatch-ledger 経由での再証拠が残件である。

本設計は残件 2 行を既存 file に test として追加する構成を定める。

## Scope

### In Scope

- `pc_c6_oracle_tr_dispatch.rs` への TR02・TR06 の 2 test 追加
- file 冒頭 doc comment の対象行記述を TR01〜TR08 全行へ更新
- fixture matrix ledger の TR02/TR06 行の実測更新（実測結果どおり）
- 本 doc-chain（spec/adr/design/plan/task/test-spec/impl-ir）

### Out of Scope

- 実 GUI / 実 plugin（VST3/CLAP）/ 実 OS / 実 RT / consumer 照合の
  再証拠 — headless/fake-plugin 証拠をこれらの成功に繰り上げない
- TR02/TR06 以外の fixture matrix ledger 行の変更
- `trace_c3_05.rs` 等の adapter 単体契約 test の変更
- 実装コード（`src/`）・manifest・`Cargo.lock` の変更
- 新規 test file の作成・rig/oracle 基盤の複製

## Components and Boundaries

- `LedgerRig` / `host_plugin` / `ledger_rig` / `fabricated_adapter`:
  既存 fixture。実経路の証拠生成はこれら経由のみ。
- 独立 oracle `oracle_*` / `assert_trace_agrees`: 期待値を fixture
  宣言から独立導出し field 単位で照合する。実装メソッド
  （`ControlTrace::freshness` 等）を期待値導出に呼ばない。
- `ParamDispatch::dispatch` → `GraphRuntime` → node report channel →
  `DispatchTraceAdapter::observe_cycle`: 実経路そのもの。test は
  この経路が生成した `ControlTrace` に対してのみ分類を検証する。
- 行対応表: TR02 → `tr02_context_mismatch_dimensions_real_ledger`、
  TR06 → `tr06_silence_vs_unobserved_real_ledger`。file 内の他 6 test
  は既存のまま変更しない。

## Data and Control Flow

TR02:
1. `ledger_rig` で rig を構成し、target 701 を bind。
2. 実 plan を `ParamDispatch::dispatch` で dispatch、`GraphRuntime`
   を駆動し report を drain、`observe_cycle` で span を観測。
3. plugin-reported evidence と trusted carried probe を記録し、
   `ControlTrace` の key（`ProjectRevision`・graph generation・
   plan generation 等）が実経路由来であることを assert。
4. 7 次元それぞれだけを 1 つ変えた context を独立 oracle と実装の
   両方に通し、同じ `StaleReason::<...>` を返すことを field 照合。
5. 全次元変更 context が `StaleReason::ProjectRevision` に解決する
   こと（実装の documented check order）と `is_coherent() == true`
   を検証。

TR06:
1. sibling target 801/802 を同一実 plan で dispatch。
2. 801 に `MeasuredValue::Silence` + carried timestamp を記録し、
   stored `ProbeObservation::Measuring` が `Silence`・timestamp・
   provenance を field 単位で保持することを assert。
3. observed/combined freshness が `Current` であること、
   未 touch の sibling が `Unavailable { NoEvidence }` であることを
   assert。
4. sibling を `Missing` / `Disabled` / `Lost { count }` で順に
   更新し、それぞれ `Unavailable { ProbeMissing }` /
   `Unavailable { ProbeDisabled }` / `DataLoss { count }` に
   分類されることを assert。
5. sibling の状態遷移が silence trace を変えないことを再 assert。

## Non-Functional Constraints

- headless/fake-plugin 経路の証拠であることを doc・impl record で
  明示し、実 GUI/plugin/OS/RT/consumer の成功に繰り上げない。
- `#[allow]` は既存 test file の長大 scenario test と同じ用途
  （`too_many_lines` のみ）に限定し、lint 隠蔽に使わない。
- 時間演算は checked 演算のみ（本 slice は test のみで新規時間演算
  を追加しないが、既存 rig の `checked_*` 規約を維持）。

## Risks and Trade-offs

- 実経路が生成しない slot variant（`observed` の `Expired`/
  `Dropped`/`Unsupported` 等）を要求しない — それらの写像は
  `trace_c3_05.rs`・project 層 oracle の責務。行の証拠範囲を
  実経路証拠の境界に正直に合わせる。
- TR02 の composite mismatch は実装の check order 依存 — documented
  order（`ProjectRevision` 先）を spec/test-spec に明記し、実装変更
  時に oracle 側も追随する構造を維持。
- file が 8 test・大規模化 — 既存の `trNN_*` 命名で行識別可能。
  分割は行数が更に増えた時点で再検討。

## Deferred Design Concerns

- 実 GUI/plugin/OS/RT/consumer 照合の再証拠 | 本 slice は headless
  fake-plugin 経路のみ | Issue #114/#101/#102・Ruviform gate 群の
  実機検証タスク着手時 | headless Pass を実機成功と誤読するリスク
- fixture matrix ledger の他行更新 | TR02/TR06 以外は本 slice 範囲外
  | 該当行の残件 slice 起票時 | ledger の陳腐化
- `observed` slot の `Expired`/`Dropped`/`Unsupported` 写像の
  実経路再証拠 | 実経路がこれらを生成しない | adapter が実経路で
  生成し得る状態が増えた時 | 写像証拠の欠落

## Verification Notes

- [ ] `cargo test -p synvelis-player --test pc_c6_oracle_tr_dispatch`
  で 8 test 全 pass。
- [ ] `mise run check` / `mise run lint` で新規 test が warning
  なく通る。
- [ ] `docs/*/README.md` index を `doc_maintenance.js index apply`
  で再生成し `index plan` が in-sync を報告する。
