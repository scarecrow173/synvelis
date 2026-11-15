---
id: "PLAN-034XI9qgQCTFctab81jyMS"
type: "plan"
status: "approved"
title: "tim-03-driver-loop-boundary"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/tim03_driver_loop_boundary.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-03-driver-loop-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-driver-loop-boundary.md"
  derived-by: []
  verifies:
    - "docs/test-specs/tim-03-driver-loop-boundary.md"
  verified-by: []
  references:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  defers: []
  deferred-by: []
---

# Plan — TIM-03 driver-side loop-wrap boundary derivation

## Scope

`crates/synvelis-player/src/driver.rs` のみ（`process_boundary.rs`
は既存 `pub(crate)` `WrapSim` API で足りるため無変更）、新規 test
`crates/synvelis-player/tests/tim03_driver_loop_boundary.rs`、本 doc
chain、`evidence/sv127/` 新規ファイル。routing 側・他 crate・
共有生成物（index は generator 再生成のみ）は範囲外。

## File map

| file | change |
| --- | --- |
| `src/driver.rs` | `AttachedDispatch` に `wrap_sim`/`sim_window`/`staged_transport`/`last_window_plan` 追加。`anchor_link` を `AttachedDispatch` method 化し `loop_wrap_boundaries` preview helper を追加。`stage_node_transport`/`stage_seek`/`observe_attached_block`/`resume_attached_transport` を更新。`attached_window_plan` accessor 追加。import に `BoundaryPoint`/`ProcessBoundaryPlan`/`WrapSim` 追加。 |
| `tests/tim03_driver_loop_boundary.rs` | 新規 — DB-01..DB-06 の 6 test（下記 test-spec 対応）。 |

## Dependency graph / order

1. `AttachedDispatch` field 追加 + `prepare_attached` 初期化
   （`WrapSim::new(frame_base)`、`sim_window = cursor`）。
2. `anchor_link`/`loop_wrap_boundaries`/`stage_node_transport`/
   `stage_seek` の boundary 供給 + `staged_transport` 記録。
3. `observe_attached_block` での 1 窓 1 回 advance。
4. test 追加 → `mise run test synvelis-player` で赤→緑確認。
5. doc chain + evidence + 全検証 lane。

## Risk register

| risk | mitigation |
| --- | --- |
| seek quiesce で node cursor と driver cursor が乖離 | `sim_window` を node cursor ドメインとして独立追跡し `set_clock` 成功時に即時更新。test DB-02/DB-05 が quiesce 経路を実測。 |
| preview と実 advance の二重計上 | preview は clone のみ、advance は `observe_attached_block` の 1 箇所に限定。 |
| `staged_transport` と実 queue の不整合 | legacy 経路も「queue に入ったものだけ」push（mirror の `note_*` と同じ規則）。queue full で落ちた command は sim にも入れない。 |
| offset 0 head boundary の扱い | oracle が applied set から除外する既存正規化に委ね、driver は `BoundaryPoint` をそのまま供給。test で `plan.boundaries` が interior offset のみを含むことを確認済み。 |

## Verification matrix

`mise run fmt-check` / `check synvelis-player` / `lint synvelis-player` /
`test synvelis-player` / `test-rt` / `check-headless-deps` /
`prek run --all-files`。OS-L1 (Windows) のみ — GUI-L2/EXT-L3/
実 plugin/consumer oracle は NotRun（別 gate）。
