---
id: "DESIGN-034W8Los2DTXD6ljySt4Rf"
type: "design"
status: "approved"
title: "RCD-01 実機 capture coverage 拡張 design — real-device rig + staging lane"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/real_input.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/tests/common/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-01-real-input-coverage.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-01-real-backend-readiness-design.md"
    - "docs/designs/rcd-01-device-recording-headless-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-real-input-coverage.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-01-real-input-coverage.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/251"
    - "https://github.com/scarecrow173/ruviform/issues/565"
  defers: []
  deferred-by: []
---

# RCD-01 実機 capture coverage 拡張 design

## Overview

先行 slice（`rcd-01-real-backend-readiness`）で `InputAdapter` →
実 `StreamWriter`/staging の結線は fake `DeviceRig` 経由で検証済み、
実機 lane は `InspectionWriter` 止まりの smoke に留まる。本 design
は実機 lane を「実 device → `InputSession`/`InputAdapter` → 実
`StreamWriter` → staging media/journal → checkpoint/finalize」
まで拡張し、2 track・fail-stop・punch 境界・長時間連続性を実機で
観測可能にする。

変更は test 基盤（`tests/common/mod.rs`）と opt-in lane
（`tests/real_input.rs`）に限定し、`src/` の公開契約は変更しない
（API gap が判明した場合のみ最小変更）。

## Detailed Design

### `tests/common/mod.rs` — rig/plan helper の一般化

#### `open_rig_core`（新規 private helper）

`DeviceRig::open` の構築部（staging_streams → `StagingArea::create`
→ `InputSession::open`/`add` → `StreamWriter::create` → sort）を
`&[(&dyn Device, &InputStreamSpec)]` + `sample_rate` を取る private
関数へ抽出する。`Device` は object-safe で
`InputSession::open/add` は既に `&dyn Device` を受けるため、
fake/real 双方が同一経路を通る。

- `sample_rate` は `StagingSessionConfig.sample_rate`（manifest
  権威）に使う。fake rig は従来どおり `RATE`(48k) を渡す。
- 戻り値は `(InputSession, Vec<(u16, StreamWriter)>, StagingArea)`。

#### `DeviceRig`（既存・signature 不変）

`open` は `inputs: &[(&FakeInputDevice, &InputStreamSpec)]` を維持し、
内部で `&dyn Device` へ coerce して `open_rig_core` を呼ぶ。
scripted handle 収集（`instance_id` ごとの open 順 ordinal →
`dev.stream(k)`）は従来どおり fake device 側で行い、`streams`
field・`deliver_all`/`drive`/`finish`（`RATE` 固定）は不変。
既存 fake test の signature/挙動は一切変わらない。

#### `RealDeviceRig`（新規 pub struct）

実機 lane 用 rig。`DeviceRig` と同じ field 構成から fake 専用の
`streams` を除き、`sample_rate` を保持する:

```rust
pub struct RealDeviceRig {
    pub session: InputSession,
    pub writers: Vec<(u16, StreamWriter)>,
    pub area: StagingArea,
    pub plan: PreparedRecordingPlan,
    pub sample_rate: u32,
}
```

- `open(root, session_id, plan, sample_rate, inputs:
  &[(&dyn Device, &InputStreamSpec)], stream_ids)` /
  `open_one(...)` — `open_rig_core` 経由。`inputs` は
  `&dyn Device` のため、同一物理 device の複数 tap（1 spec に複数
  armed tap）も、同一 clock domain の複数 device も表現できる。
- `start_all()`/`stop_all()` — `session.adapters()` 経由で各
  `adapter.stream()` の `start()`/`stop()` を呼ぶ。stop は全 stream
  を試行し最初の error を返す（部分停止を残さない）。
- `pump_all()`/`pump_engines()`/`verify_formats()` — `DeviceRig`
  と同一実装（`session.pump_all(&plan.scheduler)` + writer drain）。
- `run_for(duration, interval) -> usize` — wall-clock で
  `interval` 毎に `pump_all` を回し、fault latch が立てば早期
  return。実機 callback は非同期のため scripted `drive` の代わりに
  時間駆動する。戻り値は pump 回数（観測記録用）。
