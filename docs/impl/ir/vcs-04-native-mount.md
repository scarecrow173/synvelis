---
id: "IMPL-VCS04NativeMount001"
type: "impl"
status: "completed"
title: "VCS-04 History/Visual Diff — native mount/dock 配線 + gpui-test (synvelis#95)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/common/vcs.rs"
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
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/vcs04_history.rs"
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
      - type: "file"
        path: "docs/impl/exp/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-04-native-mount-task.md"
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/designs/vcs-04-native-mount-design.md"
    - "docs/plans/vcs-04-native-mount-plan.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-04-native-mount-design.md"
    - "docs/impl/ir/vcs-04-history-visual-diff-ui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-04-native-mount.md"
    - "docs/test-specs/vcs-04-native-mount.md"
    - "docs/impl/exp/vcs-04-native-mount.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-04-native-mount-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-04-native-mount.md"
  verified-by: []
  references:
    - "crates/synvelis-app/src/screens.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "crates/synvelis-app/src/app.rs"
    - "crates/synvelis-app/src/shell.rs"
    - "crates/synvelis-app/src/bin/synvelis-native.rs"
    - "crates/synvelis-app/tests/vcs04_native_mount.rs"
    - "crates/synvelis-app/tests/vcs04_mount_gpui.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/vcs-04-native-mount.md"
    rejected: []
---

# VCS-04 History/Visual Diff — native mount/dock 配線 + gpui-test (synvelis#95)

## Summary

Issue #95 の残件 slice。PR #252 で統合済みの headless 基盤
（`VcsHistoryBackend`/`VcsHistoryAdapter`、workspace
`VcsHistoryViews`、app `syn.vcs.view`/`syn.vcs.visual` ops、
read-only `history_screen_spec`）を、製品 GPUI アプリの
`ViewHost`/`ViewMount`/`create_view_window` 経路へ実配線し、
dock-domain 解決を registry fallback で一般化した。
`syn.vcs.visual{open}` が mint する動的 `view-{n}` を dock-domain
view として attach/detach/reattach/close できるようにし、
`vcs_screen_cell` が view ごとの `VcsScreenState` を publish
して mounted spec が op・project 変更へ追従する。
`gpui-test`（`GpuiGuiTest`）scenario で mount→read-only→
A/B pin→ChangeSet diff→pagination→update→detach→2window
remount→close を headless 検証した。実 OS GUI・IME・pixel・
実 Plugin dock は #114 gate、VCS Domain Oracle は #97 で範囲外。

## Context

Implements `docs/tasks/vcs-04-native-mount-task.md` per
`docs/specs/vcs-04-native-mount.md` and
`docs/designs/vcs-04-native-mount-design.md`, planned by
`docs/plans/vcs-04-native-mount-plan.md`, verified by
`docs/test-specs/vcs-04-native-mount.md`（synvelis#95 の
native mount/dock slice。実 OS 受入は #114）。

