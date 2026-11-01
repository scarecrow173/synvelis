---
id: "DESIGN-8OW4JtYUYaLPOTZ9ZGxXuz"
type: "design"
status: "approved"
title: "VS-B Clock / Gate Scheduler / RT Capture — Mock Input・Buffer Pool・SPSC"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
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
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
    - "docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/14"
    - "https://github.com/scarecrow173/synvelis/issues/15"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  defers: []
  deferred-by: []
---

# VS-B Clock / Gate Scheduler / RT Capture — Mock Input・Buffer Pool・SPSC

## Context

VS-A で `synvelis-project`（Stable ID・`SessionFrame`/`SourceFrame`/
`MusicalTick`/`ProjectTime`・半開 `Range`・`ClockEpoch`・`TempoMapData`/
`TickTimeMapping`）と `synvelis-recording`（`RecordingOp`・Session 状態機械・
参照検証）が実装済み。VS-B はその上に、Mock Clock/Input→Prepared Gate
Schedule→RT Capture→事前確保 SPSC Handoff の経路を追加する。spec「VS-B
RT Capture」(approved) と ADR「VS-B SPSCプリミティブ・Gate Schedule表現・
Fault通知・Chunk所有権」(accepted: Q-1+G-1+F-1+O-1+S-1) を実装可能な形に
落とす。Writer 永続化・Project Commit は VS-C/VS-D の範囲で、ここでは
Chunk 契約を渡せる検査ハーネスまでとする。

## Scope

### In Scope

- `synvelis-recording` 新モジュール: `bounded` / `plan` / `clock` / `gate` /
  `capture` / `handoff` / `engine` / `mock`。
- `ruviform-rt` git 依存の追加（`channel`/`Producer`/`Consumer`/
  `TryPushError`）。
- `recording-poc` への VS-B 決定的シナリオ追加と全 Gate 項目テスト。

### Out of Scope

- 実 Writer/Journal/Staging/ファイル I/O、Project Commit、Recovery。
- 実 Device Input、複数 Clock Domain、Monitoring、MIDI、GUI。
- 性能チューニング・実数値の最終確定。

## Components and Boundaries

### `bounded`（crate-internal）

`unsafe_code = "forbid"` の workspace 規約により `MaybeUninit` ベースの
固定配列表現は使えない。`BoundedVec<T>` は `Vec::with_capacity(capacity)`
で一度だけ確保し、`push` が `len == capacity` で `Err(BoundedFull)` を返す
薄いラッパ。`len` は初期 capacity を超えないため、pre-allocated 後の
`push`/`remove(0)` は再割当てを起こさない（`Vec` は縮小しない）。
`clear()` は再割当てしない（len を 0 に戻すだけ）。RT 経路で使用する
`BoundedVec` は全て engine/chunk のフィールドとして plan/handoff 時に
構築され、callback 中に新規構築しない（per-call の `with_capacity` は
禁止 = heap alloc になる）。`remove(0)` は O(capacity) の memmove で
有界。用途: Gate window・engine の piece スクラッチ・chunk 内 span 列・
退役照合用の id 収集（非 RT）。

### `plan`（非 RT）

```rust
pub struct CaptureTarget {
    pub lane_id: LaneId,
    pub channels: u16,          // 1..=8（preflight で検証）
    pub seed: u32,              // Mock PCM generator seed（per-stream 識別）
}
pub enum GateSpec {
    Normal { open: MusicalTick },
    Punch { r#in: MusicalTick, out: MusicalTick },
    Loop { start: MusicalTick, end: MusicalTick },
    PunchInLoop { punch_in: MusicalTick, punch_out: MusicalTick,
                  loop_start: MusicalTick, loop_end: MusicalTick },
}
pub struct BufferBudget {       // per-stream（各 stream 固有の pool/queue）
    pub chunks: usize,            // pool 総数 = free queue 容量（厳密に一致）
    pub chunk_frames: u32,        // 1 chunk が収容する最大 frame
    pub max_block_frames: u32,    // 入力契約: 1 block の最大 frame（<= chunk_frames）
    pub spans_per_chunk: usize,   // 1 chunk の固定 span 容量 = piece scratch 容量
    pub filled_capacity: usize,   // filled SPSC 容量（<= chunks）
    pub lookahead_spans: usize,   // Gate window 容量（>= 1）
}
pub struct RecordingRequest {
    pub targets: Vec<CaptureTarget>,   // target index = stream_index
    pub clock_epoch_id: ClockEpochId,
    pub sample_rate: u32,              // > 0
    pub transport_start: MusicalTick,  // session frame 0 の project 位置（負可）
    pub gate: GateSpec,
    pub stop: MusicalTick,             // session 終了の project 位置
    pub tempo_map: TempoMapData,       // project の snapshot と同一 revision/ppqn
    pub budget: BufferBudget,          // 全 stream 共通の PoC 予算
}
```

