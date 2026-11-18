---
id: "TSPEC-565ProcessorBindingCascade06"
type: "test-spec"
status: "approved"
title: "Issue #565 test-spec — 退役 cascade の検証仕様"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/565"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-565-processor-binding-cascade.md"
    - "docs/designs/issue-565-processor-binding-cascade.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-565-processor-binding-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-565-processor-binding-cascade.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-565-processor-binding-cascade.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/issue-565-processor-binding-cascade.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #565 test-spec — 退役 cascade の検証仕様

## Feature: persisted plugin-instance identity の退役不変条件

`SetProcessorBinding` の op 経路と wire prepare 経路で、
旧 `PluginInstanceId` が退役し新 id が発行されること、
かつその退役が外部参照を dangling させないことを pin する。

## Rule 1: rebind（別 PluginRef）で旧 id が退役する

- **Given**: `Resolved(pluginA)` で挿入された processor
  （instance id=1 発行済み）
- **When**: `ProjectOp::SetProcessorBinding` で
  `Resolved(pluginB)`（descriptor/version/capability_revision
  のいずれかが異なる authored identity）に rebind
- **Then**: 新 id=2 が発行され entry.plugin=B・watermark=3
  に前進。旧 id=1 は store 内のどの entry にも存在せず
  再発行されない。`resolve(proc, pluginA)`→`None`、
  `resolve(proc, pluginB)`→`Some(2)`。`validate_references`
  が post-apply で通る
- **Example**: `set_binding_rebind_retires_old_instance_id`
  （`plugin_instance_persist.rs`）

## Rule 2: 同一 PluginRef の再設定は冪等

- **Given**: `Resolved(pluginA)` で挿入された processor
  （instance id=1）
- **When**: 同一 `PluginRef` を持つ binding で
  `SetProcessorBinding` を再適用
- **Then**: `ensure` が entry を維持し id=1・watermark=2
  のまま（新 id は発行されない）。`validate_references`
  が通る
- **Example**: `set_binding_same_plugin_keeps_instance_id`

## Rule 3: Unresolved 化で entry が削除され id が退役する

- **Given**: `Resolved(pluginA)` で挿入された processor
  （instance id=1）
- **When**: `SetProcessorBinding` で `Unresolved` binding
  に変更
- **Then**: `plugin_instances.instances` から entry が
  削除され `resolve`→`None`。watermark は後退しない
  （発行済み id=1 は退役のまま）。`validate_references`
  が通る
- **Example**: `set_binding_unresolved_retires_instance_id`

## Rule 4: wire prepare が binding 変更を Write detail で観測可能にする

- **Given**: `Resolved(pluginA)` の processor を持つ
  pinned base
- **When**: `synvelis.processor.set-binding` を別
  `PluginRef` の `Resolved` binding に向けて prepare
- **Then**: `Completed` で proposal が発行される
  （`unsupported` なし — 過剰拒否がないことの反証）。
  `ResourceRef::Processor(proc)` の `Write` 効果の
  before/after detail が `binding` フィールドの差分を運び、
  `prepared.candidate.plugin_instances` が新 id 発行・
  旧 entry 置換・watermark 前進を反映する
- **Example**: `sv565_processor_set_binding_write_detail_`
  `carries_binding_and_retires_identity`（`tests/prepare.rs`）

## 検証方法

- op 経路 pin: `mise run test synvelis-project`
  （`plugin_instance_persist.rs` 新規 3 件 + 既存
  apply 経路 pin・oracle pin の非回帰）
- wire pin: `mise run test synvelis-gateway`
  （`tests/prepare.rs` 新規 pin + sv#561 系既存 pin の
  非回帰）
- OS-L1 のみ。GUI-L2/EXT-L3・MSRV・3OS・Full CI は
  POLICIES.md §5 により日常必須検証に含めない
