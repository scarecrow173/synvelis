---
id: "SPEC-571ProcessorBindingStale02"
type: "spec"
status: "approved"
title: "Issue #571 spec — SetProcessorBinding rebind で SemanticPort.binding を再導出する修正"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/571"
    - "docs/discovery/issue-571-processor-binding-stale.md"
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
    - "docs/discovery/issue-571-processor-binding-stale.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-571-processor-binding-stale.md"
    - "docs/specs/issue-565-processor-binding-cascade.md"
  refines: []
---

# Issue #571 spec — SetProcessorBinding rebind で SemanticPort.binding を再導出する修正

## 目的

`SetProcessorBinding` による rebind が、同じ最終 binding を持つ
processor を `InsertProcessor` で新規挿入した場合と等価な
persisted state を産むように修正する（Issue 本文「Expected
fix」: rebind は `proc.binding` を更新するのと同じ境界 —
project domain layer — で processor の port の
`SemanticPort.binding` を再導出し、insert-fresh と rebind の
path-equivalence を回復する）。

## 要件

- `ProjectOp::SetProcessorBinding` の apply arm で、
  `proc.binding` 差替と同じ atomic な適用内に、
  `proc.port_ids` が指す全所有 port の `binding` を
  `processor_port_binding` で再導出する（`Resolved(B)` →
  `PluginDeclared{plugin: B.descriptor, advertised,
  capability_revision: B.capability_revision}`、`Unresolved`
  → `PortBinding::Unresolved{plugin: descriptor,
  advertised}`）。
- 各 port の `advertised` slot 名は port 固有の semantic data
  として既存値を維持する（`"in"`/`"out"`/`"sidechain"` の
  slot identity は rebind で変わらない）。
- `PortBinding::Internal` の port は plugin stamp を持たない
  ため再導出の対象外とする。
- `plugin_instances.sync_processor` の identity  semantics
  （`synvelis#565` で不変条件健全と監査済み）は変更しない —
  新 `PluginRef` rebind で新 id 発行・旧 id 退役、同一
  `PluginRef` で id 維持、`Unresolved` で entry 削除、
  watermark 単調。
- `crates/synvelis-project/tests/` に rebind path の pin test
  を追加する: `Resolved→Unresolved` で全所有 port が
  `PortBinding::Unresolved` になり `resolved_endpoints` の
  `UnresolvedPort` 拒否が `InsertProcessor`+`Unresolved`
  path と同一であること、`Resolved(A)→Resolved(B)` で B の
  plugin identity（descriptor・`capability_revision`）が全
  port に stamp されること、`Unresolved→Resolved` で
  `PluginDeclared` が復元され endpoint が再び resolvable に
  なること。
- `vcs06_plugin_instance_matrix.rs` の rebind fixture を、手動
  stamp ではなく実 `SetProcessorBinding` op を通して after
  状態を作る形に修正し、fixture と実 op の乖離を解消する。
- op 定義の doc comment を、port binding 再導出を含む実挙動に
  合わせて更新する。
- slug `issue-571-processor-binding-stale` の doc chain
  （discovery/spec/test-spec）と `evidence/sv571/` の検証記録
  を作成する。docs index（`docs/**/README.md`）は編集・再生成
  しない。

## 受入条件（Issue 転記）

- `Resolved→Unresolved` rebind produces ports that the routing
  `UnresolvedPort` rejection treats identically to the
  `InsertProcessor`+`Unresolved` path (pin test).
- `Resolved(A)→Resolved(B)` rebind stamps B's plugin identity
  into port binding (pin test).
- `resolved_endpoints` rejection parity across the two paths
  verified.
- Required mise lanes green; evidence recorded.

## 非対象

- `crates/synvelis-project/src/routing_tx.rs` の一切の変更 —
  `resolved_endpoints`・`port_capability_generation` は
  `synvelis#572` の領域。読み取りと検証経路としての実行のみ。
- `plugin_instance.rs` の変更 — `sync_processor` の不変条件は
  `synvelis#565` で健全と監査済み。
- `crates/synvelis-project/src/` の `ops.rs` 以外の src 変更。
- 依存追加・`Cargo.toml`/`Cargo.lock` の変更。
- `crates/synvelis-project/tests/` 以外の test・他 crate・
  他リポジトリの変更。
- docs index の編集・再生成（Coordinator が統合時に整備）。
