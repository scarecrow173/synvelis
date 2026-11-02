---
id: "IMPL-034QfRW3cPFErlmVZ7TCoI"
type: "impl"
status: "completed"
title: "VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range (E2E-R2)"
created: "2026-11-01"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/src/crc32.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/journal.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/staging.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/faultinj.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/writer.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/recovery.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/vs_c.rs"
      - type: "file"
        path: "crates/recording-poc/tests/vsc_process.rs"
      - type: "file"
        path: "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
      - type: "file"
        path: "docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md"
      - type: "file"
        path: "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
      - type: "file"
        path: "docs/plans/vs-c-staging-writer-journal-crash-recovery.md"
      - type: "file"
        path: "docs/test-specs/vs-c-e2e-r2-staging-writer-journal-crash-recovery.md"
      - type: "file"
        path: "docs/tasks/vs-c-task1-crc32-journal-codec.md"
      - type: "file"
        path: "docs/tasks/vs-c-task2-staging-faultinj.md"
      - type: "file"
        path: "docs/tasks/vs-c-task3-staging-writer.md"
      - type: "file"
        path: "docs/tasks/vs-c-task4-recovery-scanner.md"
      - type: "file"
        path: "docs/tasks/vs-c-task5-poc-acceptance-tests.md"
      - type: "file"
        path: "docs/tasks/vs-c-task6-verification-impl-record.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/handoff.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/mock.rs"
      - type: "file"
        path: "crates/recording-poc/src/main.rs"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/plans/vs-c-staging-writer-journal-crash-recovery.md"
    - "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md"
    - "docs/tasks/vs-c-task1-crc32-journal-codec.md"
    - "docs/tasks/vs-c-task2-staging-faultinj.md"
    - "docs/tasks/vs-c-task3-staging-writer.md"
    - "docs/tasks/vs-c-task4-recovery-scanner.md"
    - "docs/tasks/vs-c-task5-poc-acceptance-tests.md"
    - "docs/tasks/vs-c-task6-verification-impl-record.md"
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

# VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range (E2E-R2)

## Summary

VS-B の `CaptureChunk` を非 RT Writer が消費し、Raw f32 LE PCM +
フレーム化 append-only journal + 明示 checkpoint で Verified Media
Range を構成する。Written → Validated → Durable Checkpoint を
区別し、crash 後は Last Valid Checkpoint までを
`finalized.json`（Project 非依存 `FinalizedSourceRef`）として原子
公開する。ruviform #191 の実 WAV Writer には依存しない。

## Context

Issue #15。前提: VS-A #13（PR #29）・VS-B #14（PR #30）。doc chain:
spec → ADR → design → plan → test-spec → 6 task。Fault/Recovery Gate
で要求された全 fault クラスを注入検証済み。

## Implementation

### Changes

- `crc32.rs`: 定数テーブル生成の CRC32（IEEE）+ streaming `Crc32` +
  `digest_fold`（全範囲を畳む決定的 digest）。外部依存なし。
- `journal.rs`: 16 byte header（magic/version/kind/seq/payload_len）+
  payload + CRC32 の framed record codec。`ChunkRecord`/
  `CheckpointRecord`/`FinalizeRecord` の 3 kind、`JournalAppender`
  （連番発行）、`replay_journal`（`StopReason`: CleanEnd/TornTail/
  BadMagic/BadVersion/BadLength/BadChecksum/SequenceGap）。
- `staging.rs`: `StagingArea`（`root/<session-tag>/` +
  `streams/stream-NNNN.{pcm,journal,finalized.json}`）+ manifest.json
  （format/channels/sample_rate/capability 観測値）+ tmp→rename 原子
  公開 + dir sync capability 検出。
- `faultinj.rs`: `StorageWriter` trait + `FaultWriter`（call 番号指定の
  `FailWrite`/`ShortWrite`/`FailSync`/`ByteLimit`）+ `corrupt_byte`。
- `writer.rs`: `StreamWriter`。filled drain → 契約検証（stream_index/
  validity/PCM 長/span 配置/session・source 連続/packed 範囲/
  `sum(frame_count)==stored_frames`）→ media 追記 → journal record →
  free 返却。`checkpoint()` は media `sync_data` → Checkpoint record
  → journal `sync_data` → dir sync の順。`finalize()` は frontier
  まで再読・CRC/digest 再検算 → `finalized.json` 原子公開 +
  Finalize record。`finish_all` は全 stream の drain+finalize。
  **Written / Validated / Durable の分離**: `media_len` は実 file の
  書込み cursor（次回 append 位置）で、`journaled_media_len` /
  `journaled_digest` / `journaled_source_frames` は journal record
  の append 成功時にのみ commit される別スナップショット。
  `verify_chunk` は副作用を持たず、検算した cursor（source/session/
  input）を候補として返し、`process_chunk` が journal append 成功後に
  commit する。`checkpoint()` が記録するのは journaled 側のみなので、
  書込み途中失敗の chunk は durable frontier を汚染しない。
