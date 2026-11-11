---
id: "TASK-7c4e9a1b2d3f4e5a6b7c8f0a2b3"
type: "task"
status: "done"
title: "PC-C4-06 task1 — syn.control.* control op surface 実装"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c4-06-control-op-surface-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-06-control-op-surface-plan.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-06-control-op-surface-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-06-control-op-surface.md"
  verified-by:
    - "docs/test-specs/pc-c4-06-control-op-surface.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4-06 task1 — syn.control.* control op surface 実装

## Work

- T1 — `crates/synvelis-app/Cargo.toml`: `ruviform-plugin-core` を
  `[dependencies]` へ追加（Coordinator 承認済み）、dev-dep
  重複除去。
- T2 — `crates/synvelis-app/src/engine.rs`: `control_frame_base`・
  `control_last_block` read-only accessor 追加。
- T3 — `crates/synvelis-app/src/ui_ops.rs`: `UiOpsState.
  last_control_drain` 追加、wire projection helper
  （`time_base_json`/`media_time_json`/`surface_report_json`/
  `tick_report_json`/`report_drain_json`/`fast_control_block_json`/
  `fast_control_outcome_json`/`control_observation_json`/
  `applied_receipt_json`）と parse helper（`write_means_param`/
  `i64_param`/`ObserveParams` DTO 群/`transport_span_param`）、
  `op_control_fast_control`/`op_control_status`/
  `op_control_observe`/`note_control_drain` 実装。
- T4 — `crates/synvelis-app/src/service.rs`: `OPS` に
  `syn.control.fast_control`/`syn.control.status`/
  `syn.control.observe` を登録し `dispatch` に 3 route 追加。
- T5 — `crates/synvelis-app/tests/pc_c4_06_control_op_surface.rs`
  （新規）: `SharedService` wrapper + `UiRuntime::dispatch_op`
  で surface 無し fail-closed・typed outcome/status・observe
  stage 選別 + Reported feed・generation gate の 4 test。

## Classification

- feature（service op surface の新規公開 — bug-fix ではない）

## Done When

- [x] 3 op が `OPS`/`dispatch` に登録され headless
  `AppService` fixture で駆動できる
- [x] `fast_control` が `FastControlOutcome` を verbatim に返し、
  typed refusal が `blocked` + kind + block_detail を返す
- [x] `status` が surface/tick/drain/generation/frame_base の
  typed evidence を返し、未装着・drain 未実施を正直に示す
- [x] `observe` が caller span で ledger を進め、`Reported` のみが
  `fed` に計上され workspace plugin-report channel に到達する
- [x] stale generation・未知 processor・malformed params が
  typed error で fail-closed
- [x] canonical verification が全て実走で green

## Verification

```sh
mise run fmt-check && mise run check && mise run lint && \
mise run test && mise run test-rt && mise run check-app-native && \
mise run lint-app-native && mise run test-msrv && mise run build
```

- [x] `mise run test` exits 0（新規 fixture 4 test + 既存全 test green）
- [x] `mise run check` exits 0
- [x] `mise run lint` exits 0
- [x] `mise run fmt-check` exits 0
- [x] `mise run test-rt` exits 0
- [x] `mise run check-app-native` exits 0
- [x] `mise run lint-app-native` exits 0
- [x] `mise run test-msrv` exits 0
- [x] `mise run build` exits 0

## Notes

- `ParamWriteMeans` blocker: `FastControlWrite.means` は
  `ruviform-plugin-core` の型で、従来 dev-dep のみ・re-export
  経路無しで `synvelis-app` lib から名指し不能だった。
  Coordinator 承認（ask 回答）により `[dependencies]` へ追加。
  player src 変更は不要だった。
- `observe` の `plan_generation`/`transport_generation` は caller
  証拠 — receipts を構造化 JSON で返し、driver の
  `TransportMirror` と同じ学習経路を提供。op は surface
  generation の一致のみを gate する。
- `observe`/`fast_control` 末尾の `ui_pump` は automation dispatch
  で plan generation を進める — fixture test は receipts の
  `plan_generation` で適用済み世代を確認する。
- `drain_output_reports`・`observe_cycle` の RT 内 call-window
  自動供給・production composition root は本 slice 範囲外
  （後続 slice、NotRun 記録）。
