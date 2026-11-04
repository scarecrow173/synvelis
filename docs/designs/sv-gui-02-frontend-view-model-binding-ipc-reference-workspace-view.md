---
id: "DESIGN-034RAAXZ1XMIuk2zr2dr4w"
type: "design"
status: "approved"
title: "SV-GUI-02 Frontend View Model・Binding IPC・Reference Workspace View"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
  changes:
    added:
      - "crates/synvelis-app/src/ui_ops.rs"
      - "crates/synvelis-app/src/ui_dto.rs"
      - "crates/synvelis-app/src/ui_outbox.rs"
      - "ui/src/binding/"
      - "ui/src/viewmodel/"
      - "ui/src/views/console.tsx"
      - "ui/test/"
    modified:
      - "crates/synvelis-app/src/app.rs"
      - "crates/synvelis-app/src/service.rs"
      - "crates/synvelis-app/src/host.rs"
      - "ui/src/ipc.ts"
      - "ui/src/adapters/ruviform.ts"
      - "ui/src/app.tsx"
      - "ui/src/store.ts"
      - "Cargo.lock"
      - "ui/package.json"
      - "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated:
      - "ui/vendor/ruviform/"
      - "crates/synvelis-app/frontend/dist/"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
    - "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/310"
    - "https://github.com/scarecrow173/ruviform/issues/311"
    - "https://github.com/scarecrow173/ruviform/issues/312"
  defers: []
  deferred-by: []
---

# SV-GUI-02 Frontend View Model・Binding IPC・Reference Workspace View

## Context

SV-GUI-01 が `synvelis-app`(Composition Root + `AppService` `syn.*` op 面)
と `ui/`(SolidJS App Shell + vendored `ruviform/ui-*`)を提供した。本設計
は spec/ADR どおり、`synvelis-workspace` の `ControlsUi`/`VizUi` と
upstream `ui/binding` を `rvf.ipc.v1` op + notify channel で接続する層を
定義する。状態正本は `ProjectSession`/`WorkspaceManager` と `ControlsUi`/
`VizUi` の ViewState — SolidJS 側はその投影のみ保持する。

## Scope

### In Scope

- `synvelis-app` への `syn.view.*`/`syn.control.*`/`syn.viz.*` op と
  notify outbox (receipt/projection/frame/lifecycle push)。
- `ui/` への `BindingPort`(`__rvf` 上)・notify router・View Model
  owner・SolidJS adapter・`ConsoleView` reference view。
- ruviform 依存を RF-GT-01/02/03 同梱 rev へ追従 + vendor 再同期。
- headless op テスト + vitest DOM/binding テスト + `ui/guitest`
  fixture を使った conformance/replay lane(補助証拠)。

### Out of Scope

- 業務 View 群・SF-05/PC-C5/OBS 専用 UI・実 multi-OS-window・実 audio
  backend/plugin・RF-GT-04 実 WebView E2E・ruviform 側改変。

## Components and Boundaries

### Rust — `crates/synvelis-app/src/ui_dto.rs` (新規)

wire DTO のみ。`serde` 付き・bounded:

- `ViewInfoDto { view_id, window_id, generation, attached }`
- `ParamView { plain, normalized, read_only, mode, armed, observation,
  capability }` — `ControlsUi::strip_row`/inspector の平坦化
- `TransportView { status, position_tick, loop_enabled, record_enabled,
  recording }` — `transport_row` 写像
- `ControlProjectionDto { view_id, target, value: ParamView|TransportView,
  generation, project_revision }`
- `VizFrameDto { view_id, surface_id, channel: "meter"|"spectrum",
  frame }` — `MeterFrame`/`SpectrumFrame` (既 serde) 同梱
- `ReceiptDto` = `synvelis_workspace::controls_ui::CommandReceipt`
  (既 serde をそのまま)

### Rust — `crates/synvelis-app/src/ui_outbox.rs` (新規)

`Rc<RefCell<VecDeque<ruviform_ui::ipc::Event>>>`。`push_notify(channel,
data)` で `Event::Notify` を積み、`event_outbox()` で共有ハンドルを
公開する。host loop(webview feature)が毎 tick drain して
`dispatch_script` eval、headless test は同じ面から drain。quit flag と
同型の共有ハンドル規約。outbox 深度上限(= 4096、overflow 時は古い
frame/snapshot 系から破棄し diagnostics カウンタに記録)。

### Rust — `crates/synvelis-app/src/ui_ops.rs` (新規)

`SynvelisApp` 上の op 実装(自由関数群、`app.rs` は薄い委譲のみ):

