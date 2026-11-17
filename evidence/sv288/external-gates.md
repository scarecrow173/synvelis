# CB-11 Creative Branch cross-gate external gates (Issue #288 / synvelis#288)

headless fixture/test の成功を実機・GUI・plugin・consumer の
Pass にしないための外部 Gate 判定記録。sv109 様式を踏襲。

| Gate | 対象 case | 状態 | 理由・所有 |
| --- | --- | --- | --- |
| 実 signal/clock（fixed signal・virtual OS endpoint） | CB-E2E-01/02/04 PCM 軸 | **NotRun** | `scarecrow173/ruviform#628` — **closed**（deterministic test runtime `ruviform-audio-test` + scripted backend→production 配線は merge 済み・synvelis-recording が dev-dep で消費済み）。ただし CB PCM 軸（T2 lane）への接続可否は未評価 — render 層が sv#518 占有のため本 slice 対象外。実 signal 接続の評価は残課題 |
| 実 Plugin ABI（本番 Host 実 ABI Test Plugin） | CB-E2E-02/06 | **NotRun** | `scarecrow173/ruviform#630` — **OPEN・段階 merge 中**。synvelis#176 — headless は `PluginRef`/`ProcessorBinding` の model-level fixture のみ |
| 実 GUI runner / native window（Difference Explorer・plugin dock） | CB-E2E-06 | **NotRun** | `scarecrow173/ruviform#631` — **OPEN・段階 merge 中**。synvelis#114（GPUI Signal Flow Editor）・synvelis#176（Native Dock）。headless Pass を GUI-L2 に換算しない |
| Audio/MIDI OS 仮想 Endpoint | CB-E2E-01/02 device 軸 | **NotRun** | `scarecrow173/ruviform#629` — 必要時のみの gate |
| AudioLens/Ruviform consumer（f64・長尺・CLI・解析 oracle） | CB-E2E-08・T4 lane | **NotRun** | `scarecrow173/ruviform#294`/`#308`/`#309` — consumer resolved SHA tuple の precision/RT/perf regression は consumer 側 gate の責務 |
| 実 RT publish/noalloc/PDC/last-good/queue-full | CB-E2E-04/05 RT 軸 | **NotRun** | `RuntimePublish` は `NoopRuntimePublish`/`FakeRuntimePublish` の receipt 分離まで。実 RT queue の振る舞いは `scarecrow173/ruviform#628`・#296 系 |
| macOS/Linux GUI-L2/EXT-L3 | 全般 | **Deferred** | `POLICIES.md` — GUI/device/plugin の使用要件が発生するまで Deferred。OS-L1（native build・headless test・required features）は CI 証拠で別管理 |
| 3OS L1 横断 | 全般 | **NotRun** | Daily/手動 Full CI の責務。本 slice は Windows のみ実測（`verification.md`） |
| CB-07 Arrangement adapter（#284） | CB-E2E-03 | **解消（slice 2）** | synvelis#284 closed — `ArrangementAdapter` merge 済み（PR #482 `16c81446`・#534 `bfde1430`）。`cb11_e2e_03_*` で project 軸 Pass。gateway `VariantPorts::standard` 未配線は src follow-up（coverage.md「判明した gap」） |
| CB-08 Compare Orchestrator（#285） | CB-E2E-04 orchestration 軸 | **解消（slice 2）** | synvelis#285 closed — `ComparisonOrchestrator` merge 済み（PR #484 `409c34f5`）。`cb11_e2e_04_orchestrated_compare_*` で headless 契約 Pass。実 RT/plugin signal は引き続き NotRun |
| CB-09 GPUI Difference Explorer（#286） | CB-E2E-06 | **Blocked** | synvelis#286 未 merge — GUI 実装自体が前提 |
| CB-10 Gateway Actions（#287） | CB-E2E-07 AI/OFF 軸・CB-X-RCP-01 action 経路 | **解消（slice 2・headless 半分）** | synvelis#287 closed — gateway typed contract merge 済み（PR #483 `696efa73`）。`cb11_ai_vcs_off.rs` で grantless fail-closed・VCS-off lifecycle Pass。実 AI 統合・実 VCS backend は外部 gate |
| RCP-07 横断（recipe cache/invalidation・RCP-GATE Case 紐付け） | CB-X-RCP-01 | **解消（slice 2・紐付け部）** | synvelis#296 headless slice merge 済み（PR #477 `f445563b`・issue は OPEN）。RCP-GATE-01..07 case-ID への mapping を `coverage.md` に記録。recipe cache/invalidation 本体・RCP-GATE-05 残部は #296/#294 側所有 |
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
