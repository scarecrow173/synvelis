---
id: "SPEC-034W1VPl3lj6S7A0E8mTJL"
type: "spec"
status: "approved"
title: "PC-C6 TR 系 oracle × dispatch ledger 実経路結合（TR01/03/04/05/07/08 再証拠化）"
created: "2026-11-12"
updated: "2026-11-12"
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
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c3-05-rt-trace-adapter.md"
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
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
---

# PC-C6 TR 系 oracle × dispatch ledger 実経路結合（TR01/03/04/05/07/08 再証拠化）

## What Should Be Built

Issue #103（PC-C6 Control Oracle）の固定 Test Matrix のうち、先行 slice
（`docs/specs/pc-c6-oracle-rel-tr.md`）が project 層 synthetic fixture で
証拠化し、dispatch-ledger 実経路を `NotRun` として記録した TR 行を、
`synvelis-player` の**実経路結合 test** として再証拠化する。

`crates/synvelis-player/tests/` に **新規 test ファイル 1 本のみ**を追加し、
`DispatchTraceAdapter` + `ParamDispatch` + in-process fake plugin +
`GraphRuntime` の実経路を駆動して、TR oracle 分類規則が実 ledger 観測に
対しても成立することを照合する。

- **TR01**: stage 独立性 — `prepared`/`dispatched`/receipt slot が
  `plugin_reported`/`observed` channel を合成しないこと。
- **TR03**: context 変更による `Stale{reason}` 分類 — 実 re-dispatch による
  plan generation re-key を含む。
- **TR04**: ledger `Unavailable`/`Reported` 終端と probe
  `Missing`/`Lost`/`Measuring` channel 状態の区別。
- **TR05**: `plugin_reported` stamp provenance — report channel に
  timestamp がないため adapter は drain-time transport position を
  `Estimated` で刻印し、`Current` にならない（adapter honesty
  contract）。
- **TR07**: 同一 `ProbeObservation` を 2 target に記録した場合の
  trace 別 context 評価。
- **TR08**: bounded retention — history ring・target cap・generation
  window expiry と、それを説明する metrics の照合。

期待値は fixture 宣言から oracle が独立に導出し、field 単位で照合する
（既存 `pc_c6_oracle_*` の流儀）。adapter 単体契約（drop 写像・expiry
window・receipt 相関の内部）は `trace_c3_05.rs`（#100 側）が網羅済み
であり、本 slice は「TR oracle 分類規則の実経路再証拠化」に絞る。

## Why Now

先行 slice `pc_c6_oracle_tr.rs` は「`synvelis-player` dispatch-ledger
path and bounded history retention (TRACE-C3-05) are out of scope and
recorded as `NotRun`」と明示しており、TR01〜TR08 のうち実経路依存の
行が headless 証拠として欠けている。TRACE-C3-05（#100）で
`DispatchTraceAdapter` が main に揃ったため、残っていた結合層を
埋めることで Issue #103 の TR 行の証拠連鎖が完結する。

## Users and Value

- PC-C6 #103: TR 系 Case の Expected/Observed を実経路で独立算出して
  記録する oracle 層。project 層 fixture の Pass を実経路 Pass に
  置き換えるのではなく、実経路での再証拠として積み上げる。
- Gate C（双方向 Relation/Trace 判定）・#114 実 GUI Scenario が参照する
  headless 証拠の一部。

## Scope

### In Scope

- `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`（新規）
  のみ。既存 test・`src/` 配下・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  CI 設定は一切変更しない。
- `DispatchTraceAdapter` + `ParamDispatch` + in-process fake plugin
  （`PluginHost`/`attach`）+ `GraphRuntime` の実経路 rig —
  `trace_c3_05.rs` の fabricated-adapter + 実 receipt 経路の流儀を踏襲。
- fixture 宣言（target/param/position/plan 構成）から独立導出した
  期待 freshness/coherence/metrics を、`ControlTrace` の観測と
  field 単位で照合。
- `docs/` 文書チェーン（spec/ADR/design/plan/test-spec/task/impl
  record）新規追加。生成 index（`docs/*/README.md`）は Coordinator
  が統合時に処理するため本 slice では更新しない。

### Out of Scope

- `crates/*/src/` の変更（契約本体・adapter 本体は oracle が照合する
  対象であり変更禁止）。
- 他 crate の変更、manifest・mise・CI 設定の変更。
- TR02（key↔ctx 7 次元不一致の網羅行列）・TR06（Measured Silence vs
  Unobserved）— project 層 oracle で既に証拠化済みであり、本 slice の
  実経路対象行に含めない（重複再主張を避ける）。
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
  再実装する（`pc_c6_oracle_tr.rs` と同じ規則）。
