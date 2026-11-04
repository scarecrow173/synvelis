---
id: "TASK-034RADaUbkmnzbFjZ29bl4"
type: "task"
status: "todo"
title: "SV-GUI-02 ruviform rev 追従 + Rust syn.* UI op 面"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
  changes:
    added:
      - "crates/synvelis-app/src/ui_dto.rs"
      - "crates/synvelis-app/src/ui_outbox.rs"
      - "crates/synvelis-app/src/ui_ops.rs"
      - "crates/synvelis-app/tests/ui_ops.rs"
    modified:
      - "crates/synvelis-app/src/app.rs"
      - "crates/synvelis-app/src/service.rs"
      - "crates/synvelis-app/src/lib.rs"
      - "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - "ui/vendor/ruviform/"
  implements:
    - "docs/plans/sv-gui-02-view-model-binding-ipc.md"
  implemented-by: []
  depends-on: []
  blocks:
    - "docs/tasks/sv-gui-02-frontend-bindingport-viewmodel-consoleview.md"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sv-gui-02-view-model-binding-ipc.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-02 ruviform rev 追従 + Rust syn.* UI op 面

## Work

1. ruviform git 依存を RF-GT-01/02/03 同梱 rev へ更新
   (`cargo update` → `Cargo.lock` 再生成 → `mise run sync-ui-deps`)。
2. `ui_dto.rs`/`ui_outbox.rs`/`ui_ops.rs` 新設、`syn.view.*`/
   `syn.control.*`/`syn.viz.*` op を `AppService` へ配線、
   capabilities 更新、outbox→notify push。
3. headless op テスト: attach→dispatch→pump→outbox receipt/projection、
   2 view 同 target、unsupported/read-only、viz bounded/stale、
   detach が project/audio/render をキャンセルしないこと。

## Classification

- `normal-plan-task`
