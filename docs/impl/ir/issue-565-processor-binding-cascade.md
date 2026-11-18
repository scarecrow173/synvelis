---
id: "IR-565ProcessorBindingCascade07"
type: "impl"
status: "completed"
title: "Issue #565 impl-ir — 退役 cascade 監査の実装記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/565"
    - "docs/specs/issue-565-processor-binding-cascade.md"
    - "docs/designs/issue-565-processor-binding-cascade.md"
    - "docs/plans/issue-565-processor-binding-cascade.md"
    - "docs/tasks/issue-565-processor-binding-cascade.md"
    - "docs/test-specs/issue-565-processor-binding-cascade.md"
  changes:
    added:
      - type: "dir"
        path: "evidence/sv565/"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/plugin_instance_persist.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/prepare.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-565-processor-binding-cascade.md"
    - "docs/test-specs/issue-565-processor-binding-cascade.md"
  implemented-by: []
  depends-on:
    - "docs/test-specs/issue-565-processor-binding-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-565-processor-binding-cascade.md"
    - "docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/issue-565-processor-binding-cascade.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/exp/issue-565-processor-binding-cascade.md"
  references: []
  defers:
    - "synvelis#128"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-565-processor-binding-cascade.md"
    rejected: []
---

# Issue #565 impl-ir — 退役 cascade 監査の実装記録

## 実装サマリ

監査の結論は「不変条件健全・外部参照 dangling なし」。
production コードの動作変更はゼロで、契約明記
（コメント 3 箇所）と test pin（project 3 件・gateway
1 件）で着地した。

## 変更ファイル

### 契約明記（コメントのみ）

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-gateway/src/prepare_engine.rs` | `synvelis.processor.set-binding` op-table entry 直上に posture コメント: rebind は persisted plugin-instance identity を退役・再発行し、`ResourceRef` で名指せない internal structure。binding 変更自体は `Processor` `Write` detail で観測可能 |
| `crates/synvelis-gateway/src/prepare.rs` | `SetProcessorBinding` op_facts arm に同 posture コメント（`AddSend`/`RemoveSend` の「internal structure」様式に準拠） |
| `crates/synvelis-project/src/ops.rs` | `SetProcessorBinding` op 定義 doc comment に「退役 id は watermark 単調性により再発行されず、旧 `(processor, plugin)` pair の resolve は `None`」を補強 |

### test pin

| ファイル | 追加 |
| --- | --- |
| `crates/synvelis-project/tests/plugin_instance_persist.rs` | `plugin_ref_b()` helper + 3 test: `set_binding_rebind_retires_old_instance_id`（新 id=2・旧 id=1 全 entry から消失・watermark=3・resolve 旧→None/新→Some(2)・oracle 通過）、`set_binding_same_plugin_keeps_instance_id`（冪等・id=1/watermark=2 維持）、`set_binding_unresolved_retires_instance_id`（entry 削除・watermark=2 後退なし・oracle 通過） |
| `crates/synvelis-gateway/tests/prepare.rs` | `PluginRef`/`ProcessorBinding` import 追加、`sv565_plugin_ref()`/`sv565_processor_controller()` file-local fixture（共有 `rig` 非接触）、`sv565_processor_set_binding_write_detail_carries_binding_and_retires_identity`: `unsupported` 空・`Processor` `Write` effect の before/after detail が `binding` 差分を運ぶ・candidate `plugin_instances` が新 id 発行・watermark 前進・`validate_references` 通過 |

## 監査結果の確定事項（evidence/sv565/audit.md に転記）

- 退役経路: `ops.rs:2383-2398` apply arm →
  `plugin_instances.sync_processor` → `ensure`/`remove`
  （`plugin_instance.rs:161-169`）。`InsertProcessor`・
  `RemoveProcessor`・VCS `apply/entity.rs:505`・v12→v13
  backfill（`migration.rs:467-472`）が同一 primitive を共有。
- 不変条件 3 軸:
  - 再読み込み: schema v13 store + backfill + `validate_against`
    oracle（`ProjectController::apply`/`load` 双方で
    `validate_references` 経由）。健全。
  - 復元: `resolve` の fail-closed `None`（missing/stale
    区別なし）と消費側 typed 経路
    （`synvelis-player/src/routing_control.rs:1407-1419`
    `without_control`、`synvelis-project/src/host_param.rs:180-221`
    `None` is data）。健全。
  - 外部参照: `ResourceKind`/`ENUMERATED_KINDS` に
    `PluginInstance` 非存在 → `ResourceRef` 不可名指 →
    dangling 構造的に起きない。VCS adopt の `Removed`
    は typed `Unsupported` fail-closed
    （`apply/entity.rs:113-125`・sv#559 占有・読み取りのみ）。
    `Processor` `Write` detail が binding 差分を運ぶため
    rebind 自体は観測可能。
- `unsupported`/op_facts 追加は不要と判定 — 実害確認
  されず過剰拒否は既存受理の回帰になる。

## 残件（本 slice 非対象・記録のみ）

- **隣接発見 ADJ-1/ADJ-2（review-protocol で CONFIRMED・
  別 Issue 要）**: `SetProcessorBinding` が persisted
  `SemanticPort.binding`（`PortBinding` stamp）を更新しない
  ため rebind 後も旧 plugin の port binding が残り、
  `resolved_endpoints` の `UnresolvedPort` 拒否迂回・
  declared-port 報告の事実矛盾が起き得る（P2・main 既存）。
  また `port_capability_generation` が `capability_revision`
  のみを合算するため同一 revision の rebind を検出しない
  （P3・main 既存）。いずれも `plugin_instances` store の
  不変条件とは別構造の問題であり、本 slice の charter
  （`PluginInstanceId` 退役 cascade）範囲外 — 詳細は
  `evidence/sv565/audit.md` 隣接発見節。別 Issue での
  検討事項として PR 本文の未対応作業にも記録する。
- **`PluginStateProbe` staleness → `synvelis#128` defer**:
  `crates/synvelis-render/src/dependency.rs:105-131` の
  `PluginStateEvidence{digest, quality}` は plugin/instance
  identity を持たず、`PluginStateProbe::evidence(ProcessorId)`
  は `ProcessorId` キーのみで evidence を引く（:543）。
  rebind 後に旧 plugin 用に capture された evidence が同一
  `ProcessorId` の新 plugin に attach し得る staleness 契約の
  問題。in-tree 実装は `NoPluginStates`/`StaticPluginStates`
  fixture のみで実害実測なし。`dependency.rs` は本 slice の
  lease 外であり、port 契約の世代区別は TIM-04
  （`synvelis#128` plugin state snapshot 一貫性）の設計に
  属するため defer と判定。
- docs index（`docs/**/README.md`）再生成は Coordinator の
  統合時作業として残す。

## 未実施・非対象の再確認

- `plugin_instance.rs` 本体の変更なし（module doc で不変条件
  は完全に明文化済み）。
- `unsupported` 追加・`ResourceKind`/`ENUMERATED_KINDS` 拡張
  なし。
- sv#559 占有パス（`apply/entity.rs`・`variant/*` 等）への
  変更なし（読み取り参照のみ）。
- `PrepareRequest`/wire schema 変更なし。
