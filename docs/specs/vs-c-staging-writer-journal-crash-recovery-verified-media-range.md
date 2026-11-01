---
id: "SPEC-034QfIc1oyjYo7pSqlYH8a"
type: "spec"
status: "approved"
title: "VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range"
created: "2026-11-01"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/15"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/14"
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/21"
    - "https://github.com/scarecrow173/synvelis/issues/22"
    - "https://github.com/scarecrow173/ruviform/issues/191"
    - "https://github.com/scarecrow173/ruviform/issues/192"
  defers: []
  deferred-by: []
---

# VS-C Staging Writer / Journal / Crash Recovery — Verified Media Range

## What Should Be Built

Recording Vertical Slice **VS-C**（Synvelis Issue #15・実行計画 #23 の
E2E-R Staging/Recovery Gate 相当）として、VS-B が出力する
`CaptureChunk`/`CaptureSpan`/`ChunkValidity`/`FaultLatch` 契約を
**非 RT Writer** が消費し、Raw PCM Staging + Journal + Crash
Recovery によって **Validated と Durable を区別した Verified Media
Range** を `synvelis-recording` に実装する。

- `RecordingSessionId` ごとに Manifest/Journal/Stream Media を隔離した
  Staging 領域。PoC 候補形式は **Raw PCM（interleaved f32 LE）+
  メタデータ** とし、WAV/RF64 等の正式保存形式は本 Gate の前提にしない
  （実 WAV Writer は ruviform #191 が後続で所有）。
- Chunk ごとに Session/Stream/Sequence/`SourceFrame`/Channel/Byte
  Offset の連続整合を検証し、決定的順序で Stream Media へ追記する。
  入力 PCM は元 Source として保全し、エラー時に欠落をサイレント補完しない。
- Version・Record Length・Sequence・Checksum を持つ追記型 Journal。
  途中で破れた末尾 Record を検出し、**最後に検証可能な状態**から復旧する。
- 保存状態 `Captured/Enqueued/Written/Validated/Durable Checkpoint/
  Finalized/Project Committed` を区別する。OS の write 成功を
  Durable とみなさず、Sync 失敗を正確に報告する。全 Project Save との
  一括 Atomic 性も主張しない（Project Commit は VS-D/#16）。
- Writer/Stream 単位の Verified/Partial/Invalid、有効 `SourceFrame`
  範囲、Discontinuity・Queue Fault の原因を記録する。
  Multi-track 障害で一部の正常 Stream が全廃棄されない。
- Byte Length/Format/Frame Count/Checksum・Journal 整合性を検査する
  Finalizer が、VS-D へ渡す `FinalizedSourceRef` 等の **Project 非依存
  出力** を構成する。
- Process 再起動時に Staging を列挙し、有効 Media と最後の検証済み
  Journal Range を照合する Recovery Scan。保存済みでない Frame を
  「復元した」と報告しない。

## Why Now

- VS-A（#13・PR #29）と VS-B（#14・PR #30）が main にマージされ、
  Chunk/Sequence/Validity/Fault 契約が確定した。VS-C はその契約の
  非 RT 消費者であり、Staging→Verified Media→Recoverable Orphan→
  Finalize の状態機械を提供する責務（#17 §8 の Media Lifecycle）を負う。
- 後続の VS-D（#16）は「Verified Blob 公開と ProjectMutation の境界」で
  接続するため、`FinalizedSourceRef` の Project 非依存出力形をここで
  固定しなければ引き渡し境界が確定しない。
- Journal format/version/checkpoint/sync policy と FS ごとの保証は
  PoC/測定で選択する方針が #15・#17 に明記されており、実装判断の
  記録先（ADR）と検証基盤が必要。

## Users and Value

- **VS-D/#16（冪等 Project Commit）:** `FinalizedSourceRef`・Valid
  Range・Orphan/Partial の状態をそのまま ProjectMutation 登録へ渡せる。
- **レビューア / Coordinator:** Fault/Recovery Gate（Partial Write・
  Short Write・Disk Full・Process Crash・Journal truncated tail・
  Checksum mismatch）を `cargo test --workspace --locked`（mise 経由）
  と `recording-poc` で決定的に再現できる。
- **将来の Recovery UI/Job 層:** Recovery Scan の報告が「何が検証済みで
  何が未検証末尾か」を区別し、復元を偽装しない出力を再利用できる。

