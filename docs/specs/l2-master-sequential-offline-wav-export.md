---
id: "SPEC-034Qmb9CC7dN4YE5VPN235"
type: "spec"
status: "approved"
title: "L2 — Master Sequential Offline PCM WAV Export・Reopen/Verify・Safe Publish"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/ruviform/issues/191"
    - "https://github.com/scarecrow173/ruviform/issues/198"
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
    - "docs/discovery/l2-master-sequential-offline-wav-export-discovery.md"
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/l2-master-sequential-offline-wav-export-discovery.md"
  derived-by:
    - "docs/designs/l2-master-sequential-offline-wav-export-design.md"
    - "docs/plans/l2-master-sequential-offline-wav-export.md"
    - "docs/specs/syn-rf305-wav-export-staged-file.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# L2 — Master Sequential Offline PCM WAV Export・Reopen/Verify・Safe Publish

## What Should Be Built

Issue #25: L1 が固定した同一 Semantic Lowering (Verified Source→Clip→Generic Graph) を
**Render Input Snapshot** として非 RT で固定し、Master Mix を Ruviform Sequential
Offline Driver (`ruviform-audio::render`, rf#198) で実 PCM WAV (`ruviform-io-wav`,
rf#191) として Staged 出力、**独立 Reader で再 Open して内容を検証**し、
Publish policy に従い Destination へ Safe Publish する E2E スライス。

新 crate `synvelis-render` が RenderSpec/Preflight/Output Recipe/Job を所有し、
Generic Codec/Graph/Transport 処理は Ruviform、Output Recipe・Overwrite Policy・
プロジェクト契約は Synvelis が決める。

## Why Now

E2E-L1 (#24, PR #33) で Local WAV→Verified Media→Headless Playback が合格し、
Ruviform 側も rf#191 (PCM WAV Encoder/Muxer/MediaSink/StagedOutput, PR #229)・
rf#198 (PreparedRender/offline_support gate/staged output lifecycle, PR #235) が
main へマージされた (#198 merge = `322a661`)。Export 経路の最初の実装単位。

## Users and Value

- DAW ユーザー: Master Mix を確定的・検証済み WAV として取得でき、失敗時も
  既存ファイルと Project を失わない。
- 開発者: Export が同一 Time/Graph 契約で検証可能となり、以後の Stem/Freeze/
  Plugin 統合の基盤契約が固定される。

## Scope

### In Scope

1. **RenderSpec (Synvelis)**: Project Snapshot Revision・Source/Asset Content
   Version (digest 集合)・TempoMap/Graph/Processor State・Render Range
   `[start,end)`・Channel/Sample Rate・Destination/Output Policy を
   非 RT で固定する frozen 値。Live Revision/Last Saved/Active Plan
   Revision と別物として記録する。
2. **Preflight**: Missing Source・Unsupported Plugin/Codec・Unknown Tail・
   Invalid Range・Insufficient Output Capability・Destination Conflict を
   Output 開始前に検出。未対応 Plugin を勝手に Bypass して Exact 成功を返さない
   (L2 最小 E2E は Plugin 不要の Deterministic Mix; Plugin node 型は graph に
   存在しないため構造的に bypass 不能)。
3. **Sequential Engine**: Clip Offset/End/Gap・Frame Boundaries・Graph
   latency/PDC・Tail/Preroll を同一 Time/Graph 契約で評価。Stateful DSP は
   先頭から連続処理、Chunk 単位の無条件時間並列化は行わない。
4. **PCM Output**: Interleaved wire・Channel Mapping・Sample Rate・Float↔PCM
   変換・Clipping/NaN/Inf 方針 (io-wav: saturate/NaN→0/±Inf→±FS/round-half-away、
   dither 無し) を Recipe として指定。Encoder→Drain→Muxer Finalize→
   Close/Sync→Reopen Validate→Safe Publish の順を守る。
5. **Job/Failure**: Progress/Cancel・Failed/Partial/Completed を区別。
   Reopen 検証前を Completed としない。外部 Export は Project を Mutation
   しない。Crash/Cancel/Error 後に既存 Destination と Raw Source を保持。

### Out of Scope

Stem/Freeze/Bounce、Remote Provider、Live real-device、フル Plugin 統合、
Dither 実装、Resample/SRC、RF64/BWF、WAV 以外の container、GUI。

## Requirements

- Render は `PreparedRender` の孤立 GraphRuntime/RtRuntime で実行し、Live
  Playback (`HeadlessDriver`) の Processor/Plugin Instance と実体を共有しない。
- 全 install processor は `OfflineRenderSupport::Verified` を preflight で要求
  (ruviform の gate)。Synvelis は clip renderer と deterministic DSP processor に
  正直な `Verified` を実装する。
- Clip 再生は prepare 時に window span を canonical f32 planar へ decode し、
  process はメモリから供給する (`ClipRenderer`)。process 内 I/O なし。
- Render 範囲外の process (preroll/PDC 補償) は emit しない。Tail は
  `TailSpec::Known/Estimated` で明示; 不明 tail は preflight で不合格。
- Output は `StagedOutput` temp → muxer `finish` → reopen verify hook →
  `publish(PublishOptions)`。失敗時は temp を破棄し既存 Destination は
  バイト不変で保持。
- Job 結果は typed: `Completed { receipt, verify }` / `Failed { stage, error }` /
  `Cancelled { phase }`。Partial は Completed と表示しない。
- Export 前後で Project live/saved snapshot・source digest・undo/redo が不変。

## Acceptance Criteria

- [ ] **L2-01**: 48kHz Mono/Stereo PCM deterministic fixture (L1 と同一
  `fixture_sample`) を Master render → 実 WAV を独立 Reader (symphonia demux +
  `SourceReader`) で reopen → Frame/Channel/Rate/PCM を tolerance 内で比較一致。
- [ ] **L2-02**: Clip が 0 より遅く開始 / 無音 gap / ソース途中参照 / 範囲外 /
  Project 負 pre-roll / 境界直前終端で、Output Frame 数とサンプルが計算どおり。
- [ ] **L2-03**: Gain/Mix および Stateful 1-pole filter/Delay を 64/128/256 の
  異なる Render Block で処理し、許容誤差内で一致 (block 非変性)。
- [ ] **L2-04**: Preroll/PDC/tail が指定範囲を誤って膨張させない。
  Unknown Tail/Unsupported Plugin は明示不合格 (mock 検証; 実 Plugin は対象外)。
- [ ] **L2-05**: Sink/Writer Full・Disk full・Close/Sync 失敗・Cancelled・
  Destination Exists・Reopen 失敗の fault 注入で Partial を Completed としない。
- [ ] **L2-06**: Export 前後で Project Live/Saved Snapshot と Source Content
  Hash が不変、Undo/Redo へ新規 Mutation を混入しない。既存 Destination は
  事前合意の Publish policy に従い保護。
- [ ] **L2-07**: rf#191/#198 の Backend Capability (`wav_capabilities`/
  `Capabilities`) / Stateful Gateway (`offline_support` map・`PreparedRender`
  compile 証拠) を添付し、MR-001〜028/EX/TT の該当番号を受入テストと追跡。
- [ ] **L2-08**: `cargo test --workspace --locked`・headless E2E test コマンド・
  Fixture hash/Frame Count・Backend/OS/Toolchain/exit code・未計測事項を記録。

## Deferred Design Concerns

- 長尺 source の decode-to-memory 上限 | L2 fixture は小規模のため未対策 |
  実尺 media の render 需要が出た時点で windowed decode/chunked supply に移行 |
  無視すれば大規模 source でメモリ圧。
- Plugin 統合 (rf#196 契約) | L2 範囲外 | Plugin 対応 Issue 着手時 |
  現在は Plugin node 型が無く bypass も不能 — 偽成功は構造的に起きない。
- Stem/Bounce/Freeze | L2 範囲外 | 該当 Issue 着手時 | —

## Sources and Relations

- Issue #25 本文 (実装範囲 1〜5・受入 L2-01〜08・依存/Rollback)
- `docs/discovery/l2-master-sequential-offline-wav-export-discovery.md`
- `docs/specs/l1-local-wav-verified-media-graph-playback.md` (L1 契約の再利用)
- `docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md`
  MR-001〜028、playback-mixer EX-001〜020、time-transport TT-001〜024
