---
id: "IMPL-034VhZh19UVNoeeFues4Ow"
type: "impl"
status: "completed"
title: "RCD-02 Durability Evidence / Recovery Review / Commit Receipt"
created: "2026-11-10"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/rcd-02-durability-evidence-recovery-review-commit-receipt.md"
      - type: "file"
        path: "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/src/crc32.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/writer.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/recovery.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/faultinj.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/staging.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/vs_c.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/recovery_review.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/project_adapter.rs"
  implements:
    - "docs/tasks/rcd-02-task1-durability-evidence-recovery-review.md"
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-02-durability-recovery-review.md"
    - "docs/tasks/rcd-02-task1-durability-evidence-recovery-review.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
    - "docs/impl/ir/rcd-02-project-library-adapter-verified-sha-256-mediareferenceprobe-saved-head.md"
    - "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
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
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-02 Durability Evidence / Recovery Review / Commit Receipt

## Summary

Issue #116 RCD-02 の task1 slice（durability evidence・recovery
review・commit receipt・fault matrix）を実装した。本 record は
merge 済み実装（PR #210 `67e0870`）と、本 dispatch で追加した
fault/receipt/e2e 補完 test・evidence ledger を統合して記録する
（前 worker が IR 未作成のまま merge したため、実 diff に基づき
事後作成）。

- `checkpoint()` が試行ごとに `DurabilityEvidence`
  （media_synced/journal_synced/dir_sync/frontier/fs_capability）
  を記録し、失敗時も観測済み stage を保持してから Err を返す。
- `scan()` が stream を `ReviewState::Pending` 等に分類し、
  `review_stream` の明示 `Adopt`/`Hold`/`Discard` でのみ確定する。
  Discard は `MediaReferenceProbe` が `Clear` の場合のみ。
- `commit_receipt`/`reconcile_commit_receipt` が Empty/Reserved/
  CommittedConfirmed/PublishIndeterminate/Conflict/Invalid を
  区別し、publish indeterminate を冪等に修復する。
- `IntegrityDigest`（CRC32 fold）と content identity を型・命名で
  分離（`verified_sha256` の接続は task2 IR を参照）。
- fault injection（kill 相当・media/journal short write・sync
  failure・DirSync unsupported・permission・disk full・
  indeterminate publish）を fake FS seam で再現。

## Context

- Task: `docs/tasks/rcd-02-task1-durability-evidence-recovery-review.md`
- Spec: `docs/specs/rcd-02-durability-evidence-recovery-review.md`
- Design: `docs/designs/rcd-02-durability-evidence-recovery-review.md`
- Plan: `docs/plans/rcd-02-durability-recovery-review.md`
- Test spec: `docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md`
- Issue: <https://github.com/scarecrow173/synvelis/issues/116>
- 前提: VS-C（staging/journal/checkpoint/recovery、#12/#15 系）と
  VS-D（commit manifest・applied_commits・commit.json marker、#17 系）

## Implementation

### Changes

PR #210（`67e0870`）での実装:

- `crates/synvelis-recording/src/crc32.rs`: `IntegrityDigest`
  newtype（CRC32 fold の integrity 用途を型で分離）。
- `crates/synvelis-recording/src/writer.rs`: `DirSyncState`/
  `DurabilityEvidence` 追加、`checkpoint()` が media sync →
  checkpoint append → journal sync → dir sync の順で各段の成否を
  記録。`DurableFrontier.digest_fold` を `IntegrityDigest` 化し、
  `finalize()` で `FinalizedSourceRef.durability_evidence` へ転記。
- `crates/synvelis-recording/src/recovery.rs`: `ReviewState`/
  `ReviewRecord`/`ReviewAction`/`DiscardCandidate`/
  `ReferenceCheck`/`MediaReferenceProbe`/`ReviewError`/
  `ReviewOutcome`/`review_stream`/`review_stream_injectable` 追加。
  `scan()` に `review_state`/`fs_capability` 追加。
  `recover_stream_finalize` の Complete 判定を「Finalize=Complete
  ∧ journal CleanEnd ∧ validated/unverified tail 共に 0」に強化。
  review 判定記録の read 失敗を fail-closed（Unreadable）に分類
  （follow-up fix commit）。
- `crates/synvelis-recording/src/commit.rs`: `DigestScheme` +
  `StreamEntry.digest_scheme`（serde default で後方互換）、
  `CommitReceipt`/`ReceiptStage`/`StreamFinalizeEvidence`/
  `commit_receipt`/`reconcile_commit_receipt` 追加。
