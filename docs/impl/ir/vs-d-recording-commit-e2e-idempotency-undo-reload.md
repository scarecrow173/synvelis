---
id: "IMPL-VSdRecCommitE2E"
type: "impl"
status: "completed"
title: "VS-D Recording Commit / End-to-End Vertical Slice — Idempotency・Undo・Reload (E2E-R3)"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/vs_d.rs"
      - type: "file"
        path: "crates/recording-poc/tests/vsd_process.rs"
      - type: "file"
        path: "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
      - type: "file"
        path: "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
      - type: "file"
        path: "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
      - type: "file"
        path: "docs/plans/vs-d-recording-commit-e2e.md"
      - type: "file"
        path: "docs/test-specs/vs-d-e2e-r3-recording-commit.md"
      - type: "file"
        path: "docs/tasks/vs-d-task1-applied-commit-registry.md"
      - type: "file"
        path: "docs/tasks/vs-d-task2-commit-manifest-plan.md"
      - type: "file"
        path: "docs/tasks/vs-d-task3-commit-recording-op.md"
      - type: "file"
        path: "docs/tasks/vs-d-task4-poc-vsd-scenario.md"
      - type: "file"
        path: "docs/tasks/vs-d-task5-acceptance-tests.md"
      - type: "file"
        path: "docs/tasks/vs-d-task6-verification-impl-record.md"
      - type: "file"
        path: "docs/impl/ir/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    modified:
      - type: "file"
        path: "crates/recording-poc/src/main.rs"
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/staging.rs"
      - type: "file"
        path: "docs/impl/ir/vs-c-staging-writer-journal-crash-recovery.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vs-d-recording-commit-e2e.md"
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
    - "docs/tasks/vs-d-task1-applied-commit-registry.md"
    - "docs/tasks/vs-d-task2-commit-manifest-plan.md"
    - "docs/tasks/vs-d-task3-commit-recording-op.md"
    - "docs/tasks/vs-d-task4-poc-vsd-scenario.md"
    - "docs/tasks/vs-d-task5-acceptance-tests.md"
    - "docs/tasks/vs-d-task6-verification-impl-record.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VS-D Recording Commit / E2E Vertical Slice — Idempotency・Undo・Reload (E2E-R3)

## Summary

Mock Input → sample-accurate Gate → bounded RT handoff → staging
writer/journal → finalize/recovery → 冪等 Project commit →
save/reload/undo/redo の縦断 slice を実装した。commit は
`commit-manifest.json` で Stable ID を事前確保した上で
`RecordingOp::CommitRecording` 単一 mutation として適用し、
`applied_commits` registry（Project 永続化対象）と
`commit.json` marker（staging 永続化）で二重登録を防止する。
PCM は command に乗らず、storage/media 操作は command の外に留まる。

## Context

Issue #16。前提: VS-A #13（PR #29）・VS-B #14（PR #30）・
VS-C #15（PR #31）。doc chain: spec → ADR → design → plan →
test-spec → 6 task。E2E-R gate の mock-only 完結を証明し、
Ruviform 実デバイス機能（#187/#188/#191）には依存しない。

## Implementation

### Changes

- `synvelis-project/src/project.rs`: `RecordingRegistry.applied_commits`
  （`BTreeMap<RecordingSessionId, AppliedCommit { content_hash, revision }>`、
  serde 永続化対象）。`AppliedCommit` を新設。
- `synvelis-project/src/controller.rs`: `draw_raw_id`（ID 発番器から
  raw u128 を引き出し、`used_id_watermark` を巻き上げる）。manifest
  用の ID 事前確保に使い、適用後の ID 再利用を防ぐ。