`preflight(request, project) -> Result<PreparedRecordingPlan, Error>` が検証:

- `targets` 非空、全 target の `lane_id` が `project.lanes` に存在、
  `1 <= channels <= 8`。
- `clock_epoch_id` が `project.clock_epochs` に存在。
- `sample_rate > 0`、`transport_start < stop`（tick 単位）。
- `request.tempo_map.revision == project.tempo_map.revision` かつ
  `request.tempo_map.ppqn == project.ppqn.0`（録音時点の snapshot を
  固定する意図を検証）。`TickTimeMapping::new` で compile 可能。
- gate 区間: `transport_start` 以降で妥当 — Normal `open >= transport_start`
  かつ `open < stop`。Punch `in < out`、`in >= transport_start`、`in < stop`
  （`out > stop` は `stop` に clamp = 録音停止まで採取）。Loop
  `start < end`、`start < stop`（`loop_end > stop` は最終 partial pass として
  `stop` で打ち切りを許容）。PunchInLoop は上記を複合し
  `loop_start <= punch_in < punch_out <= loop_end`。
- `session_end`（stop tick → frame 変換・transport_start 分減算）が
  0 frame より大きい。
- `budget`: 全容量非 0、`max_block_frames <= chunk_frames`、
  `filled_capacity <= chunks`、`lookahead_spans >= 1`。free 容量は
  `chunks` と厳密に一致（`> chunks` にすると二重返却検出が遅れるため。
  `WRITER_ERROR` 検出の前提）。
- provisioning 契約（診断用注記）: 各 stream の pool `chunks` は
  「writer が同時に保持してよい chunk 数 <= filled_capacity + engine
  in-flight 1」を前提にドライバが枯渇前に writer を pump する。
  transient な free empty を fault と区別するのではなく、pool が
  drain された時点で fail-stop とするのが合意（#12 §17.1-4）。

`PreparedRecordingPlan` は Capture Target/Clock Epoch/TempoMapRef/
budget を固定し、`pub scheduler: GateScheduler`（非 RT・span 生成器）を
持つ。`session_end: SessionFrame`・`stream_count`・`sample_rate` 等を
公開し、preflight 後の Capture Target/TempoMap の暗黙変更はない。

### `clock`（Mock Clock / 入力ブロック）

```rust
pub struct InputBlock {
    pub epoch: ClockEpochId,
    pub first_input_frame: u64,   // device 側連番（0 起点でなくてよい）
    pub frames: u32,
}
```

Engine は `expected_epoch`・`next_input_frame`・`seeded: bool` を持ち、
**最初の block で `first_input_frame` を baseline として採用**する
（実カウンタの開始値に依存しない）。以降の block は
`block.epoch != expected_epoch` → `EPOCH_DISCONTINUITY`、
`block.first_input_frame != next_input_frame` → `INPUT_DISCONTINUITY`
で Fail-stop（連続 Timestamp は捏造しない）。P0 は単一 Clock Epoch
前提（#12 §13.2「断絶は録音停止」基本に従う）。

### `gate`（Prepared Gate Schedule）

```rust
pub struct GateSpan {                    // tempo 一様区間に分割済み
    pub session_in: SessionFrame,
    pub session_out: SessionFrame,       // 半開 [in,out)
    pub pass_index: u32,
    pub project_in: MusicalTick,         // session_in の project 位置（負可）
    pub project_out: MusicalTick,        // session_out の project 位置
}
pub struct GateSchedule {                 // engine 内・RT で読む window
    spans: BoundedVec<GateSpan>,          // lookahead_spans 容量
    scheduled_until: SessionFrame,        // 分類済み保証の水位（下記契約）
}
```

