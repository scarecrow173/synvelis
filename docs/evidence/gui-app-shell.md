# GUI アプリシェル — 実行証拠

この文書は、Synvelis の GUI アプリシェル構築に関わる各 slice の実行証拠を記録する。検証は headless・fake・GPUI-simulated・実 Windows 実機の各 lane に分かれ、実 OS/実 GUI/実 Plugin レーンは別 gate（synvelis#114・#176）で未実行のものが多い。headless/fake の Pass は実機証拠へ昇格しない方針で記録し、未実施・NotRun・Deferred は実施済みと誤読されないよう明示する。

## SV-GUI-01 デスクトップ bootstrap（synvelis#110）

実 Windows で `synvelis.exe` が `Synvelis` 窓を開き、WebView2 が frontend bundle を描画し、`asset bundle ok (frontend @synvelis/ui@0.1.0+7ac830b85129)` を通過、窓 close で clean exit した。

検証 lane は fmt-check/check/lint/test（70 suites 0 failed）/build/check-ui/test-ui（16 件）/build-ui/check-ruviform-rev（`073a3ff6`、22 lockfile sources、REV match）/check-headless-deps/check-app-webview/lint-app-webview/test-app-webview（service_ops 12 + asset_check 10）/prek で、全て pass である。

macOS/Linux 実 GUI・release embed lane・`rvf://data` pull end-to-end は未検証として明示されている（synvelis#110）。

## SV-GUI-02 view model・typed IPC（synvelis#111）

Rust 側 `ui_ops` 10 件を含む workspace 全 suite pass、vitest 37 件（binding 9 + conformance 12 等）である。

lifecycle 是正では製品コード未変更で console 5 件 + model 3 件が assertion で失敗（RED）→ 修復後 focused 17/17・全 UI 45 件 GREEN を確認した。

実 Chromium 150 + fake IPC で live route 数 receipt=1/projection=4/viz.event=1/viz.frame=1、unmount 後 0 を計測した。fake IPC であり実 Rust IPC/native WebView の受入とは混同しない。

RF #376 取込時は旧 SDK `8fb665f` で新テスト 3 件 RED（epoch=0 dispatch）→ 修復 SDK `513b29b` で 23/23 focused・全 UI 51 件 GREEN。RF #383 取込時は旧 SDK `513b29b` + head `2361694` で新 suite 3 件 RED（wire ctx.generation=1 vs backend live=2）→ `49de02c` + consumer 修正で focused 22/22・全 UI 56 件 GREEN を確認した。

全 mise lane（fmt-check/check/lint/test/check-ui/test-ui/build-ui/check-ruviform-rev/check-headless-deps/gw10/check-app-webview/lint-app-webview/test-app-webview/test-msrv）exit 0 である。実 OS WebView GUI・実 Rust IPC e2e・native GPU・実 plugin/実 DSP・実 multi window は別 lane の NotRun で synvelis#112 へ継続した（synvelis#111）。

## SV-48 viz headless contract（synvelis#48）

`synvelis-workspace` に `viz_ui` を新設し `tests/sv48_viz.rs` に 36 受入テストを追加した。対象は 3 view 共有・bounded query/LRU・gap/partial coverage・stale revision 拒否・budget 超過・notify resubscribe・seq gap/dropped/stale・awaiting-data vs source error・NaN bin 保持・timeline sync/selection/playhead/revision guard・2 window 独立 state・dock move resubscribe + detached-pump 停止 + revision race・repaint-only display ops・wire serde roundtrip・unregister 劣化・clip window read 限定 + review regression coverage である。

`mise run test` は 66 suites all `ok` で sv48 acceptance 36/36 PASS である。

独立 reviewer 2 名の P0/P1 全件修正 + verification review で第 2 pass（N1: meter/spectrum status 再導出不具合を修正、N2/N3: 非有限 geometry/scale 未検証を修正）を行った。実 device GUI は manual gate で NotRun である（synvelis#48）。

## SV-GUI-03 workspace GUI slices（synvelis#112）

slice1（Transport/Inspector）は `run-app-native` 実機で Play→Playing / Stop→Stopped / Inspector tab breakdown 表示を実窓 + ログ + OCR で確認した記録がある。

slice2/3 は PR #199/#201 で main 統合済みである。

slice4 IR は status=in-progress で検証節に実走結果の記入がなく、`view_mount.rs` 27 test Pass・`test-msrv` Pass（webdav flaky 1 件は retry で解消）の記録のみである。slice5 IR も status=in-progress で Validation 節が「(実走結果を追記)」のままであり、検証一式の実走結果は IR に未記録である。

slice6 は docs-only で `doc_status lint`（sv-gui-03-06 由来 finding 0）・`audit`・`audit_impl_record`・`fmt-check`・`git diff --check` を実施し、実機検証は NotRun（#114/#176 lane）である。slice7 は headless 契約試験 TS-EX-01〜11 を `tests/sv112_keymap.rs` で実走し全件 Pass（sv112_keymap 11 件含む workspace 全 test Pass）を記録した。

