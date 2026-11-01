---
id: "DESIGN-034QfL0bH9u3cVqK0pYwT7x"
type: "design"
status: "approved"
title: "VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range"
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
  related:
    - "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/15"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  defers: []
  deferred-by: []
---

# VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range

## Context

VS-A が `synvelis-project`（Stable ID・時間型・半開 Range・Project
集約・atomic rename による snapshot 永続化）を、VS-B が
`synvelis-recording` に Mock Clock/Input・Prepared Gate Schedule・
`CaptureChunk`/`CaptureSpan`/`ChunkValidity` 契約・事前確保 SPSC
Handoff（`WriterEnds { filled_rx, free_tx }` + 共有 `FaultLatch`）を
実装済み。現行の Chunk 消費者はメモリ上で検証する
`InspectionWriter` のみで、永続化は存在しない。

VS-C はこの契約の非 RT 消費者として `StagingWriter` を実装し、Raw
PCM Staging + Journal + Crash Recovery で Validated と Durable を
区別した Verified Media Range を構成する。spec「VS-C Staging Writer
/ Journal / Crash Recovery」(approved) と ADR「VS-C Journal形式・
Checkpoint Sync方針・Fault隔離・Finalized出力」(accepted:
L-1+J-1+M-1+C-1+H-1+F-1+Z-1) を実装可能な形に落とす。

### VS-B 契約上の注意点（実コードとの照合結果）

- `CaptureChunk` に `sequence` field は存在しない（spec で予定された
  `header.sequence` は未実装。`session_frame_start`→`session_start` 等の
  rename も反映済み）。順序保証は「stream 単位で engine が FIFO に
  `filled` へ push する」性質に依存し、Writer が stream ごとに ingest
  順 `seq`（u64）を採番して Journal Record に記録する。連続整合は
  `header.{session_start, first_input_frame, block_frames}` と
  `spans[].source_start` から検算する（Sequence 追加は VS-B 側の
  契約変更であり、VS-C では既存契約から導出する）。
- Engine は `Complete` Chunk のみ `filled` へ送る。`GateClosed` は
  Chunk を出さず、`Faulted` Chunk は engine 側 `retained` に保持され
  Writer へ届かない。Writer は `Complete` のみを前提にできるが、
  防御的に `Complete` 以外を contract violation として扱う。
- `span.packed_pcm_offset` は interleaved **sample** index（frame
  index ではない）。`stored_samples == stored_frames * channels`。
- Chunk は `stream_index`（`plan.targets` の添字・u16）を持つが、
  `CaptureStreamId`・channels・sample_rate を持たない。Writer は
  `StagingStreamConfig`（呼び出し側が plan/project から供給）で
  stream_index→identity/format を解決する。

## Scope

### In Scope

- `synvelis-recording` 新規モジュール: `crc32`・`journal`・`staging`・
  `writer`・`recovery`・`faultinj`。
- Session 単位 Staging 領域（Manifest + stream 別 Raw PCM + Journal +
  Finalized marker）。
- 非 RT `StagingWriter`: filled drain・連続整合検証・決定的書込み・
  free 返却・保持 Chunk の退役照合・明示 Checkpoint・Finalize。
- Journal codec: binary framed record の append/replay・torn tail・
  checksum・seq 欠番検出。
- `RecoveryScanner`: staging root 列挙・Journal 照合・Last Valid
  Checkpoint frontier・未検証末尾の報告・recovery finalize。
- `faultinj`: 決定的 Fault Injection（Partial/Short Write・Disk
  Full・Sync Fail・Journal Interrupt・Checksum Corrupt・Writer
  Stall）。公開モジュール（PoC の crash モードが利用）。
- `recording-poc`: VS-C 決定的シナリオ（Normal/2ch/Multi-stream/
  Punch+Loop・Fault・Recovery）と crash/restart デモ。

### Out of Scope

- Project への Source/Take/Clip 登録・冪等 Commit（VS-D/#16）。
- WAV/RF64 等の正式形式（Ruviform #191）、Peak/Waveform Cache。
- GC/Retention policy・複数 process 同時 Staging・Project Lock。
- RT 経路の変更（VS-B の engine/handoff/fault 契約は不変）。

## Components and Boundaries

### Staging 領域レイアウト

