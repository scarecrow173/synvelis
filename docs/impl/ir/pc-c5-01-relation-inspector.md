---
id: "IMPL-OABQ94C9ziv3kEtnGM5cHa"
type: "impl"
status: "completed"
title: "PC-C5 UI-C5-01 Relation Index の Inspector 投影 headless slice"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/relation_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/rel_c5.rs"
      - type: "file"
        path: "docs/specs/pc-c5-01-relation-inspector.md"
      - type: "file"
        path: "docs/adr/pc-c5-01-relation-readmodel-coverage.md"
      - type: "file"
        path: "docs/designs/pc-c5-01-relation-inspector-design.md"
      - type: "file"
        path: "docs/plans/pc-c5-01-relation-inspector.md"
      - type: "file"
        path: "docs/tasks/pc-c5-01-relation-inspector.md"
      - type: "file"
        path: "docs/test-specs/pc-c5-01-relation-inspector.md"
      - type: "file"
        path: "docs/impl/ir/pc-c5-01-relation-inspector.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
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
      - type: "index"
        path: "docs/adr/README.md"
      - type: "index"
        path: "docs/designs/README.md"
      - type: "index"
        path: "docs/plans/README.md"
      - type: "index"
        path: "docs/specs/README.md"
      - type: "index"
        path: "docs/tasks/README.md"
      - type: "index"
        path: "docs/test-specs/README.md"
      - type: "index"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/plans/pc-c5-01-relation-inspector.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c5-01-relation-inspector.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/adr/pc-c5-01-relation-readmodel-coverage.md"
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
    - "docs/tasks/pc-c5-01-relation-inspector.md"
    - "docs/test-specs/pc-c5-01-relation-inspector.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c5-01-relation-inspector.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c5-01-relation-inspector.md"
  verified-by:
    - "docs/test-specs/pc-c5-01-relation-inspector.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C5 UI-C5-01 Relation Index の Inspector 投影 — 実装記録

## Summary

Issue #102（PC-C5 Signal Flow Control Layer・Context Inspector・
Automation Lane 双方向 GUI 統合）の **UI-C5-01 slice** — PC-C2
（synvelis#99）で完成した read-only
`RelationIndex`/`RelationQuery`/`RelationReply` を workspace
read model（`relation_ui.rs`）と `synvelis-app` の Inspector
表示へ投影し、`Parameter → AutomationLane / ModulationSource /
Assignment → 影響先 Parameter` の双方向探索と anchor
nav-stack による roundtrip を headless fixture で実装した。

## Context

- PC-C2 の Relation Index は project 層の正本として完成済みだが
  製品 UI へ届く投影がなかった。UI-C5-01 は Issue #102 の他
  slice（実 GUI・IME・multi-window・Signal Flow 製品化）を
  待たずに headless で検証できる最小の双方向探索 slice であり、
  後続 slice が消費する workspace read model の形をここで固定
  する（spec Why Now）。
- 前提となる既存構造: `arrangement_ui.rs` が「session を読んで
  表示 DTO を返す」read-model の先例、`MainScreenState` +
  `publish_project_length` refresh + `screens.rs` Table mount が
  確立済み。`ui_ops::capture_screen_state` が subscribed param
  の `inspector.target`（tagged `param-<32hex>`）を毎 pump で
  `MainScreenState.inspector` に書く（design Context）。
- 上流文書: spec `docs/specs/pc-c5-01-relation-inspector.md`、
  ADR `docs/adr/pc-c5-01-relation-readmodel-coverage.md`、
  design `docs/designs/pc-c5-01-relation-inspector-design.md`、
  plan `docs/plans/pc-c5-01-relation-inspector.md`、task
  `docs/tasks/pc-c5-01-relation-inspector.md`、test-spec
  `docs/test-specs/pc-c5-01-relation-inspector.md`。

## Implementation

### Changes

- `crates/synvelis-workspace/src/relation_ui.rs`（新規）: `relation_rows`/`relation_rows_with` が `ProjectSession` の live project を `session.revision()` で `RelationIndex::build` し、有界 `RelationQuery`（`RelationDirection`・`max_depth`・`max_edges`・`include_disabled`・`expected_revision`）の `RelationReply` を `RelationView`/`RelationRow` に投影。`RelationRow` は source/target/kind/state/assignment を verbatim 保持し、`neighbor(anchor)` で anchor 接触辺の反対側 entity を返す。`RelationNav` は anchor の LIFO stack で descend/pop roundtrip を提供。`RelationView::is_stale` で publish 後の revision ドリフトを検出可能。
- `crates/synvelis-workspace/src/lib.rs`: `pub mod relation_ui;` + additive re-export のみ。
- `crates/synvelis-workspace/tests/rel_c5.rs`（新規）: UI-C5-01 の双方向探索・roundtrip・coverage/state 誠実表面化を 11 fixture test で検証。
- `crates/synvelis-app/src/screens.rs`: `RelationScreenState`（anchor/coverage/rows）を `MainScreenState.relations` に追加し、Inspector tab に `relation_label`（anchor + verbatim coverage）と `relation_table`（Kind/Source/Target/Assignment/State、`selection: None` の read-only Table）を追加。
- `crates/synvelis-app/src/app.rs`: `relation_rows()` accessor と `publish_project_length` refresh を追加。anchor 解決（購読 param target の tagged id → `ParameterTargetId::from_tagged` → `ControlEntityRef::Parameter`）は `relation_view` helper に集約し、両経路が同一ロジックを共有。`publish_project_length` 周辺の additive 変更のみ。
- 変更なし: `crates/synvelis-project/`・`crates/synvelis-recording/`・`service.rs`/`ui_ops.rs`/`shell.rs`/`host.rs`・close-veto 領域（`synvelis-project` は read-only 接続のみ）。

