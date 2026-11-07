---
id: "ADR-vXwuA1OrALJD99tNRgZ8gQ"
type: "adr"
status: "accepted"
title: "ADR: Native App Shell は Driver-neutral shell + GPUI 標準 Driver。SV-GUI-01 の WebView/SolidJS 決定を supersede"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/ruviform/issues/384"
    - "https://github.com/scarecrow173/ruviform/issues/386"
    - "https://github.com/scarecrow173/ruviform/issues/388"
    - "https://github.com/scarecrow173/ruviform/issues/389"
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
    - "docs/specs/sv-gpui-01-native-app-shell.md"
  blocks: []
  supersedes:
    - "docs/adr/sv-gui-01-app-crate-webview-feature-vendored-ui-pipeline.md"
  superseded-by: []
  related:
    - "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
    - "docs/adr/sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md"
    - "docs/designs/sv175-native-shell-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ADR: Native App Shell — Driver-neutral shell + GPUI 標準 Driver

## Status

Accepted（初期 slice 実装に先行・2026-11-07）

## Context

- Issue #175（SV-GPUI-01・2026-11-07 横断レビュー確定仕様 v2）により、
  製品 Desktop Native GUI の標準方針が **「Ruviform 統一 GUI +
  GPUI native + Plugin Dock + Theming」**、初回 scope が
  **「Driver-neutral shell + GPUI Driver + Plugin Dock adapter +
  Theme + minimal Vertical Slice + Save/Reload/lifecycle + 後継
  ADR」** と確定した。
- 現行の製品 GUI は SV-GUI-01（`docs/adr/sv-gui-01-app-crate-webview-
  feature-vendored-ui-pipeline.md`・accepted）の WebView 経路:
  `synvelis-app` は headless lib、`webview` feature + `synvelis`
  bin が `ruviform-ui-webview`（wry/tao）host・`rust-embed`・
  vendored SolidJS/TS frontend（`ui/`）を組み立てる。
- 確定方針では **GPUI は標準 native Driver であって製品 domain
  API ではない**。製品 View は Driver-neutral な共通 Component/
  Binding 契約で書き、GPUI/OS 詳細は standard Driver 実装と
  composition root、明示的な driver extension に隔離する。
- Ruviform main `e8adcc09` に既に RF385（Theme resolver・
  `ResolvedTheme`・DTCG document）、RF386（`WindowDriver`/
  `WindowSpec`/`DriverEvent`/`WindowCapabilities` port +
  `GpuiDriver` + `IntentSink` + `UiRuntime` routing）、RF388/RF399
  （`NativeSurfaceSlot`/`SurfaceSlotRegistry`/`PluginEditorSession`/
  `GpuiSurfaceAdapter`）、RF390（boundary guard・driver lab・
  `FakeDriver`）が存在する。RF387 Component/View renderer は別
  worker が実装中で、最小 reviewed API は coordinator 共有待ち。
- 旧 WebView/TS/SolidJS 実装資産と Headless 契約（`syn.*` op 面・
  `synvelis-workspace` の `CommandIntent`/`DispatchContext`/
  `CommandReceipt`/`BindingSnapshot`・save/reload Store）は履歴・
  検証資産として保持する。

## Considered Options

- **A. WebView/SolidJS 継続（SV-GUI-01 踏襲）**: 既存 `synvelis` bin・
  vendored frontend・wry/tao host を延長する。既知の描画・IME・
  native surface 制約が残り、RF384 v2 で確定した GPUI 方針と矛盾。
- **B. 独自 toolkit 直結**: GPUI を含む特定 toolkit 型を domain/
  view 層へ直接露出する。backend 交換可能性・headless 検証性を失う。
- **C. Driver-neutral Native Shell（採用）**: Ruviform `WindowDriver`
  port のみに依存する shell + typed Binding adapter を `synvelis-app`
  に置き、GPUI 組立を `native` feature/bin に隔離する。FakeDriver
  headless 検証と将来の backend 追加を両立する。

## Decision

1. **製品 GUI 方針を SV-GUI-01 の WebView/SolidJS から Driver-
   neutral Native Shell（標準 Driver = GPUI）へ移行する。**
   `sv-gui-01` を `superseded` へ更新し本 ADR から `supersedes` で
   参照する。旧 ADR・旧実装・`ui/` frontend・PoC 証拠は削除せず、
   `webview` feature は Native 実証まで残置される preview lane と
   位置づける。
