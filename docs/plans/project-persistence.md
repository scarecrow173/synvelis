---
id: "PLAN-034X6OYfBlVRHkgOHFvAuM"
type: "plan"
status: "approved"
title: "Project / Asset / Persistence — 設計契約文書チェーン実装計画（synvelis#17）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/17"
  changes:
    added:
      - type: "file"
        path: "docs/plans/project-persistence.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/project-persistence.md"
  implemented-by: []
  depends-on:
    - "docs/designs/project-persistence-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/stagedfile-migration.md"
    - "docs/plans/syn-io-02-head-cas-plan.md"
    - "docs/plans/syn-io-02-head-cas-e2e-plan.md"
    - "docs/plans/rcd-02-stagedfile-publish-migration-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/project-persistence.md"
    - "docs/designs/project-persistence-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/project-persistence.md"
  references:
    - "evidence/sv17/case-ledger.md"
    - "evidence/sv84/case-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/82"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/93"
    - "https://github.com/scarecrow173/synvelis/issues/94"
    - "https://github.com/scarecrow173/synvelis/issues/95"
    - "https://github.com/scarecrow173/synvelis/issues/96"
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "https://github.com/scarecrow173/synvelis/issues/290"
    - "https://github.com/scarecrow173/synvelis/issues/291"
    - "https://github.com/scarecrow173/synvelis/issues/292"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/295"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/synvelis/issues/297"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "evidence/sv17/case-ledger.md"
  deferred-by: []
---

# Project / Asset / Persistence — 設計契約文書チェーン実装計画 Implementation Plan

## Goal

Issue #17「Project / Asset / Persistence — Immutable
Snapshot・Media Lifecycle・Safe Save・Recovery」の設計・
ADR Issue として、Project/Asset/Persistence 横断契約を
docs chain 7 文書と `evidence/sv17/` 受入証拠台帳として
固定する。実装は #82/#84/#86・#92〜#97・RCP 系等の各
Owner Issue へ分解済みであり、本 plan は **docs/evidence
slice のみ**を対象とする。

## Scope

- `docs/specs|designs|plans|tasks|test-specs|impl/ir|
  impl/exp/project-persistence*.md` の文書チェーン 7 件。
- `evidence/sv17/case-ledger.md` — Issue #17 受入条件 13
  項目の fixture 被覆台帳。
- 関連 docs index（`docs/*/README.md`）の `doc_maintenance`
  再生成。

**対象外**: `crates/` コード・`Cargo.toml`/`Cargo.lock`/
`mise.toml`/CI・Ruviform・PAP architecture 本文の変更・
実装 crate の `check`/`test`/`lint`（本 slice の正規検証は
`mise run fmt-check`）。

## Upstream Documents

- `docs/specs/project-persistence.md`（canonical spec）
- `docs/designs/project-persistence-design.md`（契約写像
  design・approved）
- `docs/designs/project-asset-persistence-architecture.md`
  （PAP 詳細設計正本）
- Issue #17（Acceptance 正本）・#23（実行計画）

## Design Inputs

- `docs/designs/project-persistence-design.md` の契約マップ
  （A〜E の 5 群・約 30 契約行）が被覆監査の対象一覧。
- `docs/test-specs/project-persistence.md` の Rule 1〜14 が
  台帳の分類規則。

## File Map

- `docs/specs/project-persistence.md` — canonical spec（作成済み）。
- `docs/designs/project-persistence-design.md` — 契約写像
  design（作成済み）。
- `docs/plans/project-persistence.md` — 本 plan。
- `docs/tasks/project-persistence-task.md` — 実施 task。
- `docs/test-specs/project-persistence.md` — 受入検証契約。
- `docs/impl/ir/project-persistence.md` — 実施記録。
- `docs/impl/exp/project-persistence.md` — 実験記録。
- `evidence/sv17/case-ledger.md` — 受入証拠台帳（新規）。
- `docs/{specs,designs,plans,tasks,test-specs,impl}/README.md`
  — `doc_maintenance.js index apply` で再生成（手編集禁止）。

## Implementation Sequence

