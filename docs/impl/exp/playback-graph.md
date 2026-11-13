---
id: "EXP-01M7BGZFXRZ5H76B3GG0J0"
type: "experiment"
status: "completed"
title: "Playback Graph — Issue #18 設計契約文書化と EX 写像の試行"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/18"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/playback-graph.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/playback-graph-design.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/playback-graph-task.md"
    - "docs/test-specs/playback-graph.md"
  defers: []
  deferred-by: []
---

# Playback Graph — Issue #18 設計契約文書化と EX 写像の試行

## 問題設定

Issue #18 の Playback / Mixer / Generic Execution Graph 設計契約を
canonical 文書鎖として確立する必要があった。上流 baseline
（v0.4 architecture）と SF-03 map 設計の headless 実装済み契約は
存在するが、Issue #18 が要求する Typed Port・PDC・Plan Activation・
EX-001〜020 を一つの正本 + evidence 台帳として固定する記述が
なかった。本 slice は docs のみで crate を変更しないため、
(a) 7 文書鎖の生成、(b) EX→既存 test/oracle/状態の写像、
(c) doc graph/index/review の整合確認が試行対象となった。

## 試行と結果

### E1: 上流契約と実装の照合

`docs/designs/sf-03-map-semantic-runtime-mapping-design.md`・
`docs/specs/l1-*`/`l2-*`/`x-*`/`tim-02-*`・`crates/synvelis-player`
の `sf03_*`/`tim*`/`l1_e2e`/`md_b` fixture・`crates/synvelis-render`
の `sf03_render`/`tim04_*`/`l2_e2e`/`x_e2e`/`rcd_04_rt_comp_pcm` を
照合し、EX-001〜020 の各ケースに対応する既存 headless 証拠を
特定した。SF-03 map 設計の世代状態機械・fault 18 分類・reconcile
優先順位・PDC lowering 同一性・consumer 同一 revision 契約が
Issue #18 の要求と一致することを確認した。

**結果**: EX→test 写像が可能。ただし全ケースが EX 全体の独立
oracle を満たすわけではなく「Pass(部分)」に留まるものが多い。
EX-017 は test でなく契約記述（非保証明示）のため「設計記録」。

### E2: Owner 分離・PDC 責務の境界確認

Synvelis semantic（`synvelis-project`）と Ruviform generic
（typed graph/compiler/RT/PDC/plugin host）の依存方向が
`synvelis-* → ruviform-*` 一方向であること、DAW 型が generic
runtime に逆流しないことを design の Owner 層表で固定した。
semantic lowering は PDC を直接符号化せず、realtime/offline で
同一 topology を lower することで同一意味の compensation を
可能にする責務分割を確認した。

**結果**: Issue #18 の Owner 分離・PDC 責務要求と一致。
Ruviform arbitrary-port DAG の完成度は upstream 受入
（#184/#308/#309）に依存し、Synvelis 側で恒久コピーしない方針を
記録した。

### E3: Evidence honesty の分類規則

headless Pass・design-review Pass・実機 Pass を区別し、実
device/plugin/OS GUI/consumer lane を capability 別
Pass/Fail/Unsupported/NotRun として分類する規則を test-spec
PG-T-R-08 として固定した。「Pass(部分)」表記で headless で関連
契約が確認できるが EX 全体の独立 oracle を満たさないケースを
限定した。

**結果**: evidence ledger で EX-001〜020 を「Pass(部分)」
（headless 関連契約確認）・「設計記録」（EX-017）・capability
gate 別 NotRun/Deferred に分類できた。headless 証拠を実機受入に
繰り上げない規則を明文化した。

### E4: 文書鎖生成と index/relation 整合

`new_spec`/`new_design`/`new_plan`/`new_task`/`new_test_spec`/
`new_impl_record` の scaffold で 7 文書を生成し、front matter の
`source`/`depends-on`/`related`/`derives-from`/`derived-by`/
`implements`/`verifies`/`references`/`defers` を相互に整合させた。
`doc_maintenance.js index apply` で触った dir の index を再生成した。

**結果**: 7 文書 + evidence/sv18 2 件が存在。index は生成物として
再生成。`doc_status lint`/`audit_impl_record`/`route_graph`/
`review-protocol` の結果は `evidence/sv18/playback-graph-verification.md`
に記録。

## 採用した判断

- fault 18 分類は Issue #106 §5 を正本として採用し、Issue #18 の
  13 失敗状況へ対応表で写像。pending activation は fault でなく
  「Published 継続」状態として区別した。
- Project revision（semantic）と execution-plan revision/generation
  （runtime）を別 receipt として記録し、NoRebuild でも新 Revision
  が Active になった偽 receipt を出さない規則を PG-R-15/PG-T-R-05
  に固定した。
- 「Pass(部分)」表記を新設し、headless で関連契約が確認できるが
  EX 全体の独立 oracle を満たさないケースを明示した。

## 結論

上流契約（v0.4 architecture・SF-03 map 設計・L1/L2/X/TIM-02）と
実装済み headless fixture が Issue #18 の要求と整合することを確認し、
EX-001〜020 を既存証拠へ写像した canonical 文書鎖を確立した。
実装・実 PCM・実 plugin・実 device・GUI・OS 受入は一切主張せず、
それらは #104〜109・#114・ruviform#294・EXT-L3 に委譲する。
