---
id: "DESIGN-2mTMmzcADhXvcwIO5uSsUn"
type: "design"
status: "approved"
title: "Issue #115 rf#628 consumer oracle Design"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
  implemented-by:
    - "docs/plans/issue-115-rf628-consumer-oracle.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-01-virtual-l1-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-115-rf628-consumer-oracle.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 consumer oracle Design

## 判断

`ruviform-audio-test` の `ScriptedDeviceHandle` は `Device` trait を
実装し `Clone` で state を共有する。`RealDeviceRig::open` は
`&[(&dyn Device, &InputStreamSpec)]` を受けるため、handle を
`&dyn Device` としてそのまま渡せる — `tests/common/mod.rs` への
glue 追加は不要（`open_rig_core` が既に `&dyn Device` 経路）。

PCM 供給の駆動は fake stream の `deliver(n)` ではなく
`ScriptedDeviceHandle::pump_input_next()` — pump 呼出し自体が
backend の data/error callback 発火を兼ねる。`PumpReport` が
`position`/`reported_frame_position`/`gap_frames`/`errors_fired`/
`clock_domain` を返すため、期待値を観測値から独立に構成できる。

## 主要決定

- **D1 pin/dep**: ruviform を `abb9f372`（`pipeline/` 収録済み最新
  main）へ `cargo update -p ruviform-*` で再解決。manifest に `rev`
  を書かない lockfile-primary を維持。`ruviform-audio-test` は
  `[workspace.dependencies]` + `synvelis-recording` `[dev-dependencies]`
  のみ — production 依存方向を汚さない。
- **D2 期待値 oracle**: `ruviform_audio_test::oracle::pcm_reference_block`
  （`PcmFixture::fill` を呼ばない独立再実装）で segment ごとの期待
  run を生成し、`pcm_assert_eq` で照合。`Sine`/`Sweep` は
  `AbsEpsilon(1e-5)`（ruviform `SINE_EPS` と同値）、他は `Exact`。
  先行 slice の emission-log 方式とは別構造 — fixture 自体が
  「device が書いた値」の一次観測源であり、oracle module が
  独立検算を担う。
- **D3 delivery 正確性**: `session_frames` を `BLOCK=500` 刻みで
  供給し、末尾端数は `StreamFault::Frames` で当該 callback の
  frame 数を残りに一致させる。scenario 被覆 `[0, SCENARIO_END)` は
  session を十分に超える固定長（PCM は事前生成しないため余剰に
  コストなし）。
- **D4 gap/fault の期待構成**: `PcmScenario` の segment 間 gap は
  backend が silence + `Xrun` で配送 → error callback が
  `FAULT_INPUT_DISCONTINUITY` に写像 → fail-stop。prefix 期待値は
  `PumpReport.gap_frames`/`callback_index` から構成（gap を
  callback 境界に揃えて prefix = `gap_start` に確定）。
  `StreamFault::FramePosition` は engine 側 position 不連続検出、
  `StreamFault::Error(ClockDiscontinuity)` は `FAULT_EPOCH_
  DISCONTINUITY`、`vanish` は `StreamInvalidated`、`OpenFaults` は
  `InputBindError::Device` の defined rejection をそれぞれ検証。
- **D5 RT 計測分離**: SV-6 は `stats_alloc` 規約どおり別 test
  binary・単一テスト・`rt_` prefix 名。warm-up で open/scratch/queue
  初期化を窓外へ出し、`pump_input_next` 1 回分（backend pump +
  scenario fill + callback）を `Region` で計測。
- **D6 honesty**: scripted-backend Pass は deterministic-backend
  headless lane のみ。rf#629/physical/GUI/real-input lane は
  NotRun/NotRequired のまま。production `src/`・`common/mod.rs`・
  既存 test file は非変更。

## リスク

| Risk | 対応 |
| --- | --- |
| pin bump が他 crate を壊す | `check`/`lint`/`test-rt` で検出。範囲外の破壊は修正せず報告（sv28 先例） |
| scenario 被覆不足で意図しない Xrun | `SCENARIO_END=16384` 固定 + delivery は `Frames` fault で厳密一致。session_frames は実行時に plan から取得 |
| gap callback が segment 境界を跨ぎ prefix が曖昧に | gap_start を `4*BLOCK` に揃え、callback 完全内包を保証 |
| `pcm_reference_block` と `PcmFixture` の frame 座標ずれ | backend は絶対 stream frame を `fill_channel` に渡す（`stream.rs` 実測）。oracle も絶対 frame で照合 |
