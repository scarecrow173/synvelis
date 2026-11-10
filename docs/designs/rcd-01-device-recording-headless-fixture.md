---
id: "DESIGN-034V2hPBihovSNoj4xusOA"
type: "design"
status: "approved"
title: "RCD-01 device-recording headless fixture — Mock Input→Raw Writer 統合設計"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-device-recording-headless-fixture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 device-recording headless fixture — Mock Input→Raw Writer 統合設計

## Context

RCD-01 の fake port 契約層（`src/input.rs`）は実装済みで、`InputAdapter` は fake `Device`/`InputStream` の callback を `InputBlock` 化し、armed tap ごとに `WriterEnds`（engine/writer SPSC endpoint）を払い出す。しかしこの endpoint を実 `StreamWriter`（staging 上の media/journal を書く非 RT writer）へ接続する統合 fixture は存在せず、既存テストは in-memory inspection writer で PCM を検証している。本設計は fake port → adapter → 実 writer → journal/finalize/recovery までの結合を `tests/` のみに追加して固定する。

## Scope

### In Scope

- `crates/synvelis-recording/tests/common/mod.rs` への統合 harness 追加（`StagingArea` 構築、`TapWriterEnds` → `StreamWriter` 配線、駆動 loop、finalize/verify helper）。
- `crates/synvelis-recording/tests/` への新規 fixture テストファイル。
- `crates/synvelis-app` の recording 状態投影（`Project.recording` entity → 表示行）と `synvelis-app` 側 fixture。

### Out of Scope

- `crates/*/src/` への変更（adapter/writer/engine は既存契約をそのまま使用）。
- 実 Device・OS driver・実機検証、録音 command、#114/#116/#117/#119 領域。

## Components and Boundaries

- **Fake port（`tests/common/mod.rs`）**: `FakeDeviceEnumerator`/`FakeDevice`/`FakeInputStream`/`MockPcm`。scripted PCM・clock domain・frame 進行・fault 注入を提供。変更は最小限（必要なら helper 追加のみ）。
- **InputAdapter（`src/input.rs`、既存）**: `InputAdapter::open(&device, &spec, fault)` → `OpenedInput { adapter, writer_ends }`。RT callback → bounded queue → `pump(&scheduler)` で `CaptureEngine`/`InputBlock` へ供給。変更しない。
- **StreamWriter（`src/writer.rs`、既存）**: `WriterEnds` を `ChunkSink` として受け、staging media/journal へ書く。`StreamWriter::create(&area, cfg, ends, fault)` で生成。
- **StagingArea（`src/staging.rs`、既存）**: session dir/manifest/media/journal/finalized/capture-fault marker の owner。
- **recovery/commit（`src/recovery.rs`/`commit.rs`、既存）**: verified prefix replay・finalize・commit 分類。本 slice は recovery/finalize までを対象とし、commit は fault marker の観測に留める。
- **synvelis-app recording 投影**: `Project.recording`（`RecordingSession`/`RecordingPass`/`CaptureStream`/`CaptureSegment`/`Take`/`ClockEpoch`）を read-only に投影する module。永続 entity を変更しない。

## Data and Control Flow

1. `StagingArea::create(root, StagingSessionConfig{session_id, sample_rate, streams})` — `plan.targets` の各 `CaptureTarget` に対応する `StagingStreamConfig`（`stream_index`, `capture_stream_id`, `lane_id`, `channels`）を manifest 化。
2. `InputAdapter::open(&device, &InputSession{spec, taps, fault})` → `OpenedInput`。
3. 各 `TapWriterEnds` を対応する `StagingStreamConfig` で `StreamWriter::create` し、`WriterEnds` を接続。
4. 駆動 loop: `stream.deliver(n)` → `adapter.pump(&plan.scheduler)` → 各 `writer.pump()`。fault latch 観測時は loop を止める。
5. fault 時: `area.record_capture_fault(detail)` → `checkpoint()` → `finish_all`（`FinalizedSourceRef`）。正常時も `checkpoint` → `finish_all`。
6. 検証: `recovery::verify_stream`/journal replay で verified prefix、media bytes と `MockPcm`+channel map からの Expected PCM の bit 一致、span の session/source/project tile、`capture_fault_marker_exists`。
7. synvelis-app: fixture が `Project.recording` entity を投入し、投影 module の出力（session/pass/stream/take 行・summary）を assert。

## Non-Functional Constraints

- fixture は完全に決定的（seed 固定・block size 固定・実時間に依存しない）。
- `InputAdapter` の RT callback 経路は変更しない。fake port は既存 `tests/common` の機構を再利用する。
- media/journal の on-disk 形式・verified prefix 意味論を変更しない。
- 全テストは `mise run` 経由で実行可能であること。

## Risks and Trade-offs

- `TapWriterEnds.stream_index` と `plan.targets`/staging manifest の対応付けを誤ると lane/channel がずれる → fixture 内で `stream_index == target index` の対応を明示し、manifest 照合で検出する。
- engine fault 時に writer は `Complete` finalize し得る（journal は fault 前まで正常）→ `capture-fault` marker が session レベルの Partial/Unknown を担う既存設計に従い、marker + recoverable 分類を観測対象とする。stream 単体の outcome 表記ゆれは既存契約の範囲として記録する。
- `synvelis-app` への追加は共有ファイル（`service.rs` 等）に触れる可能性 → additive な変更に限定し、#112 占有領域（録音以外の画面/service）には触れない。

## Deferred Design Concerns

- 実 Device open/capture・OS driver | RF#251 未提供・実機環境なし | RF#251 解消後の実 Device slice | fake fixture は実デバイス固有の問題を検出できない。
- 録音 command・UI | #114/SV-GUI-05 | #114 slice | command 未実装のままでは UI 操作は不可。

## Verification Notes

- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` が Pass する。
- [ ] 新規 fixture が fake port であることをテスト・doc で明示し、実 Device Pass と誤認されない。
