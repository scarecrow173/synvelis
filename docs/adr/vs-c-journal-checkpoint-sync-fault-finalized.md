---
id: "ADR-034QfJdymYmhzfa9PFs8fD"
type: "adr"
status: "accepted"
title: "VS-C Journal形式・Checkpoint Sync方針・Fault隔離・Finalized出力"
created: "2026-11-01"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/15"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/17"
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
    - "docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md"
    - "docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/ruviform/issues/191"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# VS-C Journal形式・Checkpoint Sync方針・Fault隔離・Finalized出力

## Context and Problem Statement

VS-C（Synvelis #15）は VS-B が出力する `CaptureChunk` 列を非 RT Writer
が Staging Media へ保存し、Journal/Checkpoint/整合性検証/障害後
Recovery を実装する。ここで固定すべき構造は (1) Staging 配置と
Journal の粒度、(2) Journal の物理 Record 形式、(3) Media 保存形式、
(4) Checkpoint/Sync の順序と Durability 意味論、(5) 検証 Checksum、
(6) Stream 単位の Fault 隔離、(7) `FinalizedSourceRef` の公開形式である。
Checkpoint 周期・Sync 頻度等の実数値は PoC 測定後に確定するため、
本 ADR は構造・意味論・所有権のみを決定する。

## Decision Drivers

* #15 実装範囲: 「`RecordingSessionId` ごとに Manifest/Journal/
  Streams を隔離」「Journal は Version・Record Length・Sequence・
  Checksum を持つ追記 Record 形式を候補とし、途中で破れた末尾 Record
  を検出して最後に検証可能な状態から復旧できる」。
* #15 Fault/Recovery Gate: Chunk Sequence/SourceFrame/Channel/Byte
  offset の連続整合・Raw PCM 内容 Checksum・Partial Write/Short
  Write/Disk Full/Process Crash/Journal truncated tail/Checksum
  mismatch を注入。Reopen では Last Valid Checkpoint まで復旧し
  未検証末尾を確定 Source へ追加しない。
* #17 §6 保証境界: write 成功 / sync 成功 / HEAD 切替 / directory
  durability / 電源断耐性はそれぞれ異なる。OS write 成功を durable
  扱いせず、全 Project Save との一括 Atomic 性も主張しない。
* #17 §8 Orphan 保護: Verified Orphan を保持し、二重 Finalize を
  作らない。
* Multi-track 障害で一部正常 Stream が全廃棄されない（#15）。
* VS-B ADR: Fault は共有 Atomic Latch + fail-stop。Writer 側の所有権
  （free 返却・保持 chunk の退役照合）を維持する。
* unsafe_code forbid・新規外部依存は最小限（workspace 規約）。

## Considered Options

Journal 粒度と Staging 配置:

* L-1（採用）: `<root>/<session-id>/manifest.json` +
  `streams/stream-<NNNN>.pcm|journal|finalized.json`。Manifest は
  session 単位、Journal は **stream 単位**。
* L-2: session 単位の単一 Journal が全 stream の Record を時系列に保持。
* L-3: stream ごとに独立した session 領域（session 単位の Manifest
  を持たない）。

Journal Record 形式:

* J-1（採用）: 固定長 Header + 可変 payload + CRC32 の binary 追記
  Record: `[magic u16]["JR"][version u8][kind u8][seq u64]
  [payload_len u32][payload][crc32 u32]`。crc32 は magic 以外の全
  field+payload を対象。replay は header 不完全・magic/len/crc/seq
  不整合で停止し、そこまでを検証可能 prefix とする。
* J-2: JSON Lines（1 Record = 1 行 + 行 checksum）。復旧・検査は容易。
* J-3: SQLite/組込 DB にイベントを記録。

Media 保存形式:

* M-1（採用）: stream 単位の Raw PCM 追記ファイル（interleaved f32
  little-endian）。Manifest に format/channels/sample_rate を記録。
* M-2: s16/s24 等の量子化 PCM。
* M-3: WAV/RF64 コンテナ（Ruviform #191 を前提）。

Checkpoint/Sync 方針:

* C-1（採用）: Checkpoint は呼び出し側の明示操作
  （`checkpoint()` + Finalize 時の最終 Checkpoint）。順序は
  「Media `sync_data` → Checkpoint Record 追記 → Journal
  `sync_data` → Directory Sync（capability 検出時のみ）」。
  `sync_data` 失敗は Write 失敗と別種の `SyncFailed` として報告し、
  Checkpoint Record を発行しない。
* C-2: Record 追記ごとに自動 sync。
* C-3: 時間間隔/バイト閾値で Writer が自動 sync。

Checksum 構成:

* H-1（採用）: CRC32（IEEE・in-tree 実装・新規依存なし）を (a)
  Journal Record frame、(b) Chunk ごとの PCM bytes に適用。Media
  全体の整合は Chunk CRC32 を決定的 fold（`acc = acc*K ^ crc`）した
  u64 digest で表現する。
* H-2: SHA-256 等の暗号学的ハッシュ（`sha2` 等新規依存）。
* H-3: Checksum を付けず長さと Sequence のみで検証。

