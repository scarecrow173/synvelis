---
id: "TASK-106SF03Gh5hIj6kLm7nOp8qRs"
type: "task"
title: "SF-03 slice1 T1 — 契約型(routing_build)・plan_playbacks 抽出・lower_semantic・StripProcessor"
status: "done"
created: "2026-11-08"
updated: "2026-11-08"
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
  implements:
    - "docs/plans/sf-03-slice1-lowering-activation.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
  blocks:
    - "docs/tasks/sf-03-slice1-activation-tests.md"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice1-lowering-activation.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice1 T1 — 契約型 + lowering

- `synvelis-project::routing_build`: BuildKey/CompileRequest/
  MappingResolution/SourceMap/Fault/ActivationState/Receipt +
  `routing_fingerprint`。
- `lowering.rs`: `plan_playbacks`/`PlaybackPlan` 抽出(legacy 不変)。
- `routing_lower.rs`: `lower_semantic`・`SemanticLowered`・node role・
  `StripProcessor`・send gain node・audibility edge prune。
- 受入: SF-ACT-01/02 の lowering 側(SourceMap revision 一致、
  Golden PCM、tap/順序、Missing Source)。
