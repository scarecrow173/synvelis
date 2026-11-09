---
id: "PLAN-034UqHfN1LJeYNaoS0SGYL"
type: "plan"
status: "approved"
title: "PC-C3 ControlTrace projection 実装計画"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c3-control-trace-projection.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-c3-control-trace-design.md"
    - "docs/adr/pc-c3-control-trace-stage-evidence-freshness.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/pc-c3-control-trace-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
  verified-by:
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C3 ControlTrace projection 実装計画 Implementation Plan

## Goal

`synvelis-project` に read-only の ControlTrace projection（`trace.rs`）を実装し、TRACE-C3-01..04 を fixture test で検証する。workspace/player/Ruviform は変更しない。

## Scope

- In: `src/trace.rs`、`src/lib.rs`（additive export）、`tests/trace_c3.rs`、docs チェーン。
- Out: TRACE-C3-05・workspace/player 統合・#101/#103/#114・実 plugin/OS/RT 観測・Cargo 依存変更。

## Upstream Documents

- spec: `docs/specs/pc-c3-control-trace-projection.md`
- adr: `docs/adr/pc-c3-control-trace-stage-evidence-freshness.md`
- design: `docs/designs/pc-c3-control-trace-design.md`

## Design Inputs

- stage slot evidence model・freshness 評価境界・probe 状態語彙・plan activation 投影（design doc が正本）。
- 先例: `relation.rs`（read-only revision-tagged projection）、`routing_build.rs`（generation-scoped key・fault taxonomy・last-good）。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-project/src/trace.rs` | 新規 — 全 projection 型 + freshness/is_coherent |
| `crates/synvelis-project/src/lib.rs` | `pub mod trace;` + re-export 追加のみ |
| `crates/synvelis-project/tests/trace_c3.rs` | 新規 — TRACE-C3-01..04 fixture test |
| `docs/impl/ir/pc-c3-control-trace-projection.md` | 実装記録（完了時） |

## Implementation Sequence

1. `trace.rs` 型定義（key/slot/evidence/freshness/plan activation）— design doc の型一覧どおり。
2. `lib.rs` export 追加。
3. `tests/trace_c3.rs` — test-spec の Feature ごとに fixture を構築。
4. `mise run fmt-check` / `check` / `lint` / `test`（canonical）。
5. impl record + docs index 再生成 + commit。

## Dependencies and Risks

- `ruviform-audio-plugin-bridge` 非依存 → `DispatchStage`/`DropReason` の semantic mirror を trace.rs 内に定義（design 記載済み）。
- `crate::id::ClockEpochId`（project epoch）と media-core `ClockEpochId`（runtime epoch）の衝突 → import alias。
- freshness の best-channel 集約は `observed_freshness()`/`plugin_reported_freshness()` で個別取得可能にして誤読を防ぐ。

## Task Breakdown

- T1: `trace.rs` 型 + freshness/is_coherent 実装
- T2: `lib.rs` export + `tests/trace_c3.rs`
- T3: 最終検証 + impl record + index 再生成

## Verification

- `mise run fmt-check` / `mise run check` / `mise run lint` / `mise run test` — 全 exit status を impl record に記録。
- test-spec `docs/test-specs/pc-c3-e2e-control-trace.md` の各 Feature が fixture で担保されること。

## Open Questions / Gaps

- なし（review-protocol で blocker なし確認済み）。

## Self-Review

- scope は spec の In Scope と一致。Cargo.toml/他 crate への変更なし。freshness の分類は design doc の決定どおり。

## Implementation Handoff

- task: `docs/tasks/pc-c3-task1-control-trace.md`

## Review Handoff

- review-protocol 実施済み（blocker なし）。
