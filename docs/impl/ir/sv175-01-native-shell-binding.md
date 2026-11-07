---
id: "IMPL-SV17501NativeShellBinding"
type: "impl"
status: "completed"
title: "SV175-01 Native App Shell 初期 slice — NativeShell lifecycle + ViewBinding adapter 実装記録"
created: "2026-11-07"
updated: "2026-11-07"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/ruviform/issues/384"
  changes:
    added:
      - "crates/synvelis-app/src/bin/synvelis-native.rs"
      - "crates/synvelis-app/src/binding.rs"
      - "crates/synvelis-app/src/shell.rs"
      - "crates/synvelis-app/tests/binding.rs"
      - "crates/synvelis-app/tests/shell.rs"
      - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
      - "docs/designs/sv175-native-shell-design.md"
      - "docs/discovery/sv175-native-shell-inventory.md"
      - "docs/plans/sv175-native-shell.md"
      - "docs/specs/sv-gpui-01-native-app-shell.md"
      - "docs/tasks/sv175-01-native-shell-binding.md"
      - "docs/test-specs/sv175-01-native-shell-binding.md"
    modified:
      - "Cargo.lock"
      - "Cargo.toml"
      - "crates/synvelis-app/Cargo.toml"
      - "crates/synvelis-app/src/lib.rs"
      - "crates/synvelis-app/src/ui_ops.rs"
      - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
      - "mise.toml"
      - "tools/check-headless-deps.mjs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - "docs/adr/README.md"
      - "docs/designs/README.md"
      - "docs/discovery/README.md"
      - "docs/plans/README.md"
      - "docs/specs/README.md"
      - "docs/tasks/README.md"
      - "docs/test-specs/README.md"
  implements:
    - "docs/tasks/sv175-01-native-shell-binding.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
    - "docs/designs/sv175-native-shell-design.md"
    - "docs/plans/sv175-native-shell.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/sv175-native-shell-inventory.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv175-01-native-shell-binding.md"
  references: []
  defers: []
  deferred-by: []
---

# SV175-01 実装記録: Native App Shell 初期 slice

## 実施内容

- `binding.rs`: typed 非 RT `BindingAdapter`。`syn.view.attach/detach/close`・
  `syn.control.dispatch`(begin/preview/commit/cancel/action)・
  `syn.control.subscribe/unsubscribe` を `Intent::new`(rvf.ipc.v1) で mint し、
  `DispatchContext`(commandId/viewId/windowId/generation/issuedAt) を常時運搬。
  result/receipt/projection/view_state/lifecycle/job/error を demux。
  attach result の generation を採用し後続 context に適用。detach/close で
  pending・gesture・generation を reset し stale event を ignore。
- `binding.rs` RF387 wire 翻訳層: `rvf.ui.command.v1` Action intent
  (`{command_id, expect_receipt, intent: snake_case CommandIntent}`)
  を `deny_unknown_fields` 厳格 decode → SV typed `CommandIntent` へ
  variant 毎に変換(`atomic` 未指定≠false を保持・rvf Action 無しは
  `None`)、同 `command_id` で `syn.control.dispatch` mint。
  `syn.control.receipt` notify → `Event::Result`(ok=非 terminal、
  `status` 文字列保存、rejected/unsupported → `error`)、
  `syn.control.projection` → `Event::Notify`(`control_id`=target、
  revision/value/provenance)。generic key 置換・serde roundtrip に
  依らない意味保存変換のみ(coordinator 指示)。
- `shell.rs`: `rvf.ui.command.v1` Action を `on_view_command` で
  横取り translate → `syn.control.dispatch` に dispatch。ok-ack は
  view に流さず receipt 一本経路を維持、失敗 ack は rejected receipt
  として転送。`syn.view.attach` result の generation を view 毎に
  採用し DispatchContext に適用。outbox drain で receipt/projection
  notify を翻訳済イベントとして view の window にも配送(生 notify は
  broadcast 維持 = ViewBinding lane 共存)。
