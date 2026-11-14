---
id: "IMPL-034X6UvpBMux0atSwt8nXg"
type: "impl"
status: "completed"
title: "Project / Asset / Persistence — 設計契約文書チェーン実装記録（synvelis#17）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/17"
  changes:
    added:
      - type: "file"
        path: "docs/specs/project-persistence.md"
      - type: "file"
        path: "docs/designs/project-persistence-design.md"
      - type: "file"
        path: "docs/plans/project-persistence.md"
      - type: "file"
        path: "docs/tasks/project-persistence-task.md"
      - type: "file"
        path: "docs/test-specs/project-persistence.md"
      - type: "file"
        path: "docs/impl/ir/project-persistence.md"
      - type: "file"
        path: "docs/impl/exp/project-persistence.md"
      - type: "dir"
        path: "evidence/sv17"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/project-persistence-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/project-persistence-task.md"
    - "docs/designs/project-persistence-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/project-persistence.md"
    - "docs/designs/project-persistence-design.md"
    - "docs/plans/project-persistence.md"
    - "docs/test-specs/project-persistence.md"
    - "evidence/sv17/case-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/project-persistence.md"
  verified-by: []
  references:
    - "docs/impl/exp/project-persistence.md"
    - "evidence/sv17/case-ledger.md"
    - "evidence/sv84/case-ledger.md"
    - "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/82"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/290"
    - "https://github.com/scarecrow173/synvelis/issues/291"
    - "https://github.com/scarecrow173/synvelis/issues/292"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/295"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/synvelis/issues/297"
    - "evidence/sv17/case-ledger.md"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/project-persistence.md"
    rejected: []
---

# Project / Asset / Persistence — 設計契約文書チェーン実装記録（synvelis#17）

## Summary

Issue #17「Project / Asset / Persistence — Immutable
Snapshot・Media Lifecycle・Safe Save・Recovery」の設計・
ADR Issue として、Project/Asset/Persistence 横断契約を
docs chain 7 文書と `evidence/sv17/` 受入証拠台帳として
固定した。コード変更は行わず、既存の実装済み spec/design/
fixture との照合（契約写像）と受入条件 13 項目の被覆
監査に限定した docs/evidence slice である。

## Context

`docs/tasks/project-persistence-task.md`・
`docs/plans/project-persistence.md`・
`docs/specs/project-persistence.md`・
`docs/designs/project-persistence-design.md`・
`docs/test-specs/project-persistence.md` に従う。PAP
architecture（`docs/designs/project-asset-persistence-
architecture.md`）が詳細設計正本であり、本 slice はその
契約を実装済み slice（SYN-IO-01/02・VS-A/C/D・ALIB-02・
VCS-01〜06・RCD-02・X/L1/L2）と Issue #17 受入条件へ
写像し、未被覆・未実装・未検証を `NotRun`/`Deferred` と
して正直に分類する。

## Implementation

### Changes

- `docs/specs/project-persistence.md` — canonical spec。
  契約正本化・被覆台帳・OS/FS Capability 分離の要求・
  Issue #17 受入条件 13 項目の転記・Deferred Design
  Concerns を定義。
- `docs/designs/project-persistence-design.md` — 契約写像
  design。A〜E の 5 群・約 30 契約行の Contract Map・
  設計決定 D1〜D6・OS/FS Capability マトリクス・
  Alternatives Considered を記録。
- `docs/plans/project-persistence.md` — 実施計画（file
  map・sequence・verification・open questions）。
- `docs/tasks/project-persistence-task.md` — 実施 task。
- `docs/test-specs/project-persistence.md` — Rule 1〜14
  の受入検証契約（証拠区分の分類規則）。
- `evidence/sv17/case-ledger.md` — Issue #17 受入条件
  13 項目を `SV17-C01`〜`SV17-C13` へ写像し、根拠
  fixture・Expected/Observed・環境・SHA・Pass/Fail/NotRun/
  Unsupported/Deferred を記録。
- `docs/impl/exp/project-persistence.md` — 実験記録
  （契約写像の監査過程・発見・棄却案）。
- docs index（`docs/*/README.md`）— `doc_maintenance.js
  index apply` で再生成（手編集なし）。

### Decisions During Implementation

- **契約写像であり新規意味論を発明しない**: PAP
  architecture が詳細設計正本、各 slice spec が実装済み
  正本であるため、本 slice は参照・写像・被覆監査のみを
  行い、新しい port・型・crate・契約を導入しない（design
  D1）。
- **`PROJ-RCP-01` の Project 側境界のみ固定**: Render Job
  Receipt 分離・late Artifact attach の実装は RCP 系
  （#290〜#297・全て OPEN）に委譲し、本 slice では
  Saved HEAD 確定が Render Job 完了を待たない・Artifact
  反映が Project CAS/Undo/HEAD を暗黙書換えしない Project
  側契約のみを固定した（design D5）。Artifact attach 固有
  fixture は未設計のため台帳では NotRun。
