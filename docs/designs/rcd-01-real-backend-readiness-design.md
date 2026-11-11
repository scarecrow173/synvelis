---
id: "DESIGN-89lmf1mLK8flCuybXMmEqR"
type: "design"
status: "approved"
title: "RCD-01 実Audio Device 到達準備 design — stamp placement fallback + real backend composition root"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/src/real_input.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/real_input.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/src/input.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "crates/synvelis-recording/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/input_adapter.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/device_recording.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/input_adapter_rt_alloc.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "Cargo.lock"
  implements:
    - "docs/specs/rcd-01-real-backend-readiness.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-01-device-recording-headless-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-real-backend-readiness.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-01-real-backend-readiness.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/251"
  defers: []
  deferred-by: []
---

# RCD-01 実Audio Device 到達準備 design

## Overview

`InputAdapter` の callback は `InputCallbackInfo` の配置情報を
`InputBlock.first_input_frame` へ写す唯一の境界である。現行は
`frame_position` のみを信頼し、`None` を fail-stop とする。実 cpal
adapter は `frame_position` を常に `None` で報告するため、このままでは
実 device の初回 callback が必ず `FAULT_INPUT_DISCONTINUITY` になる。

本 design は 2 つの変更を行う:

1. **Slice A**: `frame_position` が無い場合に限り、port 契約の
   sanctioned 経路 `capture_stamp()` → `InputStamp` →
   `ClockAnchor` + `map_stamp_to_frame` で frame を導出する fallback
   を `input.rs` に追加する。anchor は open 時（非 RT）に
   `stream.now()` から較正する。**実機実測を受けた改訂**: capture
   stamp は sub-ms の timestamp jitter を持つため、block 毎の独立
   写像は非連続 frame を生み `FAULT_INPUT_DISCONTINUITY` となる
   （後述「実機実測と jitter 許容設計」）。配置は累積 frame count で
   導出し、stamp は初回 seed と連続性/ドリフト検証に限定する。
2. **Slice B**: `src/real_input.rs` に real backend の composition
   root（host 列挙・F32 交渉・`InputStreamConfig` 組立）を置き、
   opt-in 実機 lane（`tests/real_input.rs` + `real-device` feature +
   `mise.toml` task）を追加する。

## Detailed Design

### Slice A — stamp placement fallback（`src/input.rs`）

#### 配置解決の優先順位

callback 内の frame 解決は次の順序（fail-closed、値の捏造なし）:

1. `info.frame_position: Some(pos)` → `pos` をそのまま使う
   （現行経路維持。fake port・将来の frame-position 対応 backend。
   stamp 状態機械は更新しない — position が優先される限り stamp
   経路の検証は走らない）。
2. `frame_position: None` → stamp 検証付き累積配置
   （`resolve_first_frame` + callback 内 `StampState`。後述）。
3. `frame_position: None` かつ配置信号が検証不能
   （`capture_stamp(): None`・anchor 無し・写像 rejection・
   連続性/ドリフト閾値超過）→ `FAULT_*` で fail-stop
   （連続を捏造しない）。

`capture_stamp()` は port 側の単一変換点であり、capture timestamp
不在・foreign base・負値/overflow をすべて `None` に揃える。
`InputTimestamp::callback` は決して stamp として使わない
（buffering latency 分の配置ずれを防ぐ port 契約）。callback 回数
から `frame_position` を合成しない。

#### 実機実測と jitter 許容設計（設計改訂の根拠）

初版設計は「block 毎に `capture_stamp()` → `map_stamp_to_frame` で
frame を導出」だったが、実機 lane（Windows/WASAPI・Audient iD4、
44.1 kHz F32 stereo）で以下を実測した:

- `frame_position` は常に `None`（cpal adapter が device frame
  position を `InputCallbackInfo` に載せない — 上流欠落として
  ruviform#565 を起票済み）。
- `capture_stamp()` は有効（domain `ClockDomainId(2)`、anchor
  `now()` ≈ 303,910,495,822,700 ns に対し正の delta）。
- しかし連続 block の stamp 間隔は 10.019 ms / 10.006 ms と
  sub-ms jitter を持ち、独立写像の frame 差は 443/444/439
  （真値 441 frame = 10 ms @44.1 kHz）となった。
