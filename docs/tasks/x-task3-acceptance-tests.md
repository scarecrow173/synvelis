---
id: "TASK-tU6Q6O7q6fOl2JizqTj6Kh"
type: "task"
status: "done"
title: "X task3 acceptance tests X-01..X-06 (synvelis-render/tests/x_e2e.rs)"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/x-recorded-verified-source-playback-export.md"
  implemented-by: []
  depends-on:
    - "docs/plans/x-recorded-verified-source-playback-export.md"
    - "docs/tasks/x-task2-recorded-media-publish.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/x-e2e-recorded-source-playback-export.md"
  references: []
  defers: []
  deferred-by: []
---

# X task3 acceptance tests

## Work

`crates/synvelis-render` に dev-dep `synvelis-recording` を追加し、
`tests/x_e2e.rs` に X-01..X-06 headless 受入 test を実装。

- x_normal: Normal gate 録音 → publish → commit → save/reload →
  L1 playback + L2 export + 独立 reopen、`MockPcm::sample` bit-exact。
- x_loop_punch_preroll: PunchInLoop 3-pass + 負 transport_start + AddClip
  (source offset・placement) で per-segment frame 対応。
- x_identity_revision_undo_save: ID/digest/revision/undo/redo/save-reload。
- x_finalized_only: crash before/after commit・journal truncation・
  WIP/orphan → resolve/publish 不可・blobs 未汚染。
- x_fault_classes: capture-fault・underrun・export failed の型分離。
- 全 test `import_local` 非使用 (X-06)。
