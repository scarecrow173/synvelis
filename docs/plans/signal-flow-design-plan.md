---
id: "PLAN-034X9HnJ3ksOwiuiGUkOTC"
type: "plan"
status: "approved"
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
    - "docs/specs/signal-flow-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/signal-flow-design.md"
    - "docs/designs/signal-flow-design-design.md"
    - "docs/test-specs/signal-flow-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-365-impl-index-generation.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/signal-flow-design.md"
    - "docs/designs/signal-flow-design-design.md"
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

# Signal Flow 設計正本 — 文書チェーンと証拠の作成 Implementation Plan

## Goal

Issue #4 の設計正本として、Signal Flow View の階層・Typed
Port/Connection・Semantic Routing 編集・Probe 表示・世代証拠契約を
define する文書チェーン（spec/design/plan/task/test-spec/ir/exp）と
`evidence/sv4/` 証拠を作成し、実装 Owner #104〜109 が参照する
単一の設計正本を確立する。コード実装は含まない。

## Scope

単一の実装ストリーム（文書作成のみ）。分割不要 — spec/design/
plan/task/test-spec/ir/exp の 7 文書と evidence ファイルは同一
変更単位で完結し、相互依存が強いため 1 plan とする。実装コード
（`crates/`）は本 plan の対象外であり、#104〜109 が別 plan で
実施する。

## Upstream Documents

- `docs/specs/signal-flow-design.md` — 要件・受入仕様（本 plan の
  implements 対象）
- `docs/specs/sf-01..06-*.md` — SF 系実装契約の正本群（depends-on）
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md` —
  lowering/activation の統合設計権威（参照）
- `docs/designs/sf-05-slice{1,2}-*.md` — Signal Flow View/gesture
  の slice 設計（参照）

## Design Inputs

- `docs/designs/signal-flow-design-design.md`（status: approved）
  — Canonical Design §A〜E・Decision Log・Deferred Design
  Concerns・scenario ID 対応表
- `docs/test-specs/signal-flow-design.md` — 文書受入ルール
  SFD-T1〜T7（verified-by）

## File Map

- `docs/specs/signal-flow-design.md` — 要件 SFD-R1〜R8・受入・
  Deferred・Sources（作成済み）
- `docs/designs/signal-flow-design-design.md` — 設計正本
  （作成済み・approved）
- `docs/test-specs/signal-flow-design.md` — 文書受入 test-spec
  （作成済み）
- `docs/plans/signal-flow-design-plan.md` — 本書
- `docs/tasks/signal-flow-design-task.md` — 実行 task（生成予定）
- `docs/impl/ir/signal-flow-design.md` — Implementation Record
  （生成予定・実施結果を記録）
- `docs/impl/exp/signal-flow-design.md` — Experiment Log
  （生成予定・検証試行を記録）
- `evidence/sv4/design-rationale.md` — Decision Log 根拠・代替比較
- `evidence/sv4/consistency-check.md` — 既存 SF 系 spec/design
  との整合確認記録
- `docs/specs/README.md`・`docs/designs/README.md`・
  `docs/plans/README.md`・`docs/tasks/README.md`・
  `docs/test-specs/README.md`・`docs/impl/ir/README.md`・
  `docs/impl/exp/README.md` — doc-maintenance で再生成
  （手編集禁止）

## Implementation Sequence

- [x] spec 作成（Issue #4 由来の要件・受入・Deferred・Sources）
- [x] design 作成（approved — review-protocol 相当のレビューで
  blocker なし）
- [x] test-spec 作成（SFD-T1〜T7）
- [ ] plan 作成（本書）
- [ ] task 作成（`docs/tasks/signal-flow-design-task.md`）
- [ ] evidence 2 ファイル作成
- [ ] ir/exp 生成・記録
- [ ] docs index 再生成（`doc_maintenance.js index apply`）
- [ ] `mise run fmt-check` 実行
- [ ] review-protocol 最終レビュー・blocker 解消
- [ ] Conventional Commit・push・PR 作成

## Dependencies and Risks

- **依存**: SF-01〜06 spec/design・`synvelis-project`/`workspace`/
  `app` の既存型（読み取りのみ・変更なし）。Issue #4 本文。
- **リスク**: doc-driven-dev-graph router が repository 全体の
  既存 broken relation（sf-03 spec の `defers` 等）で blocked を
  返す → 本タスク起因ではないため記録のみで進行（PR 本文に記載）。
- **リスク**: 未決事項（Visual Group 保存粒度等）を誤って決定
  してしまう → Decision Log「保留」区分で分離済み。
- **リスク**: 実機検証を Pass と誤記 → lane 状態を headless/
  NotRun で明記する規則を test-spec SFD-T6 に固定。

## Task Breakdown

- TASK-SFD-01: 文書チェーン 7 ファイルの作成と内容記入
- TASK-SFD-02: `evidence/sv4/` 証拠ファイル作成
- TASK-SFD-03: impl ir/exp 記録と検証結果の記入
- TASK-SFD-04: docs index 再生成・fmt-check・review-protocol
- TASK-SFD-05: commit/push/PR

## Verification

- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan --dir docs/<type>` が触った全 dir で `in-sync`
- [ ] `mise run fmt-check` exit 0
- [ ] `git diff` に `crates/`・`Cargo.toml`・`Cargo.lock`・
  `mise.toml`・`.github/workflows/` の変更がない
- [ ] design §E.4 scenario 表が Issue #4 受入 6 件を全てカバー
- [ ] test-spec SFD-T1〜T7 が design/spec の内容と矛盾しない
- [ ] review-protocol で CONFIRMED P0/P1 が残っていない

## Open Questions / Gaps

- doc-driven-dev-graph router の blocked 状態は repository 全体の
  既存 broken relation が原因。本タスク範囲外のため修復しない
  （PR 本文「未対応の作業」に記載）。
- 実機 GUI/Plugin/Device 受入は #114/#109 の外部 gate であり
  本タスクでは NotRun。

## Self-Review

- [x] Every spec requirement maps to a task.（SFD-R1〜R8 →
  design §A〜E・Decision Log・scenario 表に対応、task は文書
  作成単位）
- [x] No placeholders remain.（記入後に再確認）
- [x] File paths, relations, status values, and terminology are
  consistent.（`ProbeResolveState`・`RoutingEditError`・
  `SignalFlow*` 型名は実装語彙と照合済み）

## Implementation Handoff

- Recommended mode: inline execution（文書作成のみ・委譲不要）
- Independent work streams: none（単一ストリーム）
- User approval required before dispatch: yes（worker contract に
  より review-protocol を承認代替として実施済み）
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute task-by-task
  with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- 前提: design は approved（review-protocol 相当レビューで
  blocker なし）。test-spec は文書受入のみを対象とし実装検証を
  含まない。
- レビュアー注目点: scenario ID（SFD-*）が #109 Case registry と
  混同されないこと、未決事項が Decision Log「保留」に分離されて
  いること、lane 状態が headless Pass と実機 NotRun を区別して
  いること。
- 既知の外部 blocker: doc-driven-dev-graph router が repository
  全体の既存 broken relation で blocked（本タスク起因ではない）。
