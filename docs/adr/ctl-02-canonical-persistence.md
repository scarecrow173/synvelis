---
id: "ADR-7nQ03cwrW5TiwGczRwWyQx"
type: "adr"
status: "accepted"
title: "CTL-02 canonical persistence — legacy は一度だけ正規化し current は reject"
created: "2026-11-27"
updated: "2026-11-27"
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
    - "docs/specs/ctl-02-canonical-persistence.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-02-canonical-persistence.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ctl-02-canonical-persistence.md"
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

# CTL-02 canonical persistence — legacy は一度だけ正規化し current は reject

## Status

accepted

## Context

CTL-02 の内部 Strip/Send target は fixed internal semantics を持つ。
`canonical_internal_spec` は strip channel role と send level に対して
`kind`/`mapping`/`time_scope` を定め、内部 target が plugin capability
を持たないことを定義する。write path と adoption path は既にこの契約を
拒否ベースで強制する。

一方、v10→v15 の snapshot にはこの強制前の状態が残り得る。特に
master fader は v7→v8 migration が user-authored fader を well-known IDへ
re-key する際、spec フィールドを保持した。`base` と `name` は authored
state なので保持しなければならないが、`kind`/`mapping`/`time_scope`/
`capability` は internal target についてユーザーが意味を選べる自由な
値ではない。

争点は 3 つある。

1. legacy snapshot を load 時に正規化するか、reject するか。
2. current schema の異常を正規化するか、reject するか。
3. canonical spec の判定を migration と oracle でどう共有するか。

## Considered Options

- Option 1（採用）: v15→v16 migration で一度だけ deterministic に
  正規化し、v16 以降は oracle が reject する。legacy は過去に合法だった
  persisted state なので read path で意味を canonical contract へ投影する。
  current body は foreign state なので修復しない。
- Option 2: 全 version で正規化する。却下。v16 body の異常まで黙って
  書き換えると、外部編集や実装 bug が作った foreign state を検出できない。
- Option 3: 非 canonical snapshot をすべて reject する。却下。v15 以前は
  非 canonical spec が合法だったため、既存プロジェクトを開けなくする。
  authored `name`/`base` を持つ usable state を捨てる必要はない。
- Option 4: migration と oracle に個別の判定表を書く。却下。同じ政策が
  2 箇所に分かれ、将来 canonical spec が変わったときに片方だけ更新される
  構造的なずれを招く。

## Decision

1. schema version を v16 へ上げ、v15→v16 migration で internal target の
   fixed spec fields だけを canonical 値へ正規化する。`name`/`base` は
   authored state なので保持する。`id`/`owner`/`role`、automation lane、
   send binding、watermark は変更しない。
2. v16 以降の current snapshot と post-apply state は
   `Project::validate_references` で canonical spec deviation を
   `Error::Validation` により fail-closed に拒否する。load は current body
   を修復しない。
3. canonical policy は `control.rs` の `canonical_internal_spec` を単一
   source とする。deviation 判定と migration normalization はその定義を
   参照し、kind/mapping/time_scope/capability の表を複製しない。
4. SendLevel oracle は `SendConfig.gain_target` の有無に依存しない。
   `ControlOwner::Send` × `ParameterRole::SendLevel` の target 全般に
   適用する。orphan/unreferenced target の削除や cascade は本決定に
   含めない。

## Consequences

- v15 以前の usable project は authored value を保ったまま開ける。
  fixed internal semantics だけが canonical contract へ投影される。
- v16 body に対する外部編集や bug 由来の spec deviation は load/apply
  oracle で検出できる。
- migration は target ID を変えないため、automation lane、modulation
  assignment、send binding を再配線する必要がない。
- plugin/processor target と free-form target は capability を含めて保持
  される。internal target に限って capability を落とす境界が明確になる。
- schema version が上がるため、version を固定する test の期待値更新が
  必要になる。newer schema rejection は従来の fail-closed 挙動を維持する。
