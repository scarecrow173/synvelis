---
id: "EXP-AbqwSNuqTYbDIGqhdQ9swm"
type: "experiment"
status: "completed"
title: "Issue #115 rf#628 consumer oracle — fixture 駆動経路と fault 分類の実測"
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
  implements: []
  implemented-by:
    - "docs/impl/ir/issue-115-rf628-consumer-oracle.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-115-rf628-consumer-oracle.md"
    - "docs/test-specs/issue-115-rf628-consumer-oracle.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/issue-115-rf628-consumer-oracle.md"
  defers: []
  deferred-by: []
---

# Issue #115 rf#628 consumer oracle — fixture 駆動経路と fault 分類の実測

## 仮説

`ruviform-audio-test` の scripted backend を `&dyn Device` 経由で
`RealDeviceRig`/`InputSession` に接続すれば、fake `deliver` を介さず
`pump_input_next`（backend pump + scenario fill + callback 発火の
一体化）で deterministic PCM を production recording 経路へ供給
でき、`pcm_reference_block` 独立 oracle で E2E fidelity・fault
verified prefix・RT no-alloc を実測できる — ただし (i) `PumpReport`
の `gap_frames`/`errors_fired` が Xrun fault latch と整合するか、
(ii) `FramePosition` jump が engine 側 discontinuity 検出に正しく
届くか、(iii) `pump_input_next` 定常状態が真に alloc-free かは
実測が要る。

## 方法

- `ScriptedBackend::new(Arc<VirtualClock>)` + `DeviceSpec`/`InputSpec`
  （`PcmScenario`・`StreamScript` faults）で device を構築し、
  `RealDeviceRig::open` の `&dyn Device` 経路で rig 化。
- `drive_session`: `BLOCK=500` 刻みで `pump_input_next` →
  `VirtualClockDriver::advance_frames` → `rig.pump_all`。末尾端数は
  `StreamFault::Frames` で当該 callback の frame 数を残りに一致。
- 検証: journal `ChunkRecord`/`spans` で media byte を index →
  `pcm_reference_block`（絶対 stream frame・segment 単位）→
  `pcm_assert_eq`。`verified_sha256` は `sha2` で独立再計算。
- fault 系: scenario gap / `FramePosition` / `Error` /
  `OpenFaults` / `vanish` を個別 test で注入し、fault bit・
  verified prefix・commit recoverable 分類を commit→reload 越しに
  検証。
- SV-6: warm-up 8 回後に `Region` で `pump_input_next` 1 回分を計測。

## Result

`cargo test -p synvelis-recording --test rf628_oracle
--test rf628_oracle_rt_alloc` — 10 pass / 0 fail。

- **fixture 駆動経路は成立**: `PumpReport` の position/
  reported_frame_position が連続し、`clock_domain` は
  `ClockDomainId::new(7)`（backend clock epoch）で全 callback 一致。
  scenario gap callback は `gap_frames=500`/`errors_fired>=1`（Xrun）
  を正しく報告し、consumer 側 `FAULT_INPUT_DISCONTINUITY` latch →
  verified prefix = gap_start（2000 frame）に確定した。
- **`FramePosition` jump は engine 側検出に届く**: jump 先
  position=3300 が report に反映され、`first_input_frame` 不連続で
  fail-stop。prefix = 3000 frame。
- **fault 分類は正直**: `ClockDiscontinuity` →
  `FAULT_EPOCH_DISCONTINUITY`（fault_bits 実測で確認）、
  `StreamInvalidated`（vanish）→ `FAULT_INPUT_DISCONTINUITY`、
  `OpenFaults` persistent → `InputBindError::Device` defined
  rejection + clear 後 recovery。
- **RT no-alloc は成立**: warm-up 後の `pump_input_next` 1 回分で
  allocations=0・deallocations=0（`stats_alloc` `Region` 実測）。
  backend の `scratch`/`fault_errors` Vec は定常状態で割当を
  発生させない。
- **clippy 適合のため `let...else` 化**: `expect_err` が
  `TapWriterEnds`/`InputSession` の `Debug` 非実装で使用不可の
  ため、`session.add`/`InputSession::open` の rejection 検証は
  `let Err(err) = ... else { panic!() }` に変更（動作不変）。

## Follow-ups

- `evidence/sv115/` case matrix の `rf#628-fixture` 列を本実測で
  更新。rf#629/physical/GUI/real-input lane は NotRun のまま。
- ruviform 側 gap（`RunMetadata` JSON serializer 欠如・
  `InputPump` drift リスク）は本 slice では未踏 — 必要になれば
  ruviform Issue 起票（恒久 workaround は作らない）。
- Issue #115 は open 継続。
