---
id: "SPEC-034Wsudf9ocUWKIH6GUhA9"
type: "spec"
status: "implemented"
title: "Issue #365 docs/impl ir・exp 管理 index を doc-maintenance で生成可能にする"
created: "2026-11-12"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/365"
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
    - "https://github.com/scarecrow173/synvelis/issues/365"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #365 docs/impl ir・exp 管理 index を doc-maintenance で生成可能にする

## What Should Be Built

`docs/impl/ir` と `docs/impl/exp` の管理 index（`README.md`）が
`doc-maintenance` の `index plan` / `index apply` で生成・再生成できる状態にする。
具体的には、index の source 検証を通らない既存 ir 文書の front matter を
schema 準拠の形式に修正し（意味は変えない）、`docs/impl/exp/README.md` に
generated-index marker を追加して owner の `sync_impl_index` 経路で
再生成可能にする。

## Why Now

現状（2026-11-13 実測）では `index plan --dir docs/impl/ir` が
`rejected`（index sources failed validation：ir 文書 147 件中 44 件の
front matter が schema 違反）、`index plan --dir docs/impl/exp` が
`rejected`（hand-curated or ambiguous index: missing generated-index
marker）を返す。引数なしの `index apply` はこの 2 dir が `blocked` となり
exit 1 になるため、他の docs dir と同様に文書追加・status 変更のたびに
管理 index をスクリプトで再生成できない。文書を追加・status 変更する
運用では index の手動追記が必要で、drift が蓄積しやすい。

## Users and Value

- Synvelis リポジトリで docs を保守する開発者・エージェント:
  `doc-maintenance index apply` 一発で impl 系 index を含む全 dir の
  index を再生成でき、手動追記と drift による手戻りがなくなる。
- `doc-status` / 監査系ツールの利用者: index が managed format に
  揃い、`index-*` 系 finding のノイズが減る。

## Scope

### In Scope

- `docs/impl/ir/` 配下の既存文書の front matter の形式修正
  （`metadata` / `relations.changes` の構造、`type` / `status` /
  `updated` の値の schema 準拠化。本文・ファイル名・id は変更しない）
- `docs/impl/ir/README.md` と `docs/impl/exp/README.md` への
  generated-index marker 追加と owner sync 経路での再生成
- `index plan` / `index apply` が `docs/impl/ir`・`docs/impl/exp` で
  `in-sync` に到達することの確認
- 本タスクの doc-chain（spec/design/plan/task/test-spec）と
  Implementation Record の作成

### Out of Scope

- 既存文書の本文・ファイル名・id の変更、`migrate_ids` 相当の振り直し
- `docs/impl/exp/*.jsonl` の event 内容修正（既存の audit error は
  本タスクでは増やさないことのみを条件とする）
- `docs/impl/` 以外の docs dir の内容変更（新規文書追加に伴う管理
  index の再生成は生成物として許容）
- 製品コード・`Cargo.toml`・`Cargo.lock`・`mise.toml` 等の変更
- 他リポジトリ（ruviform・audio-lens）での同種対応

## Requirements

- ir 文書の front matter が impl-doc の `implementationRecordSchema` を
  満たす（`type: "impl"`、status enum、`YYYY-MM-DD` の created/updated、
  `relations.changes.*` は object 要素、`metadata.experiments` は
  `adopted`/`rejected` を持つ object）。
- 是正は意味を変えない形式修正に限る。値の判断が必要な項目
  （`type` / `status` / `updated`）は本文記述と既存の正しい文書の例から
  保守的に判断し、PR 本文に判断を明記する。
- `docs/impl/exp/README.md` は手書き部分を保持したまま generated-index
  marker を追加し、`sync_impl_index`（`doc-maintenance index apply`
  経由）で再生成する。`promote_index` は byte 一致した旧 scaffold 出力
  のみ対象で force 経路がないため使用しない。
- `audit_impl_record.js` / `audit_experiment_log.js` の error 数が
  変更前ベースラインから増えない。

## Acceptance Criteria

- [ ] `doc_maintenance.js index plan --dir docs/impl/ir` が `rejected`
  でなくなり、`index apply --dir docs/impl/ir` 後に `index plan` が
  `in-sync` を返す。
- [ ] `docs/impl/exp` も同様に `index apply` 後 `in-sync`。
- [ ] 引数なしの `doc_maintenance.js index apply` が exit 0。
- [ ] ir / exp の既存文書の本文・ファイル名・id が変わっていない
  （front matter の形式修正のみ）。
- [ ] `audit_impl_record.js` / `audit_experiment_log.js` の error が
  ベースラインから増えていない。

## Deferred Design Concerns

- `docs/impl/exp/*.jsonl` の `invalid-event-shape` error（issue-147
  sccache.jsonl 52 件）の修復 | 過去ログの event 形式が現行 schema
  と非互換で、内容の再解釈が必要。対象外とした | 別 Issue で exp
  event schema の互換方針を決めたとき | audit error が残り続け、
  将来の error 増加検知のノイズになる |

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/365>
- 実測値は 2026-11-13 に `doc_maintenance.js index plan` で再確認済み
  （ir: sources validation 44 件 rejected、exp: marker 欠落 rejected）。
- 関連: scarecrow173/apm-packages#208（doc-maintenance の index apply
  注意点明記。先行不要）。
