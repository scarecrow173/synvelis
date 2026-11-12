---
id: "DESIGN-034W1WOQzxAE3iMWUo0d49"
type: "design"
status: "approved"
title: "PC-C6 TR ledger oracle 設計 — 実経路 rig・独立 oracle・TR 行対応表"
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
    - "docs/adr/pc-c6-tr-ledger-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/adr/pc-c6-tr-ledger-oracle.md"
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
    - "docs/designs/pc-c3-05-rt-trace-adapter-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/adr/pc-c6-tr-ledger-oracle.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C6 TR ledger oracle 設計 — 実経路 rig・独立 oracle・TR 行対応表

## Context

`crates/synvelis-player` の merge 済み契約本体
（`src/trace.rs` の `DispatchTraceAdapter`/`TraceAdapterConfig`/
`TraceContext`/`TraceMetrics`/`TraceEventKind`、`src/dispatch.rs` の
`ParamDispatch`/`DispatchCycle`）を、Issue #103 の TR 系 Case
（PC-C3-TR01/03/04/05/07/08）の実経路証拠として照合する。先行 slice
`pc_c6_oracle_tr.rs` が project 層 synthetic fixture で分類規則を
証拠化し、実経路を `NotRun` と記録した残件を埋める。

編集範囲は `crates/synvelis-player/tests/` の新規 1 file と `docs/`
のみ。`src/`・他 crate・manifest・mise・CI は変更しない。

## Scope

### In Scope

- `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`（新規）:
  TR01・TR03・TR04・TR05・TR07・TR08。
- `docs/` チェーン + impl record（生成 index は Coordinator 処理の
  ため `--no-index` で生成）。

### Out of Scope

- spec の Out of Scope と同一（src/・他 crate・TR02/TR06・実機照合・
  残り PC-C6 行・GUI-PC scenario 対応表）。

## Components and Boundaries

### 実経路 rig（`LedgerRig` / `fabricated_adapter`）

```
host_plugin()                      — PluginHost + fake NoopPlugin を
                                     attach（trace_c3_05.rs と同型、
                                     descriptor/params/buses は本
                                     fixture 固有値）
├── fabricated_adapter(hosted, cfg)
│     ParamDispatch::new(attached.control, mint_node_id(), frame_base)
│     → DispatchTraceAdapter::new(dispatch, frame_base, cfg, declared_ctx)
│     — GraphRuntime 無し。receipt を持たない宣言的 span/call window
│       の observe_cycle のみ。TR08 の厳密な event/metric 会計用。
└── ledger_rig(cfg)
      Graph: Source → plugin processor(p) → Sink を compile/publish
      → GraphRuntime::new + RtRuntime
      ParamDispatch::new(attached.control, p, frame_base)
      → DispatchTraceAdapter（所有）
      drive(blocks, frames): rt.input_writer(src_port) + rt.process
        — 実 scheduler が Play/install_plan を適用し receipt を
          node report channel に投稿（実 receipt 経路）
      observe(&span): observe_cycle(span, call_start, call_frames,
        declared_ctx) — call window は span 全体を 1 call とする
```

- fixture 値は `trace_c3_05.rs`・`pc_c6_oracle_tr.rs` と意図的に別
  系統（descriptor `org.synvelis.test.pc-c6-tr-dispatch`、params
  21..=25、targets 101/201/301/302/401/501/502/601..605、
  project_id 7 / revision 5 / graph_generation 11）。
- `declared_ctx()` は caller-supplied の project-side context —
  adapter は project commit を観測できないため、全 observe で同一
  値を渡す（key の project_revision/graph_generation を固定）。
- `span()` は `TransportStatus::Playing`・`deliverable: true`・
  `position_ticks`/`occurrence`/`plan_generation` を宣言する
  TransportSpan。`event_engine_frame` の generation tag 規則により
  `plan_generation` は stage した plan の generation と一致させる。

### 独立 oracle（test 内再実装）

`pc_c6_oracle_tr.rs` と同一規則を本 file に再実装（`ControlTrace`
メソッドを呼ばない）:

```
oracle_context_stale(trace, ctx) -> Option<ObservationFreshness>
  — project_revision → control_plan → graph → transport → clock_epoch
    → loop_occurrence → evaluation_position の check order
oracle_stamp_freshness(stamp, ctx)
  — deadline 溢れ/未来 → ClockUntrusted、now>deadline → Stale{Age}、
    Estimated → Estimated、他 → Current
oracle_channel_freshness(trace, ctx, PluginReported|Observed)
  — slot 非 Present → Unavailable{reason}、Observed は
    Measuring/Lost/Missing/Disabled を写像
oracle_freshness(trace, ctx)
  — 2 channel の best（rank 最小、同 rank は Present 優先、
    さらに同点は observed 優先）
oracle_is_coherent(trace) -> bool
  — prepared/scheduled/applied/dispatched 内 generation と key の整合
assert_trace_agrees(trace, ctx)
  — freshness/plugin_reported_freshness/observed_freshness/
    is_coherent の 4 観測を oracle 導出値と field 一致で照合
```

- `oracle_rank` は `ObservationFreshness` が `#[non_exhaustive]` の
  ため wildcard を rank worst に倒す（未知 variant が live に見えない）。
- `ProbeObservation` も `#[non_exhaustive]` — 未知 variant は
  `Unavailable{NoEvidence}` に fail closed。

### TR 行対応表