実 OS/実 GPUI/実 IME/実 Plugin GUI は NotRun/Deferred である（synvelis#112）。

## SV-GUI-04 multiwindow・dock・close-veto（synvelis#113）

precursor `sv113_dock.rs` は 5 test 全 Pass（workspace dock 契約 + cross-window `origin_view` `invalid_params` 拒否）である。

two-window slice1 は `tests/two_window.rs` 8/8 Pass + `SYNVELIS_SECOND_WINDOW=1` で `w2` 予約・`view-1`/`view-2` attach を log 観測し、env 未設定で 1 窓 boot 非回帰を確認した（PR #203）。

close-veto は `tests/close_veto.rs` 6 件全 Pass（dirty_last_window veto・clean approve・non-last 無条件 approve・quit partial veto・clean quit close all）+ 既存 shell/two_window/sv113_dock 非回帰である。

dock transfer は `tests/dock_ops.rs` 12 件全 Pass + `tests/adopt.rs` 8 件全 Pass + 既存 `sv113_dock`/`close_veto`/`two_window`/`view_mount`/`shell` 非回帰を merged tree `e72bb4b` で実施した。

実 OS での window/dock 操作証拠は synvelis#114 gate のため NotRun、`run-app-native` dock smoke は未実施（build のみ検証）である（synvelis#113）。

## SV-GUI-05 real-OS E2E freeze（synvelis#114）

SV-OS-01〜09 ケース定義 freeze + 受入条件 8 行の 3 分類 ledger である。AC1/AC3 は merged-evidence pointer、AC2/AC5/AC6/AC7/AC8 は `NotRun`、AC4 は外部 Owner + `NotRun` で、`Pass` は 0 行である。文書検証のみ（fmt-check・doc_status・audit・index 再生成）で cargo レーン・実 OS レーンは実行していない（synvelis#114）。

## SV-GPUI-02 plugin dock gate（synvelis#176）

PD-01〜07 freeze + 受入条件 9 行を pointer/`NotRun`/外部 Owner に分類し `Pass` は 0 行である。headless contract test は `plugin_dock_contract.rs` 5/5 Pass（Fake lane）+ `plugin_dock_contract_gpui.rs` 2/2 Pass（gpui-test lane）+ full `test-app-gpui` green である。全 mise lane・prek・index・audit が exit 0。

review で confirmed 指摘を修正した（upstream probe case が `plugin_dock_spec` でなく `required={}` の `probe_spec` を使う事実誤認の訂正、ledger 行の再分類、PD-03 Result 語彙修正等）。実 OS/実 Plugin/Native Dock は全て `NotRun` で、Issue #176 は Close しない（synvelis#176）。

## UI-C5 control/recording op surface（synvelis#102）

`uic5_app.rs` 9 テスト + workspace `uic5.rs` 14 テストが pass した（relation 双方向探索・edge gesture lifecycle/stale・recording 5 state/actions・malformed 劣化）。fmt-check/check/lint（app+workspace）pass である。実 Window/GUI/plugin window/高 DPI/keyboard/IME/accessibility の実機検証は Deferred である（synvelis#102）。

## sv488 lint feature-unification（synvelis#488）

CI run 43784707716 の `verify (ubuntu-latest)` で `lint-app-native` が `large_enum_variant`（ImportSubmit ≥305B vs ImportRebase ≥80B、enum ≥312B）で exit 101 を返した。ローカル Windows（rustc 1.99.1）で同一サイズ値を再現し、`lint synvelis-workspace` 単独は exit 0 で feature unification 依存を確認した。

`cargo tree -p synvelis-app --features native -e features -i serde_json` で `gpui_perf → serde_json/preserve_order` 経路を確認した。修正後 `lint-app-native` exit 0、`test synvelis-app` 47 test binaries 全て 0 failed（`library_browser.rs` の `import.submit` JSON deserialize 経路を含む wire 互換実測）である。AC1 の `mise.ci.toml` pin 1.99.0・ubuntu-latest 確認は merge 後 CI 証拠へ委譲、`*-msrv` 系はユーザー指定で NotRun である（synvelis#488）。

## 横断メモ

`doc-driven-dev-graph` の `route_graph.js` が repo 全体の既存 broken `defers` relation で fail-closed `blocked` を返すのは main 由来の既知状態で、複数 slice（sv-gui-03-04/05/06、sv-gui-05、sv-gpui-02、ui-sf）で共通である。`issue-147-sccache.jsonl` の既存 invalid event も既知状態として記録されている（synvelis#112/#114/#176/#102）。

## 出処

SV-GUI-01（synvelis#110）、SV-GUI-02（synvelis#111）、SV-48（synvelis#48）、SV-GUI-03（synvelis#112）、SV-GUI-04（synvelis#113）、SV-GUI-05（synvelis#114）、SV-GPUI-02（synvelis#176）、UI-C5（synvelis#102）、sv488（synvelis#488）。
