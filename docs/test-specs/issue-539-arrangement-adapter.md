---
id: "TSPEC-539aArrngmntAdptr07"
type: "test-spec"
status: "approved"
title: "Issue #539 test-spec — gateway 経路 arrangement composite adopt の検証仕様"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/539"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-539-arrangement-adapter.md"
    - "docs/designs/issue-539-arrangement-adapter.md"
    - "docs/adr/issue-539-arrangement-adapter.md"
  implemented-by:
    - "docs/impl/ir/issue-539-arrangement-adapter.md"
  depends-on:
    - "docs/specs/issue-539-arrangement-adapter.md"
    - "docs/designs/issue-539-arrangement-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-11-cross-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-539-arrangement-adapter.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-539-arrangement-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #539 test-spec — gateway 経路 arrangement composite adopt の検証仕様

## Feature

CB-E2E-03 gateway 経路: arrangement scope の variant
fork→candidate composite edit→adopt が `VariantPorts::standard`
経由で headless 完走する。

## Rules

- R1: arrangement scope の `synvelis.branch.fork` が
  `VariantError::Unsupported` ではなく adapter dispatch を
  通り candidate branch を生成する
- R2: candidate への複数 project-op edit
  （`synvelis.clip.mute`・`synvelis.clip.move`）が蓄積し、
  adopt 前に live project を変更しない
- R3: adopt は `ApprovalPolicy::ApproveToApply` で
  human approval receipt を要求し、receipt 無しの初回
  agent apply は `ApprovalRequired` で拒否される
- R4: adopt commit は project revision を単一 CAS（+1）で
  進め、provenance に `project.committed == true`・
  `project.undo_steps == 1`・期待 `after_revision` を残す
- R5: scoped edit 2 件が live に materialize し、scope 外の
  clip・notes・MIDI sources・media sources・assets・tracks・
  lanes は bitwise 不変
- R6: candidate branch が adopted branch として確定する

## Example

`cb11_ai_vcs_off.rs::cb11_e2e_07_arrangement_scope_composite_adopt_undo_scope_isolation`
— demo_project の track 3 arrangement scope `[0, 28_800)` で
fork → mute clip 11 + move clip 13 to `Musical(960)` →
approve-gated adopt → CommitGate commit → isolation assert。

## Oracle 種別

独立 oracle: `CommitReceipt` provenance JSON・
`Project` の entity 比較（PartialEq）・revision CAS。
fixture 自身の実装を oracle にしない。

## Runnability

T0 headless — `mise run test synvelis-gateway` で実行可能。
GUI-L2・EXT-L3・実 PCM/RT は対象外（外部 gate）。
