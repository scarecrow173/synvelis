---
id: "PLAN-034UrkBlTpaeiSGII0EyIi"
type: "plan"
status: "completed"
title: "RCD-02 Durability Evidence / Recovery Review / Commit Receipt slice"
created: "2026-11-09"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  implemented-by:
    - "docs/tasks/rcd-02-task1-durability-evidence-recovery-review.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  verified-by:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Durability Evidence / Recovery Review / Commit Receipt slice Implementation Plan

## Goal

`crates/synvelis-recording` に durability evidence・explicit
recovery review・commit receipt/冪等 reconcile・digest 分離・
fault injection 拡張を実装し、fault-matrix test で検証する。

## Scope

単一実装 stream。変更は `crates/synvelis-recording/`（src + tests）
と `docs/` のみ。synvelis-project/library/app/workspace/input.rs
および他 crate には触れない。

## Upstream Documents

- `docs/specs/rcd-02-durability-evidence-recovery-review.md`

## Design Inputs

- `docs/designs/rcd-02-durability-evidence-recovery-review.md`

## File Map

- `crates/synvelis-recording/src/crc32.rs` - `IntegrityDigest` newtype
- `crates/synvelis-recording/src/writer.rs` - `DirSyncState`/
  `DurabilityEvidence`、checkpoint evidence 記録、finalize 転記
- `crates/synvelis-recording/src/recovery.rs` - `ReviewState`/
  `ReviewRecord`/`ReviewAction`/`DiscardCandidate`/
  `ReferenceCheck`/`MediaReferenceProbe`/`ReviewError`/
  `ReviewOutcome`/`review_stream`、scan review_state、Complete 強化
- `crates/synvelis-recording/src/commit.rs` - `DigestScheme`/
  `StreamEntry.digest_scheme`、`CommitReceipt`/`ReceiptStage`/
  `commit_receipt`/`reconcile_commit_receipt`
- `crates/synvelis-recording/src/faultinj.rs` - `PermissionDenied`/
  `SyncPermissionDenied`/`PublishInject`/`PublishFaults`/
  `publish_atomic_faultable`
- `crates/synvelis-recording/src/staging.rs` - publish_atomic を
  faultable 実装へ委譲
- `crates/synvelis-recording/src/lib.rs` - export 更新
- `crates/synvelis-recording/tests/recovery_review.rs` - fault-matrix
  + review/receipt 冪等検証（新規）
- 既存 `tests/vs_c.rs`/`vs_d.rs`/`tests/recording.rs` - 回帰確認

## Implementation Sequence

- [x] `IntegrityDigest` 追加（crc32.rs）
- [x] `DirSyncState`/`DurabilityEvidence` + checkpoint 記録 +
      `DurableFrontier.digest_fold` 型変更 + finalize 転記（writer.rs）
- [x] recovery: `ReviewState`/`ReviewRecord`/review API/Complete 強化
      （recovery.rs）
- [x] commit: `DigestScheme` + receipt/reconcile API（commit.rs）
- [x] faultinj/staging: publish faultable 拡張（faultinj.rs/
      staging.rs）
- [x] lib.rs export 更新
- [x] `tests/recovery_review.rs` fault-matrix 実装
- [x] 既存テスト回帰修正（必要時）

## Dependencies and Risks

- `checkpoint()` 戻り値変更は `Result<_, WriterError>` の Ok 型のみ —
  `.unwrap()`/`.unwrap_err()`/`if let Err` 呼出しは互換。
- Complete 強化は finalize outcome のみ Partial へ降格 —
  vs_c/vs_d/x_e2e の期待値を確認。
- review 判定記録は `publish_atomic` 経由で原子性確保。
- `MediaReferenceProbe` は将来 project/library adapter が実装する
  port — 本スライスでは trait + fail-closed のみ。

## Task Breakdown

- Task 1: `docs/tasks/rcd-02-task1-durability-evidence-recovery-review.md`
  （全ファイルを単一 task で実装 — 同一 crate 内の一貫した変更）

## Verification

- [x] `mise run fmt-check`
- [x] `mise run check`
- [x] `mise run lint`
- [x] `mise run test`
- [x] `mise run test-rt`
- [x] `mise run build`
- [ ] 実 OS/FS durability 実測 — NotRun（実機 gate）

## Open Questions / Gaps

- なし（spec/design で確定済み）

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（Orca worker dispatch 内）
- Independent work streams: none
- User approval required before dispatch: no（review-protocol で代替）
- If approved: execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- 前提: VS-C/VS-D 基盤は main に実装済み。`recover_finalize` の
  Complete 強化が既存テストの期待値に影響する可能性 — 修正時は
  仕様（unverified tail を Complete にしない）に従う。
