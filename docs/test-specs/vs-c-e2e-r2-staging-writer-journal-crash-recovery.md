---
id: "TSPEC-034QfPbKx2mR9tWvN5zYcHuJ"
type: "test-spec"
status: "approved"
title: "VS-C E2E-R2 検証仕様 — Staging Writer・Journal・Crash Recovery"
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
    - "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
  derived-by: []
  verifies:
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VS-C E2E-R2 検証仕様 — Staging Writer・Journal・Crash Recovery

## Purpose

spec（VS-C）の全 Acceptance Criteria を実テストへ 1:1 に落とす正本。
全テストは headless・決定的・実デバイス/実 OS fault 非依存（Fault は
`faultinj` のスクリプト注入、Process Kill は子 process の異常終了で
再現）。Mock/実 FS 上の成功を全 OS・電源断・ハードウェア故障の保証と
混同しない。

## Feature

`StagingArea`（session 別 Manifest + stream 別 Raw PCM + Journal +
Finalized marker）、binary framed Journal codec、非 RT `StagingWriter`
（drain・連続整合・決定的書込み・checkpoint・finalize・退役照合）、
`RecoveryScanner`（Last Valid Checkpoint frontier・report・recover
finalize）、`faultinj` 注入、`FinalizedSourceRef`。

## Rules

- 保存状態 `Captured/Enqueued/Written/Validated/Durable Checkpoint/
  Finalized` は区別される。OS write 成功を durable と呼ばない。
- 書込みは stream 単位で決定的順序（filled 受信順 → media 追記 →
  journal record 追記 → free 返却）。`SourceFrame` は stream 内で
  `[0, total)` の連続・重複なしを Writer 側で検証する。
- Journal replay は torn tail・checksum 不一致・seq 欠番を検出し、
  最後の検証可能 prefix で停止する。破損・未検証末尾の byte は
  削除しない（保護・報告）。
- Checkpoint は media sync → checkpoint record → journal sync →
  dir sync（capability 時のみ）の順。sync 失敗は write 失敗と別種に
  報告し、checkpoint record を発行しない。
- Recovery の復旧 frontier は Last Valid Checkpoint。未検証末尾
  （journaled ではない media byte・intact record の無い末尾）は
  Finalized 候補へ混入しない。
- Writer エラー後も既存 Validated Range と orphan/partial media を
  保護する。再実行/Recovery で二重 Finalize を作らない
  （`AlreadyFinalized`）。
- 退役時に retained + free drain + filled drain + writer held の
  合計が pool 総数と一致する（VS-B の所有権規約を維持）。
- 全ての frame/byte 演算は checked API 経由で panic しない。

## Examples（AC → test 対応表）

| Spec AC | テスト（crate/tests file::test name） |
| --- | --- |
| AC1 Normal/2ch/Multi-stream/Punch+Loop の全 frame と Session/Source/Pass 対応が media+journal から検証可能（PCM bit 一致） | `tests/vs_c.rs` `vsc_normal_stereo_pcm_bit_exact`・`vsc_multi_stream_all_ranges`・`vsc_punch_loop_spans_match`（staging から再読した PCM と `MockPcm` 期待値の bit 一致、journal span 対応） |
| AC2 Raw PCM + Manifest + Journal で元 PCM とメタデータを非破壊保存（正式登録なし） | `tests/vs_c.rs` `vsc_staging_layout_manifest`（配置・manifest 内容・media 実長・capability 記録） |
| AC3 Partial Write・Writer Stall・Queue Fault 検出・stream/segment 別 Valid Range 報告 | `tests/vs_c.rs` `vsc_partial_write_stream_failed`・`vsc_writer_stall_queue_full`・`vsc_valid_ranges_report`（faultinj 注入・latch bit・report の stream 別 range） |
| AC4 Journal Truncation/Checksum 破損検出・Last Valid Checkpoint 復旧 | `tests/vs_c.rs` `vsc_journal_torn_tail`・`vsc_journal_checksum_corrupt`・`vsc_journal_seq_gap`・`vsc_recover_last_checkpoint`（replay StopReason・frontier） |
| AC5 Write 成功と Durability の区別・Sync 失敗の正確な報告 | `tests/vs_c.rs` `vsc_sync_failed_distinct`（FailSync 注入で `SyncFailed`・checkpoint record 未発行・written≠durable） |
| AC6 複数 stream の一部失敗で正常区間を破棄しない | `tests/vs_c.rs` `vsc_multi_stream_fault_isolation`（1 stream 失敗後も他 stream が drain/finalize 可能・失敗 stream の verified prefix 保護） |
| AC7 Crash/Restart で再検証可能な Finalized 候補・未検証末尾非採用 | `crates/recording-poc/tests/vsc_crash.rs` `vsc_process_kill_recovers_checkpoint`（子 process を N record 後に異常終了→reopen→frontier 復旧・unverified tail 非採用）＋`tests/vs_c.rs` `vsc_reopen_partial_finalize` |
| AC8 Writer Error 後の Validated Range・orphan/partial 保護・二重 Finalize なし | `tests/vs_c.rs` `vsc_finalize_idempotent`（2 度目の finalize が `AlreadyFinalized`・recovery 後再 finalize も同様） |
| AC9 Writer が RT Callback に File I/O・Heap alloc・Blocking を持ち込まない | 既存 `tests/vs_b_rt_alloc.rs` `rt_path_no_alloc_no_dealloc` の維持（RT 経路変更なし・`vs_c` 試験は全て非 RT 側） |
| AC10 `recording-poc` 決定的実行・2 回同一出力 | `crates/recording-poc/tests/poc_determinism.rs`（VS-C summary を含む同一 stdout・staging 出力の byte 等価） |

## Guarantees

- 全テストは実デバイス・乱数・時刻・ネットワーク非依存（git 依存の
  初回 fetch を除く）。
- Fault 注入は `faultinj`（FailWrite/ShortWrite/FailSync/
  TruncateAfter/JournalCorrupt/WriterStall）の決定的 script で再現
  する。in-process の Error 注入試験と子 process 異常終了の Process
  Kill 試験を分ける。
- Sync 失敗・capability 非対応を「成功」と記載しない（観測値を記録）。
- Fault 経路でも Chunk 所有権総数が保存される（退役照合）。
- unverified tail・corrupt suffix の byte を試験・実装のいずれでも
  削除しない（検出・保護・報告のみ）。

## Non-goals

- Project への Source/Take/Clip 登録・冪等 Commit の検証（VS-D）。
- WAV/RF64 正式形式・Peak/Waveform Cache の検証。
- 全 OS/FS・電源断・ハードウェア故障の durability 保証（capability
  検出・記録の検証に留める）。
- 複数 process 同時 staging・Project Lock・GC policy の検証。

## Risk

- これらの保証を落とすと、Validated と Durable の区別が崩れ、
  Process Kill 後に未検証 byte が「保存済み」として Project へ
  登録される（VS-D）経路が生まれる。特に sync 失敗の握り潰し・
  unverified tail の混入・二重 Finalize は不可逆な media 破損に
  直結する。
