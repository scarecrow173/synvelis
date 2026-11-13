# SV23 Execution Plan Entry/Gate Ledger

Issue #23 の正規入口（`mise.toml` task / CI lane）と対象
OS・Gate/Feature・対象 SHA・結果状態の対応台帳。
`docs/designs/execution-plan-design.md`「正規入口 →
OS/Gate/Feature 対応」の詳細版。未実施は `NotRun`/
`Unsupported`/`Deferred` として記録し、`Pass` は実測
成功のみ。

## 判定語彙

- OS Level: `OS-L1` / `GUI-L2` / `EXT-L3`
- 要求状態: `Required` / `Deferred` / `NotApplicable`
- 実行結果: `Pass` / `Fail` / `NotRun` / `Unsupported`
- 計画参照: `NotRequired`

## mise task → OS/Gate/Feature 対応

| 入口 | 範囲 | OS Level | Gate/Feature | OS | 要求状態 | 結果状態 | 対象 SHA | 証拠 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `check-ruviform-rev` | lock/vendor/manifest SHA 一致 preflight | OS-L1 | 全 lane 共通 | Win/mac/Linux | Required | NotRun（本 slice では未実行。`Cargo.lock`/vendor 変更なしのため preflight 対象外。Daily/手動 Full CI に委譲） | — | — |
| `check-headless-deps` | headless crate に GUI/GPU/WebView 非依存 | OS-L1 | 境界不変条件 | Win/mac/Linux | Required | NotRun（本 slice 対象外・Daily/手動 Full CI 委譲） | — | — |
| `check <pkg>`/`lint <pkg>`/`test <pkg>`/`build <pkg>` | package 単位型/lint/test/build（package 引数必須） | OS-L1 | 変更 crate と直接影響範囲 | Win/mac/Linux | Required | NotRun（docs-only 変更のため対象外） | — | — |
| `test-rt` | `rt_` contract（noalloc/nonblock） | OS-L1 | RT 安全性 | Win/mac/Linux | Required | NotRun（本 slice 対象外） | — | — |
| `gw10`/`gw10-report` | GW-10 harness + gate report | OS-L1 | AI-GATE | Win/mac/Linux | Required | NotRun（本 slice 対象外） | — | — |
| `check-app-native`/`lint-app-native`/`build-app-native` | `native` feature（GPUI driver 組立 compile） | OS-L1（compile） | GUI-L2-WIN の compile 側 | Win/mac/Linux | Required | NotRun（本 slice 対象外） | — | — |
| `run-app-native` | 実窓起動（手動） | GUI-L2（手動実機） | GUI-L2-WIN 補助 | Windows | Required（提供 GUI 機能） | NotRun（手動・CI 対象外） | — | — |
| `check-app-gpui`/`lint-app-gpui`/`test-app-gpui` | `gpui-test` feature（GPUI-simulated scenario） | OS-L1（simulated） | #114 L1 契約比較。実 Window/pixel ではない | Win/mac/Linux | Required | NotRun（本 slice 対象外） | — | — |
| `check-ui`/`test-ui`/`build-ui`/`install-ui`/`setup-ui`/`sync-ui-deps` | ui/ frontend（typecheck/vitest/build/vendor 同期） | OS-L1 | 旧 WebView lane・移行検証 | Win/mac/Linux | Required | NotRun（本 slice 対象外） | — | — |
| `check-app-webview`/`lint-app-webview`/`test-app-webview`/`dev-ui`/`build-app` | `webview` feature（旧 SolidJS/WebView lane） | OS-L1 + 実窓は手動 | 歴史的/移行 lane | Win/mac/Linux | Required（移行期間） | NotRun（本 slice 対象外） | — | — |
| `check-full`/`lint-full`/`test-full`/`build-full`/`verify-full` | workspace 標準 feature（現 OS） | OS-L1 | Daily/手動 Full CI と同一範囲 | Win/mac/Linux | Required | NotRun（手動 Full CI 委譲・本 slice 対象外） | — | — |
| `test-recording-real-input`/`lint-recording-real-input` | opt-in 実 audio device | EXT-L3（手動・CI 対象外） | EXT-L3-WIN 録音 | Windows | Required（提供 Device のみ） | NotRun（手動 opt-in） | — | — |
| `test-recording-real-fs` | opt-in 実 FS durability probe | EXT-L3（手動・CI 対象外） | RCD-02 実 FS | Windows | Required（提供範囲） | NotRun（手動 opt-in） | — | — |
| `ci-summary-gate`/`test-ci-summary`/`test-ci-tracker-entry` | CI 集約 gate 契約 | CI infra | #166/#167 | CI | Required | NotRun（本 slice 対象外） | — | — |
| `lint-actions` | workflow lint | CI infra | workflow 契約 | CI | Required | NotRun（本 slice 対象外・workflow 変更なし） | — | — |
| `test-msrv` | MSRV toolchain | OS-L1（別 toolchain） | MSRV lane | Win/mac/Linux | Required | NotRun（ユーザー指定により本 slice では実行しない） | — | — |

