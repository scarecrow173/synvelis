---
id: "TSPEC-jzXzcw5lLdoj5CBmmoL86c"
type: "test-spec"
status: "approved"
title: "SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約 テスト仕様"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/sf-04-probe-adapter.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-04-probe-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-probe-adapter-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/test-specs/sf-04-slice8-probe-faults-declare.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-probe-adapter-design.md"
    - "docs/test-specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/test-specs/sf-04-slice8-probe-faults-declare.md"
  defers: []
  deferred-by: []
---

# SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約 テスト仕様

## Feature: SF-PROBE-01/03 — bound probe の entity 消失と再 attach

### Rule: SourceMap 失効は honest unbind

bound 状態の probe が指す semantic entity が project から消えた
場合、republish 後の re-resolve は `ProbeResolveState::Unresolved`
を返し、runtime probe は取り外され `runtime_probe_id = None`
になる。stale slot・旧 generation の観測を継続してはならない。
同じ key の再 request は冪等に失敗し、binding を復活させない。

#### Example: `sf04_probe_removed_entity_unbinds_and_reattach_binds_fresh`

- Given: Processor entity に `BoundActive` の Monitor probe
- When: `RemoveProcessor` → republish（gen2）
- Then: binding は `Unresolved`・`runtime_probe_id = None`・
  capture segment なし
- And: 同 key 再 request でも `Unresolved` のまま
- When: 新しい Processor id を insert → republish（gen3）→
  新 key で request
- Then: 新 binding は gen3 で `BoundActive`・新
  `RuntimeProbeId`、旧 binding は `Unresolved` のまま

## Feature: SF-PROBE-06 — subscription dedup 境界

### Rule: player 層の dedup は `ProbeSubscriptionKey` 単位

同一 key の重複 request は冪等（runtime probe を増やさず
`requested_mode` も変更しない）。mode 変更は `probe_set_mode`、
capture 開始は `probe_capture` の責務。`probe_capture` 中の重複
request + 再 `probe_capture` は `AlreadyCapturing` で fail-closed
（capture session は binding あたり 1 つ）。`session_id` が異なる
key は別 subscription として独立 install される — window/
inspector 間の lease 集約は session 層（slice5）の契約であり、
player 層は重複排除しない。

#### Example: `sf04_probe_same_key_request_is_idempotent_single_install`

- Given: strip Output に `BoundActive` の Monitor probe
- When: 同 key を `ProbeMode::Capture` で再 request
- Then: `runtime_probe_id` 不変・`requested_mode` は `Monitor` のまま
- When: `probe_capture` → 同 key 再 request → 再 `probe_capture`
- Then: `AlreadyCapturing`（open segment は 1 つ）
- When: `session_id` 違いの同 point key を request
- Then: 別 `RuntimeProbeId` で `BoundActive`（独立 subscription）

## 被覆の扱い

既存 slice テストで被覆済みのケースは重複追加しない — 被覆表は
`evidence/sv107/coverage.md`。本テスト仕様が新規に要求するのは
上記 2 テストのみ。外部 Gate（実機/plugin/GUI/Oracle）は
`evidence/sv107/external-gates.md` で NotRun/Unsupported を記録。
