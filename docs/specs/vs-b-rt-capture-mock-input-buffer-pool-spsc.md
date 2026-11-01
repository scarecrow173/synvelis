---
id: "SPEC-9EcPwSoDwcrg3WPYmvs7Gn"
type: "spec"
status: "approved"
title: "VS-B Clock / Gate Scheduler / RT Capture — Mock Input・Buffer Pool・SPSC"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/14"
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
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md"
    - "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/13"
    - "https://github.com/scarecrow173/synvelis/issues/15"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/ruviform/issues/187"
    - "https://github.com/scarecrow173/ruviform/issues/190"
  defers: []
  deferred-by: []
---

# VS-B Clock / Gate Scheduler / RT Capture — Mock Input・Buffer Pool・SPSC

## What Should Be Built

Recording Vertical Slice **VS-B**（Synvelis Issue #14・実行計画 #23 の E2E-R
R1/R2 相当）として、実 Device なしの Mock Clock/Input が発生させる決定的
Frame 列を、`Preflight → Prepared Gate Schedule → RT Capture →
事前確保 SPSC Handoff` の経路で扱い、Normal/Punch/Loop の正確な保存候補
（Frame Sequence・Channel・Session/Source Frame 対応・Validity）と、
Queue/Span/Free Pool Overflow・Clock Epoch 断絶時の安全な Fail-stop を
検証する基盤を `synvelis-recording` に実装する。

- 非 RT `RecordingRequest → preflight → PreparedRecordingPlan`。
  Track/Lane/Input Binding、Format、Clock Epoch、Buffer/Span 容量、
  TempoMap Revision、Gate/Loop Range を検証し、録音中の Capture Target を固定する。
- `SessionFrame` は Pre-roll 中も単調増加、`SourceFrame` は実際の有効
  Capture 区間でのみ進む。Loop 周回ごとに Pass を識別し、同じ Project
  時間への対応を保持する。
- 半開 `[in,out)` の Gate を、Callback Block 途中の In/Out/Loop Wrap を
  含めて Sample 単位で一意に分類する Prepared Gate Schedule。
  固定容量 `CaptureSpan` 配列による非割当・有界表現。
- 入力 Frame 番号・Channel 番号から期待値を復元できる Deterministic
  PCM Generator。固定/変動 Block Size、Mono/Stereo、同 Clock の
  Multi-track Projection。
- 録音開始前に `CaptureChunk` と PCM 領域を事前確保し、RT→Writer の
  `filled` と Writer→RT の `free` の 2 本の SPSC（それぞれ
  Producer/Consumer 単一所有）で所有権を巡回させる。
- Queue Full/Free 枯渇/Span 容量超過/Clock Epoch 断絶を、RT を待機
  させない Atomic Fault Latch + Fail-stop として扱う。失敗 `try_push`
  が返した Heap-owned Chunk を RT 上で Drop せず、固定 Slot で非 RT
  停止・退役まで保持する。Fault 通知を満杯のデータ Queue に依存させない。
- Frame Sequence / Clock Epoch / 入力 Frame 範囲 / Gate Span /
  Validity を Writer 側（VS-C/#15）へ渡せる Chunk 契約。

## Why Now

- #12 §17/§18 で Mock-first Vertical Slice（Mock Clock/Input → Gate →
  bounded handoff → Writer）の実装方針と VS-A→VS-B→VS-C→VS-D の分割が
  合意済み。VS-A が main にマージされ、Stable ID・時間型・Project 基盤が
  利用可能になった。
- VS-C（Writer/Journal/#15）が消費する Chunk/Sequence/Validity/Fault
  契約をここで固定しなければ、後続工程の引き渡し境界が確定しない。
- ruviform-rt の SPSC（`channel`/`Producer`/`Consumer`、Full 時に
  payload を返す `try_push`）が既存の利用候補として合意済みで、
  実 Device Input（Ruviform #187）・Span Scheduler（Ruviform #190）は
  本 Gate の前提にしない。

## Users and Value

- **VS-C/#15（非 RT Writer/Journal）:** filled Chunk の Header/Span/
  Validity/Fault 契約と、Writer→RT `free` 返却の所有権規約をそのまま消費できる。
