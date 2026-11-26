---
id: "SPEC-Oisc4bt6LMiyM1toYOasb8fV"
type: "spec"
status: "approved"
title: "SV-GPUI-02 PluginDock native slot — shared driver-neutral surface authority + GPUI-simulated binding (synvelis#176)"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/176"
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
    - "docs/specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gpui-02-plugin-dock-native-slot-design.md"
    - "docs/test-specs/sv-gpui-02-plugin-dock-native-slot.md"
    - "docs/specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "docs/designs/sv-gpui-02-plugin-dock-headless-panel-design.md"
    - "docs/test-specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
---

# SV-GPUI-02 PluginDock native slot — shared driver-neutral surface authority + GPUI-simulated binding

## What Should Be Built

Issue #176（SV-GPUI-02）の **S2 structural slice**。先行
`sv-gpui-02-plugin-dock-headless-panel` の spec/design/test-spec は、
PluginDock の製品 lifecycle（Open/Hide/Show/Focus/Resize/ReDock/
Close/Save/Restore、window loss、instance termination、論理
persist、DSP/Project/RT 非干渉）の baseline として残る。本 spec は
それらを supersede しない。変更するのは、native surface slot の
**ownership/authority boundary** だけである。

現行 headless slice では `PluginDock` が panel-private の
`SurfaceSlotRegistry` を抱え、slot/lease の authority が panel 内部に
閉じている。S2 ではこの authority を取り除き、slot declaration、
lease mint、validation、release、geometry、visibility、busy、event
drain を、composition root または test fixture が注入する **1 つの
共有 driver-neutral authority** へ通す。`PluginEditorSessions` は
引き続き app 共有の session registry であり、panel へ移動しない。

GPUI-simulated lane では、独立した fresh adapter ではなく
`GpuiDriver::surface()` が返す canonical shared ledger を使い、
mount 済みの製品 PluginDock ViewSpec に対して
`GpuiDriver::bind_native_slot(window, ComponentKey("plugin.editor"),
slot)` で `plugin.editor` NativeSlot と live slot を結ぶ。positive
evidence は semantic/layout projection のみであり、実 OS/native dock
証拠ではない。

## Requirements

### 先行 headless panel との関係

- 本 slice は structural follow-up であり、先行 headless panel docs の
  supersedes ではない。front matter の `supersedes` / `superseded-by`
  は空のままとする。
- 先行 docs が定める製品 lifecycle、4-ID 所有連鎖、app 共有
  `PluginEditorSessions`、adapter 拒否の正直な `Failed`/`Unavailable`
  着地、論理 persist、DSP/Project/RT 非干渉は維持する。
- S2 が変えるのは slot/lease authority の所在だけであり、panel の
  session lifecycle や `syn.plugin_dock.*` op 語彙を再設計しない。

### 共有 driver-neutral surface authority

- `PluginDock` は `SurfaceSlotRegistry` を panel-private に構築・保有
  しない。slot declaration、lease mint、validation、release、
  geometry、visibility、busy、event drain は全て注入された共有
  authority に対して行う。
- 共有 authority は composition root が製品配線用に、test fixture が
  headless 検証用に注入する。panel が独自 ledger を生成して driver
  ledger と分岐してはならない。authority 未注入時は panel-private
  fallback を作らず、`unavailable` に正直に拒否する。
- `PluginEditorSessions` は app 共有（`UiOpsState::plugin_sessions`
  相当）のままとし、panel 内へ移動しない。`SessionBusy`
  （1 `PluginHandle` = 1 editor）の panel 横断保証を弱めない。
- window 跨ぎ再配置、dock 2-phase busy、window 喪失、close/release の
  既存 lifecycle は、panel-private ledger ではなく共有 authority の
  typed error/event に従って進める。
- stale/revoked/forged/generation-mismatch lease は共有 authority が
  `LeaseRevoked`/`InvalidLease`/`StaleGeneration`/`UnknownSlot` 系で
  拒否する。製品側で成功へ再判定しない。

### Lease redemption seam

- `ParentSlotLease` は session side で opaque である。session、wire、
  panel は payload の bit layout を解釈しない。
- issuer-side redemption は、session が運ぶ raw payload を
  `SlotLease::new(raw)` として **同じ共有 authority** に照合し、live
  lease にだけ parent token を発行する。
- upstream real-driver shape に従い、redeemed `ParentLease` は
  `ParentLease::new(SlotLease::new(raw).seq(), token)` の形を取る。
  `generation` には lease sequence、payload には redemption が返す
  parent token を入れる。
- slot index を実 OS parent token として扱わない。slot index、
  `SurfaceSlotId` generation、lease payload の index bits を HWND や
  NSView 等へ流用しない。fake/headless parent token は test-only の
  値であり、実 OS token とみなさない。
- stale/revoked/forged/unknown payload は redemption 段階で失敗し、
  `ParentLease` を mint しない。

### GPUI-simulated lane

- slot は `GpuiDriver::surface()` の canonical shared ledger に宣言
  する。`GpuiSurfaceAdapter::new` 等で fresh independent adapter を
  作り、renderer/bind validation から見えない ledger を使わない。
- `GpuiDriver::bind_native_slot(window, ComponentKey("plugin.editor"),
  slot)` は、mount 済みの製品 PluginDock ViewSpec 内の `plugin.editor`
  NativeSlot をその live slot に結ぶ。
- positive evidence は semantic/layout projection（bound component の
  layout に応じた geometry/visibility projection）に限定する。
- unknown slot、stale/released slot、different-window slot、
  already-bound key/slot は bind 時に拒否されることを検証する。
- `gpui-test` では raw parent capture がない。したがって native
  container、実 child HWND、pixel、input、IME、popup、PD-01〜07 の
  verdict をこの lane から主張しない。
- macOS/Linux backend の `Unsupported` は定義済み拒否 expectation で
  あり、silent skip や未実行の成功扱いにしない。

### 判定語彙と正直性

- `Pass`/`Fail`/`NotRun`/`Unsupported`/`Deferred` と driver lane の
  `GpuiSimulated` は混同しない。`GpuiSimulated` は verdict ではなく、
  toolkit-simulated driver lane の識別子である。
- `Unsupported` は根拠を伴う技術的非対応、`NotRun` は未実行、
  `Deferred` は要求 activation の保留である。環境不在や省略を
  `Unsupported` に書き換えず、`Deferred` を `Pass`/`NotRun` に潰さない。
- headless/fake/GPUI-simulated の positive evidence を OS/native dock
  の `Pass` に昇格しない。acceptance ledger の行 1/7 は変更しない。

## Out of Scope

- 実 OS window、実 child HWND/native container、pixel、input、IME、
  popup、focus の実機検証。
- PD-01〜07 の verdict 判定、および acceptance ledger 行 1/7 の変更。
- production L1 host wiring（実 plugin ABI editor を製品 host 経路へ
  常設接続する工程）。
- `ruviform#631` real scenario JSONL の採用・実走。
- Ruviform pin bump、`Cargo.lock`/`mise`/dependency の変更。
- Issue #176 の close。
- 本 slice の範囲を超える PluginDock session lifecycle、op 語彙、
  persist format の再設計。
