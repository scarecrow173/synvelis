---
id: "TASK-034UvJHLXI3ZB4q8FYKlbH"
type: "task"
status: "done"
title: "SYN-IO-01 task2 検証計画・Gate 証拠・文書チェーン完結"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/82"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/syn-io-01-ruviform-io-boundary-design.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-io-01-ruviform-io-boundary-design.md"
    - "docs/tasks/syn-io-01-task1-boundary-design-docs.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/syn-io-01-io-boundary-verification.md"
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-01-io-boundary-verification.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/288"
  defers: []
  deferred-by: []
---

# SYN-IO-01 task2 検証計画・Gate 証拠・文書チェーン完結

## Work

Issue #82 の AC-5〜AC-7 に対応する検証計画・gate 証拠・
文書チェーン完結作業を行う。

- `docs/test-specs/syn-io-01-io-boundary-verification.md`:
  Golden（G-01..07）・Negative（N-01..10）・Crash（C-01..07）
  の Rule 定義と gate 証拠対応表。全 case は NotRun。
- `docs/plans/syn-io-01-ruviform-io-boundary-design.md`:
  文書化計画（file map・dependency graph・task breakdown・
  risk register・verification）。
- 本 task と task1。
- managed index 更新（`docs/*/README.md`）。
- review-protocol 実施と blocker 解消。
- 最終検証: `doc_status`/`doc_maintenance` lint +
  `mise run fmt-check`。

## Acceptance

- AC-5: Saved Revision/Recovery/Undo/Asset/Plugin State/
  Import/Export の Golden・Negative・Crash test 計画が定義。
- AC-6: 既存 project-save E2E 非退行と Ruviform API 移行の
  type/DAG/OS/feature 影響が文書化。
- AC-7: Ruviform #288 gate への Synvelis 独立検証証拠項目
  （GATE-SV-01..09）が定義され、未実測は NotRun。

## Verification

- `doc_status`/`doc_maintenance` lint で front matter・relation・
  index 整合を確認。
- `mise run fmt-check`。
- review-protocol で blocker なし。
