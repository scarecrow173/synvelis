---
id: "PLAN-552SendGainSeal05"
type: "plan"
status: "approved"
title: "Issue #552 plan — SetSendGainTarget seal 解放 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/552"
    - "docs/specs/issue-552-sendgain-seal.md"
    - "docs/designs/issue-552-sendgain-seal.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/ctl_02.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb06_strip_fields.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-552-sendgain-seal.md"
    - "docs/designs/issue-552-sendgain-seal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-552-sendgain-seal.md"
    - "docs/test-specs/issue-552-sendgain-seal.md"
---

# Issue #552 plan — SetSendGainTarget seal 解放 slice

## 分割方針

単一の整合的変更として 1 slice で実施する。domain
修正（ops.rs）と adapter positive 化
（variant/adapter.rs）は同一不変条件
（`(Send, SendLevel)` singleton）を共有するため分割
しない。テストは低層 op・RoutingEdit・strip-fields
candidate の 3 経路を pin する。

## File map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-project/src/ops.rs` | `SetSendGainTarget` apply arm: 旧 seal release + 新 seal を 1 apply で不可分適用。`release_send_level_target` helper 追加（旧 target + lane + assignment 閉包の削除）。 |
| `crates/synvelis-project/src/variant/adapter.rs` | `check_strip_routing_op_targets` の `SetSendGainTarget` arm: typed `Unsupported` を除去し positive 受理。scope 検査は維持。 |
| `crates/synvelis-project/tests/ctl_02.rs` | Some→Some rebind（閉包カスケード・undo 復元）・Some→None→Some・RoutingEdit 経路の 3 試験追加。 |
| `crates/synvelis-project/tests/cb06_strip_fields.rs` | rebind の typed `Unsupported` 実測を positive candidate edit 検証へ置換。 |
| `docs/` chain + `evidence/sv552/` | discovery/specs/designs/plans/tasks/test-specs/impl-ir/impl-exp と検証証拠。 |

## 依存関係

- `ops.rs` の release helper は `remove_control_owned`
  の target 粒度版 — 既存 helper の再利用ではなく
  send が `SendLevel` 以外を所有し得るため別関数。
- adapter の positive 化は ops.rs の release 実装が
  前提（release が無ければ rebind は oracle reject
  のまま）。同一 commit で不可分に変更する。
- `removal_supported_kind`（`apply/entity.rs`）は
  既収録 kind のみで足りるため変更不要。

## Risk register

| リスク | 対応 |
| --- | --- |
| 旧 target の閉包削除が広すぎる（send の他 entity を巻き込む） | release は旧 gain target 単位 — lane は `lane.target == old`、assignment は `target`/`depth_source == old` のみ。send-owned source・他 target は残す。ctl_02 で近傍 entity の生存を pin。 |
| Some→None の semantics 変更（unconfigure で target 削除）が既存利用を壊す | 旧挙動は孤立 `SendLevel` を残し後続 rebind を不能にしていた — canonical unconfigured 表現（`AddSend{gain_target:None}`・legacy send）に揃える方向の修正。既存 test の走査で退行なしを確認。 |
| variant candidate で release が scope removal check に抵触 | 離脱 entity は全て `removal_supported_kind` 収録かつ実削除 — `check_strip_fields_scope_removals` が supported deletion として受理。cb06_strip_fields.rs で実測。 |
| sv#553 同時 dispatch との編集近接 | 本 slice は `variant/adapter.rs` の `SetSendGainTarget` arm のみ — sv#553 占有の `synvelis-gateway`・cb-06 doc chain とは非重複。統合照合は Coordinator。 |

## Verification matrix

| 層 | 検証 |
| --- | --- |
| domain | `mise run test synvelis-project` — ctl_02 の rebind 3 試験（低層・unconfigure・RoutingEdit）+ cb06_strip_fields の candidate positive。 |
| 型/lint/fmt | `mise run check synvelis-project`・`mise run lint synvelis-project`・`mise run fmt-check`。 |
| hooks | `mise exec -- prek run --all-files`。 |
| headless AC | rebind 受理・半端 seal 非発生・typed Unsupported → positive を上記試験で実測。実 PCM/plugin/GUI/RT は NotRun。 |