- `finish()` — `DeviceRig::finish` と同規則（fault 時に
  `record_capture_fault` → checkpoint → `finish_all`）だが
  `finish_all` の `sample_rate` は保持した negotiated rate を使う
  （manifest/finalize ref の rate 権威と一致させる）。

fake handle が無いため deliver/注入駆動は存在しない — PCM は
device callback が供給し、test は wall-clock と fault latch のみを
観測する。

#### plan helper の rate/budget 一般化

`plan_for`/`plan_gated` は `RATE`(48k)・`budget()`・
`max_block_frames=1024` に固定されている。実機は negotiated rate
（iD4 実績 44.1k）と可変 callback block を持つため、共通部を
`plan_at(p, lanes, channels, epoch, gate, stop_tick, sample_rate,
budget, max_block_frames)` として抽出し、既存 2 関数は `RATE`/
`budget()`/`1024` で委譲する（既存 caller 不変）。

実機 lane は `plan_at` を negotiated `sample_rate`・拡大 budget
（`chunk_frames = max_block_frames = 8192`、iD4 実績 ~441 frame/
callback に対し ~18x の余裕。超過は既存 `FAULT_CONTRACT`/
`FAULT_PCM_OVERFLOW` で fail-stop = 正直な検出）で呼ぶ。
`callback_queue` は用途別（通常 32、queue-full 誘発は小さい値）。

#### `verify_capture_integrity`（新規 pub helper）

実機 PCM は `MockPcm` 期待値を持たないため、`verify_capture_pcm`
の構造部分のみを検証する helper を追加する:

- 各 `ChunkRecord` の `media[media_offset..+media_len]` の
  `crc32` == `pcm_crc32`。
- `media_offset` が 0 から連続し `media.len()` と整合。
- 各 `JournalSpan` の `media_offset`/`frame_count` が chunk の
  media 範囲内（`frame_count × channels × 4` byte）。
- PCM sample が全て有限値（NaN/Inf 検出）。
- 戻り値は検証した sample 数（観測記録用）。

`assert_session_coverage`/`assert_source_contiguous`/
`assert_prefix_contiguous`/`span_obs`/`all_journal_spans`/
`capture_of`/`span_frame_total` は rate 非依存のためそのまま再利用
する。

#### `tmp_dir` の common 昇格

`device_recording.rs` ローカルの `tmp_dir` を `pub fn` として
common へ移し、`device_recording.rs` は glob import でそのまま
使う（local 同名 fn は残しても shadow 規則上問題ないが、重複を
避けるため local 定義を削除して common 版に統一する）。

### `tests/real_input.rs` — 実機 lane 拡張

全て `#[cfg(feature = "real-device")]` + `#[ignore]` の
`real_lane` module 内に追加し、既存 `mise run
test-recording-real-input`（`-- --ignored`）に自動で載る。
`mise.toml`/`Cargo.toml` は変更しない。

#### 共通パターン（全 test 共通）

1. `default_host()` → `default_input_device()`（失敗/不在は
   loud SKIP print + return）。
2. `negotiate_input_format(device)`（`None` は loud SKIP）。
3. negotiated `format.sample_rate.get()` を session rate とし、
   `plan_at` + `real_input_config(format)` で spec/plan を構築。
4. `RealDeviceRig::open_one`/`open` → `start_all()` →
   `run_for`/`pump_all` → `stop_all()` → `finish()`。
5. 検証: `!fault.is_faulted()`、journal replay
   CleanEnd/TornTail、span coverage/連続性、
   `verify_capture_integrity`、finalize ref の outcome/
   `source_frames`/`media_byte_len`。
6. 実測値（device 名・format・frame 数・実効 rate・drift・
   pump 回数・NotRun 理由）は `println!` で記録し、PR/Issue に
   転記する。

open+start 成功後に callback が 0 件・契約違反・fault が出た場合は
hard fail（assert）。デバイス不在・権限・format 交渉不能のみ
loud SKIP。

#### 追加 test 一覧

