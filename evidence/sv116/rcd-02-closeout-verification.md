# Evidence — synvelis#116 RCD-02 residual closeout: AC 照合・canonical lane 実測・gate 棚卸し・簿記 drift 修正

Bookkeeping closeout for Issue #116 (RCD-02: Recording Durability
Evidence / Recovery Review / Commit Receipt / verified-prefix
SHA-256 / StagedFile publish migration). The production
implementation merged earlier via PR #210 / #239 / #275 / #305 /
#320 and the Task4 Blob SHA-256 promotion; this slice adds **no
production code, no new test, no new fixture**. It maps each
Issue acceptance line to the real test names, existing ledger
fixture references, and canonical lane results, records the
external gates that remain `NotRun`/`Deferred` with their owning
Issues, and fixes bookkeeping drift (task3 status, managed doc
indexes).

`Pass` below means *the stated headless/fixture lane only* — it
never implies true power-loss durability, all-OS filesystem
behavior, real audio device, real GUI, external provider/network,
cross-consumer acceptance, or real RT-thread measurement. Those
lanes stay `NotRun`/`Unsupported`/`Deferred` with named owners.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `scarecrow173/sv116-rcd02-closeout` |
| Base commit | `dcac0c1dba7c1fa446f9d0a02c1e0f83e84b6a5d` (= `origin/main` at dispatch and at final verification; `git fetch origin main` confirmed no newer main) |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1 (58cf8f9c3 2026-10-13) / cargo 1.99.1 via mise; MSRV lane (rust 1.89) is Daily Full CI delegated — NotRun |
| Change surface | docs/evidence only — zero `src/`, manifest, or `Cargo.lock` changes |
| Prior evidence consolidated (not re-derived) | `evidence/reports/rcd-02-durability-recovery-review-ledger.md` (DUR/FAULT/REVIEW/RECEIPT/SHA/PROBE/REALFS sections) and `evidence/sv116/rcd-02-blob-sha256-ledger.md` (RCD02-BS-01〜07) |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not
measured here (or is non-automatable); `Unsupported` = capability
genuinely unavailable on this host; `Deferred` = owned by another
Issue/gate.

## Canonical lane (mise tasks)

All lanes run via `mise run <task>` in this worktree on
2026-11-15 (JST). Only lanes actually executed are listed as
`Pass`; unexecuted lanes are `NotRun`.

| Command | Expected | Observed | Status |
| --- | --- | --- | --- |
| `mise run fmt-check` | exit 0 | exit 0 | Pass |
| `mise exec -- prek run --all-files` | exit 0 | exit 0 (all hooks Passed) | Pass |
| `mise run test synvelis-recording` | exit 0 | exit 0 — all test binaries ok incl. `recovery_review` 16, `staged_publish` 7, `vs_c` 16, `vs_d` 22, `project_adapter` 13 | Pass |
| `mise run test synvelis-player` | exit 0 | exit 0 — all test binaries ok incl. `rcd02_blob_digest` 10, `verified_import` 8 | Pass |
| `mise run test synvelis-project` | exit 0 | exit 0 — all test binaries ok | Pass |
| `mise run test-rt` | exit 0 (`rt_` filter) | exit 0 — 123 `rt_`-matching tests passed, 0 failed across workspace test binaries | Pass |
| `mise run test-recording-real-fs` | exit 0 (opt-in real NTFS lane) | exit 0 — `realfs_probe_fs_capabilities` + `realfs_unsynced_tail_survives_process_kill` 2 tests ok on Windows/NTFS (opt-in `#[ignore]` lane, ~30s compile + ~1s run) | Pass |
| `mise run check` / `lint` / `build` / `test` / `test-msrv` / `*-full` | — | not run in this slice (docs+evidence only; `*-msrv`/`*-full` are Daily Full CI delegated) | NotRun |

RCD-02-relevant test binaries (counts verified by `#[test]`
enumeration on this branch; all `ok`, 0 failed inside the package
lanes above):

