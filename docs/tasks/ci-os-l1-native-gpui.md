---
id: "TASK-TIQabGiCahibaHUj3amJz5"
type: "task"
status: "in-progress"
title: "Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 実施タスク"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/377"
  changes:
    added:
      - type: "file"
        path: "docs/tasks/ci-os-l1-native-gpui.md"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ci-os-l1-native-gpui.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ci-os-l1-native-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/ci-os-l1-native-gpui.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
---

# Issue #377 synvelis-native / gpui-test 3OS CI OS-L1 実施タスク

## 対象

- Issue: https://github.com/scarecrow173/synvelis/issues/377
- ブランチ: `chore/377-ci-os-l1-native-gpui`
- 計画: `docs/plans/ci-os-l1-native-gpui.md`

## 作業単位

- [x] `ci.yml` `verify` leg: timeout 60m 化、Linux GPUI apt
  install、native/gpui-test 6 task + non-empty guard、evidence 表
  7 行追加。
- [x] `ci.yml` `verify-macos` leg: 同一の feature lane + guard +
  evidence 表追加、timeout 60m 化。
- [x] ローカル検証(Windows): `lint-actions`、native/gpui-test 6
  task、guard の `-- --list` 非空確認。
- [ ] `test-ci-summary`・`prek run --all-files`・docs index 再生成
  と graph 整合確認。
- [ ] review-protocol セルフレビュー(blocker 解消)。
- [ ] push + PR 作成(日本語本文)。
- [ ] Full CI 手動 dispatch と 3OS OS-L1 結果・run URL の記録。
- [ ] `worker_done` 送信。

## 編集範囲

- `.github/workflows/ci.yml`
- `docs/specs|designs|plans|tasks|test-specs/ci-os-l1-native-gpui*`
- 生成される docs index(`doc-maintenance` 手順で再生成)

## 触らない範囲

- `crates/` 配下、`mise.ci.toml`、他 workflow、
  `tools/ci-summary*`(job/leg 不変のため)、`POLICIES.md`。