- `recovery.rs`: `scan`（session 列挙・stream 分類 Healthy/Partial/
  Corrupted/Unjournaled、validated/unverified tail 計測。Healthy は
  clean end + checkpoint 有り + tail/short 無しの場合のみ）、
  `verify_stream`（finalize/recover 共通の media 再検算）、
  `recover_finalize`（Last Valid Checkpoint までの Partial ref を
  冪等公開。checkpoint は新→旧の順に再検証し、検証を通る最初の
  frontier を採用するため、最新 checkpoint の不整合が先行する
  整合した checkpoint を覆い隠さない。journal tail が clean でない
  場合は Finalize record を追記しない）。
- `handoff.rs`: `ChunkSink` trait（`pump`/`into_ends`）。
- `mock.rs`: `Scenario<W: ChunkSink>` 汎用化（`build_with` で stream
  index 付き writer factory）。
- `recording-poc`: `vsc record <root> <session> [--crash]` /
  `vsc recover <root> <session>` サブコマンド。`--crash` は checkpoint
  後に `process::exit(3)`（真の process 終了）。

### Decisions During Implementation

- `CaptureChunk` に `sequence` フィールドは無いため、順序整合は
  header（session_start/first_input_frame/block_frames）+ span 列の
  source 連続性 + filled FIFO 順で検算する（journal の seq は writer
  側で発番）。`packed_pcm_offset` は spec の frame 単位ではなく実装
  通り sample 単位として扱う。
- `GateClosed`/`Faulted` chunk は VS-B が filled へ出さないため、
  writer は `ChunkValidity::Complete` 以外を契約違反として reject
  する。
- `StreamWriter` は `FaultWriter<File>` を具体型で保持し、構築後も
  `inject_media`/`inject_journal` で注入可能にした（`Box<dyn
  StorageWriter>` では注入へ辿れないため）。
- free 返却失敗は stream を `Failed` にせず、`FAULT_WRITER_ERROR` +
  chunk 保持のみ（所有権照合を優先し、後続の整合検証は継続）。
- `finalize` の outcome は `state==Failed` または frontier 以後の
  validated/unverified tail の存在で `Partial` とする。他 stream の
  fault で健全 stream が Partial にならないよう shared latch ではなく
  stream 自身の状態で判定する。
- `finalized.json` が finalized の authority。Finalize record は
  診断用で、journal 追記の失敗は notes 記録のみで ref 公開を
  取り消さない（二重 finalize 防止は marker の存在確認が担う）。
- `StagingArea::create` は manifest を authority とし、dir のみ残る
  partial create は manifest 未公開として冪等に完了する（stream
  file は `create_new` で truncate しない）。manifest 存在時のみ
  `AlreadyExists`。
- review で発見された durability 欠陥（`verify_chunk` が media/
  journal 成功前に cursor を commit し、checkpoint が未 journaled
  値を frontier に記録して先行する validated prefix を復旧不能に
  する）を、上記の journaled frontier 分離で修正。回帰テスト:
  `vsc_failed_chunk_preserves_validated_prefix` /
  `vsc_failed_journal_append_preserves_prefix` /
  `vsc_multi_stream_fault_isolation`（Partial 断言へ修正）。

## Related Experiments

### Adopted

なし。

### Rejected

なし。

## Validation

実行済み（全てローカル、最新 `origin/main` 取り込み後）:

- `mise run check` — pass
- `mise run lint`（clippy `-D warnings`）— pass
- `mise run fmt-check` — pass
- `mise run test` — pass。`vs_c.rs` 16 件（layout/manifest、bit-exact
  PCM、multi-stream、punch loop、partial write、disk-full byte limit、
  sync 失敗の区別、journal torn tail/checksum/seq gap、last valid
  checkpoint 復旧、unverified tail 排除、multi-stream 障害分離、
  finalize 冪等、stall→drain、contract violation、failed chunk の
  validated prefix 保持、failed journal append の unverified tail
  保持）+ `vsc_process.rs`
  2 件（`--crash` で process 終了→別 process で recover→Partial ref、
  再 recover は already_finalized、media_byte_len 一致; 通常 record
  →Complete）を含む全テスト成功。

未検証: 実デバイス/実 OS crash（power-loss）。fsync・dir sync・
atomic rename の保証範囲は manifest `capabilities` に観測値として
記録するのみで、普遍的な power-loss durability は主張しない。

## Risks

- VS-B chunk に `sequence`/`channels`/`sample_rate` を持たないため、
  writer の契約検証は header+span の連続性に依存する。明示 sequence
  の追加は VS-B 側の後続課題。
- `ByteLimit`/`ShortWrite` 後の media partial byte は orphan として
  保持するが、正式な Media GC policy は本 issue の対象外（#17 側）。
- dir sync capability は環境依存（Windows では Unsupported 観測）。
  manifest に記録済み。

## Follow-ups

- VS-D #16: `FinalizedSourceRef` → Source/Take/Clip の Project
  Mutation（idempotent registration）。
- 実 WAV/RF64 writer（ruviform #191）との差替え点は Raw PCM +
  Manifest 境界で確定済み。
- VS-B chunk への `sequence`/`channels`/`sample_rate` 補強を検討
  （spec/実装 gap は本 IR に記録）。
