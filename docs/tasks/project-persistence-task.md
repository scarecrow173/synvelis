---
id: "TASK-034X6NYGLBArth4EII4yl9"
type: "task"
status: "done"
title: "Project / Asset / Persistence — 設計契約文書チェーン実装タスク（synvelis#17）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/project-persistence.md"
  depends-on:
    - "docs/designs/project-persistence-design.md"
  verifies:
    - "docs/test-specs/project-persistence.md"
  source:
    - "https://github.com/scarecrow173/synvelis/issues/17"
---

# Project / Asset / Persistence — Task

## Task 1 — docs chain + evidence ledger（本 slice 全体）

Issue #17 の設計契約を docs chain 7 文書と
`evidence/sv17/` 受入証拠台帳として固定する docs/evidence
slice。コード変更は行わない。

- [x] `docs/specs/project-persistence.md` — canonical spec
  （契約正本化・被覆台帳・OS/FS Capability 分離の要求を
  定義）。
- [x] `docs/designs/project-persistence-design.md` —
  契約写像（A〜E の 5 群・約 30 契約行）・設計決定 D1〜D6・
  OS/FS Capability マトリクス。
- [x] `docs/plans/project-persistence.md` — 実施計画。
- [x] `docs/test-specs/project-persistence.md` — Rule 1〜14
  の受入検証契約。
- [x] `evidence/sv17/case-ledger.md` — Issue #17 受入条件
  13 項目を `SV17-C01`〜`SV17-C13` へ写像し、根拠 fixture・
  Expected/Observed・環境・SHA・Pass/Fail/NotRun/
  Unsupported/Deferred を記録。構成済みテストがない契約は
  `NotRun`。先行実測（sv84・rcd-02・vcs-06・syn-io-02-os-l1）
  は SHA・日時付きで参照し、本 slice で再実行したかのように
  見せない。
- [x] `docs/impl/ir/project-persistence.md` — 実施記録
  （作成物・判断・検証結果）。
- [x] `docs/impl/exp/project-persistence.md` — 実験記録
  （契約写像の監査過程・発見・棄却案）。
- [x] `doc_maintenance.js index apply` で影響 dir
  （specs/designs/plans/tasks/test-specs/impl）の index を
  再生成。
- [x] review-protocol レビュー（客観チェック→独立 2 視点
  →findings 検証→判定）。blocker が残れば修正。
- [x] `mise run fmt-check` 実行・結果記録。
- [x] `git fetch origin main && git merge origin/main` →
  index 再生成 → Conventional Commits で論理単位に分割
  コミット → `docs/17-project-persistence` push → PR 作成
  （日本語本文・検証/未検証/review 結果/未対応作業を記録）。

## Notes

- 編集範囲は task 指定の 8 ファイル + `evidence/sv17/` +
  生成 index のみ。範囲外変更が必要になった場合は
  `orca orchestration ask` で拡張を依頼する。
- 実装 crate の `check`/`lint`/`test` は本 slice の対象外
  （正規検証は `mise run fmt-check`）。先行 slice の実測を
  参照し、本 slice で未実行の fixture は「fixture 設定済み・
  本 slice 未実走」として区別する。
- graph-router が pre-existing broken `defers` relation で
  blocked を返した件は main の既存状態として IR/台帳に
  記録し、範囲外の修復は行わない。
