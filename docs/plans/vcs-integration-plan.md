---
id: "PLAN-034X9Fn5NTJQhX4cdeEEoq"
type: "plan"
status: "completed"
title: "VCS Integration 統合契約文書チェーン + evidence/sv7 Case 被覆台帳 (synvelis#7)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
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
    - "docs/specs/vcs-integration.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/vcs-integration-design.md"
    - "docs/specs/vcs-integration.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-integration.md"
    - "docs/designs/vcs-integration-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-integration.md"
  references:
    - "evidence/sv7/vcs-integration-case-ledger.md"
    - "evidence/sv7/vcs-integration-verification.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/reports/vcs-06-compat-evidence.md"
  defers: []
  deferred-by: []
---

# VCS Integration 統合契約文書チェーン + evidence/sv7 Case 被覆台帳 (synvelis#7) Implementation Plan

## Goal

Issue #7 の統合契約を spec/design/plan/task/test-spec/impl-ir/impl-exp
の文書チェーンとして canonical 化し、`evidence/sv7/` に受入 Case
被覆台帳と検証記録を作る docs-only スライスを完了する。

## Scope

単一の docs-only 実装ストリーム。コード・CI・本番 crate は対象外であり、
plan 分割は不要。`docs/` 7 文書 + `evidence/sv7/` 2 文書 + 触れた
dir の管理 index 再生成で構成する。

## Upstream Documents

- `docs/specs/vcs-integration.md`（本 plan が implements する spec）
- Issue #7（統合契約の出典）
- `docs/specs/vcs-01-*.md` 〜 `docs/specs/vcs-06-*.md`（責務対応の正本）

## Design Inputs

- `docs/designs/vcs-integration-design.md`（approved — 層構成・
  責務対応表・8 view 採否・Creative Branch 分離・sv#95 整合記録）

## File Map

| Path | Responsibility |
| --- | --- |
| `docs/specs/vcs-integration.md` | 統合契約 spec（What/Why/受入条件） |
| `docs/designs/vcs-integration-design.md` | 層構成・責務対応・Case 被覆・UI 採否設計 |
| `docs/plans/vcs-integration-plan.md` | 本計画 |
| `docs/tasks/vcs-integration-task.md` | 実行単位タスク |
| `docs/test-specs/vcs-integration.md` | 統合契約の保証事項 test-spec |
| `docs/impl/ir/vcs-integration.md` | 実装記録（docs-only slice） |
| `docs/impl/exp/vcs-integration.md` | 実験/判断ログ |
| `evidence/sv7/vcs-integration-case-ledger.md` | Case ID 被覆台帳（VCS-SNAP/DIFF/GIT/UI/APPLY/RESTORE/COMPAT） |
| `evidence/sv7/vcs-integration-verification.md` | 検証実行記録（コマンド・OS・toolchain・exit code） |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | `doc-maintenance`/`sync_impl_index` による再生成 index |

## Implementation Sequence

- [x] spec 作成（`new_spec.js`、status approved）
- [x] design 作成（`new_design.js --from spec`、status approved）
- [x] plan 作成（本書）
- [x] test-spec 作成（`new_test_spec.js --verifies spec --verifies design`）
- [x] task 作成（`new_task.js --plan`）
- [x] `evidence/sv7/vcs-integration-case-ledger.md` 作成 — 全 Case ID + 拡張行の
  被覆台帳（owner/依存/fixture/verification layer/requirement state/
  result state/evidence path）
- [x] `evidence/sv7/vcs-integration-verification.md` 作成 — 実行した検証の記録
- [x] impl-ir / impl-exp 作成（`new_impl_record.js`/`new_experiment_log.js`）
- [x] `doc_maintenance.js index plan|apply` で 7 dir の index 再生成
- [x] `sync_impl_index.js --kind ir|exp --apply` で impl index 同期
- [x] `mise run fmt-check` 実行
- [x] `doc_status` lint/audit・`audit_impl_record.js`・
  `audit_experiment_log.js`・`route_graph.js --focus` 実行
- [x] `review-protocol` レビュー（blocker なしを確認）
- [ ] Conventional Commits で論理単位コミット → push → PR

## Dependencies and Risks

- **依存**: 既存 vcs-01..06 spec・`evidence/reports/vcs-06-*.md`
  ledger が正本。これらは read-only 参照であり変更しない。
- **リスク**: doc graph の pre-existing broken relations（legacy
  `ISSUE-*`/`SPEC-*` token）が route/audit を阻害 → 本スライス起因の
  新規 broken relation のみ修正対象とし、既存分は記録に留める。
- **リスク**: ledger の verdict を過大評価 → `Pass*`/`NotRun`/
  `Unsupported` をそのまま転記し、headless Pass を実機受入に換算
  しない。

## Task Breakdown

- 単一 task `docs/tasks/vcs-integration-task.md` が本 plan の全工程を
  カバーする（docs-only・単一ストリームのため分割しない）。

## Verification Matrix

| Step | Verification | Pass condition |
| --- | --- | --- |
| 文書チェーン | `doc_status` lint/audit（対象 dir） | 新規文書に error なし（pre-existing finding は記録） |
| index | `doc_maintenance.js index plan/apply` + `sync_impl_index.js` | planHash 一致・drift なし |
| fmt | `mise run fmt-check` | exit 0 |
| impl audit | `audit_impl_record.js --json` / `audit_experiment_log.js --json` | error なし |
| graph | `route_graph.js --focus docs/specs/vcs-integration.md --json` | 結果を記録（pre-existing blocker は区別） |
| review | `review-protocol` | blocker 0 件 |

## Open Questions / Gaps

- チャット添付 HTML モックアップ本体はリポジトリ未収録。8 view の
  採否は Issue 本文 + 既存 view mode 決定を根拠に記録し、pixel
  レビューは別作業（spec/design に明記済み）。
- ledger 拡張行（VCS-DIFF-09/10、VCS-APPLY-07/08、
  VCS-RESTORE-07/08/09、VCS-GIT-REAL-*）の正確な verdict は
  `vcs-06-fixture-matrix-ledger.md` の現行内容を転記する。

## Self-Review

- [x] spec の全受入条件が plan の工程または evidence 台帳へ対応する
- [x] プレースホルダなし（未確定は Open Questions に明示）
- [x] ファイルパス・relation・status・用語が spec/design と一致

## Implementation Handoff

- Recommended mode: inline execution（docs-only・単一ストリーム）
- Independent work streams: none
- User approval required before dispatch: no（本スライスは worker
  dispatch として承認済み。review-protocol が承認ゲートを代替）

## Review Handoff

- reviewer は spec の 14 受入条件と `evidence/sv7/vcs-integration-case-ledger.md`
  の Case ID 被覆の 1:1 対応、および verdict の誠実性
  （NotRun/Unsupported の維持）を重点確認する。
- upstream: `docs/specs/vcs-integration.md`、
  `docs/designs/vcs-integration-design.md`、Issue #7/#97。
