---
id: "TASK-vcs05-apply-restore-headless"
type: "task"
status: "done"
title: "VCS-05 Apply/Restore headless slice — 実装 task"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "PLAN-vcs05-apply-restore-headless"
  verified_by:
    - "TSPEC-vcs05-apply-restore-fixtures"
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-05 Apply/Restore headless slice — task

## Tasks

- [x] T1 `apply/` module（intent・entity・plan・journal・engine）
- [x] T2 `restore/` module（request・ports・staged publish・receipt）
- [x] T3 `lib.rs` mod/re-export 配線
- [x] T4 `tests/vcs05_apply.rs`（13 fixtures）
- [x] T5 `tests/vcs05_restore.rs`（7 fixtures）
- [x] T6 mise 検証（fmt-check/check/lint/test/build/test-msrv）
- [x] T7 review-protocol self review + impl 記録

## Deferred（Issue へ引き継ぎ）

- 実 OS FS durability / 実 Provider / plugin host / GUI / RT → #97
- AudioLens cross-consumer → #294
- `Removed` cascade apply、recording/comping/SMF 領域 apply → 別設計
- engine 実適用 → app 層責務
