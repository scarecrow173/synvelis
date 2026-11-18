---
id: "TSPEC-552SendGainSeal07"
type: "test-spec"
status: "approved"
title: "Issue #552 test-spec — SendGainTarget seal 解放の検証仕様"
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
    - "docs/specs/issue-552-sendgain-seal.md"
    - "docs/designs/issue-552-sendgain-seal.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-552-sendgain-seal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/tasks/issue-552-sendgain-seal.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-552-sendgain-seal.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/issue-552-sendgain-seal.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #552 test-spec — SendGainTarget seal 解放の検証仕様

## 検証対象

`crates/synvelis-project/tests/ctl_02.rs` の rebind 節
（低層 op・unconfigure・`RoutingEditIntent` 経路）と
`crates/synvelis-project/tests/cb06_strip_fields.rs`
の `cb_strip_550_sends_ops_mask_gated` 内 rebind 節
（strip-fields candidate 経路）。

## Rules

- R1: Some→Some rebind が受理され、`send.gain_target`
  が新 target を指し、新 target が
  `owner = Send(send)`・`role = SendLevel` に seal
  される。send owner の `SendLevel` target は高々
  1 件（singleton 不変条件）。
- R2: release された旧 target は `control.targets`
  から消え、その `AutomationLane` と、`target` または
  `depth_source` として触れる `ModulationAssignment`
  も消える。近傍 entity（無関係 target・modulation
  source）は生存する。
- R3: release + seal は 1 apply = 1 undo entry —
  undo で rebind 直前の project が bit-for-bit 復元
  される。
- R4: Some→None unconfigure が孤立 `SendLevel` を
  残さず canonical unconfigured 表現
  （`gain_target: None`・SendLevel target 無し）に
  なる。後続の None→Some（別 target）が受理される。
- R5: `RoutingEdit::SetSendGainTarget`（`Some`・
  `None` 双方）が低層 op と同一の release/seal
  semantics で受理される。
- R6: strip-fields candidate edit で rebind が
  positive 受理され、旧 target の離脱が supported
  removal として扱われ、live は不変のまま。
  typed `Unsupported` は発生しない。

## Examples

- `send_gain_rebind_releases_the_previous_seal`:
  lane + assignment（`target`/`depth_source` 両経路）
  を持つ bind 済み send を別 target へ rebind —
  受理・閉包削除・近傍生存・singleton 1 件・undo
  bit-for-bit を検証。
- `send_gain_unconfigure_then_rebind_leaves_no_orphan_seal`:
  Some→None→Some — unconfigure で孤立 seal が残らず
  後続 bind が受理されることを検証。
- `send_gain_rebind_via_routing_intent_releases_the_seal`:
  `RoutingEditIntent` 経路で Some→Some rebind と
  `target: None` unconfigure を検証。
- `cb_strip_550_sends_ops_mask_gated`（改訂）:
  candidate 内 rebind が受理され旧 target が
  candidate から除去されること、live が pre-rebind
  状態を保持することを検証。

## NotRun / Unsupported

- 実 PCM/plugin・実 GUI・RT lane: 本 slice は domain
  semantics の headless 修正であり対象外（NotRun —
  `evidence/sv552/` に記録）。
- gateway wire positive 実測の追加
  （`cb06_strip_gateway.rs` への rebind wire 試験）:
  `synvelis#553` 占有のため本 slice では実施しない
  （wire lowering は既存・domain 修正で自動的に通る）。
