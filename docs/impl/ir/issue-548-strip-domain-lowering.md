---
id: "IR-548StripDomLowr08"
type: "impl"
status: "completed"
title: "Issue #548 impl IR — strip-domain lowering 実装記録"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/548"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-548-strip-domain-lowering.md"
      - type: "file"
        path: "docs/specs/issue-548-strip-domain-lowering.md"
      - type: "file"
        path: "docs/designs/issue-548-strip-domain-lowering.md"
      - type: "file"
        path: "docs/plans/issue-548-strip-domain-lowering.md"
      - type: "file"
        path: "docs/tasks/issue-548-strip-domain-lowering.md"
      - type: "file"
        path: "docs/test-specs/issue-548-strip-domain-lowering.md"
      - type: "file"
        path: "docs/impl/exp/issue-548-strip-domain-lowering.md"
      - type: "dir"
        path: "evidence/sv548/"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/prepare.rs"
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "evidence/sv283/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv283/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-548-strip-domain-lowering.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/issue-548-strip-domain-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-548-strip-domain-lowering.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/issue-548-strip-domain-lowering.md"
  derived-by: []
  verifies:
    - "docs/test-specs/issue-548-strip-domain-lowering.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #548 impl IR — strip-domain lowering 実装記録

## 実装サマリ

`PrepareEngine::op_table()` に strip-domain wire capability
5 件を登録し、`prepare.rs` に lower fn・`lower_for`/
`op_name`/`op_facts` arms を追加した。variant edit 経路は
inner op を ordinary op table で lower するため、登録だけで
`branch.edit` の candidate edit として wire 到達可能に
なった。mask/scope confinement は `StripFieldsAdapter` が
引き続き所有し、gateway は mask を再実装していない。

## 変更ファイル

- `crates/synvelis-gateway/src/prepare_engine.rs` —
  `op_table` に `synvelis.strip.set-mute`/
  `set-solo`/`insert-processor`・`synvelis.param.set-base`・
  `synvelis.processor.set-binding` の descriptor + schema を
  追加（`clock-epoch.register` の後）。
- `crates/synvelis-gateway/src/prepare.rs` —
  `ControlOwner`/`ParameterRole`/`ParameterTargetId`/
  `ProcessorId`/`StripId` import、`target_strip`/
  `target_param`/`target_processor` helper、lower fn 5 件、
  `lower_for` 5 entry、`op_name` 5 arm、`op_facts` 4 arm
  （mute/solo は owner+role で既存 channel target を read
  に追加）。
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs` —
  module doc の stale 前提を更新、strip-domain op builder
  6 件追加、positive mask-gated edit 試験 2 件追加
  （狭 mask で channel 系 3 件 + 全 mask で inserts 系
  2 件）、mask/scope/非 strip 負側を維持。
- `crates/synvelis-gateway/tests/prepare.rs` — ordinary
  prepare 試験 3 件追加。
- cb-06 doc 3 件・`evidence/sv283/` 2 件追記、
  `evidence/sv548/` 新規。

## 設計判断の適用

- 設計判断1（`op_facts` arms 必須）: 実施 — catch-all の
  `unsupported` に落とさず reads + preconditions を宣言。
- 設計判断2（GW-04 op 表慣例）: 実施 — `synvelis.<domain>.
  <op>` 命名・version 1.0・TargetContract は書き込み先
  資産に合わせる・effect bounds は first-use create を
  正直に反映。
- 用語確定: 「routing mask 拡張」= gateway op dispatch
  拡張として実装。send/connection/sidechain/bus op・
  scope entity removal は対象外。

## 検証状況

- `mise run check synvelis-gateway`: Pass
- `mise run test synvelis-gateway -- cb06_strip_fields_mask_gated_edit_wire_lowering`: Pass
- `mise run test synvelis-gateway -- cb06_strip_fields_insert_ops_wire_lowering`: Pass
- `mise run test synvelis-gateway -- prepare_strip`: Pass（2 件）
- `mise run test synvelis-gateway -- prepare_param_set_base`: Pass
- 全量検証（lint/test/check/fmt/prek 等）の結果は
  `evidence/sv548/` と PR 本文に記録。
