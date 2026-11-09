---
id: "SPEC-IY7kQwMIWqb4sFVpvsHnQy"
type: "spec"
status: "approved"
title: "PC-C6 headless oracle — REL01〜07 / TR01〜08 fixture（RelationIndex・ControlTrace 独立照合）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/6"
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
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c2-e2e-control-relation-index.md"
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/ruviform/issues/219"
    - "https://github.com/scarecrow173/ruviform/issues/294"
  deferred-by: []
---

# PC-C6 headless oracle — REL01〜07 / TR01〜08 fixture

## What Should Be Built

Issue #103（PC-C6 Control Oracle）の固定 Test Matrix のうち、前提実装が
main に揃っている 2 行を headless oracle fixture として実装する。
`synvelis-project` に **新規 test ファイルのみ**を追加し、synthetic
fixture から期待値を oracle が独立に算出して、main に merge 済みの契約本体
（`src/relation.rs` の `RelationIndex`/`RelationQuery`/`RelationReply`/
`RelationState`/`RelationCoverage`、`src/trace.rs` の `ControlTrace`/
`ControlTraceKey`/`TraceSlot`/`FreshnessContext`/`ObservationFreshness`/
`PlanActivationState`）の観測結果と照合する。

- **PC-C2-REL01〜REL07**: Incoming/Outgoing/Both、LFO/Macro/StepSeq/Depth、
  disabled/unsupported/複数 Assignment/循環/巨大 Graph・Index stale の
  関係 ID/検索結果/有界性。REL-C2-04 で延期された**大規模 graph
  （数万 node 級）の性能実測を本 slice で実施・記録**する
  （`docs/test-specs/pc-c2-e2e-control-relation-index.md` の Deferred
  記載どおり）。
- **PC-C3-TR01〜TR08**: Configured/Prepared/Scheduled/Applied/Dispatched/
  Reported/Observed の区別、Clock/Graph/Epoch 差・seek/loop/restart、
  Plugin Lost/Unknown・Measured Silence と Unobserved の別状態化、
  古い Receipt を現行真値にしないこと。**project 層の projection 契約と
  synthetic fixture に限定** — `synvelis-player` の dispatch ledger 実経路
  （TRACE-C3-05、別 dispatch 実装中）には触れない。

## Why Now

Issue #103 は PC-C1〜C5 の Acceptance 証拠を統合する横断 Gate であり、
REL/TR 行の headless oracle が揃わないと Gate C 双方向 Relation と
Trace の独立判定が成立しない。契約本体（#204 relation・#206 trace）は
main に merge 済みで、残るのは「別 Oracle 判定」の fixture のみ。

## Users and Value

- PC-C6 #103: 固定 Case の Expected/Observed を独立算出して記録する
  oracle 層。既存受入 test（`pc_c2_relation.rs`/`trace_c3.rs`）の
  再主張ではなく、fixture 宣言からの独立導出で照合する。
- 後続の COMP01〜05 Gate 判定・#114 実 GUI Scenario が参照する
  headless 証拠の一部。

## Scope

### In Scope

- `crates/synvelis-project/tests/` への新規 test ファイル追加のみ
  （例: `pc_c6_oracle_rel.rs` + `pc_c6_oracle_tr.rs`）。既存 test・
  `src/` 配下は一切変更しない。
- oracle は fixture が宣言した control model / trace slot 構成から
  期待値（辺集合・状態・coverage・freshness 分類）を test 側の
  独立実装で算出し、`RelationIndex`/`ControlTrace` の観測と照合する。
- 大規模 graph fixture（数万 node/edge 級）での index 構築・query の
  `Instant` 実測を test output に記録し、impl record に転記する。
- `docs/` 文書チェーン（spec/ADR/design/plan/test-spec/task/impl
  record）と生成 index の再生成。

### Out of Scope

- `crates/synvelis-project/src/` の変更（契約本体は oracle が照合する
  対象であり変更禁止）。
- 他 crate 全般（synvelis-workspace/-app/-recording/-player/-gateway/
  -render/-library/-theme）— 別 dispatch が稼働中。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI 設定の変更。
- PC-C1-R01〜R08・PC-C4-PL01〜PL08・PC-C5-UI01〜UI07・
  PC-C6-COMP01〜COMP05 — Issue の残件として記録のみ。
- synvelis-player dispatch ledger 実経路（TRACE-C3-05 を含む）。
- 実 GPUI/実 plugin/実 OS/実 VST3 との照合 — **NotRun**（#114 gate）。
- #28 Ruviform 追従・RF#219/#294 consumer 照合 — **NotRun**（別 gate）。

## Requirements

### PC-C6-R1: 独立 Oracle 判定

- 期待値は oracle が fixture 宣言から独立に算出する。既存受入 test の
  期待値をコピー・再主張しない。`RelationIndex` の観測（coverage・
  project_revision・relations 列）と oracle の期待 reply を照合する。
- oracle は edge 導出・状態分類（Enabled/Disabled/Unresolved）・
  有界 BFS・freshness 分類を test 側に独立実装し、契約本体の内部実装を
  呼ばない。
- Fake/Headless の結果を実 VST3/GPUI/OS の成功に繰り上げない。未実行
  Case・未搭載機能は NotRun/Unsupported として正直に記録する。

### PC-C6-R2: REL01〜07 網羅

