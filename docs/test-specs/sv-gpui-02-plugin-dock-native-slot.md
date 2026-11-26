---
id: "TSPEC-iHM3SpiejBbfVhTbjCGjfdoT"
type: "test-spec"
status: "approved"
title: "SV-GPUI-02 PluginDock native slot テスト仕様 — shared surface authority + GPUI-simulated binding (synvelis#176)"
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
    - "docs/specs/sv-gpui-02-plugin-dock-native-slot.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gpui-02-plugin-dock-native-slot-design.md"
    - "docs/test-specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
  verifies:
    - "docs/specs/sv-gpui-02-plugin-dock-native-slot.md"
---

# SV-GPUI-02 PluginDock native slot テスト仕様

## Purpose

S2 structural slice が要求する shared surface authority と GPUI
canonical binding を検証する planned lane/case を固定する。対象 test
file は `crates/synvelis-app/tests/plugin_dock.rs`（既存 panel/op
tier）、`crates/synvelis-app/tests/plugin_dock_surface.rs`（authority
seam）、`crates/synvelis-app/tests/plugin_dock_contract_gpui.rs`
（`gpui-test` lane）を想定する。

本書は検証計画であり、実行結果や `Pass` を主張しない。実際の実行
コマンド、OS、toolchain、exit code、case 結果は `evidence/sv176` が
記録する。

## Verdict vocabulary

- `Pass`/`Fail`/`NotRun`/`Unsupported`/`Deferred` は独立に記録する。
  `Deferred` を `NotRun` や `Unsupported` へ潰さず、未実行を `Pass`
  にしない。
- `GpuiSimulated` は verdict ではなく driver lane の識別である。
  `GpuiSimulated` の positive evidence は semantic/layout projection
  のみを意味する。
- `Unsupported` は根拠を伴う技術的非対応である。macOS/Linux backend
  の native surface slot `Unsupported` は定義済み refusal
  expectation として観測し、silent skip にしない。
- headless/fake/GPUI-simulated の成功を実 OS/native dock/PD-01〜07
  verdict へ昇格しない。

## Cases

### Headless authority seam（`tests/plugin_dock_surface.rs` + `tests/plugin_dock.rs`）

| case | planned lane | 検証点 |
| --- | --- | --- |
| PD-NS-A01 | authority sharing | composition/test fixture が注入した 1 つの共有 authority に対して、slot declaration・lease mint・validation・release・geometry・visibility・busy・event drain が通る。panel-private ledger が独立した真実として残らない |
| PD-NS-A02 | issuer redemption shape | 有効な `ParentSlotLease` が同じ authority で redeem され、`ParentLease::new(SlotLease::new(raw).seq(), token)` の shape になる。generation は lease sequence、payload は redemption token であり、slot index を parent token にしない |
| PD-NS-A03 | revoked lease refusal | release/close/supersede 後の lease が redemption/validation で `LeaseRevoked` 系に拒否され、`ParentLease` を mint しない |
| PD-NS-A04 | stale/generation refusal | slot index recycle 後の stale id/lease、generation mismatch が `StaleGeneration` 系に拒否される |
| PD-NS-A05 | forged/unknown refusal | 未発行 sequence、malformed/forged payload、unknown slot index が `InvalidLease`/`UnknownSlot` 系に拒否される |
| PD-NS-A06 | busy refusal | dock 2-phase busy 中の新規 lease mint が `SlotBusy` 系に拒否され、clear 後に再び mint できる。live lease がある slot の二重 mint は `LeaseBusy` 系に拒否される |
| PD-NS-A07 | geometry/visibility/event drain | geometry/visibility 更新と `Declared`/`Leased`/`LeaseRevoked`/`GeometryChanged`/`VisibilityChanged`/`Destroyed` 系 event が共有 authority から観測できる。panel 内の複製 ledger で再構成しない |
| PD-NS-A08 | session ownership non-regression | `PluginEditorSessions` が app 共有のままで、panel 横断の `SessionBusy`（1 `PluginHandle` = 1 editor）を維持する。session registry は panel へ移動しない |
| PD-NS-A09 | fake token boundary | headless/fake redemption token は test-only と区別され、実 OS parent token や native container evidence として記録されない |

### Op-level non-regression（`tests/plugin_dock.rs`）

