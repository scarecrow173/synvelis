---
id: "IMPL-034Ut8egNEzslbJ5OXxdeK"
type: "impl"
status: "completed"
title: "PC-C6 oracle REL/TR headless fixture 実装記録"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/pc_c6_oracle_rel.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/pc_c6_oracle_tr.rs"
      - type: "file"
        path: "docs/specs/pc-c6-oracle-rel-tr.md"
      - type: "file"
        path: "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
      - type: "file"
        path: "docs/designs/pc-c6-oracle-rel-tr-design.md"
      - type: "file"
        path: "docs/plans/pc-c6-oracle-rel-tr.md"
      - type: "file"
        path: "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
      - type: "file"
        path: "docs/tasks/pc-c6-task1-oracle-rel-tr.md"
      - type: "file"
        path: "docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md"
    modified:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/tasks/pc-c6-task1-oracle-rel-tr.md"
    - "docs/plans/pc-c6-oracle-rel-tr.md"
    - "docs/specs/pc-c6-oracle-rel-tr.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/pc-c6-task1-oracle-rel-tr.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-c2-control-relation-index-projection.md"
    - "docs/impl/ir/pc-c3-control-trace-projection.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
    - "docs/plans/pc-c6-oracle-rel-tr.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
  verified-by:
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C6 oracle REL/TR headless fixture 実装記録

## Summary

Issue #103（PC-C6 Control Oracle）のうち、main に契約本体が揃っている
2 行 — PC-C2-REL01〜07（`RelationIndex`）と PC-C3-TR01〜08
（`ControlTrace`）— を headless oracle fixture test として実装した。
`crates/synvelis-project/src/` 以下の契約本体・既存 test・他 crate・
Cargo 設定は一切変更せず、新規 test 2 ファイルと doc チェーンのみ。

全 15 case で、実装のメソッドを呼ばずに文書化契約から独立に導出した
期待値（edge/state/coverage/ordered relations、freshness 分類、
coherence 判定）と `RelationIndex::query` / `ControlTrace` の 3 つの
freshness entry point + `is_coherent` を field 単位で照合し、全て
一致（Pass）を確認した。Fake/Headless 結果を実機成功に繰り上げる
記述はなく、実 GPUI/plugin/OS/RT・player dispatch ledger・Ruviform
追従は NotRun/Deferred として spec `defers` と本記録に明記。

## Context

Spec: docs/specs/pc-c6-oracle-rel-tr.md
ADR: docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md
Design: docs/designs/pc-c6-oracle-rel-tr-design.md
Plan: docs/plans/pc-c6-oracle-rel-tr.md
Test spec: docs/test-specs/pc-c6-e2e-oracle-rel-tr.md
Task: docs/tasks/pc-c6-task1-oracle-rel-tr.md

前提実装: `RelationIndex`（PR #204・`tests/pc_c2_relation.rs`）、
`ControlTrace`（PR #206・`tests/trace_c3.rs`）。fixture 構築は両者と
同じく `Project::control` 直接 map 変更 + `ProjectRevision` tag。

## Implementation

### Changes

- `crates/synvelis-project/tests/pc_c6_oracle_rel.rs`（新規）:
  `FixtureModel`（project/model を同期させる宣言モデル）+
  `OracleIndex`（canonical edge 導出・dangling→Unresolved・
  disabled→Disabled・非 Disabled 辺グラフ上の相互到達による循環
  →Unresolved・有界 BFS）を独立実装し、`assert_query_agrees` で
  `RelationReply` 全体（coverage・revision・順序付き relations）を
  照合。REL01 direction 行列、REL02 LFO/Macro/StepSeq + Depth
  driver、REL03 重複 assignment + disabled、REL04 enabled/disabled
  cycle + dangling + Unsupported 非出力、REL05 depth/edge 予算と
  deterministic truncation、REL06 stale/unavailable/revision delta/
  削除、REL07 大規模 graph をカバー。
