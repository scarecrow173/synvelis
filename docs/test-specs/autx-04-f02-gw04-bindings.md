---
id: "TSPEC-034XKWxA9AoYLrFXHqkdyU"
type: "test-spec"
status: "approved"
title: "AUTX-04-F02 GW-04 bindings scratch-ID/rewrite seam — create-then-reference invariants"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/263"
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
    - "docs/impl/ir/autx-04-f01-composite-forward-ref.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-04-f02-gw04-bindings.md"
    - "docs/impl/exp/autx-04-f02-gw04-bindings.md"
    - "evidence/sv263/gw04-bindings-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/automation/design-atomic-macro.md"
    - "docs/impl/ir/autx-04-f01-composite-forward-ref.md"
  derived-by: []
  verifies:
    - "docs/automation/design-atomic-macro.md"
    - "docs/specs/command-gateway.md"
  verified-by: []
  references:
    - "crates/synvelis-gateway/src/prepare_engine.rs"
    - "crates/synvelis-gateway/tests/gw04_bindings.rs"
    - "evidence/sv263/gw04-bindings-ledger.md"
  defers: []
  deferred-by: []
---

# AUTX-04-F02 GW-04 bindings scratch-ID/rewrite seam — create-then-reference invariants

## Purpose

`synvelis.operation.batch` の非空 `bindings` を prepare 時に
解決する GW-04 scratch-ID/rewrite seam（synvelis#263 /
AUTX-04-F02）が保証すべき不変条件を固定する。
`docs/automation/design-atomic-macro.md` §3 の対応形 —
compiler が emit する `$output.<name>` placeholder を、
pinned base の `id_watermark` から逐次採番する scratch
適用で実 ID へ rewrite し、単一 `ProjectOp::Composite`
として既存の proposal/commit/undo 契約へ載せること —
を守る。F01（#432）の project 側前提「forward-reference
composite が `ProjectController::apply` で commit される」
の上に立つ gateway 側 slice であり、automation 側 compile
・Workflow 分解経路・実 OS/実 Plugin 結合は対象外
（別 slice・NotRun）。

## Feature

`PrepareEngine::lower_bound_batch` が `bindings` 宣言を
検証し、member を宣言順に scratch project へ trial-apply
しながら placeholder を解決し、flatten した `Composite`
と semantic commands を返す振る舞い。provisional
authorization gate は seam の scratch post-state を
`BaseFirstOracle` の candidate として受け取り、
transaction-created resource を agent caller の
scope 判定へ見せる。

## Rules

- **AUTX4F2-TS-R01 declaration shape**: `bindings[]` の
  各要素は `{name, member, kind}` の object のみ。
  未知 field・欠落 field・非 object・`member` の非整数
  /範囲外・未知 kind tag・同一 `(member, kind)` の
  重複宣言はすべて typed `SchemaViolation` /
  `UnknownResourceKind` で拒否される。
- **AUTX4F2-TS-R02 placeholder identity**: placeholder は
  `<kind-tag>-<32hex>` で raw が
  `OUTPUT_BINDING_ID_BASE + slot`（`slot < 64`）の
  reserved range に限る。range 外の文字列は一切
  rewrite されず、placeholder が object key に現れる
  場合は typed 拒否される。
- **AUTX4F2-TS-R03 resolution order**: placeholder が
  有効なのは (a) 宣言済み slot を指し、(b) kind tag が
  宣言 kind と一致し、(c) producer member が consumer
  より前にある場合のみ。後方・自己・前方参照、未宣言
  slot、kind 不一致は lowering 時に typed 拒否される。
- **AUTX4F2-TS-R04 exactly-one creation**: binding は
  producer member の scratch apply 前後で
  `resource_map` diff した created resource のうち
  宣言 kind に一致するものが**ちょうど 1 つ**のときのみ
  解決される。0 個・2 個以上は typed
  `PreparationRejected`、producer の apply 失敗で
  未解決のまま残った placeholder も typed
  `PreparationRejected` であり、ID の推測は一切ない。
- **AUTX4F2-TS-R05 deterministic id series**: scratch は
  pinned base の clone に `CounterIdGenerator::new(
  base.id_watermark)` を seed して member 順に
  `validate`+`apply` し、`Composite::apply` の commit
  系列と一致する。rewrite は lowering 前の member wire
  JSON に対して再帰的に行われ、解決済み placeholder は
  `prepare::ref_tagged` の canonical tagged id へ置換
  される。live project は prepare 中に一切変更されない。
- **AUTX4F2-TS-R06 scratch-fed provisional oracle**:
  非空 `bindings` の batch に限り provisional gate は
  `BaseFirstOracle::new(base, scratch)` を使い、
  transaction-created resource を `Known` として解決
  する。`bindings` を持たない全 operation は従来通り
  pinned-base のみの oracle を使う。
- **AUTX4F2-TS-R07 domain failure ordering**: member の
  domain validate/apply 失敗は lowering では raise
  せず、emitted `Composite` の `op.validate` が
  provisional gate の後で同じ失敗を再現する
  （SEC-03 ordering 保存）。request-shape の失敗
  （宣言・placeholder 系）のみ lowering で拒否される。
