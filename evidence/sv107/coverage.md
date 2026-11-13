# SF-PROBE-01..08 coverage map (Issue #107 / synvelis#107)

Headless project/player 層の受入写像。実装本体は SF-04 slice1..10
（全て merge 済み）。本タスクの新規証拠は
`crates/synvelis-player/tests/sf04_probe_acceptance.rs` の 2 テスト。

凡例: ✅ headless 証拠あり / ◐ 部分的（残件は後続 Issue）/ ⛔ 外部 Gate
（NotRun/Unsupported、`external-gates.md` 参照）

| AC | 内容 | 既存証拠 | 本タスク追加 | 判定 |
| --- | --- | --- | --- | --- |
| SF-PROBE-01 | Semantic Point→実 Node/Port/Slot、Unavailable/Unsupported、偽 PCM なし | `synvelis-project` `probe_resolution_*`・`map_resolution_states_*`（非 Audio port 拒否・Unavailable/Unsupported/NotLowered）、`sf04_probe_adapter`（strip/bus/processor/send 解決・BoundActive） | entity 消失→`Unresolved` unbind・re-attach fresh bind | ✅ |
| SF-PROBE-02 | Probe Off/Monitor/Capture が Bypass/Connection/DSP/perf/Raw Recording を変更しない | `sf04_probe_adapter::sf04_probe_off_is_true_observation_silence`（Off が観測を出さない）。bypass/connection 非変更は lowering 契約（#18）側 | — | ✅（headless 範囲） |
| SF-PROBE-03 | Requested→Queued→RT Installed、stale/late ack 拒否、SourceMap 失効で Unknown/Unbound | `sf04_probe_adapter` lifecycle・stale-generation・moved-point、`sf04_s2` trace bridge stale/unknown 拒否、`sf04_s3`/`sf04_s9` epoch cut | entity 消失→`Unresolved`・再 request 冪等失敗 | ✅ |
| SF-PROBE-04 | Graph swap で PCM Capture 分断・再自動 Capture なし、#308 fault injection | `sf04_probe_adapter::sf04_probe_generation_swap_cuts_capture`、`sf04_s8`（QueueFull/LostBlocks 等 fault）。#308 CLOSED → 契約証拠は有効 | — | ✅ headless / ⛔ 実機 Gate NotRun |
| SF-PROBE-05 | max_frames、ChunkPool/out channel/telemetry 満杯、LostBlocks/LostRecords/Unobserved、Silence≠Missing | `sf04_s8` 全般 | — | ✅ |
| SF-PROBE-06 | 同一 Semantic Point の複数 Window/Inspector で Broker Capture 重複なし、ownership | slice5（synvelis-workspace subscriber lease、merge 済み） | `sf04_probe_acceptance` per-key 冪等・session 境界（player 層側） | ✅（session 層は slice5 で証明済み） |
| SF-PROBE-07 | 複数 Probe の Clock Domain/Epoch/Latency/Format/Gaps 比較、Seek/Loop/Restart の Unknown/Incomparable | `sf04_s7` compare（Aligned/Conditional/Incomparable・epoch/gap/format 分離）、`sf04_s3`/`sf04_s9` epoch/seek discontinuity。#309 CLOSED → 契約証拠は有効 | — | ✅ headless / ⛔ 実測 Gate NotRun |
| SF-PROBE-08 | Diagnostics first observed drop vs proven root cause、状態分離、Accessibility | `sf04_s8` observation states（Plugin unavailable/Probe Off/PCM silence/No Data の状態分離） | — | ◐ headless は状態語彙の分離まで。原因候補解釈・UI 分離・Accessibility は #108/#109 |

## 新規テスト詳細

### `sf04_probe_removed_entity_unbinds_and_reattach_binds_fresh`

- SF-PROBE-01/03: bound probe の対象 entity（Processor）が
  `RemoveProcessor` で消失 → republish で `Unresolved`・
  `runtime_probe_id = None`・capture segment なし。同 key 再
  request でも復活しない。新 processor id で re-attach → gen3
  で fresh `BoundActive`・新 `RuntimeProbeId`、旧 binding は
  `Unresolved` のまま（old slot reuse 誤観測なし）。

### `sf04_probe_same_key_request_is_idempotent_single_install`

- SF-PROBE-06（player 層側）: 同一 `ProbeSubscriptionKey` の
  重複 request は冪等（`runtime_probe_id`・`requested_mode`
  不変）。`probe_capture` 中の重複 request + 再 `probe_capture`
  は `AlreadyCapturing` で fail-closed。`session_id` 違いの key
  は独立 subscription として install — cross-window lease 集約は
  slice5 の契約。

## Unknown/Unobserved ≠ Silence

`sf04_s8`・`sf04_s7` の既存テストで `Unknown`/`Unobserved`/
`NoData`/`Silence` が別状態として保持されることを確認済み。
本タスクでも新規テストは観測状態を Silence 値に置換しない。
