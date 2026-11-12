---
id: "TSPEC-034Wv24lt11AFRZnEjnYfR"
type: "test-spec"
status: "approved"
title: "Issue #368 docs/impl/ir missing-section 本文補完 — audit 0 件と非増加の保証"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/368"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-368-ir-missing-sections-design.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-368-ir-missing-sections.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #368 docs/impl/ir missing-section 本文補完 — audit 0 件と非増加の保証

## Purpose

`docs/impl/ir/` の Implementation Record が impl-doc の必須
`##` セクション構造を満たし、audit の `missing-section` error が
0 件であることを保証する test spec。本文補完が「形だけの見出し
追加」に留まらず、文書固有の実装・検証事実を保持・補完する
ものであること、および補完作業自体が他の audit finding を
増やさないことを、将来の文書追加・編集に対しても保持すべき
不変条件として記録する。

## Feature

ir 文書の必須セクション完備性: `docs/impl/ir/` 内の全 ir 文書は
Summary / Context / Implementation / Related Experiments /
Validation / Risks / Follow-ups の 7 つの `##` セクションを持ち、
各セクションは対象文書の実装・検証内容に基づく具体的な本文を
持つ。

## Rules

- Rule: 必須セクションの存在 — 全 ir 文書は 7 つの正規 `##`
  セクション名を持つ。audit の heading 判定（`##` レベル・
  大文字小文字非区別・フェンス内除外）に一致すること。
- Rule: 内容の具体性 — 各セクションは対象文書固有の事実を記述
  する。空見出し・TODO のみ・他文書からの無関係な転記は不可。
  根拠は対象文書の本文・relations が指す上流文書・
  `metadata.experiments` に限る。
- Rule: 非破壊性 — front matter（id / status / relations /
  metadata）とファイル名は不変。既存本文の事実記述は保持し、
  正規名以外の見出しは内容を保ったまま正規セクションへ
  再配置・改題する。
- Rule: audit 非増加 — 補完の前後で `missing-section` 以外の
  finding code の件数が増えない（baseline: missing-experiment-link
  18 / broken-relation-link 36）。

## Examples

- Example: 非正規見出しの改題 — Given `## Verification` と
  `## Remaining` を持つ文書, When `## Validation` と
  `## Follow-ups`（必要に応じ `## Risks`）へ内容を保持して
  改題・再配置する, Then audit が当該文書の missing-section を
  報告しない。
- Example: 実験未使用の記述 — Given `metadata.experiments` が
  空で `docs/impl/exp/` に関連ログが無い文書, When Related
  Experiments を補完する, Then 実験を用いていない旨の事実記述と
  なり、存在しないログへの参照を作らない。
- Example: audit 会計 — Given 60 文書の補完, When audit を
  再実行する, Then missing-section が 0 件、他 code が
  baseline を超えない。

## Guarantees

- 正しい実装は、`audit_impl_record.js --json` の `missing-section`
  finding が 0 件であることを保証する。
- 正しい実装は、ir 文書 60 件の front matter・ファイル名が
  変更前と一致することを保証する（本文のみ変更）。
- 正しい実装は、`missing-experiment-link`・`broken-relation-link`
  の件数が baseline（18 / 36）を超えないことを保証する。
- 正しい実装は、`index plan --dir docs/impl/ir` が `in-sync` を
  返すことを保証する（drift 時は owner sync で再生成）。

## Non-goals

- `missing-experiment-link` / `broken-relation-link` warning の
  解消は対象外（front matter の修正を伴う別タスク）。
- ir 文書の実装内容の正しさの再評価は対象外（実態に基づく
  補完のみ）。
- `docs/impl/exp/` や他 docs dir の文書構造は対象外。
- audit script 自体の heading 判定ルールの変更は対象外。

## Risk

- この保証が崩れると、ir 文書の構造が再び不揃いになり audit の
  error が常態化して品質ゲートとして機能しなくなる。また本文
  補完が事実を改変した場合、過去の実装記録の信頼性が損なわれ、
  IR を根拠にした後続作業（追跡・監査・復元）が誤った前提で
  進む。
