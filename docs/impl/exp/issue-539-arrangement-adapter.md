---
id: "EXP-539aArrngmntAdptr09"
type: "experiment"
status: "completed"
title: "Issue #539 experiment log — ArrangementAdapter 登録方式の検討記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/539"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-539-arrangement-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/designs/issue-539-arrangement-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-539-arrangement-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-539-arrangement-adapter.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #539 experiment log — ArrangementAdapter 登録方式の検討記録

## EXP-1: host 側個別配線 vs `standard()` 登録

- hypothesis: synvelis-cli・mcp-poc の host 側で個別に
  adapter を wire する方が影響範囲を限定できる
- result: rejected
- reason: `VariantPorts::standard` の consumer は両 host と
  test rig であり、登録を `standard()` に置けば自動伝播する。
  host ごとの配線は重複かつ将来の consumer 追加時に
  漏れの温床になる。wire 経路自体は既に汎用で、
  dispatch 追加以外の挙動変更は発生しない。

## EXP-2: `adapter_for` 特例 vs adapters map 登録

- hypothesis: `adapter_for` に Arrangement 特例分岐を
  追加する方が diff が小さい
- result: rejected
- reason: adapters map が scope→adapter の正本であり、
  特例分岐は map との二重管理になる。特例は
  fail-closed 契約（未配線 kind は `Unsupported`）を
  暗黙に弱める。map への登録が正しい境界。

## EXP-3: fail-closed 機構の削除 vs 維持

- hypothesis: 全 7 variant 網羅により `adapter_for` の
  `Unsupported` 経路は到達不能なので削除できる
- result: rejected
- reason: 将来 `VariantScopeKind` に新 kind が追加された
  際、map 未登録なら fail-closed で安全側に倒れる機構は
  防御として残すべき。機構維持・doc comment 更新のみ行い、
  挙動は変更しない。

## EXP-4: composite edit の構成（mute+move vs note ops 含む）

- hypothesis: clip-local note op まで含めた composite が
  より強い検証になる
- result: adopted（mute+move のみ）
- reason: CB-E2E-03 の gateway 経路契約は composite edit の
  蓄積・adopt・CAS/undo・scope 分離を要求し、mute+move の
  2 op で充足する。note op を含めるには `tests/rig/mod.rs`
  の demo_project fixture 拡張が必要で slice を不必要に
  広げる。project 軸の `cb11_e2e_03_*` が note op を含む
  より広い composite を既に担保している。
