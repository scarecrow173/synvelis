---
id: "SPEC-539aArrngmntAdptr02"
type: "spec"
status: "approved"
title: "Issue #539 spec — VariantPorts::standard への ArrangementAdapter 登録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/539"
    - "docs/discovery/issue-539-arrangement-adapter.md"
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
    - "docs/discovery/issue-539-arrangement-adapter.md"
    - "docs/adr/issue-539-arrangement-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-539-arrangement-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-539-arrangement-adapter.md"
  derived-by:
    - "docs/designs/issue-539-arrangement-adapter.md"
    - "docs/plans/issue-539-arrangement-adapter.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #539 spec — VariantPorts::standard への ArrangementAdapter 登録

## 目的

gateway の standard variant wiring で arrangement scope の
dispatch を有効化し、CB-E2E-03 の gateway 経路
（fork→candidate composite edit→adopt→CAS/undo）が
headless で完走することを保証する。

## 受入条件（Issue #539 転記）

- [ ] `VariantPorts::standard` の adapters map に
      `ArrangementAdapter` が登録され、arrangement scope の
      gateway dispatch が経路として選択される
- [ ] `cb11_e2e_07_arrangement_scope_fails_closed_until_gateway_wiring`
      （または新規 test）で CB-E2E-03 の gateway 経路
      composite adopt が headless で Pass する
- [ ] unsupported 以外の entity kind・fail-closed 境界は
      現行どおり維持
- [ ] CB-07 の adopt CAS/undo 原子性契約を gateway 経路でも
      維持（中間状態が観測されないこと）
- [ ] 回帰なし: `mise run check synvelis-gateway` /
      `lint synvelis-gateway` / `test synvelis-gateway` /
      `check-headless-deps` が green

## Non-goals

- `ArrangementAdapter` 本体（`synvelis-project` 側）の変更
- host 側（synvelis-cli・mcp-poc・synvelis-app）の変更
- `GateDeps.render`（sv#518 seam）への変更
- lease 外テストファイルの編集
- 実 plugin・実 GUI・実機/RT 検証（外部 gate）
- MSRV lane
- docs index の編集・再生成（Coordinator が統合時に実施）

## 制約

- PR 本文・コミットメッセージは `Refs #539` のみ。
  `Closes`/`Fixes`/`Resolves` 系は使わない。
- 新規依存・feature 追加なし。
- `adapter_for` の fail-closed 機構は維持する
  （未配線 kind が将来追加されても `Unsupported` で
  安全側に倒れる）。