2. **`synvelis-app`（製品 composition root）が Native shell と
   Binding adapter を持つ headless lib を維持し、GPUI 組立だけを
   `native` feature + `synvelis-native` bin に閉じ込める。**
   feature gate は webview lane と同型（optional dep + bin のみ
   required-features）。lib 本体は GPUI/OS/DOM/WebView 型を持たず
   `WindowDriver` port にのみ依存する。
3. **`DriverEvent` → `UiRuntime` → `RoutedEvent` → `WindowDriver::
   post_event` の routing は shell が責務を持つ。** close は
   RF386 契約どおり vetoable（`Approve`/`Veto`/`Defer`）、初期
   slice は `Approve`。window 毎の view attach 対応を shell が
   追跡し、`Destroyed` 時に `syn.view.close` で backend 側
   binding/subscription を teardown する。
4. **non-RT typed ViewModel↔Ruviform Binding adapter を `synvelis-
   app` に置く。** native view は `syn.view.*`/`syn.control.*`/
   `syn.project.*`/`syn.transport.*`/`syn.app.*` intent を生成し、
   inbound `Event` を typed notice へ demux する。binding
   lifecycle・gesture 状態機械・receipt ordering・generation
   guard の正本は backend `ControlsUi`（`synvelis-workspace`）の
   ままで、adapter は第二の domain truth を作らない。attach
   応答の `generation` を以後の `DispatchContext.generation` に
   適用する（RF#383 `ViewLink.attach` の generation seam 踏襲）。
5. **依存方向を強制する。** `gpui`・`ruviform-ui-gpui`・OS
   window 型・DOM/WebView 型は composition root 例外（`native`
   feature）と ruviform-ui-gpui 内のみ。`check-headless-deps`
   の EXACT リストに `gpui`/`ruviform-ui-gpui`/`ruviform-ui-
   webview`/`wry`/`tao` を固定し、headless crate への混入を
   構造的に拒否する。RT コードから GUI 依存・blocking UI
   操作を禁止する。
6. **Ruviform 供給の共通契約を消費し、Synvelis で複製しない。**
   `ResolvedTheme`/DTCG 解釈（RF385）、`WindowDriver`/
   `DriverEvent`/`WindowCapabilities`（RF386）、`ViewSpec`/
   Component スキーマ（RF387・候補待ち）、`NativeSurfaceSlot`
   系（RF388/RF399）は Ruviform 側の正本を使う。
7. **最小 Vertical Slice（Tab+Fader+gesture 往復+save/reload+
   close）は RF387 実 API への接続を完了条件とする。** 候補が
   未入手の間は shell/lifecycle/Binding adapter を実 API で先行
   実装し、renderer 接続部分を `NotRun` として必要契約を固定
   する。仮 ViewSpec/stub renderer での完了報告は行わない。
8. **実 OS/GPU/IME/focus/実 plugin dock の受入は観測証拠を
   必須とする。** `FakeDriver` + `GpuiDriver` の same-contract
   検証で通る契約は Pass、未観測の native 挙動は `NotRun`/
   `Unsupported`（`GpuiDriver.capabilities` の `ime`=
   Unsupported、`native_surface_slot`=backend 依存）を明記する。

## Consequences

- 製品 GUI の新規実装は Driver-neutral 契約で書き、`synvelis-
  native` bin が GPUI Driver を組み立てる。`synvelis`（webview）
  bin・`ui/` frontend は Native 実証まで preview lane として残る。
- `synvelis-app` lib は GUI driver 非依存のまま headless 検証可能
  （`FakeDriver`）。ネイティブ lane は `check-app-native`/`lint-
  app-native` で別に検証する。
- `sv-gui-02`（binding IPC surface）の `syn.*` wire 契約はそのまま
  継承する（WebView 固有の custom-elements 側は superseded lane）。
- `Cargo.lock` の ruviform rev を main `e8adcc09` 系へ更新し、
  `sync-ui-deps`→`build-ui` で vendor/manifest REV を整合させる。
- RF387 renderer 入手後に最小 slice 接続と `synvelis-native` 実機
  起動観測を後続 gate として行う。
