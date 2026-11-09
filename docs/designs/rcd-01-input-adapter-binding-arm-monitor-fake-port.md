---
id: "DESIGN-Bz2IMMfywvmEjmdIeThNvq"
type: "design"
status: "approved"
title: "RCD-01 Input Adapter — binding/Arm/Monitor/channel-map と fake port fail-stop"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "ruviform crates/ruviform-audio-core/src/device.rs @ 0f5b573"
  implements:
    - "docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md"
  implemented-by: []
  depends-on:
    - "docs/specs/vs-b-deterministic-mock-capture-engine.md"
    - "docs/specs/vs-d-multi-track-recording-session.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/vs-b-capture-engine-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 Input Adapter Design

## Overview

`synvelis-recording` に `input` モジュールを追加し、Ruviform `device` port の
`InputStream`/`InputDataCallback`/`InputCallbackInfo` を既存の
`CaptureEngine`・`FaultLatch`・SPSC handoff・writer 経路へ接続する。
本スライスは headless fake port での契約検証のみを対象とする。

## 責務分界

- **Ruviform（変更しない）**: `DeviceEnumerator`/`Device`/`InputStream`/
  `InputStreamConfig`/`InputCallbackInfo`/`DeviceError`/`same_clock_domain`/
  `ClockDomainId`/`AudioBufferView`。generic device/driver/RT は Ruviform の
  責務であり再実装しない。
- **Synvelis recording（本スライス）**: channel→track `InputBinding`、
  Arm/Monitor 独立状態、channel map、callback→`InputBlock` 変換、
  planar→interleaved 有界コピー、clock domain 検証、fault 写像。
- **既存 engine（変更しない方針）**: `CaptureEngine::process` は
  `InputBlock` + interleaved `&[f32]` を受ける。adapter は engine の前段で
  callback を `InputBlock` + interleaved PCM に正規化する。

## 状態モデル

`InputBinding`（per input stream / track）:

- `channel_map`: 入力 channel → track 内 logical channel の写像
  （mono/stereo/multichannel/2-track の binding mode を表現）。
- `arm`: Record Arm。Off のとき raw capture を行わない。
- `monitor`: Software Monitor。Arm と独立。On でも Writer/capture を
  作らない・行わない（raw 録音に依存しないことを明示）。
- `clock_domain`: stream の `ClockDomainId`。multi-track 参加は
  `same_clock_domain` が真の場合のみ許可。

Arm Off + Monitor On: Writer を作成せず、callback は monitor 状態のみを
更新し raw PCM を capture chunk に書き込まない。

## Callback 経路（RT 制約）

`InputDataCallback` は `FnMut(AudioBufferView<'a, f32>, InputCallbackInfo)`。
callback 内で行うこと:

1. `InputCallbackInfo.frame_position`/`clock_domain`/`timestamp` を検証し
   `InputBlock`（`clock_epoch`・`first_input_frame`・`frames`）へ写像。
   `frame_position: None` や domain 不一致は fail-stop。
2. channel_map に従い planar `view.channel(ch)` から interleaved
   preallocated scratch へ有界コピー（`frames * bound_channels` 要素、
   アロケーションなし）。
3. 既存 SPSC/`CaptureEngine` 経路へ引き渡す。queue-full は既存
   `FAULT_QUEUE_FULL` 等へ写像。

callback 内で禁止: allocation/deallocation・blocking・I/O・panic・
無制限処理。preallocated scratch は binding 確定時に確保する。

## Fail-stop 写像

| 事象 | fault |
| --- | --- |
| `frame_position` 不連続/epoch 変更 | `FAULT_EPOCH_DISCONTINUITY` / `FAULT_INPUT_DISCONTINUITY` |
| clock domain 不一致/UNKNOWN | binding 検証で拒否 + fault |
| sample rate / format 変更 | binding/設定検証で拒否 + fault |
| device 切断/`DeviceError` | `FAULT_INPUT_DISCONTINUITY`（error callback 経由） |
| RT queue full | `FAULT_QUEUE_FULL` |
| callback panic 回避 | panic せず fault latch へ |

既存 `FaultLatch`・Journal Verified Prefix・partial/unknown 可視化を
そのまま利用する。新規 fault bit は本スライスでは追加しない
（既存 `FAULT_*` で表現可能な範囲に限定）。

## Fake port

`fake_input`（test/検証用、`#[cfg]` ではなく公開 test-support または
`tests/` 内実装）で `DeviceEnumerator`/`Device`/`InputStream` を scripted
実装し、決定的 PCM・fault injection（drop/epoch/rate/domain/error/
queue-full）を headless 駆動する。fake は「実 Device Pass」ではなく
「fake port 契約 Pass」として記録する。

## 検証

- `tests/input_adapter.rs`（仮）: PCM fidelity・binding modes・monitor
  独立・same-clock 受理/拒否・fault injection・Arm Off/Monitor On。
- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt`。