`scheduled_until` 契約（安全の中核）: **session frame < scheduled_until の
全ては分類確定**（emitted span 内 = open、それ以外 = closed）。`GateScheduler::
extend(schedule, until)` は、「次に生成すべき span の `session_in`」を内部
状態として追跡し、window に空きがある限り `session_in < until` の span を
順序生成して push し、最後に `scheduled_until = min(until, next_span_in)`
とする。window が満杯なら watermark は次 span の `session_in` で止まり、
その先は未生成として underrun で安全停止する（silent drop しない）。
次 span が存在しない（gate 終了後）は `next_span_in = 無制限` 扱いで
watermark は `until` に追随する（session 終了以降も closed 確定扱い）。
`until` は単調非減少を要求（呼び出し側契約）。

- `GateScheduler`（非 RT）: `TickTimeMapping`・GateSpec・transport_start・
  session_end を保持。境界変換は `tick_to_media(tick, 1, sample_rate)` で
  frame 値を得て transport_start の frame を減算（ruviform の丸め
  NearestEven を境界規則として記録。tempo event 単位の連続性で隣接
  span の境界 frame は一致し、半開で二重帰属しない）。
- tempo event を跨ぐ gate 領域は tempo 境界で分割し、各 GateSpan 内で
  tick↔frame が線形（tempo 一様）になることを保証する。
- `Loop {start,end}` / `PunchInLoop`: 1 周分の span パターンを pass_index
  を進めて反復生成（全周展開しない）。pre-loop 区間
  （transport_start〜loop_start）は span を出さず gate closed。
  `session_end` 超過分は span を `session_end` で打ち切る（partial pass）。
  pass_index は「有効通過」のみに割り当て、frame 0 個の pass は作らない。
- `evaluate_gate(schedule, block_range, out: &mut BoundedVec<CapturePiece>)`:
  呼び出し側（engine）が確保済みの out（capacity = spans_per_chunk）に
  piece `{pass_index, session_start, frame_count, project_in, project_out}`
  を書く。window 先頭から `session_out <= block.start` の span を消費後、
  block 半開区間と交差する各 span について piece を生成（gap = gate
  closed で piece を出さない）。`block.end > scheduled_until` なら
  `GateError::Underrun`、out.push 失敗なら `GateError::SpanCapacity`。
  piece の project 端点は親 span 内の線形補間（i128 中間計算・floor）で、
  piece が span 全体なら端点は完全一致。
- 戻り値はエラーか `()`。piece 列は engine の scratch に残り、
  per-call の heap 確保を行わない。

### `capture`（Chunk 契約 — #15 が消費）

```rust
pub struct CaptureBlockHeader {
    pub sequence: u64,                    // stream 内の発行順（0 起算）
    pub epoch_id: ClockEpochId,
    pub session_frame_start: SessionFrame,
    pub first_input_frame: u64,
    pub input_frames: u32,                // この block の入力 frame 数
    pub stored_frames: u32,               // 実際に保存した frame 数
}
pub struct CaptureSpan {
    pub pass_index: u32,
    pub session_start: SessionFrame,
    pub frame_count: u32,
    pub packed_pcm_offset: u32,           // chunk.pcm 内の frame オフセット
    pub source_start: SourceFrame,        // stream 内の確定 source 位置
    pub project_in: MusicalTick,
    pub project_out: MusicalTick,
}
pub enum Validity { Valid }               // Partial/Invalid は VS-C で拡張
pub struct CaptureChunk {
    pub id: u32,                          // stream 内 pool slot id（照合用）
    pub stream_index: u32,
    pub header: CaptureBlockHeader,
    pub spans: BoundedVec<CaptureSpan>,   // spans_per_chunk 容量
    pub validity: Validity,
    pub pcm: Box<[f32]>,                  // interleaved、preflight 確保
    pub stored_samples: usize,            // pcm 内の有効 sample 数
}
```

