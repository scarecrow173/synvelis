---
id: "PLAN-VCS04NativeMount001"
type: "plan"
status: "approved"
title: "VCS-04 History/Visual Diff — native mount/dock 配線 plan"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "DESIGN-VCS04NativeMount001"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/vcs-04-native-mount-design.md"
  implemented-by:
    - "docs/tasks/vcs-04-native-mount-task.md"
  depends-on:
    - "docs/designs/vcs-04-native-mount-design.md"
    - "docs/specs/vcs-04-native-mount.md"
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
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-native-mount.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 History/Visual Diff — native mount/dock 配線 plan

## Scope

`crates/synvelis-app` のみ（`src/screens.rs`・`src/ui_ops.rs`・
`src/app.rs`・`src/shell.rs`・`src/bin/synvelis-native.rs`・
`src/lib.rs` + `tests/`）と docs chain。`synvelis-workspace` は
読み取り接続調整が必要な場合のみ、書き込み側は変更しない。
他 crate・`Cargo.toml`・`mise.toml`・workflow は範囲外。

## 実装ステップ

1. **screens.rs** — `VcsScreenState` 型、`history_screen_spec`
   の title を `String` 化（registry descriptor が owned title
   を持つため）、`mount_screen_view` ヘルパー抽出、
   `mount_vcs_history_view` 追加。
2. **ui_ops.rs** — `dock_domain_view(id)`（静的表→registry
   descriptor fallback、fail closed）、`syn.view.attach` result
   に `dock_domain`、`syn.vcs.visual{open,close}` で `view-{n}`
   の単調 mint/解放（ID 再利用なし）。
3. **app.rs** — `vcs_screen_cell()`、`publish_vcs_screens`、
   post-op tail（`publish_project_length`）からの呼出。
4. **shell.rs** — dock-domain id `HashSet`、attach/detach/
   close/destroy の bookkeeping 分岐、`WindowRequest` targets
   の動的 view 対応。
5. **synvelis-native.rs** — `SYNVELIS_VCS_HISTORY` gate、
   `syn.vcs.visual{open}` 発行、VCS mount spec bookkeeping、
   run loop の `update_view` 差分追従。
6. **tests/common/vcs.rs** — 共有 fake provider/backend fixture
   を `vcs04_history.rs` から抽出し、view_id パラメータ化 helper
   と authority snapshot helper を追加。
7. **tests/vcs04_native_mount.rs** — dock-domain 解決・
   fail-closed・`ViewHost` mount・state publication・shell
   lifecycle・new-window dock saga・authority 不変の 7 test。
8. **tests/vcs04_mount_gpui.rs** — `GpuiGuiTest` mount・
   read-only・A/B/diff・pagination・update・detach/remount・
   close の 3 scenario + Unsupported 記録。

## 検証マトリクス

| レイヤ | コマンド | 期待 |
| --- | --- | --- |
| fmt | `mise run fmt-check` | exit 0 |
| app check | `mise run check synvelis-app` | exit 0 |
| app lint | `mise run lint synvelis-app` | exit 0・warning 0 |
| app test | `mise run test synvelis-app` | exit 0・全 lane pass |
| native check | `mise run check-app-native` | exit 0 |
| native lint | `mise run lint-app-native` | exit 0 |
| workspace | `mise run check/lint/test synvelis-workspace` | 変更時のみ・exit 0 |
| 実 OS GUI | — | Unsupported/NotRun（#114） |

## リスク

- `view-{n}` fail-closed が既存の動的 view（他 slice で同パターンを
  使うもの）を壊さないか — 静的表 + registry の 2 段解決で
  registry-adopted view は全て通る。registry 未登録の `view-{n}`
  を legacy path で使う既存テストがあれば検出される。
- `publish_vcs_screens` が post-op tail で走るため高頻度 op で
  再投影コストが増える — view 数は少数（VCS History のみ）で、
  既存 `main_screen` publish と同じ頻度・同じ規模。
- `DockViewInfo.title` の `&'static str`→`String` 変更は
  呼出側の全 constructor を更新する必要あり — compile で網羅。