- block 毎の独立配置では `first_input_frame` が非連続となり
  `FAULT_INPUT_DISCONTINUITY`（0x40）で fail-stop — 実機 lane が
  正直に失敗として検出した。

PCM block 録音が要求するのは sample-exact な連続 frame 位置であり、
capture timestamp は測定値であって frame 数の権威ではない。したがって
配置は累積 frame count（前 block の `first_input_frame + frames`）で
導出し、stamp は以下の 2 役割に限定する:

1. **初期 seed（1 回のみ較正）**: 最初の有効 stamp を
   `map_stamp_to_frame` で写像し、その frame を先頭 block の
   `first_input_frame` とする。seed 値自体は jitter を含み得るが、
   開始位置の ±数 frame 誤差は録音全体の連続性を壊さない
   （PCM 検算は先頭 frame からの相対位置で行われる）。
2. **連続性/ドリフト検証**: 以後の各 stamp について
   - tick 逆行（`checked_sub` 失敗）→ `FAULT_INPUT_DISCONTINUITY`
   - per-callback 進行量の乖離: stamp delta（frame 換算）と
     直前 block の frame 数の差が
     `STAMP_DELTA_TOLERANCE_FRAMES = 256`（48 kHz で約 5.3 ms、
     実測 jitter ±3 frame を十分吸収しつつ buffer 規模の
     xrun/gap を検出）を超過 → `FAULT_INPUT_DISCONTINUITY`
   - 累積乖離: `map_stamp_to_frame` の写像値と累積期待位置の差が
     `STAMP_ABS_TOLERANCE_FRAMES = 16_384`（48 kHz で約 0.34 秒、
     ~100 ppm drift を 1 時間超吸収しつつ持続 divergence/xrun
     列を検出）を超過 → `FAULT_INPUT_DISCONTINUITY`
   - domain/base/epoch 不一致・`BeforeAnchor`・`Overflow` は
     毎 callback `map_stamp_to_frame` を通して既存の
     `MapRejection` → `FAULT_*` 写像で検出（下表どおり）。

閾値の根拠: `STAMP_DELTA_TOLERANCE_FRAMES` は実測 jitter
（±3 frame @44.1 kHz）に対し 2 桁の余裕を持ち、1 buffer
（~512 frame 級）の取りこぼしを確実に検出する。単発 jitter は
正負に振れるため累積しない（random walk でも 16k frame 窓内に
収まる）。`STAMP_ABS_TOLERANCE_FRAMES` は長時間 drift を許容しつつ
「stamp 時計と frame 数が持続的に乖離する」真の異常のみを捕捉する。

`frame_position` が将来供給された場合（ruviform 上流修正後）は
そちらが本線となり、本経路は fallback として残る。

#### ClockAnchor 較正（非 RT、open 時）

`InputAdapter::open` で stream open・format/domain 検証の後、
`stamp_placement()` が `Option<StampPlacement>` を構築する:

- `anchor.domain` = `stream.clock_domain()`（open 時に検証済みの
  非 UNKNOWN domain。callback の `info.clock_domain` は per-callback
  `same_clock_domain` 照合済みなので `stamp.domain == anchor.domain`
  が保証される）。
- `anchor.stamp_ticks` = `stream.now()` の `MediaTime::value()` を
  `u64` へ変換した値。`now()` 失敗・負値・base が
  `DEVICE_CLOCK_TIMEBASE` でない場合は `None`（anchor 無し）。
- `anchor.stamp_base` = `DEVICE_CLOCK_TIMEBASE`。
- `anchor.frame` = `EngineFrame::new(epoch, 0)`。epoch は本 adapter
  内で完結する固定値（`ClockEpochId::new(0)` 相当の
  `ruviform_media_core::ClockEpochId`。`synvelis_project::ClockEpochId`
  とは別型であり、`InputBlock.clock_epoch` の epoch 整合は engine 側の
  既存機構が担う）。
- `anchor.calibration` = `TimestampProvenance::Derived`
  （「now() 読み取り時点を frame 0 とみなす」宣言由来の較正であり、
  backend が運んだ fact ではないため `Carried` ではない）。
- `frame_base` = `spec.config.format.sample_rate.time_base()`
  （`1/rate`、engine frame の timebase）。
- `now` = `EngineFrame::new(epoch, 0)` — `map_stamp_to_frame` の
  epoch 整合検査専用。較正後に epoch は変わらないため定数。

