---
id: "TSPEC-034UrlOOfZIvsdF1bdNXfE"
type: "test-spec"
status: "approved"
title: "RCD-02 Durability / Recovery Review / Commit Receipt fault matrix"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Durability / Recovery Review / Commit Receipt fault matrix

## Purpose

Issue #116 slice の durability/recovery 契約を決定的に検証する
test-spec。fake FS 上の fault injection で分類・冪等・証拠の
契約を保証し、実 OS/FS durability の証明は意図的に対象外とする。

## Feature

録音 staging session の durability evidence 記録、crash 後の
recovery review 判定、commit receipt 分類と冪等再照合。

## Rules

- **R1 DurabilityEvidence**: `checkpoint()` は media sync →
  checkpoint append → journal sync → dir sync の順で試行し、
  各段の成否を `DurabilityEvidence` に記録する。sync 失敗時も
  失敗までの観測値を記録してから Err を返す。dir sync が
  `Unsupported`/`Failed`/`NotAttempted` のとき、durable 宣言は
  それ以下の保証に留まる。
- **R2 Verified Prefix のみ Complete**: unverified/corrupt tail を
  持つ stream は finalize/recover/commit のいずれでも
  `FinalizedOutcome::Complete`/`FinalizeOutcome::Complete` に
  ならない。Finalize record が Complete を主張しても、journal
  が CleanEnd でないか tail が残る場合は Partial に降格する。
- **R3 Explicit Review**: finalize されていない stream は
  `ReviewState::Pending` であり、`review_stream` の明示的な
  `Adopt`/`Hold`/`Discard` 判定なしに自動 finalize されない。
  判定記録は durable（`stream-NNNN.review.json`）。
- **R4 Discard fail-closed**: `Discard` は `MediaReferenceProbe`
  が `Clear` を返した場合のみ実行される。`Referenced`/`Unknown`/
  probe 不在は拒否され何も変更されない。参照中 media の削除を
  行わない。
- **R5 Idempotent Review/Commit**: `Adopt` 再実行・`Discard`
  再実行・`commit_session` 再実行・`reconcile_commit_receipt`
  再実行は副作用を重複させない（既存 finalized ref/registry/
  manifest を再利用）。
- **R6 Receipt 分類**: commit receipt は
  `Empty`/`Reserved`/`CommittedConfirmed`/`PublishIndeterminate`/
  `Conflict`/`Invalid` を区別し、publish indeterminate（registry
  適用済み・marker 未公開/不一致）は reconcile で冪等に修復
  される。`project_saved` は saved_revision との比較で Commit
  →Save 未完了を区別する。
- **R7 Digest 分離**: `IntegrityDigest`（CRC32 fold）は journal
  整合・commit 冪等 hash 専用であり、content identity（#86
  SHA-256 系）と型・命名で混同しない。`StreamEntry.digest_scheme`
  が採用 scheme を記録する。
- **R8 Fault Matrix**: kill/power-loss 相当・media/journal short
  write・sync failure・DirSync unsupported・permission denied・
  disk full・indeterminate publish を fault injection で再現し、
  各 fault で上記契約が保たれる。publish 経路は ruviform
  `StagedFile`/`SealedFile::publish` 上にあり、`PublishInject`
  seam は `Denied`=create 前・`SyncFail`=seal 後 publish 前
  （temp 除去）・`Indeterminate`=publish 成功後へ写像される
  （`docs/impl/ir/rcd-02-stagedfile-publish-migration.md` 参照）。

## Examples

- **E1 kill after checkpoint**: media+journal sync 済み checkpoint
  の後に kill → 再起動 `scan()` で durable frontier まで検証済み、
  unverified tail は Pending → `Adopt` で verified prefix のみ
  finalize（Complete にならない）。
- **E2 journal short write**: `ShortWrite` で journal tail が
  torn → replay は intact prefix で停止、`Corrupted` 分類、
  `recover_finalize`/`review_stream(Adopt)` とも Partial。
- **E3 media short write**: media tail が CRC 不一致 →
  `validated_tail_bytes`/`unverified_tail_bytes` で分離、
  frontier は最後の有効 checkpoint。
- **E4 sync failure**: `FailSync`/`SyncPermissionDenied` →
  `checkpoint()` は Err、`DurabilityEvidence` は media_synced
  または journal_synced=false を記録。
- **E5 DirSync unsupported**: capability `Unsupported` →
  `DirSyncState::Unsupported`、dir sync 失敗を durable 保証と
  誤認しない。
- **E6 permission/disk full**: `PermissionDenied`/`ByteLimit`
  → write 失敗が記録され、部分 write が verified prefix を
  汚染しない。
- **E7 indeterminate publish**: `PublishInject::Indeterminate`
  → marker は公開済みだが応答喪失 → receipt は
  `PublishIndeterminate`、`reconcile_commit_receipt` で
  `CommittedConfirmed` に収束し二重 marker/take を生成しない。
- **E8 discard referenced**: probe が `Referenced` → `Discard`
  拒否、media/journal/finalized は残る。
- **E9 re-review idempotent**: `Adopt` → 再 `scan()` で
  `ReviewState::Adopted` → 再 `Adopt` で同一 finalized ref。
- **E10 cross-start reconcile**: project reload + staging 再 open
  後に `commit_session`/`reconcile_commit_receipt` → registry/
  manifest で AlreadyApplied、duplicate take なし。

## Guarantees

- fake/mock FS 上の全 fault で Verified Prefix 以外が Complete
  化しない。
- review/commit の全操作が冪等または fail-closed。
- durability evidence が checkpoint 単位で観測可能。
- receipt が commit/save/publish/journal finalize を区別する。

## Non-goals

- 実 OS/FS の durability 実測（NotRun — 実機 gate）。
- #86 verified blob identity/migration・#84 StagedFile/HEAD CAS
  への実接続（`verified_sha256` 記録と `ProjectMediaProbe` による
  saved-HEAD 参照判定は task2 で接続済み。recording 側 publish の
  `StagedFile` 接続は task3 で実施済み —
  `docs/impl/ir/rcd-02-stagedfile-publish-migration.md` 参照。
  project 側 HEAD CAS 参照進化は残件）。
- 製品 GUI（#118）の recovery フロー。
- RT callback 上の noalloc/noblock の実オーディオスレッド計測
  （test-rt の既存検証に留める）。

## Risk

fault 注入が fake FS 上のみに留まるため、実 FS の rename/sync
原子性・dir sync capability の実挙動は未検証のまま残る。
`PublishIndeterminate` の分類が marker 内容の決定性に依存する
ため、manifest/registry の読み取り順序が変わると receipt が
変わり得る — test で固定する。
