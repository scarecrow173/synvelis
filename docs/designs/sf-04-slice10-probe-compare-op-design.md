---
id: "DESIGN-9kRtY3wQzX8cV6bNmLp2Sd"
type: "design"
status: "approved"
title: "SF-04 slice10 設計 — syn.probe.compare app op（wire 契約・read-only 判定・JSON mapper）"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-04-slice4-app-wiring-design.md"
    - "docs/designs/sf-04-slice6-probe-compare-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice10 設計 — syn.probe.compare app op

## Context

slice 6 で `ProbeTraceBridge::compare_probes(&self,
&ProbeComparisonRequest) -> AcousticObservation` が player 層の
公開面として確立済み（inherent method・port trait 変更なし）。
slice 4 が開いた `SessionRoutingEngine::with_probe_bridge`
（`&self` → `FnOnce(&mut ProbeTraceBridge) -> R` の scoped
port 供給）経由で app 層から到達できる。本 slice はその
consumer を `syn.probe.*` ops と同じ canonical op shape で
wire へ出すだけであり、比較意味論・drain 所有権・session
相関は一切変更しない。

## Scope

### In Scope

- `crates/synvelis-app/src/service.rs`: `syn.probe.compare`
  `OpInfo` + dispatch arm。
- `crates/synvelis-app/src/ui_ops.rs`: `op_probe_compare` +
  `probe_observation_json` 系 mapper helpers。
- `crates/synvelis-app/tests/sf04_probe_compare_op.rs`。
- `docs/` 文書チェーン。

### Out of Scope

- player/project/workspace/ui の変更、`ProbeTracePort` trait
  変更、`ipc.ts` OpName、SF-PROBE-08/09、SF04-RCP-01、
  外部 gate（#108/#109/実機）。

## 設計判断 1 — `&self` read-only（pump しない）

`op_probe_compare` は **`&self`** で、比較は最後の `ui_pump`
が `ControlSession::probe_drain`（session 唯一の drain
owner）経由で保持した retained `ProbeCoverage` のみを読む。
`&mut self` + `self.ui_pump()` 先行案は採用しない。

理由:

- **drain owner の一意性を維持する**。`ui_pump` 末尾の
  `drain_probe` が唯一の drain 経路（slice 4 設計）。query
  op が暗黙に drain すると「いつ比較したか」で結果が変わる
  だけでなく、read op が drain report・coverage・trace slot
  を mutate し、`syn.probe.state` との観測順序を壊す。
- **`syn.probe.state` 先例と一致**。state op も「保持証拠の
  投影」で drain しない。compare も同じ read-op 分類。
- **`op_probe_control` の pump は action 後の副作用**であり
  query には適用しない。action op は mutation の結果を
  即座に反映する必要があるが、compare は「現在保持されて
  いる証拠で何が言えるか」を答える op であり、鮮度は
  caller が `engine.run` + pump op の後に呼ぶことで制御
  できる。
- **stale 証拠は正直に劣化する**。古い/欠落 coverage は
  `incomparable`（`no-observation-data`/`coverage-empty` 等）
  または `conditional`（`partial-coverage`）に落ち、
  `aligned` に格上げされない — fail-closed は下位層の
  まま。

## 設計判断 2 — engine 未接続の fail-closed shape

`{engine_attached:false, observation:null}` を **Ok** で返す。
`SERVICE_ERROR`（`op_probe_control` 先例）は採用しない。

理由:

- `syn.probe.compare` は **query op** であり、
  `syn.probe.state`（read op → `engine_attached:false` +
  空投影で 200）と同じ分類。UI consumer が「engine 未接続」
  を error ではなく honest unavailable state として render
  できる。
- `SERVICE_ERROR` は lifecycle/action op（`syn.probe.control`）
  が「要求された mutation を実行できない」場合の契約。
  query に対して error を返すと consumer は「比較不能」と
  「エラー」を区別する追加ロジックを要する。
- 捏造防止は shape で保証される: `observation:null` は
  「観測なし」を明示し、空の `AcousticObservation` を
  捏造しない。

一方、**malformed `params.request` は engine 判定より先に
`INVALID_PARAMS`** — `op_probe_control` が action parse を
engine check より先に行う順序先例に従う。param fault は
環境状態に依存しない決定的エラーであるべき。

## 設計判断 3 — 手動 JSON mapper