```
<staging_root>/
  <session-tag>/                    # RecordingSessionId::to_tagged()
    manifest.json                   # 作成時に atomic publish・以後 immutable
    streams/
      stream-0000.pcm               # interleaved f32 LE・append-only
      stream-0000.journal           # binary framed records・append-only
      stream-0000.finalized.json    # FinalizedSourceRef・tmp→rename で原子公開
      stream-0001.pcm / .journal / .finalized.json
```

`StagingArea::create(root, config)` が manifest を書き各 stream file を
作成（空）。`StagingArea::open(root, session_id)` が reopen で復元する。
`manifest.json`:

```json
{
  "schema_version": 1,
  "session_id": "recording-session-<hex>",
  "sample_rate": 48000,
  "pcm_format": "f32le-interleaved",
  "streams": [
    {
      "stream_index": 0,
      "capture_stream_id": "capture-stream-<hex>",
      "lane_id": "lane-<hex>",
      "channels": 2,
      "media": "streams/stream-0000.pcm",
      "journal": "streams/stream-0000.journal",
      "finalized": "streams/stream-0000.finalized.json"
    }
  ],
  "capabilities": {
    "atomic_rename": true,
    "file_sync": true,
    "directory_sync": "supported" | "unsupported" | "failed"
  }
}
```

`capabilities.directory_sync` は作成時に 1 度検出する: session dir を
`File::open` して `sync_all` できる FS では `supported`、Open 自体が
失敗する環境（Windows 等）では `unsupported`。失敗時 `failed`。
検出値を記録して保証範囲を明示する（主張しない）。

### Journal Record 形式（J-1）

全て little-endian。Record =

```
+0   u16  magic = 0x4A52 ("JR")
+2   u8   version = 1
+3   u8   kind (1=Chunk, 2=Checkpoint, 3=Finalize)
+4   u64  seq            # stream 内で 0 から連番
+12  u32  payload_len
+16  [payload_len bytes payload]
+..  u32  crc32          # magic を除く header+payload (offset 2..crc-1)
```

Header 固定 16 bytes + payload + crc32。`payload_len` 上限は定数
`MAX_PAYLOAD`（例: 1 MiB。Chunk Record の span 列は有界）を超えたら
破損扱い。

Replay は先頭から読み、(a) header 16B 未満の残り、(b) magic/
version 不整合、(c) payload_len > MAX_PAYLOAD またはファイル残り
不足、(d) crc32 不一致、(e) seq 欠番/逆転、のいずれかで停止し、
それまでを検証可能 prefix、`prefix_len`（byte）と `last_seq`、停止
理由 `StopReason { CleanEnd, TornTail, BadMagic, BadVersion,
BadLength, BadChecksum, SequenceGap }` を返す。破損以降の byte は
削除・切詰めしない（保護・報告のみ）。

Payload schema（全て fixed-size 整数 + 有界繰返し、LE）:

- `Chunk` (kind=1):
  `chunk_id u32 / first_input_frame u64 / session_start i64 /
  block_frames u32 / stored_frames u32 / span_count u16 /
  media_offset u64 / media_len u64 / pcm_crc32 u32 /
  digest_fold u64` の後に span_count 個の
  `pass_index u32 / span_session_start i64 / frame_count u32 /
  source_start i64 / project_in i64 / project_out i64 /
  span_media_offset u64`。
  `media_offset/media_len` はこの Chunk が media に書いた byte 範囲
  （self-describing）。`pcm_crc32` は書いた byte 列の CRC32。
  `digest_fold` は `digest = digest*K ^ span_crc` を順次畳んだ
  streaming 状態（各 span の crc を順に畳んだ値）で、全 stream の
  検証を record 1 本で閉じる。
- `Checkpoint` (kind=2): `checkpoint_index u64 / media_len u64 /
  source_frames u64 / digest_fold u64 / chunk_records u32`。
  「この時点で media sync 済み・journal も sync した」ことを
  書込み側の責務として宣言する。durable frontier。
- `Finalize` (kind=3): `finalized_seq u64 / media_len u64 /
  source_frames u64 / digest_fold u64 / outcome u8
  (0=complete, 1=partial)`。Finalize 実行の journal 記録。

### 保存状態機械（stream 単位）

