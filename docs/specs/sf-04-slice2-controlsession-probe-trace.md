---
id: "SPEC-034UumeSTLbuxt64vqqJ47"
type: "spec"
status: "approved"
title: "SF-04 slice2 — ControlSession probe observation → ControlTrace 配線"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
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
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/designs/sf-03-slice5-session-engine-connect.md"
  defers: []
  deferred-by: []
---

# SF-04 slice2 — ControlSession probe observation → ControlTrace 配線

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）の integration slice として、
slice 1 で実装した project/player 層の probe adapter を
`synvelis-workspace` の `ControlSession` に配線し、drain された probe
observation を `ControlTrace` の `observed` slot（TRACE-C3-01..04 の
slot/freshness 契約）へ記録する経路をつなぐ。具体的には:

- `synvelis-project` に session 向けの port 契約を追加する:
  probe lifecycle（request/set_mode/capture/stop_capture/remove）、
  drain（contract-level observation update のみを yield — runtime
  `ProbeId`/ruviform 型を越境させない）、binding/segment の read、
  および trace 側の `record_probe`/`trace`/`traces`。
- `synvelis-player` に port の参照実装 `ProbeTraceBridge` を追加する:
  `RoutingEngine` の `probe_*` API と `DispatchTraceAdapter` を結び、
  `ProbeEvent` → `ProbeObservation` の translation（generation 照合・
  stale record の fail-closed reject・clock 未配線時の `Estimated`
  provenance）と、target → node 別 adapter の routing を担う。
- `synvelis-workspace` の `ControlSession` に probe-trace session
  state（`ProjectSessionId`・subscription key → `ParameterTargetId`
  相関）と API を追加する: lifecycle 委譲、`probe_drain`（session
  唯一の drain owner — 全 observation update を相関 target の
  `observed` slot へ記録）、binding/segment/trace/freshness の read。
- headless fake-port E2E fixture で、probe observation → drain →
  trace slot 記録 → freshness 分類を、fail-closed（未解決 point は
  観測を捏造しない）と stale-generation reject を含めて検証する。

## Why Now

slice 1（PR #200 系）で `resolve_probe_point` の fail-closed 解決、
`ProbeSession`/`RebindReceipt`/segment cut、`RoutingEngine` の probe
passthrough が確立し、PC-C3-05（PR #209 系）で `DispatchTraceAdapter`
の `record_probe`/`observe_cycle` と `ControlTrace` projection が
揃った。sv100 PR #209 で deferred と明記された「single drain owner
の workspace 側配線」が本 slice の残件であり、#108（UI）・#109
（実測 oracle）がこの経路の上に乗るため、session 境界での所有権と
generation 照合を先に固定する。

## Users and Value

- #108 UI 実装者が、parameter target ごとの `ControlTrace`（特に
  `observed` slot と freshness 分類）を session API 経由で安定して
  取得できる。
- #109 実測/oracle 作業が、plugin report と probe observation の
  独立 channel を混同しない形で比較できる。
- composition root（将来の app 層）が、engine 所有権を分散させずに
  probe→trace 経路を session に接続できる port 契約を得る。

## Scope

### In Scope

- `crates/synvelis-project/src/probe_trace.rs`（新規 port 契約）+
  `lib.rs` export。
- `crates/synvelis-player/src/probe_trace.rs`（新規 bridge）+
  `lib.rs` export。
- `crates/synvelis-workspace` の `ControlSession` probe-trace 配線
  （state + API、additive）と `lib.rs` export。
- headless テスト: workspace fake-port E2E
  `crates/synvelis-workspace/tests/sf04_s2_probe_trace.rs`、player
  bridge テスト `crates/synvelis-player/tests/sf04_s2_probe_trace_bridge.rs`。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/impl record）。

### Out of Scope

- `crates/synvelis-app/`（SF-05・別 dispatch）と
  `crates/synvelis-recording/`（RCD-03・別 dispatch）への変更。
- UI 表示（#108）、実測・oracle gate（#109）、実 plugin/OS/RT 観測
  （#114 gate）。