### Decisions During Implementation

- 既定 query は depth-1 `RelationDirection::Both` — 全 row が anchor に接するため `neighbor` → 次 anchor の写像が一意。多段探索は `RelationNav` で depth-1 query を合成し、anchor 非接触の深い row は `neighbor() = None` で display-only と正直に扱う。
- `include_disabled` 既定 `true` — Disabled 辺を `Disabled` state の row として表面化し、「0 件表示」への縮退を防ぐ。`expected_revision` pin で `Stale`、未知 anchor で `Unavailable`、`max_edges` 超過で `Truncated` を index の semantics のまま返す。
- `ControlSession::signal_flow_view`/`lane_view` は置換せず併存（置換判断は設計文書に記録）。`synvelis-project` は read-only 接続のみで変更なし。domain 正本は `RelationIndex` 一箇所。

## Related Experiments

### Adopted

なし — `metadata.experiments.adopted` は空。

### Rejected

なし — `metadata.experiments.rejected` は空。

## Validation

canonical = mise、`git merge origin/main`（`de70404` 取り込み）
後 HEAD で実施。

| コマンド | exit status |
| --- | --- |
| `mise run fmt` / `mise run fmt-check` | 0 |
| `mise run check` | 0 |
| `mise run lint` | 0 |
| `mise run test` | 0（`cargo test --workspace --locked`、rel_c5 11 tests 含む全 pass） |
| `mise run build` | 0 |
| `mise run test-msrv` | 0（rust 1.89 toolchain で `cargo test --workspace --locked` 全 pass） |
| `mise run check-app-native` | 0 |
| `mise run lint-app-native` | 0 |

補助証拠: `cargo test -p synvelis-workspace --test rel_c5` 直接実行も 11/11 pass（canonical は上記 mise）。

## review-protocol 結果

- doc chain（spec→adr→design→plan→task→test-spec）と実装 diff を review-protocol でレビュー。CONFIRMED P0/P1 なし。
- レビューで確認した点: subscription target は `to_tagged()` 形式（`param-<32hex>`）で `from_tagged` が受理すること、`CounterIdGenerator` の単調 raw により fixture helper の `rfind`/`next_back` が newest を正しく取ること、`capture_screen_state` → `publish_project_length` の順序で `inspector.target` が新鮮であること、`publish`/`accessor` が `relation_view` helper を共有し anchor 解決が分岐しないこと。
- doc-status 指摘への対応: artifact id を `PREFIX-`+22 文字規約に修正、orphan-navigation 解消のため各 doc に Markdown link の Sources and Relations 節を追加。spec の `traceability-missing-upstream` は sv-gui-04-01 等と同型の advisory で残置。

## Risks

- `publish_project_length` は service op ごとに
  `RelationIndex::build` を再構築する（`arrangement_rows` と同じ
  refresh 点・同程度のコスト特性）。`RelationIndex::build` は
  query 毎 O(N) — publish 駆動の non-RT 経路のみで現状規模では
  問題ないが、大規模 project での query コスト最適化は後続
  slice の検討事項。
- `publish_project_length` の refresh は `inspector.target` が
  未捕捉の間は空の `RelationScreenState` を書く — 「anchor なし」
  であり relation 0 件の誤表示ではない（plan Risks）。
- `RelationScreenState` は anchor/coverage/rows のみ保持し
  `project_revision`/`direction` は画面に出さない（coverage が
  `Stale` を含む場合のみ表面化）。
- 影響は additive のみ — `RelationScreenState` 追加で
  `MainScreenState` の `PartialEq`/`Default` derive は維持
  され、app 側の変更は Inspector tab 内と publish の 1 行に
  限定（close-veto/shell/service/ui_ops/host、project crate に
  触れない）。rollback は本 slice の commit revert で完結
  （永続形式・public wire 契約の変更なし、design Risk/Rollback）。
- `docs/` 監査の legacy findings（rcd-01 の broken relation 等）
  は本 slice 以前から `main` に存在 — doc-driven-dev-graph
  router が当該 broken relation で hard-block されたため、本
  slice の文書チェーンは手動で規約に沿って作成した。

## Follow-ups

- **UI-C5-02..05**（実 GUI・IME・multi-window・Signal Flow 製品化）: **NotRun** — #114/#103 gate。
- **実 OS・実 GPUI での描画証拠**: **NotRun** — headless slice のみ。`check-app-native`/`lint-app-native` は native feature 経路の確認として実行（結果は PR 本文）。
- **`ControlSession::signal_flow_view`/`lane_view` の置換**: 本 slice では併存。置換判断は設計文書の記録のみ。
