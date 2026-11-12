---
id: "ADR-034W1VSWLIzXp3JJ3fPHSJ"
type: "adr"
status: "accepted"
title: "PC-C6 TR ledger oracle — 実経路 rig と test 内独立 oracle の併用"
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
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/adr/pc-c3-05-trace-adapter-drain-history.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# PC-C6 TR ledger oracle — 実経路 rig と test 内独立 oracle の併用

## Status

accepted

## Context

Issue #103 の固定 Test Matrix は PC-C3-TR01〜08 を検証する。先行 slice
（`docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md`）は project 層の
`ControlTrace` を手組み fixture で照合し、`synvelis-player` の
dispatch-ledger 実経路と bounded retention を `NotRun` として記録した。
TRACE-C3-05（#100）で `DispatchTraceAdapter`/`ParamDispatch` が main に
揃い、`trace_c3_05.rs` が in-process fake plugin + `GraphRuntime` の
実 receipt 経路を先例として確立した。

争点は TR 分類規則を実経路で再証拠化する際の構成である:

1. trace をどこから得るか — 手組み fixture の継続か、実
   `DispatchTraceAdapter` 経路か、あるいは両方か。
2. 期待値をどう導出するか — `ControlTrace` のメソッドに委ねるか、
   分類規則を test 側に独立再実装するか。
3. adapter 単体契約との重複をどう避けるか — `trace_c3_05.rs` が
   drop 写像・expiry window・receipt 相関を網羅済み。

## Considered Options

* Option 1(採用): 実経路 rig + test 内独立 oracle の併用。
  `ParamDispatch` + in-process fake plugin + `GraphRuntime` で実
  ledger/receipt 経路を駆動して `ControlTrace` を生成し、分類規則は
  `pc_c6_oracle_tr.rs` と同じく test 側に独立再実装して照合する。
* Option 2: `trace_c3_05.rs` に TR 行 assert を追加する — 既存 file は
  adapter 単体契約（TRACE-C3-05）の証拠であり、PC-C6 oracle 行の
  独立判定という別責務を混在させると証拠の帰属が曖昧になる。却下。
* Option 3: 実経路で生成した trace の期待値を `ControlTrace` の
  メソッド自体から導出する — 「実装の答えを実装に聞く」再主張に
  留まり、Issue の「別 Oracle 判定」要件を満たさない。却下。
* Option 4: project 層 fixture（`pc_c6_oracle_tr.rs`）のまま
  `NotRun` を維持する — dispatch-ledger 経路の TR 証拠が永続的に
  欠け、#103 の TR 行が headless で閉じない。却下。

## Decision

1. **trace は実経路で生成する。** `ParamDispatch::dispatch` → node
   queue → `GraphRuntime` の `Play`/`install_plan` 適用 → scheduler
   receipt → `DispatchTraceAdapter::observe_cycle` drain →
   `ControlTrace` 投影、の実 route を駆動する。fabricated adapter
   経路は receipt を持たないため、receipt 依存の証拠（TR01 の
   scheduled/applied・TR03 の re-key）が必要な行では `LedgerRig`
   （`trace_c3_05.rs` の receipt-correlation rig と同型）を使う。
2. **oracle は test 内独立参照実装を継続する。** 分類規則
   （context mismatch check order・stamp freshness・channel rank・
   coherence）は `pc_c6_oracle_tr.rs` と同一規則を本 file に再実装
   し、`ControlTrace` メソッドの観測結果と field 単位で照合する。
   期待値の導出に実装メソッドを呼ばない原則は先行 ADR を継承する。
3. **対象行は実経路依存の 6 行に限定する。** TR02（key↔ctx 次元
   行列）・TR06（Silence vs Unobserved）は project 層で証拠化済み
   であり、実経路での重複再主張は証拠価値を増やさない。TR08 は
   bounded retention が adapter 固有の責務であるため実経路でのみ
   意味を持ち、本 slice の中核行とする。
4. **headless/fake の境界を明示する。** in-process fake plugin と
   `GraphRuntime` が証明するのは headless 結合挙動のみであり、
   実 GUI/実 VST3/実 OS/実 RT Pass/consumer 照合は #114 gate と
   Ruviform consumer gate に委譲して `NotRun` として記録する。

## Consequences

- `crates/synvelis-player/src/`・`synvelis-project`・manifest・
  mise・CI は無変更。追加は `tests/` の新規 1 file と `docs/` の
  新規 chain のみ。
- adapter 単体契約の再主張を避けるため、assert は TR 分類規則と
  fixture 宣言値の field 照合に限定する。adapter 内部（dedup・
  window・receipt attribution）の退行は `trace_c3_05.rs` が検出する。
- TR02/TR06 は実経路未証拠のまま残る — project 層 Pass を実経路
  Pass に読み替えないよう、impl record と PR に行単位で明記する。
- `GraphRuntime` 経路は決定的 headless であり、実 scheduler の
  timing/RT 性は証明しない — `test-rt` canonical と #114 gate が
  その境界を担う。
