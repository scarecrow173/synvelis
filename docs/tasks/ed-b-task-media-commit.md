---
id: "TASK-034QN1Ynloix2iEZj58mZJ"
type: "task"
status: "done"
title: "ED-B task: media move/trim/slip + marquee commit"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ed-b-media-domain-editing.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ed-b-media-domain-editing.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ED-B task: media move/trim/slip + marquee commit

## Work

`EditTimePoint` (domain-tagged position) を `selection.rs` に新設し、
`HitProbe`/gesture origin/preview position/`drag_update` をこの型へ
一般化。snap position を i64 scalar 化。`pointer_down` で probe domain
と fixation domain の不一致を開始時拒否し、target-free op は probe
domain を採用。`edit.rs` に media move (`MoveClip` anchor Absolute)、
media trim (`Composite`/`SetClipExtent`)、slip (`SetClipPlayback` +
mapping 別 `consumed_len`)、media marquee (`MediaSpan` overlap) の
commit 経路を実装。混在 domain の結果は設計の結果表どおり。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/ed-b-media-domain-editing.md`
- `docs/designs/ed-b-media-domain-editing-design.md`
- `docs/adr/ed-b-media-domain-gesture-positions-slip-trim-marquee-commit-semantics.md`

## Dependencies

- なし (plan の先頭タスク)。

## Affected Files

- `crates/synvelis-workspace/src/{selection,tool,gesture,snap,edit,workspace,lib}.rs`
- `crates/synvelis-workspace/tests/ed_a.rs` (API 追従)

## Done When

- [x] `EditTimePoint` 経由で media position が gesture に入る。
- [x] move/trim/slip/marquee が設計の結果表どおり commit/拒否される。
- [x] `mise run check` PASS (ed_a test は API 追従済み)。

## Verification

- [x] `mise run check` / `mise run lint` PASS。
