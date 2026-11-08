---
id: "TSPEC-106SF03Ij7jKl8mNo9pQr0sTu"
type: "test-spec"
status: "approved"
title: "SF-03 slice1 テスト仕様 — Lowering Golden/SourceMap・Activation 証拠"
created: "2026-11-08"
updated: "2026-11-08"
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
  depends-on:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  verified-by: []
  references: []
---

# SF-03 slice1 テスト仕様

対象: `crates/synvelis-player/tests/sf03_l1.rs`(fingerprint・契約型の
検証を含む全受入を集約)。
実行: `mise run test`(headless)/`mise run test-rt`。
実 plugin・GUI・device は NotRun/Unsupported。

## Feature: Golden PCM と SourceMap(SF-ACT-01)

| Rule | Example | 期待 |
| --- | --- | --- |
| 単一 track semantic = legacy direct | 同一 clip 配置で `lower` と `lower_semantic`+engine 駆動の出力 PCM を比較 | sample 完全一致(加算順序保存) |
| SourceMap↔revision | `lower_semantic` 結果の `source_map.project_revision`/`build_key.project_revision` | 渡した snapshot の revision と一致 |
| Missing Source 明示 | 未登録 source の clip | `SkippedPlayback::MissingSource` に記録、node 0、silent success でない |
| SemanticPort→PortRef | enabled `Main` connection | `source_map.connections[id] == Resolved(ConnectionId)`、`ports` が実 PortRef を指す |

## Feature: 順序と tap(SF-ACT-02)

| Rule | Example | 期待 |
| --- | --- | --- |
| Pre/Post tap | `SendTap::PreFader`/`PostFader` の send | pre は strip dry 出力、post は fader 後から tap(位相/振幅差が観測できる fixture) |
| Mute/Solo prune | muted strip の outgoing edge | edge 非生成、`NotLowered(MutedStrip)` 記録 |
| Sidechain | strip→processor sidechain 接続 | processor node の sidechain port に edge、main chain と独立 |
| Processor 順序 | chain [p0,p1] に注入 gain processor | p0→p1→fader の順で signal が流れ、順序が PCM に反映 |

## Feature: Stale 防止(SF-ACT-03)

| Rule | Example | 期待 |
| --- | --- | --- |
| Project revision stale | prepare 後に別 op で revision 前進 → publish | `Blocked`/`Failed` receipt、`GraphRuntime` generation 不変(publish 0) |
| Capability drift | `port_capability_generation` が変わる変更 → publish | 同上、speculative plan は publish されない |
| Temp ID 非混入 | `RoutingPlan` temp→stable 解決後の committed project | SourceMap が stable ID のみを key に持つ |

## Feature: Activation 証拠(SF-ACT-04)

| Rule | Example | 期待 |
| --- | --- | --- |
| publish≠Active | `publish` 直後・`process` 未実行 | state は `Published` であり `Active` receipt を mint しない |
| frames=0/停止 | `process(0)` または engine 停止 | `Active` にならない |
| RT 観測で Active | `process(frames>0)` 後に `active_generation` 一致 | `Active{rev,gen,epoch}` receipt、`observed_active_generation` 記録 |
| Last-good | publish 失敗/compile 失敗 | 旧 plan が runtime に残り、receipt の `last_good_generation` が旧 gen |
| Stub 非証拠 | `EngineStub.graph_builds` のみの状態 | activation receipt を `Active` にしない(型レベルで分離) |
