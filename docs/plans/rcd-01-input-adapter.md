---
id: "PLAN-0YNXPJ9xIQGrr00MWQc9aM"
type: "plan"
status: "approved"
title: "RCD-01 Input Adapter 実装計画（fake port 検証）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  implements:
    - "docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md"
  implemented-by: []
  depends-on:
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

# RCD-01 Input Adapter Plan

## Scope Split

単一スライス: `crates/synvelis-recording` に input adapter + fake port
検証を追加。`crates/synvelis-app/`・他 crate・Ruviform は変更しない。

## File Map

- `crates/synvelis-recording/Cargo.toml`: `ruviform-audio-core.workspace = true` 追加。
- `crates/synvelis-recording/src/input.rs`（新規）: `InputBinding`/channel map/
  Arm/Monitor 状態、callback→`InputBlock` 変換、planar→interleaved 有界コピー、
  clock domain 検証、fault 写像。
- `crates/synvelis-recording/src/lib.rs`: `input` モジュール export。
- `crates/synvelis-recording/tests/input_adapter.rs`（新規）: fake port
  harness + 受入テスト。
- `crates/synvelis-recording/src/fake_input.rs`（新規、test-support）:
  scripted `DeviceEnumerator`/`Device`/`InputStream`。公開 test-support
  として `lib.rs` から export するか `tests/` 内に置くかは実装時に決定
  （crate 外から fake を再利用できる利点があるため公開を優先）。

## Dependency Graph

`input` → `capture`(InputBlock/CaptureChunk)・`engine`(CaptureEngine)・
`handoff`(FaultLatch)・`clock`(ClockEpochId)・`ruviform_audio_core::device`・
`ruviform_audio_core::buffer`(AudioBufferView)・`synvelis_project`(time/id)。

## Risk Register

| risk | 対応 |
| --- | --- |
| planar→interleaved 変換の添字ミス | PCM fidelity テストで bit 一致を検証 |
| callback 内 allocation | preallocated scratch、cargo clippy + レビュー |
| clock domain 判定漏れ | `same_clock_domain` を唯一の判定に使用 |
| Arm Off/Monitor On で capture 混入 | binding 状態機械で raw capture を gate |
| fake を実機 PASS と誤認 | 文書・PR で fake port 契約 Pass と明記 |

## Verification Matrix

| 検証 | 手段 |
| --- | --- |
| fmt | `mise run fmt-check` |
| 型/lint | `mise run check` / `mise run lint` |
| 単体/統合 | `mise run test`（`tests/input_adapter.rs`） |
| RT noalloc | `mise run test-rt` + callback 経路レビュー |
| 実機 | NotRun（ruviform#251 未実装、Unsupported として記録） |

## Task Breakdown

1. `Cargo.toml` に依存追加、`input.rs` 骨格 + 状態型。
2. callback→InputBlock/planar→interleaved + clock domain 検証。
3. fake port harness + PCM fidelity/binding/monitor テスト。
4. fail-stop fault injection テスト（drop/epoch/rate/domain/error/queue-full）。
5. docs chain 確定・impl record・検証・PR。
