---
id: "TSPEC-034VpCxHsdsOcOqeEN6wGS"
type: "test-spec"
status: "approved"
title: "VCS-06 UI headless matrix tests"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
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
    - "docs/specs/vcs-06-ui-headless-matrix.md"
    - "docs/designs/vcs-06-ui-headless-matrix-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-06-ui-headless-matrix.md"
    - "docs/designs/vcs-06-ui-headless-matrix-design.md"
  derived-by: []
  verifies:
    - "docs/specs/vcs-06-ui-headless-matrix.md"
    - "docs/designs/vcs-06-ui-headless-matrix-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 UI headless matrix tests

## Purpose

`crates/synvelis-app/tests/vcs06_ui_matrix.rs` が固定する
`syn.vcs.*` op surface の欠損行契約を記録する。実機 GPUI/OS lane
（#114）での再検証が来ても、headless で担保した行と残る
`NotRun` 行の境界を失わないための仕様。

## Feature

VCS History headless fixture — Unknown/Stale/Partial honesty

## Rules

- **Rule VCS-UI-UNKNOWN** 異 `project_id` の committed 端点同士の
  pin は `selection.diff.overall == "unknown"`・
  `completeness == "Partial"`・`summary.unresolved > 0`・
  `changes == []`・issues に `ambiguous-identity` を含み、
  `equal` / 0 changes に潰れない。
- **Rule VCS-UI-DEGRADED** asset bytes が `Absent` の committed
  snapshot は degraded として `completeness == "Partial"`、
  `missing-asset` と `unresolved-source` の両 issue kind を出し、
  `history_screen_spec` の `vcs.issues` table（read-only）の行数が
  `summary.unresolved` と一致する。同一 project 内容でも
  equality は fail closed。
- **Rule VCS-UI-FRESHNESS** staged pin は `index_generation` probe
  失敗時に `freshness == "unknown"` + `freshness_reason` を返し
  committed pin は影響を受けない。backend 不在時も同じ
  `EndpointFreshness::Unknown` 経路で `unknown` + reason を返す。

## Examples

- `cross_project_committed_endpoints_report_unknown_not_equal` —
  Given committed HEAD（project_id=0）と foreign project_id の
  committed 端点、When 両側 pin、Then `overall=="unknown"`・
  issues 非空・`changes==[]`。
- `absent_asset_bytes_mark_degraded_partial_with_issue_rows` —
  Given asset 保持だが blob read が常に `Absent` の committed
  端点と live pin、When pin、Then `Partial` + 両 issue kind +
  screen-spec `vcs.issues` 行数 == unresolved。
- `staged_pin_freshness_is_unknown_when_index_probe_fails` —
  Given staged+committed pin、When pin 解決後に `index_generation`
  probe を失敗化、Then staged は `unknown`+reason、committed は
  `fresh` のまま。
- `staged_pin_freshness_is_unknown_without_backend` — Given 同上
  pin、When `VcsSession{backend: None}` で `vcs_history_view` を
  投影、Then staged は `EndpointFreshness::Unknown`（"no VCS
  backend"）・committed は `Fresh`。

## Guarantees

- 全 assert は wire JSON / `VcsEndpointView` の外部観測値に対する
  独立 oracle であり、fixture 内部の導出値をそのまま再主張しない。
- `Unknown`/`Partial`/`Stale` が `Equal`/zero-change/`fresh` に
  潰れないことを pin 単位で固定する。

## Non-goals

- A/B pin・merge parent・live stale・dock/window 状態保持・
  lifecycle・read-only 不変条件（`vcs04_history.rs` app/workspace
  両層が既に担保。ledger は参照記録）。
- 実 GPUI レンダリング・IME・keyboard・a11y・実 OS window
  （#114、`NotRun`）。
- VCS Writer / 3-way merge・非 git provider・`*-REAL` 行。

## Risk

これらの行が無いと、UI 層が degraded/unknown データを
「変更なし」と誤表示する回帰を headless で検出できない。