- `synvelis-recording/src/commit.rs`（新規・約 1,400 行）:
  - `RecordingCommitIdentity`（session_id + content_hash）/
    `RecordingCommitManifest`（identity + reserved IDs を含む
    `CommitPayload`）/`CommitOutcome`/`CommitReport`/
    `RecoverableStream`/`CommitStatus`/`CommitError`。
  - `build_payload`: `load_finalized`（`finalized.json` を stream ごと
    に読む）から `Complete` ref のみ commit 対象とし、`Partial`/
    `missing-finalized`/`finalized-unreadable`/`empty-finalize`/
    `capture-fault` を `recoverable` 分離。ref は session/stream
    帰属・format メタ（channels/sample_rate/media/schema）を
    staging manifest と照合し、不一致は commit しない。
    span を pass 単位に merge し take/segment/pass/clip の
    manifest entry を構成。clip は lane が commit 時点で空のとき
    のみ、最低 pass の take の segment から生成する（既存
    comp/clip は暗黙置換しない）。同一 lane に複数 stream が
    流入する場合は先頭 stream のみが clip を生成する。
  - `prepare_manifest`: `applied_commits` registry が authority
    — 既 commit なら staging drift（finalized.json 消失・改変・
    lane 非空化）に関わらず stored manifest を返し
    `AlreadyApplied`/`IntegrityConflict` の分類は `commit_session`
    の registry 照合へ委ねる。未適用 manifest は
    `identity.content_hash` 再検算 + fingerprint（ID 文字列を除く
    構造一致）+ clip↔segment 値整合 + `validate_commit`（session
    が commit 可能状態の場合のみ）で再利用判定し、整合すれば
    採番をスキップして再利用、不整合（stale）なら予約 ID は一度も
    Project に入っていないため破棄して再構築する（lane 非空化・
    undo 後の ID 再発行・refs 回復後の再 commit・
    NothingCommitted 凍結を全て回復可能にする）。新規時は
    `draw_raw_id` で必要数を先にまとめて確保し、
    `commit-manifest.json` を tmp→rename で原子公開 + parent
    dir sync する（rename 消失による reserved ID 喪失を防ぐ）。
  - `commit_status`/`commit_session`: registry の `applied_commits`
    で `Applied`/`AlreadyApplied`/`IntegrityConflict` を分類し、
    `StaleRevision` は最新 revision 取得で bounded retry（上限 8）
    する recoverable conflict として扱う。Applied/AlreadyApplied/
    NothingCommitted で `commit.json` marker（identity + outcome +
    revision + stream digests）を原子記録 + dir sync する。
  - `validate_commit`/`apply_commit`: schema/hash/session 状態
    （`Closed`、または registry 未登録の `Finalized`）/registry
    未登録/全 ID 一意性・未衝突/manifest 内 pass index・stream
    entry の一意性/pass_id↔pass_index 整合/参照整合（pass→
    session・stream→lane/session・segment↔take↔clip・source
    range/digest/placement）/負値・反転・overflow しない範囲計算
    を validate（副作用なし）。apply は scratch clone 上で
    asset/source/pass/segment/take/clip/registry/session→
    `Finalized` を一括登録し、失敗時は `ProjectController` が
    partial apply を捨てる。
- `synvelis-recording/src/ops.rs`: `RecordingOp::CommitRecording {
  manifest }` variant + apply 分岐。
- `synvelis-recording/src/staging.rs`: `capture-fault.json` session
  marker（`record_capture_fault`/`capture_fault`/
  `capture_fault_marker_exists`/`capture_fault_path`）。RT/engine
  側の session スコープ fault を driver が durable に残す層。
  `build_payload` は marker があれば全 stream を
  `capture-fault:<detail>` で recoverable 分離する。
- `recording-poc/src/main.rs`: `vsd <root> <session>` サブコマンド。
  project ops → mock capture →（latch fault 時は
  `capture-fault.json` を driver が記録）→ staging finalize →
  `commit_session` → save → reload（serialized project 一致）→
  retry（`AlreadyApplied`）→ undo（media 残留）→ redo（stable
  ID 復元）→ marker 確認 → `vsd-report.json` を root に出力。
  stdout は決定的 `key=value` 行。
- `recording-poc/tests/vsd_process.rs`: `vsd` を実 process で
  2 回起動し、全 stdout が byte 一致で決定的かつ `result=OK`・
  主要 key=value が揃うことを検証。

### Decisions During Implementation

- clip 生成可否（空 lane 判定）は commit 時点の project 状態由来
  なので manifest fingerprint から外し、clip↔segment の
  値整合（tick_in/out・source_offset）だけを別チェックとした。
  これで commit 後の retry（lane 非空化後）でも manifest を
  正しく再利用できる。
- RT/engine 側 fault は shared `FaultLatch` にしか残らず writer
  state に反映されないため、per-stream の fault 分離（VS-C 契約:
  `vsc_multi_stream_fault_isolation`）を壊さないよう、writer 層
  ではなく session スコープの `capture-fault.json` marker で
  運ぶ設計とした。capture fault は take 全体を中断する session
  単位の出来事であり、全 stream の recoverable 分離が正しい。
- `prepare_manifest` は ID 不使用の probe payload → fingerprint
  照合の二段構成で、manifest 再利用を「同一内容なら同じ ID」を
  ファイル比較ではなく構造一致で判定する（再採番しない）。
- `commit.json` marker は staging に残る補助記録で、authority は
  Project の `applied_commits`。marker 欠落 crash（commit 後・
  marker 前）でも registry で `AlreadyApplied` に倒れる。

### Post-Implementation Review Fixes

review protocol（cooperative + adversarial 2 系統）で確認された
P1/P2 を修正し、対象 regression test を追加した。

- P1: `applied_commits` を fingerprint 照合より先に判定し、
  commit 済み session の staging drift で `AlreadyApplied` が
  不可達になる問題を解消（`vs_d_retry_after_staging_drift_is_already_applied`）。
