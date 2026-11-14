# Evidence — synvelis#86 ALIB-02 bookkeeping closeout: spec status・AC checkbox 突合

Issue #86（ALIB-02: Streaming Verified Import・SHA-256 識別子）の
残存簿記スライス。プロダクションコードの変更は一切なく、
`docs/impl/ir/alib-02-residual-closeout.md`（status `completed`）の
measured-Pass 対応表と Validation lane 実測を正本として、2 spec の
stale status・未チェック acceptance 行を実態に突合した。

`Pass` は「記録された headless lane の実測のみ」を意味し、実 OS/FS・
実 GUI・実 provider・RT・cross-consumer の受入を示さない。それらは
外部 gate（synvelis#91/#97/#114/#331・ruviform#294）として
`NotRun`/tracking のまま残す。

## Anchor — 対象 SHA・環境

- branch: `docs/86-alib02-bookkeeping`（dispatch worktree
  `scarecrow173/sv86-alib02-bookkeeping` から作成）。base は
  dispatch 時点の `origin/main` `142a7ec`（2026-11-14 fetch 済み、
  ebd0636 からの差分は docs/impl のみで本 slice 対象に無関係）。
- 変更面: docs/evidence のみ — `crates/**`・`mise.toml`・
  `.github/workflows/`・`Cargo.toml`/`Cargo.lock` の変更ゼロ。
- 実測の出典: `docs/impl/ir/alib-02-residual-closeout.md`
  Validation 節（2026-11-12 実走、Windows/NTFS、rustc 1.99.0、
  canonical lane 7 本 Pass）と
  `evidence/issue-86-v5-migration/closeout-verification.md`。
  本 slice は新規の cargo lane を実走していない — checkbox 判定は
  closeout slice が記録した実測結果と実在テスト名への突合であり、
  推測で Pass にしていない。
- lane 実測値（closeout slice 記録の転記）: `mise run test` =
  2,091 tests / 202 binaries 全 pass。ALIB-02 関連 5 binaries
  48 tests all ok: `synvelis-library/tests/import.rs` 21、
  `import_recovery.rs` 6、`synvelis-player/tests/verified_import.rs`
  8、`synvelis-project/tests/alib02_digest.rs` 5、
  `alib02_v5_real.rs` 8。

## 変更内容

1. `docs/specs/issue-86-verified-import-wiring.md`: front-matter
   `status: "approved"` → `"implemented"`、`updated` → 2026-11-14。
   wiring impl は PR #326 で main merge 済み
   （`docs/impl/ir/issue-86-verified-import-wiring-slice.md`・
   `evidence/issue-86-verified-import-wiring/verification-ledger.md`）。
   `alib-02-residual-closeout.md` ~L198-203（境界外 drift の扱い）と
   Follow-ups ~L281-284（後続 bookkeeping PR へ委譲）が記録した
   stale status の解消。本文の意味変更なし。
2. `docs/specs/alib-02-streaming-verified-import.md`: Acceptance
   Criteria 13 行のうち実測 Pass が証明できる 11 行を `[x]` 化、
   証明できない 2 行は `[ ]` 維持（行末 HTML コメントで理由を明示）。
   `updated` → 2026-11-14。本文の意味変更なし。
3. `docs/specs/README.md`: doc-maintenance `index apply` で再生成
   （status 変更に伴う in-flight → Settled tier 移動を反映）。
4. 本ファイル新規。

## AC 行 ↔ 実測証拠 突合表

判定基準: 行が列挙する各要素について、実在する test 名と
closeout lane の Pass 記録まで辿れる場合のみ `[x]`。IR 側の対応は
`docs/specs/alib-02-residual-closeout.md` Acceptance Criteria 節
（Issue #86 AC-1〜AC-4 = headless 実測済み、AC-5 = 外部 gate 残存）・
残実装 DoD(1)〜(4) 照合と、`docs/impl/ir/alib-02-streaming-verified-
import.md` Follow-ups（後続 slice 統合済みの実測棚卸し）。

