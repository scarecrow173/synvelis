---
id: "TSPEC-034XHOEGGpnzgIiRL1WpeX"
type: "test-spec"
status: "approved"
title: "RCD-02 residual closeout — AC 照合・canonical lane・gate 棚卸し検証仕様"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
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
    - "docs/specs/rcd-02-residual-closeout.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
    - "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-residual-closeout.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-02-residual-closeout.md"
  verified-by: []
  references:
    - "evidence/sv116/rcd-02-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-02 residual closeout — AC 照合・canonical lane・gate 棚卸し検証仕様

## Purpose

RCD-02 residual closeout slice（docs+evidence only）の簿記が
誠実であることを保証する test-spec。新規 test/fixture を
追加せず、既存 test 名・既存 ledger・canonical lane 実測への
1:1 対応付けと、未実測 lane の NotRun/Deferred 記録を検証
対象とする。実装済み機能そのものの検証は既存 test-spec
（fault matrix・blob digest・unsynced-loss probe）が担う。

## Feature

Issue #116 受入条件 4 行の closeout 照合、canonical lane 実測
記録、外部 gate 棚卸し、doc status/index 簿記の整合。

## Rules

- **R1 1:1 対応付け**: Issue 本文の各受入条件行は、実在する
  test 名（`crates/*/tests/`）・既存 ledger の fixture 参照・
  実測 lane 結果のいずれかに 1:1 で対応付けられなければ
  ならない。推測で証拠を補わない。
- **R2 実測のみ Pass**: `Pass` は実際に実行して成功した lane
  のみを指す。未実行・未測定・自動化不能の lane は
  `NotRun`/`Unsupported`/`Deferred` とし、Pass と書かない。
- **R3 gate 記名**: 外部 gate（真の power-loss・他 OS
  real-FS・実 device・製品 GUI・consumer・project 側 HEAD
  CAS）は `NotRun`/`Deferred` とし、所有 Issue
  （synvelis#115・#118・#84・`scarecrow173/ruviform#294`）を
  記名する。
- **R4 mock 昇格禁止**: fake/mock FS・scripted double・
  headless fixture・truncate モデルの PASS を実 durability・
  実機の証拠に昇格しない。
- **R5 index 再生成**: `docs/*/README.md` 管理 index は
  doc-maintenance の `index apply` で再生成し、手編集しない。
  task3 status 変更・新規 doc 追加に伴う index drift は
  再生成で解消する。
- **R6 範囲遵守**: 変更は worker spec の編集許可範囲
  （新規 doc chain 7 file・`evidence/sv116/
  rcd-02-closeout-verification.md`・task3 status・生成
  index・任意 additive note）に限定され、`crates/`・`ui/`・
  共有設定・他 slug docs・`evidence/` 他 dir を変更しない。

## Examples

- **E1 AC-1 fault matrix**: `recovery_review.rs` の
  `rcd02_fault_matrix_write_sync_permission_diskfull`・
  `rcd02_fault_matrix_journal_torn_tail_review_adopt`・
  `rcd02_fault_matrix_journal_short_write_torn_append`、
  `staged_publish.rs` の `publish_faultable_*` 4 tests、
  `rcd02_unsynced_loss_*` 2 tests が Issue AC-1 の fault
  injection 行に対応する。真の power-loss 行は NotRun。
- **E2 AC-2 verified prefix/no double commit**:
  `rcd02_review_adopt_verified_prefix_and_idempotent`・
  `rcd02_finalize_record_complete_with_tail_downgrades_to_
  partial`・`vs_d_crash_before_commit_retry`・
  `vs_d_crash_after_commit_retry_no_dup`・
  `rcd02pa_commit_save_reload_undo_restore_ids` が対応。
- **E3 AC-3 digest 分離**: `rcd02_blob_digest.rs` 10 tests +
  `rcd02pa_commit_manifest_carries_verified_sha256`（
  `DigestScheme` 分離）+ `legacy_crc32_receipt_keeps_flat_blob`
  /`opaque_receipt_digest_fails_closed`（dual-scheme/
  fail-closed）が対応。v5/旧 Snapshot/HEAD 保護の横断部分は
  synvelis#86・#84 gate として分離記名。
- **E4 AC-4 OS/FS 実測**: `mise run test-recording-real-fs`
  （PR #320）の Windows/NTFS 実測行と、DirSync
  `Unsupported` の正直な分類（`rcd02_evidence_dir_sync_
  unsupported_is_honest`）が対応。他 OS は NotRun。
- **E5 簿記 drift**: task3 frontmatter status が `done`、
  `docs/test-specs/README.md` の managed index row が
  `approved`（正本 frontmatter と一致）であることを index
  再生成後に確認する。marker 管理外の保存 tail 領域に残る
  stale `proposed` 重複行は generator が verbatim 保存し
  修復手段がないため、残存 finding として記録する
  （Pass 扱いしない）。

## Guarantees

- closeout verification evidence の各行が実在 test/ledger/lane
  に trace できる。
- 未実測 lane が Pass と記録されない。
- doc chain の status/relations が実態と一致し、index が
  in-sync になる。

## Non-goals

- RCD-02 機能そのものの新規検証（既存 test-spec の責務）。
- 外部 gate の実施・Pass 昇格。
- `*-msrv`・`*-full` 系 lane（Daily Full CI 委譲）。

## Risk

照合表が実在しない test 名や過去 ledger の転記ミスを含むと、
closeout の信頼性が損なわれる。test 名は `rg` で実在確認し、
lane 結果は実行 exit code のみを記録する。