- P1: 未適用 manifest が refs/project と不整合になった場合、
  予約 ID は一度も Project に入っていないため破棄・再構築する
  経路を追加（lane 非空化後の implicit comp 拒否 deadlock・
  `NothingCommitted` 凍結・undo 後 ID 再発行を回復可能化:
  `vs_d_stale_manifest_rebuild_after_lane_change`・
  `vs_d_recommit_after_undo_recovers`）。
- P1: `commit-manifest.json`/`commit.json` の原子公開後に parent
  dir sync を追加（dir sync 非対応環境では従来通りスキップ）。
- P1: `FinalizeSession`（Closed→Finalized）で既に Finalized 化
  された session も registry 未登録なら commit 可能に
  （`vs_d_commit_after_finalize_session_op`）。commit 済みは
  registry で拒否されるため二重登録にはならない。
- P2: manifest 内 pass index・stream entry（index + stream id）の
  重複・pass_id↔pass_index 不整合・空 segment/take payload を
  `validate_commit` で拒否（`vs_d_validate_rejects_intra_manifest_dup_pass_index`）。
- P2: 負の frame 値・反転 range・checked なし算術を total
  validation で拒否し、`merge_spans` の merge 判定も checked
  化（`vs_d_validate_rejects_negative_ranges`）。
- P2: finalized ref の session/stream 帰属・format メタ
  （channels/sample_rate/media path/schema version）を staging
  manifest と照合し、stale/流用 ref を commit しない。
- P2: `commit.json` marker に stream digests を記録。
- P3 記録: `payload_hash` は非鍵付き CRC ベースの
  accidental-corruption 検出であり、暗号的 integrity ではない
  （構造的検査との二段で運用）。

## Acceptance Matrix Traceability

| AC | 検証 |
| --- | --- |
| VS-001 normal capture → verified commit | `vs_d_normal_commit_verified` |
| VS-002 2ch × 2 track + clock evidence | `vs_d_two_track_two_ch_clock` |
| VS-003 punch mid-block placement | `vs_d_punch_placement_exact` |
| VS-004 loop 3-pass takes/comp | `vs_d_loop_three_passes` |
| VS-005 punch-in-loop segments | `vs_d_punch_in_loop_segments` |
| VS-006 queue overflow → no partial apply | `vs_d_queue_overflow_not_committed`（+ `vs_b_rt_alloc` で RT 契約維持） |
| VS-007 writer/storage fault → recoverable | `vs_d_writer_error_recoverable` |
| VS-008 journal truncation → last checkpoint | `vs_d_journal_truncation_recovery`（vs_c 既存試験 + recoverable 分離） |
| VS-009 crash before commit → manifest retry | `vs_d_crash_before_commit_retry` |
| VS-010 crash after commit → no dup | `vs_d_crash_after_commit_retry_no_dup` |
| VS-011 undo/redo stable IDs・media 残留 | `vs_d_undo_redo_restores_ids` |
| VS-012 save/reload integrity + retry | `vs_d_save_reload_integrity` |
| VS-013 epoch discontinuity → no fake continuity | `vs_d_epoch_discontinuity_not_committed` |
| VS-014 revision conflict → recoverable | `vs_d_revision_conflict_recoverable` |
| VS-015 commit path 非 RT・PCM 不混入 | `vs_d_commit_path_non_rt`（+ `vs_b_rt_alloc` 維持） |

付随: 同一 identity・異内容の conflict は
`vs_d_integrity_conflict_same_identity` が registry
（`CommitStatus::IntegrityConflict`）と manifest 改ざん検出
（`CommitError::Manifest`）の両面で検証する。

## Validation

- `mise run fmt-check` / `check` / `lint` / `test` / `build` /
  `test-msrv`: 全 green（workspace、--locked 含む）。
- `cargo test -p synvelis-recording`: vs_a 15 + vs_b 15 + vs_c 15 +
  vs_d 22 他、全 pass。
- `recording-poc vsd` 2 回連続実行: stdout が byte 一致で決定的
  （finalize digest・content hash・commit/reload/undo/redo/marker の
  全 key=value が同一）+ `result=OK` + `vsd-report.json` 出力。
  実 process 版は `recording-poc/tests/vsd_process.rs` が検証。

## Known Limitations / Report

- `capture-fault.json` は session スコープの保守的分離であり、
  per-stream の engine fault 帰属（shared latch の分解）は実
  デバイス層の課題として残す。writer 個別の失敗は stream 単位で
  分離済み（VS-C 契約を維持）。
- staging recovery は media 側の回復であり、Project 状態の復旧を
  意味しない（両者は別構造で、recoverable は常に分離表示される）。

## Related Experiments

なし（exploratory experiment log の記録対象となる探索的実験は発生しなかった）。

## Risks

既知リスクはなし。検証は headless 決定的 driver で実測済み。

## Follow-ups

なし。
