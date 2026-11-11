---
id: "PLAN-gpF9kWuFqfzFATeoPdxIGF"
type: "plan"
status: "approved"
title: "RCD-01 実Audio Device 到達準備 実装計画 — stamp placement fallback + real backend 配線"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  implements:
    - "docs/specs/rcd-01-real-backend-readiness.md"
  implemented-by: []
  depends-on:
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
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-01-real-backend-readiness.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 実Audio Device 到達準備 Plan

## Scope Split

2 slice。Slice A（stamp placement fallback）は `input.rs` とその
test のみで完結し、既定 lane で全検証可能。Slice B（real backend
配線 + opt-in lane）は依存追加・新規 module・新規 test・mise task
から成り、実機実行を除き headless で検証可能。

## File Map

### Slice A

- `crates/synvelis-recording/Cargo.toml`: `ruviform-media-core` を
  dev-deps → deps へ昇格。
- `crates/synvelis-recording/src/input.rs`:
  - `StampPlacement`（private、`Copy`）: `ClockAnchor` +
    `frame_base: TimeBase` + `now: EngineFrame`。
  - `calibrate_stamp_placement(stream, frame_base) -> Option<StampPlacement>`:
    `stream.now()` → `anchor.stamp_ticks`、domain/base/frame/epoch
    を構成。失敗時 `None`。
  - `StampState`（private、callback 内 mutable）: `seeded`/
    `next_first_frame`/`prev_stamp_ticks`/`prev_frames` —
    jitter 許容の累積配置状態機械。
  - `resolve_first_frame(info, placement, frames, state)
    -> Result<u64, u32>`: `frame_position` → stamp seed/累積配置/
    連続性・ドリフト検証 → fault の優先順位を実装。
  - `STAMP_DELTA_TOLERANCE_FRAMES = 256` /
    `STAMP_ABS_TOLERANCE_FRAMES = 16_384`（閾値根拠は design 文書）。
  - `stamp_rejection_fault(MapRejection) -> u32`: 5 種 + wildcard の
    FAULT_* 写像。
  - `InputAdapter::open`: 検証後に較正し closure へ move。
  - `#[cfg(test)]` unit test: 全 `MapRejection`・経路選択・
    両方欠如・seed→累積配置・jitter 吸収・delta gap・累積 drift・
    逆行を `resolve_first_frame` へ直接投入。
- `crates/synvelis-recording/tests/common/mod.rs`:
  `FakeStreamShared` に capture stamp 発射（`report_capture`・
  `stamp_epoch_ticks`・`stamp_base`・`now_ticks` knob）と
  決定的 jitter knob（`stamp_jitter_ns`）を追加。
- `crates/synvelis-recording/tests/input_adapter.rs`:
  stamp fallback 系 test 追加（jitter 耐性を含む）、
  `missing_frame_position_faults` を「position も stamp も無い
  場合に fault」へ整合更新。
- `crates/synvelis-recording/tests/device_recording.rs`:
  stamp fallback の実 `StreamWriter` 統合 test 追加。
- `crates/synvelis-recording/tests/input_adapter_rt_alloc.rs`:
  stamp 経路の no-alloc 計測追加（同一 test 内で逐次計測）。

### Slice B

- `Cargo.toml`: `[workspace.dependencies]` に
  `ruviform-audio-device = { git = "https://github.com/scarecrow173/ruviform.git" }`。
- `crates/synvelis-recording/Cargo.toml`: `ruviform-audio-device`
  optional dep + `[features] real-device = ["dep:ruviform-audio-device"]`。
- `crates/synvelis-recording/src/real_input.rs`（新規）:
  `negotiate_input_format`・`select_f32_input_format`・
  `real_input_config`（常時）+ `pub use ruviform_audio_device::default_host`
  （`#[cfg(feature = "real-device")]`）。
- `crates/synvelis-recording/src/lib.rs`: `pub mod real_input;` と
  必要な re-export。
- `crates/synvelis-recording/tests/real_input.rs`（新規）:
  headless 交渉/config/session 結線 test + `#[cfg(feature)]`・
  `#[ignore]` 実機 lane module。
- `mise.toml`: `test-recording-real-input`・
  `lint-recording-real-input` opt-in task。
- `Cargo.lock`: 再生成。
- stale コメント更新（common/input_adapter/device_recording の
  「ruviform#251 待ち」記述）。

## Dependency Graph

```
Slice A: input.rs stamp fallback ──→ common fake knobs ──→ tests
Slice B: Cargo.toml deps ──→ real_input.rs ──→ tests/real_input.rs
                                └─→ mise.toml lane ──→ Cargo.lock
```

Slice A と Slice B は独立だが、B の headless test は A の fallback
経路（実 cpal stream が `frame_position: None` + capture stamp を
報告する想定）を前提とする。実装順は A → B。

## Risk Register

- `MapRejection` が `#[non_exhaustive]` のため wildcard arm が必要
  → 未知 variant は `FAULT_CONTRACT`（fail-closed）。
- 実機実測で capture stamp の sub-ms jitter（±3 frame 相当）が判明
  → block 毎の独立 `map_stamp_to_frame` 配置は非連続となり
  `FAULT_INPUT_DISCONTINUITY`。累積 frame count 配置 + stamp 検証
  （seed 1 回 + 連続性/ドリフト監視）へ設計改訂（coordinator 判断）。
  fake の決定的 jitter knob で headless 検証、実機 lane で再検証。
- `ruviform-audio-device` optional dep のため feature 無しでは
  `real_input.rs` の host 部分が compile されない →
  `lint-recording-real-input` 相当の `--features real-device`
  clippy を検証に含める。

## Verification Matrix

| 検証 | コマンド | canonical |
| --- | --- | --- |
| format | `mise run fmt-check` | yes |
| typecheck | `mise run check` | yes |
| lint | `mise run lint` | yes |
| test | `mise run test` | yes |
| rt contract | `mise run test-rt` | yes |
| MSRV | `mise run test-msrv` | yes |
| feature lint | `cargo clippy -p synvelis-recording --all-targets --features real-device -- -D warnings` | 補助証拠 |
| feature test compile | `cargo test -p synvelis-recording --features real-device --no-run` | 補助証拠 |
| 実機 lane | `mise run test-recording-real-input` | Windows/WASAPI（Audient iD4）で実行・pass 済み。他 OS は未検証 |

## Task Breakdown

1. Doc chain（spec/design/plan/task/test-spec）作成・review。
2. Slice A: fake knobs + 失敗 test → `input.rs` 実装 → 全 test 更新。
   実機 lane 初回実行で jitter 起因の `FAULT_INPUT_DISCONTINUITY`
   を検出 → 累積配置 + stamp 検証へ設計改訂 → fake jitter knob +
   test 追加 → 実機 lane 再実行で pass 確認。
3. Slice B: deps → `real_input.rs` → `tests/real_input.rs` → mise task。
4. stale コメント更新・`Cargo.lock` 再生成。
5. 全検証 → `git fetch origin main && git merge origin/main` → 再検証。
6. review-protocol → commit/push/PR + ruviform 上流 issue 起票。