前提: headless slice（PR #252、`docs/impl/ir/
vcs-04-history-visual-diff-ui.md`）で `syn.vcs.*` op 面と
`history_screen_spec` が完成済み。SV-GUI-04（#113）の
2-window/dock 基盤と PR #346 の同型 mount パターンを利用。

## Implementation

### Changes

- `crates/synvelis-app/src/screens.rs` — `VcsScreenState` 型、
  `history_screen_spec` の title `String` 化（registry
  descriptor が owned title を持つため）、`mount_screen_view`
  ヘルパー抽出、`mount_vcs_history_view` 追加
  （session-owned `AttachEpochAllocator` 経由・呼出側は epoch
  注入不可）。
- `crates/synvelis-app/src/ui_ops.rs` — `dock_domain_view`
  （静的表→registry descriptor fallback、`view-{n}` 未登録は
  fail closed）、`syn.view.attach` result に `dock_domain`、
  `syn.vcs.visual{open,close}` の `view-{n}` 単調 mint/解放
  （ID 再利用なし）。
- `crates/synvelis-app/src/app.rs` — `vcs_screen_cell()`、
  `publish_vcs_screens`（post-op tail で全 registered
  `ViewType::VcsHistory` を `vcs_history_view` で再投影、
  close 済み entry を除去）。
- `crates/synvelis-app/src/shell.rs` — dock-domain id
  `HashSet<String>`、attach/detach/close/destroy bookkeeping
  の pool vs legacy close replay 分岐、`WindowRequest` targets
  の動的 view 対応。
- `crates/synvelis-app/src/bin/synvelis-native.rs` —
  `SYNVELIS_VCS_HISTORY` env gate、`syn.vcs.visual{open}`
  発行、VCS mount spec bookkeeping、run loop の publish 差分
  `update_view`。
- `crates/synvelis-app/src/lib.rs` — export 調整。
- `crates/synvelis-app/tests/common/{mod,vcs}.rs` — 共有 fake
  provider/backend fixture を `vcs04_history.rs` から抽出、
  view_id パラメータ化 helper と authority snapshot helper 追加。
- `crates/synvelis-app/tests/vcs04_history.rs` — fixture 共有化
  （9 test 維持）。
- `crates/synvelis-app/tests/vcs04_native_mount.rs` — 新規 7
  test（dock-domain 解決・fail-closed・mount・publication・
  shell lifecycle・new-window saga・authority 不変）。
- `crates/synvelis-app/tests/vcs04_mount_gpui.rs` — 新規 3
  scenario（read-only mount・update/detach/2window remount・
  external gates Unsupported）。

### Decisions During Implementation

- dock-domain 解決を `ui_ops.rs` の `dock_domain_view` 1 箇所に
  集約（静的表→registry descriptor）。個別 op への registry
  照会の散在を避ける。
- `syn.view.attach` result に `dock_domain: bool` を載せ、
  shell は id set で追跡。shell が workspace registry を直接
  読まない driver-neutral 境界を維持。
- publish 点を `publish_project_length`（全 op post tail）に
  集約し、VCS op・project 変更の双方で追従させる。
- epoch を呼出側に渡させない `mount_vcs_history_view` 形状で
  fresh mint を強制（RF387 fresh-host 契約）。
- gpui-test は `GpuiGuiTest`+`SemanticGuiTest` port で構成し、
  実 OS/IME/pixel/plugin は `ScenarioVerdict::Unsupported`
  として記録（simulated pass を昇格させない）。

## Related Experiments

### Adopted

- `docs/impl/exp/vcs-04-native-mount.md` — dock-domain 一般化・
  publish 集約・epoch 強制・gpui-test composition・つまずき
  記録。

### Rejected

- なし（設計段階の代替案は design doc D1–D6 の却下欄に記録）。

## Validation

canonical = mise、OS: Windows、toolchain: mise pin。
全て実測 exit 0:

- `mise run fmt-check` — Pass
- `mise run check synvelis-app` — Pass
- `mise run lint synvelis-app` — Pass（warning 0）
- `mise run test synvelis-app` — Pass（全 suite。
  `vcs04_native_mount` 7/7・`vcs04_history` 9/9・
  `vcs04_mount_gpui` は default lane では 0 test
  （feature gate）・`view_mount` 27/27・two-window 8/8・
  `vcs06_ui_matrix` 4/4・`vcs_backend_wiring` 2/2 含む）
- `mise run check-app-native` — Pass
- `mise run lint-app-native` — Pass
- `mise run check-app-gpui` — Pass
- `mise run lint-app-gpui` — Pass
- `mise run test-app-gpui` — Pass（`vcs04_mount_gpui` 3/3）
- `mise run check/lint/test synvelis-workspace` — Pass
  （接続調整確認として実行、`vcs04_history` 13/13 含む）

`proc-macro-error2 v2.0.1` の future-incompat warning は依存由来で
本変更と無関係（warning のみ・失敗なし）。

## Risks

- `view-{n}` fail-closed が他 slice の動的 view を壊す可能性 —
  静的表+registry の 2 段解決で registry-adopted view は全て
  通る。registry 未登録の `view-{n}` を legacy path で使う
  既存経路があれば test で検出される（現行 suite では検出なし）。
- `publish_vcs_screens` の post-op tail 実行による再投影コスト —
  VCS History view 数は少数で `main_screen` publish と同頻度・
  同規模。
- `DockViewInfo.title` の `&'static str`→`String` 変更は全
  constructor 更新が必要 — compile で網羅済み。

## Follow-ups

- 実 OS GUI window・実 Git repo・2Window・IME・pixel・実 Plugin
  dock の受入 — Issue #114 gate（`Unsupported`/`NotRun` のまま）。
- VCS Domain Oracle（#97）との同一 Case 判定 — 範囲外。
- production `set_vcs_backend` の製品自動 bind — 後続 slice
  （本 slice は注入済み backend 前提。`vcs_backend_wiring` が
  `project_new` 経路の実 backend bind を別途検証済み）。
