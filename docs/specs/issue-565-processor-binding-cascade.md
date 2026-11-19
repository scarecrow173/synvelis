---
id: "SPEC-565ProcessorBindingCascade02"
type: "spec"
status: "approved"
title: "Issue #565 spec — SetProcessorBinding 退役 cascade の監査・契約明記・test pin"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/565"
    - "docs/discovery/issue-565-processor-binding-cascade.md"
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
    - "docs/discovery/issue-565-processor-binding-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-565-processor-binding-cascade.md"
    - "docs/test-specs/issue-565-processor-binding-cascade.md"
  refines: []
---

# Issue #565 spec — SetProcessorBinding 退役 cascade の監査・契約明記・test pin

## 目的

`SetProcessorBinding` 実行時に旧 `PluginInstanceId` が退役・
置換される経路（`plugin_instances.sync_processor`）について、
persisted identity の不変条件（再読み込み・復元・外部参照）
への影響を監査し、結果を doc chain/evidence に記録する。
問題があれば fail-closed または契約明記で修正し、問題が
なければ根拠を記録して完了とする。

監査の主問いは「退役する persisted identity を外部が参照
でき、dangling が起き得るか」。discovery の結論どおり
外部参照経路は構造的に存在しないため、本 slice の実装
変更は**契約明記（コメント）と test pin**に限定され、
`unsupported` 追加や store cascade の動作変更は行わない。

## 要件

- `SetProcessorBinding` の apply が `sync_processor` 経由で
  persisted plugin-instance identity を退役・再発行する
  経路を、main のコードで再確認し監査記録に残す
  （discovery 済み・evidence に転記）。
- 不変条件 3 軸の照合結果を doc chain と `evidence/sv565/`
  に記録する:
  - **再読み込み**: schema v13 store + v12→v13 backfill
    （同一 primitive 再利用）+ `validate_against` oracle
    （`validate_references` 内・apply/load 双方で実行）。
  - **復元**: `resolve` の fail-closed（missing/stale を
    区別しない `None`）と消費側の typed 経路
    （`without_control` / `None` is data）。
  - **外部参照**: `ResourceKind`/`ENUMERATED_KINDS` 非存在に
    よる `ResourceRef` 不可名指 → dangling なし、ただし
    `Processor` `Write` detail で binding 変更は観測可能、
    VCS `ObjectKind::PluginInstance` は adopt 削除が
    typed `Unsupported` fail-closed。
- `synvelis.processor.set-binding` の persisted identity
  退役・再発行が `ResourceRef` で名指せない internal
  structure であり、binding 変更自体は `Processor` `Write`
  detail で観測可能である旨を、`insert-processor`/send 系に
  倣う形でコメント明記する（`prepare.rs` op_facts arm・
  `prepare_engine.rs` op-table entry。`ops.rs` の op 定義
  コメントは退役・非再発行の明記を補強してよい）。
- `crates/synvelis-project/tests/plugin_instance_persist.rs`
  に `SetProcessorBinding` op 経路の pin を追加する:
  別 `PluginRef` への rebind で新 id 発行・旧 id 退役
  （再発行されない）・watermark 前進・`resolve`(旧 plugin)
  →`None`・oracle `validate_against` 通過。同一 `PluginRef`
  再設定の冪等性と `Unresolved` 化（entry 削除・id 退役）
  も pin する。
- `crates/synvelis-gateway/tests/prepare.rs` に
  `synvelis.processor.set-binding` の `Write` 効果が
  Processor detail に `binding` 変更を含むことの wire pin
  を追加する（sv#561 節の様式・`mod rig` 共有 harness +
  ファイルローカル fixture。共有 rig fixture は非接触）。
- `PluginStateProbe` staleness 論点（`dependency.rs` —
  lease 外）は監査結果として記録し `synvelis#128`（TIM-04）
  への defer を判定として明記する。port 契約の変更は行わない。

## 受入条件（Issue 転記）

- `SetProcessorBinding` 実行時に旧 `PluginInstanceId` が
  退役・置換される経路を調査し、persisted identity の
  不変条件（再読み込み・復元・外部参照）への影響を記録
- 問題があれば fail-closed（unsupported/op_facts）または
  適切な契約明記で修正、問題がなければ根拠を doc chain/
  evidence に記録して close
- 関連 mise 検証 exit 0・PR は `Refs #565` で close 判定は
  Coordinator に委ねる

## 非対象

- store cascade（`sync_processor`/`ensure`/`remove`/
  `validate_against`）の動作変更 — 不変条件健全のため
- `unsupported`/op_facts への追加 — 外部参照 dangling の
  実害が確認されないため（過剰拒否は既存受理の回帰）
- `crates/synvelis-render/**`・`crates/synvelis-player/**`
  の変更（`PluginStateProbe` port 契約は lease 外・
  `synvelis#128` へ defer）
- sv#559 占有パス（`apply/entity.rs`・`variant/*`・
  `cb06_strip_*` 等）への一切の変更
- `PrepareRequest`/wire schema・`ResourceKind`/
  `ENUMERATED_KINDS` への `PluginInstance` kind 追加
- docs index（`docs/**/README.md`）の編集・再生成