- `crates/synvelis-project/tests/pc_c6_oracle_tr.rs`（新規）:
  `oracle_context_stale`/`oracle_stamp_freshness`/
  `oracle_channel_freshness`/`oracle_freshness`/`oracle_is_coherent`
  を契約文書から独立再実装し、`assert_trace_agrees` で
  `freshness`/`plugin_reported_freshness`/`observed_freshness`/
  `is_coherent` を照合。TR01 stage 独立性（slot 非 Present 4 variant
  の個別写像含む）、TR02 7 mismatch 次元 + 複合 mismatch の check
  order、TR03 seek/pause/loop/restart/plan republish/project edit の
  Stale 化と key 内部 coherence の分離、TR04 plugin 不在/unknown vs
  probe Lost・dispatch Reported/Unavailable/Dropped、TR05 stamp
  provenance/age 行列（now==deadline・now==stamp.time・i64::MAX
  deadline 溢れの境界含む）、TR06 measured silence vs channel
  state、TR07 同一 probe 証拠の 2 context 独立判定、TR08 256 回反復
  の決定性 + `ControlTrace` 固定サイズ sanity bound + last-good
  plan をカバー。
- doc チェーン 6 件 + 生成 index 6 件 + 本記録。

### Decisions During Implementation

- oracle の循環判定は impl の Kosaraju SCC と等価な「辺 u→v の v から
  u へ非 Disabled 辺で到達可能か」の辺毎 reachability で独立実装。
  depth_source を持たない fixture では辺グラフが構造上 DAG のため
  cycle pass を正当に skip（REL07 で適用）。
- REL07 は Issue の「数万 node 級」要件に対し 32,200 nodes /
  32,000 edges を採用。実測値は `eprintln!` に記録し、sanity 上限
  （30s）のみ assert — 実閾値は baseline 未取得のため Issue どおり
  deferred。
- TR の fixture 値は `trace_c3.rs` と意図的に別系統（project 9 /
  target 33 / revision 21 / plan 8 / transport 13 / epoch 4 /
  loop 2 / position time(20) / max_age 2s）にし、先例 test の
  流用でなく独立 oracle 判定であることを担保。

## Related Experiments

### Adopted

<!-- なし — 実験 log は未使用。 -->

### Rejected

<!-- なし。 -->

## Validation

canonical（merge 後 `origin/main` `c1c7f0c` 取り込み済み HEAD
`91afa54` 時点）:

- `mise run fmt-check` → exit 0
- `mise run check` → exit 0
- `mise run lint` → exit 0
- `mise run test` → exit 0（workspace 全 binary `test result: ok`、
  失敗 0。`pc_c6_oracle_rel` 7 tests・`pc_c6_oracle_tr` 8 tests pass）

REL07 実測（debug、`--nocapture`）: fixture 32,200 nodes /
32,000 edges、`RelationIndex::build` 566.6ms、`OracleIndex::build`
198.9ms、query 8 件 + widest 再問合せ 全て sub-ms で oracle 一致・
決定的。

review-protocol: 上記 objective checks 全 pass 後、cooperative /
adversarial の 2 pass で docs+code をレビュー。CONFIRMED blocker
なし（P2/P3 指摘も修正済み: clippy pedantic 対応・fixture 値の
独立化・evidence 記録の精緻化）。

NotRun / Out of Scope: 実 GPUI・実 plugin・実 OS・実 VST3・実 RT
検証（#114 gate）、synvelis-player dispatch ledger 実経路との統合
照合、TRACE-C3-05 bounded history/RT adapter との結合検証、
Ruviform #28/#219/#294 consumer 照合、PC-C1-R01〜08・PC-C4-PL01〜08・
PC-C5-UI01〜07・PC-C6-COMP01〜05。全て headless fixture 証拠であり
実機成功の主張ではない。

## Risks

- oracle は契約文書の独立再実装であり、文書自体の誤読は両者に
  共通し得る（test-spec の case 表と impl の doc comment を照合
  済み）。
- doc graph router は main 既存の broken relation（本 slice 無関係:
  PC-C3 deferred free-text・RCD-01 欠落参照等）で引き続き blocked。
  本 slice の文書チェーン自体に新規 finding なし（doc_maintenance
  apply で pc-c6 関連 0 件）。

## Follow-ups

- PR 本文に検証結果・NotRun 一覧・review-protocol 結果を記録。
- 残 Issue 行（PC-C1/C4/C5/COMP・実機 gate・Ruviform 追従）は
  spec `defers` の各 Issue/PR に委譲。
