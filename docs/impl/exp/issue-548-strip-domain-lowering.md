---
id: "EXP-548StripDomLowr09"
type: "experiment"
status: "completed"
title: "Issue #548 impl EXP — strip-domain lowering 実験記録"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/548"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #548 impl EXP — strip-domain lowering 実験記録

## E1: capability 集合の選定

- **試行**: mask-gated op 全種（`AddParameterTarget`・
  automation lane/region・modulation 系を含む）を網羅するか、
  代表最小集合に絞るか。
- **結果**: 5 件（set-mute/set-solo/insert-processor/
  param.set-base/processor.set-binding）を採用。
  `AddParameterTarget` 系は wire schema が重く domain 検証
  責務も別のため見送り。automation/modulation 系は別 domain
  の positive 化と一緒に扱うべきで本 slice 対象外。
- **根拠**: Issue の AC は「positive mask-gated edit が
  candidate edit として受理される」こと — 5 件で strip・
  param target・processor の 3 resource kind をカバーし、
  channel bool・insert・rebind・base write の代表面を押さえる。

## E2: `op_facts` arms の要否

- **試行**: facts を足さず capability のみ登録する案も
  検討したが、`op_specs()` の `lower_for` expect は通る
  ものの ordinary prepare で provisional_gate の SEC-14
  unsupported 検査に常時引っかかり fail-closed になる
  半状態になることが分かった（variant inner op 経路は
  `declared_facts` が inner.unsupported を伝播しないため
  動作するが、直接 prepare は常時失敗）。
- **結果**: 設計判断1どおり reads + preconditions を正直に
  宣言する arms を追加。channel `ParameterTarget` は
  owner+role で既存分のみ read に追加（first-use create
  は apply 時発見の内部 read）。

## E3: 狭 mask vs 全 mask の試験設計

- **試行**: 全 5 field mask で全 op を positive にする案と、
  狭 mask（`inserts` 未宣言）で mask 負側も pin する案。
- **結果**: 両方採用 — 狭 mask 試験で channel 系 3 件の
  positive + inserts 系の `variant-unsupported` 負側 +
  master strip の `variant-scope-violation` + 非 strip
  `variant-unsupported` を pin し、全 mask 試験で
  insert/rebind 2 件の positive を別途証明。mask-gated
  の両側（宣言内受理・宣言外拒否）を同時に証拠化した。

## E4: `ProcessorBinding` wire decode

- **試行**: `arg_typed::<ProcessorBinding>` の serde 形を
  `{"binding": "Unresolved", "descriptor": "..."}` と推定。
- **結果**: cb06 fixture の `ProcessorBinding::Unresolved`
  と同形の JSON で decode 成功を試験で確認
  （`strip_insert_processor_op`/`processor_set_binding_op`
  が candidate edit として受理）。

## 未解決・持ち越し

- routing/send/connection/sidechain/bus op の candidate
  受理と scope entity removal は `synvelis-project` の
  mask/adapter 変更を要し別 slice（Coordinator が
  missing_issue として記録済み）。
- 実 PCM/RT/plugin・実 GUI lane は `NotRun`。
