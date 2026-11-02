---
id: "TSPEC-034QmjuUDQRs8JzfSsG8wM"
type: "test-spec"
status: "approved"
title: "L2 検証仕様 — Master Sequential Offline PCM WAV Export・Reopen/Verify・Safe Publish"
created: "2026-11-02"
updated: "2026-11-02"
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
    - "docs/designs/l2-master-sequential-offline-wav-export-design.md"
  derived-by: []
  verifies:
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# L2 Test Spec — Master Sequential Offline PCM WAV Export

検証本体: `crates/synvelis-render/tests/l2_e2e.rs` + `synvelis-render` unit tests。
Fixture は L1 と同一 deterministic `fixture_sample` (PCM16 WAV, 48kHz mono/stereo)。

## Feature: Reopen Verify (L2-01, MR-012, MR-018, EX-001, EX-018)

- mono/stereo deterministic fixture → Master render → 実 PCM WAV を
  独立 Reader (symphonia demux + SourceReader) で reopen。
- frame count / channels / rate が OutputFacts と一致、decoded PCM が
  期待値 (fixture_sample を canonical f32 → S16 quantize 往復) と
  tolerance (=1 LSB) 内で一致。

## Feature: Time/Graph Contract (L2-02, MR-014, MR-015, TT-005, TT-008)

- clip が 0 より遅く開始 → 先頭 silence + clip 区間の exact frame 配置。
- clip 間 gap → gap 区間 silence。
- source midpoint 参照 (`source_start > 0`) → 対応 sample 一致。
- range 外 clip → emit なし。
- project 負 pre-roll (anchor < 0) → L1 同様 clamp された window のみ emit。
- 境界直前終端 clip → 半開区間の exact frame count。
- render range `[start,end)` が block boundary に揃っていない場合でも
  frames_emitted = end - start。

## Feature: Stateful DSP / Block Invariance (L2-03, MR-016)

- Gain/Mix・1-pole filter・Delay processor を render block
  64/128/256 で実行し、出力が tolerance 内で一致 (先頭から連続処理)。

## Feature: Preflight Fail-Closed (L2-04, MR-013, MR-021)

- Unknown Tail (TailSpec 未指定/表現不能) → `PreflightError::UnknownTail`、
  output 未作成。
- processor が `OfflineRenderSupport::Unknown/Declared` (mock) →
  ruviform preflight `OfflineUnsupported` — bypass しない・Exact 成功を返さない。
- Missing Source / Unverified Source / Invalid Range / Insufficient
  Capability / Destination Conflict が各々 typed で不合格。

## Feature: Fault Injection / No Partial Completed (L2-05, MR-019, MR-023)

- Sink/Writer Full・Disk full・Close/Sync 失敗・Cancelled・
  Destination Exists・Reopen 失敗 → `Failed{stage}`/`Cancelled{phase}`、
  Completed receipt なし。失敗後も既存 destination bytes 不変、temp 破棄
  または recoverable。

## Feature: Snapshot Invariance (L2-06, MR-020, MR-028)

- export 前後で `Project` live revision / saved revision / undo-redo stack /
  source content hash が不変。export が `Operation` を発行しない。
- `replace_existing=false` で既存 destination 保護、`true` で置換。

## Feature: Capability Evidence (L2-07)

- `ruviform_io_wav::wav_capabilities()` (encodable wire format/layout)、
  `PreparedRender::capabilities()` (per-node `Verified` map)、
  `plan.total_latency`、tail_evidence を receipt に記録。
- Cargo.lock ruviform pin = `322a661` (rf#198 merge) を impl record に記録。

## Feature: Evidence Recording (L2-08)

- `cargo test --workspace --locked`・headless E2E コマンド・fixture hash・
  frame count・backend・OS・toolchain・exit code・未計測事項を impl record に記録。
