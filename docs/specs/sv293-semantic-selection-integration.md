---
id: "SPEC-uTwqq0mP3J2SzmvaRnBQVo"
type: "spec"
status: "approved"
title: "sv293 semantic→selection integration 仕様 (synvelis#293 deferred residual slice)"
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
    - "docs/designs/sv293-semantic-selection-integration.md"
  depends-on:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/discovery/sv293-semantic-selection-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies:
    - "docs/specs/render-checkpoint.md"
  verified-by: []
  references:
    - "docs/test-specs/sv293-semantic-selection-integration.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# sv293 semantic→selection integration 仕様

## 目的

Issue #293 の deferred residual: Synvelis の semantic
project/routing model（`synvelis_project::RoutingModel`）から、
merge 済みの RCP-04 headless 決定契約
（`synvelis_player::rcp`）が要求する pin 済み入力を決定的に
構築し、`ExecutionSelector` → `PlanReconciler` →
`RoutingEngine` prepare/publish の plan 構築経路へ接続する。

## 要求

### R1: semantic snapshot producer

`synvelis-workspace` 内の新規 module が、以下を
`Project::routing` + caller 提供 evidence から構築する:

- `ResolvedGraphSnapshot`（`project_id`/`project_revision`/
  `structure_digest`/`capability_generation`/
  `engine_config_generation`/`source_format_generation`/
  `clock_epoch`/`runtime_instance` + nodes/edges/required_outputs）。
  `structure_digest` は `routing_fingerprint` と一致させ、
  selection の `build_key` が engine の `checked_key` と
  field 一致すること。
- `PluginCapabilitySnapshot` — authored identity・state evidence・
  declared port layout・environment map から構築。plugin evidence
  は `ProcessorId` keyed で descriptor が binding と一致しない
  場合は fail closed。
- `RenderCandidateIndex` — `synvelis_render::artifact::lookup`
  の `CandidateReport` を `SemanticRenderCandidate` 経由で
  `RenderCandidate` に変換。eligibility/reproducibility/access/
  rejection を推測しない。
- `ExecutionSelector::select` の呼び出しと
  `ExecutionSelection`/`ReconciliationReceipt` の返却。

### R2: PlanReconciler → RoutingEngine 配線

`ExecutionSelection.build_key`（`RoutingBuildKey`）を
`PlanReconciler::prepare` → stale-pin 事前 gate →
`RoutingEngine::prepare` → `publish` →
`PlanReconciler::publish` の一連の plan 構築経路へ接続する。
配線は `RoutingEngine`（player）レベルまで。
`SessionRoutingEngine`（app adapter）は本 slice 対象外。

### R3: fail-closed 契約

- plugin evidence 欠落・descriptor 不一致・semantic port/connection
  不正・format/channel 不整合は typed error で拒否。
- environment map に entry が無い plugin は installed と推測
  しない（`PluginVerdict::Missing` へ写る）。
- selection pin（revision/structure digest/capability/
  engine・source generation/clock epoch/runtime instance）が
  live snapshot/capability snapshot と乖離した selection は
  engine prepare/publish の前に拒否（stale activation 0）。

### R4: oracle test

`crates/synvelis-workspace/tests/rcp_selection_integration.rs`
（新規）で MainPlayback/EditingPreview/OfflineRender/Export の
各 Intent 経路について、semantic model → snapshot/render index
producer → `ExecutionSelector::select` → receipt →
`PlanReconciler` → `RoutingEngine` plan 構築の接合部を pin する。
Missing Plugin 含む graph で Intent 毎の Plan/Blocked/Preview
verdict・cutset 置換・偽 Exact 無し・NoRebuild/ControlOnly/
Rebuild 判定・stale activation 0 を検証。公開 API のみ使用。

## 非目標

- Ruviform Graph Compiler/Plugin Host の複製。
- player/render 側 API の変更。
- app adapter（`SessionRoutingEngine`）配線。
- BranchAudition/provenance 経路（#294）。
- RCP-EX-06 実機受入（#296 RCP-GATE — NotRun を維持）。
- docs index の再生成（Coordinator が統合時に実施）。

## 受入条件（slice 確定分）

1. producer module + 公開 API が `synvelis-workspace` に存在し、
   決定的・I/O なし・全入力 explicit pin。
2. `drive_selection_plan` が `PlanReconciler`/`RoutingEngine` の
   既存 fail-closed・last-good 契約を維持したまま接続される。
3. integration oracle が上記 Intent 経路を headless で green。
4. eligibility 写像が `render_ui`/`select` の表示契約と矛盾
   しないことを oracle で pin。
5. doc chain + `evidence/sv293/` ledger/verification を追加。
