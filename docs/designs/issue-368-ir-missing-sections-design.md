---
id: "DESIGN-034WuZE8vEiOZJKbZMaRJQ"
type: "design"
status: "approved"
title: "Issue #368 docs/impl/ir missing-section 本文補完 — 設計"
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
  implements:
    - "docs/specs/issue-368-ir-missing-sections.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-368-ir-missing-sections.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #368 docs/impl/ir missing-section 本文補完 — 設計

## Context

`audit_impl_record.js` は impl-doc の `implementationRecordSchema`
を通過した各 ir 文書に対し、必須 `##` セクション 7 件
（Summary / Context / Implementation / Related Experiments /
Validation / Risks / Follow-ups）の存在を検査する。見出し判定は
`^##\s+<name>\s*$`（大文字小文字非区別、フェンス内は除外）で、
`###` 以下の小見出しはセクションとして数えない。

Issue #365（PR #366）で 44 件の front matter が schema 準拠に
なった結果、それまで section 検査に到達しなかった文書の
`missing-section` が可視化され、2026-11-13 時点で 60 文書・
finding 計 319 件が残っている。欠落の内訳は 2 パターンある。

- 正規セクション名以外の見出しに同等内容がある文書
  （`## 実装内容`・`## 検証`・`## 残件`、`## Verification`・
  `## Remaining` 等）。内容は存在するが見出しが canonical でない。
- 正規セクション自体が無く、要約的な本文のみ、または一部
  セクションのみ存在する文書。

## Scope

### In Scope

- `docs/impl/ir/` の missing-section 保持 60 文書の本文補完
- 既存本文の正規セクションへの再配置・改題（内容保持が前提）
- 補完に必要な上流文書（spec/design/plan/task/test-spec）の
  参照による事実確認
- 本タスクの doc-chain と Implementation Record の作成
- docs index の `index apply` 再生成（生成物）

### Out of Scope

- front matter・ファイル名の変更
- `docs/impl/exp/`・他 docs dir の既存文書本文の変更
- 製品コード・manifest・lockfile の変更
- `missing-experiment-link` / `broken-relation-link` warning の解消
- ir 文書の実装内容自体の再評価・訂正

## Components and Boundaries

- 文書補完（本 repo の docs のみ）: 各 ir 文書の body を、
  audit が要求する 7 つの `##` セクション構造に揃える。
  front matter は byte 単位で不変とし、ファイル名も変更しない。
- 事実の源泉: 対象文書自身の本文、front matter の relations
  （`implements` / `depends-on` / `related` / `verified-by` /
  `references`）が指す上流文書、`relations.defers`、および
  `metadata.experiments`。上流文書が実装・検証の一次記録であり、
  ir 本文はそれを要約・記録したものなので、補完は上流文書の
  記述を根拠に行う。
- index 再生成: `doc-maintenance` の `index apply` が owner の
  safe sync API に dispatch する経路のみを使い、README を手編集
  しない。

## Data and Control Flow

1. `audit_impl_record.js --json` で missing-section 保持文書と
   不足セクション名の一覧を採取する（本タスクでは 60 文書）。
2. 各文書について本文を読み、既存内容を 7 セクションへ写像する。
   - `## 実装内容` / `## Implementation` / `## Changes` 系 →
     `## Implementation`（`### Changes`・`### Decisions During
     Implementation` 等の小見出し構造は保持してよい）
   - `## 検証` / `## Verification` / `## 検証結果` → `## Validation`
   - `## 残件` / `## Remaining` / `## Follow-ups` → `## Follow-ups`
     （残リスク性の記述は `## Risks` へ振り分け）
   - `## Summary` 相当が無い場合は front matter の title・
     relations・本文冒頭から要約を書く
   - `## Context` 相当が無い場合は source Issue・上流文書・
     前提となる依存を relations から書く
   - `## Related Experiments` は `metadata.experiments` と
     `docs/impl/exp/` の実在ログを確認して記述する。実験を
     使っていない文書はその旨を事実として記す
3. 補完後に audit を再実行し、missing-section が 0 件であること、
   他 code の件数がベースライン（missing-experiment-link 18 /
   broken-relation-link 36）を超えないことを確認する。
4. `index plan --dir docs/impl/ir` が `in-sync` であることを確認し、
   drift があれば `index apply` で再生成する。

## Non-Functional Constraints

- front matter（`---` で囲まれた領域）とファイル名は全 60 文書で
  不変。本文のみを変更する。
- 追加する本文は対象文書の実装・検証の実態に基づく。推測・
  捏造・他文書からの無関係な転記をしない。根拠が文書内に無い
  場合は上流文書（spec/design/plan/task/test-spec）を参照して
  事実を確認してから書く。
- 既存本文の情報を失わない。改題・再配置はよいが、事実記述の
  削除はしない（重複の整理は許容）。
- audit の heading 判定は `##` レベルの完全一致（大小文字非区別）
  であるため、正規名と異なる見出しを残したままにしない。

## Risks and Trade-offs

- 補完作業で既存本文の事実を誤って改変するリスク → 各文書の
  変更は本文セクションの再配置・追記に限定し、front matter と
  既存の事実記述を保持する。変更は文書単位で review 可能な
  diff として残す。
- 「定型文で埋める」リスク → 受入条件が具体的本文を要求する
  ため、各セクションは対象文書固有の実装・検証事実を書く。
  実験未使用の文書の Related Experiments のように「なし」と
  事実記述する場合も、その文書の metadata.experiments が空で
  あることを確認した上で行う。
- 大量文書の並列編集で表記揺れが生じるリスク → 必須 7
  セクション名と標準的な小見出し（`### Changes` /
  `### Decisions During Implementation`）を設計に明記し、
  全文書で同じ語彙を使う。

## Deferred Design Concerns

- `missing-experiment-link` warning 18 件の解消 | front matter の
  修正を伴い本タスクの「front matter 不変」制約と衝突する |
  別 Issue で exp 参照の棚卸しを行うとき | warning が残る |
- `broken-relation-link` warning 36 件の解消 | relations の
  指す文書の実在性判断が必要で front matter 変更を伴う |
  別 Issue で relations の棚卸しを行うとき | 存在しない文書への
  参照が残る |

## Verification Notes

- [ ] `node .agents/skills/impl-doc/scripts/audit_impl_record.js
  --json` の `missing-section` finding が 0 件
- [ ] 同 audit で `missing-experiment-link` ≤ 18・
  `broken-relation-link` ≤ 36（error code の件数が増えない）
- [ ] 60 文書全件で front matter・ファイル名が不変
  （`git diff` で `---` 以降の body のみ変更を確認）
- [ ] `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index plan --dir docs/impl/ir` が `in-sync`
