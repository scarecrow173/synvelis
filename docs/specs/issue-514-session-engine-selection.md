---
id: "SPEC-5mnrHdz7IcQnkdbdZd6nsY"
type: "spec"
status: "approved"
title: "Issue #514 spec — SessionRoutingEngine の RCP selection 消費要否の確定"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
    - "docs/discovery/issue-514-session-engine-selection.md"
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
    - "docs/discovery/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-514-session-engine-selection.md"
    - "evidence/sv514/decision-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-514-session-engine-selection.md"
  derived-by:
    - "docs/designs/issue-514-session-engine-selection.md"
    - "docs/plans/issue-514-session-engine-selection.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #514 spec — SessionRoutingEngine の RCP selection 消費要否

## 目的

`SessionRoutingEngine`（synvelis-app）が RCP selection
（Missing Plugin Resolver / Reconciler の plan 選択・Safe Plan
Reconciliation）を消費する必要があるかを root-cause で確定し、
採否と根拠を canonical doc chain + evidence に記録する。
「Deferred」のまま残さない。

## 判定分岐と受入条件

### 分岐 A: 実装する（本 spec では不採用 — 判定は ADR 参照）

rebuild 経路を `resolve_selection` → `ExecutionSelector::select` →
`drive_selection_plan` に接続し、`SessionEngineConfig`/composition
経路で `SelectionInputs` の全 pin を供給する。SessionRoutingEngine
レベルで stale selection 非 publish・generation+epoch 不一致の旧
plan 非 Active・QueueFull/RetireOverflow last-good 保持を pin する。

### 分岐 B: 実装不要 — RoutingEngine レベルで責務が閉じる（採用）

以下を満たすこと:

- [ ] `SessionRoutingEngine` への RCP selection 配線について、
      「RoutingEngine レベルで責務が閉じる（実装不要）」が
      root-cause レベルで確定し、根拠が docs/evidence に記録される
- [ ] `RoutingEngine` レベルの headless oracle が app 層の責務を
      代替する設計上の根拠が記録される
- [ ] `SessionRoutingEngine` 側に未配線のまま残る経路が「意図的」
      と明記される（ADR + engine.rs module doc + evidence）
- [ ] stateless plugin `PluginStateDecl` 表現の消化有無を確認し、
      未消化なら本 Issue で対処するか別 Issue/owner への帰属を
      記録する
- [ ] `mise run test synvelis-app` / `mise run test synvelis-workspace`
      が pass し、`rcp_selection_integration` 等の既存 suite に
      回帰がない
- [ ] headless/fixture lane を本 Issue の Pass 判定とし、T3/T4 実
      device/plugin・RT 実測は別 gate（#109/#296/`scarecrow173/
      ruviform#630`/`scarecrow173/ruviform#631`）として NotRun
      記録する

## 判定材料（Coordinator 確定 — 全て調査済み）

(a) missing-plugin Blocked/substitution semantics が app 層で要求
されるか → **要求されない**。substitution は現契約で plan に反映
されず（`lower_semantic` は全 graph を lowering）、verdict を hard
gate にすると engine の ground-truth `connect` が build 可能な plan
を false-block し得る。

(b) `render.rs` の `RenderStatusScreenState.selection` との整合 →
live selection の producer は per-intent selection service が自然
（sv#295 面）。rebuild は intent 非依存の Realtime plan 実行経路。

(c) `RoutingDeps`/`SessionEngineConfig`/composition 経路で
`SelectionInputs` の全 pin を供給できるか → **供給不能**。
plugin_evidence・environments・render_candidates の production
供給経路が存在しない。

(d) 現行 rebuild が selection 非消費のまま契約を満たすか →
**満たす**。`EnginePort` 契約・SF-03 slice5 設計（honest input
供給 + lifecycle 転送、activation/last-good/stale/equivalence は
RoutingEngine 内）に適合し、Issue が要求する安全性 pin は
RoutingEngine レベルで既に pin 済み。

## 非目標

- `SessionRoutingEngine` rebuild 経路の挙動変更（selection 配線を
  含む）
- `synvelis-player` の Resolver/Reconciler/`PluginStateDecl`/
  `verify.rs` の変更
- `synvelis-project` の variant/domain adapter 契約
- T3/T4 実 device/plugin・RT 実測
- GUI 表示面（sv#114/sv#295）
