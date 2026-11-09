---
id: "ADR-OrWdkH3ASYviV5ufFtl0tD"
type: "adr"
status: "accepted"
title: "PC-C5 slice1: Relation 投影は workspace read model + verbatim coverage、index は query 毎 rebuild、探索は depth-1 hop + anchor nav-stack"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
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
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# ADR: PC-C5 slice1 — Relation UI 投影の配置・coverage 表現・探索 model

## Context and Problem Statement

Issue #102 UI-C5-01 は、PC-C2 の `RelationIndex`（project 層の
read-only 正本）を Inspector へ投影し、Parameter↔Lane/Source/
Assignment↔Parameter の双方向探索と roundtrip を headless で
検証することを要求する。投影をどこに置き、coverage/state をどう
表現し、探索をどの粒度で行うかを決める必要がある。

## Decision

1. **投影は `synvelis-workspace` の read model**（新規
   `relation_ui` モジュール）に置く。`arrangement_ui` と同じ
   「session を読み、表示 DTO を返すだけ」の形で、
   `RelationIndex::build(&Project, revision)` → `query` をそのまま
   消費する。`ControlsUi`/`ControlSession` の projection には
   追加しない — relation は control session の gesture/observation
   状態ではなく project 派生の read model であり、既存の
   `signal_flow_view`/`lane_view`（評価寄りの投影）とは責務が
   異なる。両者は併存し、本 slice では置き換えない（置換判断は
   後続 slice の設計課題として記録のみ）。
2. **coverage/state は verbatim で運ぶ**。`RelationView` が
   `RelationCoverage`（`Complete`/`Truncated`/`Stale`/`Unavailable`）
   と index generation（`project_revision`）を保持し、
   `RelationRow` が `RelationKind`/`RelationState` を保持する。
   UI 側はこれを文字列化するだけで、0 件・空文字・既定値への
   潰し込みを行わない。
3. **`RelationIndex` は query 毎に build する**。non-RT の
   publish 駆動（service op tail / cell hand-out）でしか呼ばれず、
   control model 規模の O(N) build は `arrangement_rows` の
   timeline 再走査と同程度。cache/invalidation の複雑さを
   持ち込まない。`expected_revision` は原則 live revision で、
   caller が明示した場合のみ stale query を再現できる
   （`RelationViewOptions.expected_revision`）。
4. **探索は depth-1 の双方向 query + anchor nav-stack の hop
   移動**。`relation_rows` の既定は anchor の直接 relation のみ
   （`RelationDirection::Both`）。各 row は anchor に接する edge
   なので「反対側 entity」が一意に決まり、`RelationNav` が
   anchor stack を保持して descend/return を表す。深い BFS を
   1 回で展開する方式は、row→次 anchor の対応が多値になり
   roundtrip 意味論が曖昧になるため採らない。

## Consequences

### Positive

- domain 正本（`synvelis-project::relation`）を複写せず、
  workspace は投影に徹する。PC-C2 の semantics
  （dangling/cycle→Unresolved、disabled→Disabled、bounded
  query）を UI 側で再解釈しない。
- `Stale`/`Unavailable`/`Truncated`/`Disabled`/`Unresolved` が
  型として残るため、表示層が「正直さ」を失う経路がない。
- nav-stack が純粋な値型なので、headless fixture が GUI なしに
  双方向探索・roundtrip を検証できる。

### Negative

- publish 毎の index rebuild は control model が大きくなると
  無視できないコストになり得る（現状のモデル規模では問題なし。
  必要になれば世代 cache を検討 — 今回は YAGNI）。
- depth-1 既定のため、source や影響先 parameter は anchor を
  1 hop ずつ進めないと表に出ない（静的 table は直接 relation
  のみ。GUI の行選択 navigation は後続 slice）。

## Considered Options

### Option A: workspace read model（採用）

`arrangement_ui` の先例通り、session を読んで DTO を返す
関数群を `synvelis-workspace` に置き、app は表示用に
`RelationScreenState` へ写すだけ。

-利点: domain 境界が既存パターンと一致。headless test が
workspace crate で完結する。app 側は薄い写像のみ。
- 欠点: app まで届くまでに 1 段の写像が必要（既存
  `ArrangementRow`→`MainScreenState` と同じ構造で許容）。

### Option B: `ControlsUi`/`ControlSession` の projection に追加

既存の subscribed-param projection 経路に relation rows を
載せる。

-利点: ui_ops の capture にそのまま乗り、refresh 経路を
新設しなくてよい。
- 欠点: relation は gesture/observation を持たない純粋 read
  model であり、control session の状態機械に混ぜると責務が
  濁る。`signal_flow_view` との棲み分けも曖昧になる。
  採用しない。

### Option C: app 層で `RelationIndex` を直接 query

`synvelis-app` が project を読み `RelationIndex` を直接使う。

-利点: workspace 変更が最小。
- 欠点: read model の置き場として workspace を外し、
  `arrangement_ui` 等の既存投影パターンと不整合。headless
  検証も app crate の重い依存（GPUI native 含む）に引きずられる。
  採用しない。

### Option D: 単一の深い BFS で ego-network を一括表示

`max_depth` を 2〜3 にして source・影響先 parameter まで
1 table に出す。

-利点: 静的 table の情報量が増える。
- 欠点: depth≥2 の row は anchor に接しない edge を含み、
  「この row の次にどの entity へ行くか」が一意に決まらない。
  roundtrip（UI-C5-01 の中核）を型で保証しにくい。
  採用しない — 必要なら後続 slice で表示専用の拡張として
  再検討する。

## References

- `docs/specs/pc-c5-01-relation-inspector.md`
- `docs/specs/pc-c2-control-relation-index.md` /
  `docs/designs/pc-c2-control-relation-index-design.md`
- `docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md`
  （workspace read model + MainScreenState push の先例）

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/102>
- [spec](../specs/pc-c5-01-relation-inspector.md) /
  [design](../designs/pc-c5-01-relation-inspector-design.md) /
  [plan](../plans/pc-c5-01-relation-inspector.md) /
  [task](../tasks/pc-c5-01-relation-inspector.md) /
  [test-spec](../test-specs/pc-c5-01-relation-inspector.md)