anchor が構築できない（`now()` 非対応 backend 等）場合は
`stamp_placement = None` とし、callback で stamp が来ても
`FAULT_INPUT_DISCONTINUITY` に落ちる（fail-closed。frame_position
経路は影響を受けない）。

#### MapRejection → FAULT_* 写像

`map_stamp_to_frame` の `MapRejection`（`#[non_exhaustive]`）を
既存 fault bit へ正直に写像する:

| MapRejection | 意味 | FAULT bit |
| --- | --- | --- |
| `UnmappedDomain` | stamp の clock domain に anchor が無い | `FAULT_EPOCH_DISCONTINUITY` |
| `BaseMismatch` | stamp の timebase が anchor と不一致 | `FAULT_EPOCH_DISCONTINUITY` |
| `EpochMismatch` | anchor が stale epoch（再較正なしでは不達） | `FAULT_EPOCH_DISCONTINUITY` |
| `BeforeAnchor` | stamp が anchor より過去（負 delta） | `FAULT_INPUT_DISCONTINUITY` |
| `Overflow` | tick delta / frame 変換の overflow | `FAULT_ARITHMETIC` |
| （将来の variant） | 未知の rejection | `FAULT_CONTRACT` |

根拠: domain/base/epoch の不一致は clock 系列の同一性問題であり、
既存の `same_clock_domain` 不一致 → `FAULT_EPOCH_DISCONTINUITY`、
`DeviceError::ClockDiscontinuity` → `FAULT_EPOCH_DISCONTINUITY` と
同じ分類。`BeforeAnchor` は配置不能な時刻逆行であり frame 連続性の
問題として `FAULT_INPUT_DISCONTINUITY`。`Overflow` はチェック済み
演算失敗として `FAULT_ARITHMETIC`（handoff.rs の定義どおり）。

#### RT 契約

`resolve_first_frame` は `capture_stamp()`（純粋整数）と
`map_stamp_to_frame`（純粋整数・単一 NearestEven 丸め）・
`MediaDuration::convert`（純粋整数）のみを呼び、
allocation/dealloc・blocking・I/O・panic を行わない。anchor と
`StampState` は `Copy`/スカラー値で callback closure に move され、
状態更新は callback 内のローカル mutable 変数のみ（RT thread 間で
共有しない）。

#### 既知の制約（設計上の留保）

- **stamp clock と sample clock の rate 差**: stamp の clock domain
  が `MONOTONIC` 等で sample rate と厳密に同一 rate で進まない場合、
  stamp→frame 換算の drift は累積乖離検査
  （`STAMP_ABS_TOLERANCE_FRAMES`）で監視される。閾値内の drift は
  許容（配置は累積 count のため録音の連続性は保たれる）、閾値超過は
  `FAULT_INPUT_DISCONTINUITY` で fail-stop — 「連続を捏造しない」
  fail-closed 方向の誤り。再較正・drift 補正は本 slice の範囲外。
- **seed 値の jitter**: 初回 stamp の写像値は ±数 frame の誤差を
  含み得る。録音の絶対開始位置が真の最初の capture frame と
  ずれ得るが、block 間連続性と PCM 内容には影響しない。
  `frame_position` 経路が使える backend では発生しない。

### Slice B — real backend 配線（`src/real_input.rs` + lane）

#### 依存戦略

- `ruviform-media-core`: src（`input.rs`/`real_input.rs`）で
  `map_stamp_to_frame`/`ClockAnchor`/`InputStamp`/`EngineFrame`/
  `TimeBase`/`TimestampProvenance`/`MapRejection` を使うため、
  dev-dependency から通常 dependency へ昇格（workspace dep は既存）。
- `ruviform-audio-device`: `optional = true`、feature `real-device`
  で有効化。cpal は Linux で `alsa-sys`（`libasound2-dev` 必須）等の
  OS 依存を引くため、無条件依存にすると headless な既定 lane
  （`cargo test --workspace`、ubuntu CI）の build 要件を変えてしまう。
  opt-in feature に閉じ込めることで既定 lane の依存グラフを不変に保つ
  （`synvelis-app` の `webview`/`native` feature と同じ方針）。
- ルート `[workspace.dependencies]` に `ruviform-audio-device` を
  既存 ruviform git 依存と同じ rev 指定で追加。