## Scope

### In Scope

- `synvelis-recording` への追加: Journal codec（Version/Record
  Length/Sequence/Checksum・追記・replay・破損検出）、Staging 領域管理
  （session 別 Manifest・stream 別 Raw PCM・Journal・Finalized 出力）、
  非 RT `StagingWriter`（SPSC drain・連続整合検証・決定的書込み・
  free 返却・所有権保持）、Checkpoint/Sync プロトコル（Media Sync →
  Checkpoint Record → Journal Sync → Directory Sync capability 記録）、
  Finalizer（`FinalizedSourceRef` 生成・原子公開・二重 Finalize 防止）、
  Recovery Scan（reopen 列挙・Journal 照合・Last Valid Checkpoint 復旧・
  未検証末尾の除外と保護）。
- Fault Injection: Partial Write・Short Write・Disk Full・Flush/Sync
  失敗・Writer Stall・Journal 中断（truncated tail）・Checksum
  mismatch・Process Crash（通常 Error 試験と別 Process Kill 試験）。
- `recording-poc` での決定的 VS-C シナリオ実行と受け入れ試験。
- FS 保証の調査・記録（Atomic Rename・File Sync・Directory Sync の
  capability を実行環境で検出して Manifest/Report に記録する。
  万能な電源断保証は主張しない）。

### Out of Scope

- Project への冪等 Source/Take/Clip 参照登録・Undo・正式 Save・
  `RecordingCommitIdentity`（VS-D/#16）。
- WAV/RF64 等の正式 Media 保存形式（Ruviform #191）、Peak/Waveform/
  Proxy 等の再生成 Cache。
- 実 Device Input・MIDI・VCS 連携・正式な Media GC Policy・GUI。
- 性能閾値の最終確定・大規模データでのベンチマーク最適化。
- 複数 Process 同時 Staging・Project Lock・Cloud Sync 上の保証強化。

## Requirements

- VS-B の Chunk 契約をそのまま消費する（`CaptureChunk`/
  `CaptureBlockHeader`/`CaptureSpan`/`ChunkValidity`/`WriterEnds`/
  `FaultLatch`）。Writer は RT Callback 側に File I/O・Heap alloc・
  Blocking を持ち込まない（RT 経路は VS-B のまま不変）。
- 書込み順序は決定的: 各 stream について filled 受信順 → PCM 追記 →
  Journal Record 追記 → free 返却。`SourceFrame` は stream 内で
  `[0, total)` の連続・重複なしを Writer 側でも検証し、`SessionFrame`
  の単調性・span の `packed_pcm_offset` と実 Byte Offset・Channel 数・
  `stream_index` の整合を Chunk 単位で確認する。
- 保存状態を厳密に区別する: `Captured`（engine 生成）/`Enqueued`
  （filled queue 内）/`Written`（Media write 成功・未 sync）/
  `Validated`（対応 Journal Record が永続化され Checksum で再検証可能）/
  `Durable Checkpoint`（Media+Journal Sync 成功の Checkpoint Record）/
  `Finalized`（検査済み出力が原子公開）/`Project Committed`（VS-D。
  本 slice では到達しない）。
- Checkpoint は「Media `sync_data` → Checkpoint Record 追記 →
  Journal `sync_data` → Directory Sync（capability があれば）」の順序で
  行い、各段の成功/失敗/非対応を正確に記録・報告する。write 成功のみを
  durable と呼ばない。
- Journal は Version・固定長 Header・Record Length・Sequence・
  Checksum（CRC32 以上）を持つ追記形式。replay は破れた末尾 Record・
  Checksum 不一致・Sequence 欠番を検出し、最後の検証可能位置で停止する。
- Writer エラー後も既存の Validated Range と Orphan/Partial Media を
  保護し、再実行/Recovery で二重 Finalize を作らない（Finalized 出力は
  冪等・AlreadyFinalized を報告）。
- Recovery Scan は Process 再起動を想定した列挙で、Last Valid
  Checkpoint までを復旧 frontier とし、未検証末尾（Journal に intact
  record が無い・または media bytes と照合しない末尾）を確定 Source
  候補へ追加しない。欠落・部分・健全の原因を stream 単位で記録する。
- Fault Injection は決定的: 注入点・回数・種別をスクリプト化し、
  Error 注入試験（in-process）と Process Kill 試験（実子 Process の
  異常終了）を分ける。
