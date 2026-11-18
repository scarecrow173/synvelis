---
id: "IMPLIR-552SendGainSeal08"
type: "impl"
status: "completed"
title: "Issue #552 impl-ir — SetSendGainTarget seal 解放の実装記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/552"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-552-sendgain-seal.md"
    - "docs/test-specs/issue-552-sendgain-seal.md"
  implemented-by:
    - "docs/impl/exp/issue-552-sendgain-seal.md"
  depends-on:
    - "docs/tasks/issue-552-sendgain-seal.md"
    - "docs/test-specs/issue-552-sendgain-seal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-550-strip-routing-ops.md"
    - "crates/synvelis-project/src/ops.rs"
    - "crates/synvelis-project/src/variant/adapter.rs"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/issue-552-sendgain-seal.md"
    - "docs/test-specs/issue-552-sendgain-seal.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-552-sendgain-seal.md"
    rejected: []
---

# Issue #552 impl-ir — SetSendGainTarget seal 解放の実装記録

## 実装対象

- `crates/synvelis-project/src/ops.rs`
  `ProjectOp::SetSendGainTarget` apply arm +
  `release_send_level_target` helper。
- `crates/synvelis-project/src/variant/adapter.rs`
  `check_strip_routing_op_targets` の
  `SetSendGainTarget` arm（typed `Unsupported` 除去）。
- `crates/synvelis-project/tests/ctl_02.rs` —
  3 試験追加。
- `crates/synvelis-project/tests/cb06_strip_fields.rs`
  — rebind 節を positive 検証へ置換。

## 変更内容

### ops.rs

- apply arm 冒頭で `previous =
  project.routing.sends.get(send_id)
  .and_then(|send| send.gain_target)` を捕捉。
- `send.gain_target` を `*gain_target` に更新。
- `previous == Some(old)` かつ `Some(old) != *gain_target`
  かつ旧 target が `owner == Send(*send_id)` かつ
  `role == SendLevel` を保持する場合のみ
  `release_send_level_target(project, old)` を呼ぶ。
- helper は `old` を `target` とする `AutomationLane`、
  `old` を `target` または `depth_source` とする
  `ModulationAssignment` を除去し、`old` の
  `ParameterTarget` を削除する。非失敗のみ。
- その後、従来どおり新 target へ
  `owner = ControlOwner::Send(*send_id)`・
  `role = ParameterRole::SendLevel` を seal。

### variant/adapter.rs

- `check_strip_routing_op_targets` の
  `SetSendGainTarget` arm から「previous SendLevel seal
  cannot be released」の typed `Unsupported` 分岐を
  除去。send の `SendConfig` scope 検査と新 target
  の `ParameterTarget` scope 検査（`require_scope`）
  は維持。

### tests/ctl_02.rs

- `send_gain_rebind_releases_the_previous_seal`:
  lane（`target = old`）・assignment 2 件
  （`target = old`、`depth_source = old`）を持つ
  bind 済み send を `Some(old) → Some(new)` rebind。
  受理・旧閉包削除・近傍生存・singleton 1 件・
  undo bit-for-bit を pin。
- `send_gain_unconfigure_then_rebind_leaves_no_orphan_seal`:
  `Some → None → Some(other)` — unconfigure で
  孤立 seal が残らず後続 bind が受理されることを pin。
- `send_gain_rebind_via_routing_intent_releases_the_seal`:
  `RoutingEditIntent` 経路で Some→Some rebind と
  `target: None` unconfigure を pin。

### tests/cb06_strip_fields.rs

- `cb_strip_550_sends_ops_mask_gated` の rebind 節を
  typed `Unsupported` 実測から positive candidate
  edit 検証へ置換。candidate 内で rebind 受理・
  旧 target が candidate から除去・live が
  pre-rebind 状態を保持することを検証。

## 変更しなかった面

- `control.rs`・`project.rs`・`routing_tx.rs`・
  `apply/entity.rs`・`variant/scope.rs` は無変更。
- gateway wire surface は無変更。
- docs index（`docs/**/README.md`）は未同期のまま。