- ruviform 側の probe 本体・pin 変更。
- `ControlTrace`/`RelationIndex`/adapter の公開契約の破壊的変更
  （消費・接続のみ）。
- legacy `ControlObservation`/`feed_probe` 経路の変更（別 channel の
  既存契約として維持）。

## Requirements

- Configured/Prepared/Scheduled/Applied/Dispatched/Reported/Observed
  の stage 分離を維持する。probe wiring は `observed` slot のみを
  書き、他 stage の証拠を合成・推測しない。
- missing は `Unobserved`/unavailable のまま扱い、live/Current に
  偽装しない。証拠の無い target に観測値を生成しない。
- probe drain の owner は session の `probe_drain` 一箇所に限定する
  （dispatch report drain の `observe_cycle` と同じ単一 owner 原則）。
- observation は subscription key → session 相関 → `ParameterTargetId`
  の経路でのみ trace に記録される。session が相関を持たない
  subscription の event は記録しない。
- generation 照合: record の plan generation が binding の
  `active_plan_generation` と一致しない stale record は trace に
  記録しない（fail-closed reject）。`RebindReceipt`/segment cut は
  slice 1 の経路（`RoutingEngine::publish` → `ProbeSession::on_publish`）
  をそのまま維持し、本配線が跨がない。
- `MediaTimestamp` provenance を正直に写像する: clock 未配線
  （`ProbeClockMark::is_unknown`）の record は `Estimated`（Current に
  ならない）、配線済みは `Derived`。
- RT 経路（`ProbeDeck`）には触れない。drain/translation/bookkeeping は
  すべて非 RT orchestration 側。

## Acceptance Criteria

- [ ] probe adapter → `ControlSession` → `ControlTrace` の配線があり、
  session lifecycle（request/mode/capture/stop/remove）・drain 単一
  owner・generation 照合が session API 経由で動作する。
- [ ] `RebindReceipt`/segment cut が adapter→session 経路を通じて
  維持される（publish 時の rebind・capture segment 切断が既存
  経路どおり動き、配線がそれを壊さない）。
- [ ] headless fake-port fixture で observation → drain → trace slot
  記録 → freshness 分類の E2E が検証され、fail-closed（未解決・
  未相関の observation は記録されない）と stale-generation reject
  が確認できる。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt`
  （存在する範囲）が green。

## Deferred Design Concerns

- #108 UI 接続（SF-05 側の表示 — 別 dispatch/後続）| 本 slice は
  headless session/trace 経路に限定 | #108 着手時 | UI が無いと
  session API の ergonomics は未検証のまま残る。
- #109 実測（SF-06 cross gate）| 実 plugin/OS/RT 観測は #114 gate |
  gate 通過後 | headless fake のみでは実計測値の正しさは証明できない。
- app composition root での port 実装（`SessionRoutingEngine` 共有・
  node 別 `DispatchTraceAdapter` の生成/登録）| `synvelis-app` は別
  dispatch の編集範囲 | SF-05 integration slice | port 契約は本 slice
  で固定するが、所有権の具体形は app 側で決める。
- probe capture PCM の trace/永続化接続 | 本 slice の `observed` slot
  は `MeasuredValue`（Signal/Silence）のみを運ぶ | 計測/録音統合
  slice | PCM chunk の扱いは segment ledger が既に担う。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/107>（SF-PROBE-01..09）
- 前提: SF-04 slice1（semantic contract・`ProbeSession`・engine probe
  passthrough）、PC-C3（`ControlTrace` projection）、PC-C3-05
  （`DispatchTraceAdapter`・`record_probe`/`observe_cycle`）、
  SF-03 slice5（`ProjectSession`/`EnginePort`/`ControlSession` session
  境界）。
- Ruviform: `ruviform_audio::typed::probe`（`ProbeEvent`/
  `MetricRecord`/`StatusRecord`/`RecordMeta`/`ProbeCounts`）、
  `ruviform_media_core`（`MediaTimestamp`/`TimestampProvenance`）。
- 本 spec は Issue #107 のうち slice 2（workspace ControlSession 配線）
  のみを対象とし、Issue は Open 継続が前提。
