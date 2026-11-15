---
id: "TASK-034XJDw7QIooVc0kaaWeA9"
type: "task"
status: "done"
title: "RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — 文書チェーン・ADR 合意タスク（synvelis#120）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-f02-monitoring-overdub.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-f02-monitoring-overdub.md"
    - "docs/specs/rcd-f02-monitoring-overdub.md"
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-f02-monitoring-overdub.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
    - "docs/impl/ir/rcd-f02-monitoring-overdub.md"
    - "docs/impl/exp/rcd-f02-monitoring-overdub.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/rcd-f02-monitoring-overdub.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-f02-monitoring-overdub.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  defers: []
  deferred-by: []
---

# RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — 文書チェーン・ADR 合意タスク（synvelis#120）

## Work

Issue #120 の ADR 合意を canonical 文書チェーンと evidence として固定する docs-only slice。実装は ADR merge 後の後続 slice が Owner。

## Classification

- `normal-plan-task`

## タスク分解

- [x] **T1 spec**: `docs/specs/rcd-f02-monitoring-overdub.md` — What/Why・REQ-F02-01〜07・AC-F02-01〜10・Non-goals・Sources
- [x] **T2 design**: `docs/designs/rcd-f02-monitoring-overdub-design.md` — 既存契約 inventory・UX Matrix・notice semantics・HandleGrant/retention・Undo/Recovery・Privacy・却下選択肢
- [x] **T3 plan**: `docs/plans/rcd-f02-monitoring-overdub.md` — scope・file map・dependencies・risks・verification・handoff・rollback
- [x] **T4 task**: 本書 — task 分解・完了条件・後続実装 Owner
- [x] **T5 test-spec**: `docs/test-specs/rcd-f02-monitoring-overdub.md` — Feature/Rule/Example/Guarantee・状態語彙・fixture 契約
- [x] **T6 adr**: `docs/adr/rcd-f02-monitoring-overdub.md` — **納品本体** 5 領域の MADR 決定（D1〜D5）
- [x] **T7 evidence**: `evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md`・`evidence/sv120/rcd-f02-monitoring-overdub-verification.md`
- [x] **T8 ir**: `docs/impl/ir/rcd-f02-monitoring-overdub.md` — 実施記録（docs-only・handoff）
- [x] **T9 exp**: `docs/impl/exp/rcd-f02-monitoring-overdub.md` — graph route・ID 生成・決定と選択肢の記録
- [x] **T10 index**: touched dir（specs/designs/plans/tasks/test-specs/adr/impl-ir/impl-exp）の管理 index を doc-maintenance `index apply`・`sync_impl_index` で再生成し `in-sync` 確認
- [x] **T11 verify**: `mise run fmt-check`・prek・doc-status audit・type 別 lint・impl audit・ADR audit・review-protocol（blocker 0）
- [x] **T12 ship**: `git fetch origin main && git merge origin/main` → commit → `git push -u origin scarecrow173/sv120-rcd-f02` → `gh pr create`（日本語本文・`Refs #120`）→ `worker_done`

## 後続実装 Owner（本 slice 対象外）

| 領域 | 想定 crate/module | ADR 決定 |
| --- | --- | --- |
| Overdub route | `synvelis-recording::monitor`・`synvelis-player::monitor`/`monitor_engine` | D1 `MonitorRoute::Overdub` |
| Feedback Risk | `synvelis-recording::monitor` notices・`synvelis-app` 投影 | D2 `double_monitoring` 発火条件拡張 |
| Capture Handle | `synvelis-recording::{journal,gate,plan,session}`・capture engine | D3 kind=4 `HandleGrant` + `JournalSpan` 分類 |
| Undo/Recovery | 既存 project undo + RCD-02 verified prefix | D4 既存契約整合 |
| Privacy | `synvelis-app` consent UX・session-scoped grant | D5 consent なし永続化禁止 |
| UX/Fixture | `synvelis-app::monitoring`・`synvelis-project` | Consent/Warning UX・overdub 宣言 field |

## 完了条件

- 8 文書 + `evidence/sv120/` が canonical front matter・relations・status で揃う。
- ADR が 5 領域（UX Matrix・Feedback Risk・Capture Handle Retention・Undo/Recovery・Privacy）の決定を MADR 形式で記録し、`docs/adr/README.md` index に登録される。
- UX Matrix が既存 `decide` 判定表を baseline 転記し、Overdub route 表現拡張が ADR 決定として明示されている。
- Feedback Risk が既存 notice を正本再利用し、新 notice が必要な場合は ADR 決定として記録されている。
- Capture Handle の必須属性（User Consent・保存容量・SourceRange/Clock 証拠）と retention 規則が定義され、「明示有効化区間のみ journal 記録・保存・無許可の隠し Media なし」が契約化されている。
- Undo/Recovery が RCD-02 verified prefix に整合し、Undo と Recovery が別 semantics として定義されている。
- Privacy が「monitor 内容は consent なしに永続化しない」「punch 外素材は明示区間のみ journal/保存」を決定文として含む。
- Hardware Direct の honest-capability（3 分類・制御非保証・`Unknown` を制御可能と表示しない）が明記されている。
- `Pass (design)`/`Pass (headless)`/`NotRun`/`Deferred`/`Unsupported` が POLICIES.md 語彙で厳密分離され、headless/design 証拠を実機 Pass に換算していない。
- 触れた dir の管理 index が `in-sync`。
- `mise run fmt-check`・prek・doc-status 監査・impl audit・ADR audit が実行され、pre-existing failure と slice 起因 failure が区別されている。review-protocol blocker 0。
- 差分が占有範囲 + 生成 index + evidence のみ。

## ブランチ

`scarecrow173/sv120-rcd-f02`（worktree `G:/orca/workspaces/synvelis/sv120-rcd-f02`、base `origin/main` `194436b` 以降）。