```
Captured      engine が Chunk を生成（VS-B 側）
Enqueued      filled queue 内
Written       media write_all 成功（journal record 未永続）
Validated     対応 Chunk record が journal に完全追記（再検証可能）
Durable       Checkpoint record 発行 + media/journal sync 成功
Finalized     finalized.json が原子公開 + Finalize record
Failed        Writer エラー/contract violation（以後書込み停止）
```

- write 成功≠durable。`SyncFailed` は `Io` とは別種のエラーとして
  報告し、Checkpoint record を発行しない。
- `Enqueued→Written` の順序は `StagingWriter::pump()` が
  stream index 順に各 stream の filled を drain する決定的順序。
- `Failed` stream の Validated/Durable prefix は失われない
  （append-only のため物理的に残る）。
- `Project Committed` は本 slice の外（VS-D）。

### `StagingWriter`

```rust
pub struct StagingWriter {
    area: StagingArea,                    // open 済み session dir
    streams: Vec<StreamWriter>,           // stream_index 順
    ends: Vec<WriterEnds>,                // VS-B handoff（所有）
    fault: FaultLatch,
}
struct StreamWriter {
    cfg: StagingStreamConfig,             // identity/channels/sample_rate
    media: Box<dyn StorageWriter>,        // File または faultinj wrapper
    journal: Box<dyn StorageWriter>,
    state: StreamState,                   // Active | Failed(reason) | Finalized
    seq: u64,                             // journal record seq
    source_cursor: i64,                   // 次に期待する source_start
    last_session_end: i64,                // session_start 単調検証用
    media_len: u64,                       // 書込み済み byte（Written frontier）
    digest: u64,                          // fold 状態
    checkpoint: Option<CheckpointState>,  // durable frontier
    held: Vec<CaptureChunk>,              // 未返却 Chunk（所有権保持）
}
```

`pump()`（非 RT・繰返し呼出し）:

1. stream index 順に `filled_rx.try_pop()` を drain。
2. Chunk 検証: `validity == Complete`、`stream_index` 一致、
   `stored_samples == stored_frames*channels`、span 列が順序・
   非重複・`sum(frame_count)==stored_frames`、各
   `source_start == source_cursor`（連続・巻戻り・gap なし）、
   `session_start >= last_session_end`（gate-off で chunk が出ない
   区間は前進のみ要求・等しければ overlap 違反）、
   `packed_pcm_offset + frame_count*channels <= stored_samples`。
   違反 → `FAULT_CONTRACT` latch + stream `Failed` + Chunk は
   held へ（media 未書込み）。
3. 各 span について `media.write_all(pcm[offset..offset+len])`。
   エラー → stream `Failed` + `FAULT_WRITER_ERROR` + held。
   成功 → `media_len` 更新（Written）。
4. Chunk record を encode して `journal.write_all`。
   エラー → stream `Failed`（media は書込み済み＝unverified tail）。
   成功 → Validated。`source_cursor`/`seq`/`digest`/`last_session_end` 更新。
5. `free_tx.try_push(chunk)`。Full → `FAULT_WRITER_ERROR` + held。

`checkpoint()`:

1. 全 Active stream について `media.sync_data()` → 失敗なら
   `WriterError::SyncFailed`（checkpoint record 未発行）。
2. Checkpoint record を journal へ追記。
3. `journal.sync_data()` → 失敗で `SyncFailed`（record は残るが
   durable frontier としては数えない — 次回 checkpoint で再 sync）。
4. dir sync（capability が supported の場合のみ。失敗は記録するが
   エラー扱いしない — 保証外）。
5. `checkpoint = Some({index, media_len, source_frames, digest, seq})`。

`finish() -> FinishReport`: 残りを drain → 全 stream に最終
checkpoint → 各 stream `finalize()` → 終了時に `held`/`filled`/
`free`/`retained` の総数照合（pool 総数不変の維持）。

`finalize(stream) -> Result<FinalizedSourceRef, WriterError>`:

1. stream が `Finalized` または `finalized.json` が既に存在し内容が
   整合 → `AlreadyFinalized`（二重 Finalize 防止・冪等）。
2. durable frontier = `checkpoint`（なければ暗黙 checkpoint を試行。
   sync 失敗時は `SyncFailed` で finalize 失敗）。
3. Journal を replay し `last_seq == frontier.seq` まで整合・
   media 実長が `media_len` と一致・`frontier` までの PCM byte を
   読み返し CRC/digest を再検証（Byte Length/Format/Frame Count/
   Checksum・Journal 整合性の Finalizer 検査）。
