---
id: "SPEC-034W8HMV6c6BABYhVlcEe6"
type: "spec"
status: "proposed"
title: "RCD-01 実機 capture coverage 拡張 — real-device lane の staging/2Track/fail-stop/境界/長時間検証（synvelis#115）"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
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
  implemented-by: []
  depends-on:
    - "SPEC-gut3XVRAIUtMHADNZBNFHC"
    - "SPEC-034UoScGX6dIi25hMurqCI"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "TASK-GJlWE8j5emvVOKFMtbTXHR"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/251"
    - "https://github.com/scarecrow173/ruviform/issues/565"
  defers: []
  deferred-by: []
---

# RCD-01 実機 capture coverage 拡張 — real-device lane の staging/2Track/fail-stop/境界/長時間検証（synvelis#115）

## What Should Be Built

`crates/synvelis-recording/tests/real_input.rs` の `#[ignore]` opt-in
実機 lane（`mise run test-recording-real-input`、Windows/WASAPI +
Audient iD4 で実績あり）を、fake/headless 契約証拠から実機 capture
証拠へ拡張する。現行 lane は `InspectionWriter` 止まりで実
`StreamWriter`/staging 経路・2 track・fail-stop・punch 境界・
長時間連続性を実機で検証していない。本 slice はそれらを実機で
実行可能な形にし、実行不能な行は loud SKIP + `NotRun`/`Unsupported`
として正直に記録する。

追加する実機検証行:

- **2-track capture**: iD4 の 2 入力を `ChannelMap::mono(0)` /
  `ChannelMap::mono(1)` の 2 armed tap に binding し、別 staging
  stream（別 media/journal）へ記録する（AC1 の 2Track 行）。
- **実 device → 実 `StreamWriter`/staging end-to-end**: 現行 lane が
  `InspectionWriter` で止まっているのに対し、実機 lane を実
  `StreamWriter`（staging media/journal・checkpoint/finalize・
  capture-fault marker）まで接続する。`tests/common/mod.rs` に
  `&dyn Device` を受ける実機用 rig helper を追加し、fake `DeviceRig`
  の scripted handle 経路は維持する。
- **RT queue-full fail-stop**: 極小 `callback_queue` + pump 停止で
  決定的に `FAULT_QUEUE_FULL` を誘発し、capture-fault marker・
  verified prefix の連続性・commit 層の recoverable 分類を実機で
  確認する（AC2 実機行）。
- **format/rate 不適合 open の正直な reject**: 実 device の
  supported space から非 F32・range 外 rate・未対応 layout・
  不正 `BufferSize` を構成し、`Err(DeviceError::UnsupportedConfig)`
  が backend 到達前に返ることを検証する。adapter 層でも
  `UnsupportedSampleFormat`/`ChannelOutOfRange` の fail-closed を
  確認する（AC2）。
- **punch 境界の実機 Sample 単位検証**: `GateSpec::Punch` の
  `[in, out)` が実機 capture で Sample/frame 単位に正確であることを
  journal span の session/source/project 座標で検証する（AC3 実機
  昇格）。
- **bounded 長時間 capture + 連続性/drift 監視**: 30 秒級の実機
  capture で frame 連続性・fault 無し・journal span 連続性を確認し、
  実測 frame 数と wall-clock から実効 rate/drift を記録する
  （長時間 stability 行）。
- **multi-device same-clock session**: `input_devices()` が 2 台以上の
  F32-openable device を返す環境でのみ `InputSession` の
  `same_clock_domain` 検証を通した 2 stream 録音を試行する。
  単一 device 環境・clock domain 不一致は `NotRun` 記録。
- **PCM fidelity**: deterministic source が無い限り `NotRun`/
  `Unsupported` を記録する。実機 PCM は期待値と比較せず、
  media/journal の crc32 整合・非有限値検査・構造検証に留める
  （捏造しない）。

## Why

