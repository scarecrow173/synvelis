# CB-11 Creative Branch cross-gate external gates (Issue #288 / synvelis#288)

headless fixture/test の成功を実機・GUI・plugin・consumer の
Pass にしないための外部 Gate 判定記録。sv109 様式を踏襲。

| Gate | 対象 case | 状態 | 理由・所有 |
| --- | --- | --- | --- |
| 実 signal/clock（fixed signal・virtual OS endpoint） | CB-E2E-01/02/04 PCM 軸 | **NotRun** | `scarecrow173/ruviform#628` — headless render/probe 経路に実 signal を接続する基盤が未 merge。本 slice は `synvelis-player`/`synvelis-render` を編集しない |
| 実 Plugin ABI（本番 Host 実 ABI Test Plugin） | CB-E2E-02/06 | **NotRun** | `scarecrow173/ruviform#630`・synvelis#176 — headless は `PluginRef`/`ProcessorBinding` の model-level fixture のみ |
| 実 GUI runner / native window（Difference Explorer・plugin dock） | CB-E2E-06 | **NotRun** | `scarecrow173/ruviform#631`・synvelis#114（GPUI Signal Flow Editor）・synvelis#176（Native Dock）。headless Pass を GUI-L2 に換算しない |
| Audio/MIDI OS 仮想 Endpoint | CB-E2E-01/02 device 軸 | **NotRun** | `scarecrow173/ruviform#629` — 必要時のみの gate |
| AudioLens/Ruviform consumer（f64・長尺・CLI・解析 oracle） | CB-E2E-08・T4 lane | **NotRun** | `scarecrow173/ruviform#294`/`#308`/`#309` — consumer resolved SHA tuple の precision/RT/perf regression は consumer 側 gate の責務 |
| 実 RT publish/noalloc/PDC/last-good/queue-full | CB-E2E-04/05 RT 軸 | **NotRun** | `RuntimePublish` は `NoopRuntimePublish`/`FakeRuntimePublish` の receipt 分離まで。実 RT queue の振る舞いは `scarecrow173/ruviform#628`・#296 系 |
| macOS/Linux GUI-L2/EXT-L3 | 全般 | **Deferred** | `POLICIES.md` — GUI/device/plugin の使用要件が発生するまで Deferred。OS-L1（native build・headless test・required features）は CI 証拠で別管理 |
| 3OS L1 横断 | 全般 | **NotRun** | Daily/手動 Full CI の責務。本 slice は Windows のみ実測（`verification.md`） |
| CB-07 Arrangement adapter（#284） | CB-E2E-03 | **Blocked** | synvelis#284 未 merge — `VariantScope::Arrangement` 型のみ存在。Composite CAS fixture は前提 merge 後 |
| CB-08 Compare Orchestrator（#285） | CB-E2E-04 orchestration 軸 | **Blocked** | synvelis#285 未 merge — context pin・stale derived・cache の orchestration は前提 merge 後 |
| CB-09 GPUI Difference Explorer（#286） | CB-E2E-06 | **Blocked** | synvelis#286 未 merge — GUI 実装自体が前提 |
| CB-10 Gateway Actions（#287） | CB-E2E-07 AI/OFF 軸・CB-X-RCP-01 action 経路 | **Blocked** | synvelis#287 未 merge — command/AI gateway 統合が前提 |
| RCP-07 横断（recipe cache/invalidation・RCP-GATE Case 紐付け） | CB-X-RCP-01 | **Blocked** | synvelis#296 — merged 済み RCP-01..04 artifact と CB scope の cross-gate 紐付けは前提 merge 後。本 slice は render-pin identity 層のみ pin |
| 既存 DAW Gate A 実機非回帰（#23/#114 系 evidence link） | CB-E2E-07/09 | **NotRun** | 実機 record/playback/mixer/save/export の証拠は #23/#114 側 gate。本 slice は headless suite green と「新 CI なし」のみ記録 |

## 判定ルール

- headless テストの green は「headless 契約の Pass」であり、
  実機・実 plugin・GUI・consumer の Pass を意味しない。
- `NotRun` は「未実施」、`Deferred` は「使用要件が発生する
  まで保留」、`Unsupported` は「環境/能力非対応」、
  `Blocked` は「前提 Issue 未 merge」を意味し、推測で
  Pass にしない。
- `Required`/`Deferred`/`NotApplicable` は要件状態、
  `Pass`/`Fail`/`NotRun`/`Unsupported` は実行結果 — 両者を
  混同しない（`POLICIES.md`）。
- Issue #288 は長期 tracker であり、本 slice merge は Issue
  全体の完了を宣言しない。close 判定は Coordinator/Owner。