- `view_attach(view, window)` → `controls_ui.attach_view(session, v, w)`
  + `viz_ui.attach_view(v, w)` + outbox `syn.view.state`。返値
  `ViewInfoDto`(両 UI の generation を返し、両者の lockstep を wire で
  観測可能にする)。再 attach(同 view)は detach→attach 規約で
  generation bump。
- `view_detach(view)` / `view_close(view)` → 両 UI の detach/close。
  detach は in-flight gesture の contractual cancel のみ、Project
  Audio/Record/Render は残る。
- `control_dispatch(ctx, intent)` → `controls_ui.dispatch(session, …)`。
  op result は `{accepted: bool}`(queue 受理のみ)。以後の
  `WireReceipt`/`deliver` は `ui_pump()` で outbox `syn.control.receipt`
  へ流す。
- `control_subscribe(view, target)` / `control_unsubscribe` →
  `controls_ui.subscribe`。subscribe 直後の現行 projection は outbox
  `syn.control.projection` で push(初期ハイドレーション)。
- `control_targets()` → bounded 一覧(target id + kind + read_only)。
  ページングではなく最大 N(=256)を cap として明示。
- `viz_bind(view, surface, source, kind)` → `bind_waveform`/`bind_meter`/
  `bind_spectrum`。`viz_unbind` は対応解除。`viz_meta(view, surface)`
  → `WaveformMeta`/`MeterMeta` 相当の row 写像。
- `viz_query(view, surface, start, end, buckets, max_bytes)` →
  `set_span`+`set_plot_px`+`ensure_waveform` 実行後に surface row を
  `QueryReply` 形で返す。`viz_query_cancel` は in-flight flag 解除。
- `viz_viewport(view, surface, …)` → `pan`/`zoom`/`set_span` 委譲、
  `viz_select` → `select`、`viz_plot_px` → `set_plot_px`。
- `viz_snapshot(view, surface)` → `meter_row`/`spectrum_row` の
  latest-complete-frame row 返却(latest-wins)。
- `ui_pump(now_ms)` → `controls_ui.pump`/`viz_ui.pump` + `drain_wire`/
  `drain_events` → outbox push。dispatch 経路のたびに必ず呼ぶ(従来
  `pump` を op 単位で呼ぶ規約に統一し時計は呼び出し側注入)。

### Rust — `service.rs` / `app.rs` / `host.rs` 変更

- `AppService` の op match に `syn.view.*`/`syn.control.*`/`syn.viz.*`
  を追加し、エラーは既存 `AppError → error.code` 経路。
- `capabilities()` の `views`/`multi_window` を truth に更新
  (`views: available`、`multi_window` は依然 `unavailable` — 実 OS
  window は out of scope)。
- webview `host.rs`: event loop tick で `app.ui_pump(now)` + outbox
  drain → `host.eval`、view reload 時の再 attach は frontend 契約。

### TS — `ui/src/binding/` (新規)

- `port.ts` — upstream `BindingPort` を `ipc.call` 上に実装:
  `dispatch(ctx,intent)` → `syn.control.dispatch`、
  `subscribe(ctx,cb)` → `syn.control.subscribe` + router 経路登録、
  `query(q)` → source→(view,surface) 解決後 `syn.viz.query`、
  `cancelQuery` → `syn.viz.query.cancel`。
- `router.ts` — `onEvent` で `data.channel` を見て
  `syn.control.receipt`→CommandBinding 経路(ack)、
  `syn.control.projection`→subscribe callback、`syn.viz.frame`→
  source subscriber、`syn.view.state`→view registry へ dispatch。
  (channel, view, target|surface) を key に listener map。
- `wire.ts` — op 引数/返値の型定義(DTO mirror、upstream contract 型
  `CommandIntent`/`CommandReceipt`/`ViewportQuery`/`QueryReply` を
  vendored `@ruviform/ui-binding` から import)。

### TS — `ui/src/viewmodel/` (新規・DOM 非依存)

- `console-view-model.ts` — `ViewLink` + param strip `CommandBinding`
  + transport `CommandBinding` + viz surface 管理を束ねる owner。
  `attach(viewId, windowId)` で `syn.view.attach` → subscribe →
  binding attach、`detach()` で unbind+`syn.view.detach`。dock
  transfer = detach+attach。`BindingViewState` を Solid store へ
  写像する projection adapter のみ持つ(正本は binding)。
- `viz-source.ts` — `WaveformDataSource`/`MeterDataSource` を
  `syn.viz.*` + router frame channel 上に実装(element の `source`
  property に渡す)。

### TS — `adapters/ruviform.ts` 拡張 + `views/console.tsx`

- `registerElements` で components + viz 全要素を登録。`setProp` に
  `source`/`overlays`/`style` 等の object property 経路を追加
  (attribute JSON 化禁止)。`onRvfEvent` は owner-tied cleanup。
