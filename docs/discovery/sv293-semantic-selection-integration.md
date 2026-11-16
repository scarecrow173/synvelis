---
id: "DISC-UFuXWskbvDWdDScv2CA8XP"
type: "discovery"
status: "completed"
title: "sv293 semantic→selection integration 調査記録 (synvelis#293 deferred residual)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/sv293-semantic-selection-integration.md"
  depends-on:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-player/src/rcp/mod.rs"
    - "crates/synvelis-player/src/routing_engine.rs"
    - "crates/synvelis-project/src/routing.rs"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# sv293 semantic→selection integration 調査記録

## 背景

Issue #293（RCP-04 Missing Plugin Subgraph Resolver）の headless
決定契約は player slice（PR #426・`1bdc3fd`）で merge 済み。
`crates/synvelis-player/src/rcp/mod.rs:26-30` が
「workspace semantic→selection integration は deferred residual —
another lease owns synvelis-workspace」と明記しており、本 slice が
その residual を引き受ける。

## 調査結果

### 契約入力（merge 済み・変更不可）

- `ExecutionSelector::select(&ResolvedGraphSnapshot,
  &ExecutionIntent, &PluginCapabilitySnapshot,
  &RenderCandidateIndex) -> ExecutionSelection`
  （`rcp/select.rs:197`）— deterministic・全入力 explicit pin・I/O なし。
- `ExecutionSelection.build_key: RoutingBuildKey` — downstream の
  Ruviform prepare/publish へ渡す build key。
- `PlanReconciler`/`classify_edit`（`rcp/reconcile.rs`）—
  NoRebuild/ControlOnly/Rebuild 分類・stale 拒否・last-good 保持。
- `synvelis_render::artifact::lookup::lookup`（`lookup.rs:412`）—
  `CacheLookup`/`LookupOutcome`/`CacheLookupResult`。
- `RoutingEngine::prepare/publish`（`routing_engine.rs:474`）—
  `RoutingCompileRequest` + `RoutingBuildKey` 経由、pin 検査、
  last-good 保持。

### semantic モデル側の事実

- `synvelis_project::RoutingModel`（`routing.rs:53`）が semantic
  graph の正本。`routing_fingerprint`（`routing_build.rs:510`）が
  structure digest の共有 digest。
- semantic port は `SemanticPort { owner: PortOwner, direction,
  role, domain, binding, channels }`。processor の Main in/out・
  sidechain port は `Processor::port_ids` が保持。
- `RoutingConnection.kind` は `Main`/`Send(SendId)`/`Sidechain`。
  strip owner が `StripOwner::Bus(BusId)` で bus kind が `Return`
  の場合、return bus → mix への Main edge が return feed。
- `synvelis-workspace` は `synvelis-player`/`synvelis-render`/
  `synvelis-project` に既依存。snapshot producer の置き場所は
  workspace 内新 module が自然。

### 発見した設計上の注意点

1. **依存スコープ**: RCP 契約型（`ResolvedPort.format: PortFormat`、
   `ResolvedEdge.channel_map`、`DeclaredPort`、`ArtifactOutput`）は
   `ruviform_graph`/`ruviform_audio_core` 型。`synvelis-workspace`
   は未依存だったため、Coordinator 承認のもと workspace 継承依存
   2 件（`ruviform-graph`/`ruviform-audio-core`）を追加した。
2. **`RoutingCompileRequest` の公開経路**: `routing_engine` では
   private で、`synvelis_project` が public re-export する。
   producer/配線は project 側の型を使う。
3. **`StaticProcessorHost::connect` は instance を consume する**
   （`host.rs:307-322`）。engine plan 構築を複数回 drive する
   oracle では drive 毎に host を再武装する必要がある。
   `capability_generation` は `0` 固定で consume は drift 扱い
   されない（`host.rs:270-278` の契約コメント）。
4. **`EdgeKind::Return` の向き**: 「Return-bus feed」は return
   bus → mix 方向（source strip が Return bus 所有）と解釈。
   send tap 側（track → return bus 入力）は `EdgeKind::Send` が
   担う。`docs/test-specs/sf06-cross-gate.md:94` の
   「同一 return bus → master へ繋がる」記述と整合。
5. **eligibility 写像**: `gate_candidate`（`select.rs`）は
   MainPlayback/BranchAudition に `EligibleExact` +
   非 preview-class reproducibility を要求、
   EditingPreview は `allow_non_equivalent` opt-in、
   OfflineRender は `min_reproducibility` rank、
   Export は `BitExactCertified` 完全一致を要求。
   producer は lookup 報告をそのまま `RenderCandidate` に写し、
   eligibility/reproducibility/access を推測しない。

## スコープ外（deferred）

- RCP-EX-06 実機分（PCM・実 Plugin・実 OS・実 RT）→ #296 RCP-GATE。
- `SessionRoutingEngine`（`synvelis-app/src/engine.rs`）adapter 配線
  — lease 外。`RoutingEngine` レベルまでが本 slice の境界。
- BranchAudition/provenance 経路 — #294 影響圏。
- docs index 再生成 — Coordinator が統合時に実施。
