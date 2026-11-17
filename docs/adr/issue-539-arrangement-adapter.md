---
id: "ADR-539aArrngmntAdptr04"
type: "adr"
status: "accepted"
title: "Issue #539 — ArrangementAdapter を VariantPorts::standard の adapters map に登録し fail-closed 機構は維持する"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/539"
    - "docs/discovery/issue-539-arrangement-adapter.md"
    - "docs/specs/issue-539-arrangement-adapter.md"
    - "docs/designs/issue-539-arrangement-adapter.md"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-539-arrangement-adapter.md"
      - type: "file"
        path: "docs/specs/issue-539-arrangement-adapter.md"
      - type: "file"
        path: "docs/designs/issue-539-arrangement-adapter.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-539-arrangement-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/designs/issue-539-arrangement-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-539-arrangement-adapter.md"
  derived-by:
    - "docs/plans/issue-539-arrangement-adapter.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #539 — ArrangementAdapter を VariantPorts::standard の adapters map に登録し fail-closed 機構は維持する

## Context

`VariantScopeKind::Arrangement` は存在し、
`ArrangementAdapter` も `synvelis-project` に merge 済みだが、
`VariantPorts::standard` の adapters map への登録が欠落して
いたため gateway dispatch が `VariantError::Unsupported` で
fail-closed していた（CB-11 slice 2 で gap として記録済み）。

## Decision

1. `VariantPorts::standard` の adapters map に
   `VariantScopeKind::Arrangement → ArrangementAdapter::new()`
   を登録する。`adapter_for` の fail-closed 機構は変更せず、
   未配線 kind が将来追加されても `Unsupported` で安全側に
   倒れる構造を維持する。
2. 陳腐化した doc comment（「Arrangement has no adapter」等
   4 箇所）を現行契約に合わせて更新する。
3. pin fixture を positive 検証へ flip し、gateway 経路の
   CB-E2E-03 composite adopt（単一 CAS=1 undo・scope 外
   bitwise 不変）を headless で assert する。

## Consequences

- synvelis-cli・mcp-poc 両 host と test rig が
  `standard()` 経由で arrangement dispatch を自動的に得る。
- CB-11 coverage ledger の gateway 経路 arrangement 行を
  Pass に更新できる。
- `adapter_for` の `Unsupported` 経路は全 variant 網羅後は
  到達不能になるが、将来の scope kind 追加への防御として
  機構自体は残る。
