---
id: "TSPEC-htPhSCIWfZxHOOGdEAyaeK"
type: "test-spec"
status: "proposed"
title: "RCD-01 実Audio Device 到達準備 検証仕様 — stamp placement fallback + real backend lane"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-01-real-backend-readiness.md"
    - "docs/designs/rcd-01-real-backend-readiness-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-01-real-backend-readiness-design.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-01-real-backend-readiness.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 実Audio Device 到達準備 検証仕様

## Feature: stamp placement fallback（Slice A・jitter 許容の累積配置）

- Rule: `frame_position: Some` は現行経路を維持し、値をそのまま
  `first_input_frame` に使う。`capture_stamp` の有無・stamp 状態に
  関わらず `frame_position` が優先され、stamp 状態機械は更新
  されない。
- Example: fake が position と意図的にずれた stamp を同時報告しても、
  記録位置は `frame_position` 由来で連続し fault しない
  （`frame_position_wins_over_capture_stamp`）。

- Rule: `frame_position: None` の初回有効 stamp は
  `map_stamp_to_frame` で写像した frame を seed として
  `first_input_frame` に使い、以後の block は累積 frame count
  （前 block の `first_input_frame + frames`）で sample-exact に
  連続配置される。block 毎の独立 stamp 写像は行わない。
- Example: fake の position 由来 stamp tick で 500 frame 連続
  deliver すると PCM が bit 一致で記録され fault しない
  （`capture_stamp_fallback_records_verified_pcm`）。

- Rule: timestamp jitter（実機 WASAPI 実測の sub-ms・±数 frame
  相当）は配置に反映されず、累積配置により block 連続性と PCM
  検算が維持される。jitter は連続性/ドリフト検証の観測値としてのみ
  使われる。
- Example: fake の `stamp_jitter_ns` で ±60 µs の決定的 jitter を
  注入しても verified PCM が記録される
  （`capture_stamp_jitter_still_records_verified_pcm`）。

- Rule: stamp の tick 逆行・per-callback 進行量の
  `STAMP_DELTA_TOLERANCE_FRAMES`（256 frame）超過乖離・
  `map_stamp_to_frame` 写像値と累積期待位置の
  `STAMP_ABS_TOLERANCE_FRAMES`（16,384 frame）超過乖離は
  `FAULT_INPUT_DISCONTINUITY` で fail-stop する。
- Example: unit test で stamp 逆行・1 秒 gap・持続 drift（200
  frame/callback × 100 回）を注入し fault bit を照合。

- Rule: `frame_position: None` かつ `capture_stamp(): None`（foreign
  base・capture 不在・anchor 無しを含む）では
  `FAULT_INPUT_DISCONTINUITY` で fail-stop し、連続を捏造しない。
- Example: `set_report_position(false)` + capture 無し、または
  foreign base の capture stamp で fault bit が立つ
  （`missing_position_and_stamp_faults`・
  `foreign_capture_base_faults`）。

- Rule: `MapRejection` は既存 `FAULT_*` へ正直に写像される:
  `UnmappedDomain`/`BaseMismatch`/`EpochMismatch` →
  `FAULT_EPOCH_DISCONTINUITY`、`BeforeAnchor` →
  `FAULT_INPUT_DISCONTINUITY`、`Overflow` → `FAULT_ARITHMETIC`、
  未知 variant → `FAULT_CONTRACT`。写像は毎 callback の
  `map_stamp_to_frame` 呼出しで検出される（seed・検証共通）。
- Example: `resolve_first_frame` への unit test で 5 種全ての
  rejection を注入し fault bit を照合（`stamp_rejections_map_to_faults`）。

- Rule: stamp→frame 変換は RT callback 内で allocation/dealloc を
  行わない（`map_stamp_to_frame` は純粋整数演算）。
- Example: `input_adapter_rt_alloc` が stamp 経路の deliver を
  `stats_alloc` Region で 0 alloc/dealloc と計測。

- Rule: same-clock domain の既存強制は不変 — callback の
  `clock_domain` が open 時と異なれば `FAULT_EPOCH_DISCONTINUITY`
  （stamp 解決より先に評価される）。
- Example: 既存 `clock_domain_change_mid_stream_faults` が
  stamp 有効時も同じ bit で fail する。

## Feature: real backend composition root + opt-in lane（Slice B）

- Rule: `negotiate_input_format`/`select_f32_input_format` は
  F32・妥当 rate range・既知 `ChannelLayout` のみを候補とし、
  stereo/48 kHz を優先、rate は clamp。候補無しは `None`。
- Example: 合成 `SupportedInputFormat` 列で優先順位・除外条件を
  headless 検証。

- Rule: `real_input_config` は `BufferSize::Default` を返す
  （Fixed は実 adapter が `UnsupportedConfig` で拒否し得る）。
- Example: 戻り値の `buffer_size` を assert。

- Rule: `InputSession` 結線は headless で検証できる — 交渉済み
  format の `real_input_config` + fake device で `InputSession::open`
  し、実 callback 駆動で PCM が記録される。
- Example: `tests/real_input.rs` の headless test が fake device で
  session open→drive→chunk 産出を確認。

- Rule: 実機 lane は `real-device` feature + `#[ignore]` で opt-in
  され、既定 `mise run test` には含まれない。device 不在・権限
  無し・format 不適合は SKIP（print + return）として扱い、
  open/start 後に callback 0 件は失敗。
- Example: `mise.toml` の `test-recording-real-input` task 定義と
  `tests/real_input.rs` の `#[cfg(feature = "real-device")]` module。
  実機実行は Windows/WASAPI（Audient iD4、44.1 kHz F32 stereo）で
  実施済み — timestamp jitter 下でも fault 無しで録音経路が成立
  することを確認。他 OS/デバイスは未検証。

## Feature: fail-stop / RT 契約の維持

- Rule: stamp fallback 導入後も、位置も stamp も無い callback・
  epoch/frame 不連続・queue full・format 変更の各 fault 経路は
  既存 bit で fail-stop し、verified prefix を保持する。
- Example: 既存 `input_frame_gap_faults_and_preserves_verified_prefix`・
  `device_error_callbacks_map_to_faults`・`work_queue_full_faults` 等が
  変更なく pass。

- Rule: device_recording の実 `StreamWriter` 経路でも stamp fallback
  が機能し、journal/media が verified prefix 性を保つ。
- Example: `device_recording.rs` の stamp fallback test が
  `assert_complete_normal` 相当の検証を通る。
