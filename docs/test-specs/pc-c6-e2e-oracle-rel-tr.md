---
id: "TSPEC-YEy9TAHyUte5MD6kpyZJFI"
type: "test-spec"
status: "approved"
title: "PC-C6 oracle E2E — REL01〜07 / TR01〜08 独立照合 fixture"
created: "2026-11-09"
updated: "2026-11-09"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c2-e2e-control-relation-index.md"
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
  defers: []
  deferred-by: []
---

# PC-C6 oracle 検証仕様

`crates/synvelis-project/tests/pc_c6_oracle_rel.rs` と
`pc_c6_oracle_tr.rs` が担保する契約。全て headless synthetic fixture —
実 plugin/OS/RT/GUI 観測は対象外。期待値は oracle が fixture 宣言から
独立に算出し、契約本体（`RelationIndex`/`ControlTrace`）の観測と照合
する。oracle と実装の差異が出た場合は、実装を oracle に合わせて変更
せず、契約解釈を spec/design/test-spec に立ち返って確認する。

## Feature: REL01〜07 — Relation Index 独立照合

- Rule(REL01): `Incoming`/`Outgoing`/`Both` の各 direction で、
  anchor からの有界 BFS 結果（coverage・revision・relations 全件・
  順序込み）が oracle の独立導出と一致する。
  - Example: 混合 fixture（lane+LFO/Macro/StepSeq+depth driver）の
    anchor 群 × 3 direction を全件照合。
- Rule(REL02): LFO/Macro/StepSeq の `ModulationSource` と
  `depth_source` を持つ assignment の `DepthControlledBy` 辺が
  canonical direction で oracle と一致して返る。
- Rule(REL03): 同一 source→target の複数 assignment は別辺
  （`assignment_id` 別）として区別され、disabled assignment 由来辺は
  既定 query で結果・traversal 双方に現れず、`include_disabled` で
  `Disabled` state として現れる。
- Rule(REL04): enabled 辺のみの cycle 参加辺は `Unresolved`、
  disabled 辺を含む cycle は cycle とみなさない、dangling 参照辺は
  `Unresolved`。`Unsupported` は現行 index が生成しない予約語彙で
  あり、いかなる結果にも現れない。
- Rule(REL05): `max_depth`/`max_edges` が有界化し、超過は
  `Truncated` + 決定的 prefix（同一 query の反復で完全一致）。
  `max_edges = 0` でも到達辺があれば `Truncated`。
- Rule(REL06): `expected_revision` 不一致は `Stale`（辺なし・
  `project_revision` は index 世代）、未知 anchor は `Unavailable`、
  anchor 実在・関係無しは `Complete`+空。assignment 削除/target
  消失は再構築 index に反映され、旧世代 query は `Stale`。
- Rule(REL07): 数万 node/edge 級 fixture で `RelationIndex::build` と
  bounded query を `Instant` 実測し、結果が oracle と一致・有界で
  あること。実測値は test output と impl record に記録し、絶対
  閾値は Baseline 未確定のため保守的 sanity 上限のみとする。

## Feature: TR01〜08 — ControlTrace 独立照合

- Rule(TR01): 7 stage slot は独立。`Present` 以外は
  `Missing`/`Unsupported`/`Dropped`/`Expired` を保持し他 stage から
  合成しない。`ScheduledReceipt::new`/`AppliedStageReceipt::new` は
  status が一致する receipt のみ `Some`（fail-closed）。
- Rule(TR02): `project_revision`/`control_plan_generation`/
  `graph_generation`/`transport_generation`/`clock_epoch`/
  `loop_occurrence`/`evaluation_position` の各不一致が対応する
  `StaleReason` で `Stale` になり `Current` にならない。
- Rule(TR03): seek/pause/stop 相当（`transport_generation` bump）・
  loop wrap（`loop_occurrence` bump）・restart（`clock_epoch` bump）・
  position 移動の各世代差で、旧 key の trace は `Stale` に分類され、
  古い Receipt/証拠が現行真値として `Current` にならない。
- Rule(TR04): `ProbeObservation::Lost{count}`→`DataLoss{count}`、
  `Missing`→`Unavailable{ProbeMissing}`、`Disabled`→
  `Unavailable{ProbeDisabled}`、`plugin_reported` 欠落→
  `Unavailable{NoEvidence}` 系、dispatch `Unavailable`/`Reported`
  終端は generation を持たず coherence を壊さない。2 channel は独立
  に分類され `freshness()` は best を返す。
- Rule(TR05): stamp `provenance == Estimated`→`Estimated`（Current
  にしない）、`now > stamp.time + max_age`→`Stale{Age}`、
  `now < stamp.time`→`Unavailable{ClockUntrusted}`。
- Rule(TR06): `MeasuredValue::Silence` は `Measuring` の実測値であり
  `Current` を返し得る — probe 欠落・disabled（Unobserved）とは
  別状態。
- Rule(TR07): 同一 `ProbeObservation` 証拠を持つ 2 つの trace
  （別 `parameter_target_id`/key）が互いの freshness/coherence を
  汚染せず独立に分類される。実 2 Inspector/2 window の二重起動防止は
  workspace/UI 側（#102/#114）の責務で本 fixture 対象外。
- Rule(TR08): `ControlTrace` は履歴を持たない固定サイズ投影であり、
  反復 `freshness`/`is_coherent` 評価が状態を蓄積・変異させない。
  bounded retention・RT alloc/block の実計測は TRACE-C3-05
  （synvelis-player slice）へ defer — 本 fixture では NotRun として
  記録する。

## Guarantees

- 全判定は決定論的・read-only・非 RT。`Project`/project state を
  変更しない。
- 欠落・失敗・stale・lost・disabled を成功・Live・観測済みと誤認
  しない。
- headless fixture の Pass を実 VST3/GPUI/OS の成功に繰り上げない。

## Non-goals

- PC-C1-R01〜R08・PC-C4-PL01〜PL08・PC-C5-UI01〜UI07・
  PC-C6-COMP01〜COMP05（Issue の残件）。
- synvelis-player dispatch ledger 実経路・TRACE-C3-05 実計測。
- 実 GPUI/実 plugin/実 OS 照合（#114 gate）・Ruviform #219/#294
  consumer 照合（別 gate）。

## Risk

- oracle 実装自体の誤り — spec/design の規則を一次情報とし、差異時は
  契約解釈を再確認する。
- 巨大 fixture の test 時間 — 実測で監視し規模を調整する。
