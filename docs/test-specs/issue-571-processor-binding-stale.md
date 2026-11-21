---
id: "TSPEC-571ProcessorBindingStale03"
type: "test-spec"
status: "approved"
title: "Issue #571 test-spec — rebind port stamp 再導出と UnresolvedPort 拒否 parity の検証仕様"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/571"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-571-processor-binding-stale.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-571-processor-binding-stale.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/issue-571-processor-binding-stale.md"
    - "docs/test-specs/issue-565-processor-binding-cascade.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-571-processor-binding-stale.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #571 test-spec — rebind port stamp 再導出と UnresolvedPort 拒否 parity の検証仕様

## Feature: rebind の port binding が insert-fresh と path-equivalent

`SetProcessorBinding` で rebind した processor の所有 port は、
同じ最終 binding で `InsertProcessor` した processor の port と
同一の `PortBinding` shape を持つ。検証は
`crates/synvelis-project/tests/sv571_rebind_port_binding.rs`
（新規 4 pin）と `vcs06_plugin_instance_matrix.rs` の既存 matrix
（fixture を実 op 経由に修正）で行う。

## Rule 1: `Resolved→Unresolved` rebind で全 port が Unresolved になる

- **Given**: `Resolved(pluginA)` で挿入された sidechain 対応
  processor（`in`/`out`/`sidechain` の 3 slot）
- **When**: `ProjectOp::SetProcessorBinding` で
  `Unresolved{descriptor}` に rebind
- **Then**: 全所有 port の binding が
  `PortBinding::Unresolved{plugin: descriptor, advertised}`
  となり、`InsertProcessor`+同一 `Unresolved` binding の
  insert-fresh shape（slot 名・順序を含む）と一致する。
  `validate_references` が通る
- **Example**: `rebind_to_unresolved_marks_every_port_unresolved`

## Rule 2: `UnresolvedPort` 拒否が insert path と rebind path で同一

- **Given**: Rule 1 の rebind-path project と、同一 op 列で
  `Unresolved` を insert した insert-path project（同一 id 列・
  同一 `port_capability_generation`）
- **When**: strip main output → processor sidechain input の
  `ConnectionKind::Sidechain` wire を `routing_preflight` に
  通す（processor port が合法的に取れる唯一の connection
  shape）
- **Then**: 両 path が同一の
  `RoutingEditError::UnresolvedPort` で拒否され、同一 port を
  名指す。commit path（`ProjectOp::Routing`）のエラーも同じ
  typed code `routing-unresolved-port` に畳まれる
- **Example**: `rebind_to_unresolved_rejects_endpoints_like_insert_path`

## Rule 3: `Resolved(A)→Resolved(B)` rebind で B の identity が stamp される

- **Given**: `Resolved(pluginA)` で挿入された processor
- **When**: `Resolved(pluginB)`（descriptor・
  `capability_revision` が A と異なる）に rebind
- **Then**: 全所有 port が
  `PluginDeclared{plugin: B.descriptor, capability_revision:
  B.capability_revision, advertised: <slot 固有>}` となり
  insert-fresh `Resolved(B)` shape と一致。各 slot の
  `advertised` 名は rebind 前の値を維持。
  `port_capability_generation` が insert-fresh `Resolved(B)`
  project と一致する
- **Example**: `rebind_resolved_to_resolved_restamps_every_port`

## Rule 4: `Unresolved→Resolved` で PluginDeclared が復元され endpoint が開く

- **Given**: `Unresolved` で挿入された processor（port は
  `PortBinding::Unresolved`）
- **When**: `Resolved(pluginB)` に rebind
- **Then**: 全所有 port が `PluginDeclared{B}` に復元され
  insert-fresh shape と一致。Rule 2 と同じ sidechain wire が
  `routing_preflight` を通り `validate_references` が通る
- **Example**: `rebind_unresolved_to_resolved_stamps_and_reopens_endpoints`

## Rule 5: `vcs06` rebind fixture は実 op の産物を使う

- **Given**: `plugin_instance_replaced_selection_transplants_new_identity`
  の before fixture（`Resolved(pluginA)` processor）
- **When**: after fixture を構築する
- **Then**: 手動の `capability_revision` stamp ではなく実
  `ProjectOp::SetProcessorBinding`.apply で `Resolved(pluginB)`
  に rebind して作る — fixture が「実 op が産む形」を自称する
  以上、実 op 経由で生成する。candidate の plugin-instance
  store（{id=9, plugin_b}/watermark=10）は transplant 試験の
  ために明示値で上書きする
- **Example**: `vcs06_plugin_instance_matrix.rs` 既存 9 test の
  非回帰

## 検証方法

- 新規 pin 4 件 + 全既存 pin の非回帰:
  `mise run test synvelis-project`
- `mise run check synvelis-project`・`mise run lint
  synvelis-project`（clippy `-D warnings`）
- `mise run fmt-check`・`mise exec -- prek run --all-files`
- red→green: 修正前コードで pin を実行し 4/4 失敗（stale stamp
  と拒否迂回を再現）、修正後 4/4 成功を `evidence/sv571/` に
  記録する
- OS-L1 のみ。GUI-L2/EXT-L3・MSRV・3OS・Full CI は POLICIES.md
  §5 により日常必須検証に含めない
