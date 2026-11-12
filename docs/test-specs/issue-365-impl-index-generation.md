---
id: "TSPEC-034WsAjGC5rxjfAzdzF1XI"
type: "test-spec"
status: "approved"
title: "Issue #365 impl index 生成可能化 — index in-sync と audit 非増加の保証"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source: []
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
    - "docs/designs/issue-365-impl-index-generation-design.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-365-impl-index-generation.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #365 impl index 生成可能化 — index in-sync と audit 非増加の保証

## Purpose

`docs/impl/ir`・`docs/impl/exp` の管理 index が `doc-maintenance` の
owner sync 経路で再生成可能であることを保証する test spec。
front matter 是正や marker 追加が「形式を揃えるだけ」で意味を変え
ないこと、および是正作業自体が audit error を増やさないことを、
将来の index 運用変更（schema 更新・writer 変更）に対しても
保持すべき不変条件として記録する。index が再び `rejected` に
戻る、または audit error が増える変更が入った場合にこの spec を
再確認する。

## Feature

impl 系管理 index の生成可能性: `docs/impl/ir` と `docs/impl/exp`
の `README.md` が generated-index marker を持ち、全 source が
owner の schema を満たすとき、`index plan` は `rejected` を返さず、
`index apply` 後の `index plan` は `in-sync` を返す。

## Rules

- Rule: impl-ir index の source 妥当性 — `docs/impl/ir` 内の全
  `.md` 文書（README を除く）は impl-doc の
  `implementationRecordSchema` を満たす front matter を持つ。
  `type` は `"impl"`、`status` は定義済み enum、`created`/`updated`
  は `YYYY-MM-DD`、`relations.changes.*` の要素は object、
  `metadata.experiments` は `adopted`/`rejected` を持つ object。
- Rule: managed index の marker — `docs/impl/ir/README.md` と
  `docs/impl/exp/README.md` は generated-index marker を含み、
  index 内容は owner の `sync_impl_index` safe sync API が生成した
  ものと一致する。手書き部分（preamble 等）は marker の外に保持
  される。
- Rule: 是正の非破壊性 — front matter の形式修正は文書の意味
  （本文・ファイル名・id）を変えない。`type`/`status`/`updated`
  の値は本文記述と既存の正しい文書の例から保守的に判断する。
- Rule: audit 非増加 — 是正の前後で `audit_impl_record.js` と
  `audit_experiment_log.js` の error 数が増えない。

## Examples

- Example: schema 違反の修正 — Given `updated` が
  `"2026-11-07 (N1 修復追補…)"` の文書, When front matter を
  `YYYY-MM-DD` 形式に修正する, Then `index plan` の source
  validation がその文書を違反として報告しない。
- Example: marker 追加 — Given 手書きの `docs/impl/exp/README.md`,
  When generated-index marker を追加して `index apply` を実行する,
  Then README は手書き部分を保持したまま generated table を持ち、
  再 plan が `in-sync` を返す。
- Example: audit 会計 — Given schema 違反文書の修正で
  `invalid-front-matter` error が解消し `missing-section` error が
  新規計上される可能性がある, When audit を再実行する, Then
  error 合計がベースライン（ir 331 / exp 52）を超えない。

## Guarantees

- 正しい実装は、`index plan --dir docs/impl/ir` と
  `--dir docs/impl/exp` が `rejected` を返さないことを保証する。
- 正しい実装は、`index apply`（引数なし）が exit 0 で終了することを
  保証する。
- 正しい実装は、ir/exp の既存文書について本文・ファイル名・id の
  不変性を保証する（front matter の形式のみ変更可）。
- 正しい実装は、audit error 数の非増加を保証する。schema 修正で
  section 検査が新たに走る文書があるため、error の増減は内訳の
  変化を伴いうるが、合計では増えてはならない。

## Non-goals

- 文書本文の構成（必須 `##` セクションの有無）をこの test spec で
  保証しない。`missing-section` は別途の文書品質課題であり、本
  spec の対象は index 生成可能性と是正の非破壊性に限る。
- `docs/impl/exp/*.jsonl` の event schema 適合は対象外
  （既存 error の修復は別 Issue）。
- index の表示内容（行の並び・tier 分け）の美的妥当性は対象外。
  deterministic な生成規則への一致のみを保証する。

## Risk

- この保証が崩れると、impl 系 index が再び手動管理に逆戻りし、
  文書追加・status 変更のたびに drift が蓄積する。また是正作業が
  文書の意味を変えてしまった場合、過去の実装記録の信頼性が損なわれ、
  audit error の増加は品質ゲートの有効性を弱める。
