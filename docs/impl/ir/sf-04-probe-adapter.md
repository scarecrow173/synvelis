---
id: "IMPL-O1aILQdAh3pWRFZXBCqEHy"
type: "impl"
status: "in-progress"
title: "SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約 実装記録"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
metadata:
  experiments:
    adopted: []
    rejected: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/sf04_probe_acceptance.rs"
      - type: "file"
        path: "docs/specs/sf-04-probe-adapter.md"
      - type: "file"
        path: "docs/designs/sf-04-probe-adapter-design.md"
      - type: "file"
        path: "docs/plans/sf-04-probe-adapter-plan.md"
      - type: "file"
        path: "docs/tasks/sf-04-probe-adapter-task.md"
      - type: "file"
        path: "docs/test-specs/sf-04-probe-adapter.md"
      - type: "file"
        path: "docs/impl/ir/sf-04-probe-adapter.md"
      - type: "file"
        path: "docs/impl/exp/sf-04-probe-adapter.md"
      - type: "file"
        path: "evidence/sv107/coverage.md"
      - type: "file"
        path: "evidence/sv107/verification.md"
      - type: "file"
        path: "evidence/sv107/external-gates.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-04-probe-adapter-plan.md"
    - "docs/tasks/sf-04-probe-adapter-task.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-04-probe-adapter-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
    - "docs/impl/ir/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/impl/ir/sf-04-slice8-probe-faults-declare.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-probe-adapter-design.md"
    - "docs/plans/sf-04-probe-adapter-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sf-04-probe-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約 実装記録

## Summary

Issue #107 の headless 受入面を SF-PROBE-01..08 へ写像する集約
タスク。既存 slice1..10 の実装を監査し、未被覆だった 2 つの
headless 分岐（bound probe の entity 消失 unbind・per-key
subscription dedup 境界）を `sf04_probe_acceptance.rs` の新規
テスト 2 件で固定した。`synvelis-project`/`synvelis-player` の
src 変更は監査で contract 違反が見つからなかったため行っていない。

## Context

- 実装の本体は slice1（semantic resolve + ProbeSession）・
  slice2（trace bridge）・slice3（clock epoch）・slice5
  （workspace subscriber lease）・slice6（compare）・slice8
  （faults/declare）・slice9（discontinuity declare）・slice10
  （compare op）で全て merge 済み。
- 本タスクの差分は「受入写像の固定」と「未被覆分岐の証拠補強」。
- `evidence/sv107/` に被覆表と外部 Gate の NotRun/Unsupported
  判定を記録する。

## Implementation

### Changes

- `crates/synvelis-player/tests/sf04_probe_acceptance.rs`（新規）:
  - `sf04_probe_removed_entity_unbinds_and_reattach_binds_fresh`:
    Processor entity を `RemoveProcessor` で消して republish すると
    binding が `Unresolved`・`runtime_probe_id = None` になり、
    同 key 再 request でも復活しないこと。新しい processor id で
    re-attach すると gen3 で fresh `BoundActive`・新
    `RuntimeProbeId` になること（old slot reuse 誤観測なし）。
  - `sf04_probe_same_key_request_is_idempotent_single_install`:
    同一 `ProbeSubscriptionKey` の重複 request が冪等であること、
    `probe_capture` 中の重複が `AlreadyCapturing` で fail-closed
    であること、`session_id` 違いの key は独立 subscription として
    install されること（player 層 dedup は key 単位、cross-window
    lease は slice5 の契約）。
- `evidence/sv107/{coverage,verification,external-gates}.md`:
  SF-PROBE-01..08 の被覆写像・検証結果・外部 Gate 判定。
- 文書チェーン `docs/{specs,designs,plans,tasks,test-specs,
  impl/ir,impl/exp}/sf-04-probe-adapter*.md`。

### Decisions During Implementation

- **src 変更なし**: 監査で `resolve_probe_point`・`ProbeSession::
  on_publish`・broker install・trace/compare の contract 違反は
  見つからなかった。entity 消失時の unbind は既存の
  `on_publish` re-resolve が正しく `Unresolved` を返すことで
  実現されており、テストでその契約を固定するに留めた。
- **テスト配置**: `synvelis-player/tests/` の headless 統合テスト
  として配置。`RoutingEngine` + `publish` + `probe_drain` の
  RT-observed 経路で外部観測可能な binding 状態を主張する。
- **per-key dedup の主張範囲**: player 層は `ProbeSubscriptionKey`
  単位の冪等のみを保証し、window/inspector 間の lease 集約は
  slice5（synvelis-workspace）の契約と明確に分離した。
- **外部 Gate**: Ruviform #308/#309 は CLOSED のため契約レベルの
  fault injection/alignment 証拠は有効だが、実機・実 plugin・実
  device・GUI（#108）・Domain Oracle（#109）の gate は
  NotRun/Unsupported として正直に記録する。

## Related Experiments

- `docs/impl/exp/sf-04-probe-adapter.md` — 被覆監査の過程と判断記録。

## Validation

（検証実行後に記録 — `mise run fmt-check`/`check`/`lint`/`test`/
`test-rt`/`check-ruviform-rev` の結果を `evidence/sv107/
verification.md` と併せて更新する）

## Risks

- `check-ruviform-rev` が vendor REV 不一致（lock `fd1d4d5f` vs
  vendor `20a24a66`）を報告する可能性 — 本タスクの編集範囲外
  （`ui/vendor`・`Cargo.lock`）。`mise run sync-ui-deps` の実行は
  生成物更新を伴うため、範囲確認が必要。解消しない場合は
  evidence/PR に NotRun として記録する。
- entity 消失テストが想定と異なる state を返した場合は contract
  違反の可能性があるため、`on_publish`→`resolve_probe_point` の
  返却を確認して所有境界で対処する。

## Follow-ups

- 実機/実 plugin/実 device gate、GUI 表示（#108）、Domain Oracle
  （#109）は後続 Issue の責務。
- `check-ruviform-rev` の vendor 同期は本タスク範囲外のため、
  必要なら Coordinator 経由で `sync-ui-deps` の範囲拡張を依頼する。
