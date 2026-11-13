---
id: "SPEC-npBYC7xiQlFDeYeTKsJdUk"
type: "spec"
status: "proposed"
title: "Issue #377 synvelis-native / gpui-test Feature の 3OS CI OS-L1 受入補完"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/377"
    - "docs/specs/issue-166-full-ci-ci-summary.md"
    - "docs/specs/issue-151-ui-ci-lane.md"
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  changes:
    added:
      - type: "file"
        path: "docs/specs/ci-os-l1-native-gpui.md"
      - type: "file"
        path: "docs/designs/ci-os-l1-native-gpui.md"
      - type: "file"
        path: "docs/plans/ci-os-l1-native-gpui.md"
      - type: "file"
        path: "docs/tasks/ci-os-l1-native-gpui.md"
      - type: "file"
        path: "docs/test-specs/ci-os-l1-native-gpui.md"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/ci-os-l1-native-gpui.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
    - "docs/specs/issue-151-ui-ci-lane.md"
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/ci-os-l1-native-gpui.md"
    - "docs/plans/ci-os-l1-native-gpui.md"
    - "docs/test-specs/ci-os-l1-native-gpui.md"
    - "docs/tasks/ci-os-l1-native-gpui.md"
  verifies: []
  verified-by:
    - "docs/test-specs/ci-os-l1-native-gpui.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-summary.mjs"
    - "crates/synvelis-app/Cargo.toml"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/232"
  deferred-by: []
---

# Issue #377 synvelis-native / gpui-test Feature の 3OS CI OS-L1 受入補完 仕様

## What Should Be Built

`synvelis-app` の非既定 Feature lane を既存 Daily Full CI(`ci.yml`、
workflow_dispatch 専用)へ接続し、POLICIES.md の OS-L1 検証を 3 OS
(Windows/macOS/Linux)で実走可能にする。

- `native` feature:`synvelis-app --features native` の check/Clippy と
  `synvelis-native` bin の Build/Link。
- `gpui-test` feature(headless GPUI-simulated scenario/contract lane):
  check/Clippy/test。0 件・未選択の silent pass を fail-closed で
  防ぐ。

## Why Now

- 日常のローカル検証を変更範囲へ比例化する運用(POLICIES.md §5、
  PR #376)へ移行したため、非既定 Feature の OS-L1 証拠は Daily
  Full CI が担う必要がある。現行 CI は標準 Workspace・GW-10・RT・
  MSRV・SolidJS UI を検証するが、`native`/`gpui-test` は mise task
  (`check-app-native` 等)が存在するだけで CI 未接続。
- `synvelis-native` bin は `native` feature 必須、`gpui-test` は
  `native` を内包する(`crates/synvelis-app/Cargo.toml`)。bin の
  3 OS link と GPUI-simulated scenario の 3 OS 実走は OS-L1 の
  Required であり、現状は `NotRun`。

## Scope

### In Scope

- `ci.yml` の既存 `verify`(ubuntu-latest + windows-latest matrix)
  と `verify-macos` job へ feature lane step を追加し、3 OS で
  `mise run check-app-native`/`lint-app-native`/`build-app-native`/
  `check-app-gpui`/`lint-app-gpui`/`test-app-gpui` 相当を実走
  させる。
- `gpui-test` の 0 件/未選択 silent pass を防ぐ non-empty guard
  (`rt_` guard と同型、scenario_gpui test target の `-- --list`
  非空確認)。
- Linux leg の GPUI link 依存(libxkbcommon 系)の明示 install。
- `verify`/`verify-macos` の timeout 見直し(compile 対象増加分)。
- lane evidence 表への新規 step outcome 追加。
- `evidence`(pr 互換)/`ci-summary`(full)の既存 fail-closed 集約を
  維持 — leg/job 追加は行わず、step 失敗は leg 失敗として既存
  gate が拒否する。

### Out of Scope

- 新規 job/leg の追加と `tools/ci-summary.mjs` の leg universe
  変更(不要な CI 重複を増やさない方針)。
- `webview` feature lane・`real-device`/`real-fs` opt-in lane の
  CI 接続(別 Issue・別受入)。
- 実 Window/実 OS 入力/pixel/実機 Plugin の GUI-L2/EXT-L3
  (synvelis#114、Windows 実機受入へ委譲。macOS/Linux は Deferred)。
- `crates/` 以下の実装コード、`Cargo.toml`/`Cargo.lock`、
  `mise.ci.toml`、他 workflow(daily-ci.yml・ci-issue-tracker*)。
- PR 必須 Full CI の導入、pull_request/push trigger の変更。

## Requirements

- 3 OS の既存 verify leg で `native` check/Clippy と
  `synvelis-native` bin Build/Link が実走すること。
- 3 OS で `gpui-test` の check/Clippy/test が実走し、scenario
  test が非空であることが guard で保証されること。
- ruviform resolved SHA guard・UI/SolidJS・GW-10・RT・MSRV・
  dep-guard の既存 lane と証拠を維持すること。
- leg 単位の fail-closed 集約を変更せず、OS 別 NotRun/Skipped/
  Fail を Pass 扱いしないこと。
- Linux の GPUI SDK 依存を明示し、runner image 同梱状況への
  暗黙依存を避けること。
- 追加 compile 時間を計測可能な形で記録し、verify timeout が
  実走時間を下回らないこと。

## Acceptance Criteria

- [ ] Windows/macOS/Linux の既存 Full CI で
  `synvelis-app --features native` の check/Clippy と
  `synvelis-native` bin の Build/Link が実走する。
- [ ] `gpui-test` 有効時の headless scenario/contract テスト・
  Feature 検査が 3 OS で実走し、0 件/未選択の silent pass を
  防ぐ。
- [ ] `ruviform` resolved SHA guard、UI/SolidJS、GW-10、RT、
  MSRV の既存 lane と証拠を維持する。
- [ ] `evidence`(互換)および `ci-summary`(Full)で追加検証を
  fail-closed 集約し、OS 別 NotRun/Skipped/Fail を Pass 扱い
  しない。
- [ ] CI の既存 Runner 制約、Linux GPUI SDK 依存、3 OS
  build/link の追加時間を計測し、不要な CI 重複を増やさない。
- [ ] 対象 SHA の Full CI を手動実走し、3 OS の OS-L1 検証成功を
  Run URL と共に残す。

## Deferred Design Concerns

- `synvelis-native` 実窓の mount/close・実 OS 入力・pixel 証拠は
  GUI-L2/EXT-L3 であり synvelis#114 の実機受入へ委譲する
  (`run-app-native` は手動・実機検証入口として残す)。
- `webview` feature の CI lane 化は本 Issue の対象外
  (DECLARED_NOT_RUN の real-window host Unsupported 行は維持)。
- macOS/Linux の GUI-L2/EXT-L3 は POLICIES.md どおり Deferred —
  実 GUI/実機器の利用要件が発生した時点で別 Issue 化する。
