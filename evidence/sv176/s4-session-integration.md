# S4 Headless Plugin Dock Session 連携 evidence（synvelis#176 / SV-GPUI-02）

Date: 2026-11-28
Branch: `feat/176-plugin-dock-session`
対象 Issue: `synvelis#176`（partial slice。Issue は close しない）
検証 worktree: `G:/orca/workspaces/synvelis/sv176-plugin-dock-session`
OS/toolchain: Windows 11 / MSYS2 (`MINGW64_NT-10.0-26300`), rustc 1.99.1, cargo 1.99.1
ruviform resolved SHA: `511e1a7944c35797942ba065c092b9263067c48a`
実装・検証 commit: `b58e38ce3166eb8f1a9ff69acb7078d9f0925674`（evidence 自身を含む最終 SHA は自己参照できないため PR 本文に記録）

## 目的と範囲

本 evidence は、headless PluginDock の製品状態を editor-session lifecycle・project boundary・PD-07 logical persistence に連携させる S4 slice を記録する。対象は行 2・行 3 の session structural gap と、行 4・行 6・行 9 に対する headless/branch evidence の追加である。実 OS window、実 child HWND、pixel/input/IME/popup、production L1 host wiring、Mixer 実操作経路、real scenario JSONL、Issue close は対象外である。行 1・行 7 の分類は変更しない。

## 実装・収束判断

- `PluginEditorSessions::tick_all` を registry 単位で 1 回評価し、panel 単位の session event drain と read model/notify 投影を `ui_pump` の 1 つの authoritative seam に接続した。`NativeShell` は op がない裸の `Tick` でも内部 `syn.window.lifecycle{event:"tick"}` を sealed app へ forwarding し、wall-clock deadline が frontend op 非存在時にも評価される。
- project new/open/reload の境界では、live session に host 生存時の graceful close を先に試みる。close 失敗時は、session が既に持つ `Failed` cause を優先し、失われている場合は adapter error を upstream と同じ cause 語彙へ写像して `Terminated` 系へ収束する。lease/slot は共有 `NativeSlotAuthority` 上で解放し、panel map と session registry を reset する。project 操作自体は boundary teardown の失敗で fail にせず、旧 project resource を新 session へ leak させない不変条件を優先する設計とした。
- PD-07 は `plugin-docks.json` sidecar に logical state のみを保存する。session id・slot id・lease・OS handle・window token は永続化しない。restore された panel は `Closed` 相当で、live editor session を偽装しない。unknown instance は panic/silent drop ではなく `unknown_instance` rejection として project open result に返す。

## 決定的テスト証拠

`crates/synvelis-app/tests/plugin_dock.rs` は 35 tests pass（canonical lane内）。追加固定した主な契約:

- open/hide/show の `Attached`/`Hidden`/`Shown` が exactly once・順序通り drain され、追加 pump で重複しない。
- manual clock の in-flight detach timeout が bare shell `Tick` 経由の pump で `Failed{Timeout}` に収束し、2 回目の tick で重複投影しない。
- `Degraded` は requested-mode `Attached` として記録されず、event vocabulary と read model mode が分離される。
- host 不在および adapter close refusal の両方で project boundary が panel/session/lease/slot を残さず収束する。
- project save→open で logical state（instance・geometry・visibility・placement）が復元され、restored session state は `closed`。window adoption 後の明示 open だけが editor を再 open する。
- sidecar JSON を recursive key scan し、`session`・`slot`・`lease`・`handle`・`token` key がないことを固定した。
- 既存の stale/revoked lease、`SessionBusy`、window-loss、2-window dock move、DSP/Project/RT 非干渉、op-level save/restore round-trip は弱めていない。

## real-ABI probe 拡張

`tests/plugin_dock_probe.rs` に `product_app_pump_projects_vst3_test_gain_refusal_once` を追加した。ruviform#630 製 real VST3 `test_gain` を `InProcessHost` で activate し、製品 `SynvelisApp`/`syn.plugin_dock.open` 経路で docked open を試行する。`test_gain` は editor を持たないため `Unsupported` refusal が発生し、authoritative `ui_pump` が `Failed{Unsupported}` を exactly once で notify/read model へ投影することを確認した。記録 artifact: `evidence/sv176/s4-legs/product-app-pump-test-gain-open.jsonl`。これは in-process real ABI/headless evidence であり、native dock・real OS window・child HWND evidence ではない。

## Canonical validation lanes（Windows 11、canonical = mise）

| lane | exit | verdict / 記録 |
| --- | ---: | --- |
| `mise run fmt-check` | 1 | **Fail（環境起因）** — `cargo fmt --all -- --check` が Windows command-line/path 長制限で `os error 206` を返し rustfmt を起動できない。補助証拠（canonical = mise ではない）として `cargo fmt -p synvelis-app -- --check` は exit 0 |
| `mise run check synvelis-app` | 0 | Pass |
| `mise run lint synvelis-app` | 0 | Pass（`-D warnings`） |
| `mise run test synvelis-app` | 0 | Pass。`tests/plugin_dock.rs` 35 tests・`tests/plugin_dock_probe.rs` 8 tests を含む package test 全件 pass |
| `mise run check-app-native` | 0 | Pass |
| `mise run lint-app-native` | 0 | Pass（`-D warnings`） |
| `mise run check-app-gpui` | 0 | Pass |
| `mise run lint-app-gpui` | 0 | Pass（`-D warnings`） |
| `mise run test-app-gpui` | 0 | Pass。`plugin_dock_contract_gpui` を含む gpui-test target 全件 pass |
| `mise run check-headless-deps` | 0 | Pass |
| `mise run check-ruviform-rev` | 1 | **Fail（scope 外の stale vendor）** — vendor `20a24a66ee90f4d462224391e78c10c93f4f31ba` vs lock `511e1a7944c35797942ba065c092b9263067c48a`。本 slice は vendor/pin 非接触のため `sync-ui-deps` は実行していない |

補助証拠（canonical = mise ではない）: `cargo test --locked -p synvelis-app --test plugin_dock --test plugin_dock_probe --test plugin_dock_contract --test plugin_dock_surface` は pass。probe artifact 生成のための単体 `cargo test` も exit 0。直接 cargo の結果は上表と区別する。

## Branch CI

- branch head SHA と branch CI Run URL/確認時点状態は push 後に PR 本文へ記録する。CI 非ブロッキング方針のため、未完了なら未完了のまま記録し、Pass に読み替えない。手動 dispatch は起動しない。
- ローカル検証は変更範囲の検証であり、Issue 全体の統合受入ではない。統合検証は merge 後対象 SHA の CI 証拠として Coordinator が確認する。

## Non-claims / 残件

- real OS window、real native plugin dock、real child HWND/container、pixel oracle、input/IME/popup、PD-01〜07 Real OS/Real Plugin verdict を主張しない。
- headless session integration・logical persistence・in-process real ABI の結果を Native Dock Pass や real editor re-attach evidence に昇格しない。
- production L1/L0 host wiring（`synvelis#101` 領域）、`synvelis-native` composition への注入配線、Mixer 実操作からの open 経路は残件。
- `ruviform#631` real scenario JSONL/real consumer projection、行 1/7 の Required real-window gate、PD-07 の real reload 実走は残件。
- docs index（`docs/**/README.md`）は編集していない。Coordinator が統合時に整備する。
