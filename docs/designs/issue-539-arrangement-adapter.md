---
id: "DESIGN-539aArrngmntAdptr03"
type: "design"
status: "approved"
title: "Issue #539 design — adapters map への ArrangementAdapter 登録と pin fixture positive flip"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/539"
    - "docs/specs/issue-539-arrangement-adapter.md"
    - "docs/discovery/issue-539-arrangement-adapter.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-539-arrangement-adapter.md"
  implemented-by:
    - "docs/plans/issue-539-arrangement-adapter.md"
  depends-on:
    - "docs/specs/issue-539-arrangement-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/issue-539-arrangement-adapter.md"
  refines:
    - "docs/specs/issue-539-arrangement-adapter.md"
  refined-by: []
  derives-from:
    - "docs/specs/issue-539-arrangement-adapter.md"
  derived-by:
    - "docs/plans/issue-539-arrangement-adapter.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #539 design — adapters map への ArrangementAdapter 登録と pin fixture positive flip

## 設計判断

### 配線位置: `VariantPorts::standard`

`ArrangementAdapter` は `synvelis_project::variant` に既に存在し、
scope validation・candidate edit・preflight adopt check を実装済み。
gateway 側の不足は adapters map への登録のみ。登録を
`standard()` に置くことで synvelis-cli・mcp-poc 両 host と
test rig へ自動伝播し、host 側変更を不要にする。

### fail-closed 機構の維持

`adapter_for` の `VariantError::Unsupported` 経路は変更しない。
全 7 variant に adapter が揃った現在この経路は到達不能だが、
将来の scope kind 追加時に未配線 kind が他 adapter を
黙って借用することを防ぐ防御として残す。陳腐化した
「Arrangement has no adapter」「no adapter yet」系の
doc comment 4 箇所を現行契約に合わせて更新する。

### test: pin fixture の positive flip

旧 `cb11_e2e_07_arrangement_scope_fails_closed_until_gateway_wiring`
を `cb11_e2e_07_arrangement_scope_composite_adopt_undo_scope_isolation`
へ置き換える。検証内容:

- demo_project の track 3 上の arrangement scope
  `[0, 28_800)` で fork
- candidate へ composite edit 2 件
  （`synvelis.clip.mute` on clip 11・`synvelis.clip.move`
  on clip 13 → `Musical(960)`）— adopt 前に live 不変を確認
- `ApprovalPolicy::ApproveToApply` の agent grant で
  adopt を prepare→apply。初回 apply は `ApprovalRequired`、
  human approval receipt 付き retry で commit
- `CommitGate` 経由で project revision が +1（単一 CAS）、
  provenance に `project.committed == true`・
  `project.undo_steps == 1`・期待 `after_revision`
- 両 scoped edit が live に materialize、clip 12・notes・
  MIDI sources・media sources・assets・tracks・lanes は
  bitwise 不変
- candidate branch が adopted branch になる

### gap 記録の解消

`evidence/sv288/coverage.md` の gateway 経路行・fixture 表・
「判明した gap」節、`docs/impl/ir|exp|test-specs/cb-11-cross-gate.md`
の該当注記に解消を記録する。本 slice の検証証拠は
`evidence/sv539/` に置く。

## 却下した代替案

- host 側で個別に adapter を wire する案: `standard()` への
  登録で自動伝播するため冗長。consumer ごとの配線漏れを
  生む。
- `adapter_for` に Arrangement 特例を追加する案:
  adapters map が正本であり、特例は fail-closed 契約を
  弱める。