Issue #115（RCD-01）の実機受入 gate は「実 device → 実
`StreamWriter`/staging → finalize」の経路で 2 track・fail-stop・
境界・長時間を実測することを要求する。先行 slice
（`rcd-01-real-backend-readiness`）で composition root と opt-in
lane の土台はできたが、lane は `InspectionWriter` までで実 staging
永続化を通っておらず、fake `DeviceRig` は `&FakeInputDevice` 固定で
実 device を結線できない。本 slice はその gap を埋め、実機で
得られる証拠と得られない証拠（NotRun/Unsupported）を分離して記録
する。

## Users and Value

- Synvelis recording 基盤の受入判定者: 実機 lane の各行が
  Pass/NotRun/Unsupported のいずれかで説明可能になり、fake 証拠を
  実機 Pass に誤昇格させない。
- 後続 slice（#114 製品 E2E・#116 recovery・#117 monitor UX）の
  前提となる実機 staging 証拠を確保する。

## Scope

### In Scope

- `tests/real_input.rs` の `#[ignore]` 実機 lane 拡張（同一 binary
  に追加するだけで既存 `mise run test-recording-real-input` /
  `lint-recording-real-input` lane に載る。`mise.toml`・
  `Cargo.toml` は変更しない）。
- `tests/common/mod.rs` への実機 rig helper 追加（`&dyn Device`
  対応）と rate 指定 plan helper の追加。既存 fake `DeviceRig`・
  既存 test の signature/挙動は維持する。
- `src/input.rs`/`src/real_input.rs` は API gap が出た場合のみの
  最小変更。
- 上記 doc chain（spec/design/plan/task/test-spec/impl IR）。

### Out of Scope

- 製品録音 Command（`synvelis-app` の `syn.recording.*`、
  `service.rs`/`app.rs`/`engine.rs`/`binding.rs`）— #114・他 Worker
  占有。
- cross-clock multi-device（#119 の先取り禁止）。
- macOS/Linux 実機 lane（ruviform#251 Stage M/L 依存）。
- physical USB 抜去・3ch 超 multi-channel の自動化（iD4 は 2in、
  自動化不能/ハード不足 → NotRun 維持）。
- Mock/fake 結果の実機 Pass への昇格（AC4）。
- `mise.toml`/`Cargo.toml`/`Cargo.lock`/`.github/workflows/`/
  `synvelis-app/**` の変更。

## Requirements

- 実機不在・未接続・format 交渉不能・必要 channel 数不足・
  multi-device 前提不成立は loud SKIP（print + return）とし、
  test binary を落とさない。device があるのに open/start/callback/
  録音経路が契約を満たさない場合は hard fail —「open+start だが
  callback 0 件」は成功ではなく失敗。
- 実機 lane は `#[ignore]` opt-in のまま。既定 `mise run test` は
  hardware 非依存を維持する。
- RT callback の契約（allocation/dealloc・blocking・I/O・panic 禁止）
  は変更しない。queue-full 誘発は pump 停止という非 RT 側の駆動で
  行い、callback 側の挙動は既存契約のまま観測する。
- 実機 PCM を deterministic 期待値と比較しない。検証は構造
  （journal replay・span 連続性・source 連続性・crc32・media 長・
  finalize outcome）と観測値（frame 数・rate・drift・RMS/peak）の
  記録に限定する。
- `NotRun`/`Unsupported`/`blocked` の記録は test の print 出力と
  doc（test-spec/impl IR/PR 本文）で行い、暗黙の pass にしない。

## Acceptance Criteria（本 slice 分・Issue #115 転記）

- [ ] 実 device の 2 入力が `ChannelMap::mono(0)`/`mono(1)` の 2 tap
  で別 staging stream へ記録され、両 stream が capture data を受け
  Complete finalize + 連続 journal span を持つ（2ch 未満・device
  不在は SKIP/NotRun）。
- [ ] 実 device が実 `StreamWriter`/staging（media/journal/
  checkpoint/finalize/capture-fault marker）まで end-to-end で
  録音し、journal replay が intact で span が session 0 から連続し
  source が連続する。
