---
id: "SPEC-034XHM3cBgw6hRT0wUniFS"
type: "spec"
status: "implemented"
title: "RCD-02 residual closeout — Issue #116 AC 照合・canonical lane 実測・gate 棚卸し・簿記 drift 修正"
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
      - type: "file"
        path: "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
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
  implements: []
  implemented-by:
    - "docs/designs/rcd-02-residual-closeout-design.md"
    - "docs/plans/rcd-02-residual-closeout.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
    - "docs/specs/rcd-02-blob-sha256.md"
    - "docs/impl/ir/rcd-02-durability-evidence-recovery-review-commit-receipt.md"
    - "docs/impl/ir/rcd-02-stagedfile-publish-migration.md"
    - "docs/impl/ir/rcd-02-unsynced-loss-real-fs-probe-slice.md"
    - "docs/impl/ir/rcd-02-blob-sha256.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    - "evidence/sv116/rcd-02-blob-sha256-ledger.md"
    - "evidence/sv116/rcd-02-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-02 residual closeout — Issue #116 AC 照合・canonical lane 実測・gate 棚卸し・簿記 drift 修正

## What Should Be Built

Issue #116（RCD-02: 録音 Durability / Recovery Review / Commit
Receipt / verified-prefix SHA-256 / StagedFile publish 移行）の
headless 実装は PR #210・#239・#275・#305・#320・Task4 Blob
SHA-256 promotion で main に統合済みである。本 spec が定めるのは
**簿記クローズアウト**のみであり、新規のプロダクション実装・
test・fixture の追加は含まない。

- Issue 本文の受入条件 4 行を、実在する test 名・既存 ledger の
  fixture 参照と 1:1 で照合し、`evidence/sv116/
  rcd-02-closeout-verification.md` に記録する（既存 2 ledger の
  列挙を consolidate するだけで再 derive しない）。
- canonical lane（`mise run test synvelis-recording` /
  `synvelis-player` / `synvelis-project` / `test-rt`）を現行
  main ベースの worktree で実走し、Pass/Fail を記録する。
  `*-msrv`・`*-full` 系は Daily Full CI 委譲で NotRun とする。
- 外部 gate（真の power-loss・他 OS real-FS・実 device・製品
  GUI・consumer・project 側 HEAD CAS）は `NotRun`/`Deferred` と
  し、所有 Issue を記名する。mock/double/fixture の PASS を
  実機証拠に昇格しない。
- 簿記 drift を修正する: `docs/tasks/
  rcd-02-task3-stagedfile-publish-migration.md` の status
  `in-progress` → `done`（PR #305 merge 済み）。
  `docs/test-specs/README.md` の dup row 不整合（同一
  `TSPEC-034UrlOOfZIvsdF1bdNXfE` が managed index で
  `approved`・保存 tail 領域で `proposed` と併存）は、
  managed index を index 再生成で正本 frontmatter
  （`approved`）側に揃え、tail 領域の stale 重複行は
  generator が verbatim 保存し修復手段がないため残存
  finding として記録する。
- doc status の簿記を実態に整合させ、Implementation Record
  `docs/impl/ir/rcd-02-residual-closeout.md` と Experiment Log
  `docs/impl/exp/rcd-02-residual-closeout.md` に結果を固定する。

## Why Now

RCD-02 の headless 実装は全て merge 済みだが、Issue の受入条件
各行と実測証拠の対応表・残存 gate の所在・task/test-spec index
の簿記が一箇所に集約されていない。Issue は GUI（sv#118）・
consumer（`scarecrow173/ruviform#294`）等の外部 gate が残るため
open 継続が前提であり、「何が headless で証明済みで、何がどの
Issue の gate か」を canonical doc に固定することで、後続の
close 判定と並行 Worker の衝突を防ぐ。sv#86 の
`alib-02-residual-closeout` 先例と同型の closeout slice。

## Users and Value

- **Issue #116 owner / reviewer**: 受入条件各行の証拠所在と
  外部 gate の owner が一目で分かり、close 判定の材料になる。
- **後続 Worker / 保守者**: どの doc・fixture・lane が RCD-02 の
  何を証明するかの正本にたどり着ける。
