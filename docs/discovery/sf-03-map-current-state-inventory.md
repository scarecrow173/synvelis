---
id: "DISC-034Ux6PrPhfKsxfYRXYkPJ"
type: "discovery"
status: "resolved"
title: "SF-03 semantic→Ruviform runtime 写像 — 現状棚卸しと設計ギャップ"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/specs/pc-c6-oracle-rel-tr.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  derived-by:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/103"
  defers: []
  deferred-by: []
---

# SF-03 semantic→Ruviform runtime 写像 — 現状棚卸しと設計ギャップ

## Summary

Issue #106（SF-03 Semantic→Ruviform Typed Graph Lowering・Revision Map・
Plan Activation/Last-good Consumer）の semantic→runtime 写像について、
main に統合済みの実装 slice（SF-03 slice1〜5）と後続統合契約
（TRACE-C3・SF-04・SF-05・PC-C6）を棚卸しし、設計文書として確定すべき
不変条件・失敗写像・検証計画のギャップを特定した。結論: 実装基盤は
slice 単位で確立済みだが、consumer 横断の「同一 Revision/Generation を
指す」証拠契約と失敗不変条件の一覧は単一文書に集約されていない —
本チェーンがその設計権威となる。確度: 高（実コード・統合済み文書を
直接確認済み）。

## Exploration Goal

本 dispatch は Issue #106 のうち **semantic→Ruviform runtime 写像の
設計 slice（docs のみ）** を対象とする。実装の複雑さと crate 競合の
ため設計を先行させ、以下を確定する。

- `Configured→Prepared→Published→Active` の世代証拠分離
  （停止中/frames=0・nonRT publish success・後続 process callback の
  区別 — TRACE-C3 と整合）。
- Last-good/旧 Plan 継続・Publish success≠RT 駆動の誤表示防止・
  Pending 明示。
- Semantic→Typed Graph Lowering・Runtime SourceMap・Plan
  Generation の責務境界。
- PDC/lowering 意味論・gesture CAS・Plan publish の失敗不変条件。
- SF-ACT-01/02/03/04 の各行の設計根拠、実装順序、検証計画、
  fixture 一覧（headless 優先・実 OS/device/VST3 は NotRun）。

## Key Issues and Assumptions

- **実装済み前提**: `synvelis-project::routing_build`（契約型）・
  `synvelis-player::routing_lower`/`routing_engine`（lowering・
  activation ledger・reconcile）・`ProcessorHost` boundary・
  session 接続（`EnginePort`/`SessionRoutingEngine`）は PR
  #191/#192/#195/#197/#198 で main 統合済み。設計はこれらを
  「確定済み契約」として扱い、再設計しない。
- **統合済み前提**: TRACE-C3（`ControlTrace`/`ControlTraceKey`/
  `ObservationFreshness`、PR #206/#209/#215）、SF-04 probe
  adapter→ControlSession→ControlTrace（PR #200/#215）、SF-05
  headless signal_flow/relation_ui（PR #216）、PC-C6 oracle
  REL/TR fixture（PR #212）は main 統合済み。これらは全て
  `ProjectRevision`/`graph_generation`/`plan_generation` を
  相関キーに使い、SF-03 の証拠系と同一世代名前空間を共有する
  必要がある。
- **SF-03 slice 文書は 2026-11-08/09 付で approved** だが、
  TRACE-C3/SF-04/SF-05 統合前に書かれており、consumer 横断の
  同一 Revision 不変条件を明示していない。本チェーンは既存
  slice 文書を supersede せず、上位の統合設計として `related`/
  `depends-on` で接続する。
- **PDC の所在**: plugin delay compensation の機構は Ruviform
  typed runtime（Slot/PDC）と `synvelis-render`（offline render
  job）が所有し、semantic lowering は PDC を直接符号化しない。
  SF-ACT-02 の「PDC」は「RT/Offline で同じ意味へ lowering される」
  ことを要求する — lowered graph の topology が同一であることが
  設計上の担保点。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| A. 既存 sf-03-slice 文書を個別に更新して統合契約を追記 | 既存チェーンに一体化 | slice 文書は approved・実装済みで、横断契約の追記は各 slice の責務を越える。consumer 側（TRACE-C3/SF-04/SF-05）との相互参照が分散する | no |
| B. 新規の統合設計チェーン（本書）を上位権威として追加 | 単一文書で失敗不変条件・世代証拠・consumer 契約を一覧化。slice 文書は実装記録として残る | 文書数が増える | yes |
| C. 実装先行で設計を後追い | — | タスク指定（設計先行・docs のみ）に反する | no |

## Tentative Conclusions and Hypotheses

