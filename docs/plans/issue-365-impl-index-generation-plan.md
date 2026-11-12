---
id: "PLAN-034Wsz1RfPQFUBGiJzAeHe"
type: "plan"
status: "completed"
title: "Issue #365 impl index 生成可能化 — front matter 是正と index 再生成"
created: "2026-11-12"
updated: "2026-11-13"
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
  implements:
    - "docs/specs/issue-365-impl-index-generation.md"
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
    - "docs/designs/issue-365-impl-index-generation-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-365-impl-index-generation.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #365 impl index 生成可能化 — front matter 是正と index 再生成 Implementation Plan

## Goal

`docs/impl/ir`・`docs/impl/exp` の管理 index を `doc-maintenance` の
`index plan` / `index apply` で生成・再生成できる状態にする
（Issue #365）。

## Scope

単一の実装ストリーム。front matter 是正・marker 追加・index 再生成・
検証は同じ受入条件（`in-sync` 到達と audit error 非増加）に束ねられ、
分割すると中間状態で受入条件を満たせないため 1 plan とする。

## Upstream Documents

- `docs/specs/issue-365-impl-index-generation.md`

## Design Inputs

- `docs/designs/issue-365-impl-index-generation-design.md`

## File Map

- `docs/impl/ir/*.md`（44 件）- front matter の schema 準拠化。
  本文・ファイル名・id は不変
- `docs/impl/ir/README.md` - generated-index marker 追加後、
  owner sync で再生成（手編集しない）
- `docs/impl/exp/README.md` - 同上（手書き部分は保持）
- `docs/specs|designs|plans|tasks|test-specs/issue-365-*.md` -
  本タスクの doc-chain（新規）
- `docs/impl/ir/<new record>.md` - 本タスクの Implementation Record
  （`new_impl_record.js` で作成）
- `docs/*/README.md`（上記新規文書を含む dir）- 生成物として
  `index apply` で再生成

## Implementation Sequence

- [ ] `index plan --dir docs/impl/ir --json` で違反一覧を再採取し、
  audit ベースライン（error 数）を記録する
- [ ] ir 44 件の front matter を schema 準拠へ変換する
  （機械的変換 + 設計書に記した保守的判断）
- [ ] `docs/impl/exp/README.md`・`docs/impl/ir/README.md` に
  generated-index marker を追加する
- [ ] `index apply`（引数なし）で全 dir の index を再生成する
- [ ] `index plan` で `docs/impl/ir`・`docs/impl/exp` が `in-sync`
  であることを確認する
- [ ] `audit_impl_record.js` / `audit_experiment_log.js` を再実行し、
  error 数がベースラインを超えないことを確認する
- [ ] 新規 doc-chain 文書と ir record を含めて commit・push・PR 化する

## Dependencies and Risks

- dependency: なし（兄弟 Issue ruviform#586・audio-lens#199・
  apm-packages#208 とは独立）
- risk: schema 修正で未検出だった `missing-section` finding が新規
  計上され audit error が増える → 是正後に再実測し、増えた場合は
  原因を特定して対処方針を再検討する（本文編集は禁止）
- risk: `type`/`status`/`updated` の値判断ミス → 本文記述と既存の
  正しい文書の例で裏付け、PR 本文に判断を明記する

## Task Breakdown

- `docs/tasks/issue-365-impl-index-generation.md` に全手順を集約した
  単一 task とする（ front matter 是正は機械的変換が主体で、
  task を細分化する利益がない）

## Verification

- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/impl/ir` が `rejected` でない
- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply` が exit 0
- [ ] `index plan` で `docs/impl/ir`・`docs/impl/exp` が `in-sync`
- [ ] `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` の error ≤ 331、`audit_experiment_log.js --json` の error ≤ 52
- [ ] `git diff` で既存 ir/exp 文書の本文・ファイル名・id に差分がない

## Open Questions / Gaps

- `metadata.experiments` が配列形式の gw-06〜10 について、各要素を
  `adopted` に寄せる判断でよいか（要素は exp log への path ではなく
  PoC アプローチの記述文字列）。本文に Related Experiments 節がない
  ため `adopted` への集約が最も情報を失わない選択と判断した。

## Self-Review

- [x] spec の要求がすべて task に対応している
- [x] placeholder が残っていない
- [x] file path、relation、status、用語が整合している

## Implementation Handoff

- Recommended mode: inline execution（機械的変換が主体のため委譲不要）
- Independent work streams: none
- User approval required before dispatch: yes
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- レビュー観点: front matter 是正の判断ルール（型・status・updated の
  保守的割当）が設計書の記述と一致しているか。audit error 非増加の
  見通し（missing-section の新規計上 vs invalid-front-matter の解消）
  に楽観/悲観の偏りがないか。
- upstream: spec / design を参照。
