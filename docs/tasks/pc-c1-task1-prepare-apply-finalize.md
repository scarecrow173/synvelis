---
id: "TASK-034RoXjRUKOjib6ULatmIn"
type: "task"
status: "done"
title: "PC-C1: prepare/apply/finalize 実装タスク"
created: "2026-11-03"
updated: "2026-11-04"
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
    - "docs/plans/pc-c1-recording-transaction.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
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
    - "docs/test-specs/pc-c1-e2e-recording-transaction.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C1: prepare/apply/finalize 実装タスク

## Work

## T1 — transaction 拡張 + gate 閉鎖(control.rs)

- `RecordingTransactionId`(u64, session 単調)、`RecordingTxnState`
  (Pending/Conflict/Rejected/Committed)、`SealReason` 拡張
  (Stopped/Seek/ModeChanged/ArmChanged/ProjectChange)。
- `RecordingTransaction` に `id`/`epoch`/`fixed_lanes`/`state` を
  追加。`record_start` は id を採番して返し、write-lane 写像・
  epoch を固定する。
- `record_points` は `fixed_lanes` を参照し、`RECORD_POINT_BUDGET`
  (262_144 合計点)を超える新規書き込みを拒否。
- `bind_write_lane` は固定対象への再 bind を拒否。
- `transport_stop`/`transport_seek`/`set_mode`/`set_arm`(固定対象)/
  `note_project_changed` で seal。pause は seal しない。

## T2 — prepare/finish/rebase + error 型(control.rs)

- `RecordCommitError`/`PreparedRecordingCommit`/`RebasePreview`/
  `RebaseLaneReport`、FNV-1a fingerprint、applied registry
  (cap 64)+ `last_disposition`。
- `prepare_record_commit`(非破壊、Conflict/Invalid を state 記録)、
  `finish_record_commit`(committed 消費・AlreadyApplied/
  IntegrityConflict/UnknownTransaction)、`note_commit_rejected`、
  `rebase_preview`/`approve_rebase`。

## T3 — session orchestration + UI 適合(session/controls_ui/lib)

- `control_record_commit` を prepare→`apply_commit`(private、
  controller CAS + engine rebuild)→`finish_record_commit` に再構成。
- `apply`/`undo`/`redo` 成功時に `note_project_changed`。
- `RecordingOutcome.id` 追加。`controls_ui::record_commit` の error 型を
  `RecordCommitError` に、`record_rebase_preview`/
  `record_approve_rebase` を追加。lib.rs exports。

## T4 — 受入テスト

- `tests/pc_c1.rs`: REC-C1-01..06。
- `tests/pc_a.rs`/`tests/pc_b.rs`/`tests/sv47_controls.rs` の
  mid-session apply 依存を録音前へ再構成し、新契約に適合。

## T5 — 最終検証 + impl record

- `git fetch origin` → `git merge origin/main` →
  `mise run fmt/fmt-check/check/lint/test` → `docs/impl/ir/pc-c1-*.md`
  → commit。

## Classification

- `normal-plan-task`

## Upstream Documents

- <!-- plan, spec, ADR, or task path -->

## Dependencies

- <!-- prerequisite, blocker, or related task -->

## Affected Files

- <!-- file, directory, module, or interface -->

## Done When

- [ ] <!-- completion criterion -->

## Verification

- [ ] <!-- command, test, review step, or observable behavior -->
