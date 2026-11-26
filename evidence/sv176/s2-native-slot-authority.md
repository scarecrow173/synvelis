# SV-GPUI-02 S2 — shared NativeSurfaceSlot/lease authority evidence

Date: 2026-11-26
Branch: `feat/176-plugin-dock-native-slot`
対象 Issue: `synvelis#176`（partial slice。Issue は close しない）
検証 worktree: `G:/orca/workspaces/synvelis/sv176-native-slot`
OS/toolchain: Windows 11 / MSYS2 (`MINGW64_NT-10.0-26300`), rustc 1.99.1, cargo 1.99.1
merge 取込後 base: `a24ff229d5084a6a44890ff3a2a06dc940cab0c9`
ruviform resolved SHA: `87a7ba8ff3fbe6d4502bea27d152d7f74c105899`（作業中に最新 main へ取り込まれた pin。S2 自身は pin bump を実施していない）

## 目的と範囲

本 evidence は、行 2・行 3 の structural gap に対する S2 slice を記録する。`PluginDock` から panel-private `SurfaceSlotRegistry` を取り除き、slot declaration・lease mint/validate/release・geometry/visibility・busy 制御を、composition root または test fixture から注入された単一の driver-neutral `NativeSlotAuthority` に集約した。lease redemption は upstream real-driver reference と同じく、opaque `ParentSlotLease` の raw payload を同じ authority に照合し、`ParentLease::new(SlotLease::new(raw).seq(), token)` に写像する。

本 slice は production L1 host wiring、real OS window、real child HWND、pixel/input/IME/popup、PD-01〜07 verdict、real scenario JSONL、Issue close を対象にしない。行 1・行 7 の受入分類は変えない。

## 実測した構造

- `PluginDock` は panel-private ledger を construct/own せず、共有 authority handle への参照だけを保持する。session authority は従来どおり app-shared `PluginEditorSessions` であり、panel へ移していない。
- `SynvelisApp::set_native_slot_authority` / `UiOpsState::native_slot_authority` が injection seam。authority 未注入時の `syn.plugin_dock.open`/`restore` は fake success ではなく `unavailable` を返す。
- `#[cfg(feature = "native")]` では upstream canonical `ruviform_ui_gpui::SharedSurface` に `NativeSlotAuthority` を実装する。fresh `GpuiSurfaceAdapter::new` は canonical ledger として使っていない。
- panel ごとの event drain API は残していない。共有 authority の event queue は panel が私物化せず、composition/test 側が authority 経由で観測する。
- headless test authority は upstream `FakeNativeSlot` を共用する test fixture。redemption token は lease sequence から作る明示的な synthetic test-only token であり、slot index でも OS handle でもなく、native container evidence ではない。

## Headless deterministic evidence

`tests/plugin_dock.rs`（26 tests）と `tests/plugin_dock_surface.rs`（7 tests）で、共有 authority に閉じた lease 不変式と既存 lifecycle 非回帰を確認した。

- 2 つの panel が同じ authority を共有し、一方が mint した lease は同じ authority で validate/redeem できるが、foreign authority では拒否される。
- close/release 後の revoked lease、二重 release、forged payload、slot index recycle 後の stale generation/lease は拒否される。
- dock 2-phase の busy mark 中は新規 lease mint が拒否され、clear 後に成功する。
- open/hide/show/redock/close、cross-window reattach、window-loss 収束、panel 横断 `SessionBusy`、logical save/restore、DSP/Project/RT 非干渉は非回帰。
- `tests/plugin_dock_probe.rs` は Coordinator 承認済みの lease expansion に従い、既存 real-ABI probe が新しい authority 注入 constructor に適合するよう機械的に更新した。同 probe 7 tests は pass したが、これは in-process real-ABI/host lifecycle の補強証拠であり native dock evidence ではない。

## GPUI-simulated evidence

`tests/plugin_dock_contract_gpui.rs` の `gpui-test` lane（5 tests）は semantic/layout evidence である。

- 製品 `plugin_dock_screen_spec` を `ViewHost + create_view_window` で mount し、canonical `GpuiDriver::surface()` に zero-sized Hidden slot を declare した。
- `GpuiDriver::bind_native_slot(window, ComponentKey("plugin.editor"), slot)` を first paint 前に実行し、renderer が non-empty geometry と `Visible` を canonical surface ledger に投影すること、`GeometryChanged`/`VisibilityChanged` event が残ることを確認した。
- `unbind_native_slot` 後の再描画で slot visibility が `Hidden` に suppress されることを確認した。
- unknown window、unknown/phantom slot、duplicate key、already-bound slot、cross-window slot、released/stale slot の bind refusal を固定した。
- 非 Windows では canonical `declare_slot` が `SlotError::Unsupported` になることを defined refusal expectation として固定した。silent skip ではない。
- `gpui-test` は raw parent を capture しないため lease/redeem を実行していない。native container、child HWND、pixel、input、IME、popup、PD verdict は一切主張しない。