4. `FinalizedSourceRef` を JSON へ serialize → tmp→rename で原子公開
   → Finalize record 追記 → journal sync → dir sync attempt。
5. `state = Finalized`。`outcome`: 全 source が frontier に含まれる
   完全録音は `complete`、途中 `Failed` の stream は verified prefix
   のみで `partial`。

### `FinalizedSourceRef`（Project 非依存）

```json
{
  "schema_version": 1,
  "session_id": "recording-session-<hex>",
  "stream_index": 0,
  "capture_stream_id": "capture-stream-<hex>",
  "lane_id": "lane-<hex>",
  "sample_rate": 48000,
  "pcm_format": "f32le-interleaved",
  "channels": 2,
  "media": "streams/stream-0000.pcm",
  "media_byte_len": 12345,
  "source_frames": 1234,
  "outcome": "complete" | "partial",
  "spans": [
    {
      "pass_index": 0,
      "session_start": 0, "session_end": 48000,
      "source_start": 0, "source_end": 48000,
      "project_in": 0, "project_out": 15360,
      "media_offset": 0, "media_len": 384000
    }
  ],
  "digest": "<u64 hex>",
  "journal_seq": 7,
  "checkpoint_index": 3,
  "faults": []
}
```

`SourceId`/`AssetId`/`TakeId` 等の Project 参照を持たない。VS-D は
`capture_stream_id`/`lane_id`/`pass_index`/`session`/`source` range
と `media`/`digest` から `RegisterSegment`/`MarkTakeVerified` を構成
する。

### `RecoveryScanner`

`scan(root) -> RecoveryReport`:

1. staging root 配下の session dir を列挙（manifest があるもの）。
2. 各 stream: journal replay → intact prefix・`StopReason`・最後の
   Checkpoint record（durable frontier）・Chunk record 列。media
   実長との照合。
3. stream 単位の復旧分類:
   - `Healthy`: checkpoint があり frontier まで media/journal 整合。
   - `Partial`: frontier 以降に validated な Chunk record 残、
     または unverified tail（media > journaled frontier）がある。
   - `Corrupted`: replay が BadChecksum/BadMagic/SequenceGap 等で
     停止（最後の intact checkpoint までしか信頼しない）。
   - `Unjournaled`: media は存在するが journal が空/不在
     （orphan media。保護・報告のみ）。
4. `RecoveryReport`:
   `sessions[].streams[].{stream_index, classification,
   durable_frontier{checkpoint_index, source_frames, media_len},
   verified_ranges(Vec<SessionRange/SourceRange>)、
   validated_tail_frames, unverified_tail_bytes, stop_reason, faults}`。
   未検証 byte を「復元した」とは報告しない。

`recover_finalize(area)`: reopen した StagingArea の各 stream に
ついて、durable frontier までの検証を行い `partial` として
`finalized.json` を公開（frontier が無い stream は未 finalize。
既に finalized があれば `AlreadyFinalized` で報告のみ）。

### `faultinj`

`StorageWriter` trait（`write_all`/`sync_data`/`stream_len`/`truncate`
不使用の append モデル）を File が実装。`FaultWriter` が File を wrap
しスクリプト化注入:

- `FailWrite { at_call }`: N 回目の write で `ErrorKind::StorageFull`
  または generic error。
- `ShortWrite { call, bytes }`: 指定 call で一部だけ Ok(短) を返し、
  続く write を失敗（write_all の途中失敗を再現）。
- `FailSync { call }`: sync_data を失敗。
- `TruncateAfter { bytes }`: N byte 書いた時点で以後の write を
  全て失敗（torn tail 用に record 途中で止める）。実際には write
  を byte 境界で切る。
- `JournalCorrupt`（試験側 util）: ファイル上の指定 byte を反転
  （checksum mismatch 注入）。
- `WriterStall { hold }`: `StagingWriter` ではなく PoC/試験側で
  pump を N 回遅延し filled queue を満杯にする。

Process Kill 試験は注入ではなく `recording-poc --vsc-crash-at
<records>` で子 process が N record 書込み後に `std::process::exit`
or abort し、親が recover する。

## Data and Control Flow

