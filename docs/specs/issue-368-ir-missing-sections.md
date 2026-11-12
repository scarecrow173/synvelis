---
id: "SPEC-034WuZoDEfeT3L23cjJAFD"
type: "spec"
status: "implemented"
title: "Issue #368 docs/impl/ir missing-section 本文補完"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
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
    - "https://github.com/scarecrow173/synvelis/issues/368"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #368 docs/impl/ir missing-section 本文補完

## What Should Be Built

`docs/impl/ir/` 配下の Implementation Record について、
`audit_impl_record.js` が要求する必須 `##` セクション
（Summary / Context / Implementation / Related Experiments /
Validation / Risks / Follow-ups）がすべて存在し、各セクションに
対象文書の実装・検証内容に基づく具体的な本文がある状態にする。

既存本文が正規セクション名以外の見出し（`## 実装内容`・
`## 検証`・`## 残件`・`## Verification`・`## Remaining` 等）や
`###` 小見出しの下に存在する場合は、内容を保持したまま正規
セクションへ再配置・改題する。front matter（id / status /
relations を含む）とファイル名は変更しない。

## Why Now

Issue #365（PR #366）で ir 文書の front matter を impl-doc schema
準拠に正規化した結果、schema 検証を通過した文書に対して本文
セクション検査が新たに走り、`missing-section` error が 319 件
計上された（ir の `issue-365-...` Implementation Record の
Follow-ups に別タスク化として記録済み）。`missing-section` は
audit の error severity であり、impl 記録の必須構造が欠けた
ままでは audit が品質ゲートとして機能しない。本 Issue #368 が
そのフォローアップタスクである。

## Users and Value

- docs/impl/ir を監査・参照する開発者・エージェント:
  `audit_impl_record.js --json` の `missing-section` が 0 件になり、
  error が `missing-experiment-link` / `broken-relation-link`
  （いずれも warning）のみとなる。
- IR を読む利用者: どの文書でも Summary→Follow-ups の共通構造で
  実装内容・検証・残リスクを追える。

## Scope

### In Scope

- `docs/impl/ir/*.md` の本文への必須 `##` セクション補完
  （2026-11-13 実測で missing-section を持つ 60 文書）
- 既存本文の正規セクションへの再配置・改題（内容の保持が前提）
- `docs/impl/ir/README.md` の drift が生じた場合の
  `doc-maintenance` / `sync_impl_index` 経由の再生成
- 本タスクの doc-chain（spec/design/plan/task/test-spec）と
  Implementation Record の作成
- 新規 doc-chain 文書を含む各 docs dir の管理 index の
  `index apply` 再生成（生成物）

### Out of Scope

- 各 ir 文書の front matter（id / status / relations /
  metadata）とファイル名の変更
- `docs/impl/ir` 以外の既存文書本文の変更
- `docs/impl/exp/` 配下の変更
- 製品コード・Cargo manifest・lockfile・mise 設定の変更
- `missing-experiment-link`（18 件）・`broken-relation-link`
  （36 件）の解消（warning であり front matter 由来のため本タスクの
  範囲外。件数を増やさないことのみを条件とする）
- ir 文書の内容そのものの再評価・書き換え（実態に基づく補完のみ）

## Requirements

- `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`
  の `missing-section` finding が 0 件であること。
- 追加・再配置した全セクションに、対象文書の実装・検証内容に
  基づく具体的な本文があること（空見出し・TODO のみ・定型文の
  羅列でない）。
- 各文書の front matter とファイル名が変更前と一致すること。
- `missing-section` 以外の error code の件数が現状値
  （missing-experiment-link 18 / broken-relation-link 36）を
  超えないこと。
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan --dir docs/impl/ir` が `in-sync` であること
  （本文のみの変更のため。drift が出た場合は `index apply` で
  再生成して同じ PR に含める）。

## Acceptance Criteria

- [ ] `audit_impl_record.js --json` の `missing-section` が 0 件
- [ ] 追加した全セクションに実装・検証内容に基づく具体的な本文がある
- [ ] 各文書の front matter（id / status / relations を含む）と
  ファイル名が変わっていない
- [ ] `missing-section` 以外の error code の件数が現状値
  （missing-experiment-link 18 / broken-relation-link 36）を超えない
- [ ] `doc_maintenance.js index plan --dir docs/impl/ir` が `in-sync`

## Deferred Design Concerns

- `missing-experiment-link` warning 18 件の解消 | front matter の
  `metadata.experiments` が指す exp ログ実体の欠落・参照ずれで、
  本文補完とは別の修復判断が必要 | 別 Issue で exp 参照の棚卸しを
  行うとき | warning が残り続け error 監視のノイズになる |
- `broken-relation-link` warning 36 件の解消 | relations の指す
  文書が未作成・改名済みで、リンク先の実在性の判断が必要 |
  本タスクでは front matter を変更しないため対象外 | 別 Issue で
  relations の棚卸しを行うとき | 存在しない文書への参照が残る |

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/368>
- 前提: PR scarecrow173/synvelis#366（Issue #365、front matter
  schema 適合化・ir index 生成可能化）統合済み。main `7bd5f959`
  以降が基準。
- 実測値: 2026-11-13 に `audit_impl_record.js --json` を実行し、
  missing-section を持つ文書 60 件・finding 計 319 件、
  他 code は missing-experiment-link 18 / broken-relation-link 36
  を確認済み。
- 関連: `docs/impl/ir/issue-365-impl-index-ir-front-matter-marker-index.md`
  （本タスクを Follow-ups に記録した先行 IR）。
