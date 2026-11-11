---
id: "TASK-WHTOrax2xknKoKwYLbnSX"
type: "task"
status: "done"
title: "VCS-06 evidence completion slice"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs"
      - type: "file"
        path: "evidence/reports/vcs-06-compat-evidence.md"
      - type: "file"
        path: "docs/specs/vcs-06-evidence-completion.md"
      - type: "file"
        path: "docs/designs/vcs-06-evidence-completion-design.md"
      - type: "file"
        path: "docs/plans/vcs-06-evidence-completion-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-06-evidence-completion-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-06-evidence-completion.md"
      - type: "file"
        path: "docs/impl/ir/vcs-06-evidence-completion.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "PLAN-4RRd44dWWEduAvHmubiVw"
  implemented-by: []
  depends-on:
    - "PLAN-4RRd44dWWEduAvHmubiVw"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "TASK-4Sfq6WWBaX1avImCs5OcLh"
  refines: []
  refined-by: []
  derives-from:
    - "PLAN-4RRd44dWWEduAvHmubiVw"
  derived-by: []
  verifies:
    - "TSPEC-TDeaZLlpVBAaTB6kuxZzZ"
  verified-by: []
  references:
    - "evidence/reports/vcs-06-compat-evidence.md"
  defers: []
  deferred-by: []
---

# VCS-06 evidence completion slice

## 概要

synvelis#97 VCS-06 の headless で完結する evidence 補完 slice。
RESTORE-08 fixture（plugin_instances verbatim restore-as-copy）
と COMPAT-03/04 CI leg 実測記録を追加し、doc chain を作成する。
`src/` 変更・ledger 編集は行わない。

## 作業項目

- [x] `vcs06_plugin_instance_matrix.rs` に
  `restore_as_copy_preserves_plugin_instances_verbatim` を追加
  （resolved processor 2 件 + unresolved 1 件、envelope
  readback で `plugin_instances` verbatim pin）。
- [x] `gh pr checks` / `gh run list` / `gh run view` で
  #268/#306/#323 をカバーする ci.yml run の URL・outcome を
  実取得。
- [x] `evidence/reports/vcs-06-compat-evidence.md` に
  required（ubuntu/windows verify）/observed（verify-macos）
  区別つきで記録。#83 Close に伴う現行 ci.yml 錨を明記。
- [x] doc chain（spec/design/plan/task/test-spec/impl-ir）を
  `vcs-06-evidence-completion` 命名で作成。
- [x] canonical 検証 7 lane を実行し impl-ir に記録。

## 完了条件

- RESTORE-08 test が pass し verbatim 保持を pin する。
- COMPAT evidence が実測 URL/outcome を required/observed
  区別つきで記録する。
- ledger・`src/`・`docs/*/README.md` に差分がない。
- 全 canonical lane が pass（または実測どおりに記録）。