- `shell.rs`: driver-neutral `NativeShell`。`WindowSpec` から primary window を
  生成し `WindowLifecycle` Created/Ready/Action/CloseRequested/Destroyed/Tick を
  処理。`Action(Intent)` は `UiRuntime` 経由で `AppService` に dispatch し、
  origin window に routed `post_event` で返送。unrouted event は broadcast。
  `syn.app.quit` は silent destroy でなく close request。最後の window の
  `Destroyed` で view teardown + loop exit。Tick で outbox drain。
- `synvelis-native.rs`: `native` feature 専用 bin。`GpuiDriver` を組立て
  `NativeShell` に差し込む composition root。GPUI 型はこの bin に隔離。
- `ui_ops.rs`: deterministic UI clock(`set_ui_now`)を追加し issue 時刻を
  注入可能化。
- `Cargo.toml`/`synvelis-app/Cargo.toml`: `ruviform-ui-gpui`(optional),
  `ruviform-ui-test`(dev-dep), `native` feature, `synvelis-native` bin を追加。
  既存 `webview` feature/bin は変更せず preview lane として保持。
- `check-headless-deps.mjs`: `synvelis-app` は GUI composition crate のため
  対象外のまま維持(変更は guard の説明のみ)。

## 検証結果(実測)

| lane | 結果 |
| --- | --- |
| `mise run fmt` (cargo fmt --all) | pass |
| `mise run check` (cargo check --workspace --all-targets) | pass |
| `mise run lint` (cargo clippy -D warnings) | pass, warning 0 |
| `mise run test` (cargo test --workspace --locked) | pass, 失敗 0 |
| `node tools/check-headless-deps.mjs` | pass, 9 headless crates に GUI 依存なし |
| `node tools/check-ruviform-rev.mjs` | pass, 26 sources すべて `e8adcc09` |
| `cargo test -p synvelis-app --test binding` | 12 pass |
| `cargo test -p synvelis-app --test shell` | 10 pass |
| `cargo check -p synvelis-app --features native --all-targets` | pass(3m23s、transitive `proc-macro-error2` の future-incompat 警告のみ) |

## 残課題 / 未達 scope

- **実 OS/GPU 上の GPUI 実行は未観測**。`synvelis-native` の起動・実 window 表示・
  IME/native surface 挙動は headless CI では検証不可。FakeDriver による
  lifecycle 検証は契約上の正しさのみで、native acceptance には別途手動/実機
  検証が必要(driver lab / RF390 系)。
- **RF387 Component/View renderer は別 Worker 実装中**(候補 `4c53e28` を
  実コード調査済)。本 slice は Window/Host/Intent/Runtime 層と
  `rvf.ui.command.v1` wire 翻訳まで接続しており、実描画 ViewSpec/
  `create_view_window` mount は候補の main 取込後。Tab+Fader は
  headless `syn.control.*` 契約 + rvf wire 経路で検証済み。
- RF387 側差分(報告済・upstream 側の選択): consumer 側 `drain_inbox` が
  ok→Applied へ statuses を潰すため accepted/queued は `data.status` でのみ
  保存。projection の `control_id` は本 slice では `target` を使用
  (control_id≠target の view は独自 map が必要)。
- RF389 owner の実 desktop 入力 slot、SV112/113/176/114 は scope 外のまま。

## 主要決定

- `WindowLifecycle::Action(Intent)` を typed route とし raw-string message を廃止。
- close は vetoable `CloseRequested` を shell policy で approve し、`Destroyed` で
  最終 teardown と last-window exit を分離。
- `BindingAdapter` は translation layer のみで、`ControlsUi`/`VizUi` の
  gesture/receipt authority を複製しない。
- 旧 ADR sv-gui-01 は `status: superseded` + `superseded-by` link のみ。
  WebView/PoC 資産は削除しない。
