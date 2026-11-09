---
id: "PLAN-034UC46sceMw6QCzqVjGOO"
type: "plan"
status: "approved"
title: "TIM-04 plugin state snapshot consistency — implementation plan"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/tim-04-plugin-state-snapshot-consistency-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-04-plugin-state-snapshot-consistency-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-04-plugin-state-snapshot-consistency-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-04 plugin state snapshot consistency — implementation plan

## Goal

Issue #128 の headless/mock slice として、plugin state の
capture 品質分類・content-addressed store・save/restore
receipt・HEAD reconcile・offline recipe 品質ゲートを
`synvelis-project` に実装し、`synvelis-player` の非 RT host
adapter と `synvelis-render` の recipe gate に接続する。

## Scope

単一 implementation stream。project の `plugin_state`
module が中核で、player adapter と render gate はそれを
消費する薄い接続。実 plugin/OS/GUI は対象外。

## Upstream Documents

- `docs/specs/tim-04-plugin-state-snapshot-consistency.md`
- `docs/designs/tim-04-plugin-state-snapshot-consistency-design.md`

## Design Inputs

- `docs/designs/tim-04-plugin-state-snapshot-consistency-design.md`
- `docs/specs/vcs-01-immutable-snapshot-resolver.md`
- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`
- `docs/specs/tim03-process-boundary.md`
- Ruviform `HostCommand::SaveState`/`RestoreState`（#196）

## File Map

- `crates/synvelis-project/src/plugin_state/{mod,types,port,store,session}.rs` — 品質型・port・store・session
- `crates/synvelis-project/src/lib.rs` — additive export
- `crates/synvelis-project/tests/plugin_state.rs` — mock port/store tests
- `crates/synvelis-player/src/plugin_state.rs` — `PluginHost` → `PluginStatePort` adapter
- `crates/synvelis-player/src/{host,routing_engine,lib}.rs` — `state_handle` vend 経路
- `crates/synvelis-player/tests/tim04_plugin_state.rs` — adapter tests
- `crates/synvelis-render/src/{spec,preflight,job,lib}.rs` — `PluginStateRecipe` + gate + receipt
- `crates/synvelis-render/tests/tim04_offline_recipe.rs` — gate tests
- `docs/tasks/tim-04-plugin-state-snapshot.md`・`docs/test-specs/tim-04-plugin-state-snapshot.md`

## Implementation Sequence

- [ ] `plugin_state::types` — 品質 enum/record/manifest/receipt/summary
- [ ] `plugin_state::port` + `plugin_state::store` — trait + InMemory/Directory store
- [ ] `plugin_state::session` — save/restore orchestration + reconcile
- [ ] `synvelis-project` tests — mock port/store acceptance
- [ ] player adapter + engine vend + player tests
- [ ] render recipe + gate + receipt + render tests
- [ ] docs chain 完成 + impl record
- [ ] mise 検証 + origin/main merge

## Dependencies and Risks

- `PluginStatePort` は `&mut` で capture/restore —
  `PluginHost` adapter は `Rc<RefCell<..>>` 経由で
  non-RT のみ（RT callback から呼ばない契約）。
- manifest publish の `Indeterminate` は HEAD readback で
  reconcile — `ProjectStore` の effect 分類先例に倣う。
- `state_handle` の copy vend は generation counter で
  stale 拒否される既存機構に依存。

## Task Breakdown

- TS1: project plugin_state module + tests
- TS2: player adapter + vend + tests
- TS3: render recipe + gate + tests
- TS4: docs/impl record + verification

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test`
- [ ] `mise run test-rt` / `test-msrv`（存在するもの）
- [ ] app-native tasks（存在するもの）

## Open Questions / Gaps

- `PluginStateSession`（project）と player adapter の
  同名回避: player 側は `PluginStateHost`/`PluginStatePort`
  impl として命名を分ける。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, terminology consistent.

## Implementation Handoff

- Recommended mode: inline execution
- Independent work streams: none（直列: project→player→render）
- User approval required before dispatch: no（worker dispatch で実行）
- If approved: execute task-by-task with review checkpoints.

## Review Handoff

- reviewer focus: quality enum の序列/`exact_eligible` 定義、
  Strict fail-closed が store 未接触であること、indeterminate
  reconcile の HEAD readback、player adapter の非 RT 保証、
  render gate が evidence を偽らないこと。
