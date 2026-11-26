---
id: "DESIGN-JCeMt2CNWWebw7vrpDhnTJgD"
type: "design"
status: "approved"
title: "SV-GPUI-02 PluginDock native slot 詳細設計 — shared surface authority・opaque lease redemption・canonical GPUI binding"
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
  implements:
    - "docs/specs/sv-gpui-02-plugin-dock-native-slot.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "docs/designs/sv-gpui-02-plugin-dock-headless-panel-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gpui-02-plugin-dock-native-slot.md"
    - "docs/test-specs/sv-gpui-02-plugin-dock-native-slot.md"
    - "docs/specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "docs/test-specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "evidence/sv176/acceptance-ledger.md"
---

# SV-GPUI-02 PluginDock native slot 詳細設計

## 構成

```
StablePluginInstanceId ──resolve──► PluginHandle ──open──► PluginEditorSessionId
        ▲                                                        │
        │                                          opaque ParentSlotLease
        │                                                        ▼
   PluginDockBinding ◄── ViewInstanceId ── SurfaceSlotId / SlotLease

Composition root / test fixture ──injects──► shared surface authority ◄──slot/lease ops── PluginDock panels
                                                      ▲
                                                      └── GPUI lane: GpuiDriver::surface()（canonical shared ledger）

shared surface authority operations:
  declare / lease / validate / release / geometry / visibility /
  busy / drain_events / issuer-side redeem

UiOpsState ──owns──► PluginDock panels
UiOpsState ──owns──► app-shared PluginEditorSessions（panel へ移動しない）
```

- 先行 headless panel design が定める 4-ID 所有連鎖と session
  lifecycle は baseline として維持する。本設計は supersede ではなく、
  slot/lease authority の所在だけを panel-private から共有 authority
  へ移す structural follow-up である。
- `PluginDock` は live binding として `SurfaceSlotId` や lease への
  参照を保持し得るが、`SurfaceSlotRegistry` そのものを construct・
  own しない。slot/lease の真実は常に注入された共有 authority にある。
- 共有 authority は composition root または test fixture が注入する。
  GPUI lane では `GpuiDriver::surface()` が返す canonical
  `SharedSurface`、headless lane では test fixture が用意する共有
  driver-neutral implementation を使う。panel が fresh registry や
  fresh adapter を作って driver ledger と分岐しない。authority 未注入
  時は panel-private fallback を作らず、op 層が `unavailable` に
  正直に拒否する。
- `PluginEditorSessions` は引き続き app 共有
  （`UiOpsState::plugin_sessions` 相当）とする。panel 内へ移動
  しないため、`SessionBusy`（1 `PluginHandle` = 1 editor）の panel
  横断保証は変わらない。
- `syn.plugin_dock.*` op 語彙、dock 2-phase seam、論理 snapshot、
  DSP/Project/RT 非干渉は先行 headless slice の契約を使う。S2 は
  それらの操作が叩く slot/lease API の向き先を共有 authority に
  統一する。

## Lease redemption seam

`ParentSlotLease` は session side で opaque である。panel、session、
wire は payload を解釈せず、issuer-side redemption だけが次の手順で
parent lease を mint する。

1. 共有 authority が `lease_parent(slot)` で `SlotLease` を mint し、
   panel はその raw payload を `ParentSlotLease::new(lease.raw())`
   として session へ渡す。
2. host adapter は session から受け取った `ParentSlotLease` を issuer
   side redeem hook へ渡す。hook は `raw` を取り出し、
   `SlotLease::new(raw)` を **同じ共有 authority** に照合する。
3. live lease にだけ authority が parent token を返す。real driver
   では token は native container の raw OS parent token、headless
   fake では test-only token である。panel や session が token を
   自作しない。
4. adapter は upstream real-driver shape に従って
   `ParentLease::new(SlotLease::new(raw).seq(), token)` を mint
   する。`generation` は lease sequence であり、`SurfaceSlotId` の
   generation ではない。
5. stale/revoked/forged/unknown payload は validation/redemption で
   失敗し、`ParentLease` を mint しない。失敗を adapter 側で成功へ
   再判定しない。

slot index は実 OS parent token ではない。lease payload に packed された
index bits、`SurfaceSlotId::index()`、`SurfaceSlotId::generation()` を
HWND/NSView 等へ流用しない。fake/headless token は test fixture が
assert するためだけの値であり、native parent handle の証拠にしない。