- `source_start` は stream 内で [0, stored_total) の連続値として engine が
  `next_source_frame` を進めて採番する（Writer が累積復元せずに
  contiguity/重複/欠落を検証できるようにする Chunk 契約の一部）。
- `stored_frames` == `spans` の `frame_count` 合計（不変条件・テスト検証）。
  `stored_samples` == `stored_frames * channels`。
- Gate 外の frame は一切 pcm に書かない（無音合成もしない）。
- Chunk は永続化しない transient handoff 型（serde 不要・Project
  Snapshot には入らない）。

### `handoff`（Buffer Pool + 2 本の SPSC + Fault Latch）

```rust
pub struct FaultLatch(Arc<AtomicU32>);
// bit: QUEUE_FULL=1, FREE_EXHAUSTED=2, SPAN_OVERFLOW=4,
// SCHEDULE_UNDERRUN=8, EPOCH_DISCONTINUITY=16, INPUT_DISCONTINUITY=32,
// WRITER_ERROR=64, CONTRACT=128
//   CONTRACT: 入力契約違反（block.frames > max_block_frames、
//   pcm slice 長不一致）と checked 演算 Err の畳み込み。
```

`FaultLatch::set(bits)`/`bits()`/`is_faulted()` のみ（clear は退役時の
非 RT 再生成のみ）。データ Queue と独立した共有 atomic で、RT は
待たず・割当てずに fail-stop を伝播する。writer（非 RT）側も `free`
への `try_push` が `Full` を返した場合に `WRITER_ERROR` を立てる
（正常系では free 容量 == chunks なので、writer が正当に保持する
chunk の返却が Full になることはない → Full は破損の確定検出）。

`StreamHandoff::new(target, budget, fault) -> (engine 側端点群, writer 側端点群)`
は stream ごとに pool（`chunks` 個の `CaptureChunk`、各 `Box<[f32]>` =
`chunk_frames * channels`）を生成し、全 chunk を `free` Producer 側へ
事前投入（free 容量 == chunks 厳密）。`filled` は `filled_capacity`。
**stream ごとに独立した pool + free/filled の対**（同 Clock multi-track は
同一 InputBlock 列を N engine が消費する形で、SPSC の 1:1 所有を壊さない）。

### `engine`（RT 側 — callback 内で実行される全処理が RT 契約下）

```rust
pub struct CaptureEngine {
    stream_index: u32, channels: u16, seed: u32,
    schedule: GateSchedule,
    free_rx: Consumer<CaptureChunk>,      // free SPSC consumer（1 owner）
    filled_tx: Producer<CaptureChunk>,    // filled SPSC producer（1 owner）
    fault: FaultLatch,
    retained: Option<CaptureChunk>,       // 失敗 push 等の退避 slot（容量 1）
    pieces: BoundedVec<CapturePiece>,     // evaluate_gate 用 scratch（preflight 確保）
    expected_epoch: ClockEpochId,
    next_input_frame: u64,                // 初 block で baseline 採用
    seeded: bool,
    session_cursor: SessionFrame,
    next_source_frame: SourceFrame,       // stream 内連番
    next_sequence: u64,
    max_block_frames: u32,
    faulted: bool,
}
```

`process(&mut self, block: &InputBlock, pcm: &[f32])`（RT 経路 —
alloc/dealloc/lock/OS I/O/panic/無界ループを行わない。
`pcm` 契約: `pcm.len() == block.frames * channels`、interleaved）:

1. `faulted || fault.is_faulted()` → `session_cursor` のみ進めて return
   （有界 no-op: queue/chunk/clock 処理をしない。SessionFrame は session
   時間軸として進行し続けるが、clock 由来の `next_input_frame` は
   更新しない = 断絶を捏造しない。checked 演算 Err 時は進行を止めるだけ）。
2. clock 検証: 未 seeded → `next_input_frame = block.first_input_frame` で
   baseline 採用。`block.epoch != expected_epoch` → EPOCH_DISCONTINUITY。
   `block.first_input_frame != next_input_frame` → INPUT_DISCONTINUITY。
   fault latch + faulted → step1 相当で return。