| Binary | Tests | Notes |
| --- | --- | --- |
| `crates/synvelis-recording/tests/recovery_review.rs` | 16 | DurabilityEvidence / RecoveryReview / CommitReceipt / unsynced-loss fixtures |
| `crates/synvelis-recording/tests/staged_publish.rs` | 7 | `publish_atomic_*` / `publish_faultable_*` on Ruviform `StagedFile`/`SealedFile` pipeline |
| `crates/synvelis-recording/tests/vs_c.rs` | 16 | VS-C journal/staging regression (`vsc_*`) |
| `crates/synvelis-recording/tests/vs_d.rs` | 22 | VS-D commit/idempotency/undo/reload (`vs_d_*`) |
| `crates/synvelis-recording/tests/project_adapter.rs` | 13 | `rcd02pa_*` verified_sha256 / MediaReferenceProbe / saved-HEAD adapter |
| `crates/synvelis-player/tests/rcd02_blob_digest.rs` | 10 | sha256 blob namespace / dual-scheme / observation-reconcile |
| `crates/recording-poc/tests/real_fs_probe.rs` | 2 | both `#[ignore]` — opt-in `test-recording-real-fs` lane only |

## Issue #116 受入条件 — 1:1 照合

Issue body lists 4 acceptance lines under 受入条件. Root evidence
is limited to real test names verified by enumeration and the two
existing ledgers above (consolidated, not re-derived).

| # | Issue line (要約) | Evidence | Status |
| --- | --- | --- | --- |
| AC-1 | Kill/Power-loss 相当/Media short write/journal short write/sync failure/DirSync unsupported/Permission/DiskFull/indeterminate publish を Fault Injection | `recovery_review.rs`: `rcd02_fault_matrix_write_sync_permission_diskfull`（permission/diskfull/sync failure）, `rcd02_fault_matrix_journal_torn_tail_review_adopt`, `rcd02_fault_matrix_journal_short_write_torn_append`, `rcd02_evidence_dir_sync_unsupported_is_honest`, `rcd02_publish_indeterminate_adopt_recovers`, `rcd02_unsynced_loss_truncates_to_sync_frontier`, `rcd02_unsynced_loss_asymmetric_tail`; `staged_publish.rs` `publish_faultable_*` 4 tests（Denied/SyncFail/Indeterminate on StagedFile pipeline）; `vs_c.rs` `vsc_partial_write_stream_failed`・`vsc_failed_chunk_preserves_validated_prefix`・`vsc_sync_failed_distinct_from_write`・`vsc_disk_full_byte_limit`・`vsc_journal_torn_tail_and_checksum_and_gap`; kill 相当（finalize なし drop + torn tail）= `rcd02_fault_matrix_journal_torn_tail_review_adopt`・`rcd02_commit_receipt_adopted_partial_finalize_record_absent` | Pass (headless/injected fault on real temp FS) / 真の power-loss = NotRun（非破壊的に自動化不能 — 下記 gate 棚卸し） |
| AC-2 | Verified Prefix 以外を Complete としない。別 Start 後に同じ Session/Stream を二重 Commit しない | `rcd02_review_adopt_verified_prefix_and_idempotent`（Adopt は verified prefix のみ・冪等）, `rcd02_finalize_record_complete_with_tail_downgrades_to_partial`（journal Complete でも tail 残存なら Partial 降格）, `rcd02_unsynced_loss_*`（喪失 tail を Complete に昇格しない）, `vs_d_crash_before_commit_retry`・`vs_d_crash_after_commit_retry_no_dup`・`vs_d_retry_after_staging_drift_is_already_applied`・`vs_d_recommit_after_undo_recovers`（別 Start/retry で `AlreadyApplied`・duplicate take/asset/segment なし）, `rcd02pa_commit_save_reload_undo_restore_ids`（commit→save→reload→undo/redo→AlreadyApplied e2e）。double-commit 拒否は `applied_commits` registry + commit-manifest + commit.json marker の三重冪等 | Pass (headless) |
| AC-3 | CRC32 と SHA256 の用途を区別し、Legacy v5/Asset/Project Schema migration との相互参照、旧 Snapshot/HEAD 保護 | RCD-02 側責務分: `IntegrityDigest`（CRC32 fold、integrity/idempotency 専用）と `verified_sha256`（content identity、`sha256:<64hex>`）が型・命名・`digest_scheme` で分離 — `rcd02pa_finalize_records_verified_prefix_sha256`・`rcd02pa_recovery_adopt_records_verified_prefix_sha256`・`rcd02pa_commit_manifest_carries_verified_sha256`; `rcd02_blob_digest.rs` 10 tests（`publish_writes_sha256_blob_and_receipt`・`republish_is_idempotent_and_reuses_verified_blob`・`missing_sha256_blob_is_republished`・`tampered_sha256_blob_fails_closed`・`legacy_crc32_receipt_keeps_flat_blob`・`opaque_receipt_digest_fails_closed`・`indeterminate_publish_is_reconciled_by_observation`・`published_effect_failure_is_reconciled_by_observation`・`unlanded_publish_retries_once_then_fails_closed`・`report_carries_provenance_fields`）。横断部分（Legacy v5/Asset/Project Schema migration 相互参照・旧 Snapshot/HEAD 保護）は ALIB-02（synvelis#86、merge 済み PR #220/#236/#253/#256/#326/#328 + `alib02_v5_real.rs` 実 v5 fixture）と project 側 HEAD CAS（synvelis#84 gate）にまたがる — RCD-02 側の分離責務のみ Pass、横断部分は gate 記名で分離 | Pass (RCD-02 分) / 横断部分 = Deferred（#86 merge 済み・#84 gate 残存） |
| AC-4 | OS/FS 別 Durability と Recovery 可能範囲を実測し、NotRun/Unsupported を Pass 扱いしない | Windows/NTFS real-FS lane `mise run test-recording-real-fs`（PR #320、opt-in `recording-poc` `realfs` subcommand + `real_fs_probe.rs` 2 tests `#[ignore]`）で rename 可視性/原子性・create_new no-replace・StagedFile publish semantics・dir sync capability・process kill + unsynced tail を実測。DirSync `Unsupported`（Windows/NTFS で `dir_open`/`dir_sync` が `PermissionDenied` → `detect_dir_sync=Unsupported`）を偽装しないことを `rcd02_evidence_dir_sync_unsupported_is_honest` と real-FS probe で確認。他 OS real-FS・真の power-loss は NotRun | Pass (Windows/NTFS 実測分) / NotRun（他 OS・power-loss） |

