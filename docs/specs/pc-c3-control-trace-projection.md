---
id: "SPEC-034UqtrnCaL8J9mhrd4SyZ"
type: "spec"
status: "approved"
title: "PC-C3 ControlTrace — Configured/Prepared/Applied/Reported/Observed projection slice"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/4"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/23"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
  defers:
    - "TRACE-C3-05 bounded history / no-RT alloc-block / 実計測 RT adapter（synvelis-player 後続 slice）"
    - "synvelis-workspace ControlSession 接続・single drain owner・2 Inspector 二重起動防止（後続 integration slice）"
  deferred-by: []
---

# PC-C3 ControlTrace — Configured/Prepared/Applied/Reported/Observed projection slice

## What Should Be Built

`crates/synvelis-project` に read-only の ControlTrace projection を追加する。1 つの parameter target について、Configure → Prepare → Install/Applied → Dispatch → PluginReport → Probe の各 stage の証拠を独立した optional slot として保持し、generation / plan identity / runtime instance / clock epoch / timestamp / 欠落・stale 状態を typed evidence として投影する。

- `ControlTraceKey`: `{project_id, parameter_target_id, project_revision, control_plan_generation, graph_generation, transport_generation, clock_epoch, evaluation_position}` — 証拠の帰属を一意に決める相関キー。
- `ControlTrace`: stage ごとの独立 `TraceSlot`（`configured` / `prepared` / `scheduled` / `applied` / `dispatched` / `plugin_reported` / `observed`）。欠落 stage を合成しない — `Missing` / `Unsupported` / `Dropped` / `Expired` を typed に保持する。
- `ObservationFreshness`: `Current` / `Stale{reason}` / `Estimated` / `Unavailable{reason}` / `DataLoss{count?}`。generation・epoch・transport・position・age budget の不一致は `Stale` または `Unavailable` に分類し、誤 Live を出さない。
- Probe の `missing` / `lost` / `disabled` と Measured Silence を型レベルで区別する。
- `Applied` / `Dispatched` は dispatch 経路の投影であり、plugin 内部処理や出力の実測を意味しない。plugin report と probe 観測は独立した証拠チャネル。
- 失敗した新 plan は active として投影せず、旧 active plan の継続を正しく投影する。

## Why Now

Issue #100 の rationale: `ControlEvaluation`・`ParamDispatchLedger`・`ControlObservation` は generation・時刻・clock epoch・欠落 stage・freshness を一貫した typed evidence model で提供していない。特に observation 分類は graph/plan generation・clock epoch・経過時間 stale を十分に扱っていない。PC-C2（#99）の revision-tagged projection 先例と `routing_build.rs` の generation-scoped key・`RoutingFault`・last-good semantics を土台に、project 層で完結する projection slice を先に確立する。

## Users and Value

- 後続の workspace integration slice（`ControlSession` 接続・single drain owner）と UI projection（#102）が、誤 Live なしに stage 証拠を消費できる。
- Inspector / Control Oracle（#103）が configured 値・dispatch 証拠・plugin 報告・probe 実測を混同しない。

## Scope

### In Scope

- `crates/synvelis-project/src/trace.rs` 新設: `ControlTraceKey` / `ControlTrace` / `TraceSlot` / stage evidence 型 / `ObservationFreshness` / `StaleReason` / `UnavailableReason` / probe 状態型 / plan activation 投影型。
- `crates/synvelis-project/src/lib.rs` への additive export のみ。
- `crates/synvelis-project/tests/trace_c3.rs` fixture test（TRACE-C3-01..04）。
- `docs/` 文書チェーン。

### Out of Scope

- TRACE-C3-05: bounded history・RT での alloc/block 禁止・f64/clock/latency/lost metrics の実計測 — synvelis-player RT adapter slice へ defer。
- `crates/synvelis-workspace/` の `ControlSession`/`ControlObservation`/`observation_state` 変更 — 本 slice では接続しない（Issue 編集可範囲外）。
- `crates/synvelis-player/` の `ParamDispatch`/`ParamDispatchLedger` RT adapter 変更 — 参照のみ。
- #101 Binding/FrameBase・#103 Control Oracle・#114 GPUI との統合 — NotRun。
- 実 plugin report / OS / RT 観測 — NotRun（mock 出力で実 OS 成功を主張しない）。
- 永続 schema 変更・Cargo 依存追加・他 crate の変更。

