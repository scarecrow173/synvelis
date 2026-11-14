---
id: "PLAN-034X9KLNXdoJS7xcxSSn1n"
type: "plan"
status: "approved"
title: "標準 DAW 制作ワークフロー UX 正本化（Issue #8 文書チェーン）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/8"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/daw-workflow.md"
  implemented-by: []
  depends-on:
    - "docs/designs/daw-workflow-design.md"
    - "docs/test-specs/daw-workflow.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/daw-workflow-task.md"
    - "docs/impl/ir/daw-workflow.md"
    - "docs/impl/exp/daw-workflow.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/daw-workflow.md"
    - "docs/designs/daw-workflow-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/daw-workflow.md"
  references: []
  defers: []
  deferred-by: []
---

# 標準 DAW 制作ワークフロー UX 正本化（Issue #8 文書チェーン） Implementation Plan

## Goal

Issue #8 の UX 仕様を spec → design → plan → task →
test-spec → impl(ir/exp) の文書チェーンと `evidence/sv8/`
台帳として `docs/` に確定し、Gate A 代表ワークフロー・
共通操作体系・Keyboard/Gesture/Tool UX・非依存境界を
検証可能な形で固定する。

## Scope

本 plan は **文書成果物の作成のみ**を対象とする単一の
implementation stream である。`crates/` 以下のコード実装、
実 OS/実 GUI/実 Device/実 Plugin の検証、`Cargo.toml`/
`Cargo.lock`/`mise.toml`/CI の変更はスコープ外であり、
実装は #10/#11/#12/#9/#112/#114/#176 等の Owner Issue に
委譲する。

## Upstream Documents

- `docs/specs/daw-workflow.md` — WF-R-01〜10・WF-AC-01〜10・
  制作領域と View 対応・P0/P1/P2 ロードマップ。
- `docs/designs/daw-workflow-design.md` — Gate A backbone
  GA-01〜08、WF-SCN-MIDI/AUDIO/MIX、Keyboard/Gesture/Tool
  契約（§7）、共通操作体系（§8）、Recording 状態（§9）、
  非依存境界（§10）、Decision Log D-1〜D-7。

## Design Inputs

- `docs/designs/daw-workflow-design.md`（approved）
- 上位正本: `docs/designs/workspace-ux-scenarios-design.md`、
  `project-model-design.md`、`timeline-editing-design.md`、
  `recording-take-comping-design.md`、`playback-graph-design.md`、
  `transport-integration-design.md`、`media-render-design.md`、
  `workspace-docking-state-architecture.md`、
  `workspace-model-design.md`

## File Map

- `docs/specs/daw-workflow.md` - 要件・受入条件・領域対応・
  ロードマップの正本（作成済み）
- `docs/designs/daw-workflow-design.md` - UX 規範本文:
  ワークフロー/Oracle/状態図/Decision Log（作成済み）
- `docs/plans/daw-workflow-plan.md` - 本 plan
- `docs/tasks/daw-workflow-task.md` - 実装スライスと検証手順
- `docs/test-specs/daw-workflow.md` - 文書契約の検証仕様
- `docs/impl/ir/daw-workflow.md` - 実施記録（Implementation
  Record）