- **AUTX4F2-TS-R08 single transaction contract**: retained
  `ProjectOp` は flatten された単一 `Composite` であり、
  candidate apply・digest・proposal・apply gate・
  `ProjectController` の単一 undo entry は既存契約のまま。
  第二 transaction・第二 approval・第二 commit 経路は
  存在しない。
- **AUTX4F2-TS-R09 legacy lowering**: `bindings` 欠落・
  `bindings: []` は従来の member recursion のまま
  lower され、scratch post-state を生成しない。

## Examples

- **EX-01**: agent caller が
  `[track.create, lane.create(parent=$output.new_track)]`
  + `bindings:[{new_track, member:0, kind:track}]` を
  prepare すると、placeholder は `track-<raw 1000>`
  へ rewrite され、candidate に `TrackId(1000)` と
  それを parent に持つ `LaneId(1001)` が現れ、
  `accept_apply` が stored effects を authorize し、
  `ProjectController` の 1 apply/1 undo で両 resource
  がまとめて戻る
  （`agent_create_then_reference_commits_as_one_undo_unit`）。
- **EX-02**: `[track.create, lane.create, clip.add]` +
  2 slot の bindings で track→lane→clip の多段
  placeholder が各々前段の実 ID へ解決される
  （`chained_bindings_resolve_across_three_members`）。
- **EX-03**: 同 batch を `create_within` なしの grant
  で agent prepare すると derived-effects 段で
  `Denied`、`include_future: false` の dynamic scope
  では provisional 段で `Denied` — placeholder 解決は
  authorization を迂回しない
  （`agent_without_create_axis_still_denies` /
  `agent_scope_without_future_membership_denies`）。
- **EX-04**: 非 object・未知 field・欠落 field・
  範囲外/非整数 member・未知 kind・重複宣言の各
  bindings が typed 拒否
  （`rejects_malformed_binding_declarations`）。
- **EX-05**: 未宣言 slot・kind tag 不一致・前方/自己
  参照の placeholder が typed 拒否
  （`rejects_undeclared_and_kind_mismatched_placeholders` /
  `rejects_backward_self_and_premature_references`）。
- **EX-06**: apply 失敗 producer の未解決 placeholder、
  created 0 個（kind 不一致）・2 個（detach-source の
  note 複製）producer が typed `PreparationRejected`
  （`rejects_unresolvable_producer_output` /
  `rejects_wrong_created_arity`）。
- **EX-07**: object key の placeholder・nested batch
  member が typed 拒否、`bindings` 欠落/`[]` は legacy
  lowering のまま完了
  （`rejects_placeholder_object_keys_and_nested_batches` /
  `empty_or_absent_bindings_keep_legacy_lowering`）。

## Guarantees

- 成功した bindings batch は 1 proposal・1 composite・
  1 commit・1 undo の既存契約を維持し、rewrite 後の
  member ops・commands・effects・candidate がすべて
  実 ID を持ち、placeholder 文字列はいかなる面にも
  残らない。
- scratch 採番系列は `Composite::apply` の commit 系列
  と一致し、prepare を通過した composite が commit で
  ID 不整合により失敗しない。
- fail-closed: malformed・ambiguous・premature・
  unresolved・wrong-arity のいずれの bindings も
  typed error で拒否され、部分 rewrite・ID 推測・
  live project への変更は発生しない。
- agent caller の provisional authorization は
  transaction-created resource を scratch 経由で
  `Known` に解決するが、scope・`create_within`・
  `include_future`・data-access の各判定は通常通り
  効く。

## Non-goals

- automation 側の macro compile・`$output` 式・
  `MacroBindingWire` 生成 — `synvelis-automation`
  （AUTX-04 本体 slice）の責務。gateway は wire 契約の
  消費側のみを実装する。
- Workflow による create→query→reference 分解経路
  （AUTX-05/06）— typed Unsupported の代替正規経路と
  して残るが、本 slice は対応形の実装であり分解は
  行わない。
- `draw_raw_id` 予約窓を跨ぐ採番系列の一致保証 —
  F01 と同じく watermark シードの fail-closed 既定を
  継承する。
- 実 OS・実 Plugin・実 RT・`*-msrv` レーン — 対象外
  （NotRun）。

## Risk

- member 毎の trial-apply が project clone を伴うため、
  member 数（≤64）に比例した prepare コストが増える。
  逐次化を外す・scratch を省略する変更が入ると
  placeholder 解決と provisional oracle の両方が
  崩れ、create-then-reference が `NotFound`/`Denied`
  で再び失敗する。
- `OUTPUT_BINDING_ID_BASE`/`MAX_BINDING_SLOTS` の
  mirror が automation 側定数と乖離すると placeholder
  が解決不能になる。wire 定数の一致は両 crate の
  test で間接的に固定されるが、定数変更時は両側の
  同時更新が必要。
- provisional gate へ scratch を渡す分岐を
  `bindings` 以外へ拡張すると、pinned-base 前提の
  SEC-03/SEC-14 判定が崩れる恐れがある。
