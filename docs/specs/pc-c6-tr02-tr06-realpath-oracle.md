---
id: "SPEC-034WxkcNtl9FIF13m9IaBJ"
type: "spec"
status: "approved"
title: "PC-C6 TR02/TR06 dispatch ledger 実経路 oracle 再証拠化"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/100"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
---

# PC-C6 TR02/TR06 dispatch ledger 実経路 oracle 再証拠化

## What Should Be Built

Issue #103（PC-C6 Control Oracle）の固定 Test Matrix のうち、先行 2 slice
が証拠化しながら実経路を残件とした TR02・TR06 の 2 行を、
`synvelis-player` の**実経路結合 test** として再証拠化する。

- 先行 slice `pc_c6_oracle_tr.rs`（`docs/specs/pc-c6-oracle-rel-tr.md`）
  は project 層 synthetic fixture で TR01〜TR08 の分類規則を証拠化し、
  dispatch-ledger 実経路を `NotRun` と記録した。
- 直前 slice `pc_c6_oracle_tr_dispatch.rs`
  （`docs/specs/pc-c6-tr-ledger-oracle.md`、PR #354）は TR01/03/04/05/
  07/08 を実経路で再証拠化し、TR02・TR06 を「project 層で証拠化済み、
  必要なら別 slice」として Deferred に記録した。

本 slice はその残件を閉じる。`crates/synvelis-player/tests/
pc_c6_oracle_tr_dispatch.rs` に **TR02・TR06 の 2 test を追加**し
（既存 rig・fixture・独立 oracle を再利用し、新規 file は作らない）、
`DispatchTraceAdapter` + `ParamDispatch` + in-process fake plugin +
`GraphRuntime` の実経路を駆動して、TR02・TR06 の oracle 分類規則が
実 ledger 観測に対しても成立することを照合する。

- **TR02**: `project_revision`/`control_plan_generation`/
  `graph_generation`/`transport_generation`/`clock_epoch`/
  `loop_occurrence`/`evaluation_position` の 7 次元それぞれの不一致が
  対応する `StaleReason` に分類され `Current` にならないこと、および
  全次元同時不一致が check order 先頭の `ProjectRevision` に解決
  されること — 全て実経路生成の `ControlTrace` 上で検証する。
- **TR06**: `MeasuredValue::Silence` が `Measuring` の実測値として
  `Current` を返し得ること、probe 欠落（slot `Missing`・
  `ProbeObservation::Missing`/`Disabled`/`Lost`）がそれとは別の
  正直な状態として分類されること — 実経路 trace 上で検証する。

期待値は fixture 宣言から oracle が独立に導出し、field 単位で照合する
（既存 `pc_c6_oracle_*` の流儀）。adapter 単体契約（drop 写像・expiry
window・receipt 相関の内部・slot variant 写像）は `trace_c3_05.rs`
（#100 側）と project 層 oracle が網羅済みであり、本 slice は
「TR02/TR06 分類規則の実経路再証拠化」に絞る。

## Why Now

直前 slice の impl record・task doc・test-spec が TR02/TR06 を
「project 層で証拠化済み。実経路での再証拠が必要と判断された場合は
別 slice」として明示的に Deferred に記録しており、Issue #103 の
PC-C3-TR02/TR06 行の実経路証拠がまだ欠けている。本 slice で閉じる
ことで TR 系 8 行全てが実経路再証拠化され、#103 の残件が実機 gate
（実 GUI/plugin/OS/RT・consumer 照合）と他 Case 群のみに絞られる。

## Users and Value

- PC-C6 #103: TR02/TR06 Case の Expected/Observed を実経路で独立算出
  して記録する oracle 層。project 層 fixture の Pass を実経路 Pass に
  置き換えるのではなく、実経路での再証拠として積み上げる。
- Gate C（双方向 Relation/Trace 判定）・#114 実 GUI Scenario が参照する
  headless 証拠の一部。

## Scope

### In Scope

- `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` への
  TR02・TR06 test 2 本の追加と file 冒頭 doc comment の対象行更新。
  既存 test・`src/` 配下・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  CI 設定は一切変更しない。
- 既存の実経路 rig（`host_plugin`/`ledger_rig`/`fabricated_adapter`）・
  fixture・独立 oracle（`oracle_*`/`assert_trace_agrees`）の再利用。
- `docs/` 文書チェーン（spec/ADR/design/plan/test-spec/task/impl
  record）新規追加。生成 index（`docs/*/README.md`）は doc-maintenance
  の `index apply` で再生成する。

### Out of Scope

- `crates/*/src/` の変更（契約本体・adapter 本体は oracle が照合する
  対象であり変更禁止）。
- `crates/synvelis-player/tests/` の既存 test や他 crate の変更、
  manifest・mise・CI 設定の変更。
- TR01/03/04/05/07/08 — 直前 slice（PR #354）で実経路再証拠化済み。
  本 slice はそれらを変更・再主張しない。
- PC-C1-R01〜R08・PC-C4-PL01〜PL08・PC-C5-UI01〜UI07・
  PC-C6-COMP01〜COMP05 — Issue の残件として記録のみ。
- 実 GUI/実 VST3/実 CLAP/実 OS/実 RT Pass・consumer 照合 — **NotRun**
  （#114 gate・Ruviform consumer gate）。
- GUI-PC-01〜04 scenario 対応表 — #114・sv112 着地後の別作業。

## Requirements

### PC-C6-R1: 実経路駆動

