# Evidence — synvelis#86 ALIB-02 residual closeout: DoD 照合・canonical lane 実測・残存 gate 棚卸し

Bookkeeping closeout for Issue #86 (ALIB-02: Streaming Verified Import,
SHA-256 identity, v5 snapshot compat migration, Project CAS). The
production implementation merged earlier via PR #220 / #236 / #253 /
#256 / #326 / #328; this slice adds **no production code**. It maps each
Issue acceptance/DoD line to the real fixtures, test names, and
canonical lane results, and records the external gates that remain
`NotRun` with their owning Issues.

`Pass` below means *the stated headless lane only* — it never implies
real OS/FS durability differences, real GUI/audio device, real network
provider, RT-thread, or cross-consumer acceptance. Those lanes stay
`NotRun`/`Unsupported` with named owners.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `scarecrow173/sv86-closeout` |
| Base commit | `36c2254` (origin/main at dispatch; later main `4e5907a` touches no file in this diff) |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.0 (b940084d7); MSRV lane rustc 1.89.0 via `mise exec rust@1.89.0` |
| Change surface | docs/evidence only — zero `src/` or manifest changes |
| Fixture | `crates/synvelis-project/tests/fixtures/alib02-v5-legacy.{json,blob}` (real v5 wire bytes, see `README.md` in this directory) |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not measured
here; `Unsupported` = capability genuinely unavailable on this host.

## Canonical lane (mise tasks, `--locked` where applicable)

All lanes run via `mise run <task>` in this worktree on 2026-11-12
(JST). Raw per-lane logs were captured outside the repo during the run.

| Command | Expected | Observed | Status |
| --- | --- | --- | --- |
| `mise run fmt-check` | exit 0 | exit 0 (2026-11-12 04:42 JST) | Pass |
| `mise run check` | exit 0 | exit 0 | Pass |
| `mise run lint` | exit 0 (`-D warnings`) | exit 0 | Pass |
| `mise run test` | exit 0, workspace tests pass | exit 0 — 2,091 tests passed, 0 failed across 202 test binaries | Pass |
| `mise run build` | exit 0 | exit 0 (`dev` profile, 1m18s) | Pass |
| `mise run test-rt` | exit 0 (`rt_` filter) | exit 0 — 104 `rt_` tests passed | Pass |
| `mise run test-msrv` | exit 0 (rust 1.89 workspace tests) | exit 0 — 2,091 tests passed, 0 failed (rustc 1.89.0) | Pass |

ALIB-02-relevant test binaries inside `mise run test` (all `ok`,
0 failed):

| Binary | Tests | Result |
| --- | --- | --- |
| `synvelis-library/tests/import.rs` | 21 | 21 passed |
| `synvelis-library/tests/import_recovery.rs` | 6 | 6 passed |
| `synvelis-player/tests/verified_import.rs` | 8 | 8 passed |
| `synvelis-project/tests/alib02_digest.rs` | 5 | 5 passed |
| `synvelis-project/tests/alib02_v5_real.rs` | 8 | 8 passed |

## Issue #86 受入条件 (Acceptance) — 1:1 照合

Issue body lists 5 acceptance lines (identical under `### Acceptance`
and `### 受入条件`). Mapping uses real test/fixture names verified by
`rg`/`cargo test` enumeration on this branch.