3. 入力契約: `block.frames > max_block_frames` または
   `pcm.len() != block.frames * channels` → CONTRACT latch + faulted。
4. `evaluate_gate(&mut schedule, [s, s+n), &mut pieces)` →
   `Underrun` → SCHEDULE_UNDERRUN、`SpanCapacity` → SPAN_OVERFLOW、
   latch + faulted（chunk 未着手）。
5. pieces が空（全 gate 閉）→ chunk を pop せず step 9 へ（cursor 進行）。
   gate off の無音を chunk 化せず pool を消費しない。
6. `free_rx.try_pop()` → `Empty` → FREE_EXHAUSTED latch + faulted。
   `chunk.stream_index != self.stream_index` → WRITER_ERROR latch +
   retained 退避 + faulted（誤配 chunk を所有権喪失せず退役へ）。
7. chunk 状態リセット（`spans.clear()`・`stored_samples=0`・
   `stored_frames=0`・`validity=Valid`・header 記入）。piece ごとに
   `pcm` の該当区間を `chunk.pcm` へ copy（interleaved、
   `packed_pcm_offset` 累積）、`spans.push(record)`（piece 数 <=
   spans_per_chunk は step4 で保証済みなので失敗しない）。
8. `filled_tx.try_push(chunk)` → `Full(chunk)` → QUEUE_FULL latch +
   retained 退避 + faulted。
9. `session_cursor += n`、`next_input_frame += n`（`SessionFrame`/
   `u64` の checked 演算。Err → CONTRACT latch + faulted）。
   gate 閉のみでも両 cursor は進める（連続性の正本）。

- retained 容量 1 は「最初の Fault で停止」設計により充足
  （step 6–8 の fault 経路はいずれも faulted を立て、以後 step1 の
  no-op になるため、同時に 2 個目の退避は発生しない）。
- `next_source_frame` は emit した frame 数分だけ進み、gate off/
  Fault 区間は詰めない（連続・重複なしの [0, stored_total)）。

非 RT 側 API（callback 外でのみ使用）:
- `engine.schedule_mut()` — `GateScheduler::extend` の受け口（refill）。
  PoC では単一スレッド pump で「RT callback 間の非 RT タイミング」を
  模す。将来は snapshot publish 等で分離する拡張余地を残す。
- `engine.teardown()` — `retained` と `free_rx` drain を回収に渡す。

### `mock`（Deterministic Input / Fault Injection / Driver 補助 — 非 RT）

```rust
pub struct MockPcm { seed: u32 }
impl MockPcm {
    pub fn sample(&self, input_frame: u64, channel: u16) -> f32;
    // (frame*P ^ channel*Q ^ seed*R) & 0xFFFF → [0,1)。全 (frame,ch,seed)
    // で復元可能・決定的。
}
pub struct MockDriver;        // block 列生成 + refill/pump の orchestration
pub struct InspectionWriter {
    filled_rx, free_tx, fault,
    held: Vec<CaptureChunk>,           // 現在所有する chunk（未返却分）
    log: Vec<ChunkLog>,                // 受領 metadata の検証用記録
}
pub struct ChunkLog {                  // 所有権移転後も検証可能な写し
    pub header: CaptureBlockHeader,
    pub spans: Vec<CaptureSpan>,       // metadata 複製（非 RT での Vec 使用可）
    pub pcm_checksum: u64,             // stored_samples の簡易チェックサム
    pub validity: Validity,
}
```

- `InspectionWriter::pump()` は filled を drain → `log` に metadata を
  記録 → 原則 free へ返却（返却しても検証は `log` が担う）。Fault
  injection: `hold(n)` で返却保留（free 枯渇誘発）、`inject_duplicate`
  で同一 id の偽 chunk 生成+返却（WRITER_ERROR/照合異常誘発）、
  `misroute` で他 stream の free へ返却（stream_index 不一致誘発）。
- `accounting()`（退役照合・非 RT）: stream ごとに `engine.retained` +
  `free_rx` drain + `filled_rx` drain + `writer.held` の chunk id が
  重複なく pool 総数 `chunks` と一致することを検証。偽造/喪失は
  id 重複・不足として検出する。

## Data and Control Flow

