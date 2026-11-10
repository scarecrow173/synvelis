---
id: "SPEC-034VoS9yTOt7fYOoDYlyRU"
type: "spec"
status: "approved"
title: "SF-03 slice8 — app/workspace same-Revision fixture"
created: "2026-11-10"
updated: "2026-11-10"
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
  depends-on:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-03-slice7-render-consumption.md"
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  derived-by:
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice8-revision-fixture.md"
  references:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  defers: []
  deferred-by: []
---

# SF-03 slice8 — app/workspace same-Revision fixture

## What Should Be Built

map design §8.2-1 が指定する consumer 横断 same-Revision fixture の
app/workspace 側残余を、headless test のみで固定する。本番コード
変更なし — 必要な公開面（`SessionRoutingEngine` の
receipt/source_map/probe_bindings/reconcile、
`SynvelisApp::signal_flow_view`、workspace `signal_flow_graph` +
`SignalFlowRuntimeEvidence` 合成経路）は全て公開済み。

- `crates/synvelis-app/tests/sf03_revision.rs`（新規）:
  `SessionRoutingEngine` 実体 + `ProjectSession` +
  `SynvelisApp::signal_flow_view` を跨ぐ fixture。tempdir fixture
  WAV + `StaticProcessorHost` + `PumpPolicy` 駆動
  （`sf03_session_engine.rs`/`sf05_signal_flow.rs`/
  `sf04_probe_wiring.rs` の helper パターン踏襲）。
- `crates/synvelis-workspace/tests/sf03_revision.rs`（新規）:
  workspace は player/app 非依存のため `EngineStub`/`EnginePort`
  通知 + 合成 `SignalFlowRuntimeEvidence` 経路で、
  `session.revision`・signal_flow 投影・undo/redo/save の同一
  Revision を固定する。

## Why Now

render 側の cross-consumer same-Revision fixture は slice7
（`sf03_render.rs`、PR #208 系）で完了済みだが、design §8.2-1 の
app/workspace 側は別 dispatch として明示的に deferred されていた。
slice5（session→engine 接続）・slice6（control surface）・
slice7（render 消費）で各 consumer の契約は個別に固定されたが、
app の `signal_flow_view` が receipt/source_map/probe binding と
workspace 投影の双方と同一 committed revision を指すことを
横断的に固定する fixture はまだない。Issue #106 の SF-03 完了
条件の一部であり、後続 consumer（GUI slice）が依存する証拠軸を
先に固定する。

## Users and Value

- 後続の GUI/consumer slice: app/workspace が観測する revision・
  generation・semantic ID が engine の publish/activation 証拠と
  一致することの headless 保証。
- fixture 作者: `EnginePort` 通知・reconcile drift・save≠activation
  の回帰を production 変更なしで検出できる deterministic 経路。

## Scope

### In Scope

- `crates/synvelis-app/tests/sf03_revision.rs`（新規・4 fixture）。
- `crates/synvelis-workspace/tests/sf03_revision.rs`（新規・3 fixture）。
- docs チェーン（spec/design/plan/task/test-spec/impl）と managed
  index の再生成。

### Out of Scope

- `crates/*/src/` の一切の変更（公開面は全て既存）。
- SF-ACT-07（policy 未決・要 Issue）・SF-ACT-09（design 変更を
 伴う別 slice）・SF-ACT-05 実機・SF-ACT-08 upstream/AudioLens・
  全 GUI gate・SF03-RCP-01/02（#293 未実装）・defensive-only
  fault の live firing — `NotRun`/「未対応の作業」に記録。
- `synvelis-render` 側 fixture（slice7 で完了済み）。
- Issue #106 の Close（Coordinator が行う）。

## Requirements

- app fixture は `session.apply`/visual-only op/controller bypass/
  `save` を通し、`receipt.build_key.project_revision`・
  `source_map().project_revision`+`generation`・probe binding
  `project_revision`+`active_plan_generation`・
  `signal_flow_view.status` 各行が同一 committed revision を指す
  ことを固定する。
- semantic node/port ID は project routing の `SemanticNodeRef`
  そのもので、source map の key と view node の `Resolved`
  generation が一致すること（implicit conversion なし）。
- viewstate-only change は project revision・undo history・
  engine build・activation receipt を不変に保つ
  （`NoRebuild(Equivalent)`）。
- semantic no-op revision は engine 側 `Equivalent`（generation
  不変）だが、projection は旧 generation-scoped map を `stale`
  と正直に表示する — equivalence を新規証拠に見せかけない。
- controller bypass → `reconcile` が `ProjectRevision` mismatch +
  fault を報告し、次の session apply で live revision に収束する。
- `save` は snapshot HEAD を publish する永続化証拠であり、
  activation/RT 観測証拠ではない（save≠activation）。
- workspace fixture は `EnginePort` 通知 revision・
  `session.revision`・`signal_flow_graph` 投影・undo/redo/save が
  同一 revision 軸に留まることを、合成 evidence で固定する。
- 失敗時の振る舞い: engine 非装着・stale expected revision・
  未解決 semantic point は既存契約通り fail closed（本 slice で
  新規に緩めない）。

## Acceptance Criteria

- [ ] `crates/synvelis-app/tests/sf03_revision.rs` が green:
  same-Revision・viewstate/no-op honest stale・bypass→drift→
  converge・save≠activation の 4 fixture。
- [ ] `crates/synvelis-workspace/tests/sf03_revision.rs` が green:
  same-Revision・revision transition stale・save revision 軸の
  3 fixture。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-headless-deps` が exit 0。
- [ ] 本番コード・無関係ファイルの変更なし（docs index の再生成を
  除く）。

## Deferred Design Concerns

- SF-ACT-07 Partial Preview vs Export policy・SF-ACT-08/09・
  実機 RT latency・GUI gate — 別 slice/Issue。
- probe の RT-confirmed mode 実機証拠 — headless fixture は
  requested/pending 状態の正直な投影までを固定する。

## Sources and Relations

- Issue #106（SF-03）。
- `docs/specs/sf-03-map-semantic-runtime-contract.md`（親 contract）。
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
  §8.2-1（本 slice の根拠）。
- slice5 `sf-03-slice5-session-engine-connect.md`・slice7
  `sf-03-slice7-render-consumption.md`（consumer 別 fixture の先例）。
- 実装: `crates/synvelis-app/tests/sf03_revision.rs`、
  `crates/synvelis-workspace/tests/sf03_revision.rs`。