- `docs/impl/exp/daw-workflow.md` - 設計判断の実験/検討記録
- `evidence/sv8/` - 受入項目→仕様節→既存 docs/実装 Issue
  の追跡台帳・検証記録
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/README.md` - doc-maintenance `index apply` による
  生成 index（手編集しない）

## Implementation Sequence

- [x] spec 作成・review-protocol（cooperative/adversarial）
  実施・blocker 解消・approved 化
- [x] design 作成・review-protocol 実施・blocker 解消・
  approved 化
- [x] plan 作成（approved design を前提）
- [ ] task 作成（approved plan を前提）
- [ ] test-spec 作成・内容記入（verifies=spec）
- [ ] evidence/sv8/ 台帳作成（WF-AC→仕様節→docs/Issue 対応・
  OS-L1/GUI-L2/EXT-L3 マトリクス・NotRun/Deferred 記録）
- [ ] impl ir/exp 作成・内容記入
- [ ] doc-maintenance `index plan`→`index apply` で触れた
  dir の index を再生成
- [ ] `doc_status lint/audit`・`mise run fmt-check`・
  `git diff --check` で検証
- [ ] Conventional Commit → push → PR 作成（日本語本文）

## Dependencies and Risks

- **依存**: 上位正本 docs（#3/#10/#11/#12/#17/#18/#19/#20/
  #21）の存在と approved 状態 — 確認済み。
- **依存**: plan-doc ゲート（approved design 必須）— design
  を review-protocol 合格後に approved 化して解消。
- **リスク R-1**: 記述した Oracle が実装時に検証不能になる
  可能性 → Oracle は「文書上の期待結果」として定義し、実機
  検証は #114/#176 lane に委譲。`evidence/sv8/` で
  NotRun/Deferred を明示。
- **リスク R-2**: Keyboard/Gesture/Tool の最小操作集合が
  #112 実装で不足 → Deferred として明記済み、実装時に
  design 改訂で対応。
- **リスク R-3**: doc-driven-dev-graph router が無関係な
  既存 broken relation で fail-closed する可能性
  （sv3 先例）→ 発生時は blocker を記録し、宣言済み文書順で
  手動継続するか判断。router 成功を偽装しない。

## Task Breakdown

- T-1: docs/specs/daw-workflow.md（完了）
- T-2: docs/designs/daw-workflow-design.md（完了）
- T-3: docs/plans/daw-workflow-plan.md（本書）
- T-4: docs/tasks/daw-workflow-task.md
- T-5: docs/test-specs/daw-workflow.md
- T-6: evidence/sv8/ 台帳
- T-7: docs/impl/ir/daw-workflow.md + docs/impl/exp/
  daw-workflow.md
- T-8: index 再生成 + 検証 + commit/push/PR

## Verification

- [x] `node .agents/skills/doc-status/scripts/doc_status.js
  lint --dir docs/specs` / `--dir docs/designs` で新規文書の
  deterministic finding を確認（broken relation は plan/task
  作成・index 再生成で解消する既知分のみ）
- [ ] `node scripts/doc_maintenance.js index plan --dir
  docs/specs` 等で index 差分を preview し、`index apply`
  で適用
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js
  audit` で blocking finding が新規文書由来でないことを確認
- [ ] `mise run fmt-check`（docs-only のため cargo fmt の
  no-op 確認）
- [ ] `git diff --check`（空白エラーなし）
- [ ] `git status` で変更が許可範囲 + 生成 index のみである
  ことを確認

## Open Questions / Gaps

- Keyboard の具体的キー割当 → #112 で確定（Deferred）。
- Tool HitTest/Snap 数値 → #11 が正本（Deferred）。
- 実 OS/実 GUI/実 Device/実 Plugin の受入 → #114/#176 lane
  （NotRun として記録）。
- 新規製品トレードオフ・契約破壊・定量 SLO・OS サポート宣言
  → 人間決定待ち（本 plan では決定しない）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology
  are consistent.

## Implementation Handoff

- Recommended mode: inline execution（本 plan は文書作業のみ、
  委譲対象のコード実装なし）
- Independent work streams: none
- User approval required before dispatch: yes（review-
  protocol による代替承認済み — coordinator 契約に基づく）
- If approved: discover available implementation and
  delegation capabilities in the current environment, then
  execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- 前提: spec/design は review-protocol（2 pass + validate +
  judge）で blocker なしを確認済み。
- レビュアー注目点: WF-AC と Issue #8 受入条件 7 項目の
  対応、GA-01〜08 の Oracle が独立検証可能か、detour 契約
  が「診断を強制しない」を正しく実装しているか、
  `evidence/sv8/` の NotRun/Deferred 記録が誠実か。
