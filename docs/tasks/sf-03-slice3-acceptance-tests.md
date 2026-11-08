---
id: "TASK-034UfGnJu6lN4FLgs2mZpK"
type: "task"
status: "todo"
title: "SF-03 slice3 T3 — sf03_l3 受入テスト・test-spec"
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
  implements:
    - "docs/plans/sf-03-slice3-plugin-host-teardown.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice3-plugin-host-teardown.md"
    - "docs/tasks/sf-03-slice3-engine-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice3-plugin-host-connect.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice3 T3 — sf03_l3 受入テスト・test-spec

## Work

`crates/synvelis-player/tests/sf03_l3.rs` を新規作成し、test-spec の
Feature/Rule を実測する。

- static host: `StaticProcessorHost` に GainProcessor 相当を登録し、
  prepare→publish→activation で音声が processor を通ること、
  未登録 `ProcessorId` で `PluginUnavailable` fault + last-good 維持。
- real host: `PluginHostProcessors` + `PluginHost` に test plugin
  （`BusReport::Reported` で semantic port 順と一致する bus 表、
  gain を適用する `PluginProcessor`）を登録し、
  Instantiate→Initialize→Activate→StartProcessing→attach の実経路で
  prepare→publish→activation し、出力 audio に plugin の gain が
  現れることを検証。
- fault: bus table 不一致 / bounds 不一致 →
  `UnsupportedFormatOrTimeMap`、lifecycle 拒否（未登録 plugin id や
  不正 `ProcessConfig` を誘発する plugin）→ `PluginUnavailable`、
  binding `Unresolved` → `PluginUnavailable`。
- 世代 teardown: rapid に 2 世代 publish し、pending 置換で旧 lease
  が解放されること（host 側 instance 数または instrumented counter
  で観測）。新世代 Active 観測後に旧世代 instance が destroy される
  こと。
- 接続失敗 cleanup: 2 台目の connect で失敗させ、1 台目の lease が
  drop される（= teardown 実行）ことを観測。
- offline: `BuildMode::Offline` で real host 接続 processor が
  `OfflineRenderSupport::Unknown` → `OfflineCapabilityMismatch` で
  fail-closed すること。
- `docs/test-specs/sf-03-slice3-plugin-host-connect.md` を完成させ、
  実測結果と対応 rule を記録。

## Acceptance

- `cargo test -p synvelis-player --test sf03_l3` が green。
- `mise run test` 全体で回帰なし。
- test-spec の全 Rule に対応する test が存在し、実測値が記録される。

## Classification

- `normal-plan-task`

## Upstream Documents

- <!-- plan, spec, ADR, or task path -->

## Dependencies

- <!-- prerequisite, blocker, or related task -->

## Affected Files

- <!-- file, directory, module, or interface -->

## Done When

- [ ] <!-- completion criterion -->

## Verification

- [ ] <!-- command, test, review step, or observable behavior -->
