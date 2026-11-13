---
id: "ADR-WLmD48s7O730hUCAvB65qT"
type: "adr"
status: "accepted"
title: "CTL-02 adoption 経路 — 非 canonical SendLevel は reject で抑止"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# CTL-02 adoption 経路 — 非 canonical SendLevel は reject で抑止

## Status

accepted

## Context

`ProjectOp::AddSend` / `SetSendGainTarget` の adoption は apply 時に
採用 target の `owner`/`role` のみを `Send`/`SendLevel` に書き換え、
`kind`/`mapping`/`time_scope`/`capability` は書き換えない。
canonical spec を持たない自由形式 target を adopt すると、CTL-02
契約（`canonical_internal_spec` が定める `Send`+`SendLevel` の
`Continuous`/`Linear`/`ProjectMusical`/`capability: None`）に
反する `SendLevel` が persisted され得る。

先行 slice（PR#350）は `AddParameterTarget` の write-time
validation で同契約を fail-closed に強制した。adoption はその
op を通らず既存 target を再所有するため、同じ契約をどこで・
どう担保するかを決める必要がある。争点は 2 つ:

1. **reject か正規化か** — 非 canonical spec を持つ target の
   adopt を typed error で拒否するか、apply 時に spec を canonical
   値へ書き換えるか。
2. **検査の位置** — validate（pre-apply）か apply か、
   `ProjectOp` か `routing_tx` staging か。

## Considered Options

* Option 1(採用): validate 時に reject。`AddSend` /
  `SetSendGainTarget` の `gain_target: Some` 検査に、採用対象が
  `Send`+`SendLevel` canonical spec を既に満たすことを要求する
  検査を追加し、違反は `Error::Validation`。apply は owner/role
  の seal のみを行い spec は書き換えない。
* Option 2: apply 時に正規化。owner/role と同時に kind/mapping/
  time_scope/capability を canonical 値へ上書きする。却下 —
  `capability: Some` は plugin `ParamId` binding を黙って捨て、
  `time_scope` の書き換えは automation の評価座標を黙って変え、
  `mapping`/`kind` の書き換えは値域の意味を変える。authored
  spec の silent な改変は「reject して caller に直させる」より
  危険であり、CTL-02 の fail-closed write-time 契約とも不整合。
* Option 3: `routing_tx` staging のみで検査。`ProjectOp` 直接経路
  （低層 op・既存の `SetSendGainTarget` caller）が無防御のまま
  残る。却下 — 契約の authoritative な実装は op 層に置くべきで、
  transaction は op に stage するため op 検査が両経路を覆う。
* Option 4: `validate_references`（load 時 oracle）で persisted
  形状を矯正/拒否。却下 — load oracle は既存データの寛容な受理が
  役割であり、write-time の契約強制とは責務が異なる。既存
  persisted 形状への対処は別 slice の判断事項。

## Decision

1. **validate 時に reject する（fail-closed）。** 採用対象 target
   の persisted spec が `Send`+`SendLevel` canonical 契約
   （`kind == Continuous`、`mapping == Linear`、
   `time_scope == ProjectMusical`、`capability == None`）を
   満たさない `AddSend`/`SetSendGainTarget` は `Error::Validation`
   で拒否し、project は無変更。`name`/`base` は契約対象外で
   authored のまま。
2. **検査は `ProjectOp::validate` に置く。** apply は従来どおり
   owner/role の seal のみを行う。`RoutingEdit` 経路は
   `staging.apply_op` が `op.validate` を通すため、transaction
   経路・低層 op 経路の双方に同一契約が atomic に効く。
3. **canonical spec の定義は `control.rs` に単一 source を置く。**
   `canonical_internal_spec`（`AddParameterTarget` 検査と共有）
   と採用検査が同じ `(kind, mapping, time_scope)` 定数を参照し、
   契約が 2 箇所に分岐しない構造にする。
4. **`ProjectAbsolute` の扱いを狭めることを明示する。** 旧来の
   adoption 検査は `ProjectMusical | ProjectAbsolute` を許したが、
   canonical `SendLevel` は `ProjectMusical` 固定であるため、
   `ProjectAbsolute` target の adoption は本 slice で reject
   される。これは契約の穴を塞ぐ意図的変更であり、暗黙の緩和を
   残さない。

## Consequences

- 非 canonical spec を持つ target の adopt は typed reject となり、
  caller は spec を canonical に修正するか `SendGainSpec::Create`
  （canonical target を同一 transaction で発行）を使う。
- `capability: Some`（plugin `ParamId` binding）を持つ target の
  adopt も reject される — plugin param を send gain に転用する
  経路は既に Strip/Processor owner guard で塞がれており、自由形式
  owner の capability 持ち target も同契約で拒否される。
- 既存の canonical adopt（`SendGainSpec::Adopt` で canonical spec
  の自由形式 target、`SetSendGainTarget` での Send-owned canonical
  `SendLevel` の再 bind、`SendGainSpec::Create` staging）は
  不変で green。
- apply の owner/role seal は不変 — persisted される `SendLevel`
  は常に canonical spec を持つため、`canonical_internal_spec`
  が定義する形状と adoption 結果が一致する。
