---
id: "SPEC-034UoScGX6dIi25hMurqCI"
type: "spec"
status: "proposed"
title: "RCD-01 Recording Input Adapter — channel binding・Arm/Monitor・fake port fail-stop（synvelis#115 slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "ruviform crates/ruviform-audio-core/src/device.rs @ 0f5b573"
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vs-b-deterministic-mock-capture-engine.md"
    - "docs/specs/vs-d-multi-track-recording-session.md"
    - "docs/specs/vs-c-staging-writer-journal-recovery.md"
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

# RCD-01 Recording Input Adapter — channel binding・Arm/Monitor・fake port fail-stop（synvelis#115 slice）

## What Should Be Built

`crates/synvelis-recording` に、Ruviform `ruviform_audio_core::device` の入力 port
（`Device`/`InputStream`/`InputDataCallback`/`InputCallbackInfo`/`same_clock_domain`）
に接続する入力 adapter 層を追加する。adapter は入力 callback の planar
`AudioBufferView<f32>` を既存の interleaved `CaptureChunk`/`CaptureEngine`/SPSC
handoff 経路へ、アロケーション・ブロッキング・I/O を伴わない RT 制約で転送し、
channel→track binding・Arm・Monitor・clock domain 判定を明示状態として保持する。

本スライスは headless・fake port で検証可能な契約層のみを対象とする。
synvelis-app の製品録音 Command/状態投影、実 Audio Device open/capture、
OS 別ドライバ実機検証は対象外。

## Why Now

Issue #115（RCD-01）で実デバイス録音統合が要求されているが、VS-A〜VS-D の
recording 基盤（capture engine・bounded handoff・raw writer・journal・recovery）は
既に存在し、入力側の adapter 層が未実装である。Ruviform 側の入力 port 契約
（pinned rev `0f5b573`）は確定済みのため、まず fake port で契約層を固定し、
実 backend（ruviform#251）到着後に同一契約で接続できる状態を作る。

## Users and Value

- Synvelis recording crate の利用側（将来の synvelis-app host 層）:
  channel→track binding・Arm・Monitor・clock domain 検証を再利用可能な
  契約として得る。
- 検証者: fake port による決定的な PCM fidelity・fail-stop 検証を得る。
  Mock Pass を実 Device Pass に昇格させない区分を維持できる。

## Scope

### In Scope

- `InputBinding`（channel→track binding: mono/stereo/multichannel/2-track）、
  Arm、Monitor、channel map の明示状態と検証。
- `InputCallbackInfo`（`frame_position`/`clock_domain`/`timestamp`）から
  `InputBlock` への変換と、`same_clock_domain` による同一 clock domain 判定。
- planar `AudioBufferView<f32>` → interleaved `CaptureChunk` への有界コピー
  （preallocated storage、RT noalloc/no-lock/no-I/O）。
- fake `Device`/`InputStream`（scripted port 実装）による headless 駆動。
- fail-stop: epoch 不連続/rate 変更/切断/drop/queue-full/device error で
  既存 `FAULT_*` への安全停止と Journal Verified Prefix 保持。
- Arm Off / Monitor On で Writer を作成しないこと。

### Out of Scope

- synvelis-app 製品録音 Command/状態投影（別 dispatch が app crate を担当）。
- 実 Audio Device open/capture・OS driver matrix（ruviform#251 が前提、
  `open_input_stream` は `DeviceError::Unsupported`）。
- 異なる clock domain の multi-device 録音（事前対応しない）。
- 受入証拠の #83/#114 リンク（実機 gate）。
- Ruviform 側の generic audio-device/driver/RT 実装。

## Requirements

- adapter は `ruviform-audio-core` の workspace 依存のみを `synvelis-recording`
  に追加し、Ruviform 側のコードを変更・再実装しない。
- callback 経路でアロケーション・ロック・I/O・panic を行わない。
- raw 録音は software monitor routing に依存しない（独立状態）。
- binding/channel-map/arm/monitor/clock domain の不正・不一致・未知値は
  fail closed とする。
- 補間・リサンプル・連続性の捏造を行わない。
- 既存の `CaptureEngine`・`FaultLatch`・writer/journal/recovery を再利用し、
  別経路を新設しない。

## Acceptance Criteria

- [ ] fake port で deterministic f32 PCM が SourceNativeFrames と Input PCM を
      照合できる（Mono/Stereo/MultiChannel/2Track binding、PCM fidelity）。
- [ ] Raw capture と Software Monitor が独立であり、Arm Off + Monitor On で
      Writer が作成されず raw input も capture されない。
- [ ] input drop・clock epoch/domain 変更・rate 変更・device 切断・
      RT queue full で既存 `FAULT_*` により安全停止し、Journal Verified
      Prefix が保持され Unknown/Partial が可視化される。
- [ ] `same_clock_domain` が一致する domain のみ multi-track/multi-stream
      参加を許可し、不一致・UNKNOWN を拒否する。
- [ ] callback は preallocated storage のみを使い、RT noalloc/no-lock/no-I/O
      を維持する。
- [ ] fake/mock の検証結果を実 Device Pass と偽装しない（実機は Unsupported
      として記録）。

## Deferred Design Concerns

- 実 Audio Device open/capture・OS driver matrix | ruviform#251 の実 backend
  が未実装で `open_input_stream` は `Unsupported` | ruviform#251 完了・実機
  gate（#83/#114） | fake port 検証を実機合格と誤認するリスク
- synvelis-app 製品録音 Command/状態投影 | 別 dispatch が `synvelis-app` を
  編集中でありスコープ外 | app 側 dispatch 完了後 | adapter 契約と app 側
  状態投影の齟齬

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/115>（RCD-01）
- Ruviform pin: `Cargo.lock` resolved `0f5b57372e9a43f84415f7964b2f9636774174f0`
  の `crates/ruviform-audio-core/src/device.rs`（`Device`/`InputStream`/
  `InputDataCallback`/`InputCallbackInfo`/`same_clock_domain`/`ClockDomainId`）
- 関連 spec: VS-B（deterministic mock capture engine）、VS-C（staging writer
  /journal/recovery）、VS-D（multi-track recording session）
