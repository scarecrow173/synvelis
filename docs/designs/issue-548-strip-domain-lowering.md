---
id: "DESIGN-548StripDomLowr03"
type: "design"
status: "approved"
title: "Issue #548 design — strip-domain capability 登録と lowering"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/548"
    - "docs/specs/issue-548-strip-domain-lowering.md"
    - "docs/discovery/issue-548-strip-domain-lowering.md"
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
    - "docs/specs/issue-548-strip-domain-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-548-strip-domain-lowering.md"
    - "crates/synvelis-gateway/src/prepare.rs"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-548-strip-domain-lowering.md"
  derived-by:
    - "docs/plans/issue-548-strip-domain-lowering.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #548 design — strip-domain capability 登録と lowering

## 概要

`PrepareEngine::op_table()` に strip-domain wire capability を
5 件登録し、`prepare.rs` に対応する lower fn・`lower_for`
entry・`op_name` arm・`op_facts` arms を追加する。variant edit
経路は inner op を ordinary op table で lower するため、
この登録だけで `branch.edit` の candidate edit として
wire 到達可能になる。mask/scope confinement は
`StripFieldsAdapter` が引き続き所有し、gateway は mask を
再実装も再判定もしない。

## capability descriptor（`prepare_engine.rs`）

| id | target | effects | input schema |
| --- | --- | --- | --- |
| `synvelis.strip.set-mute` | `Resource{Strip}` | `[Create, Write]` | `{muted: Bool}` required |
| `synvelis.strip.set-solo` | `Resource{Strip}` | `[Create, Write]` | `{soloed: Bool}` required |
| `synvelis.strip.insert-processor` | `Resource{Strip}` | `[Create, Write]` | `{index: Int 0.., name: Str ≤256, binding: Obj, sidechain_input: Bool}` required |
| `synvelis.param.set-base` | `Resource{ParameterTarget}` | `[Write]` | `{value: Num}` required |
| `synvelis.processor.set-binding` | `Resource{Processor}` | `[Write]` | `{binding: Obj}` required |

- 全て `deny_unknown_fields`、capability version `1.0`。
- bool op が `Create` を含むのは channel `ParameterTarget`
  の first-use create を正直に反映するため。

## lowering（`prepare.rs`）

- `lower_strip_set_mute`/`lower_strip_set_solo`:
  `arg_bool` で `muted`/`soloed` を読み `SetStripMute`/
  `SetStripSolo` を生成。target helper `target_strip` が
  `ResourceRef::Strip` → `StripId` を bind。
- `lower_strip_insert_processor`: `index` を `arg_i64` で読み
  `usize::try_from`（負値は typed malformed）、`name` は
  `arg_string`、`binding` は `arg_typed::<ProcessorBinding>`
  （serde 済み wire decode）、`sidechain_input` は `arg_bool`。
- `lower_param_set_base`: `arg_f64` で `value`、target helper
  `target_param` が `ParameterTargetId` を bind。
- `lower_processor_set_binding`: `arg_typed::<ProcessorBinding>`
  で `binding`、target helper `target_processor` が
  `ProcessorId` を bind。
- `lower_for` に 5 entry、`op_name` に対応 arm を追加。

## facts / preconditions（`op_facts`）

- `SetStripMute`/`SetStripSolo`: `ResourceRef::Strip` を read、
  `strip-exists` precondition。channel `ParameterTarget` が
  既存なら owner+role（`ControlOwner::Strip` +
  `ParameterRole::Mute|Solo`）で発見して read に追加
  （first-use create は read 不要 — apply 時に発見される
  内部 read として正直に宣言）。
- `InsertProcessor`: `Strip` read + `strip-exists` +
  `insert-index-in-range`。
- `SetParameterBase`: `ParameterTarget` read +
  `param-target-exists` + `param-not-readonly` +
  `param-base-in-range`。
- `SetProcessorBinding`: `Processor` read +
  `processor-exists`。
- catch-all の `unsupported` には落とさない — 登録 capability
  が ordinary prepare で常時 fail-closed になる半状態を
  防ぐ（設計判断1）。

## 責務境界

- gateway: wire decode・target bind・lowering・facts/effects
  宣言のみ。
- `synvelis-project`: `StripDomainMask` 判定・scope
  confinement・apply 時検証の唯一の所有者。
- **やらないこと**: routing op の candidate 受理、scope
  entity removal、他 domain の positive 化、mask の
  gateway 側再実装。

## 失敗時の倒れ方

- schema/type 違反 → `SchemaViolation`/`MalformedRequest`。
- 存在しない target → apply 時 validate が typed reject。
- mask 未宣言 family・scope 外 target・非 strip op →
  `StripFieldsAdapter` が `variant-unsupported`/
  `variant-scope-violation` で `PreparationRejected`。
- observability を分類できない op → 従来どおり
  `unsupported` で fail-closed（緩和しない）。
