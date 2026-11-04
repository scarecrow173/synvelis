---
id: "ADR-034RAA8oZVNDaL6mMAMJ55"
type: "adr"
status: "accepted"
title: "SV-GUI-02 syn.* binding IPC surface・notify outbox・bounded wire DTO"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-02-frontend-view-models-ruviform-custom-elements-typed-ipc-receipt.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/311"
    - "https://github.com/scarecrow173/ruviform/issues/310"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# SV-GUI-02 syn.* binding IPC surface・notify outbox・bounded wire DTO

## Context and Problem Statement

SV-GUI-02 は `synvelis-workspace` の headless 契約 (`ControlsUi`/
`VizUi`) を SolidJS frontend の Ruviform Custom Elements へ接続する。
両端の正本 (upstream `ui/binding` と `ControlsUi`/`VizUi`) は既に
同一 vocabulary を話すため、本 Issue の実作業は「`rvf.ipc.v1` op 面
と push 経路」の設計であり、以下を誤ると契約が壊れる:

- `Service::call` は 1 intent → 1 outcome であり、receipt/projection/
  viz frame のような **service 起点の push を返す仕組みがない**。
- projection/receipt を同一フラグや同一 op result に潰すと、
  `Accepted/Applied/Observed` 分離契約 (Issue #111) を偽装する。
- `ControlsUi` 内部型 (`InspectorBreakdown`/`ControlEvaluation`) は
  serde を持たず深い — そのまま送ると payload 無制限送信に抵触し、
  wire schema が内部実装へ結合する。
- `ViewportQuery` は source 基準、`VizUi` の query executor は
  (view, surface) 基準 — query 経路を 2 系統にすると stale/cache
  規則が分岐する。

## Decision Drivers

* `Accepted/Applied/Reported(Observed)/Saved` の分離と、遅延/逆順/
  重複/stale で確定を偽装しないこと (Issue #111 AC)。
* upstream `ui/binding` の `CommandBinding`/`ViewLink`/`dom.ts` と
  `ControlsUi`/`VizUi` を再実装しないこと。
* `rvf.ipc.v1` envelope (result/job/notify/lifecycle/error) を拡張
  しないこと — `sys.*` は runtime、製品 push は `notify` kind 内の
  `data.channel` 判別。
* headless `UiRuntime::dispatch_op` と webview host の同一契約 —
  push 経路も両方で drain できること。
* wire DTO は bounded・安定 — 内部型の serde を増やさない。

## Considered Options

### A. receipt/projection も op result に同梱する

`syn.control.dispatch` の result に receipt を詰め、projection も
`subscribe` result で返す。

* メリット: op が自明に完結し追加経路が不要。
* デメリット: 1 コマンドの `accepted`→`applied`→`observed` の時間列を
  result 1 回で表現できない。broadcast は他 view へ届かない。
  upstream contract の「receipt は ack channel、projection は
  subscribe channel」という分離を破壊する。不採用。

### B. `notify` event の `data.channel` で全 push を運ぶ

`syn.control.receipt` / `syn.control.projection` / `syn.viz.frame` /
`syn.viz.event` / `syn.view.state` を `Event::Notify` の `data` に
discriminator 付きで流し、`SynvelisApp` が `VecDeque<Event>` outbox
へ積む。host loop / headless test は共有 `Rc<RefCell<…>>` /
`Arc<Mutex<…>>` drain 面から取り出す。

* メリット: envelope 非拡張・全 view へ broadcast 可能・latest-wins
  notify は snapshot frame と semantic 一致・headless で完全に検証可。
* デメリット: `UiRuntime::handle` の返り値と別経路になるため
  drain を host loop が必ず呼ぶ規約が必要 (既存の quit flag と同型
  の共有ハンドルで解決)。採用。

### C. `rvf.ipc.v1` に製品専用 kind を追加する

`kind: "syn.receipt"` のような新 kind を Rust/TS 両側に加える。

* メリット: 型が厳密。
* デメリット: ruviform 側 schema (`parse_intent`/envelope/fixture
  corpus) を改変しないと成立せず、公開境界不変の制約に違反。
  不採用。

### D. wire DTO ではなく内部型へ serde を付与する

`InspectorBreakdown`/`ControlEvaluation`/`ProjectionValue` に
`Serialize` を足してそのまま送る。

* メリット: DTO 写像コードが不要。
* デメリット: session 内部の深い構造 (contributions/observation/
  capability) を全送信 = payload 無制限に抵触。内部型の変更が即
  wire 変更になる。不採用。

### E. viz query を 2 系統 (element 直問い合わせ + VizUi surface) にする

`IpcWaveformSource.query` が port に直接届く別経路を作る。

* メリット: element 契約が表面的に近い。
* デメリット: `VizUi` の surface 状態 (span/cache/stale/status) を
  bypass し、stale/byte-bound 規則が分岐して 2 正本化。不採用。

## Decision Outcome

**B + bounded wire DTO + VizUi 単一 query executor** を採用する。

1. `syn.*` op 面: `syn.view.attach|detach|close|list`、
   `syn.control.dispatch|subscribe|unsubscribe|targets`、
   `syn.viz.bind|unbind|meta|query|query.cancel|viewport|select|
   plot-px|snapshot`。op result は最小 ack のみ。
2. push は `notify` + `data.channel`:
   `syn.view.state` / `syn.control.receipt` /
   `syn.control.projection` / `syn.viz.frame` / `syn.viz.event` /
   `syn.app.state` (既存 Ready push)。`SynvelisApp` が outbox
   `VecDeque<Event>` へ積み、`event_outbox()` で共有
   `Rc<RefCell<…>>` を公開 — host loop が毎 iteration drain +
   `host.eval(dispatch_script)`、headless テストも同じ面を drain。
   `Arc<Mutex<…>>` ではなく `Rc` を使う — `SynvelisApp` は単一
   スレッド設計であり、UiRuntime dispatch も同一スレッド。
3. wire DTO: receipt は `CommandReceipt` (既 serde) をそのまま、
   projection は `ParamView` (`plain`/`normalized`/`read_only`/
   `mode`/`armed`/`observation`/`capability`) と `TransportView`
   (`status`/`position`/`loop`/`record`/`recording`) に写像。
   viz は `WaveformMeta`/`WaveformView`/`QueryReply`/`MeterFrame`/
   `SpectrumFrame`/`VizViewEvent` (既 serde) をそのまま利用。
4. viz query は `syn.viz.query {view_id, surface_id, start, end,
   buckets, max_bytes}` → `set_span`+`set_plot_px`+`ensure_waveform`
   → surface row を `QueryReply` 形で返す。sourceId 基準の汎用
   `BindingPort.query` は (view, source) → bound surface 解決経由で
   同じ executor に落とす。

## Consequences

- receipt/projection/frame の wire は `data.channel` discriminated
  union 1 系統 — frontend の notify router が (channel, view_id,
  target/surface) で route する。
- `syn.control.dispatch` は「受理した」ことだけを返し、結果は
  receipt channel で届く — frontend CommandBinding の ack 経路と
  一致する。
- `syn.view.attach` は frontend ViewLink generation と backend
  ViewState generation を lockstep で動かす責務を持つ (両者とも
  attach で +1)。attach/detach/close は必ず `syn.view.*` を通す。
- view close は `ControlsUi::close_view`/`VizUi::close_view` のみ —
  recording transaction/audio/render job は session 側に残る。
- 16 MiB を超える viz payload は現状 `over-budget` error — `rvf://
  data` 経路は需要時に別 ADR で移行する。
- ruviform 依存を RF-GT-01/02/03 同梱 rev へ追従する — `Cargo.lock`
  更新 + `mise run sync-ui-deps` 再生成で `check-ruviform-rev` 整合。

## Alternatives Considered

Considered Options 節の A/C/D/E を参照 — いずれも契約分離・境界不変・
bounded payload・単一 executor のいずれかを破壊するため不採用。
