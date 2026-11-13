---
id: "EXP-sv17-project-persistence"
type: "experiment"
status: "completed"
title: "Project / Asset / Persistence — 設計契約写像の実験記録（synvelis#17）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/17"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/project-persistence.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-persistence-design.md"
    - "docs/specs/project-persistence.md"
    - "docs/tasks/project-persistence-task.md"
    - "evidence/sv17/case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/project-asset-persistence-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv17/case-ledger.md"
    - "evidence/sv84/case-ledger.md"
    - "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/290"
    - "https://github.com/scarecrow173/synvelis/issues/291"
    - "https://github.com/scarecrow173/synvelis/issues/292"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/295"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/synvelis/issues/297"
  deferred-by: []
---

# Project / Asset / Persistence — 設計契約写像の実験記録（synvelis#17）

## 仮説

Issue #17 の dispatch 時点で、Project/Asset/Persistence の
横断契約の大部分は先行 slice（VS-A/C/D・SYN-IO-01/02・
ALIB-02・VCS-01〜06・RCD-02・X/L1/L2）により main 統合
済みであり、本 task の実作業はコード変更ではなく docs
chain・証拠台帳・被覆監査に限定されるのではないか —
PAP architecture・既存 spec/design・fixture の棚卸しで
確認する。

## 方法

- `docs/designs/project-asset-persistence-architecture.md`
  （PAP architecture）と Issue #17 本文（受入条件 13
  項目・10 項目合意・PAP-01〜18）を読み、横断契約の
  一覧を抽出した。
- `docs/specs|designs|plans|tasks|test-specs/` の既存
  `syn-io-*`・`vs-*`・`alib-02-*`・`vcs-*`・`rcd-02-*`・
  `x-*`・`l1-*`・`l2-*`・`stagedfile-migration`・
  `project-model` を読み、各契約の正本 spec/design と
  実装 Owner（crate）を特定した。
- `crates/{synvelis-project,synvelis-library,
  synvelis-recording,synvelis-render}/tests/` の fixture
  を `grep`/`ls` で棚卸しし、Issue #17 受入条件 13 項目
  それぞれに対応する test 関数・test file の存在を確認
  した（`store_publish.rs`・`head_cas.rs`・`controller_
  store.rs`・`vcs01_snapshot.rs`・`vcs02_diff.rs`・
  `vcs05_restore.rs`・`alib02_digest.rs`・`alib02_v5_real.rs`・
  `plugin_state.rs`・`monitoring_persist.rs`・`import.rs`・
  `import_recovery.rs`・`vs_d.rs`・`recovery_review.rs`・
  `project_adapter.rs`・`x_e2e.rs`・`l2_e2e.rs` 等）。
- 先行 slice の実測証拠を `evidence/sv84/case-ledger.md`・
  `evidence/reports/{rcd-02-durability-recovery-review-
  ledger,vcs-06-fixture-matrix-ledger,syn-io-02-os-l1-
  evidence}.md` で確認し、SHA・環境・分類を抽出した。
- `gh issue view` で関連 Issue（#82/#84/#86/#92〜#97/
  #289〜#297/#10）の state を確認し、実装済み/OPEN を
  区別した。
- `doc-driven-dev-graph` router を実行し、graph 状態と
  route を確認した（結果は後述）。

## 観測結果

- **契約の大部分は実装済み**: Snapshot/HEAD publish・
  StagedFile/CAS・Asset Lifecycle・Import pipeline・
  Recording CommitIdentity・Recovery Review・GC 保護・
  Schema Migration・VCS read-only・外部変更検出の各契約に
  対応する fixture が `crates/` 配下に存在する。PAP
  architecture の 10 項目合意と Issue #17 受入条件は、
  既存 spec/design/fixture の組合せでほぼ被覆される。
- **未実装・未設計の契約が残る**: `PROJ-RCP-01` の Render
  Job Receipt 分離・Artifact attach 統合（RCP 系 #290〜
  #297 は全て OPEN）・Active Execution Plan revision の
  永続化接続・Save Job scheduler 本体・Autosave/Recovery
  Root 物理配置・Backup/Save As・Retention 期間・GC UI
  は fixture なし — `NotRun`/`Deferred` として台帳に記録。
- **先行実測は豊富だが本 slice では再実行しない**:
  `sv84`（StagedFile/CAS・Windows NTFS）・`rcd-02`
  （Recording fault matrix・Windows）・`vcs-06`（VCS
  fixture matrix・Windows）の各 ledger に SHA・日時付きの
  実測がある。本 slice の正規検証は `mise run fmt-check`
  のみであり、実装 fixture の fresh run は対象外 — 台帳は
  「fixture 設定済み・本 slice 未実走」と「構成済み
  テストなし=NotRun」を区別した。
- **graph-router は blocked**: `doc-driven-dev-graph`
  router が repository 全体の pre-existing broken `defers`
  relation で blocked を返した。これは main の既存状態
  であり本 slice の欠陥ではない — 範囲外の修復は行わず、
  plan/IR/台帳に記録した。
- **#10 Project Model が作業中に統合**: `git merge
  origin/main` で PR #398（synvelis#10 Project Model docs
  chain）を取り込み、spec/design の relations と「#10
  実施中」記述を更新した。
- **`impl-doc` script の filename 導出**: `new_impl_record.js`
  /`new_experiment_log.js` が title から `project-asset-
  persistence-synvelis-17.*` を生成したため、task 指定の
  `project-persistence.md` へ rename/手動作成で揃えた
  （`.jsonl` は空 placeholder のため削除）。

## 結論

採用: docs/evidence slice として進める。PAP architecture
の契約を新規意味論なしに写像し、Issue #17 受入条件 13
項目を `SV17-C01`〜`SV17-C13` で fixture 被覆・先行実測・
NotRun/Deferred へ分類した `evidence/sv17/case-ledger.md`
を正本とする。`docs/impl/ir/project-persistence.md` に
実装記録・検証結果を記録する。

棄却した案:

- **PAP architecture 本文への契約追記**: PAP architecture
  は詳細設計正本として既に大規模であり、横断契約の
  「どこに実装済みか」の写像は別文書（design Contract
  Map + case-ledger）が適切。本文変更は既存 `refines`/
  `derives-from` 関係を持つ多数文書の前提を揺らがせる。
- **新規 port/trait の導入**: 既存の `ProjectStore`/
  `ProjectController`/`CommitIdentity`/`MediaReferenceProbe`
  等が契約を既にコード化しており、重複抽象化は YAGNI/DRY
  に反する。
- **Ruviform への CAS/HEAD 意味論移譲**: `syn-io-01` 境界
  spec で HEAD CAS は Synvelis 責務と確定済み。Ruviform
  は generic single-file publish のみを提供する。
- **全 fixture の本 slice での再実行**: 実装 crate の検証は
  対象外（正規検証は `mise run fmt-check`）。先行実測を
  SHA 付きで参照し、未実行 fixture は正直に NotRun とする。
