---
id: "TSPEC-034UfG7hrZDa6LMSTkgvCQ"
type: "test-spec"
status: "approved"
title: "SF-03 slice3 plugin host 接続 test spec"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source: []
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
    - "docs/designs/sf-03-slice3-plugin-host-connect-design.md"
    - "docs/plans/sf-03-slice3-plugin-host-teardown.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice3 plugin host 接続 test spec

## Purpose

SF-03 slice3（実 plugin host 接続）の受入契約を固定する。spec
`sf-03-slice3-plugin-host-connect.md` の R1–R4 と Issue #106 slice 3
受入条件を、headless で再現可能な test として規定する。実 OS/デバイス
依存の検証は本 spec の対象外（PR の未検証欄に記録）。

## Feature

Semantic routing の lowered typed graph に対し、processor provisioning
が `ProcessorHost` boundary 経由で実 plugin host lifecycle
（Instantiate→Initialize→Activate→StartProcessing→attach）を駆動し、
接続失敗が `RoutingFault` へ決定的に写像され、generation 連動で
plugin instance が teardown され、last-good が維持される。

## Rules

- Rule 1（接続契約）: `host.connect` は semantic `Processor` と解決済み
  port 一覧・build 世代・engine 形式・max_frames を受け取り、
  `(GraphProcessor, ProcessorLease)` を返す。`ProcessorLease` は
  opaque で drop 時に host teardown を実行する。
- Rule 2（static boundary）: `StaticProcessorHost` は `ProcessorId` で
  instance を消費し、未登録は `Unavailable`（→ `PluginUnavailable`）
  を返す。no-op lease を返す。
- Rule 3（real lifecycle）: `PluginHostProcessors` は
  `ProcessorBinding::Resolved` を要求し、host command を
  Instantiate→Initialize→Activate(ProcessConfig)→StartProcessing の順で
  発行してから `attach` する。いずれかの失敗は typed error +
  best-effort teardown。
- Rule 4（fault 写像）: `Unavailable`/`Lifecycle` →
  `PluginUnavailable`、`Incompatible` → `UnsupportedFormatOrTimeMap`、
  `Invalid` → `InvalidRouting`。新規 `RoutingFault` variant は追加
  しない。
- Rule 5（世代 teardown）: publish 成功時に generation 単位で lease を
  保持し、pending 世代が `last_good` 以外で置換された時点で旧 lease を
  解放する。新世代が RT 観測で Active になった時点で他世代の lease を
  全て解放する。
- Rule 6（last-good）: connect/compile/publish/activation のいずれの
  失敗でも既存 last-good plan は live のまま維持される。
- Rule 7（offline fail-closed）: `BuildMode::Offline` では接続済み
  processor の `offline_support()` を検査し、`Unknown` を含む場合は
  `OfflineCapabilityMismatch` で fail する。
- Rule 8（RT 非依存）: host lifecycle・lease teardown は RT path で
  実行されない。`PluginNode` のみが `GraphProcessor` として graph に
  乗る。

## Examples

- Example 1（static 接続成功）: Given `StaticProcessorHost` に
  `ProcessorId` → GainProcessor を登録、When semantic chain に当該
  processor を含む project を prepare→publish→activation、Then
  processor が audio を処理し activation が観測される。
- Example 2（未登録 fault）: Given static host に未登録の
  `ProcessorId` を含む project、When prepare、Then
  `PluginUnavailable` が mint され last-good が維持される。
- Example 3（real host audio）: Given `PluginHostProcessors` に gain
  plugin（`BusReport::Reported`、semantic port 順一致）を登録、When
  prepare→publish→activation、Then 出力 audio に plugin gain が現れ、
  lifecycle が Instantiate→Initialize→Activate→StartProcessing→attach
  の順で観測される。
- Example 4（bus/bounds fault）: Given plugin の bus 表が semantic
  port と不一致、または bounds が `AttachConfig` と矛盾、When connect、
  Then `UnsupportedFormatOrTimeMap` が mint される。
- Example 5（世代 teardown）: Given 2 世代を連続 publish、When 2 世代目
  が pending を置換、Then 1 世代目の lease が解放（host instance
  destroy）される。When 2 世代目が Active 観測、Then 残存する旧世代
  lease が全て解放される。
- Example 6（接続失敗 cleanup）: Given chain に 2 processor、2 台目の
  connect が失敗、When prepare が fault を返す、Then 1 台目の lease
  が drop され teardown が走る。
- Example 7（offline fail-closed）: Given `BuildMode::Offline` で real
  host 接続 processor、When prepare、Then
  `OfflineCapabilityMismatch` が mint される（`Unknown` fail-closed）。

## Guarantees

- `sf03_l3.rs` が Rule 1–8 を網羅し、`mise run test` で green。
- 既存 `sf03_l1.rs`/`sf03_l2.rs` が `StaticProcessorHost` 移行後も
  green（slice1/2 契約の回帰なし）。
- `RoutingDeps.processors` が撤去され、直接 `Box<dyn GraphProcessor>`
  注入の経路が残っていない。