- trace は全て実経路で生成する: `ParamDispatch::dispatch` が
  generation-tagged `TransportPlan` を stage → node queue 経由で
  `GraphRuntime` が `Play`/`install_plan` を適用 → scheduler receipt
  が node report channel に投稿 → `DispatchTraceAdapter::observe_cycle`
  が report を drain して ledger 観測と receipt を `ControlTrace` に
  投影する。手組みの `ControlTrace`/ledger 観測を使わない。
- fake plugin は in-process の no-op processor であり、DSP 出力・
  実 plugin semantics は対象外。headless/fake の結果を実 GUI/実
  VST3/実 OS/実 RT Pass に繰り上げない（Issue L96・Evidence L85）。

### PC-C6-R2: 独立 Oracle 判定

- 期待値は fixture 宣言から oracle が独立に導出する。`ControlTrace`
  の `freshness`/`plugin_reported_freshness`/`observed_freshness`/
  `is_coherent` を呼んで期待値を得ない — 分類規則（context mismatch
  check order・stamp freshness・channel rank・coherence）は test 側に
  再実装する（`pc_c6_oracle_tr.rs`・`pc_c6_oracle_tr_dispatch.rs`
  と同じ規則）。
- field 単位照合: trace key の context 次元値（`project_revision`/
  `graph_generation`/`control_plan_generation`）と `observed` slot の
  `ProbeObservation::Measuring` variant・`value`（`MeasuredValue`）・
  stamp `time`/`provenance` を fixture 宣言値・実経路生成値と比較する。
- adapter 単体契約・`ControlTrace` slot variant 写像は先行 test が
  網羅済み — 本 slice は TR02/TR06 分類規則の再証拠化であり、
  adapter test・project 層 oracle の再主張にしない。

### PC-C6-R3: TR02/TR06 行網羅（実経路）

- TR02: live evidence を持つ実経路 trace に対し、7 次元それぞれを
  1 次元ずつ動かした context が対応 `StaleReason` の `Stale` に
  分類され `Current` にならないこと、全次元同時不一致が
  `StaleReason::ProjectRevision`（check order 先頭）に解決される
  こと。coherence は key 内部整合であり context に依らず true の
  ままであること。
- TR06: `record_probe` に `MeasuredValue::Silence` を記録した実経路
  trace の `observed` slot が verbatim `Silence` を保持し、
  `observed_freshness`/`freshness` が `Current` になること。
  同一 rig の sibling trace で slot `Missing`→`NoEvidence`、
  `ProbeObservation::Missing`→`ProbeMissing`、`Disabled`→
  `ProbeDisabled`、`Lost{count}`→`DataLoss{count}` が互いに異なる
  正直な答として分類され、いずれも `Current` にならず、測定値を
  合成しないこと。

### PC-C6-R4: 証拠記録

- 固定 Case ごとに Synvelis commit・Ruviform SHA（`Cargo.lock`
  resolved rev）・OS/Backend/Fixture・Expected/Observed・
  Pass/Fail/NotRun・CI/PR URL を impl record / PR に記録する。
  本 slice の全 Case は headless + in-process fake plugin であり、
  その旨を明記する。

## Acceptance Criteria

- [ ] `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` に
  TR02・TR06 の 2 test が追加され、file 内全 8 test が全 pass。
- [ ] 期待値が fixture 宣言から独立導出され、`oracle_*` 関数が
  `ControlTrace` メソッドを呼ばない。
- [ ] `src/`・manifest・mise・CI・他 crate・fixture matrix ledger の
  TR02/TR06 以外の行に変更なし。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `check-app-native` / `lint-app-native` / `test-msrv` / `build` が
  全て exit 0。
- [ ] doc chain 完走（spec/ADR/design/plan/test-spec/task/impl
  record 新規）、TR02/TR06 の fixture matrix ledger 行が実測結果
  どおりに記録され、残件が NotRun/Deferred として記録され、
  headless 結果を実機成功に繰り上げていない。

## Deferred Design Concerns

- PC-C6-COMP01〜COMP05（Gate A/B/C 統合判定）: 全行統合後の gate。
- GUI-PC-01〜04 scenario 対応表・実 GUI/実 plugin/実 OS/実 RT
  Pass・consumer 照合: #114 gate・別 dispatch（sv112 等）。
- Ruviform #219/#294 consumer 照合・#28 Ruviform 追従: 別 gate。
- PC-C1-R01〜R08・PC-C4-PL01〜PL08・PC-C5-UI01〜UI07: Issue の残件、
  別 slice。

## Sources and Relations

- Issue #103（本 spec の直接の source）: 固定 Test Matrix の
  PC-C3-TR02/TR06 行と証拠記録要件。
- Issue #100（TRACE-C3-05）: `DispatchTraceAdapter`/`ParamDispatch`
  の契約本体と `trace_c3_05.rs` rig の先例。
- `docs/specs/pc-c6-oracle-rel-tr.md`: 先行 oracle slice — TR 行の
  project 層証拠と `NotRun` 記録の正本。
- `docs/specs/pc-c6-tr-ledger-oracle.md`: 直前 slice — TR01/03/04/
  05/07/08 の実経路再証拠と TR02/TR06 の Deferred 記録の正本。
- `docs/specs/pc-c3-05-rt-trace-adapter.md`・
  `docs/specs/pc-c3-control-trace-projection.md`: oracle が照合する
  adapter/trace 契約本体。
- `docs/adr/pc-c6-tr02-tr06-realpath-oracle.md` — 実経路結合・独立
  oracle 併用・既存 file への test 追加の決定。
