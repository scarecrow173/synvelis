# CB-08 Comparison Orchestrator external gates (Issue #285 / synvelis#285)

headless test の成功を実機・GUI・plugin・consumer・実信号の
Pass にしないための外部 Gate 判定記録。sv109/sv288 様式を
踏襲。

| Gate | 対象 case | 状態 | 理由・所有 |
| --- | --- | --- | --- |
| 実 signal render/probe（render→probe→PCM evidence 供給） | CB-CMP-03/04/05 signal 軸・CB-CMP-RCP-01 artifact 軸 | **NotRun** | 実 renderer/probe provider は本 slice に存在しない。`SignalComparisonProvider` seam のみ — 実装は Ruviform render/probe（#308/#309 系）・CB-10 surface 前提 |
| 同一 PCM zero-diff・gain/latency/PDC 分類の実測 | CB-CMP-03 | **NotRun** | 実 PCM fixture と alignment oracle が必要 — headless は `AlignmentPolicy` 宣言と `Unaligned` refusal のみ pin |
| 非整列/非決定 delta の試聴分類・loudness match 非破壊の実測 | CB-CMP-04 | **NotRun** | 試聴 A/B・level-match は interactive audition の責務。`MonitoringPolicy` は audition path 宣言 policy であり headless は比較 signal を変更しないことのみ契約化 |
| MIDI/arrangement tempo・length 変化の signal 整列 | CB-CMP-05 | **NotRun** | `AlignmentPolicy::Mapped` の実 time-mapping provider 前提 — headless は semantic 層の pin/diff のみ |
| 非 adopted candidate の連続 RT・Live/Offline resource policy・plugin failure fallback 実測 | CB-CMP-06 | **NotRun** | `RuntimePublish`/plugin host/RT queue の実挙動は外部 gate。headless は compare が read-only derived で RT を実行しないことのみ証明 |
| Verified Artifact vs Derived Probe/Preview の artifact 級判定 | CB-CMP-RCP-01 | **NotRun** | RCP render checkpoint artifact（#289〜#297・#296 RCP-07）の責務。本 slice は `EvidenceOrigin`（Pipeline/Derived/Mock）の区別と stale 分類のみ |
| 実 Plugin ABI・plugin state `Known` 検証 | signal gate の plugin 軸 | **NotRun** | `scarecrow173/ruviform#630`・synvelis#176 — headless は opaque state 非 `Known` → `PluginStateUnknown` 降格のみ |
| 実 GUI Difference Explorer・Audition A/B | CB-CMP-04/06 表示軸 | **NotRun** | synvelis#286（CB-09）・#287（CB-10）・`scarecrow173/ruviform#631` — headless Pass を GUI-L2 に換算しない |
| AudioLens/Ruviform consumer（f64・長尺・precision 非回帰） | CB-CMP-07 | **NotRun** | `scarecrow173/ruviform#294`/`#308`/`#309` — consumer resolved SHA tuple の regression は consumer 側 gate の責務 |
| macOS/Linux GUI-L2/EXT-L3・3OS L1 横断 | 全般 | **Deferred/NotRun** | `POLICIES.md` — GUI/device/plugin の使用要件が発生するまで Deferred。OS-L1 は CI 証拠で別管理 |
| CB-09 Difference Explorer（#286）・CB-10 actions（#287）・CB-11 cross-gate（#288）の consumer 接続 | 全 case の surface 軸 | **Deferred** | consumer Issue の責務 — 本 slice は `ComparisonSession`/`ComparisonResult` の非 persistent derived 契約を提供するのみ |

## 判定ルール

- headless テストの green は「headless 契約の Pass」であり、
  実機・実 plugin・GUI・consumer・実信号の Pass を意味しない。
- `NotRun` は「未実施」、`Deferred` は「使用要件が発生する
  まで保留」、`Unsupported` は「環境/能力非対応」、
  `Blocked` は「前提 Issue 未 merge」を意味し、推測で
  Pass にしない。
- `Required`/`Deferred`/`NotApplicable` は要件状態、
  `Pass`/`Fail`/`NotRun`/`Unsupported` は実行結果 — 両者を
  混同しない（`POLICIES.md`）。
- Issue #285 の close 判定は Coordinator/Owner — 本 slice
  merge は headless orchestration 契約の完了を意味し、
  実信号・試聴・外部 gate の完了を宣言しない。
