---
id: "TSPEC-034W91IonN281HbgpyCQ1P"
type: "test-spec"
status: "proposed"
title: "RCD-01 実機 capture coverage 拡張 test-spec"
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
    - "docs/plans/rcd-01-real-input-coverage-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-real-input-coverage.md"
  derived-by: []
  verifies:
    - "docs/designs/rcd-01-real-input-coverage-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/251"
    - "https://github.com/scarecrow173/ruviform/issues/565"
  defers: []
  deferred-by: []
---

# RCD-01 実機 capture coverage 拡張 test-spec

## Purpose

Issue #115（RCD-01）の実機受入 gate のうち、本 slice が担当する
行（実 `StreamWriter`/staging 経路・2 track・fail-stop・
format/rate reject・punch 境界・長時間連続性・PCM fidelity の
正直な記録）について、各行が Pass/NotRun/Unsupported のいずれで
あるかを判定可能にする検証規則を固定する。fake/headless の
契約証拠を実機 Pass に昇格させない境界をここに明記する。

## Feature

実機 lane（`mise run test-recording-real-input`、
`tests/real_input.rs` の `#[ignore]` + `real-device` feature）は、
接続された実 audio device に対して「実 device →
`InputSession`/`InputAdapter` → 実 `StreamWriter` → staging
media/journal → checkpoint/finalize」の経路を駆動し、観測した
構造・連続性・fail-stop・境界を検証する。実機が無い・必要な
前提（channel 数・format 交渉・same-clock multi-device・
deterministic PCM source）を満たさない行は loud SKIP/`NotRun`/
`Unsupported` として記録し、binary を落とさない。

## Rules

- **R1（opt-in 分離）**: 全実機 test は `#[ignore]` +
  `real-device` feature。既定 `mise run test`/`check`/`lint`/
  `test-rt`/`test-msrv`/`build` は hardware 非依存を維持する。
- **R2（loud SKIP vs hard fail）**: device 不在・`default_host`
  失敗・format 交渉不能・必要 channel 数不足・権限失敗・
  multi-device 前提不成立は loud SKIP（print + return）。device
  があり open/start/callback/録音経路が契約を満たさない場合
  （callback 0 件・`FAULT_*`・format 不一致）は hard fail。
- **R3（構造検証のみ）**: 実機 PCM を deterministic 期待値
  （`MockPcm` 等）と比較しない。検証は journal replay intact・
  span session/source/project 座標・media 連続性・chunk
  `pcm_crc32`・media 長・finalize outcome・PCM 有限値に限定する
  （`verify_capture_integrity`）。
- **R4（fail-stop 正直性）**: queue-full 等の fault 誘発は
  pump 停止という非 RT 駆動で行い、誘発できなかった場合は
  Pass にせず観測値を記録して fail。fault 後の入力が記録に
  追加されないこと（verified prefix 保持）を確認する。
- **R5（rate 非依存）**: negotiated `sample_rate` を
  `plan_at`/`StagingSessionConfig`/`finish_all` で一致させ、
  境界期待値は `plan.mapping.tick_to_media` で rate 別に計算
  する。48k 固定値を実機期待値に使わない。
- **R6（NotRun/Unsupported 記録）**: deterministic PCM source
  不在・same-clock multi-device 不成立・format/backend が
  変換受理して reject を観測できない行は `NotRun`/`Unsupported`
  として print + doc 記録し、暗黙の pass にしない。
- **R7（fake rig 非破壊）**: `DeviceRig` の signature・挙動・
  scripted handle 経路は不変。一般化は `open_rig_core` 抽出と
  `&dyn Device` coerce に留め、既存 fake test が無変更で pass
  すること。

## Examples

- **2-track**: Given iD4 が stereo F32 で open 可能、
  When `ChannelMap::mono(0)`/`mono(1)` の 2 armed tap を 1 spec
  に束ねて録音、Then 両 stream が capture data を受け、各
  staging media/journal が連続 span を持ち Complete finalize
  する。channel 数 <2 なら loud SKIP。
- **end-to-end writer**: Given negotiated format、
  When `ChannelMap::identity` で録音し `finish`、Then journal
  replay が CleanEnd/TornTail・span が session 0 から連続・
  source が連続・media 長が `source_frames × channels × 4`
  と一致・`build_payload` が capture-fault 無しで Complete
  分類する。
- **RT queue full**: Given `callback_queue` 小 + warm pump 済み、
  When pump を停止して work queue を満杯にする、Then
  `FAULT_QUEUE_FULL` が立ち、capture-fault marker・verified
  prefix 連続性・`build_payload` の recoverable 分類を確認
  できる。fault 後に新規 span が増えない。
- **format/rate reject**: Given negotiated format、
  When 非 F32・range 外 rate・未対応 layout・`BufferSize::
  Fixed` を構成して open、Then `Err(DeviceError::
  UnsupportedConfig)`/`InputBindError` で fail-closed reject
  されるか、backend 変換受理の場合は `Unsupported`/`NotRun`
  記録。
- **punch 境界**: Given `GateSpec::Punch`、
  When 実機 capture、Then journal span の session/source/
  project 座標が `tick_to_media(punch_in/out)` の期待値と
  Sample 単位で一致し、media が punch 区間のみを含む。
- **長時間**: Given `GateSpec::Normal`・約 30 秒相当の stop、
  When bounded capture、Then fault 無し・journal/span/source
  連続・`verify_capture_integrity` が通り、実測 frame 数・
  実効 rate・drift が記録される。
- **multi-device**: Given `input_devices()` が 2 台以上、
  When `InputSession::open`+`add`、Then same-clock 検証を通れば
  2 stream 録音、`ClockDomainMismatch`/`UNKNOWN`/`Device(_)` は
  `NotRun` 記録。
- **PCM fidelity**: Given deterministic source 不在、
  Then `NotRun`/`Unsupported` を記録して return。実機 PCM を
  捏造期待値と比較しない。

## Guarantees

- 実機 lane の各行が Pass/NotRun/Unsupported/blocked のいずれか
  で説明可能であり、fake/mock 結果が実機 Pass に見えない。
- 実 `StreamWriter`/staging（media/journal/checkpoint/
  finalize/capture-fault marker）までの経路が実機で駆動され、
  durable prefix と commit 層分類が観測される。
- 既定 lane（`mise run test` 等）が hardware 非依存のまま
  全 pass する。

## Non-goals

- `synvelis-app` の製品録音 Command・state projection・
  app-level E2E（#114・他 Worker 占有）。
- cross-clock multi-device（#119）・macOS/Linux lane
  （ruviform#251 Stage M/L）・physical USB 抜去・3ch 超
  multi-channel の自動化。
- 実機 PCM の deterministic fidelity 比較（source が無い限り
  `NotRun`/`Unsupported`）。
- `mise.toml`/`Cargo.toml`/`Cargo.lock`/`.github/workflows/` の
  変更。

## Risk

- queue-full 誘発が backend の callback 周期に依存し、timeout
  で fail し得る — 「誘発不能」を Pass にしない規則で担保。
- negotiated rate が 48k でない場合に境界期待値を誤る —
  `tick_to_media` で rate 別計算する規則で担保。
- `BufferSize::Default` の可変 block が `chunk_frames` を
  超える device で fail-stop — 上限 8192 で吸収し、超過は
  正直に fail。
