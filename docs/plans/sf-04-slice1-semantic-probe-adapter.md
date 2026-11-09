---
id: "PLAN-034Um6RlcTKbVmfUr7A4yJ"
type: "plan"
status: "approved"
title: "SF-04 slice1 実装計画 — project/player Semantic Probe Adapter + ruviform pin bump"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice1 実装計画 — project/player Semantic Probe Adapter + ruviform pin bump Implementation Plan

## Goal

Issue #107 の headless 基盤として、semantic probe point の project 契約・
runtime 解決・binding lifecycle・capture segment cut を project/player 層に
実装し、ruviform pin を #308/#309 を含む `0f5b5737` へ更新する。

## Scope

Single implementation stream — `synvelis-project`（契約+解決）と
`synvelis-player`（runtime adapter+engine 配線）は同一 dispatch の 1 slice。
app/workspace/UI/実測は後続 slice。

## Upstream Documents

- docs/specs/sf-04-slice1-semantic-probe-adapter.md

## Design Inputs

- docs/designs/sf-04-slice1-semantic-probe-adapter-design.md

## File Map

- `crates/synvelis-project/src/probe.rs` - semantic contract 型 + `resolve_probe_point`
- `crates/synvelis-project/src/lib.rs` - probe module export
- `crates/synvelis-player/src/probe.rs` - `ProbeSession`（broker/collector/ledger/segment）
- `crates/synvelis-player/src/routing_engine.rs` - probe deck install・publish rebind・public passthrough
- `crates/synvelis-player/src/lib.rs` - probe module export
- `crates/synvelis-project/tests/sf04_project_probe.rs` - 解決規則・fail-closed テスト
- `crates/synvelis-player/tests/sf04_probe_adapter.rs` - lifecycle・stale・segment cut テスト
- `Cargo.lock` - ruviform pin `0f5b5737`（commit `10a1bea` 済み）

## Implementation Sequence

- [x] ruviform pin bump（`cc7d5296` → `0f5b5737`、lockfile のみ、API 破壊なし）
- [ ] `synvelis-project/src/probe.rs` 契約型 + `resolve_probe_point` + lib export
- [ ] `synvelis-player/src/probe.rs` `ProbeSession` + lib export
- [ ] `routing_engine.rs` probe 配線（deck install・publish on_publish・passthrough）
- [ ] `sf04_project_probe.rs` / `sf04_probe_adapter.rs` headless テスト
- [ ] impl record + 全検証 green

## Dependencies and Risks

- ruviform `ProbeBroker::install`/`on_publish` は published plan に対し物理
  slot を再解決する — semantic 解決（SourceMap）と物理解決（plan）の二段構えを
  混同しない。
- `on_publish` の `CommandChannelFull` は bindings を旧 generation に残す
  （RT が `Unbound` を出す）— publish 自体は失敗させない。
- `Input`/`PreInsert`・`Output`/`PostFader` は現 lowering で同一点 — design
  の規則表どおり実装し、暗黙の別扱いをしない。

## Task Breakdown

- docs/tasks/sf-04-slice1-semantic-probe-adapter.md（T1 = 本 slice 全体）

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint`
- [ ] `mise run test` / `mise run test-rt`
- [ ] `mise run check-ruviform-rev`（pin 更新済み green）
- [ ] `sf04_*.rs` が解決成功/Unavailable/Unsupported・stale reject・
  segment cut を検証

## Open Questions / Gaps

- なし（境界規則・lifecycle・segment 意味論は design で確定済み）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution
- Independent work streams: none
- User approval required before dispatch: no（dispatch 済み worker task）
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- 境界規則表（design）と実装の一致、`ProbeResolveState` 遷移の
  requested/queued/installed 区別、segment cut が RT status 駆動で
  あることを重点確認。
