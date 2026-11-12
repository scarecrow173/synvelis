---
id: "PLAN-034Wv23ayc9G1WRGoi3DaA"
type: "plan"
status: "completed"
title: "Issue #368 docs/impl/ir missing-section 本文補完 — 実装 plan"
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
    - "docs/designs/issue-368-ir-missing-sections-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-368-ir-missing-sections.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #368 docs/impl/ir missing-section 本文補完 — 実装 plan

## Goal

`docs/impl/ir/` の missing-section 保持 60 文書の本文を、audit が
要求する 7 つの `##` セクション構造に揃え、`missing-section`
finding を 0 件にする（Issue #368）。

## Scope

単一の実装ストリーム。60 文書の本文補完は同一の受入条件
（audit 0 件・他 code 非増加・front matter 不変）に束ねられ、
文書ごとの検証は同一コマンドで行うため 1 plan とする。文書単位の
作業は独立しており、内部で並列化してよい。

## Upstream Documents

- `docs/specs/issue-368-ir-missing-sections.md`

## Design Inputs

- `docs/designs/issue-368-ir-missing-sections-design.md`

## File Map

- `docs/impl/ir/*.md`（missing-section 保持の 60 文書）- 本文の
  必須 `##` セクション補完・既存内容の正規セクションへの再配置。
  front matter・ファイル名は不変
- `docs/specs|designs|plans|tasks|test-specs/issue-368-*.md` -
  本タスクの doc-chain（新規）
- `docs/impl/ir/issue-368-*.md` - 本タスクの Implementation Record
  （`new_impl_record.js` で作成）
- `docs/*/README.md`（上記新規文書を含む dir と `docs/impl/ir`）-
  生成物として `index apply` で再生成

## Implementation Sequence

- [ ] `audit_impl_record.js --json` で missing-section 保持文書と
  不足セクションの一覧を再採取し、他 code のベースライン
  （missing-experiment-link 18 / broken-relation-link 36）を記録する
- [ ] 60 文書の本文を設計書の写像ルールに従って補完する
  （既存内容の保持・改題を優先し、根拠の無い記述は上流文書で
  事実確認してから書く）
- [ ] `audit_impl_record.js --json` を再実行し、missing-section
  0 件・他 code 非増加を確認する
- [ ] `doc_maintenance.js index plan --dir docs/impl/ir` で drift を
  確認し、必要なら `index apply` で再生成する
- [ ] 新規 doc-chain 文書と ir record を含めて commit・push・PR 化する

## Dependencies and Risks

- dependency: なし（Issue #365 / PR #366 は統合済み）
- risk: 補完時に既存本文の事実を改変する → 変更は本文セクションの
  再配置・追記に限定し、front matter を触らない
- risk: 定型文による形だけの補完 → 受入条件どおり文書固有の
  実装・検証事実を書く。根拠が無い場合は上流文書を参照する
- risk: 並列作業での表記揺れ → 必須 7 セクション名と標準小見出しを
  設計書で固定し、audit の heading 判定（`##` 完全一致）に合わせる

## Task Breakdown

- `docs/tasks/issue-368-ir-missing-sections.md` に全手順を集約した
  単一 task とする（文書補完は同一検証で一括確認でき、細分化の
  利益がない）

## Verification

- [ ] `node .agents/skills/impl-doc/scripts/audit_impl_record.js
  --json` の `missing-section` が 0 件
- [ ] 同 audit で `missing-experiment-link` ≤ 18・
  `broken-relation-link` ≤ 36
- [ ] `git diff` で 60 文書の front matter・ファイル名に差分がない
- [ ] `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index plan --dir docs/impl/ir` が `in-sync`

## Open Questions / Gaps

- `docs/impl/ir` 以外の docs dir の既存文書に同種の missing-section
  相当の不備があるかは未調査（impl audit の対象外であり本タスクの
  範囲外）。

## Self-Review

- [x] spec の要求がすべて task に対応している
- [x] placeholder が残っていない
- [x] file path、relation、status、用語が整合している

## Implementation Handoff

- Recommended mode: 並列実行可能（文書単位で独立）。各文書の補完は
  上流文書の参照を要するため、まとまった単位で担当を分ける
- Independent work streams: 60 文書をファイル単位で分割可能
- User approval required before dispatch: yes
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- レビュー観点: 補完が「既存内容の保持・改題」を守り、根拠の無い
  記述を追加していないか。front matter・ファイル名が全件不変か。
  audit の missing-section 0 件と他 code 非増加が実測で確認できるか。
- upstream: spec / design を参照。