## Requirements

### TRC-C3-R1: stage 別独立証拠 slot（TRACE-C3-01）

`ControlTrace` は configured / prepared / scheduled / applied / dispatched / plugin_reported / observed の 7 slot を持ち、各 slot は `Present(evidence)` または欠落理由（`Missing` / `Unsupported` / `Dropped` / `Expired`）を typed に保持する。いずれかの stage の証拠欠落を他 stage の成功から推論しない。receipt 相関は `ControlTraceKey` の generation 群で行い、slot 内 evidence が保持する generation/epoch/position と key の整合を検査可能にする。

### TRC-C3-R2: freshness 判定（TRACE-C3-02）

`ObservationFreshness` は評価コンテキスト（現在の project_revision / plan generation / graph generation / transport generation / clock epoch / evaluation position / now / max_age）と証拠 stamp を比較し、`Current` / `Estimated` / `Stale{reason}` / `Unavailable{reason}` / `DataLoss{count}` を返す。Seek/Loop/Pause/DeviceRestart は transport_generation・clock_epoch・position の不連続として表現される。推定 stamp（`StampProvenance::Estimated`）だけで観測済みと断定しない。古い・未来・age 超過の stamp は `Stale`/`Unavailable`。

### TRC-C3-R3: probe 状態と Measured Silence の区別（TRACE-C3-03）

probe 観測 slot は `ProbeChannelState`（`Missing` / `Disabled` / `Lost{count}` / `Measuring`）を持ち、`Measuring` のみが実測値を持つ。Measured Silence（実際に無音が測定された）は `MeasuredValue` として `Measuring` と区別される。2 Inspector の二重起動防止は workspace 接続時の責務として文書に記録し、本 slice では実装しない。

### TRC-C3-R4: dispatch 意味論と失敗 plan の投影（TRACE-C3-04）

`Dispatched` evidence は「計画/スパンへの投影」（送信済み・カバー済み）を意味し、plugin 側 ACK・内部処理・出力実測を意味しない。`Applied`（scheduler/RT 適用 ACK）・`PluginReported`（plugin 可視報告）・`Observed`（probe 実測）は独立 slot。失敗 plan の投影は `PlanActivation` 型で旧 active plan の継続を表現し、失敗 plan を active として投影しない。

### TRC-C3-R5: read-only・決定論・二重正本禁止

projection 型は project state・controller・undo・evaluator・routing graph を変更しない。project state が唯一の永続正本であり、trace 型は導出・read-only。全ての比較・分類は決定論的で、RT 割当・ブロッキング・ログを伴わない（本 slice は非 RT の projection 層）。

## Acceptance Criteria

- [ ] TRACE-C3-01: Configure→Prepare→Install/Applied→Dispatch→PluginReport/Probe の receipt 相関が `ControlTraceKey` + slot 構造で表現され、欠落 stage が `Missing`/`Unsupported`/`Dropped`/`Expired` のまま Unknown 維持される（fixture test で検証）。
- [ ] TRACE-C3-02: graph/plan/clock epoch/transport/position mismatch・estimated/older/too-old stamp が `Stale`/`Unavailable`/`Estimated` に分類され、`Current`（誤 Live）にならない（fixture test で検証）。
- [ ] TRACE-C3-03: probe missing/lost/disabled と Measured Silence が型レベルで区別される（fixture test で検証）。二重起動防止は文書記録のみ。
- [ ] TRACE-C3-04: `Applied`/`Dispatched` が plugin 内部処理・出力実測と分離され、失敗 plan で旧 active plan 継続が投影される（fixture test で検証）。

## Deferred Design Concerns

- TRACE-C3-05（bounded history・実計測）は synvelis-player RT adapter slice。
- workspace `ControlSession` 接続・single drain owner・2 Inspector 二重起動防止の実配線は後続 integration slice。
- #101 Binding/FrameBase・#103 Oracle・#114 GPUI との同一 Revision/Case ID 統合は各 Issue gate。

## Sources and Relations

- Issue #100（ControlTrace）— 本 spec の source。
- PC-C2 spec（revision-tagged projection 先例）、PC-A/PC-B spec（control model / dispatch scope）。
- `docs/adr/pc-c3-control-trace-stage-evidence-freshness.md` — stage slot・freshness 境界・probe 語彙の決定。
