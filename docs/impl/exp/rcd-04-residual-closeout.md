---
id: "EXP-rcd04-residual-closeout-consolidate"
type: "experiment"
status: "completed"
title: "RCD-04 residual closeout — AC 照合の consolidate 手法と簿記 drift 修正"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/rcd-04-residual-closeout.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-04-residual-closeout-design.md"
    - "docs/test-specs/rcd-04-residual-closeout.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-residual-closeout.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/rcd-04-residual-closeout.md"
    - "docs/discovery/rcd-04-residual-closeout.md"
    - "evidence/sv118/rcd-04-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-04 residual closeout — AC 照合の consolidate 手法と簿記 drift 修正

## 仮説

RCD-04 の AC 照合は、新規 fixture/test を追加して再 derive
するより、merge 済み PR（#219/#300/#319/#374）に対応する
既存 test 名を `rg`/`#[test]` 列挙で確認し、canonical lane を
実測して consolidate する方が、簿記 slice の誠実性と工数の
両方で優れる — ただし「merge 記録の test 名が現行 main に
実在するか」「task/impl status が実態と乖離していないか」は
検証が要る。

## 方法

- Issue #118 本文の受入条件 4 行を抽出し、各行について
  `crates/*/tests/rcd_04_*.rs` の実在 test 名を列挙して
  merge 済み PR 記録と対応付けた（再 derive なし・引用のみ）。
- merge 済み 3 task file・3 impl/ir file の frontmatter
  `status` と Done When/Verification の checkbox を、対応
  merge PR 番号と突き合わせて drift を特定した。
- canonical lane（`mise run test synvelis-project` /
  `synvelis-recording` / `synvelis-player` /
  `synvelis-render` / `test-rt`、opt-in `synvelis-app`）を
  現在 main で実測し、exit code を evidence に記録した。
- `docs/**/README.md` index は一切触れず、`doc_maintenance.js
  index apply` も実行しなかった（Coordinator が統合時に
  再生成）。

## 観測結果

- Issue #118 の AC 4 行は全て merge 済み headless 実装に
  1:1 で対応付けられた（AC-1→PR #219/#319、AC-2→PR
  #219/#374、AC-3→PR #219/#300、AC-4→PR #219 + lane 全体
  pass）。実 GUI・実 device・実 RT・consumer 分は外部 gate
  として NotRun/Deferred に分離した。
- 簿記 drift は想定どおり merge 済み 3 task（`in-progress`）
  + 3 impl/ir（`in-progress`）の計 6 file に限定された。
  `docs/tasks/rcd-04-task1-comp-midi-fixture.md` と
  `docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-
  fixture-slice.md`（PR #219 分）は既に `done`/`completed`
  で整合済み・修正対象外だった。
- canonical lane の実測結果は `evidence/sv118/
  rcd-04-closeout-verification.md` に記録（実行した lane の
  み Pass。opt-in `synvelis-app` も実行して結果を記録）。
- route_graph router は pre-existing repo-wide baseline の
  `broken-relation` finding（本 slice 起因ではない）で
  `probe` edge が blocked を返したため、doc-* skill の
  sub-flow を直接辿って chain を作成した（RCD-02 先例と
  同じ対処）。

## 結論

採用: AC 照合は既存 test/merge 記録の consolidate +
canonical lane 実測のみで行い、新規 fixture/test は作ら
ない。簿記 drift は status と stale checkbox の事実整合に
限定し、当時の作業記録本文は書き換えない。外部 gate は
NotRun/Deferred のまま正直に記録し、headless の PASS を
昇格しない。docs index は Coordinator が統合時に再生成
するため本 slice では未同期のまま残す。