- REL01: Incoming/Outgoing/Both の方向別到達を oracle 照合。
- REL02: LFO/Macro/StepSeq source と Depth Driver を含む fixture。
- REL03: 同一 source→target の複数 Assignment 区別・disabled 意味論
  （既定非寄与・`include_disabled` での Disabled 表示）。
- REL04: enabled cycle → Unresolved、disabled cycle 非 cycle、
  dangling 参照 → Unresolved、Unsupported は予約語彙で生成されない
  ことの確認。
- REL05: `max_depth`/`max_edges` の有界性・Truncated の決定的 prefix。
- REL06: Index stale（revision 不一致 → Stale）・未知 anchor
  （Unavailable）・revision delta/削除反映。
- REL07: 数万 node 級の巨大 graph で build/query を実測し、結果の
  有界性と oracle 一致を確認。閾値は実 Baseline 未確定のため、測定値を
  記録しつつ pathological な非終了/異常肥大のみを fail とする
  保守的 assertion に留める（Issue の「Perf thresholds は実 Baselines
  を測定してから確定」に従う）。

### PC-C6-R3: TR01〜08 網羅（project 層 projection 限定）

- TR01: Configured/Prepared/Scheduled/Applied/Dispatched/Reported/
  Observed の 7 stage が独立 slot であり、欠落 stage が合成されない
  こと、receipt slot の status fail-closed を oracle 照合。
- TR02: Clock/Graph/Epoch 差を含む key↔ctx 全次元の不一致が対応する
  `StaleReason` に分類されること。
- TR03: seek/loop/restart/pause 相当の transport_generation/
  loop_occurrence/clock_epoch/evaluation_position 変化で旧証拠が
  現行真値にならないこと。
- TR04: Plugin Lost/Unknown（`ProbeObservation::Lost`/`Disabled`/
  `Missing`、`plugin_reported` 欠落、dispatch `Unavailable`/`Reported`
  終端）を別状態として分類。
- TR05: lost/stale/estimated stamp — `DataLoss`/`Stale{Age}`/
  `Estimated`/`Unavailable{ClockUntrusted}` の分類。
- TR06: Measured Silence（`MeasuredValue::Silence`）と Unobserved
  （probe 欠落・disabled）を別状態として分類。
- TR07: 2 Inspector 相当の共有 probe 証拠を持つ複数 trace が互いの
  状態を汚染しないこと（project 層では key 別独立評価として検証。
  実 2 window/実 Inspector は #114 gate で NotRun）。
- TR08: bounded retention — project 層 projection は履歴を持たない
  固定サイズ値であり、反復評価が状態を蓄積しないこと。実 retention/
  no-RT alloc 計測は TRACE-C3-05（player slice）へ defer として記録。

### PC-C6-R4: 証拠記録

- 固定 Case ごとに Synvelis commit・Ruviform SHA（`Cargo.lock`
  resolved）・OS/Backend/Fixture・Expected/Observed・Pass/Fail/NotRun・
  CI/PR URL を impl record / PR に記録する。本 slice の全 Case は
  headless fixture であり、その旨を明記する。

## Acceptance Criteria

- [ ] PC-C2-REL01〜REL07 が新規 oracle fixture test で独立算出の
  期待値と照合され、全 pass。
- [ ] PC-C3-TR01〜TR08 が同様に照合され、全 pass。
- [ ] 大規模 graph（数万 node 級）の build/query 実測値が test output
  と impl record に記録される。
- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` が全て exit 0。
- [ ] doc chain 完走、docs/impl に impl record、生成 index 再生成済み。
- [ ] 未実行・対象外 Case（PC-C1/C4/C5/COMP・実機照合・player
  dispatch ledger）が NotRun/Deferred として記録され、headless
  fixture を実機成功に繰り上げていない。

## Deferred Design Concerns

- PC-C1-R01〜R08（Recording write/Undo oracle）: 別 Issue で実装中・
  未 merge — 本 slice 対象外。
- PC-C4-PL01〜PL08（plugin binding oracle）: #101 未実装 — 対象外。
- PC-C5-UI01〜UI07（relation UI oracle）: 別 dispatch 実装中 — 対象外。
- PC-C6-COMP01〜COMP05（Gate A/B/C 統合判定）: 全行統合後の gate。
- synvelis-player dispatch ledger 実経路・TRACE-C3-05 bounded
  retention/RT alloc 計測: 別 dispatch `ctx_60ad8cab15d8` が
  TRACE-C3-05 を実装中 — 本 slice は project 層契約のみ。
- 実 GPUI/実 plugin/実 OS 照合: #114 gate。Ruviform #219/#294
  consumer 照合: 別 gate。

## Sources and Relations

- Issue #103（本 spec の直接の source）: 固定 Test Matrix の
  PC-C2-REL01〜07 / PC-C3-TR01〜08 行と証拠記録要件。
- Issue #6（Design Owner）: Automation / Modulation の意味論。
- PC-C2 spec（#99 Relation Index）・PC-C3 spec（#100 ControlTrace）に
  depends-on — oracle が照合する契約本体。
- `docs/test-specs/pc-c2-e2e-control-relation-index.md`: REL-C2-04 で
  延期された巨大 graph 実測の引き継ぎ先。
- `docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md` — oracle 判定方式の
  決定。