## Slice 受入条件（Coordinator 確定）— 簿記照合

| # | Slice 条件 | Evidence | Status |
| --- | --- | --- | --- |
| S-1 | Doc chain 7 files（`rcd-02-residual-closeout` slug）+ `docs/*/README.md` index regen | `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/rcd-02-residual-closeout*.md` 作成、`doc_maintenance.js index apply` で 6 index 再生成（`docs/impl/exp` は `.jsonl` のみ管理対象で `.md` exp は index 非掲載 — 既存慣行どおり） | Pass |
| S-2 | `evidence/sv116/rcd-02-closeout-verification.md` に AC 1:1 map・lane 実測・gate 棚卸し | 本ファイル | Pass |
| S-3 | task3 status `in-progress`→`done` + `docs/tasks/README.md` regen | `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md` frontmatter `status: "done"`（PR #305 merge 済みで done 相当。Done When/Verification checkbox は当時の作業記録として不変）、`docs/tasks/README.md` managed row が `done` | Pass |
| S-4 | `docs/test-specs/README.md` dup row を regen で正本側に揃える | managed index row（line 100）は `approved`（正本 frontmatter と一致）に揃った。**残存 finding**: marker 管理外の保存 tail 領域（line 230 付近、PR #212 由来の重複 table — `docs/designs/README.md` にも同型 tail あり）に stale `proposed` 行が残る。generator は tail を verbatim 保存し `index plan` は in-sync を返すため doc-maintenance whitelist では修復不能。手編集は禁止のため未対応作業として記録 | Partial — managed 領域 Pass / tail 領域は残存 finding |
| S-5 | review-protocol proportionate・blocker 0 | docs+evidence 小 slice の self-review（diff 精査 + doc audit）— PR 本文に記録 | Pass |
| S-6 | honesty: 未測定 lane を Pass と書かない | 本ファイルの lane 表・gate 棚卸しは実行済み lane のみ Pass | Pass |

## 残存外部 gate (NotRun/Deferred — 所有 Issue 記名)