```
RT engine (VS-B・変更なし)
  │ filled SPSC (Complete Chunk のみ)
  ▼
StagingWriter::pump   [非 RT]
  ├─ validate continuity (header/spans/cursor)
  ├─ media.write_all (raw f32 LE)         → Written
  ├─ journal.write_all (Chunk record)     → Validated
  └─ free_tx.try_push(chunk)              → pool へ返却
  │
  ├─ checkpoint() [明示]
  │    media.sync → Checkpoint record → journal.sync → dir sync?
  │                                           → Durable
  └─ finish()/finalize()
       replay+再検証 → finalized.json(tmp→rename) → Finalize record
                                            → Finalized
  │
  ▼ crash/restart
RecoveryScanner::scan → RecoveryReport
  └─ recover_finalize → partial FinalizedSourceRef
```

## Non-Functional Constraints

- RT 経路は変更しない（`StagingWriter` は非 RT 側の消費者であり、
  `WriterEnds`/`FaultLatch` の契約は VS-B のまま）。`vs_b_rt_alloc`
  の計測証拠を維持する。
- 全ての file 書込みは `write_all`（短 write を成功扱いしない）。
  append-only・in-place 書換えなし・unverified tail を切り捨てない。
- `unsafe_code` forbid・`clippy pedantic` 適合・新規外部依存なし
  （`serde`/`serde_json` のみ再利用。CRC32 は in-tree）。
- Frame/Tick 演算は `checked_add`/`checked_sub`/`value()` 経由で、
  失敗は `FAULT_ARITHMETIC` 系の contract violation。
- 決定性: wall clock・乱数・環境依存を record/JSON 出力に混入しない
  （`faults`/`capabilities` は観測値として記録してよい）。
- エラーは `WriterError`（`Io`/`SyncFailed`/`ContractViolation`/
  `StreamFailed`/`AlreadyFinalized`/`Corrupt`/`ManifestInvalid`
  `NotFound`）に集約し、Write/Sync を区別する。

## Risks and Trade-offs

- **明示 Checkpoint 依存**: checkpoint を呼ばないと durable frontier
  が進まない。`finish()` の最終 checkpoint と RecoveryReport の
  `validated_tail` 可視化で緩和。周期の実数値は PoC で測る。
- **dir sync 非対応環境**: rename の durability が FS 依存。
  capability を記録して保証範囲を明示（主張しない）。
- **`Finalized` の後に未消費 Chunk が来る異常系**: `pump` は
  `Finalized` stream の Chunk を contract violation 扱い（書込まず
  held）。session の終了管理は呼び出し側責務。
- **source_cursor の単調性だけでは検出できない Fault**: Gate-off
  区間で chunk が出ないのは正常（source は連続のまま）。異常は
  engine 側 fault（queue full 等）で latch が立つため、Writer は
  `fault.bits()` を pump 毎に確認し報告に含める。
- **Chunk 列に seq が無い**: stream 単位 FIFO + header/spans の
  整合検算で代替。journal `seq` が永続化される順序の正本になる。
  VS-B への sequence 追加は後続検討事項として記録する。

## Deferred Design Concerns

- Journal Sync/Checkpoint の自動周期（PoC 測定で決定）。
- WAV/RF64 正式形式と staged raw の移行（Ruviform #191/#192）。
- `FinalizedSourceRef` → `RecordingOp` 変換と冪等 Commit（VS-D/#16）。
- Orphan/Partial media の GC policy（#17 未確定事項）。
- VS-B Chunk への explicit `sequence` field 追加（契約変更のため
  別途 Issue/ADR）。
- 暗号学的 Content Digest（#17/#21 の Asset/Blob 設計で確定）。

## Verification Notes

- `tests/vs_c.rs`: journal codec（encode/decode roundtrip・torn
  tail・crc 破損・seq gap）、staging 配置・capability 検出、
  writer 正常系（Normal/2ch/Multi-stream/Punch+Loop・連続整合・
  PCM bit 一致検算）、checkpoint/finalize・AlreadyFinalized、
  各 faultinj（partial/short/diskfull/syncfail/stall/corrupt）、
  複数 stream 隔離、recovery scan・partial finalize。
- `recording-poc`: VS-C シナリオを追加し `poc_determinism` で
  2 回同一 stdout。crash モード + recover の子 process 試験。
- `mise run fmt-check/check/lint/test/build`、可能なら
  `test-msrv`。
