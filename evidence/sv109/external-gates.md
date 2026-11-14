# SF-06 cross-gate external gates (Issue #109 / synvelis#109)

headless Fixture/テストの成功を実機・GUI・plugin・consumer の
Pass にしないための外部 Gate 判定記録。

| Gate | 対象 case | 状態 | 理由 |
| --- | --- | --- | --- |
| 実 OS GUI（Windows GUI-L2: window/input/focus/display/pixel/DPI/IME） | SF-UX-01..08・SF-XG-01..05 | **NotRun** | #114（GPUI Signal Flow Editor）・#176（Native Dock）の実装・受入が必要。本 slice は `synvelis-app`/`synvelis-native` を編集しない |
| 実 plugin（VST3/AU/CLAP editor・state・PDC） | SF-ACT-06/07・SF-PROBE-01/08・SF-COMPAT-04 | **NotRun** | `StaticProcessorHost`/`GainProcessor` の headless Fixture のみ。実 plugin の dynamic port・state・latency は EXT-L3 の別 gate |
| 実 audio device / RT callback | SF-ACT-04/06・SF-PROBE-09・SF-COMPAT-04 | **NotRun** | headless `RoutingEngine` + Ruviform deck のみ。実 device の clock/latency/underflow/xrun は未観測 |
| macOS/Linux GUI-L2/EXT-L3 | 全般 | **Deferred** | `POLICIES.md` — GUI/device/plugin の使用要件が発生するまで Deferred。OS-L1（native build・headless test）は別途 CI 証拠 |
| AudioLens consumer（Synvelis #219/#294 提出分） | SF-COMPAT-05・T4 lane | **NotRun** | consumer resolved SHA tuple・precision/RT/perf regression は Gate 側の責務。本 slice は対応表記録のみ |
| Ruviform #308（probe capture/rebind 実機 fault） | SF-PROBE-04・SF-XG-03 | **NotRun** | headless は `sf04_s8`/`sf04_s9` の契約レベル fault のみ。実機 RT 負荷下の挙動は未観測 |
| Ruviform #309（frame/clock/epoch/latency 実測 alignment） | SF-PROBE-07 | **NotRun** | `sf04_s7` compare は契約レベル。実測 precision gate は別 gate |
| EX-001〜020（#18）・Gate A/B/C | SF-COMPAT-01..03・SF06-RCP-01 | **NotRun** | #18 の evidence を link 参照。Gate 判定は全 lane の証拠が揃ってから実施 |
| SF-XR-02..05（RCD/TIM cross-product） | SF-XR-02..05 | **NotRun** | #119/#126/#127/#128 依存 — 本 slice 範囲外 |
| SF-XG-01..05（GUI cross-product） | SF-XG-01..05 | **NotRun** | GUI 操作→Project/PCM の cross-product — #114 系 |

## 判定ルール

- headless テストの green は「headless 契約の Pass」であり、
  実機・実 plugin・GUI・consumer の Pass を意味しない。
- `NotRun` は「未実施」、`Deferred` は「使用要件が発生する
  まで保留」、`Unsupported` は「環境/能力非対応」を意味し、
  推測で Pass にしない。
- `Required`/`Deferred`/`NotApplicable` は要件状態、
  `Pass`/`Fail`/`NotRun`/`Unsupported` は実行結果 — 両者を
  混同しない（`POLICIES.md`）。