| Gate | 内容 | Owner | Status |
| --- | --- | --- | --- |
| 真の power-loss（電源断・OS crash 相当） | 非破壊的に自動化不能。process kill は dirty page を保持し power-loss 喪失を再現しない（RCD-02-REALFS 実測済み） | 本 slice 対象外（実機試験） | NotRun |
| 他 OS・他 FS real-FS（Linux/macOS 等） | Windows/NTFS のみ実測済み | synvelis#91 系の OS 横断 gate | NotRun |
| 実 device 録音・disconnect/LowDisk 実 fault | headless/fixture のみ | synvelis#115 | Deferred |
| 製品 GUI・Comp/Take/Recovery E2E | headless slice は GUI 非対象 | synvelis#118 | Deferred |
| consumer 照合（#86 verified blob 実接続・AudioLens 比較） | recording publish 側のみ | `scarecrow173/ruviform#294` | NotRun |
| project 側 HEAD CAS 参照進化 | recording 側 publish は PR #305 で `StagedFile`/`SealedFile` 経由に接続済み | synvelis#84 | Deferred |
| 統合/app 層 acceptance・実 GPUI/audio device | app 層 E2E | synvelis#97・#114 | NotRun |
| real provider/remote LFS/network | 実 fetch 系 | synvelis#331 | NotRun |
| `*-msrv`・`*-full` 系 lane | Daily Full CI 委譲（ユーザー指定により本 slice では実行しない） | Daily Full CI | NotRun |
| consumer precision（AudioLens 系） | 別 consumer gate | synvelis#219 | NotRun |

Mock FS・scripted double・headless fixture・truncate モデルの
PASS は上記 gate の証拠に昇格しない。process kill の実測結果を
power-loss 証拠としない（RCD-02-REALFS ledger どおり）。

## Doc 監査 (実走結果)

- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan/apply`: 新規 doc 7 件 + task3 status 変更に伴い
  `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/README.md`
  を再生成（blocked なし）。`docs/impl/exp/README.md` は `.jsonl`
  のみ管理対象のため `.md` exp 追加では変化なし（`rcd-02-blob-
  sha256.md` 等の既存 `.md` exp と同じ扱い）。全対象 dir で
  `index plan` = in-sync を確認。
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan --dir docs/test-specs`: verdict `in-sync`。
  tail 領域の stale `proposed` 行は managed index 外のため
  plan に現れない（S-4 の残存 finding）。
- `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type all --json`: repo-wide 698 blocking findings（大半は
  legacy `invalid-id-*` 等の baseline migration 対象）。
  本 slice の新規・変更ファイル
  （`docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  rcd-02-residual-closeout*`・`docs/tasks/rcd-02-task3-
  stagedfile-publish-migration.md`・`evidence/sv116/
  rcd-02-closeout-verification.md`）に対する finding は 0 件。
- `node .agents/skills/doc-status/scripts/doc_status.js audit
  --dir docs/test-specs`: 82 blocking findings（全て既存
  baseline — legacy `invalid-id-*`・vcs-* の verifies 系）。
  本 slice の `rcd-02-residual-closeout.md` は finding 0 件
  （evidence file 作成前の初回 audit で一時的に
  `broken-relation-link` が出たが、file 作成後の再実行で
  解消を確認）。
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js --dir
  docs/impl/ir`: exit 0。本 slice の IR
  （`rcd-02-residual-closeout.md`）に error/warning なし。
  出力される error/warning は全て HEAD 時点の既存 baseline
  （`automation-modulation.md`・`autx-*`・`vcs-*` 等の
  missing-section / defers 未解決）であり本 slice では増やして
  いない。

## 再現手順

```sh
# worktree root (G:/orca/workspaces/synvelis/sv116-rcd02-closeout)
mise install
mise run fmt-check        # cargo fmt --all -- --check
mise exec -- prek run --all-files
mise run test synvelis-recording
mise run test synvelis-player
mise run test synvelis-project
mise run test-rt          # cargo test --workspace --locked rt_
mise run test-recording-real-fs   # opt-in: cargo test -p recording-poc --test real_fs_probe -- --ignored
```

照合に使った列挙コマンド（補助証拠）:

```sh
rg -c "#\[test\]" crates/synvelis-recording/tests/{recovery_review,staged_publish,vs_c,vs_d,project_adapter}.rs \
  crates/synvelis-player/tests/rcd02_blob_digest.rs \
  crates/recording-poc/tests/real_fs_probe.rs
git log --oneline -S "pc-c6-e2e-oracle-rel-tr" -- docs/test-specs/README.md   # tail 領域の出所確認（PR #212）
```
