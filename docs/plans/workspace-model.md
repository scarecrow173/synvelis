---
id: "PLAN-034X52muh3ycW5cWGEkyBN"
type: "plan"
status: "in-progress"
title: "Workspace Model Plan — design + headless verification"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/2"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/workspace-model-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/workspace-model-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/workspace-model.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/workspace-model-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/workspace-model.md"
  references: []
  defers: []
  deferred-by: []
---

# Workspace Model Plan — design + headless verification

## Goal

Issue #2 の Workspace Model を、GPUI-neutral View/Driver Port + Plugin Slot を
反映した設計文書と、既存 headless fixture を Case ID 付きで台帳化した
`evidence/sv2/` 検証記録として確定する。

## Scope

単一の設計・検証ストリーム。製品実装（#113/#175）や実機 GUI 検証（#114）は
含まない。コード変更は headless 契約ギャップが確認された場合の最小限の
test 追加に限定する。

## Upstream Documents

- `docs/specs/workspace-model.md`（SPEC-034X4TqMpHOeFHvohktOZV）
- `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
- `docs/specs/workspace-ux-scenarios.md`

## Design Inputs

- `docs/designs/workspace-model-design.md`（DESIGN-034X4TrgrbyPHCdc8l0xPg, approved）
- `docs/designs/workspace-docking-state-architecture.md`（初期ベースライン、legacy 注記の対象）
- `docs/designs/workspace-ux-scenarios-design.md`
- `docs/designs/ws-a-workspace-foundation-design.md`

## File Map

- `docs/specs/workspace-model.md` — 要求・受入条件（WSM-R-*/WSM-AC-*）
- `docs/designs/workspace-model-design.md` — GPUI-neutral 構造図・不変条件・
  識別子・ライフサイクル・routing・Decision Log・未決事項
- `docs/plans/workspace-model.md` — 本書
- `docs/tasks/workspace-model-task.md` — 実施 task
- `docs/test-specs/workspace-model.md` — Case ID → test/oracle 対応
- `docs/impl/ir/workspace-model.md` — 実装記録
- `docs/impl/exp/workspace-model.md` — 実験・検証記録
- `evidence/sv2/workspace-model-case-ledger.md` — Case ID/Expected/Observed/NotRun 台帳
- `evidence/sv2/workspace-model-verification.md` — 実行コマンド・OS・toolchain・exit code
- `crates/synvelis-workspace/tests/workspace_model.rs` — playback 中 churn の
  契約ギャップ補完 test（WSM-T-08。既存 `ws_a_05` は playing 未設定のため
  「再生中」の qualifier を直接検証する test を追加）

## Implementation Sequence

- [x] spec 作成（WSM-R-01〜09 / WSM-AC-01〜09）
- [x] design 作成（GPUI-neutral 図・不変条件・Decision Log・legacy 注記）
- [x] task/test-spec 作成（Case ID 対応付け）
- [x] 契約ギャップ確認: playback 中 dock/window churn の直接 test が無いことを
  確認し `workspace_model.rs` を追加（他は既存 fixture で充足）
- [x] `mise run fmt-check` / `check` / `lint` / `test`（synvelis-workspace,
  synvelis-app）を実行し evidence に記録
- [x] `evidence/sv2/` 台帳・検証記録を作成
- [x] impl/ir・impl/exp を作成
- [x] review-protocol spec+design レビュー → blocker 解消
- [x] docs index 再生成（doc-maintenance `index apply`）
- [ ] `git fetch origin main && git merge origin/main` → commit → push → PR

## Dependencies and Risks

- 依存: WS-A/ED-A/UI-C5/SV-GUI-04 の既存実装と test（全て本 branch に存在）。
- リスク: `doc-driven-dev-graph` router が pre-existing broken-relation で
  fail-closed blocked — 本 task の文書起因でないことを evidence に記録し、
  範囲外の修復は行わない。
- リスク: headless 合格を実機合格と誤読 — evidence で lane を明示。

## Task Breakdown

- T1: 文書チェーン（spec/design/plan/task/test-spec）
- T2: headless fixture 補完（`workspace_model.rs` playback churn test のみ）
- T3: 検証実行と evidence 台帳
- T4: impl 記録・index 再生成・review・commit/PR

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check synvelis-workspace` / `mise run check synvelis-app`
- [ ] `mise run lint synvelis-workspace` / `mise run lint synvelis-app`
- [ ] `mise run test synvelis-workspace` / `mise run test synvelis-app`
- [ ] `cargo test -p synvelis-workspace --test workspace_model`（mise test に
  含まれるが、追加 test の存在を個別確認）
- [ ] docs index 整合（`doc_maintenance.js index apply` + `audit_docs.js`）

## Open Questions / Gaps

- 実 OS 2 Window/Dock/IME/Keyboard/plugin GUI — #114/#176 gate（NotRun）。
- `ShellWindowHost` multi-window 製品配線 — #113/#175（本 slice 対象外）。
- window-owned job cancel の実挙動 — `CallOutcome::Job` op 未実装（NotRun）。