- **レビューア / Coordinator:** Issue #14 の Gate 項目が
  `cargo test --workspace --locked`（mise 経由）で決定的に再現できる。
- **R4 以降の実 Device 統合:** RT 契約（非 Block・非割当・Fail-stop・
  危険 Drop なし）が Mock で先に検証済みの経路へ、Device Input を差し替える
  だけで乗せられる。

## Scope

### In Scope

- `synvelis-recording` への追加: `RecordingRequest`/preflight/
  `PreparedRecordingPlan`、Mock Clock（Clock Epoch + 連続入力 Frame
  カウンタ）、Prepared Gate Schedule（有界・半開・Pass 識別・Lookahead
  補充と補充不能時の安全停止）、`CaptureChunk`/`CaptureBlockHeader`/
  `CaptureSpan`、Buffer Pool + 2 本の SPSC Handoff、Atomic Fault Latch、
  RT 側失敗 Chunk の固定 Slot 保持と非 RT 回収照合。
- Deterministic PCM Generator・Mock Input（固定/変動 Block Size、
  Mono/Stereo、Multi-track 同 Clock Projection）と Fault Injection。
- `recording-poc` での決定的シナリオ実行と、Issue #14 の全 Gate 項目を
  網羅するテスト（RT alloc/lock/I/O/危険 Drop の計測証拠を含む）。
- #15 が消費する Chunk/Sequence/Validity/Fault 契約の定義。

### Out of Scope

- Media 永続化・Staging/ジャーナル（VS-C/#15）、冪等 Project Commit と
  Recovery（VS-D/#16）、実 Device Input（Ruviform #187）、MIDI Event
  （Ruviform #188）、Monitoring DSP、GUI、Plugin。
- Writer が実際にファイルへ書く処理（VS-B の Writer 側は Chunk を
  受け取り検証して free 返却する検査ハーネスまで）。
- 性能閾値・Chunk サイズ・Queue 容量の最終確定（PoC 測定後に決定。
  本 spec は容量を注入可能な構成のみ要求する）。
- 複数独立 Device Clock の Drift 保証（同一 Clock Domain の単一 Epoch
  を前提とし、断絶は Fail-stop）。

## Requirements

- VS-A の型を再利用し、時間型・Stable ID を重複実装しない
  （`SessionFrame`/`SourceFrame`/`MusicalTick`/`ProjectTime`/`Range`/
  `ClockEpochId`/`LaneId`/`TempoMapRef`/`TickTimeMapping`）。
- RT 経路（callback 内で実行される全ての処理）は、heap alloc/dealloc・
  lock・OS I/O・panic・無界ループを行わない。事前確保した有界表現のみを
  触り、`try_push`/`try_pop`/atomic/固定長 copy のみを使う。
- Gate は半開 `[in,out)` で、同一 Sample が 2 つの保存区間に二重帰属
  しない。Gate 外の Sample は正常な PCM や無音として自動保存しない。
- `SessionFrame` は pre-roll 中を含めて単調増加し、Loop Wrap・Fault・
  Fail-stop 後も巻き戻さない。`SourceFrame` は保存が確定した有効
  Capture 区間でのみ進む（Gate off 区間・欠落区間を詰めた連続値）。
- SPSC の Producer/Consumer はそれぞれ 1 Owner。Chunk の唯一の Owner が
  RT/Queue/Writer の間を移動し、所有権喪失が起きないことを
  停止・退役時の総数照合で検証する。
- Fault は `filled`/`free` データ Queue に依存しない独立した Atomic
  Latch で通知する。Fail-stop 後の callback は有界な no-op とし、
  RT を待たせない。
- Clock Epoch 断絶・入力 Frame 不連続は異常として報告し、連続
  Timestamp を捏造しない（P0 は単一 Clock Epoch の録音を前提とし、
  断絶・レート変更は Fail-stop）。
- Chunk 契約は `sequence`/`epoch_id`/`session_frame_start`/`input_frames`/
  `stored_frames`/Gate Span 列/Validity を含み、Writer 側が Frame/Channel
  ごとの重複・欠落を検算できる。