1. `preflight(request, &project)` → `PreparedRecordingPlan`（Target/Epoch/
   TempoMapRef/Budget/session_end 固定、`scheduler` 準備済み）。
2. stream ごとに `StreamHandoff::new` → pool 確保・全 chunk を free 投入・
   両 SPSC 作成・`CaptureEngine` 組立・`InspectionWriter` 組立（全非 RT）。
3. シナリオループ（block ごと・決定的）:
   a. 非 RT: `scheduler.extend(engine.schedule_mut(), until)`。
   b. 非 RT: `MockPcm` で block の interleaved PCM を生成（
      `frames * channels`）。
   c. RT: 各 stream の `engine.process(&block, &pcm)`。
   d. 非 RT: `writer.pump()`（drain→log→free 返却）。
4. 終了/故障後: `teardown()` → `accounting()` で総数照合 → 収集 `log` を
   検証（sample 一致・`source_start` 連続・重複/欠落なし・pass mapping）。

## Non-Functional Constraints

- RT 経路（`CaptureEngine::process` + `evaluate_gate` + SPSC try_* +
  atomic + 固定長 copy）で heap alloc/dealloc・lock・OS I/O・panic・
  無界ループを行わない。証拠: counting allocator（dev-dep `stats_alloc`
  の `StatsAlloc<System>`/`Region`）で alloc/dealloc 0 件を計測する。
  計測は専用 test binary（`tests/rt_safety.rs` 単独 test）で並行 noise
  を排除し、計測 window 前に endpoints/コードを warm-up する
  （初回呼出しの lazy 初期化を window 外に出す）+ counter が計数
  することを示す control を設ける。lock/I-O は「RT モジュールが
  Mutex/fs/println/所有物の Drop を持たない」コード監査を
  impl record に記録する。
- 全 Session/Source frame 演算は checked API 経由（Err は CONTRACT
  fault）。境界の tick→frame 変換は ruviform `tick_to_media`
  （NearestEven 丸め規則を記録）を用いる。
- 決定的: 乱数・時刻・実デバイス非依存。mock PCM は (frame,ch,seed) の
  純粋関数。
- 単一 Clock Domain・単一 Epoch 前提。Multi-track は同一 InputBlock 列を
  複数 stream engine が同一 session 基準で消費する。
- 退役照合では各 stream の chunk id が pool 総数と一致する
  （per-stream 集合の完全性）。

## Risks and Trade-offs

- `BoundedVec` は「capacity 超過しない」運用規約で非割当を実現する。
  push は `Err` を返すため取りこぼしは Fault で可視化される。
- schedule refill を engine 所有 window への非 RT メソッドとして簡略化
  した（実スレッド分離での publish/retire は後続検討）。RT 側の読み・
  消費ロジック自体は契約どおり有界。
- retained 容量 1 は「最初の Fault で停止」設計に依存（ADR O-1）。
- piece project 補間は tempo 一様 span 内の線形 floor。端点は正確、
  中間は tempo 境界分割により一様性が保証される。
- free 容量を chunks と厳密一致にしたのは `Full` を二重返却の確定検出に
  使うため（slack があると検出が遅れる）。
- `stats_alloc` は global 計測 → 専用 binary + warm-up + control で
  誤検出を排除する。

## Deferred Design Concerns

- 実数値（chunk_frames/chunks/filled_capacity/spans_per_chunk/
  lookahead_spans/max_block_frames）の確定 — Fault 試験の計測結果で
  決める。
- schedule refill の実スレッド分離（snapshot publish/retire）と
  Coordinator 側の補充タイミング。
- `Validity` の Partial/Invalid 拡張、Segment/Take への集約（VS-C/D）。
- 複数 Epoch/clock 切替の継続録音、Writer stall 検出の時間ベース化。

## Verification Notes

- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`build` 全緑。
- [ ] spec AC 全項目を test-spec の対応表どおりに検証。
- [ ] `recording-poc` 2 回実行で同一 stdout。
- [ ] RT 計測（alloc/dealloc 0）と退役照合が緑。
- [ ] `mise run test-msrv`（rust 1.89 toolchain が利用可能な場合）。