Fault 隔離:

* F-1（採用）: Writer エラーは (a) その stream の writer 状態を
  `Failed` にする、(b) 共有 `FaultLatch` に `FAULT_WRITER_ERROR`
  （契約違反は `FAULT_CONTRACT`）を立て全 engine を fail-stop、
  (c) 他 stream の writer は独立に drain を継続し健全な Validated
  Range を保全する。
* F-2: 1 stream の Writer 障害で全 staging を即時停止・全 stream の
  書込みを中断。
* F-3: Writer 障害を latch せず該当 stream のみ黙って停止。

Finalized 出力:

* Z-1（採用）: `FinalizedSourceRef` を JSON（serde・`deny_unknown_
  fields`）で `stream-<NNNN>.finalized.json` に tmp→rename で原子公開。
  既存なら再 Finalize せず `AlreadyFinalized` を返す（冪等）。
* Z-2: Journal 末尾に Finalized Record を追記するだけ（marker file
  を作らない）。
* Z-3: session 単位の単一 finalized manifest に全 stream を集約。

## Decision Outcome

Chosen option: "L-1 + J-1 + M-1 + C-1 + H-1 + F-1 + Z-1"。

* L-1: stream 別 Journal は「一部正常 Stream が全廃棄されない」を
  構造で保証する。stream A の torn tail が stream B の検証可能
  prefix を汚染しない。L-2 は時系列の一貫性に利点があるが、破損点
  以降の全 stream 記録を失い、Fault 隔離と矛盾する。Manifest が
  session 単位の記述（format 表・capability 記録）を担う。
* J-1: 「途中で破れた末尾 Record を検出する」要件は frame 長+CRC の
  binary 形式で最も単純かつ厳密に満たせる。J-2 の JSONL は可読だが、
  行の途中書き切りと「不完全ながら JSON として成立する行」を区別
  するために結局 per-line checksum と同型の機構が要り、binary より
  検証が複雑になる。J-3 は依存と回復面倒の両方で過剰（YAGNI）。
* M-1: Raw f32 LE は `CaptureChunk.pcm` のメモリ表現と 1:1 で、
  量子化誤差・変換バグの検証対象を持ち込まず「元 Source として
  保全」に一致する。M-2 はサイズ半減だが不可逆変換で元 PCM の検算を
  壊す。M-3 は #191 の責務であり #15 が明示的に除外している。
* C-1: write/sync/durable の区別を型と Record で正直に表現できる
  唯一の構成。C-2 は性能を測定しないまま最頻 sync を選ぶことになり、
  C-3 は時計依存で決定的試験と相性が悪い。明示 checkpoint は
  orchestrator（PoC/将来の Job 層）の責務として残し、Finalize が
  最終 Checkpoint を必ず挟む。
* H-1: CRC32 は IEEE 標準・in-tree 約 40 行・依存ゼロ。Chunk 単位
  CRC があれば Recovery の tail 再検証が record 単位で閉じる。
  digest は暗号学的強度を主張しない（integrity 検出用）。H-2 は
  staging 内部整合の用途に過剰。H-3 は Fault/Recovery Gate の
  「Raw PCM 内容 Checksum」を満たせない。
* F-1: 共有 latch が「録音全体を止める」既存契約（VS-B）を維持し
  つつ、writer の per-stream 状態機械が「どの stream のどこまで
  Validated か」を独立に記録する。F-2 は正常区間の放棄で要件違反、
  F-3 は fail-stop 契約の破壊。
* Z-1: Finalized marker を独立ファイル + atomic rename にすると、
  「Journal 末尾の Finalized Record まで書けたが rename 前に死んだ」
  中間態を残留せず、reopen での判定が marker の有無+内容再検証に
  単純化される。Z-2 は publish と journal append の atomicity を
  主張できない。Z-3 は stream 単位 Finalize と二重公開の境界を
  複雑化する。

### Consequences

* Good: Journal replay が「固定長 header を読み、len/crc/seq で打切る」
  だけの閉じた手続きになり、Fault Injection（truncated tail・
  破損・欠番）の試験が決定的。
* Good: per-stream 状態機械と共有 latch の分担で、Multi-track 障害の
  検証が stream 単位の報告に写像できる。
* Good: `FinalizedSourceRef` が Project 型を一切参照しないため、
  VS-D の「Verified Blob 公開と ProjectMutation の境界」を
  ファイル 1 枚の契約として渡せる。
* Bad: 明示 checkpoint は呼び出し忘れで「Written だが Durable でない」
  区間が伸び得る。Finalizer が最終 checkpoint を強制し、Recovery
  Report が durable frontier と written/validated tail を区別表示する
  ことで可視化する（周期の実数値は PoC 測定で確定）。
* Bad: Directory Sync は std API では全 OS で実行できない
  （Windows では `File::open(dir)` が失敗する）。capability 検出で
  `unsupported` を正直に記録し、保証を過剰主張しない。
* Bad: f32 LE は s16 比で 2 倍の容量。PoC 候補として受け入れ、正式
  形式は #191 に委譲。
