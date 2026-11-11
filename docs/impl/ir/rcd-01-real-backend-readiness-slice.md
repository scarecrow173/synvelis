---
id: "IMPL-RCD01rbkSl1cE9mQ2xW"
type: "impl"
status: "completed"
title: "RCD-01 実Audio Device 到達準備 — stamp placement fallback（jitter 許容の累積配置）+ real backend 配線"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/src/real_input.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/real_input.rs"
      - type: "file"
        path: "docs/specs/rcd-01-real-backend-readiness.md"
      - type: "file"
        path: "docs/designs/rcd-01-real-backend-readiness-design.md"
      - type: "file"
        path: "docs/plans/rcd-01-real-backend-readiness-plan.md"
      - type: "file"
        path: "docs/test-specs/rcd-01-real-backend-readiness.md"
      - type: "file"
        path: "docs/tasks/rcd-01-task1-real-backend-readiness.md"
      - type: "file"
        path: "docs/impl/ir/rcd-01-real-backend-readiness-slice.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-recording/src/input.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/input_adapter.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/device_recording.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/input_adapter_rt_alloc.rs"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "Cargo.lock"
  implements:
    - "docs/plans/rcd-01-real-backend-readiness-plan.md"
    - "docs/designs/rcd-01-real-backend-readiness-design.md"
    - "docs/specs/rcd-01-real-backend-readiness.md"
    - "docs/tasks/rcd-01-task1-real-backend-readiness.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-01-audio-device-headless-slice-fake-port-streamwriter-fixture-recording-stat.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/251"
  defers:
    - "実機 gate 行（実録音受入・Raw capture fidelity 実測・multi-device 実機相関・xrun/removal/長時間・証拠リンク）— Issue #115 後続 slice"
    - "cpal WASAPI adapter の IAudioClock::GetPosition device frame position 破棄（ruviform#565 起票済み）"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "stamp 検証付き累積 frame count 配置（timestamp jitter 許容）"
    rejected:
      - "block 毎の独立 map_stamp_to_frame 配置（実機 jitter で非連続 → FAULT_INPUT_DISCONTINUITY）"
---

# RCD-01 実Audio Device 到達準備 — stamp placement fallback + real backend 配線

## Summary

`InputAdapter` の `frame_position: None` fail-stop を解消する
stamp placement fallback を実装し、`ruviform-audio-device`
（cpal backend）への composition root と opt-in 実機 lane を
追加した。初版の「block 毎に `capture_stamp()` →
`map_stamp_to_frame` で独立配置」は実機 lane（Windows/WASAPI・
Audient iD4）で capture stamp の sub-ms timestamp jitter
（10.019/10.006 ms、独立写像 443/444/439 vs 期待 441 frame）に
より `FAULT_INPUT_DISCONTINUITY` で fail-stop したため、
coordinator 判断で「累積 frame count 配置 + stamp 検証（初回
seed・連続性/ドリフト監視）」へ設計改訂し、fake jitter 注入と
実機 lane の両方で sample-exact 連続録音を確認した。

## Context

Issue #115 / RCD-01。実 cpal input stream は `frame_position` を
常に `None` で報告するため、従来 adapter は実 device の初回
callback で必ず fault になった。port 契約の sanctioned 経路
`capture_stamp()` → `ClockAnchor` + `map_stamp_to_frame`
（`ruviform-media-core`）で代替配置を実装したが、実機実測で
stamp の jitter が判明し設計を改訂した。依存は
`ruviform-media-core` の src 昇格と `ruviform-audio-device` の
optional dep + `real-device` feature（cpal/alsa-sys の OS 依存を
既定 headless lane から隔離）。

## Implementation

### Changes

- `src/input.rs`: `StampPlacement`（open 時に `stream.now()` から
  較正する `ClockAnchor` + `frame_base` + `now`）、callback 内
  `StampState`（`seeded`/`next_first_frame`/`prev_stamp_ticks`/
  `prev_frames`）、`resolve_first_frame`（`frame_position` 優先 →
  stamp seed/累積配置/連続性・ドリフト検証 → `FAULT_*`）、
  `stamp_rejection_fault`（5 種 `MapRejection` + wildcard →
  `FAULT_EPOCH_DISCONTINUITY`/`FAULT_INPUT_DISCONTINUITY`/
  `FAULT_ARITHMETIC`/`FAULT_CONTRACT`）、閾値定数
  `STAMP_DELTA_TOLERANCE_FRAMES = 256`・
  `STAMP_ABS_TOLERANCE_FRAMES = 16_384`（根拠は design 文書）。
- `src/real_input.rs`（新規）: `negotiate_input_format`/
  `select_f32_input_format`/`real_input_config`
  （`BufferSize::Default` 固定）+ feature-gated
  `pub use ruviform_audio_device::default_host`。
- `tests/common/mod.rs`: fake stream に capture stamp 発射
  （`report_capture`/`stamp_epoch_ticks`/`stamp_base`/`now_value`/
  `stamp_fixed_ticks`）と決定的 jitter knob（`stamp_jitter_ns`、
  `[0,+1,+2,+3,-3,-2,-1]` cycle）を追加。
- `tests/input_adapter.rs`: stamp fallback 系 test（fallback・
  jitter 耐性・position 優先・foreign base・逆行・before-anchor）
  と `missing_position_and_stamp_faults` の意味論更新。
- `tests/device_recording.rs`: stamp fallback の実 `StreamWriter`
  統合 test。
- `tests/input_adapter_rt_alloc.rs`: stamp 経路の no-alloc/no-dealloc
  計測。
- `tests/real_input.rs`（新規）: headless 交渉/config/session 結線
  test + `#[cfg(feature = "real-device")]`・`#[ignore]` 実機 lane
  （timestamp fields probe + 録音経路 test）。
- `mise.toml`: `test-recording-real-input`/
  `lint-recording-real-input` opt-in task（CI 対象外）。
- `Cargo.toml`/`Cargo.lock`: `ruviform-audio-device` workspace dep
  追加、全 ruviform crate が branch HEAD `9865cd57` へ再解決
  （`b02942c2` → `9865cd57`、workspace check で整合確認）。

## Verification

- `cargo test -p synvelis-recording --locked`: 全 test pass
  （unit 23・device_recording 17・input_adapter 26・rt_alloc 1・
  他既存 suite）。
- `cargo clippy -p synvelis-recording --all-targets --locked
  --features real-device -- -D warnings`: pass（補助証拠）。
- 実機 lane `cargo test -p synvelis-recording --locked
  --features real-device --test real_input -- --ignored
  --test-threads=1`: Windows/WASAPI（Audient iD4）で 2 test pass。
  初回実行（独立写像版）は `FAULT_INPUT_DISCONTINUITY` で
  正直に fail → 設計改訂後に pass。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`:
  PR 前に実行（結果は PR 本文に記録）。

## Notes / Deferred

- cpal WASAPI adapter が `IAudioClock::GetPosition` の device
  frame position を取得しながら `InputCallbackInfo` へ載せず
  破棄している上流欠落を確認。`frame_position` が供給されれば
  本線経路となり stamp fallback は不要になる — ruviform#565 を
  起票済み（synvelis#115 関連）。
- 実機 gate 行（fidelity 実測・multi-device 相関・xrun/長時間・
  証拠リンク）は本 slice 範囲外で NotRun。
- `frame_position` 優先順位は維持 — 上流修正後は自動的に
  position 経路が本線になる。
