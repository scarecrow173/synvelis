---
id: "TSPEC-vzMmDCOZB12ay3I4Zd5yHK"
type: "test-spec"
status: "approved"
title: "VS-D E2E-R3 検証仕様 — Recording Commit・Idempotency・Undo・Reload"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  derived-by: []
  verifies:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  defers: []
  deferred-by: []
---

# VS-D E2E-R3 検証仕様 — Recording Commit・Idempotency・Undo・Reload

## Purpose

spec（VS-D）の Acceptance Matrix VS-001〜015 を実テストへ 1:1 に
落とす正本。全テストは headless・決定的・実デバイス/実 OS fault 非
依存（fault は `faultinj`、crash は in-process 中断と save→reload
で再現）。Mock/実 FS 上の成功を全 OS・電源断・ハードウェア故障の
保証と混同しない。

## Feature

`RecordingOp::CommitRecording`（manifest payload の単一 mutation）、
`commit-manifest.json`（事前 ID 確保・staging 原子公開）、
`applied_commits` registry（AlreadyApplied/IntegrityConflict 判定）、
`commit.json` marker、recoverable 分離（Partial/faulted stream は
commit 対象外）、undo/redo（1 session = 1 undo 単位・media 非削除）、
save/reload（registry 永続化）。

## Rules

- commit の `validate` は全参照・ID 衝突・範囲・Clock Evidence・
  session Closed・hash 照合を副作用なしに検査する。`apply` 成功時
  に限り revision が進む（部分適用なし）。
- `commit_status` は `NotApplied | AlreadyApplied | IntegrityConflict`
  を返す。AlreadyApplied は新 revision・新 entity を生まない。
- `StaleRevision` は recoverable conflict で、最新 revision での
  bounded retry 後に成功しなければ report 化して失敗しない。
- manifest ID は `ProjectController` の generator から発行し、
  `apply` が使用済み最大 raw ID 以後へ `reseed` する。
- commit 対象は `FinalizedOutcome::Complete` のみ。Partial/
  missing/faulted は `CommitReport.recoverable` に分離し、Project
  登録と自動昇格しない。
- Undo/redo は Project 参照のみで staging media/journal/finalized/
  commit files を削除しない。
- RT パス変更なし。commit/driver 経路は非 RT で完結する。
- PCM byte は command に乗らない（manifest/command は tagged ID・
  digest・範囲のみ）。

## Examples（AC → test 対応表）

| AC | test |
| --- | --- |
| VS-001 | `vs_d_normal_commit_verified` |
| VS-002 | `vs_d_two_track_two_ch_clock` |
| VS-003 | `vs_d_punch_placement_exact` |
| VS-004 | `vs_d_loop_three_passes` |
| VS-005 | `vs_d_punch_in_loop_segments` |
| VS-006 | `vs_d_queue_overflow_not_committed` + `vs_b_rt_alloc` |
| VS-007 | `vs_d_writer_error_recoverable` |
| VS-008 | `vs_d_journal_truncation_recovery`（vs_c 既存試験 + recoverable 分離確認） |
| VS-009 | `vs_d_crash_before_commit_retry` |
| VS-010 | `vs_d_crash_after_commit_retry_no_dup` |
| VS-011 | `vs_d_undo_redo_restores_ids` |
| VS-012 | `vs_d_save_reload_integrity` |
| VS-013 | `vs_d_epoch_discontinuity_not_committed` |
| VS-014 | `vs_d_revision_conflict_recoverable` |
| VS-015 | `vs_b_rt_alloc` 維持 + `vs_d_commit_path_non_rt` |

## Coverage

- 受入試験: `crates/synvelis-recording/tests/vs_d.rs`（VS-001〜015）。
- PoC 試験: `crates/recording-poc/tests/vsd.rs`（決定的 report・
  2 回同一出力・既存 poc_determinism 非破壊）。
- 補助: manifest 単体試験（plan/hash/marker）を `commit.rs` の
  `#[cfg(test)]` に持つ。

## Verification Environment

- `mise run test`（`cargo test --workspace --locked`）。
- 実機/実 OS fault/電源断は対象外。FS capability は staging 既存
  記録を再利用。

## Non-Goals

- 性能閾値・大規模データ・GUI・実 DSP・実 WAV writer・Playback/
  Export 統合（E2E-X #26）・media GC。