* Bad: digest（u64 fold）は衝突耐性を主張しない。Content Digest の
  正式規格は #17/#21 の Asset/Blob 設計で確定する。

### Confirmation

* `mise run check`/`lint`/`test` が緑で、Journal の truncated tail・
  checksum 破損・seq 欠番が replay で検出される。
* Partial Write/Short Write/Disk Full/Sync 失敗/Writer Stall の各
  注入で、該当 stream の Validated Range が保持され他 stream が
  継続する。
* Process Kill 試験（実子プロセス異常終了）で reopen した
  RecoveryScan が Last Valid Checkpoint まで復旧し未検証末尾を
  Finalized 候補に含めない。
* Sync 失敗注入で `SyncFailed` が Write 失敗と区別され、
  Checkpoint Record が発行されない。
* Finalize を 2 度呼んでも `AlreadyFinalized` で新しい出力が
  作られない。

## Implementation Plan

* Affected paths:
  * `crates/synvelis-recording/src/` — `crc32.rs`/`journal.rs`/
    `staging.rs`/`writer.rs`/`recovery.rs`/`faultinj.rs`（新規）、
    `lib.rs`（export 追加）。
  * `crates/synvelis-recording/tests/` — `vs_c.rs`（受け入れ試験）。
  * `crates/recording-poc/` — VS-C シナリオ・crash モード・
    `tests/`（Process Kill 試験・決定性試験の更新）。
  * `docs/` — spec/ADR/design/plan/test-spec/tasks/impl chain。
* Patterns to follow:
  * 追記のみ・in-place 書換え禁止の append-only ファイルモデル
    （`DirectoryBundleStore::write_atomic` の tmp→sync→rename と
    同じ publish 規約）。
  * `serde`/`serde_json` の workspace 依存を再利用（新規依存なし）。
  * `SessionFrame`/`SourceFrame`/`MusicalTick`/`Range` の checked API。
  * VS-B の WriterEnds/FaultLatch/所有権規約（free 返却・保持 chunk
    の退役照合）をそのまま消費。
* Patterns to avoid:
  * `std::io::Write` の `write` 部分書込みを成功扱いする経路
    （`write_all` のみ・short write は注入側も本物もエラー）。
  * write 成功を durable と呼ぶ命名・状態。
  * 破損検出時に媒体を黙って切詰め/削除する挙動（unverified tail は
    保護・報告のみ。削除は GC 責務で対象外）。
  * Journal をデータ Queue や `Vec<u8>` 全保持のメモリモデルにする
    設計（追記 replay 前提）。
* Dependencies or configuration: 新規外部依存なし（CRC32 は
  in-tree）。fault 注入は公開 `faultinj` モジュールとして提供し
  cfg(test) に閉じない（PoC の crash モードが使う）。

## Verification

* [ ] `mise run test` で Journal codec/staging/writer/recovery/
  fault 注入の全受け入れ試験が緑。
* [ ] `recording-poc` の VS-C シナリオが 2 回同一出力で、crash
  モード→recover の流れが決定的。
* [ ] 実子プロセス異常終了後の Recovery Scan が Last Valid
  Checkpoint を frontier とし unverified tail を除外する。
* [ ] 2 stream 中 1 stream の書込み失敗で他 stream の Validated
  Range が失われない。

## Pros and Cons of the Options

### Binary framed Journal（J-1）

* Good: torn tail 検出が厳密・単純。record 境界・長さ・checksum が
  一体で、破損点を byte 精度で特定できる。
* Bad: 人間可読でない（replay dump ツールは recovery.rs が担う）。

### JSONL Journal（J-2）

* Good: 目視検査・差分 debug が容易。
* Bad: 不完全行と有効行の区別に checksum/終端記号が必須で、結局
  binary と同型の機構。serde の parse コストが record ごとに掛かる。

### Per-stream Journal（L-1）

* Good: Fault 隔離・stream 単位復旧・partial stream 保護が構造準拠。
* Bad: Record 群の全 session 時系列は後付け（manifest の stream 表 +
  各 seq で再構成。VS-C では要求されない）。

### 明示 Checkpoint（C-1）

* Good: 呼び出し側の意図が durability 境界になる。決定的試験と整合。
* Bad: 自動周期がないため orchestrator 依存（Deferred: 周期数値は
  PoC 測定で決定）。

## More Information

* [Synvelis #15](https://github.com/scarecrow173/synvelis/issues/15)
  実装範囲・完了条件・Fault/Recovery Gate。
* [Synvelis #17](https://github.com/scarecrow173/synvelis/issues/17) /
  `docs/designs/project-asset-persistence-architecture.md` §6/§8/§9
  （保存プロトコル・Media Lifecycle・Crash Consistency Matrix）。
* [Synvelis #16](https://github.com/scarecrow173/synvelis/issues/16)
  Verified Blob 公開と ProjectMutation の境界。
* VS-B ADR `docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md`
  （FaultLatch・失敗 push 返却・Chunk 所有権の前提）。
* [Ruviform #191](https://github.com/scarecrow173/ruviform/issues/191)
  実 WAV Writer（本決定の範囲外・後続移行先）。