#### `src/real_input.rs`（常時 compile 部分）

`ruviform-audio-core` の型だけに依存する部分は feature 無しで常時
compile し、headless test で検証する:

- `negotiate_input_format(device: &dyn Device) -> Option<AudioFormat>`:
  `default_input_format()` が F32 なら採用、さもなくば
  `supported_input_formats()` から F32 range を選択
  （ruviform `tests/real_input_e2e.rs` の同名関数と同じ規則を薄い
  module として移植。println は持たず `Option` を返す）。
- `select_f32_input_format(supported: &[SupportedInputFormat])
  -> Option<AudioFormat>`: F32・妥当な rate range・既知
  `ChannelLayout` 写像可能な range のみを候補に、stereo 優先・
  48 kHz 含有優先で選び、rate は 48 kHz に clamp。
- `real_input_config(format: AudioFormat) -> InputStreamConfig`:
  `buffer_size: BufferSize::Default` 固定。実 cpal adapter は
  `BufferSize::Fixed` を `DeviceError::UnsupportedConfig` で拒否
  し得るため Default を使う。実効 callback block size は可変であり、
  `InputAdapter` の `max_block_frames` 超過は既存の `FAULT_CONTRACT`
  で fail-stop する（spec 側で十分な上限を与えるのは caller 責務）。

#### `src/real_input.rs`（`real-device` feature 部分）

- `pub use ruviform_audio_device::default_host;` — host 生成の
  唯一の入口。`Host` は `DeviceEnumerator` を実装し、
  `input_devices()`/`default_input_device()` で実 device を列挙する。

`InputSession` 結線自体は既存 `InputSession::open`/`add` が担う
（公開 signature 不変）。composition root は「host → device →
format 交渉 → `InputStreamConfig`」までを提供し、spec 構築
（plan/tap/budget は project 側の知識）と session 駆動は caller
（実機 lane test）が行う。

#### 実機 lane（`tests/real_input.rs` + `mise.toml`）

- `tests/real_input.rs` は file-level `#![cfg]` を付けず、
  headless 部（交渉・config・fake device 経由の session 結線検証）を
  既定 lane で常時実行する。実機 test は
  `#[cfg(feature = "real-device")]` + `#[ignore]` の module に隔離し、
  既定 `cargo test` では compile されない。
- `mise.toml` に opt-in task を追加（ruviform の
  `test-audio-real-input`/`l1-real-verify` 先例どおり CI 対象外）:
  - `test-recording-real-input`:
    `cargo test -p synvelis-recording --features real-device
    --test real_input -- --ignored --test-threads=1`
  - `lint-recording-real-input`:
    `cargo clippy -p synvelis-recording --all-targets
    --features real-device -- -D warnings`
    （feature 有効時の compile/lint を headless で検証する経路）。
- 実機 test の SKIP 規則: `default_host()` 失敗・default input
  device 不在・F32 交渉不能は環境的欠如として print + return
  （ruviform lane と同じ「SKIP loudly」）。device があるのに
  open/start/callback/録音経路が契約を満たさない場合は hard fail
  —「open+start だが callback 0 件」は成功ではなく失敗。
- 実機 test が検証するもの: 列挙 → 交渉 → `InputSession::open`
  → `stream.start()` → callback 配信・frame 数・timestamp/domain
  一貫性 → `pump_all` + writer までの実 capture 経路統合
  （fault 無し・span が session 範囲を cover・media 産出）。
  実機 gate 行（fidelity 実測・multi-device 相関・xrun/長時間・
  証拠リンク）は本 slice 範囲外で NotRun 記録。

#### 既存コメントの更新

`tests/common/mod.rs`・`tests/input_adapter.rs`・
`tests/device_recording.rs` の「実 backend は ruviform#251 待ちで
`Unsupported`」という記述を、ruviform#251 が pin `b02942c2` に
含有済みである現状と opt-in lane の存在に合わせて更新する
（fake port 契約である点は変わらない）。

## File Map

- `Cargo.toml`: `[workspace.dependencies]` に
  `ruviform-audio-device` 追加。
- `crates/synvelis-recording/Cargo.toml`: `ruviform-media-core` を
  dev-deps → deps へ昇格、`ruviform-audio-device` optional dep +
  `real-device` feature 追加。
