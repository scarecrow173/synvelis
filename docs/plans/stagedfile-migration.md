---
id: "PLAN-034X5CZadqoSw73TKCY7cN"
type: "plan"
status: "completed"
title: "Issue #84 StagedFile Migration — 実装・証拠統合計画"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/stagedfile-migration.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/syn-io-02-directory-bundle-store-stagedfile.md"
    - "docs/plans/syn-io-02-head-cas-plan.md"
    - "docs/plans/syn-io-02-head-cas-e2e-plan.md"
    - "docs/plans/rcd-02-stagedfile-publish-migration-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/stagedfile-migration.md"
    - "docs/designs/stagedfile-migration-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/stagedfile-migration.md"
    - "docs/test-specs/syn-io-02-store-publish-contract.md"
    - "docs/test-specs/syn-io-02-head-cas.md"
    - "docs/test-specs/syn-io-02-head-cas-e2e.md"
  references:
    - "evidence/sv84/case-ledger.md"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
  defers:
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/306"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/audio-lens/issues/143"
    - "evidence/sv84/case-ledger.md"
  deferred-by: []
---

# Issue #84 StagedFile Migration — 実装・証拠統合計画 Implementation Plan

## Goal

Issue #84「DirectoryBundleStore・Snapshot/HEAD 保存を Ruviform
StagedFile へ移行」の完了 — 実装 slice（main 統合済み）と
残受入（OS-L1/Windows 実 FS 証拠・責務境界・両 Consumer
Golden 照合）を一つの Issue 単位で確定する。

## Scope

Issue #84 全体を覆う上位計画。実装本体は先行 slice の
plan（`syn-io-02-directory-bundle-store-stagedfile`・
`syn-io-02-head-cas`・`syn-io-02-head-cas-e2e`）が正本で
あり、本 plan は (a) それらの統合状態の確認、(b) 残受入の
証拠収集・台帳化、(c) 本 doc chain と `evidence/sv84/` の
整備を対象とする。コード変更が必要になった場合のみ
`crates/synvelis-project/` 内で最小の整合的変更を行う。

## Upstream Documents

- `docs/specs/stagedfile-migration.md`（本 Issue canonical spec）
- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`・
  `docs/specs/syn-io-02-head-cas.md`（詳細振る舞い正本）
- Issue #84（Acceptance 正本）・#82（Consumer 設計正本）

## Design Inputs

- `docs/designs/stagedfile-migration-design.md`（approved・
  as-built 設計）
- `docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md`
- `docs/designs/project-asset-persistence-architecture.md`

## File Map

- `crates/synvelis-project/src/store.rs` — as-built 参照のみ
  （本 plan での変更は原則なし。差分が必要なら最小修正）。
- `crates/synvelis-project/tests/{store_publish,head_cas,
  controller_store}.rs` — as-built fixture 参照。
- `evidence/sv84/case-ledger.md` — Case ID・Expected/
  Observed/NotRun の受入台帳（新規）。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/stagedfile-migration*.md` — 本 doc chain（新規）。
- `docs/*/README.md` — `doc_maintenance.js index apply` で
  再生成（手編集禁止）。

## Implementation Sequence

- [x] store.rs `write_atomic` → staged publish 移管
  （task1・PR #178 で完了）。
- [x] `save_cas`/`HEAD.lock`/`HeadCasError` 実装
  （Issue #330・PR #342 で完了）。
- [x] 2-process CAS e2e fixture（PR #382 で完了）。
- [x] syn-io-02 OS-L1 証拠記録
  （`evidence/reports/syn-io-02-os-l1-evidence.md`）。
- [ ] 本 dispatch: fixture 網羅性の再監査（書戻し・二重
  Publish・旧 Writer 上書き・Orphan 禁止・Crash window
  模擬・Recovery）と不足時の最小 fixture 追加。
- [ ] `mise run fmt-check`/`check synvelis-project`/
  `lint synvelis-project`/`test synvelis-project` を Windows
  実機で実行し、実 FS 状態遷移・Readback/再 Open・衝突/
  失敗復旧の結果を `evidence/sv84/` に記録。
- [ ] 既存 ci.yml の対象 SHA/nearest-SHA 証拠を台帳化
  （macOS/Linux L1・Windows CI leg）。
- [ ] 責務境界（Publish ≠ Project Commit）と両 Consumer
  独立 Golden の確認を design/test-spec/IR に記録。
- [ ] docs index 再生成・review-protocol レビュー・
  PR 作成。

## Dependencies and Risks

- **対象 SHA の CI run 未実施**: e0fbbe8（main HEAD）に
  対する ci.yml run は dispatch-only のため未実施の可能性。
  祖先 SHA `285e1c45` の covering run 43457068626（全 job
  success）を、`store.rs`・3 fixture・`Cargo.toml` が byte
  同一であることを `git diff` で確認した上で nearest-SHA
  証拠として採用し、対象 SHA 自体は NotRun pending と記録。
- **Crash Simulation の限界**: 真の process kill/電源断は
  fixture 範囲外 — crash window（orphan・stale lock・temp
  残滓）の決定的模擬で契約を pin し、物理電断は Deferred。
- **コード差分ゼロ想定**: 監査で契約 gap が見つかった
  場合のみ最小修正 — その場合は fixture 追加と検証を
  同一 PR で行い、IR に判断を記録する。

## Task Breakdown

- `docs/tasks/stagedfile-migration-task.md` — 本 plan の
  証拠統合・検証 slice（単一 task）。

## Verification

- [ ] `mise run fmt-check` exit 0
- [ ] `mise run check synvelis-project` exit 0
- [ ] `mise run lint synvelis-project` exit 0
- [ ] `mise run test synvelis-project` exit 0（Windows 実 FS）
- [ ] `evidence/sv84/case-ledger.md` に Case ID・Expected/
  Observed/NotRun・SHA・OS・CI Run URL が記録されている。
- [ ] review-protocol レビューで blocker なし。

## Open Questions / Gaps

- merge 後の対象 SHA ci.yml run は daily-ci/手動 dispatch
  待ち — 統合受入は Coordinator が対象 SHA CI 証拠で確認
  （本 plan の `defers` に記録）。
- Windows 実機 NTFS Case の「残件」解釈: 本 dispatch の
  Windows 実機 fixture 実走（実ローカル FS・状態遷移・
  Readback/再 Open・衝突/失敗復旧）を対象機能の L2 相当
  証拠として記録する。専用 real-FS probe lane（RCD-02
  `test-recording-real-fs` 型）は recording 側の先例で
  あり、project store には未整備 — 必要なら別 Issue で
  検討（本 Issue の現行 DoD 範囲では fixture 実 FS 実走を
  もって実機検証とする）。

## Self-Review

- [x] Every spec requirement maps to a task or evidence item.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology
  are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 task・証拠/文書
  中心。コード変更が必要になった場合のみ実装 subagent を
  検討 — ユーザー指定「実装は subagent 優先」）。
- Independent work streams: none.
- User approval required before dispatch: yes → worker-guide
  により review-protocol の独立レビューで代替。

## Review Handoff

- nearest-SHA 証拠の採用条件（byte 同一範囲の明示）。
- 「Windows 実機 fixture 実走 = L2 相当」の解釈が Issue #84
  の「実 Windows FS の状態遷移・Readback/再 Open・衝突/
  失敗復旧を対象機能に応じて検証する」を満たすかの判定は
  Coordinator/受入側の確認事項として残す。