| case | planned lane | 検証点 |
| --- | --- | --- |
| PD-NS-O01 | full lifecycle op | 既存 `syn.plugin_dock.{open,hide,show,focus,resize,close,save,restore,state,list}` の lifecycle が共有 authority 化後も維持される |
| PD-NS-O02 | redock/refusal honesty | dock 2-phase、window 跨ぎ relocate、adapter 拒否時の旧 parent seat 保持と `Failed` 着地が baseline どおりである |
| PD-NS-O03 | lifecycle convergence | window loss、instance termination、close、save/restore が baseline の収束・論理 persist 契約を維持する |
| PD-NS-O04 | op isolation | plugin dock op が project/transport/DSP state に干渉しない。UI スレッド・非 RT 境界を維持する |
| PD-NS-O05 | error mapping | host/authority 未注入、adapter `Unsupported`、lease rejection、unknown/duplicate instance が既存の正直な error 写像を維持する。authority 未注入で panel-private fallback を作らない |

### GPUI-simulated lane（`tests/plugin_dock_contract_gpui.rs`）

| case | planned lane | 検証点 |
| --- | --- | --- |
| PD-NS-G01 | canonical surface ledger | slot が `GpuiDriver::surface()` の canonical shared ledger に宣言される。fresh independent adapter 側だけに存在する slot は bind validation で unknown/refused になる |
| PD-NS-G02 | product ViewSpec bind | mount 済みの製品 PluginDock ViewSpec に対して `GpuiDriver::bind_native_slot(window, ComponentKey("plugin.editor"), slot)` が `plugin.editor` NativeSlot を live slot に結ぶ |
| PD-NS-G03 | layout projection | active/painted な bound slot の layout bounds が canonical ledger の geometry/visibility へ projection される。positive evidence は semantic/layout projection のみと明記する |
| PD-NS-G04 | visibility/unbind suppression | inactive/unpainted 状態または `unbind_native_slot` 後に slot visibility が `Hidden` へ suppress される |
| PD-NS-G05 | bind refusal matrix | unknown slot、stale/released slot、different-window slot、already-bound key/slot が拒否される。silent no-op にしない |
| PD-NS-G06 | gpui-test evidence boundary | `gpui-test` に raw parent capture がないことを前提に、native container・実 child HWND・pixel・input・IME・popup・PD verdict を主張しない |
| PD-NS-G07 | macOS/Linux refusal | macOS/Linux backend の native surface slot op が定義済み `Unsupported` として観測され、silent skip や成功扱いにならない |
| PD-NS-G08 | report vocabulary | scenario report を使う case では driver lane `GpuiSimulated` と verdict が区別され、`NativePluginDock`/`VisualCapture` capability を偽装しない |

## Validation（planned lanes — canonical = mise）

| lane | planned command / 対象 | 記録 |
| --- | --- | --- |
| headless authority + op | `mise run test synvelis-app`（`tests/plugin_dock.rs`・`tests/plugin_dock_surface.rs`） | planned — 実測は `evidence/sv176` に記録 |
| GPUI-simulated | `mise run check-app-gpui` / `mise run lint-app-gpui` / `mise run test-app-gpui`（`tests/plugin_dock_contract_gpui.rs`） | planned — 実測は `evidence/sv176` に記録 |
| app hygiene | `mise run check synvelis-app` / `mise run lint synvelis-app` / `mise run fmt-check` / `mise run check-headless-deps` | planned — 実測は `evidence/sv176` に記録 |

本書作成時点ではいずれの lane も実行結果を主張しない。未実施の
case は `NotRun`、実装・環境が整っていない受入は要求状態に応じて
`Deferred`、技術的に定義された非対応だけを `Unsupported` とする。

## Non-claims

- 実 OS window、実 child HWND/native container、pixel、input、IME、
  popup、focus routing、PD-01〜07 verdict を主張しない。
- GPUI-simulated の layout/visibility projection を native dock pass、
  production L1 host wiring、`ruviform#631` real scenario JSONL の
  証拠へ昇格しない。
- headless/fake parent token を実 OS parent token とみなさない。
- acceptance ledger 行 1/7 を変更せず、Issue #176 close を主張しない。
- Ruviform pin bump、`Cargo.lock`/`mise`/dependency 変更を本 test
  spec の検証済み事項として扱わない。
