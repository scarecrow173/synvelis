---
id: "IMPL-GWuKwuDKXNXu38FweAsWla"
type: "impl"
status: "completed"
title: "sv293 semantic→selection integration 実装記録 (synvelis#293 deferred residual slice)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/rcp_selection.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/rcp_selection_integration.rs"
      - type: "file"
        path: "docs/specs/sv293-semantic-selection-integration.md"
      - type: "file"
        path: "docs/designs/sv293-semantic-selection-integration.md"
      - type: "file"
        path: "docs/plans/sv293-semantic-selection-integration.md"
      - type: "file"
        path: "docs/tasks/sv293-semantic-selection-integration.md"
      - type: "file"
        path: "docs/test-specs/sv293-semantic-selection-integration.md"
      - type: "file"
        path: "docs/discovery/sv293-semantic-selection-integration.md"
      - type: "file"
        path: "docs/impl/exp/sv293-semantic-selection-integration.md"
      - type: "file"
        path: "docs/impl/exp/sv293-semantic-selection-integration-exp.jsonl"
      - type: "file"
        path: "evidence/sv293/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv293/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/test-specs/sv293-semantic-selection-integration.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv293-semantic-selection-integration.md"
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "docs/plans/sv293-semantic-selection-integration.md"
    - "docs/tasks/sv293-semantic-selection-integration.md"
    - "docs/test-specs/sv293-semantic-selection-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
    - "docs/impl/exp/sv293-semantic-selection-integration.md"
    - "evidence/sv293/acceptance-ledger.md"
    - "evidence/sv293/verification.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv293-semantic-selection-integration.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-workspace/src/rcp_selection.rs"
    - "crates/synvelis-workspace/tests/rcp_selection_integration.rs"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# sv293 semantic→selection integration 実装記録

## 実装内容

### `crates/synvelis-workspace/src/rcp_selection.rs`（新規）

`synvelis_project::RoutingModel` + caller pin evidence から
RCP-04 契約入力を構築し、`ExecutionSelector` →
`PlanReconciler` → `RoutingEngine` を駆動する module。

公開 API:

- `SelectionInputs` — project・revision・snapshot_id・engine・
  各 generation pin・clock epoch・runtime instance・
  `plugin_evidence`（`ProcessorId` keyed）・`environments`
  （descriptor keyed）・`render_index_generation`・
  `render_candidates`。
- `ProcessorPluginEvidence` — authored identity・state decl・
  automation identity completeness。
- `SemanticRenderCoverage` / `SemanticRenderCandidate` —
  semantic entity coverage + lookup `CandidateReport`。
- `SelectionResolution { snapshot, capabilities, renders }`。
- `SelectionBuildError` — `MissingPluginEvidence`/
  `DescriptorMismatch`/`UnknownCoverage`/semantic port・接続・
  format 系の typed error。
- `resolve_selection(&SelectionInputs) -> Result<
  SelectionResolution, SelectionBuildError>`。
- `drive_selection_plan(engine, reconciler, project, revision,
  live, capabilities, selection, request, deps) ->
  SelectionPlanDrive` — `Published`/`Equivalent`/`Failed`。
- `selection_pin_fault` — stale gate（private 相当の公開
  振る舞いは drive 経由で pin）。

### `crates/synvelis-workspace/src/lib.rs`

`pub mod rcp_selection` + 公開型の re-export。

### `crates/synvelis-workspace/Cargo.toml` / `Cargo.lock`

`ruviform-graph.workspace = true`・
`ruviform-audio-core.workspace = true` を追加
（Coordinator 承認済み — RCP 契約型 `PortFormat`/
`SignalDomain`/`AudioFormat` がこれらの型）。
`Cargo.lock` は dep 追加に伴う最小差分のみ。

### `crates/synvelis-workspace/tests/rcp_selection_integration.rs`（新規）

9 test・全て公開 API のみ。fixture は `ProjectController` で
semantic routing model を構築し、`StaticProcessorHost` +
`GainProcessor` で engine plan 構築脚を実走させる
（plugin 実行可能性 verdict は capability evidence 駆動のまま）。

| test | 対応 |
| --- | --- |
| `rcp_ex01_main_playback_all_live_publishes` | RCP-EX-01 MainPlayback Exact→publish + build key pin 一致 |
| `rcp_ex01_missing_plugin_blocks_main_playback` | RCP-EX-01/03 Missing→Blocked・偽 Exact 無し |
| `rcp_ex01_missing_plugin_exact_substitution_publishes` | RCP-EX-01 Exact candidate 置換→publish |
| `rcp_ex01_offline_render_enforces_min_reproducibility` | RCP-EX-01 OfflineRender min_reproducibility gate |
| `rcp_ex03_export_rejects_preview_only_artifact` | RCP-EX-03 Export/MainPlayback 拒否 + EditingPreview opt-in |
| `rcp_ex04_edit_lifecycle_norebuild_controlonly_rebuild` | RCP-EX-04 NoRebuild/ControlOnly/Rebuild + Equivalent |
| `rcp_ex05_stale_selection_never_publishes` | RCP-EX-05 stale gate・Blocked・last-good・fresh 復帰 |
| `rcp_ex02_topology_edge_kinds_preserved` | RCP-EX-02 Send/Sidechain/Return/Main edge kind + chain 順 |
| `rcp_producer_fails_closed_on_bad_evidence` | producer fail-closed 契約 |

## 設計上の決定

- `structure_digest` = `routing_fingerprint(project).0` —
  engine `checked_key` と field 一致させるための共有 digest。
- `EdgeKind::Return` = source strip が Return bus 所有の Main
  edge（return bus → mix feed）。send tap は `EdgeKind::Send`。
- `drive_selection_plan` は engine 呼出前に
  `selection_pin_fault` で stale を拒否（reconciler の
  publish 時再検証だけでは engine activation が先行し得る
  ための二重 gate）。
- `RoutingCompileRequest` は `synvelis_project` の public
  re-export を使用。
- test の `StaticProcessorHost` は drive 毎に再武装
  （`connect` が instance を consume する契約。
  `capability_generation=0` で consume は drift 扱いされない）。

## 検証

`evidence/sv293/verification.md` 参照。oracle 9/9 green、
`mise run` 系 check/lint/test/fmt + `prek run --all-files` 実行。