- [ ] 極小 `callback_queue` + pump 停止で `FAULT_QUEUE_FULL` を
  実機誘発し、capture-fault marker・verified prefix 連続性・
  commit 層の recoverable 分類を確認する。
- [ ] 非 F32・range 外 rate・未対応 layout・不正 buffer size の
  open が `Err(DeviceError::UnsupportedConfig)`（adapter 層は
  `InputBindError`）で fail-closed に reject される。判定不能な
  行は NotRun/Unsupported。
- [ ] `GateSpec::Punch` の境界が実機 capture で Sample/frame 単位
  に正確（span の session/source/project 座標が期待値と一致、
  media が punch 区間のみを含む）。
- [ ] 30 秒級の bounded capture で fault 無し・journal 連続・
  frame 連続性を確認し、実測値（frame 数・実効 rate・drift）を
  記録する。
- [ ] deterministic source が無い PCM fidelity 行は `NotRun`/
  `Unsupported` として記録され、fake/mock 結果が実機 Pass に
  見えない。
- [ ] 全検証は `mise run` canonical task で実走し、結果を
  Pass/NotRun/Unsupported/blocked に分けて PR・Issue #115 へ記録
  する。

## Constraints

- `tests/real_input.rs`・`tests/common/mod.rs`・`src/input.rs`・
  `src/real_input.rs` のみ編集可（`src/*` は API gap が出た場合の
  最小変更に限る）。`mise.toml`・`Cargo.toml`・`Cargo.lock`・
  `.github/workflows/`・`synvelis-app/**`・docs index は変更しない
  （index 統合は Coordinator）。
- 実機 lane の追加 test は全て `#[ignore]`。既定 lane
  （`mise run test`/`check`/`lint`/`test-rt`/`test-msrv`/`build`）を
  hardware 非依存に保つ。
- device 列挙・format 交渉・open は非 RT control path。
  `callback_queue`/`max_block_frames` は spec 契約どおり caller が
  与え、超過は既存 `FAULT_*` で fail-stop する（捏造しない）。
- `InputSession` の `same_clock_domain` 検証を回避・緩和しない。
  clock domain 不一致は `NotRun` 記録であり pass にしない。
- 時間演算は `Tick`/`TickDuration` の checked 演算規約に従う
  （本 slice の追加コードは wall-clock `Instant`/`Duration` と
  frame 数の観測のみで tick 演算を直接行わない）。

## Deferred Design Concerns

- physical USB 抜去・device removal の自動検証 | 実機操作の自動化
  不能 | 手動 double-opt-in lane（ruviform 先例）整備時 | removal
  時の fail-stop が未検証のまま残る。
- 3ch 超 multi-channel・cross-clock multi-device 相関 | iD4 は
  2in・#119 が clock 相関を所有 | #119 / 対応 hardware 取得時 |
  multi-device 証拠が単一 clock domain に限定される。
- OS loopback 等の deterministic PCM source | 環境依存で保証不能 |
  deterministic source が利用可能になった時 | PCM fidelity が
  NotRun のまま残る。
- macOS/Linux 実機 lane | ruviform#251 Stage M/L 未到達 | Stage
  M/L 完了時 | 実機証拠が Windows/WASAPI に限定される。

## Sources and Relations

- Issue #115（RCD-01）— 実機録音受入 gate の出典。
- `docs/specs/rcd-01-real-backend-readiness.md`（SPEC-gut3XVRAIUtMHADNZBNFHC）
  — composition root + opt-in lane の先行 slice。
- `docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md`
  （SPEC-034UoScGX6dIi25hMurqCI）— input adapter 契約の正本。
- `docs/tasks/rcd-01-task1-real-backend-readiness.md`
  （TASK-GJlWE8j5emvVOKFMtbTXHR）— 先行 slice の実施記録。
- ruviform#251（real input stream/duplex/clock/latency）・
  ruviform#565（WASAPI frame-position 供給）— 上流依存。