- field 単位照合: `dispatched`/`plugin_reported`/`observed` の
  variant・`value`/`position`/`plan_generation`/`call_start`/
  `frame_offset`/stamp `time`/`provenance` を fixture 宣言値と比較
  する。
- adapter 単体契約（drop reason 写像・generation window・receipt
  attribution の内部・dedup）は `trace_c3_05.rs` が網羅済み —
  本 slice は TR 分類規則の再証拠化であり、adapter test の再主張に
  しない。

### PC-C6-R3: TR 行網羅（実経路対象行）

- TR01: dispatch/receipt だけでは観測 channel が `Missing` のまま
  であること、`record_configured`/`record_probe` が各 slot のみを
  満たすこと。
- TR03: key 全次元の context mismatch が対応 `StaleReason` に分類
  されること + 実 2 回目 dispatch による `control_plan_generation`
  re-key で旧 context が `Stale{ControlPlan}` になること。
- TR04: `mark_reporting_unavailable` → `Unavailable` 終端、
  `record_reported` → `Reported` + `plugin_reported`、probe
  `Missing`/`Lost`/`Measuring` の channel 別分類が実 trace 上で
  独立であること。
- TR05: `plugin_reported` stamp が drain-time span position・
  `Estimated` provenance であり `Current` に分類されないこと、
  age 予算超過で `Stale{Age}`、未来 stamp で `ClockUntrusted`、
  trusted probe channel が combined freshness を `Current` に
  できること。
- TR07: 同一 `ProbeObservation` を 2 target に記録し、各 trace が
  自身の key context で `Current`、相手の plan context で
  `Stale{ControlPlan}` と分類されること。
- TR08: `history_capacity`/`max_targets`/`plan_gen_window` 指定の
  adapter で、宣言した操作列に対し `events_recorded`/`history_lost`/
  `targets_lost`/`expired_correlations`/`dropped_observations`/
  retained history 内容が独立導出値と一致すること。

### PC-C6-R4: 証拠記録

- 固定 Case ごとに Synvelis commit・Ruviform SHA（`Cargo.lock`
  resolved rev）・OS/Backend/Fixture・Expected/Observed・
  Pass/Fail/NotRun・CI/PR URL を impl record / PR に記録する。
  本 slice の全 Case は headless + in-process fake plugin であり、
  その旨を明記する。

## Acceptance Criteria

- [ ] `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` が
  新規追加され、TR01/TR03/TR04/TR05/TR07/TR08 の 6 test が全 pass。
- [ ] 期待値が fixture 宣言から独立導出され、`oracle_*` 関数が
  `ControlTrace` メソッドを呼ばない。
- [ ] `src/`・manifest・mise・CI・既存 test/docs index に変更なし。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `check-app-native` / `lint-app-native` / `test-msrv` / `build` が
  全て exit 0。
- [ ] doc chain 完走（spec/ADR/design/plan/test-spec/task/impl
  record 新規）、残件が NotRun/Deferred として記録され、headless
  結果を実機成功に繰り上げていない。

## Deferred Design Concerns

- TR02・TR06: project 層 oracle（`pc_c6_oracle_tr.rs`）で証拠化済み。
  実経路での再証拠が必要と判断された場合は別 slice。
- PC-C6-COMP01〜COMP05（Gate A/B/C 統合判定）: 全行統合後の gate。
- GUI-PC-01〜04 scenario 対応表・実 GUI/実 plugin/実 OS/実 RT
  Pass・consumer 照合: #114 gate・別 dispatch（sv112 等）。
- Ruviform #219/#294 consumer 照合・#28 Ruviform 追従: 別 gate。

## Sources and Relations

- Issue #103（本 spec の直接の source）: 固定 Test Matrix の
  PC-C3-TR01〜08 行と証拠記録要件。
- Issue #100（TRACE-C3-05）: `DispatchTraceAdapter`/`ParamDispatch`
  の契約本体と `trace_c3_05.rs` rig の先例。
- `docs/specs/pc-c6-oracle-rel-tr.md`: 先行 oracle slice — TR 行の
  project 層証拠と `NotRun` 記録の正本。
- `docs/specs/pc-c3-05-rt-trace-adapter.md`・
  `docs/specs/pc-c3-control-trace-projection.md`: oracle が照合する
  adapter/trace 契約本体。
- `docs/adr/pc-c6-tr-ledger-oracle.md` — 実経路結合・独立 oracle
  併用の決定。