- **doc-driven-dev 監査**: status/relations が実態と一致し、
  stale-upstream・orphan の誤検出を減らす。

## Scope

### In Scope

- `docs/` 配下の RCD-02 / Issue #116 関連 doc chain の status・
  relations の実態整合（編集許可範囲に限定）。
- 新規 doc chain: 本 spec・design・plan・task・test-spec・IR・
  exp（`rcd-02-residual-closeout*`）。
- `evidence/sv116/rcd-02-closeout-verification.md` の新規作成
  （AC 照合表・canonical lane 実測・gate 棚卸し）。
- `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md` の
  status 修正と `docs/*/README.md` 管理 index の再生成。
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
  への additive note（closeout verification への誘導。
  任意・既存記述は不変）。
- canonical mise lane の実走と結果記録、doc_status/doc_maintenance
  監査の実走。
- Issue #116 への進捗コメントと PR 作成（Issue の close・merge
  は行わない）。

### Out of Scope

- プロダクションコード（`crates/`）・`ui/`・Cargo/mise/workflow
  定義の変更。新規 test・fixture・harness の作成。
- 未検証バイトの verified 昇格、RT 契約・legacy CRC 経路の変更。
- 外部 gate の NotRun→Pass 昇格、または gate 自体の実施
  （真の power-loss・他 OS real-FS・実 device・GUI・consumer・
  project 側 HEAD CAS）。
- `*-msrv`・`*-full` 系 lane（Daily Full CI 委譲）。
- Issue #116 の編集（close は Coordinator が merge 後に判断）。
- 他 Issue 所有の残 gate 実装（sv#115 device・sv#118 GUI・
  sv#84 project 側 HEAD CAS・`scarecrow173/ruviform#294`
  consumer）。

## Requirements

- Issue 本文の各受入条件行を、実在する test 名・既存 ledger の
  fixture 参照・lane 結果と 1:1 で対応付ける。対応する証拠が無い
  行は理由付きで残す。
- canonical lane は `mise run <task>` 経由で実走し、Pass/Fail を
  分けて記録する。未実行を Pass と書かない。
- 外部 gate は `NotRun`/`Deferred`/`Unsupported` とし、所有
  Issue（sv#115・sv#118・sv#84・`scarecrow173/ruviform#294`）を
  記名する。
- fake/mock FS・scripted double・headless fixture・truncate モデル
  の結果を実 durability・実機の証拠として扱わない。
- doc 間の relations は双方向整合を保ち、既存の legacy id 形式
  finding（repo-wide migration 対象）を新規に増やさない。
- `docs/*/README.md` 管理 index は手編集せず、doc-maintenance の
  index 再生成で更新する。

## Acceptance Criteria

Issue 本文「受入条件」4 行の照合結果（詳細は
`docs/impl/ir/rcd-02-residual-closeout.md` と
`evidence/sv116/rcd-02-closeout-verification.md`）:

- [x] AC-1 Kill/Power-loss 相当/Media short write/journal short
  write/sync failure/DirSync unsupported/Permission/DiskFull/
  indeterminate publish を Fault Injection — `recovery_review.rs`
  16 tests・`vs_c.rs` 16 tests・`staged_publish.rs` 7 tests・
  `vs_d.rs` 22 tests で実測（`rcd02_fault_matrix_*`・
  `rcd02_unsynced_loss_*`・`publish_faultable_*` 等）。真の
  power-loss は非破壊的に自動化不能のため NotRun（gate 棚卸し
  参照）。
- [x] AC-2 Verified Prefix 以外を Complete としない・別 Start 後に
  同じ Session/Stream を二重 Commit しない —
  `rcd02_review_adopt_verified_prefix_and_idempotent`・
  `rcd02_finalize_record_complete_with_tail_downgrades_to_partial`・
  `rcd02_unsynced_loss_*`・`vs_d_crash_before_commit_retry`・
  `vs_d_crash_after_commit_retry_no_dup`・
  `vs_d_retry_after_staging_drift_is_already_applied`・
  `rcd02pa_commit_save_reload_undo_restore_ids` 等で実測。