- `crates/synvelis-recording/src/faultinj.rs`/`staging.rs`:
  `PermissionDenied`/`SyncPermissionDenied`/`PublishInject`/
  `PublishFaults`/`publish_atomic_faultable` 追加、
  `publish_atomic` を委譲。
- `crates/synvelis-recording/src/lib.rs`: export 更新。
- `crates/synvelis-recording/tests/recovery_review.rs`: fault-matrix
  test 初版。
- `crates/synvelis-recording/tests/vs_c.rs`: 互換修正 1 箇所。

本 dispatch（`feature/116-rcd02-durability`）での補完:

- `tests/recovery_review.rs` +3 tests:
  - `rcd02_fault_matrix_journal_short_write_torn_append` —
    journal `Inject::ShortWrite` で tail torn → `StreamState::Failed`、
    scan で `StreamClass::Partial` + `ReviewState::Pending`、
    `Adopt` が `FinalizedOutcome::Partial`（`source_frames=128`）。
  - `rcd02_commit_receipt_finalize_publish_lost_is_distinct` —
    `finalized.json` 喪失 + journal Finalize=Complete が別状態と
    して観測されることを固定。
  - `rcd02_commit_receipt_adopted_partial_finalize_record_absent` —
    Partial adopt は finalized file あり・journal Finalize record
    なしで commit が `NothingCommitted` + recoverable
    `partial-finalize` となることを固定。
- `tests/project_adapter.rs` +1 test
  （`rcd02pa_commit_save_reload_undo_restore_ids` — commit→save→
  reload→undo/redo→`AlreadyApplied` e2e。詳細は task2 IR 参照）。
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
  新規（verdict 行形式の統合 evidence ledger）。

### Decisions During Implementation

- checkpoint 順序（media sync → checkpoint append → journal sync →
  dir sync）は `StreamWriter::checkpoint()` が所有し、evidence は
  各試行で観測値を保持してから Err を返す（失敗 stage を消さない）。
- unverified/journaled tail は finalize/recover/commit のいずれでも
  `Complete` に昇格しない。Finalize record が Complete を主張しても
  journal 非 CleanEnd または tail 残存なら Partial に降格する。
- review 判定記録は `publish_atomic` 経由で原子公開。read 失敗は
  不在扱いせず `Unreadable`（fail-closed、既存記録を上書きしない）。
- Discard は `MediaReferenceProbe::Clear` のみ許可。`Referenced`/
  `Unknown`/probe 不在は拒否（参照中 media を削除しない）。
- receipt は registry 適用・marker 公開・journal finalize record・
  finalized JSON・saved revision を別 field で区別し、publish
  indeterminate（rename 後応答喪失）は `reconcile_commit_receipt`
  で冪等に `CommittedConfirmed` へ収束させる。
- CRC32 fold（`IntegrityDigest`）は integrity/idempotency 専用で
  content identity ではない。`verified_sha256`（#86 形式
  `sha256:<64hex>`）は task2 で別 namespace として接続。
- 実 OS/FS durability・実 device・製品 GUI・consumer 精度は本
  slice の対象外であり、evidence ledger で `NotRun`/`Unsupported`
  と正直に記録する。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

PR #210 時点の canonical 検証（`mise run fmt-check`/`check`/`lint`/
`test`/`test-rt`/`build`/`test-msrv`）は全て Pass（当時の PR 記録）。
本 change set では `cargo test -p synvelis-recording --test
recovery_review --test project_adapter`（補助証拠）で新規 4 tests
を含む 27 tests が Pass。最終 canonical 検証は本 PR の `mise run`
結果を参照。

NotRun: 実 OS/FS durability 実測・実 device・#118 製品 GUI E2E・
多 OS matrix・consumer 精度（いずれも別 gate）。

## Risks

- fault 注入は fake/injected seam のみで、実 FS の rename/sync
  原子性・dir sync capability の実挙動は未検証（実機 gate）。
- `PublishIndeterminate` 分類は marker 内容の決定性に依存する。
- `MediaReferenceProbe` は port のみで、project 実装は task2
  （`ProjectMediaProbe`/`CompositeProbe`）が担当。

## Follow-ups

- 実 OS/FS durability 実測（power-loss/kill 実機・多 OS matrix）。
- #115 実 device 録音 fault、#118 製品 Comp/Take/Recovery GUI E2E。
- #86 verified blob identity・#84 StagedFile/HEAD CAS との実接続
  （task2 IR の Follow-ups を参照）。
