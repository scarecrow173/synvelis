---
id: "IMPL-034XHQONJOa6EzQAgZoFxf"
type: "impl"
status: "completed"
title: "RCD-02 residual closeout — Issue #116 AC 照合・canonical lane 実測・残存 gate 棚卸し・簿記 drift 修正"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/designs/rcd-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/test-specs/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-02-residual-closeout.md"
      - type: "file"
        path: "evidence/sv116/rcd-02-closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/rcd-02-residual-closeout.md"
    - "docs/plans/rcd-02-residual-closeout.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/rcd-02-durability-evidence-recovery-review-commit-receipt.md"
    - "docs/impl/ir/rcd-02-project-library-adapter-verified-sha-256-mediareferenceprobe-saved-head.md"
    - "docs/impl/ir/rcd-02-stagedfile-publish-migration.md"
    - "docs/impl/ir/rcd-02-unsynced-loss-real-fs-probe-slice.md"
    - "docs/impl/ir/rcd-02-blob-sha256.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-02-residual-closeout.md"
    - "docs/designs/rcd-02-residual-closeout-design.md"
    - "docs/impl/ir/rcd-02-durability-evidence-recovery-review-commit-receipt.md"
    - "docs/impl/ir/rcd-02-stagedfile-publish-migration.md"
    - "docs/impl/ir/rcd-02-unsynced-loss-real-fs-probe-slice.md"
    - "docs/impl/ir/rcd-02-blob-sha256.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-residual-closeout.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-02-residual-closeout.md"
  verified-by: []
  references:
    - "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    - "evidence/sv116/rcd-02-blob-sha256-ledger.md"
    - "evidence/sv116/rcd-02-closeout-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcd-02-residual-closeout.md"
    rejected: []
---

# RCD-02 residual closeout — Issue #116 AC 照合・canonical lane 実測・残存 gate 棚卸し・簿記 drift 修正

## Summary

Issue #116（RCD-02）の簿記クローズアウト。プロダクションコード・
test・fixture の変更は一切なく、受入条件 4 行を実在する
test 名・既存 2 ledger の fixture 参照・canonical lane 実測
結果と 1:1 で照合し、doc chain・evidence・本 IR・exp に固定
した。外部 gate（真の power-loss・他 OS real-FS・実 device・
製品 GUI・consumer・project 側 HEAD CAS）は `NotRun`/
`Deferred` として所有 Issue を記名し、headless PASS を実機
証拠に昇格しない方針を維持した。併せて task3 status
（`in-progress`→`done`）と docs index の簿記 drift を修正
した。

照合の結論: headless 範囲の受入条件 AC-1〜AC-4 は実測済み
証拠で充足。AC-3 の「Legacy v5/Asset/Project Schema
migration との相互参照、旧 Snapshot/HEAD 保護」の横断部分と
AC-4 の他 OS real-FS は外部 gate が残り、Issue は open 継続が
前提（GUI=sv#118・consumer=`scarecrow173/ruviform#294` 等）。

## Context

- Issue: <https://github.com/scarecrow173/synvelis/issues/116>
  （RCD-02 tracker — auto-close しない。外部 gate 残存のため
  open 継続が前提）
- 統合済み実装 PR: #210（DurabilityEvidence/RecoveryReview/
  CommitReceipt/IntegrityDigest + `tests/recovery_review.rs`）・
  #239（verified_sha256/MediaReferenceProbe/saved-HEAD）・
  #275（torn-append/finalize/AlreadyApplied e2e）・#305
  （StagedFile/SealedFile TOCTOU publish 移行）・#320
  （unsynced-write loss fixture + Windows/NTFS real-FS probe +
  `test-recording-real-fs` lane）・Task4 Blob SHA-256 promotion
  （`crates/synvelis-player/tests/rcd02_blob_digest.rs`）。
- 本 slice の上流 doc chain:
  `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  rcd-02-residual-closeout*`。
- 証拠: `evidence/reports/
  rcd-02-durability-recovery-review-ledger.md`・
  `evidence/sv116/rcd-02-blob-sha256-ledger.md`・本 slice の
  `evidence/sv116/rcd-02-closeout-verification.md`。
- 先例: sv#86 `alib-02-residual-closeout`（同型の簿記
  closeout slice）。

## Implementation

### Changes（docs/evidence のみ。src/test/fixture 変更なし）

- 新規 doc chain 7 件:
  `docs/specs/rcd-02-residual-closeout.md`（status
  `implemented`、AC 照合 ledger）・
  `docs/designs/rcd-02-residual-closeout-design.md`（status
  `approved`）・`docs/plans/rcd-02-residual-closeout.md`（status
  `completed`）・`docs/tasks/rcd-02-residual-closeout.md`（status
  `done`）・`docs/test-specs/rcd-02-residual-closeout.md`
  （status `approved`）・本 IR（status `completed`）・
  `docs/impl/exp/rcd-02-residual-closeout.md`（status
  `completed`）。
