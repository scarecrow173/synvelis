---
id: "EXP-QeHl1sVbI1ZYbnQGYGNYev"
type: "experiment"
status: "completed"
title: "Target Architecture 設計レビュー 実験記録 — DAG 検査・型漏出走査・証拠分類の試行と結果"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/impl/ir/target-architecture.md"
  changes:
    added:
      - type: "file"
        path: "docs/impl/exp/target-architecture.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/target-architecture.md"
  depends-on:
    - "docs/impl/ir/target-architecture.md"
    - "docs/designs/target-architecture-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/target-architecture.md"
    - "docs/test-specs/target-architecture.md"
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/impl/ir/target-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  defers: []
  deferred-by: []
---

# Target Architecture 設計レビュー — Experiment Log

本書は synvelis#22 設計レビューで行った**調査・検証の試行と
結果**を記録する。コード変更を伴わない docs/evidence slice
のため、「実験」は証拠収集・分類・照合の試行を指す。

## E-01: doc-driven-dev-graph route の起動

- **試行**: `.agents/skills/doc-driven-dev-graph/scripts/route_graph.js`
  でグラフルートを取得。
- **結果**: `status: "blocked"`、`current: "probe"`、
  `next: "probe"`、edge なし。blockers に `artifact-graph`
  と多数の `broken-relation`（無関係な既存 docs）。
- **対応**: pre-existing の repo 全体問題であり本 slice 範囲外。
  修復せず、sv18/sv19 の docs-contract パターンに従って
  spec→design→plan→task→test-spec→ir/exp を作成。
  route の `.apm/...` パスは存在せず `.agents/` 側を使用。

## E-02: Cargo resolve グラフの cycle 検査（TA-001/TA-005）

- **試行**: `cargo metadata --format-version 1 --locked
  --offline` で resolve グラフを取得し、node ワンライナーで
  `dep_kinds.kind=="normal"` の辺のみを対象に DFS cycle 検査。
  dev/build 辺は別カウント。
- **結果**: normal-dep cycle = **0**（Synvelis resolve 全体 +
  Ruviform workspace fd1d4d5・42 member）。dev/build 辺の
  cycle も観測上 0。Synvelis→Ruviform 一方向・Ruviform→
  Consumer 依存なし。
- **学び**: `cargo tree` 目視では dev/optional/target 辺の
  分類が不明瞭なため、metadata + dep_kinds 分類で再現可能な
  記録とした（design D2）。plugin-host→audio の dev-dep 辺は
  normal 辺としては存在せず TA-005 の境界を満たす。

## E-03: `check-ruviform-rev` の stale vendor 検出と復旧

- **試行**: `mise run check-ruviform-rev` を実行。
- **結果（初回）**: fail — vendored UI revision
  `20a24a66…` が Cargo.lock resolved `fd1d4d5…` と不一致。
- **対応**: `mise run sync-ui-deps` で `ui/vendor/ruviform`
  を `fd1d4d5` へ再同期（gitignore された生成物・コミット
  対象外）。再実行で **Pass**（27 lockfile sources、REV match）。
- **学び**: synvelis#230 の既知 stale vendor 事象。lock rev
  変更後は `sync-ui-deps` が必要 — 本 slice は lock を変更
  していないが worktree の vendor が古かったため復旧した。

## E-04: Ruviform resolved rev の型漏出走査（TA-002）

- **試行**: `~/.cargo/git/checkouts/ruviform-8f0ffdb08acc57df/fd1d4d5`
  の `src/` 全域を `synvelis`・DAW 意味論型名で grep。
- **結果**: Synvelis crate 依存・Project/Track/Take/Comp/
  Automation/Undo/RenderRecipe 型の公開 API 侵入なし。
  `ruviform-ui-theme/src/generated.rs` の `pub mod synvelis_dark`
  と doc 例示の `synvelis::` 言及を検出 — DAW 型ではなく
  consumer 名 theme データ/doc のため P2 観測として分離。