| # | Issue line (要約) | Evidence | Status |
| --- | --- | --- | --- |
| AC-1 | Local WAV Golden/ID/PCM/Save/Reload/Undo/Redo/Record/Playback/E2E 維持 + bounded memory | `import.rs::bounded_memory_streaming_with_tiny_chunks`, `import_recovery.rs` (6: save/load replay, save failure, undo re-import, stale→target-loss, multi-track, legacy namespace coexist), `verified_import.rs` (8: CAS/Undo, Saved HEAD→load→replay, resume), workspace `mise run test` Pass. Record/Playback/E2E at app/GUI layer → synvelis#114・#97 gate, NotRun | Pass (headless) / NotRun (GUI-E2E) |
| AC-2 | 同一 ImportId replay・重複/fault/Crash 直前直後・StaleRevision・対象消失・他 Track 誤配置防止 | `import.rs` 21 tests incl. `same_import_id_different_content_is_conflict`, `stale_revision_rejected_then_explicit_rebase_converges`, `publish_failure_resumes_from_manifest`, `indeterminate_publish_is_reconciled_not_misreported`, `directory_sync_failure_is_recorded_not_hidden`, `orphan_gc_protects_reachable_and_deletes_only_orphans`, `subprocess_crash_during_publish_or_commit_resumes_across_processes`, `cancel_*_is_typed_and_same_import_resumes` (4 variants); `import_recovery.rs::multi_track_placement_lands_on_intended_lane_only`, `stale_then_target_gone_rejects_and_orphan_protected`. 複数 Window 誤配置 (GUI) → synvelis#114 gate, NotRun | Pass (headless) / NotRun (multi-window GUI) |
| AC-3 | SHA-256 と旧 blob namespace 非衝突・v5 lossless/Degraded 読み込み・Provenance/ID/opaque 維持 | `alib02_digest.rs` 5 tests; `alib02_v5_real.rs` 8 tests on real v5 fixture `alib02-v5-legacy.{json,blob}`; `import.rs::sha256_namespace_cannot_collide_with_legacy_flat`; `import_recovery.rs::legacy_crc_blob_coexists_with_sha256_namespace`; `alib02_v5_real.rs::crc_collision_assets_stay_separate_and_weak`, `missing_blob_classifies_degraded_not_rejected`; `verified_import.rs::opaque_digest_fails_closed` | Pass |
| AC-4 | Staging→Verify→Published→LiveCommit→Save/HEAD 各障害点注入・Indeterminate/DirSync 正直分類・Orphan 保護/明示 GC/復旧 | `import.rs` fault-injection tests listed under AC-2 (`publish_failure_resumes_from_manifest`, `indeterminate_publish_is_reconciled_not_misreported`, `directory_sync_failure_is_recorded_not_hidden`, `save_failure_preserves_commit_and_recovers` in `import_recovery.rs`, `orphan_gc_protects_reachable_and_deletes_only_orphans`, `subprocess_crash_during_publish_or_commit_resumes_across_processes`) | Pass |
| AC-5 | Linux/Windows/macOS FS 差・RT 非阻害・AudioLens baseline・Ruviform IO-01(#84) との DAG/責務差の証拠記録 | Windows/NTFS 実測 + `mise run test-rt` Pass + 責務差の文書記録 (本ファイル・IR)。Linux/macOS 実 FS → synvelis#91 NotRun; AudioLens/cross-consumer → ruviform#294 NotRun; 実 provider/remote LFS → synvelis#331 NotRun | Partial — 外部 gate 残存 |

## Issue #86 残実装 DoD (2026-11-10 Close判定) — 1:1 照合

| # | DoD 行 (要約) | Evidence | Status |
| --- | --- | --- | --- |
| DoD(1) | v5 旧 Asset/Source/Clip ID・Blob path/旧 CRC を versioned reader と共用・read-only 原本保存・安全移行・再読込 | PR #253 (digest 分類・v10→v11 step・`import_recovery`) + PR #328 (実 v5 fixture・`alib02_v5_real.rs` 8 tests・本 directory の README/emit_v5_fixture.rs). `real_v5_snapshot_decodes_migrates_and_reloads` 他 7 tests Pass | Pass |
| DoD(2) | Import Receipt→ProjectController CAS/Undo→Saved HEAD/Recovery→Offline Playback 実結合 | PR #326: `synvelis-player::verified_import::import_verified_local` + `blob::open_blob_verified` digest-kind 分岐; `verified_import.rs` 8 tests (CAS/Undo 収束・Saved HEAD→load→再生・中断 job inspect→resume→再生) Pass. 実 device/GUI 再生 → synvelis#114 gate NotRun | Pass (headless) / NotRun (device) |
| DoD(3) | Duplicate ImportId/StaleRevision/Plugin Opaque/欠落 Asset/Cancel/Crash を独立 fixture で拒否・復旧 | `same_import_id_different_content_is_conflict`, `stale_revision_rejected_then_explicit_rebase_converges`, `opaque_digest_fails_closed`, `missing_blob_classifies_degraded_not_rejected`, `cancel_mid_stream/cancel_during_verify/cancel_during_publish/cancel_at_commit_boundary` (typed + same-import resume), `subprocess_crash_during_publish_or_commit_resumes_across_processes` — all Pass | Pass |
| DoD(4) | 新旧 schema round trip と Ruviform staged publish/RT 非回帰 | `v5_snapshot_with_crc_digests_migrates_losslessly`, `real_v5_snapshot_decodes_migrates_and_reloads`, `saved_head_load_replays`; `mise run test-rt` Pass (104 rt_ tests); `StagedBlobPublisher` は ruviform `StagedFile` 経由で独自実装なし | Pass |

## 残存外部 gate (NotRun — 所有 Issue 記名)

| Gate | 内容 | Owner | Status |
| --- | --- | --- | --- |
| 実 OS/FS durability・DirSync OS 差・Linux/macOS 実測 | 実 filesystem 挙動の OS 差 | synvelis#91 | NotRun |
| 統合試験 (app 層 E2E) | VCS/統合 acceptance | synvelis#97 | NotRun |
| 実 GPUI / GUI / 実 audio device | 実 GUI 配線・device 再生・複数 Window | synvelis#114 | NotRun |
| real provider / 実ネットワーク / remote LFS | 実 fetch・remote LFS object | synvelis#331 | NotRun |
| cross-consumer / AudioLens baseline / f64 比較 | 別 consumer での比較 gate | ruviform#294 | NotRun |

Mock FS・scripted double・headless fixture の PASS は上記 gate の
証拠に昇格しない。

## Doc 監査 (実走結果)

- `node .agents/skills/doc-status/scripts/doc_status.js lint --type all`:
  repo-wide 647 findings (大半は legacy invalid-id-format 等の
  baseline migration 対象)。本 slice の新規・変更ファイルに対する
  新規 finding は `closeout-verification.md` への
  broken-relation-link 参照切れのみで、本ファイル作成により解消。
  `index-missing-entry`/`orphan-*` は新規 doc 5 件分 — `docs/*/README.md`
  は merge 時に Coordinator が再生成する既知の一時状態。
  `docs/specs/README.md` の `index-metadata-mismatch`
  (alib-02-v5-snapshot-compat: approved→implemented) も同再生成で解消。
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js plan
  --type all`: deterministic repair は index rebuild のみ
  (designs/plans/specs/tasks/test-specs README)。apply は merge 時の
  index 再生成に委ね、本 slice では未実施。
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js --dir
  docs/impl/ir`: 本 slice の IR (`alib-02-residual-closeout.md`) に
  error なし。`alib-02-v5-snapshot-compat-slice.md` の missing-section
  error・`alib-02-v5-snapshot-compat.md` の metadata error・
  `issue-86-verified-import-wiring-slice.md` の missing-section error
  は HEAD 時点の既存 baseline であり本 slice の変更では増やしていない。

## 再現手順

```sh
# worktree root (C:/Users/akiyoshi/orca/workspaces/synvelis/sv86-closeout)
mise install
mise run fmt-check   # cargo fmt --all -- --check
mise run check       # cargo check --workspace --all-targets
mise run lint        # cargo clippy --workspace --all-targets -- -D warnings
mise run test        # cargo test --workspace --locked
mise run build       # cargo build --workspace --locked
mise run test-rt     # cargo test --workspace --locked rt_
mise run test-msrv   # mise exec rust@1.89.0 -- cargo test --workspace --locked

# ALIB-02 focused binaries
cargo test -p synvelis-library --test import            # 21 tests
cargo test -p synvelis-library --test import_recovery   # 6 tests
cargo test -p synvelis-player --test verified_import    # 8 tests
cargo test -p synvelis-project --test alib02_digest     # 5 tests
cargo test -p synvelis-project --test alib02_v5_real    # 8 tests (real v5 fixture)
```

照合に使った列挙コマンド:

```sh
rg -c "#\[test\]" crates/synvelis-library/tests/import.rs \
  crates/synvelis-library/tests/import_recovery.rs \
  crates/synvelis-player/tests/verified_import.rs \
  crates/synvelis-project/tests/alib02_digest.rs \
  crates/synvelis-project/tests/alib02_v5_real.rs
ls crates/synvelis-project/tests/fixtures/
```
