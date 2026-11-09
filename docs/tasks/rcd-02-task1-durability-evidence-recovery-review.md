---
id: "TASK-034UrlZ4TNqATDPM8UO4Cd"
type: "task"
status: "in-progress"
title: "RCD-02 Task1 Durability Evidence / Recovery Review / Commit Receipt"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/plans/rcd-02-durability-recovery-review.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-02-durability-recovery-review.md"
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
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Task1 Durability Evidence / Recovery Review / Commit Receipt

## Work

`crates/synvelis-recording` に RCD-02 slice を実装する。

1. `crc32.rs`: `IntegrityDigest` newtype（CRC32 fold の integrity
   用途を型で分離）。
2. `writer.rs`: `DirSyncState`/`DurabilityEvidence` を追加し、
   `checkpoint()` が試行ごとに evidence を記録。`DurableFrontier.
   digest_fold` を `IntegrityDigest` に変更。`finalize()` で
   `FinalizedSourceRef.durability_evidence` へ転記。
3. `recovery.rs`: `ReviewState`/`ReviewRecord`/`ReviewAction`/
   `DiscardCandidate`/`ReferenceCheck`/`MediaReferenceProbe`/
   `ReviewError`/`ReviewOutcome`/`review_stream`/`review_stream_
   injectable` を追加。`scan()` に `review_state` と
   `fs_capability` を追加。`recover_stream_finalize` の Complete
   判定を「Finalize=Complete ∧ journal CleanEnd ∧ validated/
   unverified tail 共に 0」に強化。
4. `commit.rs`: `DigestScheme` + `StreamEntry.digest_scheme`
   （serde default で後方互換）、`CommitReceipt`/`ReceiptStage`/
   `StreamFinalizeEvidence`/`commit_receipt`/`reconcile_commit_
   receipt` を追加。
5. `faultinj.rs`/`staging.rs`: `PermissionDenied`/
   `SyncPermissionDenied`/`PublishInject`/`PublishFaults`/
   `publish_atomic_faultable` を追加し、`publish_atomic` を委譲。
6. `lib.rs`: export 更新。
7. `tests/recovery_review.rs`: fault-matrix test を追加。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-02-durability-recovery-review.md`
- `docs/designs/rcd-02-durability-evidence-recovery-review.md`
- `docs/specs/rcd-02-durability-evidence-recovery-review.md`

## Dependencies

- VS-C staging/journal/recovery（main 実装済み）
- VS-D commit manifest/applied_commits（main 実装済み）

## Affected Files

- `crates/synvelis-recording/src/{crc32,writer,recovery,commit,
  faultinj,staging,lib}.rs`
- `crates/synvelis-recording/tests/recovery_review.rs`（新規）

## Done When

- [ ] checkpoint ごとの `DurabilityEvidence` が記録・公開される
- [ ] `scan()` が review_state を分類し `Adopt`/`Hold`/`Discard`
      の明示 API が動作する（Discard は参照チェック必須・
      参照中 media を削除しない）
- [ ] commit receipt が commit/marker/journal finalize/save を
      区別し、publish indeterminate を冪等に reconcile する
- [ ] 別 Start 後の再照合で二重 Commit/take を生成しない
- [ ] CRC32 fold と content identity が型・命名で分離される
- [ ] fault-matrix test が kill/short write/sync fail/DirSync
      unsupported/permission/DiskFull/indeterminate publish を
      検証する
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`build`
      が全て Pass

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check`
- [ ] `mise run lint`
- [ ] `mise run test`
- [ ] `mise run test-rt`
- [ ] `mise run build`
- [ ] 実 OS/FS durability — NotRun（実機 gate）
