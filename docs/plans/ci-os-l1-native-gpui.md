---
id: "PLAN-VqCkmPqwMy7x4374pKJMzz"
type: "plan"
status: "approved"
title: "Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 実施計画"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/377"
    - "docs/specs/ci-os-l1-native-gpui.md"
    - "docs/designs/ci-os-l1-native-gpui.md"
  changes:
    added:
      - type: "file"
        path: "docs/plans/ci-os-l1-native-gpui.md"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/ci-os-l1-native-gpui.md"
  implemented-by:
    - "docs/tasks/ci-os-l1-native-gpui.md"
  depends-on:
    - "docs/designs/ci-os-l1-native-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-166-full-ci-ci-summary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/ci-os-l1-native-gpui.md"
  derived-by:
    - "docs/tasks/ci-os-l1-native-gpui.md"
  verifies: []
  verified-by:
    - "docs/test-specs/ci-os-l1-native-gpui.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-summary.mjs"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
---

# Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 実施計画

## 方針

既存 `verify`(ubuntu+windows matrix)と `verify-macos` の各 leg に
`native`/`gpui-test` feature lane の step を追加する。新規 job は
作らず、`tools/ci-summary.mjs` の leg universe と gate 契約は
不変とする。検証の詳細は `docs/test-specs/ci-os-l1-native-gpui.md`、
実施単位は `docs/tasks/ci-os-l1-native-gpui.md` を参照。

## 手順

### 1. ci.yml 変更(`verify` leg)

- `timeout-minutes` を 40 → 60。
- Linux leg に `Install Linux GPUI native dependencies` step
  (`if: runner.os == 'Linux'`、apt で `libxkbcommon-dev`
  `libxkbcommon-x11-dev`)。
- `mise run test-full` と `Assert Cargo.lock unchanged` の間に
  6 task step + non-empty guard を追加:
  `check-app-native` / `lint-app-native` / `build-app-native` /
  `check-app-gpui` / `lint-app-gpui` / `test-app-gpui` /
  `Require non-empty gpui-test scenario selection`。
- lane evidence 表に 7 行追加(native check/lint/build、gpui
  check/lint、gpui guard、gpui test)。

### 2. ci.yml 変更(`verify-macos` leg)

- `timeout-minutes` を 40 → 60。
- 同一の 6 task step + guard(step 名・evidence 名は verify leg と
  同一)。apt step は Linux 限定のため不要。
- evidence 表に同じ 7 行を追加。

### 3. ローカル検証(Windows)

- `mise run lint-actions`(actionlint)。
- `mise run test-ci-summary`(`tools/ci-summary*` 不変でも gate
  契約の回帰確認として実行)。
- `mise run check-app-native` / `lint-app-native` /
  `build-app-native` / `check-app-gpui` / `lint-app-gpui` /
  `test-app-gpui` を実走し、非既定 feature の compile/link/test
  がローカルで成立することを確認(Linux/macOS の同等検証は
  Full CI の OS-L1 結果に委譲)。
- `mise exec -- prek run --all-files`(hook 相当の軽量チェック)。
- docs index を doc-maintenance 手順で再生成し、graph 整合を
  確認。

### 4. Full CI 実走

- push 後に `gh workflow run ci.yml --ref
  chore/377-ci-os-l1-native-gpui` で手動 dispatch。
- `verify (ubuntu-latest)` / `verify (windows-latest)` /
  `verify-macos` の 3 leg で新規 step が実走・成功したこと、
  run URL、leg 所要時間(追加 compile 時間の計測)を記録。
- `ci-summary`(full)が fail-closed に通過することを確認。

## 成果物

- `.github/workflows/ci.yml`(変更)
- docs 5 件(spec/design/plan/task/test-spec)+ 再生成 index
- 本タスクの PR(Issue #377)

## リスクと対策

- Linux leg で link 不足が出た場合は apt step を追補し再 dispatch。
- verify timeout 超過時は sccache 温機後の実測を見て判断(60m で
  不足なら leg 分割は集約器変更を伴うため別 Issue 化を検討)。
- `test-ci-summary` は本変更で ci-summary.mjs を触らないが、
  leg 名の暗黙契約(job 名→leg 名の写像)に影響がないことの
  回帰確認として実行する。