| # | spec AC 行（要約） | 判定 | 根拠（実測 test 名・lane） |
| --- | --- | --- | --- |
| 1 | bounded-memory streaming 実証（小 chunk・大 byte 数） | [x] | `import.rs::bounded_memory_streaming_with_tiny_chunks`（1 MiB を 7 byte chunks で供給・byte-exact publish/commit）、`import_streams_verifies_publishes_and_commits`。64 KiB 固定 chunk の構造的 bounded memory は `src/import.rs` `COPY_CHUNK`。`mise run test` Pass（closeout lane） |
| 2 | 同一 ImportId replay・異 content conflict・重複登録なし | [x] | `replay_same_import_is_idempotent`（reused・同一 ID・件数不変）、`same_import_id_different_content_is_conflict`（IntegrityConflict・applied_imports=1 のまま）。同 lane Pass |
| 3 | crash 直前直後（staging 途中失敗・publish 失敗後 resume）で stage machine 収束 | [x] | staging 途中失敗: `cancel_mid_stream_is_typed_and_same_import_resumes`（stream 途中 cancel → manifest Started → 同一 ImportId で再 stage → commit 収束）。publish 失敗後 resume: `publish_failure_resumes_from_manifest`（Verified から resume・再 stage しない）、`cancel_during_publish_is_typed_and_same_import_resumes`。実 crash: `subprocess_crash_during_publish_or_commit_resumes_across_processes`（別 process が publish ack 喪失/pre-commit で死亡 → manifest+disk のみで cross-process resume 収束）。同 lane Pass |
| 4 | StaleRevision 安全拒否・rebase_import 後冪等収束 | [x] | `stale_revision_rejected_then_explicit_rebase_converges`（typed `StaleRevision{expected,actual}` → 明示 `rebase_import` → commit）、`stale_then_target_gone_rejects_and_orphan_protected`（`import_recovery.rs`）。同 lane Pass |
| 5 | 対象 lane/track 消失・他 track lane 指定を commit 拒否 | [x] | `missing_lane_target_is_rejected_without_clip`・`wrong_track_lane_target_is_rejected`（`import.rs`）、`multi_track_placement_lands_on_intended_lane_only`（`import_recovery.rs`、clip が意図 lane のみに着地）。同 lane Pass |
| 6 | hash/size/staged drift/既存 blob 不一致を IntegrityMismatch/Corrupt 拒否 | [ ] | **部分証明のみ**。hash mismatch: `digest_mismatch_is_integrity_error`（pin 期待 digest ≠ 実測 → `IntegrityMismatch`・未 commit）で実測。size mismatch（pin `expected_size` 照合 `src/import.rs` ~L837-841）・staged drift（`verify_staged` readback 再計測 ~L947-951 → `Corrupt`）・既存 blob 不一致（`verify_existing_blob` ~L961-974 → `IntegrityMismatch`）は実装済みだが、`import_verified`/`import_verified_local` を呼ぶ test（`import.rs` 21・`import_recovery.rs` 6・`git.rs` consumer 系・`verified_import.rs` 8）を `rg` で全件確認した範囲で、これらを個別に注入する test は存在しない。`same_bytes_across_providers_share_one_verified_blob` は既存 blob の**一致**経路（Reused）のみ。行の全要素を実測で証明できないため未チェック維持 |
| 7 | PublishedIndeterminate の stat reconcile 正直分類 | [x] | `indeterminate_publish_is_reconciled_not_misreported`（2 連続 indeterminate → reconcile 不検出 → retry → なお indeterminate → `PublishIndeterminate`、manifest `publish_indeterminate` 永続化、「未公開」誤報も無条件成功もしない）。同 lane Pass |
| 8 | DirSync Unsupported/Failed が receipt/manifest に記録・publish 成否と分離 | [ ] | **部分証明のみ**。`Failed`: `directory_sync_failure_is_recorded_not_hidden`（publish Published + dirsync Failed を outcome `blob.directory_sync` に記録しつつ commit 収束 = publish 成否と分離）で実測。`Unsupported`: `src/import.rs` ~L492-508 の FileSync fallback + ~L452 分類で実装済みだが、Windows/NTFS の実 `StagedBlobPublisher` 経路では `PublishFailureKind::Unsupported`+`NotPublished` が発生せず（ruviform `OsFileOps::validate_publish` は常に Ok、dirsync 相の失敗は `Failed` に分類）、scripted `BlobPublisher` による `Unsupported` 注入 test も存在しない。`DirectorySync::Unsupported` を値として assert する test は全 crate になし。spec 自身の Out of Scope も「Linux/macOS 実機 FS 差・DirSync 可否 = NotRun」と宣言しており、行の全要素を本 host で実測証明できないため未チェック維持 |
| 9 | Orphan blob/staging 保護・gc_orphans が reachability で明示削除 | [x] | `orphan_gc_protects_reachable_and_deletes_only_orphans`（orphan sha256 blob 削除・project 参照/foreign/legacy 温存・committed+applied の staging のみ削除）、`incomplete_job_and_blob_are_protected_from_gc`（published-uncommitted は `ActiveImport`/`Incomplete` で保持）、`stale_then_target_gone_rejects_and_orphan_protected`。同 lane Pass |
| 10 | SHA-256 identity が旧 CRC16 flat namespace と非衝突（型・test） | [x] | `sha256_namespace_cannot_collide_with_legacy_flat`（`import.rs`、flat `blobs/<name>` と `blobs/sha256/<hex>` の別 path 解決・shadow 不成立）、`legacy_crc_blob_coexists_with_sha256_namespace`（`import_recovery.rs`、旧 flat blob 温存 + 新規 import 共存 + GC 非対象）、`crc_collision_assets_stay_separate_and_weak`（`alib02_v5_real.rs`）、型側は `alib02_digest.rs` `digest_kind_classifies_persisted_forms`・`crc_digest_collision_never_becomes_content_addressed`。同 lane Pass |
| 11 | opaque bytes・provenance（pin/provider asset/expected revision）が manifest・receipt・project に維持 | [x] | `import_streams_verifies_publishes_and_commits`（invalid UTF-8/NUL 含む 300,008 bytes が byte-exact round-trip、pin provider/`object_identity`・`applied_imports` receipt の `content_hash`/`asset_id`/`source_id`・`saved()==None` を assert）。manifest 側は `source: PinnedAssetRef`・`expected_project_revision` を永続化（`ImportManifest` ~L181-184）し、`cancel_mid_stream` resume が persisted pin で再 stage、`stale_revision` test が persisted expected revision を強制することで実測的に立証。project 側は `live.assets`/`sources`/`applied_imports` assert。同 lane Pass |
| 12 | CommittedLive 後に SavedRevision/HEAD が進まない・save 失敗が receipt を毀さない | [x] | `import_streams_verifies_publishes_and_commits`（commit 後 `c.saved().is_none()`）、`save_failure_preserves_commit_and_recovers`（`FailOnceStore` で save 失敗 → live commit/receipt 無傷・HEAD 空 → retry save → load → replay 収束）、`receipt_survives_save_load_and_replays`（`import_recovery.rs`）。同 lane Pass |
| 13 | 既存 synvelis-library tests が全て pass | [x] | closeout lane `mise run test` Pass（2,091 tests / 0 failed、`import.rs` 21/21・`import_recovery.rs` 6/6 を含む）。同 lane Pass |