- **非協調 writer の保証境界を明示**: `HEAD.lock`+expected-
  head CAS は協調 `save_cas` caller のみを直列化する。
  `save()` 直接呼出・lock 非取得・外部プロセス変更は
  advisory lock 保証外であり、検出は次回 CAS conflict・
  `head()` 再読込の正直報告に依存する（design D4・
  `syn-io-02-head-cas` design の境界を継承・拡張しない）。
- **FS Durable と Application Commit の分離**: publish
  `Ok`（durability+可視性）と HEAD repoint（commit 点）を
  別 receipt・別保証として記録し、片方の成功を他方の
  証明にしない（design D3）。
- **fixture 被覆の正直な分類**: 実装 crate の検証は本
  slice の対象外（正規検証は `mise run fmt-check`）。
  先行 slice の実測（`sv84`・`rcd-02`・`vcs-06`・
  `syn-io-02-os-l1`）を SHA・日時付きで参照し、本 slice
  で未実行の fixture は「fixture 設定済み・本 slice 未
  実走」として `構成済みテストなし=NotRun` と区別した。
- **#10 Project Model 統合の反映**: 作業中に `origin/main`
  の PR #398（synvelis#10 Project Model docs chain）が
  merge されたため、`git merge origin/main` で取り込み、
  spec/design の relations に `project-model` docs を追加
  し「#10 は実施中」記述を「PR #398 統合済み」へ更新した。
- **graph-router の blocked 結果を記録**: `doc-driven-dev-
  graph` router が repository 全体の pre-existing broken
  `defers` relation で blocked を返した。main の既存状態
  であり本 slice の欠陥ではないため修復せず、plan/本 IR
  に記録した。

## Related Experiments

### Adopted

- `docs/impl/exp/project-persistence.md` — 契約写像の
  監査過程・発見・棄却案を記録。

### Rejected

- なし（docs/evidence slice のため実験的分岐は発生せず）。

## Validation

- `mise run fmt-check` — **Pass**（exit 0）。Windows
  (win32)、rustc 1.99.1 (58cf8f9c3)・cargo 1.99.1（mise
  管理）。`cargo fmt --all -- --check` が docs/設計
  スライスの正規検証として完了。
- docs index 整合 — **Pass**。`doc_maintenance.js index
  apply` で specs/designs/plans/tasks/test-specs/impl/ir
  の index を再生成（`docs/impl/exp` は `.jsonl` のみ
  管理のため `.md` 追加で差分なし・in-sync 確認）。
  `doc_status.js lint` を影響 dir 全てに実行し、
  `project-persistence` 関連 file の finding は修正後
  ゼロ（初回検出の broken-relation・invalid-id・
  inconsistent-reciprocal を全て解消）。
- review-protocol — 客観チェック（doc-status lint・index
  整合・relation target 実在確認）→ Reviewer A（協調:
  完全性・要件写像）→ Reviewer B（敵対: 失敗モード・
  証拠の誇張・fixture path 精度）→ Validator（findings
  を実ファイル/Issue state で検証）→ Judge（severity
  判定）を separated pass で実施。CONFIRMED P0/P1 なし、
  ACCEPT WITH NON-BLOCKING NOTES。
- 実装 crate の `check`/`lint`/`test` は本 slice の対象外
  （タスク指定の正規検証は `mise run fmt-check`）。

## Risks

- **対象 SHA の CI 証拠**: 本 slice は docs/evidence のみ
  でコード変更なし。merge 後の対象 SHA ci.yml run は
  daily-ci/手動 dispatch 待ち — 統合受入は Coordinator が
  対象 SHA CI 証拠で確認する（`defers`）。nearest-SHA
  証拠を対象 SHA 証拠と混同しない。
- **fixture 被覆の解釈**: 多くの契約は先行 slice で実装・
  実測済みだが、本 slice は再実行していない。台帳は
  「fixture 設定済み・本 slice 未実走」と「構成済み
  テストなし=NotRun」を厳密に区別しているが、統合受入時
  に対象 SHA での fresh run が必要になれば各 Owner Issue
  の gate で実施される。
- **RCP 系の未実装**: `PROJ-RCP-01` の Render Job Receipt
  分離・Artifact attach 統合 fixture は未設計 — Project
  側契約のみ固定し、統合実測は RCP Issue の gate に委譲。
- **graph-router の既存 broken relations**: repository
  全体の pre-existing `defers` relation 不整合は main の
  既存状態。本 slice の新規文書が新たな broken relation
  を追加していないことを `doc-status`/index 整合で確認する。

## Follow-ups

- `evidence/sv17/case-ledger.md` の fixture 行について、
  統合受入時に対象 SHA で `mise run test <crate>` を実行し
  fresh evidence を追記するかは Coordinator/各 Owner
  Issue の判断に委譲。
- RCP 系（#290〜#297）で Artifact attach 統合 fixture が
  新設された際、`SV17-C01` の NotRun を Pass/Deferred
  へ更新する。
- macOS/Linux の File Sync/DirSync/Atomic Publish/
  Indeterminate の実機証拠は OS-L1 実機 lane で別途取得。
- 物理電断・真 power-loss・Network/Cloud FS は全 OS で
  NotRun/Deferred — 実機 lane が整備され次第分類を更新。
