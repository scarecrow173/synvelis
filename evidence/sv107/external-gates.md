# SF-PROBE external gates (Issue #107 / synvelis#107)

headless Fixture/テストの成功を実機 Pass にしないための外部 Gate
判定記録。

| Gate | 対象 AC | 状態 | 理由 |
| --- | --- | --- | --- |
| 実機 audio device での Probe capture/monitor | SF-PROBE-04/07 | **NotRun** | 実デバイス環境が必要。headless `RoutingEngine` + Ruviform deck の Fixture で契約レベルは検証済みだが、実機の clock/latency/underflow 挙動は未観測 |
| 実 plugin (VST3/AU/CLAP) の port 再構成・probe | SF-PROBE-01/03/08 | **NotRun** | `StaticProcessorHost`/`GainProcessor` の Fixture のみ。実 plugin の dynamic port 構成・scan 失敗時の挙動は #114/#117 系の実 OS/plugin 検証が必要 |
| GUI 表示（probe 状態の UI 分離・Accessibility） | SF-PROBE-08 | **Unsupported** | #108（Signal Flow UI integration）の範囲。本タスクは `synvelis-app`/`synvelis-native` を編集しない |
| Domain Oracle（first observed drop vs proven root cause の判定） | SF-PROBE-08 | **Unsupported** | #109（Probe/RT/Consumer oracle）の範囲。headless は状態語彙の分離まで |
| Ruviform #308 QueueFull/ClearAll＋Bind 途中失敗 | SF-PROBE-04 | **契約証拠のみ** | #308 CLOSED。`sf04_s8` の fault injection テストが契約レベルを被覆。実機での RT 負荷下の挙動は NotRun |
| Ruviform #309 実測 alignment gate | SF-PROBE-07 | **契約証拠のみ** | #309 CLOSED。`sf04_s7` compare が `align_observation_points` への委譲と semantic 写像を被覆。実測 latency/clock の精度 gate は NotRun |
| OS-L1 native validation（Windows/macOS/Linux） | 全般 | **NotRun** | 本タスクは headless のみ。OS 別の native 挙動は #114 系の責務 |

## 判定ルール

- headless テストの green は「headless 契約の Pass」であり、
  実機・実 plugin・GUI・Oracle の Pass を意味しない。
- `NotRun`/`Unsupported` は「未実施」または「本タスク範囲外」
  を意味し、推測で Pass にしない。