## capability 分離の注意

- `gpui-test`/`webview`/`test-app-*` の Pass は
  GPUI-simulated/Browser 相当であり、GUI-L2（実
  Window/実入力/実 pixel）の Pass を意味しない。
- `run-app-native` の手動起動確認は GUI-L2 の一部に
  過ぎず、#114 の製品 E2E 受入を代替しない。
- `test-recording-real-*` は opt-in 手動であり CI 必須
  lane ではない。実施記録がない限り EXT-L3 は
  `NotRun`。
- Job 定義・Skip/ignored・Feature 無効・環境不在は
  `NotRun` であり `Pass`/`Unsupported` ではない
  （POLICIES.md §3）。

## OS 別要求状態

| OS | OS-L1 | GUI-L2 | EXT-L3 |
| --- | --- | --- | --- |
| Windows | Required | 提供・利用 GUI 機能のみ Required（#112/#114/#176/#177 系） | 提供・利用 Device/Plugin のみ Required（#115/#117/#126/#127/#176 + Ruviform#251/#253/#388/#389/#532） |
| macOS | Required | Deferred（利用要件発生まで） | Deferred（利用要件発生まで） |
| Linux | Required | Deferred（利用要件発生まで） | Deferred（利用要件発生まで） |

## 候補 SHA tuple 追跡欄

| 追跡対象 | synvelis_sha | ruviform_resolved_sha | os | feature_set | oracle_set | result_states |
| --- | --- | --- | --- | --- | --- | --- |
| 本 slice（docs） | branch HEAD（commit 後に確定） | `Cargo.lock` pin（変更なし） | Windows（作業環境） | docs/markdown のみ | doc_status/audit/fmt | verification.md 参照 |
| GPUI 採用（RF#262→#419） | — | 未確定 | Win/mac/Linux | Window/Dock/Plugin Editor | Ruviform 側実証 | NotRun |
| Consumer 集約（RF#294） | 未確定 | 未確定 | Win/mac/Linux | PCM/Project + f64/CLI/RT | SV#28・AL#127 | NotRun |
| 製品 Domain Oracle（SV#114/#109/#103） | 未確定 | `Cargo.lock` pin | Windows（GUI-L2）/全 OS（L1） | Scenario/Case ID | 独立 Oracle | NotRun |

## CI lane（Daily/手動 Full）

| Lane | 対象 | 結果状態 | 備考 |
| --- | --- | --- | --- |
| Daily Full CI（変更検知） | merge 後 main SHA | 運用中（#83 closed） | PR 単位ではなく merge 後 SHA が統合検証の証拠 |
| 手動 Full CI | 任意 SHA | 運用中 | 必要時に実行 |
| 新規 Workflow | — | NotApplicable | 増設禁止（EP-R-09） |
