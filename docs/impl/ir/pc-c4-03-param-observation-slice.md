---
id: "IMPL-PCc4030bs3vN1pQ8wXyZ2aG"
type: "impl"
status: "completed"
title: "PC-C4-03 param observation — host drain → ParamDispatch ledger"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-03-param-observation.md"
      - type: "file"
        path: "docs/designs/pc-c4-03-param-observation-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-03-param-observation-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c4-03-task1-param-observation.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-03-param-observation.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-03-param-observation-slice.md"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c4_03_observation.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/routing_control.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/pc-c4-03-task1-param-observation.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/pc-c4-03-task1-param-observation.md"
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

# PC-C4-03 impl record — param observation slice

## 実装内容

- `RoutingControlDispatch::drain_output_reports(endpoints, generation,
  active_generation, drain) -> ReportDrainReport`
  - generation/active gate（endpoint 非接触で refuse）。
  - endpoint→`NodeSurface` 解決、未解決は `unrouted`。
  - `Drained`: `EventKind::ParamValue` を `session.bindings()` で
    `ParamId → synvelis_target` 逆引き → `record_reported`。
    `Unavailable`/`InvalidHandle`/`Failed` → `mark_reporting_unavailable`。
    その他 `HostError`/未知 variant → `failed`+`errors`。
- `RoutingControlDispatch::observe_cycle(processor, generation, span,
  call_start, call_frames) -> Option<DispatchCycle>` — node scheduler
  report channel の generation-scoped drain owner。
- `ReportDrainReport` — `attached`/`generation`/`generation_gated`/
  `inactive_gated`/`drained`/`unavailable`/`failed`/`events`/
  `param_reports`/`reported`/`unbound_param`/`no_pending`/`unrouted`/
  `errors`。
- `lib.rs`: `ReportDrainReport` re-export。

## 設計判断

- drain seam を `impl FnMut(PluginHandle, &mut Vec<Event>) ->
  Result<ReportDrain, HostError>` の注入にした — `PluginHost` 型への
  直接依存を排し、`Unavailable` 分岐を L1 worker なしで検証可能に
  した（実 L1 は pin 未対応で `NotRun`）。
- `InvalidHandle`/`Failed` を terminal（`Unavailable`）に分類 —
  instance 消失/収束で report channel が存在しないことが確定的なため。
- `Event.frame_offset` は ledger に渡さない — `record_reported` の
  API が frame を取らず、最新 eligible entry 意味論が upstream の
  owning abstraction。

## 検証結果

- `cargo test -p synvelis-player --test pc_c4_03_observation`:
  8/8 Pass（補助証拠、canonical は `mise run test`）。
- `cargo clippy -p synvelis-player --all-targets`: warning なし。

## 残件

- L1 slot 実値検証: pin bump（RF#253 PR #564 取込）後に実施。
- app/workspace 層の呼び出し配線: pc-c4-04。
