---
id: "TASK-VCS04NativeMount001"
type: "task"
status: "done"
title: "VCS-04 native mount/dock 配線 + gpui-test slice"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/common/vcs.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/vcs04_history.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/vcs04_native_mount.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/vcs04_mount_gpui.rs"
      - type: "file"
        path: "docs/specs/vcs-04-native-mount.md"
      - type: "file"
        path: "docs/designs/vcs-04-native-mount-design.md"
      - type: "file"
        path: "docs/plans/vcs-04-native-mount-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-04-native-mount-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-04-native-mount.md"
      - type: "file"
        path: "docs/impl/ir/vcs-04-native-mount.md"
      - type: "file"
        path: "docs/impl/exp/vcs-04-native-mount.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-04-native-mount-plan.md"
  implemented-by:
    - "docs/impl/ir/vcs-04-native-mount.md"
  depends-on:
    - "docs/plans/vcs-04-native-mount-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/designs/vcs-04-native-mount-design.md"
    - "docs/test-specs/vcs-04-native-mount.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-native-mount.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 native mount/dock 配線 + gpui-test slice タスク

## Work

Issue #95 の残件。headless slice（PR #252）で完成した VCS History
projection/op 面を native `ViewHost`/`ViewMount`/`create_view_window`
経路へ配線し、`gpui-test` scenario で mount→A/B→diff→close の
headless 検証を行う。実 OS GUI・IME・pixel・plugin dock は #114、
Domain Oracle は #97 で範囲外。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/vcs-04-native-mount.md`
- `docs/designs/vcs-04-native-mount-design.md`
- `docs/plans/vcs-04-native-mount-plan.md`
- `docs/test-specs/vcs-04-native-mount.md`

## Dependencies

- headless slice（PR #252）: `VcsHistoryBackend`/`VcsHistoryAdapter`、
  workspace `VcsHistoryViews`、app `syn.vcs.*` ops、
  `history_screen_spec`。
- SV-GUI-04（#113）2-window/dock 基盤、SV-GUI-03-05 gpui scenario
  harness、PR #346 の同型 mount パターン。
- ブランチ `feat/95-vcs-native-mount`（worktree
  `scarecrow173/sv95-vcs-mount`）。

## Affected Files

- `crates/synvelis-app/src/{screens,ui_ops,app,shell}.rs`、
  `src/bin/synvelis-native.rs`、`src/lib.rs`
- `crates/synvelis-app/tests/common/{mod,vcs}.rs`、
  `tests/vcs04_history.rs`、`tests/vcs04_native_mount.rs`（新規）、
  `tests/vcs04_mount_gpui.rs`（新規）
- `docs/` チェーン一式

## Done When

- [x] `syn.vcs.visual{open}` の動的 `view-{n}` が dock-domain view
  として attach/detach/reattach/close でき、registry descriptor が
  kind/title を供給する（`vcs04_native_mount.rs` 7 test で固定）
- [x] `view-{n}` で静的表にも registry にも無い id が fail closed
  される（同上・`view_shaped_id_without_registration_is_refused`）
- [x] `vcs_screen_cell` が view ごとの `VcsScreenState` を publish
  し、op 後・project 変更後に追従、close で除去される
  （`published_state_tracks_ops_and_drops_closed_views`）
- [x] `mount_vcs_history_view` が session-owned epoch で
  `ViewHost` を構築し read-only spec を mount する
  （`vcs_screen_cell_publishes_and_mount_vcs_history_view_builds_read_only_host`）
- [x] `NativeShell`+`FakeDriver` で open→attach→window destroy
  （pool）→close、new-window dock saga が動的 view で完走する
  （`native_shell_*` 2 test）
- [x] `GpuiGuiTest` scenario が mount→全 component resolve→
  read-only（select/invoke 拒否・intent 0）→update→detach stale→
  2 window remount→close を検証し project authority 不変
  （`vcs04_mount_gpui.rs` 3 test）
- [x] `Unsupported`/`Partial`/`Stale`/`Unknown`/`no-backend` が
  mounted spec で区別表示され equal/empty に潰れない
- [x] 実 OS GUI/IME/pixel/plugin dock は `Unsupported`/`NotRun`
  として記録（`gpui_vcs_history_external_gates_report_unsupported`）

## Verification

- [x] `mise run fmt-check` — Pass（exit 0）
- [x] `mise run check synvelis-app` — Pass（exit 0）
- [x] `mise run lint synvelis-app` — Pass（exit 0）
- [x] `mise run test synvelis-app` — Pass（exit 0・gpui-test lane
  含む全 suite。`vcs04_history` 9/9・`vcs04_native_mount` 7/7・
  `vcs04_mount_gpui` 3/3・`view_mount` 27/27・two-window 8/8・
  `vcs06_ui_matrix` 4/4・`vcs_backend_wiring` 2/2 を含む）
- [x] `mise run check-app-native`/`lint-app-native` — Pass（exit 0）
- [x] `mise run check/lint/test synvelis-workspace` — Pass（exit 0、
  接続調整の確認として実行）
- [ ] 実 OS/GPUI window・実 Git・IME・pixel は NotRun（#114 gate）

## Sources and Relations

- [spec](../specs/vcs-04-native-mount.md) /
  [design](../designs/vcs-04-native-mount-design.md) /
  [plan](../plans/vcs-04-native-mount-plan.md) /
  [test-spec](../test-specs/vcs-04-native-mount.md)