- **学び**: 文字列 `synvelis` の出現と DAW 意味論型の侵入を
  区別する必要がある。theme fixture は共通契約の汚染ではない
  が、consumer 名を Ruviform ソースに持つ点は Owner が
  棚卸しする観測事項。

## E-05: GUI 採用方針の文書照合（C13/C14）

- **試行**: Synvelis `sv175`/`sv-gui-01` ADR と Ruviform
  #384 ADR・#388/#389/#176/#262 の関係を照合。
- **結果**:
  - `sv175-native-shell-driver-neutral-gpui.md` = **accepted**、
    `supersedes` で sv-gui-01 を明示。sv-gui-01 = **superseded**。
  - Ruviform #384 ADR = **proposed（仮決定）** — Driver-neutral
    契約層 + GPUI 標準 Driver で #175 と方向一致だが、
    **最終採用は #262/#388/#389 実機証拠後**と明記。
  - `plugin-gui-embedding.md`（wry `build_as_child`）= accepted
    だが WebView 時代の旧経路 — #389 で再設計予定。
  - `synvelis-app` の `native`/`webview`/`gpui-test` feature
    が GPUI/WebView/theme を optional dep として分離。
    `check-headless-deps` Pass で 10 headless crate に GUI 非含有。
- **学び**: 「採用方針の整合」と「GPUI 最終採用の PASS」は
  別物。#384 proposed を accepted とみなすと #262/#388/#389
  の実機 Gate を迂回する誤判定になるため、C13=整合 Pass・
  C14=Deferred に分離した（design D4）。

## E-06: 証拠分類の境界確認（TA-007 の原則）

- **試行**: ruviform fixture（`rt_*`/`poc_ex_*`/`g2_*`/
  `roundtrip_*`/`staged_*`/`multi_window`/`close_veto` 等）の
  存在をソース pin し、実走の要否を検討。
- **結果**: fixture は resolved rev で実在するが、別 repo の
  crate であり本 docs slice の build 対象外。`mise run test`/
  `test-rt` は package 引数必須で crate 変更もない。
- **対応**: fixture 存在を「契約実在 = Pass（pin）」、実走を
  「NotRun（先行 slice/CI の SHA 付き記録を参照）」に分離。
  alloc/block・実 OS/Plugin/GUI の実測を Pass にしない。
- **学び**: 「ソースに存在する」ことと「実行して成功した」
  ことを台帳で明確に分けないと、Mock/Headless の成功を
  実 device/Plugin/GUI 受入と混同する誤りを犯す
  （spec Evidence 分類ルールに固定）。

## E-07: pre-existing 問題の切り分け

- **試行**: route graph blocked・v0.7.1 crate 数差・
  PPQN 範囲差・legacy window 経路を評価。
- **結果**:
  - route blocked = 無関係な既存 docs の broken-relation
    が原因 → 範囲外として記録のみ（design D7）。
  - v0.7.1「28 crate」vs fd1d4d5「42 member」= M1〜M7 での
    増加による文書ドリフト。責務分担は整合 → P2 観測。
  - PPQN: Ruviform `1..=65535` vs Synvelis `1..=32767` =
    consumer が厳しい部分集合 → 整合（P3 記録）。
  - legacy window close=終了 vs multi-window close≠Job
    Cancel = 別契約 → TA-011 判定で区別記録（design D5）。
- **学び**: 「設計の否定」と「文書ドリフト/部分集合制約/
  別経路の区別」を分けることで、blocker を出すべきでない
  観測を P2/P3 に適切分類できた。

## 結論

TA-001〜012 + GUI 方針 2 条件の設計レビューを、再現可能な
コマンド・対象 Revision・fixture pin・証拠分類で完結させた。
blocker は残っていない。実装ギャップ・実 OS/Plugin/GUI 受入・
AudioLens 互換は各 Owner Issue に Deferred 接続した。
