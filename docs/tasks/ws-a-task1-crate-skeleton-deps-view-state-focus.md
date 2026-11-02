---
id: "TASK-034Qx0YZqo0BocaH8p0zvQ"
type: "task"
status: "done"
title: "WS-A task1 crate skeleton・deps更新・error/view/state/focus"
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
    - "docs/plans/ws-a-workspace-foundation.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ws-a-workspace-foundation.md"
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

## Work

- `Cargo.toml` に `crates/synvelis-workspace` を member 追加し、
  `ruviform-ui` git 依存を workspace deps へ追加。全 ruviform source を
  #181/#182 含有 rev へ更新し `Cargo.lock` 再生成。
- `crates/synvelis-workspace` 骨格: lib/error/view/state/focus。
  ViewType・ViewDescriptor・ViewRegistry・ViewFactory(HeadlessView)・
  ViewState(EditingSelection/InspectionFocus/LinkedContext/Viewport)・
  ViewStateRegistry・FocusState(NativeWindowFocus/ActiveView/InputFocus)。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ws-a-workspace-foundation.md
- docs/designs/ws-a-workspace-foundation-design.md

## Dependencies

- なし (最初の task)

## Affected Files

- Cargo.toml / Cargo.lock
- crates/synvelis-workspace/**

## Done When

- [ ] `mise run check` が synvelis-workspace 込みで通り、全 ruviform
  source が同一 rev に解決される。

## Verification

- [ ] `mise run check` / `cargo tree -p synvelis-workspace` で依存確認
