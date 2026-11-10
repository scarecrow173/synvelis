---
id: "PLAN-034UyECBCu7Wg67fct0qi7"
type: "plan"
status: "approved"
title: "TIM-03 plugin process boundary plan"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim03-process-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/tim03-task1-process-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim03-process-boundary.md"
  references:
    - "docs/tasks/tim03-task1-process-boundary.md"
  defers: []
  deferred-by: []
---

# TIM-03 plugin process boundary plan Implementation Plan

## Goal

`synvelis-player` に consumer 側 process boundary/capability/degraded
policy oracle（`process_boundary.rs`）を実装し、headless mock で
100/128/256f window・boundary・min/max・consent・offline・
generation・RT no-alloc を検証する。

## Scope

単一実装 stream。`crates/synvelis-player/src/` + 同 crate tests +
`docs/` chain のみ。Ruviform・他 crate・CI 設定は不変。

## Upstream Documents

- `docs/specs/tim03-process-boundary.md`
- `docs/tasks/tim03-task1-process-boundary.md`
- `docs/test-specs/tim03-process-boundary.md`

## Design Inputs

- `docs/designs/tim03-process-boundary-design.md`
- `docs/specs/pc-c4-plugin-param-binding.md`（PR #221 merged の
  consumer 基盤）

## File Map

- `crates/synvelis-player/src/process_boundary.rs` — policy oracle
  型群と `ProcessCapability::evaluate`。
- `crates/synvelis-player/src/lib.rs` — module export。
- `crates/synvelis-player/tests/tim03_process_boundary.rs` —
  evaluator + scheduler + PluginNode 3 層 fixture test。
- `crates/synvelis-player/tests/tim03_process_boundary_rt.rs` —
  `stats_alloc` RT hot path 証拠。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/tim03-*` —
  doc chain。

## Implementation Sequence

- [x] Ruviform API 照合（`BlockContract`/`legal_span_len`/
  `partition`、`TransportScheduler::find_legal_boundary`/
  `resolve_illegal_boundary`/`emit`/`apply`、`event_engine_frame`/
  `span_event_window`、`PluginNode`/`attach`/`NodeReport`）。
- [x] `process_boundary.rs` 実装（型 + evaluator + deferral scan）。
- [x] `lib.rs` export。
- [x] `tim03_process_boundary.rs` 3 層 test。
- [x] `tim03_process_boundary_rt.rs` RT 証拠。
- [x] doc chain 作成。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-headless-deps`。
- [ ] `git fetch origin main && git merge origin/main` 後の再検証。
- [ ] review-protocol self-review → commit → push → PR。

## Dependencies and Risks

- Ruviform pin `0f5b573` の semantics 依存: deferral `next_bound`
  は pending non-wrap apply のみ、wrap は position-driven。
  oracle がこれを正確に mirror すること（design 記載・test 固定）。
- `BlockContract::legal_span_len` は `max_frames==0` で除算 panic
  し得る → oracle が degenerate contract を `Unsupported` で
  事前拒否。
- upstream の degraded wrap は undeliverable span + wrap 継続だが、
  oracle は fail-closed（`DeferralExceeded`）を選択 — 設計判断
  として design/spec に明記済み。

## Task Breakdown

- `docs/tasks/tim03-task1-process-boundary.md` — 全実装を単一
  task で実施（小規模 slice のため分割しない）。

## Verification

- [x] `cargo test -p synvelis-player --test tim03_process_boundary`
  — 補助証拠（canonical = mise）。17 tests pass。
- [x] `cargo test -p synvelis-player --test tim03_process_boundary_rt`
  — 補助証拠。`rt_process_boundary_hot_path_alloc_free` pass。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv` / `check-headless-deps` — canonical。

## Open Questions / Gaps

- oracle の composition root 配線（plan → `DeliveryPolicy`/
  `ScheduleBudget`/consent の attach 反映）は後続 slice。
- `TimingPrecision` certified class の認定 provenance は未定義
  （mock では constructor 注入）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（完了済み）。
- Independent work streams: none。
- User approval required before dispatch: review-protocol で代替
  （blocker なしを承認とみなす）。

## Review Handoff

- upstream semantics mirror の正しさ（deferral `next_bound`・
  coalescing・wrap collapse）は test で固定済み。reviewer は
  `process_boundary.rs` の `degraded_plan`/`find_legal_boundary`
  と upstream `transport_schedule.rs` の対応箇所を照合されたい。