`AcousticObservation` は `ClockContextRef` 等の clock id を
含む runtime evidence で serde 対象外（slice 6 設計 —
`ClockContextRef` と同じ規則）。wire 表現は app 層が明示的に
decide する:

- `before`/`after`（`SemanticProbePoint`）・`interval`
  （`ProbeFrameInterval`）は serde contract 型 → verbatim。
- `quality` → `{kind, ...}`:
  - `aligned`: `basis`・`offset_frames`・`error_budget_frames`
  - `conditional`: 上記 + `conditions[]`・`excluded_intervals`
  - `incomparable`: `reason`（kind + payload）
- `basis` → `shared-cursor`（`clock:{domain,epoch}`）/
  `engine-frame`（`epoch`）/ `unknown`。
- `conditions[]` → `latency-bounded`/`provenance-estimated`/
  `partial-coverage`/`cross-generation`/`mixed-precision`/
  `unknown`。
- `reason` → 全 variant を stable `kind` tag + payload で
  投影（`side` は `"before"`/`"after"`、`channels`/`a`/`b`
  等の payload は verbatim）。
- `gap_mask[]` → `{side, range, kind}`（`side`/`kind` は
  contract serde）。
- `provenance` → `before_clock`/`after_clock`
  （`{domain,epoch}` or null）・`before_generations`/
  `after_generations`・`runtime_instance`・`sample_rate`。
- 全ての `#[non_exhaustive]` enum の `_` arm は
  `{"kind":"unknown"}` — 将来 variant を推測せず
  fail-closed で投影する。

## Wire 契約

`syn.probe.compare` params:

```json
{ "request": {
    "before": { "entity": {"entity":"Strip","id":"strip-…"},
                "boundary": "post-fader", "port": null },
    "after":  { "entity": {"entity":"Strip","id":"strip-…"},
                "boundary": "pre-fader",  "port": null },
    "observed_range": { "start": 0, "end": 4096 },
    "metric_set": ["peak-rms"],
    "max_bytes": 1048576 } }
```

`request` は serde `ProbeComparisonRequest` verbatim
（`before`/`after` は `SemanticProbePoint`、
`observed_range` は `ProbeFrameInterval`、`metric_set` は
`ProbeMetricKind` kebab-case、`alignment_policy` は省略可で
default = fail-closed 方針）。

戻り値:

```json
{ "engine_attached": true,
  "observation": {
    "before": <SemanticProbePoint>,
    "after":  <SemanticProbePoint>,
    "quality": { "kind": "aligned",
                 "basis": {"kind":"shared-cursor",
                           "clock":{"domain":1,"epoch":1}},
                 "offset_frames": 0,
                 "error_budget_frames": 0 },
    "interval": {"start":0,"end":2048},
    "gap_mask": [],
    "provenance": { "before_clock":{"domain":1,"epoch":1},
                    "after_clock":{"domain":1,"epoch":1},
                    "before_generations":{"first":1,"last":1},
                    "after_generations":{"first":1,"last":1},
                    "runtime_instance":1, "sample_rate":48000 } } }
```

engine 未接続: `{"engine_attached":false,"observation":null}`。
`incomparable` の場合 `interval` は `null`、`gap_mask` は `[]`。

## Components and Boundaries

- `service.rs`: `OPS` テーブルに `OpInfo{name:"syn.probe.compare",
  summary:...}`、`dispatch` に `"syn.probe.compare" =>
  self.app.op_probe_compare(params)`。
- `ui_ops.rs`: `op_probe_compare(&self, params)` —
  `json_param::<ProbeComparisonRequest>(params,"request")`
  → engine check → `with_probe_bridge(compare_probes)` →
  `probe_observation_json`。
- mapper helpers（全て private fn）:
  `probe_clock_context_json`・`probe_comparison_basis_json`・
  `probe_align_condition_json`・`probe_incomparable_reason_json`・
  `probe_comparison_quality_json`・`probe_comparison_gap_json`・
  `probe_comparison_provenance_json`・`probe_observation_json`。

## Invariants

- compare は drain しない — `syn.probe.state` の drain report・
  bindings・project revision・graph_builds・master PCM を
  変えない。
- 同一 retained evidence → 同一 observation（決定的）。
- fail-closed は下位層のまま: stale/missing evidence は
  `incomparable`/`conditional` に劣化し `aligned` を偽装
  しない。
- `#[non_exhaustive]` 将来 variant は `"unknown"` —
  wire shape の前方互換を推測で壊さない。
