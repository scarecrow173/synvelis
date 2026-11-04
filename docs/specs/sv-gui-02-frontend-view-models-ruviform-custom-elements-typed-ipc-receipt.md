---
id: "SPEC-034RAxWpKzQgp5rbvJLZMR"
type: "spec"
status: "approved"
title: "SV-GUI-02 Frontend View Models — Ruviform Custom Elements・Typed IPC/Receipt統合"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/111"
    - "https://github.com/scarecrow173/synvelis/issues/23"
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
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/overview.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/310"
    - "https://github.com/scarecrow173/ruviform/issues/311"
    - "https://github.com/scarecrow173/ruviform/issues/312"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/81"
    - "https://github.com/scarecrow173/synvelis/issues/112"
  defers: []
  deferred-by: []
---

# SV-GUI-02 Frontend View Models — Ruviform Custom Elements・Typed IPC/Receipt統合

## What Should Be Built

Issue #111: **すべての製品 View が共有する薄い View Model / Typed IPC /
Binding / Receipt / Lifecycle Adapter 層**を構築し、SV-GUI-01 で用意した
実 App Shell(`ui/` SolidJS + `synvelis-app` `syn.*` op 面)上で Ruviform
Custom Elements を実 DOM に接続する。

本 Issue が所有するのは共有基盤のみであり、個別業務 View(Signal Flow
専用 Projection は #108/SF-05、Control Relations UI は #102/PC-C5、
Observability Snapshot は #81)は各 Owner の範囲とする。確定方針
(Issue #111 本文・2026-11-04):

- **Frontend は SolidJS + TypeScript**。状態正本は既存 Rust
  Project/Workspace/Session のまま。SolidJS Store/Signal を第二の
  Project/Engine/Authorization 正本にしない。
- **Ruviform の公開境界は変更しない**。framework-neutral plain TS
  Custom Elements / typed DOM properties / CustomEvent / Binding /
  Viz / Host をそのまま消費し、SolidJS 用の薄い型付き Adapter は
  Synvelis `ui/` 側に置く。Ruviform 本番/標準 Showcase/共通 Test
  Harness へ SolidJS 依存を持ち込まない。
- **Headless 契約を再利用する**。`synvelis-workspace` の
  `ControlsUi`(SV-47)・`VizUi`(SV-48)と upstream `ui/binding` の
  `CommandBinding`/`QueryBinding`/`ViewLink`/`dom.ts` が両端の正本。
  本 Issue はこの 2 つの正本を `rvf.ipc.v1` op 面で接続する Adapter
  を作るのであり、receipt/phase/generation/stale 規則を再実装しない。
- **Receipt 分離**。`Accepted/Queued/Applied/Reported(Observed)/Saved`
  を 1 フラグへ潰さない。`rejected`/`unsupported`/`timeout`/`stale`
  を明示し、遅延・逆順・重複・欠落で確定を偽装しない。不明/欠落は
  `Unknown`/`Stale` として表示する。
- **Event/Query/Receipt は `ProjectRevision + ViewInstanceId/WindowId +
  InteractionId(commandId)/Sequence + domain-specific Generation` を
  保持**し、Payload を無制限に WebView へ送らない(query の maxBytes、
  notify の bounded row)。

初期 View として、App Shell に **Reference Workspace View**(仮称
`ConsoleView`)を mount する — transport・seek・1 本の parameter strip
(knob/fader)・waveform surface・meter surface を Ruviform Reference
Components で構成し、未接続ソース/ターゲットは capability/unavailable
として正直に表示する。業務 View への展開は SV-GUI-03 (#112) 以降。

## Why Now

Execution DAG #23 で本 Issue は GUI トラック 2 番手 (#110→#111→#112)。
SV-GUI-01 が Window/Host/Asset/op 面を確立したため、残る欠落は
「headless 契約 (SV-47/SV-48/WS-A) を実 DOM へ接続する層」だけである。
Ruviform 側では RF-GT-01 (`ui/browser-test` Playwright harness)、
RF-GT-02 (`ui/guitest` IPC conformance + scenario fixture + 独立
oracle)、RF-GT-03 (`ruviform-ui-test` crate) が main マージ済みで、
共有 fixture をそのまま再利用できる。

## Users and Value

- DAW ユーザー: gesture (begin→preview→commit) が「表示された値 =
  確定値」を意味しないことを正確に見る — pending/rejected/timeout が
  可視化され、失敗した intent が truth を装わない。
- 製品 View 実装者 (SV-GUI-03 以降・SF-05/PC-C5/OBS): 1 つの View
  Model + Binding + IPC Adapter パターンに載せるだけで、receipt 分離・
  stale guard・subscription cleanup・dock/remount 契約を再実装せずに
  済む。
- 検証者: RF-GT-01/02 fixture と独立 oracle で、記録済み応答の再生
  ではなく契約モデルとの照合で Pass/Fail が決まる。

## Scope

### In Scope

1. `synvelis-app` の `syn.*` op 面拡張(headless・webview feature 双方
   で同一 dispatch):
   - `syn.view.*` — logical view の attach/detach/close/list(`view_id`
     × `window_id` × generation を `ControlsUi`/`VizUi` 双方へ反映)。
   - `syn.control.*` — `CommandIntent + DispatchContext` の dispatch、
     subscribe/unsubscribe、bindable target 一覧(bounded)。
   - `syn.viz.*` — surface bind/unbind・bounded query・viewport/select
     intent forward・meter/spectrum snapshot 取得。
   -  Receipt/projection/frame/lifecycle の push は `notify` event
     (`data.channel` 判別)として outbox に積み、host loop と headless
     双方が drain できる共有 drain 面を用意する。payload は bounded
     row(DTO)とし、内部型をそのまま垂れ流さない。
2. `ui/` SolidJS frontend(SolidJS 依存は `ui/` に閉じる):
   - `window.__rvf` 上の **`BindingPort` 実装**(dispatch→
     `syn.control.dispatch`、subscribe→`syn.control.subscribe` +
     notify route、query→`syn.viz.query`、cancelQuery→
     `syn.viz.query.cancel`)。
   - DOM 非依存の **View Model 層**(`ViewLink`/`LinkRegistry` +
     binding 群 + viz source adapter を束ねる薄い owner)。
   - **SolidJS adapter**: `onMount/onCleanup` と Custom Element
     connect/disconnect/reconnect の整合、`ref` による typed property
     反映(`value`/`pending`/`unsupported`/`state`/`source`/
     `overlays` 等、object property を attribute へ JSON 化しない)、
     ネイティブ CustomEvent listener の owner-tied cleanup。
   - Reference Workspace View(`ConsoleView`)を App Shell に mount:
     `rvf-transport`/`rvf-seekbar`/`rvf-knob`/`rvf-fader`/
     `rvf-waveform`/`rvf-meter` を binding 経由で接続。
3. ライフサイクル契約の両面実装: View mount/unmount・dock 移動
   (`dock-transfer` detach+attach)・WebView 再作成(frontend reload →
   再 attach/rehydrate)・theme 切替(`rvf-theme` token 切替 + 状態再
   attach で描画維持)・2 logical window 同 target subscription・
   subscription cleanup/rehydration。View close で Project
   Audio/Record/Render Job を暗黙キャンセルしない。
4. Capability/unavailable 安全表示: 未登録 source・未存在 param
   target・未接続 port/plugin/probe は `unsupported`/`unavailable`/
   `no-source` として偽装せず表示する。
5. 検証: RF-GT-02 `ui/guitest` fixture を vendored 共有物として
   `ui/` 側の conformance/replay lane に組み込み、独立
   `ContractModel` oracle と照合する。RF-GT-01 `ui/browser-test` の
   DAW-shaped lane を consumer fixture で配線する(実ブラウザ実行は
   環境依存のため実施可否を検証報告で明示)。
6. ruviform 依存を RF-GT-01/02/03 同梱 rev へ追従し、vendor 同期・
   `Cargo.lock`・`mise run check-ruviform-rev` 整合を回復する。

### Out of Scope

- 全 DAW 業務 View(Arrangement/Mixer/Inspector 等の画面実装 —
  SV-GUI-03 以降)、SF-05 Signal Flow 専用 Projection(#108)、PC-C5
  Control Relations UI(#102)、Observability Snapshot(#81)。
- 実 multi-OS-window host(`run_multi` 実機配線)、実 OS window の
  Dock detach — `window_id` は wire 契約として保持し、論理 window
  (同一 WebView 内 dock pane 相当)での 2 window 検証に留める。
- 実 audio engine/backend・実 plugin host・実 DSP — meter/spectrum
  は port 未登録時の `unavailable` 表示が正直な初期状態。
- RF-GT-04(#313)の実 WebView E2E・OS 入力注入 — SV-GUI-05 (#114) の
  範囲。`ruviform-ui-test` の test host control port は本 Issue では
  製品 bin に組み込まない。
- Ruviform 側への変更(upstream fixture/contract/element の fork・
  SolidJS 依存の逆流)。製品 Plugin/Auth/Undo の Ruviform 移管。
- macOS/Linux 実機検証(本機は Windows — 未検証として明記)。

## Requirements

- `syn.*` op は headless `UiRuntime::dispatch_op` と webview host
  経由で同一結果を返し、GUI 依存が core crate へ逆流しない
  (`mise run check-headless-deps`)。
- Frontend の全 mutation は binding → `syn.*` op 経由であり、View
  Model/Store は `Project`/`ProjectController`/engine/authorization の
  正本を持たない。
- Binding layer は upstream `ui/binding` の `CommandBinding`/
  `QueryBinding`/`ViewLink`/`dom.ts` を直接使用し、receipt
  correlation・ack timeout・coalescing・generation guard・
  revision-guarded projection を再実装しない。
- Rust 側は `ControlsUi`/`VizUi` を通じてのみ `ProjectSession` を
  触り、op ハンドラが `ProjectController` を直接叩かない(transport
  set 等の既存 session op は現行のまま)。
- Event/Query/Receipt 経路で `ProjectRevision`・`view_id`・
  `window_id`・`command_id`/`sequence`・`generation` を保持し、欠落/
  不明をゼロ値に潰さない(`Unknown`/`Stale` 表現)。
- Query/notify payload は bounded:`maxBytes` enforcement・bounded
  row DTO・frame は latest-wins snapshot のみ。
- view detach は in-flight gesture の contractual cancel を行い、
  Project Audio/Record/Render Job は暗黙キャンセルしない
  (`ControlsUi`/`VizUi` 契約どおり)。
- 本番とテストで同一 frontend build artifact を使う。
- SolidJS の Signal/Effect→element property 更新は element の
  controlled-write 規約(書き込みは intent を発火しない)に従い、
  feedback loop・二重 command を発生させない。

## Acceptance Criteria

Issue #111 本文の Acceptance を検証可能な形に分解したもの。

- [ ] **SV-GUI-02-A**: `rvf-knob`/`rvf-fader`/`rvf-transport`/
  `rvf-seekbar`/`rvf-waveform`/`rvf-meter` が実 DOM(happy-dom
  harness + 実ブラウザ lane)上で upstream `bindControlElement`/
  `bindTransportElement`/`bindViewIntents` 経路で動作し、Synvelis
  固有の gesture/renderer fork が存在しない(vendored `ui/*` は
  upstream rev のまま改変なし)。
- [ ] **SV-GUI-02-B**: fake host(`__rvf` 実装)と実 `SynvelisApp`
  headless `UiRuntime` の両経路で Input→Intent→Receipt→Projection
  →View 更新が確認できる。受領・適用・観測は別状態として扱われる。
- [ ] **SV-GUI-02-C**: 2 logical window 同 target・mount/unmount/
  remount・dock 移動・focus 変更で、誤配送・誤適用・subscription/
  listener 二重登録・listener/GPU leak がない(attach/detach/
  remount の各計数で示す)。
- [ ] **SV-GUI-02-D**: delayed/reordered/duplicate/unsupported/
  timeout/stale の各故障注入で、phase が `rejected`/`unsupported`/
  `timeout`/`stale` に落ち、確定値表示を偽装しない。
- [ ] **SV-GUI-02-E**: read-only/unsupported target の gesture が
  `rejected`/`unsupported` receipt として表示され、UI 値丸め
  (normalized↔plain)が host 規約どおり。Keyboard/ARIA の element
  契約が upstream element のまま保持される。
- [ ] **SV-GUI-02-F**: `syn.*` op が headless `UiRuntime` と同一
  結果を返し、`mise run check`/`lint`/`test`/`check-ui`/`test-ui`/
  `build-ui`/`check-app-webview`/`lint-app-webview`/`test-app-webview`
  が pass。headless 証拠と実機/実ブラウザ証拠を分離して報告し、
  未検証項目を明記する。
- [ ] **SV-GUI-02-G**: RF-GT-02 fixture(scenario corpus + 独立
  `ContractModel` oracle)が vendored lane で pass し、replay は
  記録済み応答への一致ではなく oracle 期待値への一致で判定される。

## Deferred Design Concerns

- 実 multi-OS-window host(`ruviform-ui-webview` `run_multi`)と実 Dock
  detach — `window_id` は wire/contract で保持済み。SV-GUI-03 での実機
  配線を予定。
- `ruviform-ui-test`(RF-GT-03)test host control port の製品 bin 統合 —
  SV-GUI-05 (#114) の実 GUI E2E harness で再検討。
- 大容量 viz payload の `rvf://data` 移行 — 現行は op result の
  `maxBytes` 強制で bounded。16 MiB 超の需要が出た時点で移行。

## Sources and Relations

- Issue #111 本文 (Acceptance が正本)・Issue #23 Execution DAG・
  SV-GUI-01 spec (#110) の接続点記述。
- `synvelis-workspace::controls_ui` (SV-47 headless bridge)・
  `synvelis-workspace::viz_ui` (SV-48 headless viz contract) —
  backend 側正本。
- Ruviform `ui/binding` (contract/binding/link/query/dom)・
  `ui/components` + `ui/viz` Custom Elements — frontend 側正本。
- RF-GT-01 `ui/browser-test`・RF-GT-02 `ui/guitest`・RF-GT-03
  `ruviform-ui-test` — 共有検証基盤 (再利用、再実装しない)。
