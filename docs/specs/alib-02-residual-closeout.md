---
id: "SPEC-034VMQHlJRjZlqnsqBmhnw"
type: "spec"
status: "implemented"
title: "ALIB-02 residual closeout — Issue #86 DoD 照合・証拠・外部 gate 棚卸し"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/designs/alib-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/alib-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-residual-closeout.md"
      - type: "file"
        path: "evidence/issue-86-v5-migration/closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/plans/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/test-specs/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/alib-02-residual-closeout-design.md"
    - "docs/plans/alib-02-residual-closeout.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/specs/issue-86-verified-import-wiring.md"
    - "docs/impl/ir/alib-02-streaming-verified-import.md"
    - "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    - "docs/impl/ir/issue-86-verified-import-wiring-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/issue-86-v5-migration/README.md"
    - "evidence/issue-86-v5-migration/closeout-verification.md"
    - "evidence/issue-86-verified-import-wiring/verification-ledger.md"
  defers: []
  deferred-by: []
---

# ALIB-02 residual closeout — Issue #86 DoD 照合・証拠・外部 gate 棚卸し

## What Should Be Built

Issue #86（ALIB-02: Streaming Verified Import・SHA-256 識別子・v5
Snapshot 互換移行・Project CAS）の実装は PR #220 / #236 / #253 /
#256 / #326 / #328 で main に統合済みである。本 spec が定めるのは
**簿記クローズアウト**のみであり、新規のプロダクション実装は
含まない。

- Issue 本文の受入条件 5 行と残実装 DoD 4 行を、実在する
  fixture・テスト名と 1:1 で照合し、[x]/[ ] で記録する。
- canonical lane（`mise run fmt-check` / `check` / `lint` / `test` /
  `build` / `test-rt` / `test-msrv`）を現行 main で実走し、
  Pass/Fail を記録する。
- 外部 gate（実 OS/FS・実ネットワーク・実 GUI・AudioLens 連携・
  Ruviform consumer gate）は `NotRun` とし、所有 Issue を記名する。
  mock/double/fixture の PASS を実機証拠に昇格しない。
- doc status の簿記を実態に整合させ（status・relations・実測値）、
  Implementation Record `docs/impl/ir/alib-02-residual-closeout.md`
  と evidence `evidence/issue-86-v5-migration/closeout-verification.md`
  に結果を固定する。

## Why Now

PR #326（DoD(2) consumer 配線）と PR #328（DoD(1) 実 v5 fixture）で
残実装 DoD の headless 分は統合済みだが、Issue の受入条件/DoD 行と
実測証拠の対応表・外部 gate の所在・doc status が一箫所に
集約されていない。Issue は外部 gate が残るため open 継続が前提であり、
「何が headless で証明済みで、何がどの Issue の gate か」を
canonical doc に固定することで、後続の close 判定と並行 Worker の
衝突を防ぐ。

## Users and Value

- **Issue #86 owner / reviewer**: 受入条件各行の証拠所在と外部 gate の
  owner が一目で分かり、close 判定の材料になる。
- **後続 Worker / 保守者**: どの doc・fixture・lane が ALIB-02 の
  何を証明するかの正本にたどり着ける。
- **doc-driven-dev 監査**: status/relations が実態と一致し、
  stale-upstream・orphan の誤検出を減らす。

## Scope

### In Scope

- `docs/` 配下の ALIB-02 / Issue #86 関連 doc chain の status・
  relations・checkbox の実態整合（編集許可範囲に限定）。
- 新規 doc chain: 本 spec・design・plan・task・IR
  （`alib-02-residual-closeout*`）。
- `evidence/issue-86-v5-migration/` への lane 結果・再現手順の追記。
- canonical mise lane の実走と結果記録、doc_status/doc_maintenance
  監査の実走。
- Issue #86 への進捗コメントと PR 作成（Issue の close・merge は
  行わない）。

### Out of Scope

- プロダクションコード（`src/`）・Ruviform IO・Cargo/mise/workflow
  定義の変更。
- 未検証バイトの verified 昇格、RT 契約・legacy CRC 経路の変更。
- 外部 gate の NotRun→Pass 昇格、または gate 自体の実施
  （実 OS/FS・実ネットワーク・GUI・AudioLens・cross-consumer）。
- `docs/*/README.md` 等の管理 index の手動編集（merge 時に
  Coordinator が再生成）。
- Issue #86 の編集権限外ファイル（例:
  `docs/specs/issue-86-verified-import-wiring.md`、
  `docs/specs/alib-02-streaming-verified-import.md` の status・
  checkbox）の変更 — 残存簿記として IR/PR に記録する。
- Issue の close、PR の merge。

## Requirements

- Issue 本文の各受入条件/DoD 行を、実在する fixture・テスト名・
  lane 結果と 1:1 で対応付ける。対応する証拠が無い行は [ ] のまま
  理由付きで残す。
- canonical lane は `mise run <task>` 経由で実走し、Pass/Fail を
  分けて記録する。未実行を Pass と書かない。
- 外部 gate は `NotRun`/`Unsupported` とし、所有 Issue
  （synvelis#91・#97・#114・#331、ruviform#294）を記名する。
- mock FS・scripted double・headless fixture の結果を実 durability・
  実機の証拠として扱わない。
- doc 間の relations は双方向整合を保ち、既存の legacy id 形式
  finding（repo-wide migration 対象）を新規に増やさない。

## Acceptance Criteria

Issue 本文「受入条件」5 行の照合結果（詳細は
`docs/impl/ir/alib-02-residual-closeout.md` と
`evidence/issue-86-v5-migration/closeout-verification.md`）:

