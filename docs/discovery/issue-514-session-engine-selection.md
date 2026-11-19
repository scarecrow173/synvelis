---
id: "DISC-R611Xhqyq8ib0vHl2lV2e1"
type: "discovery"
status: "resolved"
title: "Issue #514 discovery — SessionRoutingEngine の RCP selection 消費要否の root-cause 調査"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
    - "https://github.com/scarecrow173/synvelis/issues/293"
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
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/designs/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
    - "evidence/sv514/decision-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-514-session-engine-selection.md"
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
    - "evidence/sv293/acceptance-ledger.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
    - "crates/synvelis-app/src/engine.rs"
    - "crates/synvelis-app/src/composition.rs"
    - "crates/synvelis-workspace/src/rcp_selection.rs"
    - "crates/synvelis-player/src/routing_engine.rs"
    - "crates/synvelis-player/src/rcp/verify.rs"
    - "crates/synvelis-player/src/rcp/model.rs"
  defers: []
  deferred-by: []
---

# Issue #514 discovery — SessionRoutingEngine の RCP selection 消費要否

## Issue framing

Synvelis#293 / Synvelis#106 の residual: RCP-04 semantic→selection
統合（`resolve_selection` / `drive_selection_plan`）は
`synvelis-workspace` / `synvelis-player` レベルで実装・テスト済みだが、
production caller は 0 であり、`SessionRoutingEngine`
（`crates/synvelis-app/src/engine.rs`）が selection を消費するかは
「Deferred」のまま未確定だった。本 Issue の deliverable は、app 層
adapter が RCP selection を消費すべきかを root-cause で確定し、
採否を記録すること。実装する場合は配線 + adapter レベル pin、
実装不要の場合は責務が閉じる根拠と「意図的な未配線」の明記が必要。

## 調査結果（実測）

### 現行 rebuild 経路

`SessionRoutingEngine` は `EnginePort::on_project_revision` で
`(&Project, ProjectRevision)` のみを受け取り、`plan_playbacks` →
`resolver::prepare`（PreparedClip 組立）→ `RoutingCompileRequest` /
`RoutingDeps` 組立 → `RoutingEngine::prepare` → `publish` を直接駆動
する（`crates/synvelis-app/src/engine.rs` rebuild 経路）。
`resolve_selection` / `ExecutionSelector` / `PlanReconciler` /
`drive_selection_plan` への参照は synvelis-app 全体で 0 件
（git grep 実測）。

### selection 側の契約形状

`drive_selection_plan`（`crates/synvelis-workspace/src/rcp_selection.rs`）
は PlanReconciler plan → `selection_pin_fault`（stale pin の
pre-publish gate）→ `RoutingEngine::prepare(request, project,
revision, deps)` → `publish` を駆動する。**engine へ渡す入力は
rebuild と同一**であり、selection は published plan の内容を変えない
（`ExecutionSelection.substitutions` は decision 証拠であり、
`lower_semantic` は常に `project.routing` 全体を lowering する）。

### SelectionInputs の供給可能性

`SelectionInputs` は `plugin_evidence`（`PluginIdentity` +
`PluginStateDecl` + automation completeness）、`environments`
（descriptor 毎の `PluginEnvironment` facts）、`render_candidates`
（`SemanticRenderCandidate{coverage, CandidateReport}`）を要求する。

- `plugin_evidence`: `ProcessorBinding` は descriptor/version/
  capability_revision のみ保持。identity/state/automation の producer
  は存在しない（TIM-04 state capture・env scan は未着地）。
- `environments`: `PluginHost` は Scan(descriptors)/Instantiate のみ
  公開し、environment facts を返す API がない。
- `render_candidates`: `CandidateReport` は `synvelis-render` の
  lookup 経路由来で、`SemanticRenderCoverage`（artifact→semantic
  subgraph 対応）の producer は存在しない（RCP-01/02/05 領域）。

synvelis-app は synvelis-render 非依存の設計であり、
`CandidateReport` を直接名指しすると dep 追加になる。workspace 側
供給 port で回避可能だが、供給元自体が存在しない。

### 安全性 pin の所在

Issue が「実装する」分岐で要求する pin は全て既存:

- stale selection 非 publish: `selection_pin_fault` pre-gate +
  `rcp_ex05_stale_selection_never_publishes`（workspace
  `rcp_selection_integration.rs`）。
- generation/epoch/build-key 不一致で旧 plan が Active にならない:
  `RoutingEngine::checked_key`（project_revision・routing_fingerprint・
  port_capability_generation・engine config generation・source format
  generation・lowering policy・build mode・device clock epoch・runtime
  instance）+ publish 再検証、player `rcp_04` oracle。
- QueueFull/RetireOverflow で last-good 保持:
  `ex05_queue_full_and_retire_failure_retain_last_good` + engine の
  receipt 契約。
- adapter レベルの session pin: `sf03_session_engine.rs`（publish→
  Active→last-good→equivalence→stale revision/reconcile）。

`ProcessorHost::connect` は plugin を実際に instantiate する
ground-truth の executability oracle であり、selection evidence は
caller 申告の弱い二次情報。evidence/host が乖離した場合、verdict を
hard gate にすると engine が build できる plan を false-block する
失敗モードを新たに持ち込むだけで、能力は増えない。

### consumer 側の確認

`render.rs` の `RenderStatusScreenState.selection:
Option<RenderSelectionView>` と workspace `project_selection`
（`ExecutionSelection` → `RenderSelectionView` 投影）は live
selection の producer を待つ構成（sv#295 面）。producer は
per-intent の selection service が自然であり、intent 非依存・
`BuildMode::Realtime` 固定の rebuild hot path ではない。
OfflineRender/Export intent は synvelis-render 側の lowering 経路を
通り、SessionRoutingEngine を経由しない。

## 判定

**「RoutingEngine レベルで責務が閉じる（実装不要）」**。

根拠の要約:

1. `EnginePort` は `(&Project, ProjectRevision)` のみを渡し、intent・
   plugin 証拠・environment facts・render candidates は渡さない。
   rebuild は plan 実行 adapter であり、selection の入力を組み立てる
   責務も消費する責務も持たない。
2. `drive_selection_plan` が駆動する engine build は rebuild と同一。
   selection は published plan を変えず、substitution は現契約では
   再生に反映されない（engine 側の plan shaping が必要 — player 契約
   変更、lease 外）。
3. `SelectionInputs` の全 pin を production で供給する経路が存在
   しない。空 evidence は fail-closed、捏造 evidence は honest-input
   契約違反、部分 evidence の gate 化は false-block を持ち込む。
4. Issue が要求する安全性 pin は RoutingEngine レベルで既に pin 済み
   であり、adapter は engine の契約をそのまま転送するだけ。

詳細な設計根拠は `docs/designs/issue-514-session-engine-selection.md`、
決定記録は `docs/adr/issue-514-session-engine-selection.md`、
証拠は `evidence/sv514/decision-ledger.md` を参照。