- [x] spec/design の作成（契約写像の固定）。
- [x] test-spec の作成（Rule 1〜14 の分類規則）。
- [ ] task の実施: Issue #17 受入条件 13 項目を Case ID
  （`SV17-C01`〜`SV17-C13`）へ写像し、各項目の根拠 fixture
  （crate::test 名）・Expected/Observed・環境・SHA・
  Pass/Fail/NotRun/Unsupported/Deferred を
  `evidence/sv17/case-ledger.md` に記録。
- [ ] `doc_maintenance.js index apply` で影響 dir の index
  を再生成し、整合を確認。
- [ ] review-protocol レビュー（客観チェック→独立 2 視点→
  findings 検証→判定）。blocker が残れば修正して再レビュー。
- [ ] `mise run fmt-check` を実行し結果を記録。
- [ ] `git fetch origin main && git merge origin/main` で
  最新 main を取り込み、index を再生成してコミットに含める。
- [ ] Conventional Commits で論理単位に分割してコミット、
  `docs/17-project-persistence` ブランチへ push、PR 作成。

## Dependencies and Risks

- **対象 SHA の CI 証拠**: 本 slice は docs/evidence のみで
  コード変更なし。merge 後の対象 SHA ci.yml run は
  daily-ci/手動 dispatch 待ちであり、統合受入は
  Coordinator が対象 SHA CI 証拠で確認する（`defers` に
  記録）。nearest-SHA 証拠を対象 SHA 証拠と混同しない。
- **fixture 被覆の解釈**: 多くの契約は先行 slice で実装・
  実測済みだが、本 slice は実装 crate の検証を再実行しない。
  「fixture 設定済み・本 slice 未実走」と「構成済みテスト
  なし=NotRun」を台帳で厳密に区別する。先行実測は
  `evidence/sv84/`・`evidence/reports/` の SHA・日時付き
  記録を参照する。
- **graph-router の既存 broken relations**: `doc-driven-dev-
  graph` router が repository 全体の pre-existing broken
  `defers` relation で blocked を返した。これは main の
  既存状態であり本 slice の欠陥ではない — 台帳/IR に記録
  し、範囲外の修復は行わない。
- **スコープ境界**: 編集範囲は task 指定の 8 ファイル+
  `evidence/sv17/`+生成 index のみ。範囲外変更が必要に
  なった場合は `orca orchestration ask` で拡張を依頼する。

## Task Breakdown

- `docs/tasks/project-persistence-task.md` — 本 plan の
  docs/evidence slice（単一 task）。

## Verification

- [ ] `mise run fmt-check` exit 0（docs/設計スライスの正規
  検証。実装 crate の `check`/`lint`/`test` は対象外）。
- [ ] `evidence/sv17/case-ledger.md` に Case ID・根拠
  fixture・Expected/Observed・環境・SHA・NotRun 分類が
  記録されている。
- [ ] docs index が `doc_maintenance.js index apply` で
  再生成され、手編集差分がない。
- [ ] review-protocol レビューで blocker なし。

## Open Questions / Gaps

- RCP 系（#290〜#297）の Artifact attach 統合 fixture は
  未設計 — `PROJ-RCP-01` の Render Job Receipt 分離は
  Project 側契約のみ固定し、統合実測は RCP Issue の gate
  に委譲（`defers`）。
- macOS/Linux の File Sync/DirSync/Atomic Publish/Indeterminate
  の実機証拠は NotRun — 実機 lane は別途（POLICIES.md §2
  の OS-L1 区分）。
- 物理電断・真の power-loss・Network/Cloud FS は全 OS で
  NotRun/Deferred — injected/process-kill 相当の fixture
  のみが契約を pin する。
- Save Job scheduler・Autosave Root 物理配置・Backup/Save
  As・Retention 期間・GC UI は #92 系/PAP arch §15 の
  Deferred 事項。

## Self-Review

- [x] Every spec requirement maps to a task or evidence item.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology
  are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 task・docs/
  evidence 中心・コード変更なし）。
- Independent work streams: none.
- User approval required before dispatch: yes → worker-guide
  により review-protocol の独立レビューで代替。

## Review Handoff

- 「fixture 設定済み・本 slice 未実走」の分類が
  `evidence/sv17/case-ledger.md` で先行実測（sv84・rcd-02・
  vcs-06）と正しく区別されているか。
- `PROJ-RCP-01` の Project 側境界（D5）の解釈が Issue #17
  の意図と一致するか — Render Job Receipt の実装詳細は
  RCP 系に委譲する判断の妥当性。