- `views/console.tsx` — `rvf-transport`/`rvf-seekbar`/`rvf-knob`/
  `rvf-fader`/`rvf-waveform`/`rvf-meter` を `bindControlElement`/
  `bindTransportElement`/`bindViewIntents` 経由で接続。未接続
  source/target は `unsupported`/`unavailable` 表示。
- `app.tsx` — mount で `syn.view.attach` して ConsoleView を表示、
  reload/remount で再 attach(rehydrate)。

## Data and Control Flow

Gesture commit の例:

1. `rvf-knob` が `rvf-gesture` CustomEvent を発火 → `bindControlElement`
   (upstream) が `CommandIntent` へ変換 → `CommandBinding`。
2. `BindingPort.dispatch` → `ipc.call("syn.control.dispatch", …)` →
   `ControlsUi::dispatch` → `ProjectSession` 制御セッション。返値は
   受理のみ。
3. `ui_pump` が `drain_wire` → outbox `syn.control.receipt` notify →
   frontend router → `CommandBinding` ack 経路 → phase 遷移
   (accepted/applied/rejected/timeout/stale)。
4. 状態確定後 `controls_ui` が subscription 経由で projection 生成 →
   `ui_pump` が `syn.control.projection` notify → router → subscribe
   callback → `CommandBinding` projection 経路 → element `value`/
   `pending`/`unsupported`/`state` へ写像(SolidJS は結果の投影のみ)。
5. viz: `rvf-waveform` が viewport query → `WaveformDataSource.query`
   → `syn.viz.query` → `VizUi::ensure_waveform` → bounded row を返す。
   meter は `syn.viz.frame` notify → `SnapshotSource` 更新 → element
   再描画。

## Non-Functional Constraints

- `maxBytes`/`buckets`/`channels` は op 引数と `VizUi` 双方で強制、
  bounded 超過は `over-budget`。
- outbox は 4096 上限、frame 系は latest-wins 破棄で圧迫しない。
- frontend の reactive 更新は element property への書き込みだけ —
  viz source 再計算・FFT/LOD 再生成を直接発火しない。
- headless dispatch と webview dispatch は同一 `AppService` 実装 —
  feature 差は host の配線のみ。
- SolidJS/Ruviform 依存は `ui/` と `webview` feature に閉じる。

## Risks and Trade-offs

- **generation lockstep ずれ**(frontend ViewLink と backend ViewState):
  `syn.view.attach` が両 UI generation を返し、frontend は自身の
  generation を dispatch ctx へ明示する。不一致 receipt/projection は
  stale 扱い — 契約どおり確定を偽装しない方向へ倒れるので安全側。
- **outbox の drain 漏れ**(host tick 実装ミス): headless drain テストで
  「op→pump→outbox notify」順序を検証し、host 側は `quit_flag` と同型
  の共有ハンドルで単一規約にする。
- **ruviform rev 追従の互換性**(`073a3ff`→`8fb665f`): upstream diff は
  追加中心。vendor 再同期 + `check-ruviform-rev` + 既存 headless 全
  テストで退行検出。破壊的変更があれば plan で明示する。
- **`BindingPort.subscribe` の callback 型**: upstream は per-target
  callback を返す設計 — router が (view,target) で複数 callback を
  多重登録できるよう fan-out とし、unsubscribe は登録解除のみ。

## Deferred Design Concerns

- 実 multi-OS-window host・実 Dock detach・`ruviform-ui-test` control
  port の bin 組込 — SV-GUI-03/05 で再検討(spec の defer と同値)。
- `rvf://data` への viz 大容量移行 — 16MiB 超需要時。
- timeline surface の IPC 化 — `VizUi` 側 op は同期(sync_timeline)が
  session 依存で、wire 化は signal-flow 業務 View (#108) と一緒が
  妥当。本設計では waveform/meter/spectrum のみ。

## Verification Notes

- [ ] headless `UiRuntime::dispatch_op` で `syn.view.attach`→
  `syn.control.dispatch`→pump→outbox `syn.control.receipt`→
  `syn.control.projection` の順序がテストで観測できる。
- [ ] fake `__rvf` 上の `BindingPort`+`CommandBinding`+
  `bindControlElement` で delayed/duplicate/stale/timeout/unsupported
  を注入して phase が正しく落ちる(vitest + happy-dom)。
- [ ] `ui/guitest` fixture の scenario を `BindingPort` 実装へ replay
  し、独立 `ContractModel` oracle 期待値と一致する(補助証拠)。
- [ ] `mise run check/lint/test/check-ui/test-ui/build-ui/
  check-app-webview/lint-app-webview/test-app-webview` が pass。
