---
id: "TSPEC-VCS04NativeMount001"
type: "test-spec"
status: "approved"
title: "VCS-04 native mount/dock fixtures"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "SPEC-VCS04NativeMount001"
    - "DESIGN-VCS04NativeMount001"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/designs/vcs-04-native-mount-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/designs/vcs-04-native-mount-design.md"
  derived-by: []
  verifies:
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/designs/vcs-04-native-mount-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 native mount/dock fixtures

Spec `SPEC-VCS04NativeMount001` / Design
`DESIGN-VCS04NativeMount001` を検証する fixture test 仕様。
実装は `crates/synvelis-app/tests/vcs04_native_mount.rs`（headless
`NativeShell`+`FakeDriver` lane、7 test）と
`crates/synvelis-app/tests/vcs04_mount_gpui.rs`
（`GpuiGuiTest` lane、3 scenario）。fixture は
`tests/common/vcs.rs` の共有 fake provider/backend を使う
（`vcs04_history.rs` 9 test と同一データ源）。

## Feature: 動的 VCS view の dock-domain 解決

- **Rule**: `syn.vcs.visual{open}` が mint する `view-{n}` は
  `syn.view.attach` で dock-domain view として受理され、
  result の `dock_domain` が true を返す。
- **Rule**: `view-{n}` 形で静的表にも registry descriptor にも
  無い id は `syn.view.attach` が fail closed で拒否する
  （legacy UI-only attach へ落ちない）。
- **Example**: `dynamically_opened_vcs_view_is_a_dock_domain_view`、
  `view_shaped_id_without_registration_is_refused_not_legacy_attached`。

## Feature: screen-state publication

- **Rule**: `vcs_screen_cell` は登録済み `ViewType::VcsHistory`
  ごとに 1 `VcsScreenState` を publish し、VCS op・project 変更の
  双方の post-op tail で最新投影へ追従する。close された view の
  entry は次回 publish で落ちる。
- **Example**: `published_state_tracks_ops_and_drops_closed_views`。

## Feature: mount + read-only

- **Rule**: `mount_vcs_history_view` は session-owned
  `AttachEpochAllocator` から mint した epoch で `ViewHost` を
  構築し、`history_screen_spec`（status/selection/diff label +
  history/changes/issues の 3 read-only table、intent 0、
  selection なし）を mount する。呼出側は epoch を注入できない。
- **Example**: `vcs_screen_cell_publishes_and_mount_vcs_history_view_builds_read_only_host`、
  `gpui_vcs_history_mount_is_a_read_only_projection`。

## Feature: lifecycle（NativeShell + FakeDriver）

- **Rule**: open→attach→window destroy（pool、close ではない）→
  close が成立し、detach 後の reattach・new-window dock saga
  （`dock_prepare_new_window`→`WindowRequest`→saga Ready attach
  +commit）が動的 view で完走する。
- **Example**: `native_shell_mounts_pools_and_closes_dynamically_opened_vcs_view`、
  `native_shell_runs_new_window_dock_saga_for_vcs_view`。

## Feature: GPUI scenario

- **Rule**: `GpuiGuiTest` で `create_view_window` mount →
  全 component resolve → select/invoke 拒否（read-only）→
  `syn.vcs.visual` op による A/B pin・ChangeSet diff・load_more
  pagination が publish→`update_view` で追従 → detach stale →
  別 window で remount → close まで検証する。
- **Rule**: `Unknown`/`Partial`/`Stale`/`no-backend`/`failed`/
  `cancelled`/`empty` は mounted spec 上で区別された label/table
  として残り、equal/0-changes に潰れない。
- **Rule**: view navigation/attach/detach/close・pin/compare/
  refresh は project revision・undo/redo・transport・engine graph
  build・Git index/worktree・commit state を不変に保つ。
- **Example**: `gpui_vcs_history_update_detach_and_second_window_remount`、
  `gpui_vcs_history_external_gates_report_unsupported`。

## Feature: 外部 gate の正直な記録

- **Rule**: 実 OS GUI・2Window・IME・pixel・実 Plugin dock・
  VCS Domain Oracle 同一 Case 判定は `Unsupported`/`NotRun` と
  して `ScenarioVerdict::Unsupported` で記録し、simulated pass を
  昇格させない。
- **Example**: `gpui_vcs_history_external_gates_report_unsupported`。
