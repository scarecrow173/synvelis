---
id: "DESIGN-034WsvXEZuKq8V4PoYjVxZ"
type: "design"
status: "approved"
title: "Issue #365 impl index 生成可能化 — front matter 是正と marker 追加の設計"
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
    - "docs/specs/issue-365-impl-index-generation.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #365 impl index 生成可能化 — front matter 是正と marker 追加の設計

## Context

`doc-maintenance` の `index` コマンドは、owner ごとの managed index
writer に dispatch して index を再生成する。`docs/impl/ir` と
`docs/impl/exp` は impl-doc の `sync_impl_index` safe sync API が
owner であり、index の source（ir は各文書の front matter、exp は
`.jsonl` ログ）を schema 検証してから plan を立てる。

2026-11-13 時点の実測では:

- `docs/impl/ir`: 147 件中 44 件の front matter が
  `implementationRecordSchema`（impl-doc の zod schema）に違反し、
  `index plan` が `rejected`（index sources failed validation）を返す。
  違反の内訳は `metadata` 欠落 31 件、`relations.changes.*` の文字列
  要素 14 件、`metadata.experiments` の配列形式 5 件、`type` 値
  `"ir"` 6 件、`status` 値 `"done"`/`"final"` 4 件、`updated` の
  日付形式外 1 件・欠落 3 件（複数違反を持つ文書があるため延べ件数）。
- `docs/impl/exp`: `README.md` に generated-index marker
  （`<!-- doc-suite:generated-index -->`）がなく、hand-curated /
  ambiguous index として `rejected`。
- `docs/impl/ir/README.md` も marker を持たない旧来の手書き表だが、
  source validation が先に失敗するため現状は sources 起因の
  `rejected` が表面化している。source を修正した後も marker が
  なければ同様に rejected になる見込みのため、両 README に marker を
  追加する。

なお `audit_impl_record.js` は schema 検証を通った文書にのみ必須
`##` セクション（Summary / Context / Implementation / Related
Experiments / Validation / Risks / Follow-ups）の検査を行う。
schema 違反の文書は section 検査に到達しないため、front matter を
修正すると未検出だった `missing-section` finding が新たに計上
される可能性がある。error 数が増えないことが受入条件のため、
是正後に audit を再実測して確認する。

## Scope

### In Scope

- `docs/impl/ir/` 44 件の front matter 形式修正
- `docs/impl/ir/README.md`・`docs/impl/exp/README.md` への
  generated-index marker 追加
- `sync_impl_index`（`doc_maintenance.js index apply` 経由）による
  両 index の再生成と `in-sync` 確認
- 是正に伴う audit error 数の前後比較

### Out of Scope

- 文書本文・ファイル名・id の変更
- `docs/impl/exp/*.jsonl` の event 修正
- index writer / sync engine 自体の変更（apm-packages 側の管轄）
- 他 docs dir の index drift 解消

## Components and Boundaries

- front matter 是正（本 repo の docs のみ）: schema の要求する構造へ
  の形式変換。値の判断（`type`/`status`/`updated`）は文書の意味を
  変えない範囲で保守的に行う。
- generated-index marker 追加: README の手書き部分（preamble /
  settled 以降の記述）を保持し、生成領域の境界として marker を置く。
- index 再生成: `doc_maintenance.js index apply` が owner の
  `sync_impl_index` safe sync API に dispatch し、source の
  SHA-256 pin・file list・README bytes を再検証してから atomic に
  書き込む。手編集は行わない。

## Data and Control Flow

1. `index plan --dir docs/impl/ir` で全違反ファイルと理由を列挙
   （事前に JSON で採取済み）。
2. 各ファイルの front matter を schema 準拠へ変換:
   - `metadata` 欠落 → `metadata.experiments.adopted: []` /
     `rejected: []` を追加（body の Related Experiments 記述と
     既存の正しい文書の例から採否を判断）
   - `relations.changes.*` の文字列要素 → `{type: "file", path: "..."}`
     object へ変換（既存の正しい文書と同じ形式）
   - `metadata.experiments` の配列 → `adopted`/`rejected` object へ
     再構成（採用された PoC 記述は `adopted` へ）
   - `type: "ir"` → `"impl"`（ir dir の resident type は `impl`）
   - `status: "done"` / `"final"` → `"completed"`（いずれも完了済み
     作業の記録であり、enum の `completed` が対応値）
   - `updated` の日付+注記 → 日付部分のみ（`YYYY-MM-DD`）
   - `updated` 欠落 → `created` と同日を保守的に採用
3. 両 README に generated-index marker を追加し、
   `index apply` で再生成。apply は plan を再計算して drift pin を
   再検証するため、手順 2 の途中状態では apply しない。
4. `index plan` で `in-sync` を確認し、audit を再実行して error 数を
   ベースライン（ir: error 331 / exp: error 52）と比較する。

## Non-Functional Constraints

- 既存文書の本文・ファイル名・id は不変（front matter の形式のみ）。
- 是正の判断は再現可能であること: 各ファイルの変更は機械的規則または
  本設計に記した保守的判断に基づき、PR 本文に一覧を残す。
- index の再生成は owner writer 経由のみ。README の手編集・
  `promote_index`（byte 一致 scaffold のみ対象）・force 経路は使わない。

## Risks and Trade-offs

- schema 修正で `missing-section` error が新規計上され audit error が
  増えるリスク → 是正後に audit を再実測し、増えた場合は原因を特定して
  対処方針を再検討する（本文編集は禁止のため、増える場合は要相談）。
- `status`/`type`/`updated` の値判断を誤ると index の tier や表示が
  変わるリスク → `completed` への寄せは本文の完了記述と既存文書の
  用例で裏付け、PR 本文に判断を明記する。
- 44 件の機械的変換で YAML 構造を壊すリスク → 変換後に
  `index plan` と audit で全件の parse を再検証する。

## Deferred Design Concerns

- `docs/impl/exp/*.jsonl` の `invalid-event-shape` error 修復 |
  過去ログの event 形式が現行 schema と非互換 | exp event schema の
  互換方針を決める別 Issue | audit error の残存がノイズになる |

## Verification Notes

- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/impl/ir --json` が `rejected` でない verdict を返す
- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply --dir docs/impl/ir` 実行後に `index plan` が `in-sync`
- [ ] `docs/impl/exp` も同様に `in-sync`
- [ ] 引数なし `index apply` が exit 0
- [ ] `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` の error 数 ≤ 331、`audit_experiment_log.js --json` の error 数 ≤ 52
- [ ] `git diff` で既存文書の本文・ファイル名・id に差分がないことを確認