1. `real_two_track_records_both_inputs_to_separate_streams`
   — iD4 stereo の 2 入力を `ChannelMap::mono(0)`/`mono(1)` の
   2 armed tap（別 `stream_index`/lane）で 1 spec に束ね、
   別 staging stream（別 media/journal）へ記録。両 stream が
   capture data を受け Complete finalize + 連続 journal span。
   channel 数 <2 は loud SKIP。
2. `real_device_records_end_to_end_through_stream_writer`
   — negotiated format の全 channel を `ChannelMap::identity` で
   1 stream に録音し、実 `StreamWriter`/staging（media/journal/
   checkpoint/finalize）まで end-to-end。journal replay intact・
   span が session 0 から連続・source 連続・media 長が
   `source_frames × channels × 4` と一致。`build_payload` で
   commit 層の Complete 分類も確認（capture-fault 無しなら
   recoverable 空）。
3. `real_rt_queue_full_fails_stop_and_preserves_prefix`
   — `callback_queue` を小さく（8）構成し、warm phase で正常に
   pump（verified prefix を作る）→ pump を止めて work queue を
   満杯にし `FAULT_QUEUE_FULL` を誘発。fault 後に stream を
   継続駆動しても新規 span が増えないこと、capture-fault
   marker・verified prefix 連続性・`build_payload` の
   recoverable 分類を確認。queue が満杯にならない環境
   （callback が来ない等）は timeout で loud fail せず観測値を
   記録して fail —「誘発できなかった」は Pass にしない。
4. `real_incompatible_format_and_rate_open_rejected`
   — negotiated format から (a) 非 F32 `sample_format`、(b)
   supported range 外の `sample_rate`、(c) 未対応
   `ChannelLayout`/channel 数、(d) `BufferSize::Fixed` を構成し、
   `device.open_input_stream` が `Err(DeviceError::
   UnsupportedConfig)`/`Unsupported` 系で reject するか、
   adapter 層 `InputAdapter::open` が `InputBindError` で
   fail-closed することを確認。backend が変換受理する場合は
   その事実を `Unsupported`/`NotRun` として記録し Pass にしない
   （Mock 受理を実機 Pass に昇格しない）。
5. `real_punch_boundaries_are_sample_exact`
   — `GateSpec::Punch` で実機 capture し、journal span の
   session/source/project 座標が `plan.mapping.
   tick_to_media(punch_in/out)` から計算した期待値と Sample
   単位で一致すること、media が punch 区間のみを含むことを検証。
   境界値は negotiated rate で計算するため rate 非依存。
6. `real_bounded_long_capture_monitors_continuity`
   — `GateSpec::Normal`・stop tick を約 30 秒相当（tick は
   rate 非依存の音楽時間）に設定し bounded capture。frame
   連続性（fault 無し）・journal span 連続・source 連続・
   `verify_capture_integrity` を確認し、実測 frame 数・wall
   clock・実効 rate・pump 回数を記録。span 総数が
   `session_end` に達しない場合は drift/drop として記録して
   fail（黙って pass にしない）。
7. `real_multi_device_same_clock_records_when_available`
   — `host.input_devices()` が 2 台以上を返す環境でのみ、
   `InputSession::open`+`add` で same-clock 検証を通した 2
   stream 録音を試行。`ClockDomainMismatch`/`ClockDomainUnknown`/
   `Device(_)` は `NotRun` 記録で return（#119 先取り禁止・
   実機相関は blocked）。単一 device 環境は loud SKIP。
8. `real_pcm_fidelity_records_honest_status`
   — deterministic PCM source（OS loopback 等）が利用可能かを
   判定し、無ければ `NotRun`/`Unsupported` を print 記録して
   return。実機 PCM を `MockPcm` 等の捏造期待値と比較しない
   ことを明示する diagnostic test。

既存の `real_input_device_records_through_input_adapter`（
`InspectionWriter` smoke）と `real_input_probe_reports_timestamp_fields`
（観測 probe）はそのまま維持する。

### File Map

- `crates/synvelis-recording/tests/common/mod.rs`:
  `open_rig_core` 抽出、`RealDeviceRig` 追加、`plan_at` 追加 +
  `plan_for`/`plan_gated` 委譲化、`verify_capture_integrity`
  追加、`tmp_dir` 昇格。
