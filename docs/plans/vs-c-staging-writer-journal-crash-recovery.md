---
id: "PLAN-034QfNV0kVj6j2cR8sXyLqWm"
type: "plan"
status: "approved"
title: "VS-C Staging Writer / Journal / Crash Recovery 実装計画"
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
  implements:
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vs-c-e2e-r2-staging-writer-journal-crash-recovery.md"
  references: []
  defers: []
  deferred-by: []
---

# VS-C Staging Writer / Journal / Crash Recovery 実装計画 Implementation Plan

## Goal

Issue #15 の Staging/Recovery Gate: VS-B の `CaptureChunk` 契約を非 RT
`StagingWriter` が消費し、Raw PCM Staging + Journal + Crash Recovery で
Validated と Durable を区別した Verified Media Range を構成する。
Finalizer は Byte Length/Format/Frame Count/Checksum・Journal 整合性を
検査し、VS-D へ渡す `FinalizedSourceRef`（Project 非依存）を生成する。

## Scope

単一実装ストリーム（1 plan / 1 worktree）。`synvelis-recording` への
モジュール追加と `recording-poc` のシナリオ/試験拡張のみ。
`synvelis-project`・VS-B の RT 経路（engine/handoff/capture 契約）は
変更しない。新規外部依存なし（CRC32 は in-tree、serde は再利用）。

## Upstream Documents

- `docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md`（approved）
- `docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md`（accepted: L-1+J-1+M-1+C-1+H-1+F-1+Z-1）

## Design Inputs

- `docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md`（approved）

## File Map

- `crates/synvelis-recording/src/crc32.rs` - IEEE CRC32（table +
  incremental、in-tree・新規依存なし）
- `crates/synvelis-recording/src/journal.rs` - Record codec
  （encode/decode・`JournalWriter`・`JournalReplay`・`StopReason`）
- `crates/synvelis-recording/src/staging.rs` - `StagingArea`（create/
  open・manifest・stream 配置・capability 検出・atomic publish）
- `crates/synvelis-recording/src/writer.rs` - `StagingWriter`（drain・
  連続整合検証・checkpoint・finalize・held/退役照合・
  `FinalizedSourceRef`）+ `WriterError`
- `crates/synvelis-recording/src/recovery.rs` - `RecoveryScanner`（列挙・
  frontier 照合・`RecoveryReport`・recover finalize）
- `crates/synvelis-recording/src/faultinj.rs` - `StorageWriter` trait・
  `FaultWriter` 注入（FailWrite/ShortWrite/FailSync/TruncateAfter/
  JournalCorrupt）・WriterStall 補助
- `crates/synvelis-recording/src/lib.rs` - モジュール公開・re-export
- `crates/synvelis-recording/tests/vs_c.rs` - 受け入れ・fault・recovery
  試験
- `crates/recording-poc/src/main.rs` - VS-C シナリオ + crash/recover
  モード（`--vsc-crash-at`）
- `crates/recording-poc/tests/*.rs` - Process Kill→reopen 試験 +
  `poc_determinism` 期待値更新
- `docs/impl/ir/` - 実装記録（implementation-flow で作成）

## Implementation Sequence

- [ ] task1: `crc32` + `journal` codec（encode/decode・replay・
  StopReason・単体試験）
- [ ] task2: `staging`（StagingArea/manifest/capability/atomic publish）
  + `faultinj`（StorageWriter/FaultWriter）
- [ ] task3: `writer`（StagingWriter・連続整合・checkpoint・finalize・
  FinalizedSourceRef・退役照合）
- [ ] task4: `recovery`（RecoveryScanner・frontier・report・
  recover_finalize）
- [ ] task5: `recording-poc` VS-C シナリオ + `tests/vs_c.rs` 受入・
  fault・process-kill 試験 全網羅
- [ ] task6: 検証・merge origin/main・生成物再生成・impl record

## Dependencies and Risks

- `CaptureChunk` に `sequence` field は無い。Writer が stream 単位
  ingest 順 `seq` を採番して journal に記録し、連続整合は
  header/spans から検算する（design の「VS-B 契約上の注意点」に
  記録済み。契約追加は後続の検討事項）。
- Windows では dir sync が非対応 → capability `unsupported` を記録。
  保証範囲を過剰主張しない（ADR C-1/確認項）。
- Fault injection の write 失敗注入は `StorageWriter` trait 境界で
  行い、本番 File 経路と同一コードを通す。
- Process Kill 試験は子 process を N record 後に異常終了させる方式で
  決定性を確保（OS kill 非依存の再現）。
- `unsafe_code = "forbid"`、全 file write は `write_all`、append-only。

## Task Breakdown

- task1 → task2 → task3 → task4 → task5 → task6（直列。task3 は
  task1/2 の codec/staging/faultinj に依存、task4 は task3 の writer/
  journal に依存、task5 は task4 の recovery に依存）

## Verification

- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`build` 全緑
- [ ] `mise run test`（`cargo test --workspace --locked`）が lockfile 再現
- [ ] `recording-poc` を 2 回実行し同一 stdout（`poc_determinism`）
- [ ] Process Kill→reopen で Last Valid Checkpoint 復旧・未検証末尾
  非採用の実測
- [ ] 全 spec AC を網羅するテスト（test-spec の AC→test 対応表に一致）
- [ ] `mise run test-msrv`（rust 1.89 toolchain が利用可能な場合）

## Open Questions / Gaps

- Checkpoint/Sync 周期・`MAX_PAYLOAD` 等の実数値は注入可能な構成で
  先行し、PoC 計測結果で確定（design の Deferred と同一）。
- `ChunkValidity::GateClosed` は現行 engine が出力しない。Writer は
  `Complete` 以外を contract violation 扱い（design 記載）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（subagent 分割不要・単一 worktree
  直列）
- Independent work streams: none
- User approval required before dispatch: no（workspace 契約で
  review-protocol が承認 gate を代替、design approved 済）
- If approved: task-doc 単位で順次実装、各 task 完了時に関連テスト実行。

## Review Handoff

- 重点: journal replay の破損検出が厳密か、write/sync の状態区別が
  型と record で維持されるか、stream 単位 Fault 隔離が正常 Range を
  失わないか、unverified tail が finalized へ混入しないか、
  `free` 返却と held の総数照合が Chunk 所有権を失わないか、
  `FinalizedSourceRef` が Project 型を参照しないか。