- `evidence/sv116/rcd-02-closeout-verification.md`（新規）:
  AC 1:1 照合表・canonical lane 実測・gate 棚卸し・再現手順。
- 簿記 drift 修正:
  - `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md`:
    status `in-progress` → `done`（PR #305 で StagedFile/
    SealedFile TOCTOU publish 移行は merge 済みで `done` 相当。
    Done When/Verification の checkbox は当時の作業記録として
    そのまま残す — status frontmatter のみ実態に追従）。
  - `docs/*/README.md` 管理 index を doc-maintenance
    `index apply` で再生成。`docs/test-specs/README.md` の
    dup row（同一 `TSPEC-034UrlOOfZIvsdF1bdNXfE`）は
    managed index 領域の行が正本 frontmatter（`approved`）
    に揃った。一方、generated-index marker 管理外の保存
    tail 領域（PR #212 で混入した重複 table、generator が
    verbatim 保存）には stale `proposed` 行が残存し、
    `index plan` は in-sync を返す — 修復手段がないため
    残存 finding として PR 本文「未対応の作業」に記録。

### Issue #116 受入条件 照合結果

詳細な対応表は `docs/specs/rcd-02-residual-closeout.md` の
Acceptance Criteria 節と `evidence/sv116/
rcd-02-closeout-verification.md` を参照。要約:

- AC-1（kill/power-loss 相当・media/journal short write・sync
  failure・DirSync unsupported・permission・disk full・
  indeterminate publish の Fault Injection）: 実測済み。
  `recovery_review.rs` 16 tests（`rcd02_fault_matrix_*`・
  `rcd02_evidence_dir_sync_unsupported_is_honest`・
  `rcd02_unsynced_loss_*` 等）・`vs_c.rs` 16 tests・
  `staged_publish.rs` 7 tests（`publish_faultable_*`）・
  `vs_d.rs` 22 tests・`project_adapter.rs` 13 tests。
  真の power-loss（電源断）は非破壊的に自動化不能のため
  NotRun（gate 棚卸し参照）。
- AC-2（Verified Prefix 以外を Complete としない・別 Start 後の
  二重 Commit なし）: 実測済み。
  `rcd02_review_adopt_verified_prefix_and_idempotent`・
  `rcd02_finalize_record_complete_with_tail_downgrades_to_
  partial`・`rcd02_unsynced_loss_truncates_to_sync_frontier`・
  `rcd02_unsynced_loss_asymmetric_tail`・
  `vs_d_crash_before_commit_retry`・
  `vs_d_crash_after_commit_retry_no_dup`・
  `vs_d_retry_after_staging_drift_is_already_applied`・
  `vs_d_recommit_after_undo_recovers`・
  `rcd02pa_commit_save_reload_undo_restore_ids` 等。
- AC-3（CRC32 と SHA256 の用途区別・Legacy v5/Asset/Project
  Schema migration との相互参照・旧 Snapshot/HEAD 保護）:
  RCD-02 側の責務分は実測済み — `IntegrityDigest`（CRC32
  fold、integrity/idempotency 専用）と `verified_sha256`
  （content identity）の型・命名・`digest_scheme` 分離、
  `rcd02_blob_digest.rs` 10 tests（sha256 namespace・legacy
  CRC32 dual-scheme・opaque fail-closed・idempotent republish・
  tampered fail-closed・observation-reconcile）。横断部分
  （v5/Asset/Project Schema migration・旧 Snapshot/HEAD 保護）
  は ALIB-02（synvelis#86）・project 側 HEAD CAS（synvelis#84
  gate）にまたがるため分離記名。
- AC-4（OS/FS 別 Durability と Recovery 可能範囲の実測・
  NotRun/Unsupported を Pass 扱いしない）: Windows/NTFS
  real-FS lane（`mise run test-recording-real-fs`、PR #320）で
  rename/dir sync/publish/unsynced tail を実測、DirSync
  `Unsupported` を偽装しないことを確認（`realfs_probe_fs_
  capabilities`・`realfs_unsynced_tail_survives_process_kill`
  は `#[ignore]` opt-in lane）。真の power-loss・Linux/macOS
  real-FS は NotRun として所有 gate を記名。

### 判断と根拠

- **task3 status `done` 化の根拠**: PR #305 で StagedFile/
  SealedFile publish 移行は merge 済みであり、IR
  `rcd-02-stagedfile-publish-migration.md` も `completed`。
  task frontmatter の `in-progress` は stale だったため実態に
  追従。Done When/Verification の checkbox は当時の作業
  checklist 記録として変更しない（status frontmatter のみが
  drift 対象）。
