---
id: "DESIGN-poso4hBPDap0Gs9cisA92Y"
type: "design"
status: "approved"
title: "SF-03 slice7 — render semantic-lowering 消費 design"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/sf-03-slice7-render-consumption.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice7-render-consumption.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice7-render-consumption.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  defers: []
  deferred-by: []
---

# SF-03 slice7 — render semantic-lowering 消費 design

## Overview

legacy `lower_with_inserts`（clip→insert→sink の project 非対応
legacy graph）を廃し、export を semantic lowering の offline 変種
`lower_semantic_offline` へ移す。realtime engine と同一の
strip/chain/send/bus topology・playback plan・source map を共有し、
output 境界だけを `SemanticOutput` で拡張する。

## Detailed Design

### 1. `SemanticOutput`（`routing_lower.rs`）

```rust
pub struct SemanticOutput {
    pub insert_count: usize,        // master post-fader → sink の insert slot 数
    pub format: Option<AudioFormat>, // device-sink tap format override
}
```

- `lower_semantic` = `lower_semantic_inner(.., SemanticOutput::default())`
  で realtime 経路は不変（insert 0・sink = engine format）。
- `lower_semantic_offline(.., output)` が offline/render 経路。
- `format` は engine format と sample rate/sample format が一致必須
  （不一致は `SemanticLowerError::OutputFormat` →
  `RoutingFault::UnsupportedFormatOrTimeMap`）。channel 数差のみ許容
  し、sink edge に `GraphChannelMap`（sink ch `d` ← graph ch
  `min(d, src-1)`、playback edge と同一 convention）を明示付与。
  semantic graph 内部に暗黙 mix は存在しない。
- insert node は engine format の Processor node で、in/out を
  直列接続: master post-out → in0 → out0 → … → outN → sink。
  `SemanticLowered::inserts: Vec<InsertBinding>` に signal order で
  返し、`node_roles`/`source_map` には登録しない（semantic 意味を
  持たない output-stage のため）。

### 2. `synvelis-render::export` 移行

- graph engine geometry は `project.routing.port_channel_count()`
  由来（uniform 宣言のみ；mixed/empty は `UnsupportedFormatOrTimeMap`
  で fail-closed）。`request.engine.channels` は output tap channel
  数（preflight の 1..=2 gate は従来通り output に適用）。
- `RoutingBuildKey`: `project_id`/`project_revision`(=snapshot
  live_revision)/`routing_fingerprint`/`capability_generation`/
  `LOWERING_POLICY_VERSION`/`BuildMode::Offline` を snapshot から
  計算。`engine_config_generation`/`source_format_generation`/
  `device_clock_epoch`/`runtime_instance` は stateless export に
  世代系譜がないため `0` sentinel（coherence evidence は
  revision+fingerprint+capability が担う）。generation は `1`
  （当該 build の唯一の plan generation）。
- `BuildBudget` は固定上限（nodes 2^16 / connections 2^17 /
  max_frames=max_block）— offline 呼出に caller budget 経路がない
  ため。
- staging: `node_roles` を走査し `Playback`→decode+ClipRenderer、
  `StripFader`→StripProcessor、`SendGain`→GainProcessor、
  `ChainProcessor`→`ProcessorHost::connect`（`BuildMode::Offline`、
  port_format=graph_format）。host 非供給は `PluginUnavailable`、
  connect error は `e.fault()`、instance の
  `offline_support()!=Verified` は `OfflineCapabilityMismatch`。
  lease/control/state_handle は render 完了まで保持。
- output insert は graph channel 数で state 確保（output channel
  数ではない — node port format は graph format）。
- `RenderSpec.output_port = lowered.device_in`、`layout` は output
  tap layout。`total_latency` の declared 検査は不変。
- `ExportReceipt.source_map` を追加し generation-scoped
  `RoutingSourceMap` を証拠として残す。

### 3. `PreflightError::RoutingLowering { fault, detail }`

lowering/provisioning の全失敗を typed `RoutingFault` + detail で
分類。realtime engine の `RoutingFault` taxonomy と同一語彙 —
cross-consumer で同じ defect が同じ class に落ちる。

### 4. `RoutingModel::port_channel_count()`（`routing.rs`）

non-empty uniform port channel 数を `Option<u32>` で返す helper。
consumer が lowering engine geometry を決めるための読み取り専用
projection で、routing semantics は変更しない。

### 5. fixture 更新

semantic lowering は clip の lane→track→strip 解決を要求するため、
render test の `lane()`/`controller_with()` helper に
`ProjectOp::AddTrackStrip`（fader base 1.0 = unity、bit-exact-safe）
を追加。`sf03_l2_10` は `GainProcessor` が Verified 化したため
明示的 `UnknownGain` wrapper を導入。

## Risks

- mono output: semantic graph は stereo port で構築され sink edge
  のみ mono tap — l2 の mono fixture が bit-exact を維持することで
  実証済み。
- insert 配線ミス（dangling node）: 初回実装で発生し test で検出・
  修正済み。`connect_master_sink` は master→insert 直列→sink の
  単純鎖に整理。
