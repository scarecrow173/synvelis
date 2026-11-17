# sv530 verification — 検証実測記録

Issue: https://github.com/scarecrow173/synvelis/issues/530
Date: 2026-11-19
OS: Windows (worktree `G:\orca\workspaces\synvelis\sv530-plugin-state-decl`)
Toolchain: mise lock（rustc 1.99.1 系 — `mise run` 経由の cargo）

## 実行結果

| コマンド | exit | 結果 | 備考 |
| --- | --- | --- | --- |
| `mise run fmt-check` | 0 | Pass | 初回は新規 test code の fmt 差分で fail → `cargo fmt -p synvelis-player -p synvelis-workspace` で整形後に再実行し pass（os error 206 flake は未再現） |
| `mise run check synvelis-player` | 0 | Pass | `--all-targets`（test target 含む） |
| `mise run lint synvelis-player` | 0 | Pass | 初回 `clippy::assert_is_empty` で fail → `assert_eq!` 規約に修正後 pass。`-D warnings` で warning なし |
| `mise run test synvelis-player` | 0 | Pass | `rcp_04` 30/30 pass（新規 oracle 4 件含む）・全 suite 0 failed |
| `mise run check synvelis-workspace` | 0 | Pass | `--all-targets` |
| `mise run lint synvelis-workspace` | 0 | Pass | `-D warnings` で warning なし |
| `mise run test synvelis-workspace` | 0 | Pass | `rcp_selection_integration` 11/11 pass（新規 stateless case 含む）・全 suite 0 failed |

## 補助証拠（canonical = mise）

| コマンド | exit | 結果 | 備考 |
| --- | --- | --- | --- |
| `mise run check synvelis-render` | 0 | Pass | `--all-targets` — `rcp07_gate.rs` の `PluginStateDecl` literal が無変更で compile されることを実証（lease 外波及なしの確認） |

## NotRun / Unsupported

| lane | 状態 | 理由 |
| --- | --- | --- |
| T3/T4 実 device/plugin・実 OS・RT/PCM 実測 | **NotRun** | headless 契約変更のみ。gate: #109・#296・`scarecrow173/ruviform#630`・`scarecrow173/ruviform#631`。headless Pass を実機 Pass に昇格しない |
| `mise run check/lint/test synvelis-render`（lint/test） | **NotRun** | `rcp07_gate.rs` は無変更。`check`（all-targets compile）のみ補助実行。変更比例の原則により lint/test は対象外 |
| `check-full`/`lint-full`/`test-full`/`build-full`/`verify-full`・`*-msrv`・3OS 横断 | **NotRun** | 本 slice の必須検証に含めない（Daily Full CI・別 gate 委譲） |
| 統合検証（merge 後 main SHA の必須 CI） | **Pending** | ローカル検証は変更範囲の検証。統合受入は merge 後 CI 証拠（Coordinator 確認） |

## 新規 oracle 一覧

- `crates/synvelis-player/tests/rcp_04.rs`
  - `ex03_stateless_profile_is_typed_and_distinct_from_uncaptured`
  - `ex03_stateless_env_decl_verdict_matrix`
  - `ex03_stateless_plugin_selects_live`
  - `ex03_stateless_decl_env_drift_and_uncaptured_stay_blocked`
- `crates/synvelis-workspace/tests/rcp_selection_integration.rs`
  - `rcp_ex01_stateless_plugin_evidence_is_live`
    （stateless evidence → Exact/Live/Published + 未 capture →
    StateUnsupported/Blocked の 2 部構成）