## 残した未チェック行と理由

- **AC 行 6（hash/size/staged drift/既存 blob 不一致の拒否）**:
  `IntegrityMismatch`/`Corrupt` を返す 4 経路のうち hash mismatch
  （pin 期待 digest 照合）のみ `digest_mismatch_is_integrity_error`
  で実測済み。size mismatch（`expected_size` 照合）・staged drift
  （`verify_staged` readback）・既存 blob 不一致（`verify_existing_
  blob`）は `src/import.rs` に実装があるが、現行 suite に個別の
  fault 注入 test がなく実測 Pass を示せない。test 追加は本 slice の
  編集範囲外（`crates/**` 変更禁止）のため、follow-up として記録する
  に留める。
- **AC 行 8（DirSync `Unsupported`/`Failed` の記録と分離）**:
  `Failed` は `directory_sync_failure_is_recorded_not_hidden` で
  実測（publish 成功と dirsync 失敗の分離記録）。`Unsupported` は
  実 publisher が本 host（Windows/NTFS）で当該値を生成する経路が
  なく、scripted 注入 test も未整備。spec Out of Scope が
  「Linux/macOS 実機 FS 差・DirSync 可否 = NotRun」と宣言する領域
  でもあるため、行全体の実測証明は不能と判断し未チェック維持。

## 外部 gate 残件（NotRun/tracking — 本 slice の検証対象外）

