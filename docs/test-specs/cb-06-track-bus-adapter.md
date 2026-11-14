---
id: "TSPEC-034XpajVZWjKQ7G6z21j8o"
type: "test-spec"
status: "draft"
title: "CB-06 Track/Bus Strip 候補 Adapter — mask・routing 参照保護・adopt"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-05-fx-range-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CB-06 Track/Bus Strip 候補 Adapter — mask・routing 参照保護・adopt

## Purpose

Track/Bus の Creative Branch 候補が `StripDomainMask` で宣言した
field family のみを編集・比較・採用できること、および strip に
接続する外部 routing（main route・send・sidechain feed）が
候補編集で無断に張り替えられず、adopt も宣言範囲だけを
materialize することを保証する。この spec は Issue #283 の
`CB-STRIP-01`〜`CB-STRIP-05` と `CB-STRIP-RCP-01` の headless
検証意図を固定する。実 plugin/PCM/OS/RT の受入は対象外であり、
それらの保証が必要になった時点で別 test-spec が必要。

## Feature

Track/Bus Strip 候補 Adapter の mask 適合編集と routing 参照保護

## Rules

- Rule: `VariantScope::StripFields` は安定 `StripId` と非空
  `StripDomainMask` で scope を宣言し、Track/Bus どちらの owner
  の strip も同一の `StripId` で解決される。UI 上の「Track」
  表示が strip を二重インスタンス化しない（CB-STRIP-01）。
- Rule: 候補編集は宣言 mask の field family に対応する op のみ
  受理する。`SetStripMute`/`SetStripSolo` は対応 field 宣言時
  のみ、channel `ParameterTarget`/automation/modulation op は
  scope-owned target のみ、`InsertProcessor` は `Inserts` 宣言時
  のみ受理する。mask 外 field・scope 外 entity・外部 routing op
  は `Unsupported`/`ScopeViolation` の型付き拒否であり、silent
  normalize や scope 拡大はしない（CB-STRIP-02/03）。
- Rule: scope 所有は `scope_objects` が正本 — strip entity、
  channel field の singleton target + automation/modulation
  closure、Inserts 宣言時の `ProcessorChain` entity
  （processor 順序を含む）・processor/port/control
  closure/完全内部 connection。strip port・send・外部
  connection・bus・他 strip は共有 context であり候補が
  書き換えない（CB-STRIP-02/03）。
- Rule: modulation assignment は effect target（および
  depth_source）が scope 所有のときのみ scope 所有 —
  scope 所有 source から mask 外 target への既存 assignment
  の `Set`/`RemoveModulationAssignment` は型付き拒否される
  （CB-STRIP-02）。
- Rule: `ProcessorChain` の processor 順序は scope identity
  の一部 — intent 取得後に live 側で chain reorder が起きると
  scope digest が変わり adopt は `StaleScope` で型付き拒否
  され、候補でも live でもない順序が生成されない
  （CB-STRIP-04）。
- Rule: strip-affiliated `SemanticPortId` endpoint を持つ
  connection（main route・send・sidechain feed・strip↔
  processor 混在）は `boundary_dependencies` が typed
  inventory する。境界を跨ぐ配線変更は shared-context digest
  を動かし比較 context を stale にする — false equivalence を
  作らない（CB-STRIP-03/RCP-01）。
- Rule: adopt は宣言 mask 内の全 field を 1 Project CAS/Undo
  で materialize し、対象外 strip/bus/track・外部 routing・
  scope 外 entity を変更しない。stale scope digest・missing
  plugin・recording active は typed reject で部分変更なし
  （CB-STRIP-04/RCP-01）。
- Rule: Bus/Return/Main/Solo/Automation の既存挙動は SF-06
  Oracle と同一 model を共有し回帰しない（CB-STRIP-05）。

## Examples

- Example: mask=`{Mute}` の track strip 候補で `SetStripMute`
  は受理され Mute target の base が変わるが、`SetStripSolo`・
  `SetParameterBase`(Fader target)・`InsertProcessor`・
  `AddSend`・`AddConnection` は全て型付き拒否される。
- Example: mask=`{Fader, Inserts}` の bus strip 候補で chain
  への `InsertProcessor` と processor param の
  `SetParameterBase` は受理されるが、`ReorderProcessor`・
  `RemoveProcessor`・send tap 変更は拒否される。
- Example: scoped strip の main output→master 入力や
  strip→return bus の send、key strip→scoped processor の
  sidechain feed は `boundary_dependencies` に列挙され、
  live 側でそれらを変更すると scoped/shared digest が動き
  adopt の scope digest 照合が stale で拒否される。
- Example: 2 つの独立 mask の strip 候補（例: `{Fader}` と
  `{Inserts}`）が別々に編集・adopt されても、互いの field・
  他 track の strip・外部 routing は不変のまま。
- Example: mask=`{Mute,Pan}` の候補で strip-owned LFO から
  未宣言 Fader target への既存 assignment は
  `Set`/`RemoveModulationAssignment` が `ScopeViolation` で
  拒否され、宣言済み Pan target への assignment は編集できる。
  宣言済み target でも `depth_source` が scope 外の assignment
  は非所有として同様に拒否される。
- Example: `{Inserts}` 候補の adopt intent 取得後に live 側で
  `ReorderProcessor` が適用されると、adopt は
  `VariantError::StaleScope` で拒否され live project は不変。

## Guarantees

- mask 外 field への候補編集波及が型付き拒否される。
- 外部 routing 参照（send/sidechain/main route）が候補で
  無断再接続されず、adopt が宣言範囲のみを書く。
- Track/Bus 両 owner で同一 `StripId` 解決・二重 DSP なし。
- capability 報告と enforcement が一致する。

## Non-goals

- Routing op（send/connection/sidechain/bus 編集）の候補受理
  — 別 Issue の Routing 拡張 scope。
- 実 plugin state の deep inspect、PCM/RT render 比較、
  GUI mixer surface、3-way merge。
- scope entity removal（processor/target/lane/source）の
  候補編集 — 本 slice では Unsupported。

## Risk

- mask 判定を `scope_objects` 以外で再実装すると所有権の
  drift で mask 外波及を見逃す。
- strip port を scope object と誤認すると外部 route が
  候補所有になり共有 context 保護が崩れる。
- boundary inventory の見落としは false equivalence（変化した
  外部 feed を同一視）を生む。