- 検証は `mise run <task>` 経由、Commit は Conventional Commits・論理単位分割。
- Mock の成功を実 Audio Device/Backend の時刻精度・RT 安全性の証拠と
  混同しない（未検証 Backend を「検証済み」と記載しない）。

## Acceptance Criteria

- [ ] Normal 録音の全 Sample/Channel が生成関数と一致し、順序・重複・
  欠落がない（Channel/Frame ごとの重複欠落検出を含む）。
- [ ] Block 途中 Punch In/Out（例: 257 Frame callback、in=113）で、
  保存区間と Packed PCM 長が一致し、Gate 外は保存されない。
- [ ] 3 周 Loop（120 BPM・4/4・48 kHz・4 小節 = 384,000 Frame ↔
  15,360 Tick/周）で `SessionFrame` が [0,384000),[384000,768000),
  [768000,1152000) と単調増加し、Pass ごとの Project Mapping が一致する。
- [ ] Punch+Loop 複合・変動 Block Size・Tempo 境界・Pre-roll 負
  Project Time で半開境界が一意に分類される。
- [ ] Queue Full・Free 枯渇・Span 容量超過が非 Blocking に Atomic Fault
  Latch + Fail-stop となり、RT 上で Heap Drop・alloc・OS I/O がないこと。
  失敗 `try_push` の返却 Chunk が固定 Slot で保持され、非 RT
  Shutdown/Retire で回収総数が照合される。
- [ ] Clock Epoch 断絶・入力 Frame 不連続が異常として報告され、連続
  Timestamp を捏造しない。
- [ ] SPSC の Producer/Consumer 1:1 所有と、停止・退役時の全 Buffer
  回収（pool 総数照合）を試験する。
- [ ] RT 無割当・禁止 Drop を計測する（counting allocator 等で alloc/
  dealloc 0 件を実測）。未計測 Backend を「検証済み」と記載しない。
- [ ] `recording-poc` が headless で決定的に実行され、2 回の実行で
  同一出力となる。
- [ ] #15 が安全に消費できる Chunk/Sequence/Validity/Fault 契約が
  公開 API として定義される。

## Deferred Design Concerns

- 実 Chunk サイズ・Queue 容量・Span 上限・Lookahead 長の最終数値
  （Fault 試験の結果で確定。本 spec は注入可能な構成のみ固定する）。
- Writer/Staging/Journal の物理形式と Sync 保証（VS-C）。
- `RecordingCommitIdentity`・冪等 Commit・Media GC/Retention（VS-D）。
- 有限 Lookahead と Transport 連携の本実装（Ruviform #190 系）、複数
  Clock Domain・Sample Rate 変更の継続録音（断絶時の部分復旧ポリシーは
  VS-C 以降）。
- 実 Device 入力での xrun/欠落検出（Ruviform #187 PoC）。

## Sources and Relations

- 正本 Issue: [Synvelis #14](https://github.com/scarecrow173/synvelis/issues/14)
  （実装範囲・受入テスト・実装/故障 Gate・終了条件）。
- 実行計画: [Synvelis #23](https://github.com/scarecrow173/synvelis/issues/23)
  E2E-R RT Capture の受入境界。
- 設計正本: [Synvelis #12 §13・§17・§18](https://github.com/scarecrow173/synvelis/issues/12)
  （時間分離・Mock-first・3 層分離・bounded handoff・Fail-stop・VS-01 仕様）。
- 依存 spec: `docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md`
  （VS-A の時間型・Stable ID・Project 基盤・TempoMap Mapping）。
- 関連: [Synvelis #20](https://github.com/scarecrow173/synvelis/issues/20)
  （Session Frame/Source Frame/Clock Epoch を Loop Wrap で巻き戻さない
  合意）、`docs/designs/time-transport-midi-automation-plugin-integration-architecture.md`。
- 非依存を明記: [Ruviform #187](https://github.com/scarecrow173/ruviform/issues/187)
  （実 Device Input）・[Ruviform #190](https://github.com/scarecrow173/ruviform/issues/190)
  （Span Scheduler）は本 Gate の前提にしない。
- 後続: [Synvelis #15](https://github.com/scarecrow173/synvelis/issues/15)
  が本 spec の Chunk 契約を消費する。
