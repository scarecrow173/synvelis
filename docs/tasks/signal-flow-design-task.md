---
id: "TASK-034X9IBxPNjnO5mk3R7awi"
type: "task"
status: "completed"
title: "Signal Flow 設計正本 — 文書チェーンと証拠の作成"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/4"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/signal-flow-design-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/signal-flow-design-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/signal-flow-design.md"
    - "docs/impl/exp/signal-flow-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/signal-flow-design-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/signal-flow-design.md"
  verified-by:
    - "docs/test-specs/signal-flow-design.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/109"
  defers: []
  deferred-by: []
---

# Signal Flow 設計正本 — 文書チェーンと証拠の作成

## Work

Issue #4 の設計正本として、Signal Flow View の階層・Typed
Port/Connection・Semantic Routing 編集・Probe 表示・世代証拠契約を
定義する文書チェーン（spec/design/plan/task/test-spec/ir/exp）と
`evidence/sv4/` 証拠を作成する。コード実装は対象外 — 実装 Owner
は #104〜109。

- `docs/specs/signal-flow-design.md` — 要件 SFD-R1〜R8・受入・
  Deferred・Sources
- `docs/designs/signal-flow-design-design.md` — 設計正本
  （Canonical Design §A〜E・Decision Log・scenario ID 対応表）
- `docs/plans/signal-flow-design-plan.md` — 本タスクの plan
- `docs/tasks/signal-flow-design-task.md` — 本書
- `docs/test-specs/signal-flow-design.md` — 文書受入 SFD-T1〜T7
- `docs/impl/ir/signal-flow-design.md` — Implementation Record
- `docs/impl/exp/signal-flow-design.md` — Experiment Log
- `evidence/sv4/design-rationale.md`・
  `evidence/sv4/consistency-check.md` — 設計根拠と既存 SF 系
  文書との整合確認
- `docs/*/README.md` — doc-maintenance で再生成

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/signal-flow-design-plan.md`
- `docs/specs/signal-flow-design.md`
- `docs/designs/signal-flow-design-design.md`
- `docs/test-specs/signal-flow-design.md`
- Issue: <https://github.com/scarecrow173/synvelis/issues/4>

## Dependencies

- SF-01〜06 spec/design（depends-on として relations に記録、
  内容は参照のみ・変更なし）
- `synvelis-project`/`synvelis-workspace`/`synvelis-app` の既存
  型（読み取りのみ）

## Affected Files

- `docs/specs/signal-flow-design.md`
- `docs/designs/signal-flow-design-design.md`
- `docs/plans/signal-flow-design-plan.md`
- `docs/tasks/signal-flow-design-task.md`
- `docs/test-specs/signal-flow-design.md`
- `docs/impl/ir/signal-flow-design.md`
- `docs/impl/exp/signal-flow-design.md`
- `evidence/sv4/design-rationale.md`
- `evidence/sv4/consistency-check.md`
- `docs/specs/README.md`・`docs/designs/README.md`・
  `docs/plans/README.md`・`docs/tasks/README.md`・
  `docs/test-specs/README.md`・`docs/impl/ir/README.md`・
  `docs/impl/exp/README.md`（再生成のみ）

## Done When

- [ ] 文書チェーン 7 ファイルが存在し front matter の
  `type`/`status`/`relations` が規約に適合し相互参照が解決する
- [ ] design が SFD-R1〜R8 をカバーし Issue #4 受入 6 件に
  scenario ID が割り当てられている
- [ ] `evidence/sv4/` に設計根拠・代替比較・整合確認記録がある
- [ ] `docs/*/README.md` が再生成され `index plan` が `in-sync`
- [ ] `mise run fmt-check` が exit 0
- [ ] review-protocol で CONFIRMED P0/P1 blocker が残っていない
- [ ] `crates/`・manifest/lockfile/CI 設定に差分がない

## Verification

- [ ] `mise run fmt-check` — exit 0（docs/監査のみの軽量チェック）
- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan --dir docs/specs` 他、触った全 docs dir で
  `in-sync` を確認
- [ ] `node .agents/skills/impl-doc/scripts/audit_impl_record.js
  --json` — ir の audit が blocker なし
- [ ] `git status`/`git diff` で変更が docs/・evidence/ のみである
  ことを確認