- `crates/synvelis-recording/src/input.rs`: `StampPlacement` 較正・
  `StampState`（seeded/next_first_frame/prev_stamp_ticks/
  prev_frames）・`resolve_first_frame`・`stamp_rejection_fault`・
  `STAMP_DELTA_TOLERANCE_FRAMES`/`STAMP_ABS_TOLERANCE_FRAMES`・
  callback 組込み・unit test。
- `crates/synvelis-recording/src/real_input.rs`（新規）: 交渉・
  config・feature-gated `default_host` re-export。
- `crates/synvelis-recording/src/lib.rs`: `real_input` module 追加、
  新規公開項目の re-export。
- `crates/synvelis-recording/tests/common/mod.rs`: fake stream に
  capture stamp 発射 + 注入 knob（domain/base 不一致・stamp 逆行・
  固定 tick・**決定的 jitter `stamp_jitter_ns`**）追加、
  stale コメント更新。
- `crates/synvelis-recording/tests/input_adapter.rs`: stamp fallback
  系 test 追加（jitter 耐性・delta gap・累積 drift・逆行を含む）、
  `missing_frame_position_faults` の意味論更新、stale コメント更新。
- `crates/synvelis-recording/tests/device_recording.rs`: stamp fallback
  の実 writer 統合 test 追加、stale コメント更新。
- `crates/synvelis-recording/tests/input_adapter_rt_alloc.rs`:
  stamp 経路の no-alloc 計測を追加。
- `crates/synvelis-recording/tests/real_input.rs`（新規）: headless
  交渉/結線 test + `real-device` feature 実機 lane。
- `mise.toml`: `test-recording-real-input` / `lint-recording-real-input`
  opt-in task 追加。
- `Cargo.lock`: 依存追加に伴う再生成。
- docs: spec/design/plan/task/test-spec/impl IR。

## Risk Register

- **cpal/alsa-sys 等の OS 依存**: optional dep + feature gate で既定
  lane を不変に保つ。feature lane の compile 検証は Windows host で
  実施（WASAPI は追加 SDK 不要）。Linux での feature build は
  `libasound2-dev` 等が必要であり、opt-in lane の前提として文書化。
- **stamp clock と sample clock の rate 差（drift）**: 累積乖離
  検査（`STAMP_ABS_TOLERANCE_FRAMES` = 16,384 frame）で監視。
  閾値超過は `FAULT_INPUT_DISCONTINUITY` で fail-stop（fail-closed
  方向）。閾値内 drift は累積 count 配置により録音連続性へ影響
  しない。実機 lane の長時間観測は本 slice 範囲外。
- **timestamp jitter による誤検出**: 実機実測（±3 frame）に対し
  delta 許容 256 frame・累積許容 16,384 frame で 2 桁以上の余裕。
  fake の決定的 jitter 注入（±60 µs）で headless 検証済み。
- **`now()` 非対応 backend**: anchor 無し → stamp 経路は全て
  `FAULT_INPUT_DISCONTINUITY`。frame_position backend には無影響。
  fail-closed であり silent な誤配置は起きない。
- **`BufferSize::Default` の可変 block size**: `max_block_frames` を
  spec で十分大きく取り、超過は `FAULT_CONTRACT` で検出。
- **Cargo.lock 再生成**: `ruviform-audio-device` + cpal 系の新規
  lock entry が増える。`--locked` task は lock コミット後に整合。

## Rollback

- Slice A の fallback は `frame_position` 優先順位の末尾追加であり、
  `frame_position` を報告する backend/fake の経路は不変。revert は
  `input.rs` の該当 hunk + test のみで完結。
- Slice B は新規 module/test/mise task/optional dep の追加であり、
  既定 lane の挙動を変えない。revert は追加物の削除で完結。

## Verification Matrix

| 検証 | 経路 | 期待 |
| --- | --- | --- |
| fmt/check/lint/test/test-rt | `mise run` 既定 lane | 全 pass（hardware 非依存） |
| test-msrv | `mise run test-msrv` | MSRV 1.89 で pass |
| feature compile/lint | `cargo clippy -p synvelis-recording --all-targets --features real-device -- -D warnings` | pass（補助証拠） |
| 実機 lane | `mise run test-recording-real-input` | Windows/WASAPI（Audient iD4）で実行・pass。jitter 下の連続録音を確認。他 OS/デバイスは未検証 |