## Canonical validation lanes（Windows 11、canonical = mise）

| lane | exit | verdict / 記録 |
| --- | ---: | --- |
| `mise run fmt-check` | 1 | **Fail（環境起因）** — `cargo fmt --all -- --check` が Windows command-line 長さ上限により `os error 206` で rustfmt を起動できない。workspace path を junction/subst で短くしても cargo が canonicalize した長い UNC path を使うため同じ失敗。補助証拠（canonical = mise ではない）として `cargo fmt -p synvelis-app -- --check` は exit 0 |
| `mise run check synvelis-app` | 0 | Pass |
| `mise run lint synvelis-app` | 0 | Pass（`-D warnings`） |
| `mise run test synvelis-app` | 0 | Pass。初回は既存 L1 runner artifact 未 build で `tim03_l1_process_oracle` のみ失敗したため、先に `mise run test-tim03-l1`（補助 lane、exit 0）を実行して runner を build し、再実行で pass |
| `mise run check-app-native` | 0 | Pass |
| `mise run lint-app-native` | 0 | Pass（`-D warnings`） |
| `mise run check-app-gpui` | 0 | Pass |
| `mise run lint-app-gpui` | 0 | Pass（初回 exit 101 は追加 test helper の clippy `needless_pass_by_value`。修正後に再実行して pass） |
| `mise run test-app-gpui` | 0 | Pass（clippy 修正・token fixture 修正後の最終再実行。`plugin_dock_contract_gpui` 5 tests を含む全 gpui-test target pass） |
| `mise run check-headless-deps` | 0 | Pass |
| `mise run check-ruviform-rev` | 0 | Pass。main merge で ruviform pin `87a7ba8f` が入り vendor REV が stale になったため、先に `mise run sync-ui-deps`（補助 lane、exit 0）で gitignored vendor を同期した |

直接 cargo は補助証拠に限る。`cargo check --locked --all-targets -p synvelis-app`、`cargo test --locked -p synvelis-app --features gpui-test --test plugin_dock_contract_gpui`（5 passed）、`cargo fmt -p synvelis-app -- --check` は実施済みだが、上表の canonical lane と区別する。

## Backend capability / verdict 分類

- Windows local lane: GPUI native surface slot backend は存在するが、`gpui-test` の positive result はあくまで `GpuiSimulated` semantic/layout projection。
- macOS/Linux: upstream backend は native surface slot を `Unsupported` と定義している。本 slice はその defined refusal を test expectation として記録し、positive lane を silent skip しない。
- `Pass`: 上表で exit 0 の canonical lane。
- `Fail`: `fmt-check` の Windows local 環境起因失敗。コード差分の format 自体は package-scoped rustfmt check で確認。
- `NotRun`: real OS window、real child HWND、pixel/input/IME/popup、PD-01〜07、production L1 host wiring、real scenario JSONL、3OS post-merge CI。
- `Unsupported`: 非 Windows backend native surface slot（defined refusal expectation）。
- `Deferred`: macOS/Linux の実 GUI/実 Device/実 Plugin、行 1・行 7 の Required real-window gate。

## Branch CI

- 実装・検証対象 commit: `682aafaeb72d8f59293986a2d3231380bcb94421`（merge 取込後 base `a24ff229d5084a6a44890ff3a2a06dc940cab0c9` に対する S2 実装・テスト・docs/evidence commit）。evidence 自身を含む最終 commit の SHA を同じファイル内で自己参照することはできないため、push 時点の正確な branch head SHA は PR 本文に記録する。
- branch CI Run URL/状態: push 後に PR 本文へ記録する。CI 非ブロッキング方針のため、未完了なら未完了のまま記録し、Pass に読み替えない。手動 dispatch は起動しない。

## Non-claims

- real OS window、real native plugin dock、real child HWND/container、pixel oracle、input/IME/popup、PD-01〜07 verdict を主張しない。
- headless/fake/GPUI-simulated の成功を Native Dock Pass に昇格しない。
- production L1 host wiring は未実装。`synvelis#101` 統合領域として後続に残る。
- `ruviform#631` real scenario JSONL / real consumer projection は未実施。
- docs index（`docs/**/README.md`）は編集していない。Coordinator が統合時に整備する。

## 残件

1. S3: real consumer projection、`ruviform#631` real scenario JSONL。
2. production L1 host wiring（`synvelis#101` 統合領域）。
3. 行 1・行 7 の Required real-window/native-dock gate（実 child HWND、独立 pixel/input oracle、DComp 修正込み実走）。
4. merge 後対象 SHA の必須 CI 照合（Coordinator 証拠工程）。
5. Windows local での `cargo fmt --all` command-line 長さ問題の恒久的対処（本 slice の範囲外）。