- `crates/synvelis-recording/tests/real_input.rs`:
  `real_lane` module に上記 8 test 追加 + 必要 import。
- `crates/synvelis-recording/tests/device_recording.rs`:
  local `tmp_dir` を common 版へ置換（重複解消のみ）。
- `crates/synvelis-recording/src/*`: 変更なし（API gap が
  出た場合のみ最小変更）。
- docs: spec/design/plan/task/test-spec/impl IR。

## Non-Functional Constraints

- RT callback 契約不変（allocation/dealloc・blocking・I/O・
  panic 禁止）。queue-full 誘発は非 RT の pump 停止で行い、
  callback 側の挙動は既存契約の観測に留める。
- `#[ignore]` opt-in 維持。既定 `mise run test`/`check`/`lint`/
  `test-rt`/`test-msrv`/`build` は hardware 非依存。
- 実機 PCM を deterministic 期待値と比較しない。検証は構造
  （journal replay・span/source 連続・crc32・media 長・
  finalize outcome）+ 観測値記録に限定。
- `NotRun`/`Unsupported`/`blocked` は print + doc 記録で明示し、
  暗黙の pass にしない。
- 時間演算は `Instant`/`Duration` の wall-clock のみ（tick/frame
  演算は `TickTimeMapping`/`SessionFrame` の既存 checked 経路）。

## Risks and Trade-offs

- **`BufferSize::Default` の可変 block size**: negotiated rate で
  `chunk_frames=max_block_frames=8192`（~186ms@44.1k）を上限とし、
  超過は `FAULT_CONTRACT`/`FAULT_PCM_OVERFLOW` で fail-stop。
  iD4 実績 ~441 frame に対し十分な余裕。block が上限を超える
  device は正直に fail する（Pass にしない）。
- **queue-full 誘発の非決定性**: callback 周期は backend 依存。
  `callback_queue=8` + pump 停止で ~80ms（10ms 周期想定）以内に
  満杯になる設計とし、timeout（10s）まで fault が立たなければ
  観測値を記録して fail — 「誘発不能」を Pass にしない。
- **44.1k 等の非 48k rate**: tick→frame は `plan.mapping.
  tick_to_media` で rate 別に計算し、境界期待値を rate 非依存に
  する。session rate・staging manifest・finalize ref の rate を
  negotiated 値で一致させる。
- **session_end 超過 block**: engine は `finished()` で早期
  return し尻尾を評価しない（`eval_end` clamp）— 実機が
  session_end を跨ぐ block を供給しても fault にならず、
  span は session_end で正確に切れる。
- **multi-device clock domain**: `same_clock_domain` 検証を
  緩和しない。UNKNOWN/不一致は `NotRun` 記録（#119 範囲）。
- **PCM fidelity**: deterministic source 不在を `NotRun`/
  `Unsupported` として記録し、構造検証のみで Pass を構成する。

## Deferred Design Concerns

- physical USB 抜去・device removal の自動検証 | 実機操作の
  自動化不能 | 手動 double-opt-in lane 整備時 | removal 時の
  fail-stop が未検証のまま残る。
- cross-clock multi-device 相関・3ch 超 multi-channel | iD4 は
  2in・#119 が clock 相関を所有 | #119 / 対応 hardware 取得時 |
  multi-device 証拠が same-clock に限定される。
- OS loopback 等 deterministic PCM source | 環境依存で保証不能 |
  deterministic source 利用可能時 | PCM fidelity が NotRun のまま。
- macOS/Linux 実機 lane | ruviform#251 Stage M/L 未到達 |
  Stage M/L 完了時 | 実機証拠が Windows/WASAPI に限定される。

## Verification Notes

- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `build` — 既定 lane 全 pass
  （hardware 非依存）。
- [ ] `mise run lint-recording-real-input` — feature 有効
  clippy pass。
- [ ] `mise run test-recording-real-input` — 実機 lane 実行。
  iD4 接続時は各 test の Pass/NotRun/Unsupported を記録、
  不在時は loud SKIP で binary が落ちないこと。
- [ ] `mise run check-app-native` / `lint-app-native` — app 側
  無変更の確認。