- **confirmed**: 世代証拠の authoritative owner は
  `RoutingEngine` の activation ledger + `RoutingActivationReceipt`
  であり、consumer（probe/control/UI/render）は自身の「active」
  概念を持たず generation 照合で stale を拒否する。これは
  slice1 ADR・slice4 `take_processor_control`・SF-04
  `StaleGeneration` reject・TRACE-C3 freshness 判定で既に
  部分的に実装されている。
- **confirmed**: `RoutingSourceMap` は lowering と同時に生成され
  `PreparedRouting` に同梱、publish 時に `BadSourceMap` 整合検査を
  通る。Project へ runtime ID を永続化しない。
- **confirmed**: `last_good_generation` は最後に RT が非零 frame で
  観測した Active 世代のみが前進させ、失敗経路では後退しない。
- **hypothesis→設計で確定**: 「Probe/Controls/PDC/PCM render/
  Undo/Save が同じ Revision を指す」は、全 consumer が
  `ProjectRevision` + plan `generation`（SourceMap/receipt/
  ControlTraceKey 経由）を相関キーとし、独自の active 判定を
  持たないことで担保する — 本チェーンの設計文書で不変条件として
  明文化する。
- **open**: SF-ACT-07 の Partial Preview vs Export fail-closed
  分岐 policy は未規定（slice4 で「両 mode fail-closed 維持」と
  記録済み）。本チェーンでは現行維持を設計判断として記録し、
  policy 決定は後続 Issue へ defer する。

## Open Questions

- 実 OS device/VST3・RT latency/underrun・AudioLens f64 実機
  非回帰は headless では検証不能 — NotRun として fixture 一覧に
  分離記録し、#114 gate / 実機タスクへ送る（設計文書で明示）。
- Ruviform generic の不足が判明した場合は Synvelis 側で恒久的な
  重複実装をせず基盤 Issue へ起票する運用を記録（現時点で
  blocking な不足は未検出 — probe rebind #308・clock alignment
  #309・typed graph/RT API は pin `0f5b5737` で利用可能）。

## Promotion Candidates

- [x] spec-doc needed: semantic→runtime 写像の要求事項と
  SF-ACT-01/02/03/04 の設計行を確定する spec。
- [x] adr-doc needed: 世代スコープ証拠の authoritative owner と
  consumer 境界の決定。
- [x] design-doc needed: 責務境界・lowering/PDC・SourceMap・
  世代証拠・失敗不変条件・実装順序・検証計画。

## Sources and Relations

- Issue #106（本 slice の source）、#104/#105（SF-01/02 正本）、
  #100（TRACE-C3）、#107（SF-04）、#108（SF-05）、#103（PC-C6）。
- 実装: `crates/synvelis-project/src/routing_build.rs`・
  `trace.rs`・`probe.rs`・`probe_trace.rs`、
  `crates/synvelis-player/src/routing_lower.rs`・
  `routing_engine.rs`・`probe.rs`・`probe_trace.rs`・`trace.rs`、
  `crates/synvelis-workspace`（`EnginePort`/`ControlSession`/
  `signal_flow_ui`）、`crates/synvelis-app`
  （`SessionRoutingEngine`）。
- 文書: `docs/specs|designs|adr|plans|tasks|test-specs/sf-03-*`、
  `pc-c3-*`、`sf-04-*`、`sf-05-*`、`pc-c6-*`。

## Evidence

- 2026-11-09 棚卸し実施: `git log` で SF-03 slice1〜5 の merge
  （PR #191/#192/#195/#197/#198）と TRACE-C3/SF-04/SF-05/PC-C6
  （PR #200/#206/#209/#212/#215/#216）を確認。既存 sf-03 文書に
  TRACE-C3/SF-04/SF-05 への参照が無いことを grep で確認
  （consumer 横断契約は未記述）。
- `crates/synvelis-project/src/routing_build.rs` の
  `RoutingBuildKey`/`RoutingSourceMap`/`RoutingFault`（18 分類）/
  `RoutingActivationState`/`RoutingActivationReceipt`/
  `RoutingReconcileReport`/`ExecutionEquivalence` を確認。
- `crates/synvelis-player/src/routing_engine.rs` の
  `prepare`/`publish`/`reconcile`/`observe_activation`/
  `take_processor_control`/probe passthrough を確認。
- `crates/synvelis-project/src/trace.rs` の `ControlTraceKey`
  （`project_revision`/`graph_generation`/`control_plan_generation`/
  `transport_generation`/`clock_epoch`/`evaluation_position`）と
  `ObservationFreshness` を確認。
- `crates/synvelis-project/src/probe.rs` の `resolve_probe_point`
  （`RoutingSourceMap`+`GraphSpec` 経由の fail-closed 解決）を確認。