- 検証は `mise run <task>` 経由、Commit は Conventional Commits・
  論理単位分割。
- Mock/実 FS 上の成功を、全 OS/FS・電源断・ハードウェア故障の保証と
  混同しない（capability を検出・記録し、保証範囲を明示する）。

## Acceptance Criteria

- [ ] Normal・2ch・Multi-stream・Punch+Loop の各録音で、Media に保存
  された全 Frame の `SourceFrame`/`SessionFrame`/Pass 対応を Journal
  と実 PCM から検証できる（元 PCM と bit 一致の検算を含む）。
- [ ] 再生可能な Staging 形式（Raw PCM + Manifest + Journal）で元 PCM
  とメタデータを非破壊保存できる（正式 Project 登録を含まない）。
- [ ] Partial Write・Writer Stall・Queue Fault を検出し、保存済み
  Stream/Segment 別の Valid Range を報告できる。
- [ ] Journal の Truncation/Checksum 破損が検出され、Last Valid
  Checkpoint まで回復できる。
- [ ] Write 成功と実際の Durability を混同せず、Sync 失敗が正確に
  表示される（Write 成功・Sync 失敗・非対応 capability を別状態として
  報告）。
- [ ] 複数 Stream の一部が失敗しても、正常保存区間（他 Stream の
  Validated/Durable Range）を破棄しない。
- [ ] Crash/Restart 試験（実子 Process の異常終了を含む）で、再検証
  可能な Finalized Source 候補を返し、未検証末尾を確定 Source へ
  追加しない。
- [ ] Writer Error 後の既存 Validated Range と Orphan/Partial Media
  が保護され、再実行/Recovery で二重 Finalize が作られない。
- [ ] Writer は RT Callback に File I/O・Heap alloc・Blocking を
  持ち込まない（RT 経路の VS-B 計測証拠が維持される）。
- [ ] `recording-poc` が headless で決定的に実行され、2 回の実行で
  同一出力となる。

## Deferred Design Concerns

- 正式 Media 形式（WAV/RF64）への移行（Ruviform #191 実装後、Raw
  PCM+Manifest 候補との互換方針を再評価）。
- `RecordingCommitIdentity`・冪等 Project Commit・Media GC/Retention
  （VS-D/#16）。
- Checkpoint 周期・Journal Sync 周期・Channel 数/容量の最終数値
  （PoC 測定後に確定。本 spec は注入可能な構成のみ固定する）。
- Network FS/Cloud Sync 上の Atomic Replace/Directory Sync 保証、
  Multi-process Staging Lock。
- Journal の一般 Project Store Checkpoint 連携（#17 §15 の未確定事項）。

## Sources and Relations

- 正本 Issue: [Synvelis #15](https://github.com/scarecrow173/synvelis/issues/15)
  （実装範囲・完了条件・Fault/Recovery Gate・実装判断）。
- 実行計画: [Synvelis #23](https://github.com/scarecrow173/synvelis/issues/23)
  E2E-R Staging/Recovery Gate の受入境界。
- 設計正本: [Synvelis #12](https://github.com/scarecrow173/synvelis/issues/12)・
  `docs/designs/project-asset-persistence-architecture.md`（#17・Media
  Lifecycle/状態区別/Orphan 保護/Crash Consistency Matrix）。
- 依存 spec: `docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md`
  （Chunk/Sequence/Validity/Fault 契約）、
  `docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md`
  （Stable ID・時間型）。
- 後続境界: [Synvelis #16](https://github.com/scarecrow173/synvelis/issues/16)
  （Verified Blob 公開と ProjectMutation の境界）。
- 非依存を明記: [Ruviform #191](https://github.com/scarecrow173/ruviform/issues/191)
  （実 WAV Writer）は本 Gate の前提にせず Raw PCM+Manifest 候補を先行検証する。
- 関連: [Synvelis #17](https://github.com/scarecrow173/synvelis/issues/17)・
  [#20](https://github.com/scarecrow173/synvelis/issues/20)・
  [#21](https://github.com/scarecrow173/synvelis/issues/21)・
  [#22](https://github.com/scarecrow173/synvelis/issues/22)・
  [Ruviform #192](https://github.com/scarecrow173/ruviform/issues/192)。
