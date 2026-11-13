---
id: "DESIGN-CwatwmnW371rKsz3F2ajlh"
type: "design"
status: "approved"
title: "SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約設計"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/sf-04-probe-adapter.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-04-probe-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
    - "docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md"
    - "docs/designs/sf-04-slice6-probe-compare-design.md"
    - "docs/designs/sf-04-slice8-probe-faults-declare-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
    - "docs/designs/sf-04-slice3-clock-epoch-discontinuity-design.md"
    - "docs/designs/sf-04-slice10-probe-compare-op-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
    - "docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md"
    - "docs/designs/sf-04-slice6-probe-compare-design.md"
    - "docs/designs/sf-04-slice8-probe-faults-declare-design.md"
    - "docs/designs/sf-04-slice10-probe-compare-op-design.md"
  defers: []
  deferred-by: []
---

# SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約設計

## 概要

headless project/player 層の実装は slice1..10 で完了済み。本設計は
受入集約タスクとして「既存実装の被覆監査 → 未被覆分岐の headless
テスト補強 → 外部 Gate の NotRun/Unsupported 記録」のみを定める。
`synvelis-project`/`synvelis-player` の src 変更は監査で contract
違反が見つかった場合の最小修正に限定し、見つからなければ行わない。

## アーキテクチャ

### 責務配置（既存・変更なし）

- `synvelis-project/src/probe.rs`: `SemanticProbePoint` /
  `resolve_probe_point` / `ProbeResolveState` / `ProbeObservation*` /
  `ProbeCaptureSegment` / `AcousticObservation` の semantic contract。
- `synvelis-player/src/probe.rs`: `ProbeSession`（binding ledger・
  capture segment・epoch cut）＋ Ruviform broker deck。
- `synvelis-player/src/probe_trace.rs`: `ProbeTraceBridge`
  （runtime event → `ProbeObservationUpdate`、stale/unknown 拒否）。
- `synvelis-player/src/probe_compare.rs`: 複数 Probe 比較の
  bounded non-RT consumer（#309 alignment 委譲）。
- `synvelis-workspace`（slice5）: subscriber lease/capture owner の
  session 層。本タスクの player 層テストはその境界を固定する。

### 新規テストの設計判断

`sf04_probe_acceptance.rs` の 2 テスト:

1. `sf04_probe_removed_entity_unbinds_and_reattach_binds_fresh`
   - entity（Processor）を `RemoveProcessor` で消して republish。
     `on_publish` の re-resolve が `Unresolved` を返し broker probe
     が外れて `runtime_probe_id = None` になることを RT-observed
     binding で検証（stale slot 観測なし）。
   - 同じ key の再 request が冪等に失敗（再 bind しない）こと。
   - 新しい processor id で re-attach → gen3 で fresh
     `BoundActive`・新 `RuntimeProbeId`・旧 binding は
     `Unresolved` のまま（old slot reuse 誤観測なし）。
   - 検証経路: `RoutingEngine::publish` → `ProbeSession::on_publish`
     → `resolve_probe_point`（`synvelis-project`）→ broker remove/
     install → `probe_drain` の RT-observed status。最下位で意味の
     ある層（project の resolve 関数自体は slice1 の
     `probe_resolution_*` テストで被覆済み）で、外部から観測可能な
     binding 状態を主張する。
2. `sf04_probe_same_key_request_is_idempotent_single_install`
   - 同一 `ProbeSubscriptionKey` の重複 request（mode 違い含む）
     が runtime probe を増やさず `requested_mode` も変更しないこと
     （冪等 early-return）。
   - `probe_capture` 中の重複 request + 再 `probe_capture` が
     `AlreadyCapturing` で fail-closed（capture session は binding
     あたり 1 つ）。
   - `session_id` の異なる key は別 subscription として独立に
     install されること — player 層の dedup 境界を明示し、window/
     inspector 間の lease 集約は session 層（slice5）の契約である
     ことを固定する。

## 被覆写像（SF-PROBE-01..08 → 既存証拠）

| AC | headless 証拠 |
| --- | --- |
| SF-PROBE-01 | project `probe_resolution_*`/`map_resolution_states_*`（Unavailable/Unsupported/非Audio拒否）、player `sf04_probe_adapter`（strip/bus/processor/send 解決・BoundActive）＋ 本タスクの entity 消失/re-attach |
| SF-PROBE-02 | `sf04_probe_adapter::sf04_probe_off_is_true_observation_silence`（Off が観測を出さない）＋ compare/slice テスト群で bypass/connection 非変更は lowering 側契約（#18） |
| SF-PROBE-03 | `sf04_probe_adapter` lifecycle/stale-generation/moved-point、`sf04_s2` trace bridge の stale/unknown 拒否、`sf04_s3`/`sf04_s9` epoch cut ＋ 本タスクの SourceMap 失効 unbind |
| SF-PROBE-04 | `sf04_probe_adapter::sf04_probe_generation_swap_cuts_capture`、`sf04_s8` fault（QueueFull/LostBlocks 等の #308 系 fault injection）— #308 CLOSED のため契約証拠は有効、実機 Gate は NotRun |
| SF-PROBE-05 | `sf04_s8`（max_frames・ChunkPool/out channel/telemetry 満杯・LostBlocks/LostRecords/Unobserved・Silence≠Missing） |
| SF-PROBE-06 | slice5（synvelis-workspace subscriber lease、merge 済み）＋ 本タスクの per-key 冪等・session 境界テスト |
| SF-PROBE-07 | `sf04_s7` compare（Aligned/Conditional/Incomparable・epoch/gap/format 分離）、`sf04_s3`/`sf04_s9` epoch/seek discontinuity — #309 CLOSED、実測 Gate は NotRun |
| SF-PROBE-08 | `sf04_s8` observation states（Plugin unavailable/Probe Off/PCM silence/No Data の状態分離）— Diagnostics 原因候補解釈・UI 分離・Accessibility は #108/#109 |

詳細な行単位の被覆表は `evidence/sv107/coverage.md` に記録する。

## 失敗時の扱い

- テスト失敗 = contract 違反の可能性 → `resolve_probe_point` /
  `ProbeSession::on_publish` / broker install のいずれが invariant
  を破ったかを特定し、所有境界で最小修正。src 変更が必要になった
  場合は spec の Out of Scope に反するため、まず監査結果を
  evidence と PR に記録して判断を Coordinator に委ねる。
- 外部 Gate（実機・実 plugin・GUI・Oracle）は `NotRun`/
  `Unsupported` として evidence に明記し、Fake/Fixture の成功を
  実機 Pass にしない。
