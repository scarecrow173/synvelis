---
id: "TSPEC-oNOHOT3cXJQKjlUyodpyBj"
type: "test-spec"
status: "approved"
title: "Issue #514 test-spec — 判定記録 slice の検証仕様"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/designs/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/designs/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-514-session-engine-selection.md"
    - "evidence/sv514/verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-514-session-engine-selection.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #514 test-spec

## Feature: 判定記録 slice の検証

本 slice は「実装不要」判定の記録が deliverable であり、rebuild
経路の挙動を変更しない。したがって新規の behavior test は追加
しない。検証は (a) 既存 suite の非回帰、(b) 判定記録の完全性、
(c) lane の正直な Pass/NotRun 簿記で構成する。

## Rule TS-1: 既存 suite 非回帰

`mise run test synvelis-app` と `mise run test synvelis-workspace`
が pass し、特に `rcp_selection_integration`（workspace）と
`sf03_session_engine`（app）が回帰しないこと。`engine.rs` の変更は
module doc comment のみであり、挙動 pin の対象外。

## Rule TS-2: 判定記録の完全性

- `docs/adr/issue-514-session-engine-selection.md` に「実装不要」
  判定と root-cause 根拠が記録されていること。
- `crates/synvelis-app/src/engine.rs` module doc に selection 非消費が
  意図的である旨が明記されていること。
- `evidence/sv514/decision-ledger.md` に判定材料 (a)–(d)・AC4 の
  帰属・lane Pass/NotRun 簿記があること。

## Rule TS-3: lane の正直記録

- headless/fixture lane の Pass は当該 lane が実際に exercise した
  挙動のみに適用する。
- T3/T4 実 device/plugin・実 OS・RT/PCM 実測は NotRun として記録し、
  headless Pass を実機 Pass に昇格しない。gate: #109・#296・
  `scarecrow173/ruviform#630`・`scarecrow173/ruviform#631`。

## Examples

- Example 1: `mise run test synvelis-workspace` 実行 →
  `rcp_selection_integration` を含む全 test pass。
- Example 2: `mise run test synvelis-app` 実行 → `sf03_session_engine`
  を含む全 test pass。
- Example 3: `git diff` で `engine.rs` の変更が module doc comment
  のみであることを確認。
