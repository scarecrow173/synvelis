---
id: "TSPEC-Ua2lj36FOpxjA8UBWt5nhl"
type: "test-spec"
status: "proposed"
title: "RCD-01 fake port input adapter 検証仕様"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md"
    - "docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
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

# RCD-01 fake port input adapter 検証仕様

## Feature: fake port PCM fidelity

- Rule: scripted fake `InputStream` が与える deterministic f32 planar PCM が、
  binding/channel map を経て interleaved `CaptureChunk` に bit 一致で現れる。
- Example: Mono/Stereo/MultiChannel/2Track の各 binding で source frames と
  capture PCM を照合。

## Feature: Arm/Monitor 独立

- Rule: Arm Off + Monitor On では Writer が作成されず raw input を capture
  しない。Monitor On は raw 録音に依存しない。
- Example: monitor のみ on の状態で callback を駆動しても capture chunk /
  writer 出力が増えない。

## Feature: same-clock domain

- Rule: `same_clock_domain` が真の stream のみ multi-track/multi-stream 参加を
  許可。不一致・`ClockDomainId::UNKNOWN` は拒否。
- Example: 同一 domain の 2 stream は受理、異 domain / UNKNOWN は fault。

## Feature: fail-stop

- Rule: input drop・clock epoch/domain 変更・rate 変更・device 切断・
  RT queue full で既存 `FAULT_*` により安全停止し、Journal Verified Prefix
  が保持され Unknown/Partial が可視化される。
- Example: fault injection 後に fault latch が立ち、verified prefix 以降が
  partial/unknown として観測できる。

## Feature: RT 制約

- Rule: callback 経路は preallocated storage のみを使い、allocation・
  blocking・I/O・panic を行わない。
- Example: `mise run test-rt` + コードレビューで確認。
