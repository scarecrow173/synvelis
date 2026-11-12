---
id: "TSPEC-034W1WRQXbMr2y30mwzX1z"
type: "test-spec"
status: "approved"
title: "PC-C6 TR ledger oracle 検証仕様 — TR01/03/04/05/07/08 実経路照合"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/designs/pc-c6-tr-ledger-oracle-design.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c3-05-rt-trace-adapter.md"
  defers: []
  deferred-by: []
---

# PC-C6 TR ledger oracle 検証仕様

`crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` が担保する
契約。全て headless + in-process fake plugin — 実 GUI/実 plugin/OS/RT
観測は対象外。trace は `ParamDispatch` + `DispatchTraceAdapter` +
`GraphRuntime` の実経路で生成し、期待 freshness/coherence/metrics は
oracle が fixture 宣言から独立に導出して `ControlTrace` /
`TraceMetrics` の観測と field 単位で照合する。oracle と実装の差異が
出た場合は、実装を oracle に合わせて変更せず、契約解釈を
spec/design/test-spec に立ち返って確認する。

## Feature: TR01 — stage 独立性（実 ledger 経路）

- Rule(TR01-r1): `dispatch` のみ（observe 前）は `prepared` が
  staged plan generation の `Present` であり、`scheduled`/`applied`/
  `dispatched`/`plugin_reported`/`observed` の全 slot が `Missing`。
- Rule(TR01-r2): 実 `GraphRuntime` receipt + `observe_cycle` で
  `dispatched` が `Dispatched` 投影になる（`value`/`position`/
  `plan_generation`/`call_start`/`frame_offset` を fixture 宣言値と
  field 照合）が、`plugin_reported`/`observed` は `Missing` のまま —
  dispatch coverage と receipt は観測 channel を合成しない。
  freshness は `Unavailable{NoEvidence}`。
- Rule(TR01-r3): `record_configured` は `configured` slot のみを
  `Present` にし観測 channel を変えない。`record_probe(Missing)` は
  `observed` を `Present(Missing)` にして `ProbeMissing`、
  `record_probe(Lost{count})` は `DataLoss{count}` に分類され、
  いずれも他 slot を変更しない。

## Feature: TR03 — context 変更 stale（実 re-key 含む）

- Rule(TR03-r1): live evidence（Reported + Carried probe）を持つ
  trace は、`transport_generation`/`loop_occurrence`/`clock_epoch`/
  `control_plan_generation`/`project_revision` の各次元を動かした
  context で対応 `StaleReason` の `Stale` に分類され `Current`
  にならない。coherence は key 内部整合であり context に依らず true。
- Rule(TR03-r2): 実 2 回目 `dispatch` + `record_reported` + `drive` +
  `observe` で `key.control_plan_generation` が gen2 に re-key され、
  gen1 の freshness context に対し `Stale{ControlPlan}`（check order
  で最初の不一致次元）に分類される。declared context
  （project_revision/graph_generation）は re-key で不変。

## Feature: TR04 — plugin 終端 vs probe channel 状態

- Rule(TR04-r1): `record_reported` 済み entry は `dispatched`=
  `Reported` + `plugin_reported` `Present`(Estimated stamp)。
  `mark_reporting_unavailable` は pending entry を
  `Unavailable{position}` 終端にし `plugin_reported` は `Missing`。
- Rule(TR04-r2): dispatch 終端（`Unavailable`/`Reported`）は
  observation channel を含意しない — 両 channel `Missing` の
  freshness は `Unavailable{NoEvidence}`、`Reported` のみでは
  `Estimated` が最良答。
- Rule(TR04-r3): probe `Missing`→`Unavailable{ProbeMissing}`、
  `Lost{count}`→`DataLoss{count}`、`Measuring(Carried)`→`Current`
  が channel 別に独立分類され、combined `freshness` は best を返す。
  別 target の probe 証拠は漏れない。

## Feature: TR05 — plugin_reported stamp provenance

- Rule(TR05-r1): `plugin_reported` の stamp は drain-time
  `span.ctx.position()`・`TimestampProvenance::Estimated` であり、
  plugin 側 timestamp や acknowledgment を主張しない（field 照合）。
- Rule(TR05-r2): context 一致・`now == stamp.time` でも
  `Estimated` — `Current` にならない。`now` が age 予算内なら
  `Estimated`、`now > stamp.time + max_age` なら `Stale{Age}`
  （age は provenance より先に判定）。
- Rule(TR05-r3): trusted（Carried）probe channel は combined
  freshness を `Current` にできるが `plugin_reported` は
  `Estimated` のまま。Estimated probe は両 channel `Estimated`、
  未来 stamp の probe は `Unavailable{ClockUntrusted}` で
  combined は Estimated report が答える。

## Feature: TR07 — 同一 probe・2 context

- Rule(TR07-r1): 同一 `ProbeObservation` bytes を 2 target に記録
  すると `trace1.observed == trace2.observed`（slot 値は同一）だが、
  trace は別 `control_plan_generation` の key を持つ。
- Rule(TR07-r2): 各 trace は自身の key context で `Current`、
  相手の plan generation context で `Stale{ControlPlan}` と
  分類される — freshness は trace 個別の性質であり観測値の
  大域属性ではない。

## Feature: TR08 — bounded retention metrics 照合

- Rule(TR08-r1): `history_capacity:4`/`max_targets:3`/
  `plan_gen_window:2` の adapter に対し、宣言操作列
  （dispatch 5・observe 2・configured 1・probe 3）の結果として
  `events_recorded=14`・`history_lost=10`・`targets_lost=2`・
  `expired_correlations=1`・`dropped_observations=1`・
  `latency_samples=0`（fabricated 経路は receipt 無し）・
  `last_plan_generation`/`last_value` が独立導出値と一致する。
- Rule(TR08-r2): retained history は最終 4 event
  （`ConfiguredRecorded` + `ProbeRecorded`×3）で seq 単調増加、
  ring は最古から eviction する。
- Rule(TR08-r3): LRU target eviction された target の trace は
  `None`（復活しない）。生存 target の `dispatched` は
  `Dropped{PlanReplaced}`/`Dispatched` の field 照合どおりで、
  out-of-window revision は `Expired` に計上され捏造 generation
  を持たない。64 回反復評価で freshness/coherence が決定的。

## Guarantees

- 全判定は決定論的・read-only 評価。`GraphRuntime` 駆動は
  headless で実時間に依存しない。
- 欠落・失敗・stale・lost・unavailable を成功・Live・観測済みと
  誤認しない。`Estimated` は `Current` に昇格しない。
- headless + fake plugin の Pass を実 GUI/実 VST3/実 OS/実 RT の
  成功に繰り上げない。

## Non-goals

- TR02・TR06 の実経路再証拠（project 層で証拠化済み）。
- PC-C1-R01〜R08・PC-C4-PL01〜PL08・PC-C5-UI01〜UI07・
  PC-C6-COMP01〜COMP05（Issue の残件）。
- 実 GUI/plugin/OS/RT・consumer 照合（#114・Ruviform gate）。
- adapter 単体契約（drop 写像・expiry window・receipt attribution
  内部・dedup）の再検証 — `trace_c3_05.rs` の責務。

## Risk

- oracle 実装自体の誤り — spec/design の規則を一次情報とし、
  差異時は契約解釈を再確認する。
- TR08 の会計値は adapter 内部の event 生成順に依存する —
  内部変更は本 test の fail として顕在化させ、会計の再導出を
  要求する（bounded retention の性質上不可避）。
