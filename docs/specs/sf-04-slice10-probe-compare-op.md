---
id: "SPEC-H7wQpL2mNvR4tY8xZ1cJkD"
type: "spec"
status: "approved"
title: "SF-04 slice10 — syn.probe.compare app op（ProbeComparisonRequest → AcousticObservation の wire 露出）"
created: "2026-11-12"
updated: "2026-11-12"
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
    - "docs/specs/sf-04-slice4-app-wiring.md"
    - "docs/specs/sf-04-slice6-probe-compare.md"
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
    - "docs/specs/sf-04-slice4-app-wiring.md"
    - "docs/specs/sf-04-slice6-probe-compare.md"
    - "docs/designs/sf-04-slice6-probe-compare-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice10 — syn.probe.compare app op（ProbeComparisonRequest → AcousticObservation の wire 露出）

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）の app op slice。
slice 6（PR #348）が `ProbeTraceBridge::compare_probes` として
実装した `ProbeComparisonRequest → AcousticObservation` consumer
を、`syn.probe.*` ops と同じ canonical op shape で app/UI 面へ
露出する:

- `crates/synvelis-app/src/service.rs` に `syn.probe.compare`
  の `OpInfo` エントリと dispatch arm を追加し、
  `self.app.op_probe_compare(params)` へ振る。
- `crates/synvelis-app/src/ui_ops.rs` に `op_probe_compare`
  （`&self`）と手動 JSON mapper `probe_observation_json` を
  追加。`params.request` は serde `ProbeComparisonRequest` を
  そのまま parse し、
  `engine.with_probe_bridge(|port| port.compare_probes(&request))`
  で player 層 consumer へ到達する（port trait・player 層は
  変更しない）。
- `AcousticObservation` は clock id を含む runtime evidence で
  serde 対象外のため、`quality`（`Aligned`/`Conditional`/
  `Incomparable`）・`basis`・`gap_mask`・`provenance` を
  field-by-field で JSON 化する。`before`/`after`/`interval`
  は serde contract 型なので verbatim serialize する。
  `#[non_exhaustive]` の将来 variant は `"unknown"` に
  fail-closed で写像し、推測しない。
- engine 未接続時は `op_probe_state` と同じ read-op 先例で
  `{engine_attached:false, observation:null}` を Ok で返す。
  malformed `params.request` は engine 判定より先に
  `INVALID_PARAMS`（`op_probe_control` の検証順序先例）。

比較は **read-only query** であり drain しない。比較対象は
最後の `ui_pump` が `ControlSession::probe_drain` 経由で保持
した retained coverage のみ — `syn.probe.state` と同じく
「保持証拠の投影」であり、stale/欠落証拠は正直な
`incomparable`/`conditional` に劣化し、捏造した `aligned`
にはならない。

## Why Now

slice 6 で比較 consumer 本体は完成したが、app/UI から到達
する op surface が無く、#108 UI・#109 oracle がこの結果を
消費できなかった。本 slice がその wire 露出を完成させ、
semantic probe の app-facing 契約を閉じる。

## Users and Value

- #108 UI/製品 session 実装者が、2 つの semantic probe point
  間の整列品質を `syn.probe.compare` 経由で取得できる。
- #109 実測/domain oracle が provenance・gap mask・quality
  reason を wire 上で独立検証できる。

## Scope

### In Scope

- `crates/synvelis-app/src/service.rs`: `syn.probe.compare`
  `OpInfo` + dispatch arm。
- `crates/synvelis-app/src/ui_ops.rs`: `op_probe_compare` +
  `probe_observation_json` 系 JSON mapper helpers。
- `crates/synvelis-app/tests/sf04_probe_compare_op.rs`（新規）:
  `AppService` dispatch 経由の headless fixture。
- `docs/` 文書チェーン（本 spec/design/plan/task/test-spec/
  impl record）。

### Out of Scope

- `crates/synvelis-player/**`・`crates/synvelis-project/**`・
  `crates/synvelis-workspace/**`・`ui/**` の変更（consume-only）。
- `ProbeTracePort` trait・`compare_probes` signature の変更。
- `ui/src/ipc.ts` `OpName` union・per-node adapter 登録
  （#108/SF-05 側）。
- signal payload 比較（`metric_set` は intent のみ —
  SF-PROBE-08/09）、SF04-RCP-01、実機・#109 oracle・#108 UI
  の外部 gate。

## Acceptance Criteria

- [ ] `syn.probe.compare` が `AppService::dispatch` 経由で
  登録・到達可能で、`params.request` を
  `ProbeComparisonRequest` として parse する。
- [ ] `engine.with_probe_bridge(|port| port.compare_probes(&request))`
  で player consumer に到達し、trait 変更なし。
- [ ] `Aligned`/`Conditional`/`Incomparable` の 3 quality が
  wire JSON として正しく投影される（basis/conditions/
  excluded_intervals/reason payload を含む）。
- [ ] consumer 側 fail-closed reason（`point-unresolved`/
  `probe-not-bound`/`no-observation-data`/`coverage-empty`/
  `evidence-budget-exceeded` 等）がそのまま投影され、
  `aligned` への格上げがない。
- [ ] op は read-only: drain を起こさず、`syn.probe.state` の
  drain/bindings・project revision を変えない。
- [ ] engine 未接続で `{engine_attached:false,
  observation:null}` を返し、malformed request は
  `INVALID_PARAMS`。
- [ ] `#[non_exhaustive]` 将来 variant が `"unknown"` に
  fail-closed で写像される。

## Non-Goals（本スライス対象外 — Issue 残件）

- SF-PROBE-08/09（signal payload 比較）・SF04-RCP-01・
  #108 UI・#109 oracle・実機計測 — 外部 gate として NotRun。
- probe state の persist — session のみ（slice 4 設計どおり）。
- `syn.probe.compare` の streaming/push 露出 — pull 型 query
  op のみ。

## Dependencies

- slice 4 app/session wiring（`SessionRoutingEngine::
  with_probe_bridge`・`syn.probe.control`/`syn.probe.state`・
  drain hook — docs/specs/sf-04-slice4-app-wiring.md）。
- slice 6 比較 consumer（`ProbeTraceBridge::compare_probes`・
  `probe_compare`・`ProbeCoverage` —
  docs/specs/sf-04-slice6-probe-compare.md）。
