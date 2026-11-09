---
id: "PLAN-034Uqx7xPwFa05K8WYNQ19"
type: "plan"
status: "approved"
title: "SV-GUI-04 slice2 実施計画 — close-veto/cancel"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-04-02-close-veto.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-02-close-veto.md"
    - "docs/designs/sv-gui-04-02-close-veto-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-04-02-close-veto.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice2 実施計画 — close-veto/cancel Implementation Plan

## Goal

`CloseRequested` の無条件 `Approve` を `syn.window.close_guard`
query 経由の vetoable decision に置き換え、未保存変更・録音中の
末尾 window close を fail-closed に veto する。

## Steps

1. `crates/synvelis-app/src/app.rs`: `close_veto_reasons()` を追加
   （unsaved_changes / recording_active / recorder_active）。
2. `crates/synvelis-app/src/service.rs`: `OPS` へ
   `syn.window.close_guard` を登録し、`dispatch` に Result
   `{"reasons": [...]}` を追加。
3. `crates/synvelis-app/src/shell.rs`: `CloseRequested` 分岐を
   `close_decision` へ委譲。末尾 live window 判定、
   `dispatch_window_intent` による同期 query、Result 解析、
   fail-closed 写像を実装。module doc 更新。
4. `crates/synvelis-app/tests/close_veto.rs` を新規作成し、
   test-spec の全 case をカバーする fixture テストを追加。
5. `mise run fmt-check` / `check` / `lint` / `test` / `build` /
   `check-app-native` / `lint-app-native` / `build-app-native`
   を実行し、実結果を記録。
6. review-protocol で自己レビュー → docs/impl に実施記録 →
   commit → push → PR → `worker_done`。

## Scope Guard

- `crates/synvelis-workspace/`・`crates/synvelis-recording/`・
  `crates/synvelis-project/` は変更しない。
- `tests/two_window.rs`・`tests/sv113_dock.rs`・既存 fixture は
  変更せず、新規 `tests/close_veto.rs` に閉じる。
- `Defer`・保存確認 prompt・非同期 close 継続は対象外。