| Case | rig | fixture / 照合内容 |
| --- | --- | --- |
| TR01 | ledger | dispatch→observe→receipt で `dispatched`=Dispatched 投影（value/position/plan_gen/call_start/frame_offset を宣言値と照合）しても `plugin_reported`/`observed` は `Missing` のまま。`record_configured`/`record_probe(Missing/Lost)` が各 slot のみ更新し freshness が `NoEvidence`→`ProbeMissing`→`DataLoss` と正直に遷移 |
| TR03 | ledger | Reported + Carried probe の live trace が transport/loop/clock_epoch/plan/project 各次元の moved context で対応 `StaleReason` に。実 2 回目 dispatch で `key.control_plan_generation` が gen2 に re-key され、gen1 context が `Stale{ControlPlan}` |
| TR04 | ledger | 1 plan 2 target: t2 `record_reported` → `Reported`+`plugin_reported`(Estimated)、t1 `mark_reporting_unavailable` → `Unavailable{position}`。probe `Missing`/`Lost{5}`/`Measuring(Carried)` が channel 別に独立分類、t2 の trusted probe が combined を `Current` にし t1 に漏れない |
| TR05 | ledger | `plugin_reported` stamp = drain-time `span.ctx.position()`・`Estimated` を field 照合。now==stamp で `Estimated`（Current 不可）、now+1 でも Estimated、now>deadline で `Stale{Age}`。Carried probe で combined `Current`・reported は Estimated のまま。Estimated probe で両 channel Estimated。未来 stamp で observed `ClockUntrusted`・combined は Estimated |
| TR07 | ledger | plan1→t1・plan2→t2 の別 generation trace に同一 `ProbeObservation` bytes を記録。`trace1.observed == trace2.observed` かつ各自行 context で `Current`、相手 context で `Stale{ControlPlan}` |
| TR08 | fabricated | `history_capacity:4`/`max_targets:3`/`plan_gen_window:2` で宣言操作列（5 dispatch・2 observe・configured 1・probe 3）を実行。`events_recorded=14`/`history_lost=10`/`targets_lost=2`/`expired_correlations=1`/`dropped_observations=1`/`latency_samples=0`/retained history kind 列・evicted target 不在・surviving trace の field 照合・64 回反復の決定性を独立導出値と照合 |

## Data and Control Flow

- 構築: `host_plugin()`（PluginHost lifecycle: Instantiate→Initialize→
  Activate→StartProcessing→attach）→ rig 別に fabricated/ledger。
- 駆動: `adapter.bind` → `adapter.dispatch(writes)`（`Play`+
  `install_plan` を node queue へ）→ `rig.drive`（`rt.process` で
  scheduler 適用・receipt 投稿）→ `rig.observe(span)`（report drain・
  ledger 投影・receipt attribution・slot 更新）→ `record_reported`/
  `mark_reporting_unavailable`/`record_configured`/`record_probe`。
- 照合: `adapter.trace(target)` の `ControlTrace` を読み、
  field pin（variant・値・generation・stamp）+ `assert_trace_agrees`
  （oracle 導出 freshness/coherence）+ `adapter.metrics()` の
  独立導出値照合。

## Non-Functional Constraints

- test のみ・`src/` 不変・Cargo 依存追加なし（既存 dev-deps のみ）。
- 決定性: fixture は固定値、`GraphRuntime` 駆動は決定的 headless。
  `Instant`/乱数不使用。
- lint: workspace `[lints]`（clippy `-D warnings`）適合。
  `#[allow(clippy::too_many_lines)] // sequential oracle scenario`
  は既存 test 先例（`md_b.rs`/`pc_b.rs`/`pc_c4_binding.rs`）に倣い
  長い逐次 scenario のみ。
- MSRV 1.89 / toolchain 1.99.0、edition 2024。

## Risks and Trade-offs

- **receipt 有無への依存**: `scheduler_gen` gate により、receipt が
  `last_plan` と一致しない限り `project_call` は投影しない。fabricated
  経路は receipt を持たないため常に投影可、ledger 経路は `drive`
  後の receipt が plan generation を裏付ける — この差異を TR08 で
  fabricated、他行で ledger に割り当てている。
- **oracle 自体の誤実装リスク**: 先行 slice と同じ — spec/design/
  test-spec の規則を一次情報とし、差異時は契約解釈を再確認する。
- **key 更新順序の複雑さ**: `dispatch`/`observe`/`record_*` の各
  経路が `key` の異なる次元を更新するため、評価 context は常に
  `live_ctx(trace, now)` で key から再構成し、mismatch は key を
  動かさず context 側だけを変えて作る。
- **TR08 の会計は実装詳細に敏感**: event 数・eviction 順は adapter
  の `push_event`/`target_slot`/`plan_gens` 実装に依存する。
  独立導出は「宣言操作列 → 期待 event 列」の手計算であり、
  実装の内部関数は呼ばないが、内部順序の変更には脆い — これは
  bounded retention の会計照合という行の性質上不可避であり、
  許容する（内部変更時は test が fail して会計の再導出を要求する）。

## Deferred Design Concerns

- TR02・TR06 の実経路再証拠: project 層証拠で足りると判断。
  必要なら別 slice。
- 実 scheduler timing・実 RT pass: `test-rt` canonical + #114 gate。
- GUI-PC-01〜04 scenario 対応表: #114・sv112 着地後。

## Verification Notes

- canonical: `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `lint-app-native` / `test-msrv` /
  `build`。
- test-spec `docs/test-specs/pc-c6-tr-ledger-oracle.md` の各 Feature
  が fixture で担保されること。
- TR08 の期待会計値は impl record に手計算根拠つきで記録する。
