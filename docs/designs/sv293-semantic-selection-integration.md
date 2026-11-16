---
id: "DESIGN-XIH1YjE7gCOlIHlrIPt8Rn"
type: "design"
status: "approved"
title: "sv293 semantic→selection integration 設計 (synvelis#293 deferred residual slice)"
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
  implements:
    - "docs/specs/sv293-semantic-selection-integration.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv293-semantic-selection-integration.md"
    - "docs/discovery/sv293-semantic-selection-integration.md"
    - "docs/designs/rcp-render-recipe-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/plans/sv293-semantic-selection-integration.md"
    - "docs/test-specs/sv293-semantic-selection-integration.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# sv293 semantic→selection integration 設計

## 全体構造

```
Project::routing (synvelis_project::RoutingModel)
        │
        ▼
resolve_selection(SelectionInputs)
        │  ├─ ResolvedGraphSnapshot   (semantic ports/edges → RCP nodes/ports/edges)
        │  ├─ PluginCapabilitySnapshot (evidence × environment → per-plugin facts)
        │  └─ RenderCandidateIndex     (SemanticRenderCandidate → RenderCandidate)
        ▼
ExecutionSelector::select(snapshot, intent, capabilities, renders)
        │  → ExecutionSelection { verdict, substitutions, unresolved,
        │                       degraded, receipt(ReconciliationReceipt) }
        ▼
drive_selection_plan(engine, reconciler, project, revision,
                     live_snapshot, capabilities, selection,
                     request, deps)
        │  1. reconciler.prepare(&selection.receipt)
        │  2. selection_pin_fault — stale gate (engine 呼出前)
        │  3. engine.prepare(request, project, revision, deps)
        │  4. Equivalent → return / Built → engine.publish
        │  5. reconciler.publish(plan, live, caps, engine_result)
        ▼
SelectionPlanDrive::{Published, Equivalent, Failed}
```

## producer の写像規則

### Node

- `ChannelStrip`（track/bus/return/master owner）→ `NodeClass::Strip`
  相当の processor node + chain processor node。
- chain processor → `NodeClass::Plugin` + `PluginDeclaration`
  （authored identity・state digest/quality/status・declared ports）。
- device sink → `NodeClass::Sink`。
- strip/bus plumbing（非空 chain の in/out 受け渡し）は snapshot
  local の `StripPlumbing` で構築し、semantic entity には
  後付け ID を振らない。

### Port

- `SemanticPort` → `ResolvedPort`。`PortFormat::Audio(engine の
  AudioFormat)`、multiplicity は semantic `channels` から、
  availability/activation は port `binding` と接続の enabled 状態
  から決定。`DeclaredPort`（advertised layout）は environment の
  `advertised_ports` をそのまま使い、無い場合は fail closed では
  なく `None`（verify 層の判定材料として欠落を保持）。

### Edge

- `RoutingConnection` → `ResolvedEdge`。`kind`:
  `Send`→`Send`、`Sidechain`→`Sidechain`、`Main` は source strip
  が Return bus 所有なら `Return`、それ以外 `Main`。
- `channel_map` は `ExplicitChannelMap`（destination→source）を
  `(source, destination)` pair へ反転（lowering と同規約）。
- chain 内の processor 連続は `EdgeKind::Main` edge として
  chain 順に構築。

### Pin 世代

- `structure_digest` = `routing_fingerprint(project).0` —
  `RoutingBuildKey.routing_fingerprint` と同一 digest で、
  selection receipt の build key が engine `checked_key` と
  field 一致する（oracle で pin）。
- `capability_generation` = 入力 `SelectionInputs` の pin 値
  （caller が project 側 capability generation を供給）。
- `engine_config_generation`/`source_format_generation`/
  `clock_epoch`/`runtime_instance` は全て caller pin —
  producer は既定値を捏造しない。

## plugin evidence → capability の契約

- `ProcessorPluginEvidence` は `ProcessorId` keyed。
  `descriptor` が processor binding（Resolved `PluginRef` または
  Unresolved の last-known descriptor）と一致しない場合
  `DescriptorMismatch` で fail closed。
- evidence 欠落は `MissingPluginEvidence`。
- environment map は descriptor keyed。entry が無い plugin は
  `PluginEnvironment` を生成せず、capability snapshot 側で
  `Missing` verdict へ写る（installed 推測なし）。
- `PluginEnvironment` の `advertised_ports`/`state_restore`/
  `access`/`format_supported`/`resolved_version`/
  `resolved_binary_digest`/`uid_hash` は caller 供給値を保持。

## render candidate の契約

- `SemanticRenderCandidate { coverage: SemanticRenderCoverage,
  report: CandidateReport, content_hash }`。
- `coverage` は semantic entity（`ProcessorId`/`SemanticPortId`/
  `StripId`/`BusId`/`ConnectionId`/`SourceId`）を指し、
  producer が RCP node/port/edge ID へ解決する。未知 entity は
  `UnknownCoverage` で fail closed。
- `report`（lookup の `CandidateReport`）の
  `eligibility`/`reproducibility`/`access`/`rejection`/
  `artifact` をそのまま `RenderCandidate` へ写す —
  `EligibleExact` 昇格・preview→exact 誤分類はしない。
- `snapshot_revision`/`snapshot_digest` は snapshot の
  `project_revision`/`structure_digest` を pin（`gate_candidate`
  の stale 判定と整合）。

## drive_selection_plan の順序契約

1. `reconciler.prepare(&selection.receipt)` — candidate 採番。
2. `selection_pin_fault` — receipt.build_key と live
   snapshot/capability snapshot の pin 比較。乖離時は engine を
   一切呼ばず `Failed{StaleProjectSnapshot 等}`。
   （reconciler の publish 時再検証だけでは engine activation が
   先行し得るため、境界側で同規則を先に適用する二重 gate。）
3. `engine.prepare` → `Equivalent` は `SelectionPlanDrive::
   Equivalent`（publish/reconciler 記録なし — engine の
   NoRebuild 契約どおり）。
4. `Built` → `engine.publish` → `reconciler.publish(plan, live,
   caps, Ok)` → `Published`。
5. いずれかの fault → `reconciler.publish(plan, live, caps,
   Err(fault))` → `Failed`（last-good 保持は reconciler/engine
   の既存契約に委譲）。

## 依存追加

`crates/synvelis-workspace/Cargo.toml` に workspace 継承で
`ruviform-graph`/`ruviform-audio-core` を追加（Coordinator 承認
済み — RCP 契約型の `PortFormat`/`SignalDomain`/`AudioFormat`
がこれらの型であり、producer が synvelis-workspace に置かれる
以上必須）。`Cargo.lock` は dep 追加に伴う最小差分のみ。

## スコープ外

- `SessionRoutingEngine`（app）adapter — lease 外。
- BranchAudition/provenance — #294。
- RCP-EX-06 実機 — #296。