## GPUI canonical binding

- slot は `GpuiDriver::surface()` の canonical shared ledger に宣言
  する。`GpuiSurfaceAdapter::new` で independent ledger を作り、
  renderer の projection や `bind_native_slot` validation から見え
  ない slot を使わない。
- 製品 PluginDock ViewSpec（upstream 語彙の `plugin.editor`
  NativeSlot を含むもの）を mount した window に対し、
  `GpuiDriver::bind_native_slot(window, ComponentKey("plugin.editor"),
  slot)` で component key と live slot を結ぶ。
- bind 後、mounted view は layout 上の bounds と表示状態を共有
  authority へ projection する。S2 が positive evidence とするのは
  この semantic/layout projection だけである。
- `unbind_native_slot` 後は、次に描画される frame で bound slot が
  suppress（`Hidden`）へ投影されることを期待する。unbind は slot
  release と同一ではなく、release は consumer が共有 authority に
  対して行う。
- unknown slot、stale/released slot、different-window slot、
  already-bound key/slot は bind 時に拒否する。これらを silent no-op
  にしない。
- `gpui-test` では raw parent capture がない。GPUI-simulated lane の
  positive evidence は semantic/layout projection に限定し、native
  container、実 child HWND、pixel、input、IME、popup、PD-01〜07 の
  verdict を主張しない。
- macOS/Linux backend は native surface slot backend を持たず
  `Unsupported` が定義済み拒否である。test は skip で黙殺せず、
  その refusal expectation を観測する。

## 状態遷移との関係

先行 headless panel design の状態遷移を維持し、slot/lease 操作だけを
共有 authority へ向ける。

- `open`/`restore` — slot ensure/declare、lease mint、session open の
  順序は baseline に従い、lease validation/redemption は共有
  authority に照合する。失敗は正直に `Failed`/`unavailable` 系へ
  着地する。
- `hide`/`show` — session visibility と slot visibility の対応は
  baselineどおり。slot visibility 更新は共有 authority に記録する。
- dock 2-phase — prepare で busy mark、commit/reattach で relocate、
  abort で busy clear を行う対象は panel-private registry ではなく
  共有 authority である。
- window 跨ぎ再配置 — 新 slot/lease を共有 authority に mint し、
  reattach 成功後にのみ旧 slot/lease を解放する baseline を維持する。
  adapter 拒否時に旧 parent seat の保持を偽装しない。
- window loss / instance termination / close — release、lease revoke、
  terminal 収束は共有 authority の typed error/event に従う。
- event drain — slot lifecycle event は共有 authority が drain する
  ものを観測し、panel 内の複製 ledger で再構成しない。
- save/restore — persist は論理状態のみで、session id・slot id・
  lease・OS handle を含めない baseline を変更しない。

## エラー写像

- `LeaseRevoked`/`InvalidLease`/`StaleGeneration`/`UnknownSlot` は
  共有 authority の typed refusal として扱い、panel/op 層で成功へ
  再判定しない。
- `Unsupported` は定義済み非対応、`Backend` は操作が定義されている
  が backend が失敗した場合として区別する。macOS/Linux backend の
  `Unsupported` を silent skip にしない。
- adapter/host 未注入、`EditorOpError::Unsupported`、ops lend 拒否の
  `Failed`/`unavailable` 写像は先行 headless panel design に従う。
- GPUI-simulated lane の report は driver lane として `GpuiSimulated`
  を記録し得るが、これは verdict ではない。`Pass`/`Fail`/`NotRun`/
  `Unsupported`/`Deferred` と混同しない。

## RT 境界

`PluginDock`、共有 surface authority、lease redemption、GPUI binding は
全て UI スレッドの非 RT 経路である。audio/RT 経路、project authority、
DSP engine generation には触れない。

## 正直性の設計上の要点

- positive GPUI evidence は semantic/layout projection のみであり、
  native dock・window・pixel・input・IME・popup の証拠ではない。
- headless/fake parent token は test-only であり、実 OS parent token
  の証拠にしない。
- 本設計は acceptance ledger 行 1/7、PD-01〜07 の verdict、production
  L1 host wiring、`ruviform#631` real scenario JSONL、Issue close を
  変更しない。