- [x] AC-1 Local WAV Golden/ID/PCM/Save/Reload/Undo/Redo/Record/
  Playback/E2E 維持 + bounded memory — headless lane で実測
  （`bounded_memory_streaming_with_tiny_chunks`、
  `import_recovery.rs` 6 tests、`verified_import.rs` 8 tests、
  workspace `mise run test` Pass）。app/GUI 層の Record/Playback/
  E2E は外部 gate（synvelis#114・#97）として NotRun を分離記録。
- [x] AC-2 同一 ImportId replay・重複/fault/Crash 直前直後・
  StaleRevision・対象消失・他 Track 誤配置防止 —
  `import.rs` 21 tests + `import_recovery.rs` +
  `verified_import.rs` で実測。複数 Window 誤配置は GUI 領域のため
  synvelis#114 gate として NotRun。
- [x] AC-3 SHA-256 と旧 blob namespace 非衝突・v5
  （正常/不正/欠落/旧 CRC 一致でも bytes 差）の lossless/Degraded
  読み込み・Provenance/ID/opaque bytes 維持 —
  `alib02_digest.rs` 5 tests・`alib02_v5_real.rs` 8 tests・
  `sha256_namespace_cannot_collide_with_legacy_flat`・
  `legacy_crc_blob_coexists_with_sha256_namespace`・
  `crc_collision_assets_stay_separate_and_weak` で実測。
- [x] AC-4 Staging→Verify→Published→LiveCommit→Save/HEAD 各障害点の
  注入・Indeterminate/DirSync の正直な分類・Orphan 保護/明示 GC/
  復旧経路 — `publish_failure_resumes_from_manifest`・
  `indeterminate_publish_is_reconciled_not_misreported`・
  `directory_sync_failure_is_recorded_not_hidden`・
  `save_failure_preserves_commit_and_recovers`・
  `orphan_gc_protects_reachable_and_deletes_only_orphans`・
  `subprocess_crash_during_publish_or_commit_resumes_across_processes`
  等で実測。
- [ ] AC-5 Linux/Windows/macOS FS 差・RT 非阻害・AudioLens baseline・
  Ruviform IO-01(#84) との DAG/責務差の証拠記録 — Windows 実測と
  `test-rt` lane・責務差の文書記録は実施。Linux/macOS 実 FS は
  synvelis#91、AudioLens/cross-consumer は ruviform#294、
  実 provider/remote LFS は synvelis#331 の gate として NotRun。

Issue 本文「残実装 DoD」4 行（2026-11-10 close 判定）の照合結果:

- [x] DoD(1) v5 旧 Asset/Source/Clip ID・Blob path/旧 CRC を既存
  versioned reader と共用・read-only 原本保存・安全移行・再読込 —
  PR #253（digest 分類・v10→v11 step・`import_recovery`）+
  PR #328（実 v5 fixture `alib02-v5-legacy.{json,blob}`・
  `alib02_v5_real.rs` 8 tests・`evidence/issue-86-v5-migration/`）。
- [x] DoD(2) Import Receipt→ProjectController CAS/Undo→Saved
  HEAD/Recovery→Offline Playback 実結合 — PR #326
  （`import_verified_local` + `open_blob_verified` digest-kind 分岐、
  `verified_import.rs` 8 tests: CAS/Undo 収束・Saved HEAD→load→
  再生・中断 job inspect→resume→再生）。実 device/GUI 再生は
  synvelis#114 gate。
- [x] DoD(3) Duplicate ImportId/StaleRevision/Plugin Opaque/欠落
  Asset/Cancel/Crash の独立 fixture 拒否・復旧 — 上記 fixture 群で
  実測（`same_import_id_different_content_is_conflict`・
  `stale_revision_rejected_then_explicit_rebase_converges`・
  `opaque_digest_fails_closed`・`missing_blob_classifies_degraded_not_rejected`・
  `cancel_*_is_typed_and_same_import_resumes`・
  `subprocess_crash_during_publish_or_commit_resumes_across_processes`）。
- [x] DoD(4) 新旧 schema round trip と Ruviform staged publish/RT
  非回帰 — `v5_snapshot_with_crc_digests_migrates_losslessly`・
  `real_v5_snapshot_decodes_migrates_and_reloads`・
  `saved_head_load_replays`・`mise run test-rt` Pass・
  `StagedBlobPublisher` は ruviform `StagedFile` 経由で独自実装なし。

## Deferred Design Concerns

- 実 OS/FS durability・directory sync の OS 差 | headless/Windows
  実測のみ | synvelis#91 の real-FS lane | Unsupported を Supported
  と誤記する簿記 drift。
- 実 GUI・実 audio device・app 層配線の受入 | 本 slice は
  synvelis-player 入口まで | synvelis#114（実 GPUI）・#97
  （統合試験） | headless PASS を実機 PASS と誤認。
- AudioLens native-f64・cross-consumer 比較 | consumer gate は
  別 Issue | ruviform#294 | Synvelis 側証拠の過剰一般化。
- 実 provider・remote LFS・実ネットワーク fetch | fixture は
  local/temp provider のみ | synvelis#331 等 | mock を実
  provider 証拠と誤認。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/86>
  （ALIB-02 本体・2026-11-10 close 判定の残実装 DoD）。
- 統合済み実装: PR #220（library slice）・#236（ALIB-07 fault
  matrix）・#253（v5 digest/migration slice）・#256（ALIB-07
  import cancel/crash slice）・#326（DoD(2) consumer 配線）・
  #328（DoD(1) 実 v5 fixture）。
- 関連 spec: `alib-02-streaming-verified-import`・
  `alib-02-v5-snapshot-compat`・`issue-86-verified-import-wiring`。
- 証拠: `evidence/issue-86-v5-migration/`・
  `evidence/issue-86-verified-import-wiring/verification-ledger.md`。