- **test-specs index dup row の扱い**: `docs/test-specs/README.md`
  は `doc-suite:generated-index` marker 付きの生成物で、同一
  `TSPEC-034UrlOOfZIvsdF1bdNXfE` が managed index（line 100、
  `approved`）と marker 管理外の保存 tail 領域（line 230、
  `proposed`、PR #212 由来の重複 table）に併存していた。
  正本は各 test-spec file の frontmatter（`approved`）であり、
  `doc_maintenance.js index apply --dir docs/test-specs` の
  再生成で managed index 側は `approved` に揃った。
  tail 領域は generator が verbatim 保存し `index plan` が
  in-sync を返すため、doc-maintenance の whitelist（managed
  index 再生成・link case 修復）では修復できない。手編集は
  worker spec で禁止されているため、残存 `proposed` 行は
  finding として記録し PR 本文「未対応の作業」に明記する。
- **exp doc の形式**: `docs/impl/exp/` には CLI 管理の `.jsonl`
  event log と markdown の実験記録が併存する。本 slice は
  「既存 ledger の consolidate のみ・再 derive しない」という
  手法上の判断を記録するため、markdown 形式の exp
  （`rcd-02-blob-sha256.md` 先例どおり）とした。実験的な
  実装探索は伴わない簿記 slice だが、AC 照合の「consolidate
  のみ」方針と index dup row の発見・解消は記録価値がある
  ため exp を残す。
- **index README の同コミット同梱**: worker spec の編集許可
  範囲は生成 index の再生成を許容し、本 slice は status 変更・
  新規 doc 追加を含むため、再生成物を同じコミットに含める
  （sv#86 先例は merge 時再生成に委ねたが、本 slice は占有
  範囲に index regen が明示されている）。

## Related Experiments

### Adopted

- `docs/impl/exp/rcd-02-residual-closeout.md` — AC 照合を
  「既存 2 ledger の consolidate のみ・再 derive しない」で
  行う方針と、test-specs index dup row（`proposed` vs 正本
  `approved`）の発見・index 再生成での解消を記録。

### Rejected

なし。

## Validation

canonical lane を `mise run <task>` 経由で worktree
（Windows/NTFS、toolchain rustc 1.99.1、対象 commit は本 PR の
head で `origin/main` `dcac0c1` 以降をベース）で実走した。
結果の詳細・再現手順は `evidence/sv116/
rcd-02-closeout-verification.md` を参照。

| Lane | Command | Result |
| --- | --- | --- |
| fmt | `mise run fmt-check` | Pass (exit 0) |
| hygiene | `mise exec -- prek run --all-files` | Pass (exit 0 — 全 hook Passed) |
| test-recording | `mise run test synvelis-recording` | Pass (exit 0 — `recovery_review` 16・`staged_publish` 7・`vs_c` 16・`vs_d` 22・`project_adapter` 13 tests all ok) |
| test-player | `mise run test synvelis-player` | Pass (exit 0 — `rcd02_blob_digest` 10・`verified_import` 8 tests all ok) |
| test-project | `mise run test synvelis-project` | Pass (exit 0 — 全 test binaries ok) |
| rt | `mise run test-rt` | Pass (exit 0 — workspace `rt_` filter tests all ok) |
| real-fs | `mise run test-recording-real-fs` | Pass (exit 0 — opt-in 実 NTFS lane、`real_fs_probe` 2 tests ok。真の power-loss は NotRun のまま) |

doc 監査: `doc_status.js lint`/`audit`・`audit_impl_record.js`・
`doc_maintenance.js index plan/apply` の実走結果は
`evidence/sv116/rcd-02-closeout-verification.md` の Doc 監査節
に記録。

NotRun: `*-msrv`・`*-full` 系（Daily Full CI 委譲）・真の
power-loss・他 OS real-FS・実 device（#115）・製品 GUI（#118）・
consumer（`scarecrow173/ruviform#294`）・project 側 HEAD CAS
（#84）。

## Risks

- **doc drift の再発**: repo-wide baseline の legacy
  invalid-id 群・一部 reciprocal 警告は migration 対象であり
  本 slice の責務外。新規 doc が新規 finding を増やさないこと
  を監査で確認する。
- **lane の環境依存 flake**: 過去に webdav flake・LNK1102 の
  観測例がある。Fail が出た場合は単独再実行で切り分け、最終
  結果を正直に記録する方針。
- **AC-3 横断行の解釈**: 「Legacy v5/Asset/Project Schema
  migration との相互参照、旧 Snapshot/HEAD 保護」は RCD-02
  単体ではなく ALIB-02（#86）・project 側（#84）にまたがる。
  RCD-02 側の責務分のみを Pass とし、横断部分は gate 記名で
  分離した — reviewer はこの分離の妥当性を確認されたい。

## Follow-ups

- Issue #116 の close 判定: AC-3 横断部分・AC-4 他 OS
  real-FS・実 device（sv#115）・製品 GUI（sv#118）・consumer
  （`scarecrow173/ruviform#294`）・project 側 HEAD CAS（sv#84）
  の外部 gate が完了するまで Issue は open 継続。
- `docs/*/README.md` 以外の repo-wide doc drift（legacy
  invalid-id 群等）は別 slice の migration 対象。