Issue #86 AC-5 および headless PASS の実機昇格に関わる外部 gate。
所有 Issue は `alib-02-residual-closeout` spec・IR・
`closeout-verification.md` の記名どおり:

| Gate | Owner | Status |
| --- | --- | --- |
| 実 OS/FS durability・DirSync OS 差・Linux/macOS 実測 | synvelis#91 | NotRun |
| 統合試験（app 層 E2E） | synvelis#97 | NotRun |
| 実 GPUI/GUI/実 audio device・複数 Window | synvelis#114 | NotRun |
| real provider・実ネットワーク・remote LFS | synvelis#331 | NotRun |
| cross-consumer・AudioLens baseline・f64 比較 | ruviform#294 | NotRun |

mock/double/fixture/headless の PASS は上記 gate の証拠に昇格しない。
Issue #86 はこれらが残るため open 継続（本 PR は `Refs #86`、
`Closes` は使わない）。

## 検証結果（本 slice 実走分）

| Command | OS | Toolchain | Exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | rustc 1.99.0（mise） | 0 | Pass（`cargo fmt --all -- --check`、2026-11-14 実走） |
| `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/specs` | Windows | node（repo script） | 0 | verdict=`sync` changes=1 sources=180（planHash `bbe1e19d…`） |
| `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply --dir docs/specs --expect bbe1e19dc4834b5ece12457ef9a5947ef6c1203d3ae87db8ad540d0128d43080` | Windows | node（repo script） | 0 | 1 written / 0 blocked（`docs/specs/README.md` のみ） |
| `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/specs`（rescan） | Windows | node（repo script） | 0 | verdict=`in-sync` changes=0 |
| `node .agents/skills/doc-status/scripts/doc_status.js lint --type spec` | Windows | node（repo script） | 0 | 171 findings。触れたファイルの finding は `alib-02-streaming-verified-import.md` の `traceability-missing-upstream` warning 1 件のみで closeout IR ~L244-245 が記録した既存 baseline（編集境界外の残存 drift）。`issue-86-verified-import-wiring.md`・`docs/specs/README.md` は 0 findings |
| `node .agents/skills/doc-status/scripts/doc_status.js lint --type all` | Windows | node（repo script） | 0 | 885 findings（大半は legacy `invalid-id-format`/`invalid-id-prefix` 等の repo-wide migration baseline）。本 slice の変更ファイルに対する新規 finding なし。`docs/specs/README.md` の index finding 0。`docs/designs/README.md` 44 件・`docs/test-specs/README.md` 39 件の `index-*` finding は本 slice 範囲外 dir の既存 stale index（merge 時の index 再生成対象）であり増やしていない |

cargo `check`/`lint`/`test`/`build` は docs/evidence のみの変更のため
未実施（共通契約「文書のみの変更は軽量チェックが原則」）。ALIB-02
関連 lane の Pass は closeout slice の 2026-11-12 実測を出典として
転記したものであり、本 slice での再実測ではない。

### doc-status lint の新規 finding と既存 baseline の区別

- 新規 finding: **なし**。status 変更（approved→implemented）と
  checkbox 更新は finding を増やさず、index 再生成で
  `index-metadata-mismatch`/tier 移動も解消済み
  （`docs/specs` index finding 0・rescan `in-sync`）。
- 既存 baseline（本 slice では増やしていない・記録のみ）:
  - `alib-02-streaming-verified-import.md` の
    `traceability-missing-upstream`（upstream relation 未宣言）—
    closeout IR が「編集境界外のため残存として記録」と明記した
    既知残件。relations 追加は本 slice の編集許可
    （status/checkbox/`updated` のみ）を超えるため未対処。
  - ALIB-02 doc chain の legacy id 形式
    （`DSN-alib02-streaming-import`・`PLAN-alib02-streaming-import`・
    `TASK-alib02-t1`・`TESTSPEC-alib02`）— repo-wide
    `invalid-id-format`/`invalid-id-prefix` migration baseline。
  - `docs/plans/alib-02-streaming-verified-import.md` の
    `inconsistent-reciprocal-relation`（plan `implements` ↔ spec
    `implemented-by` の非対称）— relations 未変更のため既存 baseline。
  - `docs/designs/README.md`・`docs/test-specs/README.md` の
    `index-*` finding 83 件 — 本 slice 範囲外 dir の stale index。
