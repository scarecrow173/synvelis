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
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
      - type: "file"
        path: "crates/synvelis-app/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/binding.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/shell.rs"
      - type: "file"
        path: "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
      - type: "file"
        path: "docs/designs/sv175-native-shell-design.md"
      - type: "file"
        path: "docs/discovery/sv175-native-shell-inventory.md"
      - type: "file"
        path: "docs/plans/sv175-native-shell.md"
      - type: "file"
        path: "docs/specs/sv-gpui-01-native-app-shell.md"
      - type: "file"
        path: "docs/tasks/sv175-01-native-shell-binding.md"
      - type: "file"
        path: "docs/test-specs/sv175-01-native-shell-binding.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "tools/check-headless-deps.mjs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/discovery/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
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
metadata:
  experiments:
    adopted: []
    rejected: []
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

## 独立 review と対応

実装後に独立 reviewer 2 系統(コード正当性 / 契約・spec 適合)で検査し、
以下を対応した。

- **[major→fix] `ProjectionRow.preview` の wire 型誤り**: `Option<ControlValue>`
  で mirror していたが、producer は `BindingSnapshot.preview: Option<f64>`。
  preview 中の projection row が typed decode に落ちる実害のため `Option<f64>`
  へ修正。
- **[fix] `pending_view_ops` leak**: 失敗した `syn.view.*` op の tracking entry が
  remove されず残存。任意 Result で entry を消費し `ok` 時のみ reconcile へ変更。
- **[fix] attach result の `generation` 欠落**: `unwrap_or(0)` で epoch-0 binding が
  黙って成立し backend が stale drop する経路。`generation` 非数値は
  `invalid_params` 相当の失敗 result として報告するよう厳格化。
- **[fix] `Ready` 二重配信**: `initial_notify` が Ready 毎に再送される可能性を
  once-guard で排除。
- **[fix] `syn.viz.event` の view フィルタ欠落**: payload には top-level `view_id`
  がなく nested `VizViewEvent.viewId`(camelCase)を見て `for_me` 判定するよう修正。
- **[fix] `gesture_begin` が `command_id` を返さない**: begin も backend が
  receipt を発行するため `(gesture_id, command_id)` を返すよう変更。
- **[確認済・変更不要] `coalescible` 不存在**: 候補 `4c53e28` の `CommandIntent`
  serde を実コード照合し、wire に `coalescible` フィールドは存在しない。
  `expect_receipt` は envelope に常時 emit(commit/action のみ true)、
  `origin_view: Some("view-{id}")` も常時 emit — 本 decode は候補 wire に
  field-exact であり、`#[non_exhaustive]` 将来拡張への厳格性は `rvf.ui.command.v1`
  の versioning に委ねる設計と確認。
- **[確認済・変更不要] id 衝突なし**: 本側 mint は `{view}-{tag}{seq}`、候補
  command_id は `{view_id}:{counter}` で書式が交差しない。

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
- 独立 review で残置を確認した minor items(いずれも本 slice では許容):
  `expect_receipt` は decode するが情報保持のみで suppress 判定には未使用
  (receipt 必要性は backend が intent kind から導出、peer も独自規則で mint)。
  `origin_view` 欠落時の view 解決は multi-view window で非決定的
  (本 slice は 1 window 1 view)。`attached()/closed()` flag は
  `syn.view.state` push では reconcile しない(op result のみ)。
  mounted view には翻訳済 Result/Notify と raw notify broadcast が併存
  (classic consumer 互換のため意図的)。

## 主要決定

- `WindowLifecycle::Action(Intent)` を typed route とし raw-string message を廃止。
- close は vetoable `CloseRequested` を shell policy で approve し、`Destroyed` で
  最終 teardown と last-window exit を分離。
- `BindingAdapter` は translation layer のみで、`ControlsUi`/`VizUi` の
  gesture/receipt authority を複製しない。
- 旧 ADR sv-gui-01 は `status: superseded` + `superseded-by` link のみ。
  WebView/PoC 資産は削除しない。
