---
id: "TSPEC-034Uj7Mjz5Aoak1Vz7O3S4"
type: "test-spec"
status: "approved"
title: "SF-03 slice5 受入テスト仕様 — session→routing activation e2e"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  verified-by: []
  references:
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
  defers: []
  deferred-by: []
---

# SF-03 slice5 受入テスト仕様 — session→routing activation e2e

## Purpose

SF-03 slice5 の受入契約を固定する。spec
`sf-03-slice5-session-engine-connect.md` の R1–R7 を、
`crates/synvelis-app/tests/sf03_session_engine.rs` として
`ProjectSession::apply` → `SessionRoutingEngine` → `RoutingEngine`
の session 経路で実測する。GUI/実機・`syn.*` op・SF-ACT-07/08 は
対象外。

## Feature

session が committed revision ごとに `EnginePort` へ
`(&Project, revision)` を通知し、`SessionRoutingEngine` が
`RoutingDeps`/`RoutingCompileRequest` を組み立てて
`RoutingEngine::prepare`→`publish` を駆動する。RT `run` による
generation observe で `Active` receipt が立ち、失敗 build では
last-good が維持され、semantic no-op では equivalence で再 build
が省略され、revision ずれは stale として拒否される。

## Rules

- Rule 1（通知境界）: `apply`/`undo`/`redo` の commit 成功時のみ
  通知され、失敗 command・view/layout 系では `graph_builds` が
  増えない。
- Rule 2（publish→activation 区別）: `session.apply` 直後の receipt
  は `Published`（または Blocked/Failed）であり、`Active` は
  `run` が generation を observe した後にのみ立つ。
- Rule 3（last-good）: prepare/publish 失敗後も receipt の
  `last_good_generation` が直前の live generation を指し、
  `run` が旧 plan の処理を継続する。
- Rule 4（NoRebuild）: semantic fingerprint 同値の revision では
  `Equivalent` となり generation 消費・receipt 変化がない。
  `graph_builds`（通知回数）は増える。
- Rule 5（stale）: `session.apply` の expected revision 不一致は
  controller が拒否し engine に届かない。controller bypass による
  revision ずれは engine の reconcile/key 照合で
  `ProjectRevision`/`StaleProjectSnapshot` 系として観測される。
- Rule 6（adapter 失敗）: clip 解決失敗時は engine を呼ばず
  `last_prepare_error` に記録し、live plan/receipt を維持する。

## Examples

- `session_apply_publishes_and_activates`: import→track→clip の
  project で `session.apply` 後 receipt が `Published`、`run(1,
  Every(1))` 後 `Active` かつ `last_good_generation` が立つ。
- `failed_build_keeps_last_good`: 有効 plan 公開後に unresolved
  processor binding を `session.apply` → receipt `Failed`+
  `last_good_generation`、旧 generation が `run` で継続。
- `semantic_noop_is_equivalent`: project rename の `session.apply`
  で通知は届くが generation/receipt が不変。
- `stale_revision_rejected`: `session.apply(op, ずれた revision)` が
  `Err(StaleRevision)` で `graph_builds` 不変。controller 直接
  変更後の `reconcile` が revision mismatch を報告。

## Guarantees

- engine の build-key 検証・equivalence・last-good・reconcile・
  generation-scoped control は engine 既存契約に委譲され、
  session/adapter 側で再実装しない。
- `ProcessorHost` 注入境界（`set_processor_host`）経由で chain
  processor の connect/lease/teardown が行われる。

## Non-goals

- GUI 表示・`AudioRuntimeSlot` 実接続・実デバイス検証。
- `syn.*` service op・preview policy・automation lowering。
- 複数 project 同時 engine・project 切替の世代連続性。

## Risk

- `plan_playbacks` 二重呼出しの非決定性 → canonical_of cache と
  同一 engine config で決定的であることを test で担保。
- `Rc<RefCell>` handle の borrow 競合 → test は逐次呼出し。