- [x] AC-3 CRC32 と SHA256 の用途を区別し、Legacy v5/Asset/Project
  Schema migration との相互参照、旧 Snapshot/HEAD 保護 —
  `IntegrityDigest`（CRC32 fold、integrity/idempotency 専用）と
  `verified_sha256`（content identity）の型・命名分離
  （`rcd02pa_commit_manifest_carries_verified_sha256`・
  `DigestScheme`）、`rcd02_blob_digest.rs` 10 tests で
  sha256 namespace/legacy CRC32 dual-scheme/opaque fail-closed を
  実測。v5/Asset/Project Schema migration・旧 Snapshot/HEAD 保護は
  ALIB-02 側（synvelis#86）と project 側 HEAD CAS（sv#84 gate）に
  またがるため該当行を分離記録。
- [x] AC-4 OS/FS 別 Durability と Recovery 可能範囲を実測し、
  NotRun/Unsupported を Pass 扱いしない — Windows/NTFS real-FS
  lane（`mise run test-recording-real-fs`、PR #320）で
  rename/dir sync/publish/unsynced tail を実測、DirSync
  `Unsupported` を偽装しないことを確認。真の power-loss・
  Linux/macOS real-FS は NotRun として所有 gate を記名。
- [x] 簿記 drift 修正（slice 受入条件）— task3 status を
  `done` に修正し `docs/tasks/README.md` を再生成。
  `docs/test-specs/README.md` の managed index row は
  `approved`（正本 frontmatter と一致）に揃ったが、
  generator 管理外の保存 tail 領域に stale `proposed`
  重複行が残存（PR #212 由来・verbatim 保存で修復手段
  なし）— 残存 finding として PR 本文に記録。

## Deferred Design Concerns

- 真の power-loss（電源断・OS crash 相当）の実機 durability |
  非破壊的に自動化不能 | 実機電源断試験 | process kill/fixture
  truncate を power-loss 証拠と誤認する簿記 drift。
- Linux/macOS 等の他 OS real-FS matrix | Windows/NTFS のみ実測済み
  | 該当 OS lane 整備 | OS 差の未検証を Supported と誤記。
- 実 device 録音・disconnect/LowDisk 実 fault | headless/fixture のみ
  | synvelis#115 | mock 録音を実 device 証拠と誤認。
- 製品 GUI・Comp/Take/Recovery E2E | headless slice は GUI 非対象
  | synvelis#118 | headless PASS を実 GUI PASS と誤認。
- consumer 照合（#86 verified blob 実接続・AudioLens 比較） |
  recording publish 側のみ | `scarecrow173/ruviform#294` | Synvelis
  側証拠の過剰一般化。
- project 側 HEAD CAS 参照進化 | recording 側 publish は接続済み
  （PR #305） | synvelis#84 | project 側残件の未追跡。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/116>
  （RCD-02 tracker — auto-close しない。GUI=sv#118・consumer=
  `scarecrow173/ruviform#294` 等の外部 gate が残存）。
- 統合済み実装: PR #210（DurabilityEvidence/RecoveryReview/
  CommitReceipt/IntegrityDigest + `tests/recovery_review.rs`）・
  PR #239（verified_sha256/MediaReferenceProbe/saved-HEAD）・
  PR #275（torn-append/finalize/AlreadyApplied e2e）・PR #305
  （StagedFile/SealedFile TOCTOU）・PR #320（unsynced-write loss
  fixture + Windows/NTFS real-FS probe + `test-recording-real-fs`
  lane）・Task4 Blob SHA-256 promotion
  （`crates/synvelis-player/tests/rcd02_blob_digest.rs`）。
- 関連 spec: `rcd-02-durability-evidence-recovery-review`・
  `rcd-02-stagedfile-publish-migration`・
  `rcd-02-unsynced-loss-real-fs-probe`・`rcd-02-blob-sha256`。
- 証拠: `evidence/reports/
  rcd-02-durability-recovery-review-ledger.md`・
  `evidence/sv116/rcd-02-blob-sha256-ledger.md`・
  `evidence/sv116/rcd-02-closeout-verification.md`。
