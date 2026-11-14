# PC-C6 acceptance ledger external gates (Issue #103 / synvelis#103)

headless Fixture/テストの成功を実機・GUI・plugin・consumer の
Pass にしないための外部 Gate 判定記録。Issue #103 の受入条件
「固定Caseごとに…未実行Case・Fake/Headlessを実VST3/GPUI/OS
成功に繰り上げない」「#114の実GPUI GUI製品Scenarioと#28
Ruviform継続追従・Ruviform #219/#294のAudioLens/Synvelis別
結果を同じCase/run IDで照合」に対応。

| Gate | 対象行/case | 状態 | 理由・Owner |
| --- | --- | --- | --- |
| 実 GPUI GUI 製品 Scenario（GUI-PC-01〜04・PC-C5-UI の実機部・PC-C1-R の gesture 実機部） | GUI-PC-01〜04・PC-C5-UI01〜07・PC-X2/X3 の GUI 側 | **NotRun** | Scenario Owner は synvelis#114（SV-GUI-05 real-OS case freeze は `evidence/sv114/` に merge 済み）。実 Window/Input/Focus/Pixel は GUI-L2。本 slice は Oracle 側証拠対応のみ記録 |
| 実 Plugin Dock / 実 VST3/CLAP/AU（PC-C4-PL の実機部・PC-X6 の実 plugin 部） | PC-C4-PL01〜08・PC-X6 | **NotRun** | in-process fake hosted plugin のみ。実 plugin capability/state/minframes は EXT-L3。Owner: synvelis#176（Native Dock）+ 実 plugin 環境 |
| 実 audio device / RT callback（録音・RT noalloc の実機部） | PC-C1-R01〜R08・PC-C3-TR08・PC-C4-PL05・PC-X4/X5 | **NotRun** | headless `GraphRuntime`/fake plugin のみ。実 device の clock/latency/underflow/xrun・実機録音 fidelity は未観測 |
| 実行/製品 Gate A/B/C 判定（PC-C6-COMP01〜03 の製品判定部） | PC-C6-COMP01・COMP02・COMP03 | **NotRun** | headless 構成/観測/推定分離・双方向探索の証拠は Pass-measured として記録するが、製品 Gate 判定は synvelis#23 の責務。Gate B/C 未受入でも Gate A は Signal Flow/AI/VCS 非依存で独立に評価可能な構成は維持 |
| 両 Consumer 横断判定（PC-C6-COMP04） | PC-C6-COMP04 | **NotRun** | Ruviform #219/#294（候補 tuple・AudioLens/Synvelis 別結果）+ synvelis#28（共通基盤追従）が Owner。Synvelis 側は `Cargo.lock` 単一 pin と `check-ruviform-rev` 整合のみ記録 |
| Ruviform Bridge Clock | PC-C3-TR 系の clock/epoch 実測部 | **external owner** | ruviform#307 が Owner。本 slice は headless の clock_epoch/transport_generation 分類のみ |
| 絶対性能閾値（REL07 大規模 graph・X5 recorded points perf・RT latency baseline） | PC-C2-REL07・PC-X5・PC-C4-PL05・PC-C3-TR08 | **NotRun** | Perf thresholds は実 Baselines を測定してから確定（Issue #103 証拠条件）。現状は alloc-free/sanity 上限のみ |
| 実 plugin state の Coordinated/Quiescent 実機挙動 | PC-X6 | **NotRun** | `tim04_plugin_state.rs` は in-process fake の quality 分類のみ。実 plugin の state capture 実挙動は EXT-L3 |
| 旧 Plugin RawID 再利用の実機誤配送 | PC-X6・PC-C4-PL07 | **NotRun** | headless は delivery log 上の誤配送 0 を検証済み。実 plugin instance 再利用時の誤配送は実機 gate |
| 録音/実 Device の Incomplete | PC-C1-R・PC-X4/X5 | **NotRun（隠さない）** | Gate B/C 未受入でも Gate A を独立に評価する構成だが、録音/実 Device の Incomplete は本 ledger に明示し隠さない（Issue #103 証拠条件） |
| macOS/Linux GUI-L2/EXT-L3 | 全般 | **Deferred** | `POLICIES.md` — GUI/device/plugin の使用要件が発生するまで Deferred。OS-L1（native build・headless test）は別途 CI 証拠 |
| merge 後対象 SHA の CI covering run | 全行 | **NotRun pending** | Daily/Manual Full CI の対象 SHA 証拠。Job green のみでは個別 Case の Pass を示さない（POLICIES.md §3）。Coordinator が merge 後に確認 |

## 判定ルール

- headless テストの green は「headless 契約の Pass」であり、
  実機・実 plugin・GUI・consumer の Pass を意味しない。
- `NotRun` は「未実施」、`Deferred` は「使用要件が発生する
  まで保留」、`Unsupported` は「環境/能力非対応」を意味し、
  推測で Pass にしない。
- `Required`/`Deferred`/`NotApplicable` は要件状態、
  `Pass`/`Fail`/`NotRun`/`Unsupported` は実行結果 — 両者を
  混同しない（`POLICIES.md` §3）。
- 同一 Case/run ID での #114 GUI Scenario・#28 追従・
  ruviform#219/#294 consumer 結果の照合は、各 Owner の証拠が
  揃った後に実施する（本 slice は対応表のみ）。
