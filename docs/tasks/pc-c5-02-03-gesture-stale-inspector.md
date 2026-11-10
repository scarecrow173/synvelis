---
id: "TASK-034UxYCkPWXOotRSbjIPex"
type: "task"
status: "done"
title: "PC-C5 slice2 タスク分解 — Gesture 固定・stale drop・Inspector mode（UI-C5-02/03・headless）"
created: "2026-11-09"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c5-02-03-gesture-stale-inspector.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c5-02-03-gesture-stale-inspector.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/designs/pc-c5-02-03-gesture-stale-inspector-design.md"
    - "docs/test-specs/pc-c5-02-03-gesture-stale-inspector.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c5-02-03-gesture-stale-inspector.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice2 タスク

| # | タスク | 完了条件 |
| --- | --- | --- |
| 1 | `state.rs` Inspector/gesture state 拡張 | `InspectionMode::Explore`・`FollowSource`・`InspectionFocus.follow_source`/`explore_back`/`explore_fwd`・`ViewState.control_edge_gesture` が型付きで存在し、session-only field は serde skip・preset 非永続化を維持 |
| 2 | `control_edge.rs` + workspace control edge API | Begin が view・typed endpoint・`ProjectRevision`・window epoch を固定。Preview/Cancel は project 不変、Commit は fixated revision で `AddModulationAssignment` を 1 回だけ適用し、window epoch/revision stale は `Stale` で drop（retarget しない） |
| 3 | workspace inspector API + `link.rs` deliver 照合 | `inspect`/`set_inspection_mode`/`set_follow_source`/`explore_back`/`explore_forward` が editing selection・project・undo に触れない。deliver は Follow+follow_source 一致時のみ target を更新し、Explore/Pinned は不変 |
| 4 | `preset.rs`/`lib.rs` 追従 | preset 再構築が新 field に追従し、module・型が re-export され compile する |
| 5 | `tests/uic5.rs` fixture | UI-C5-02/03 の rule を外部観測可能な挙動で証明する 12 fixture が pass |
| 6 | canonical 検証 | `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` と `check-app-native`/`lint-app-native`/`build-app-native` が pass |
| 7 | docs + impl record + PR | 文書チェーン完備、index 再生成、impl record、`feature/102-uic5-gesture-stale` push・PR（`Refs #102`） |

## Sources and Relations

- [spec](../specs/pc-c5-02-03-gesture-stale-inspector.md) /
  [design](../designs/pc-c5-02-03-gesture-stale-inspector-design.md) /
  [plan](../plans/pc-c5-02-03-gesture-stale-inspector.md) /
  [test-spec](../test-specs/pc-c5-02-03-gesture-stale-inspector.md)
