---
id: "EXP-sv84-stagedfile-migration"
type: "experiment"
status: "completed"
title: "Issue #84 StagedFile Migration — 実装済み状態の確認と証拠収集方針"
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
  implements: []
  implemented-by:
    - "docs/impl/ir/stagedfile-migration.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/stagedfile-migration-design.md"
    - "docs/tasks/stagedfile-migration-task.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv84/case-ledger.md"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
  defers: []
  deferred-by: []
---

# Issue #84 StagedFile Migration — 実装済み状態の確認と証拠収集方針

## 仮説

Issue #84 の dispatch 時点で、`DirectoryBundleStore` の
StagedFile 移行と HEAD CAS は先行 slice（PR #178・#342・
#382）により main 統合済みであり、本 task の実作業は
コード変更ではなく doc chain・証拠台帳・検証実行に限定
されるのではないか — `store.rs` の現状と git 履歴で確認
する。

## 方法

- `git log`/`git show` で `store.rs` の変更履歴を追い、
  `write_atomic` 撤去と `publish_staged`/`save_cas`/
  `HEAD.lock` の導入 commit（`b527e0e`・`d653062`・
  `285e1c4`）を特定した。
- `store.rs` 現行コードを読み、snapshot `CreateNew`・
  HEAD `Replace`・`FileAndDirectorySync`・
  `snapshot_collision`・`HEAD.lock` CAS・fencing が
  Issue #84 の契約どおり実装済みであることを確認した。
- 既存 fixture（`store_publish.rs` 16・`head_cas.rs` 9・
  `controller_store.rs` 11）と test-spec Rule の対応を
  監査した。
- CI 証拠は `gh run list`/`gh run view --job --log` で
  直近 covering run を特定し、対象 fixture binary の
  `test result` 行を leg 別に収集した。
- nearest-SHA 証拠の採用可否は `git diff --stat
  <run-sha>..<main-sha>` で byte 同一範囲を確認して判定
  した。

## 観測結果

- **実装は統合済み**: `store.rs` は `ruviform_io_file` の
  `StagedFile`/`PublishFailure` のみを使用し、独自
  `.tmp`→`sync_all`→`rename` は残っていない。HEAD CAS・
  stale-break・fencing・2-process e2e も main に存在する。
- **fixture 網羅性**: test-spec Rule 1–8 に対応する test
  は全て既存 — 書戻し禁止・二重 Publish single winner・
  orphan・crash window 模擬（stale lock・temp 残滓・外部
  HEAD 書換え・再 Open）・CAS Conflict・旧 Writer 上書き
  禁止を pin。追加 fixture は不要と判断（gap なし）。
- **CI 証拠**: 対象コード SHA（dispatch 時点
  `e0fbbe8`、merge 後 `origin/main` `1947623` —
  両者でコード同一）に対する ci.yml run は未実施
  （`gh run list --commit` 両 SHA で 0 件・
  dispatch-only）。直近 covering run `43457068626`
  （head_sha `285e1c45`、全 14 job success）で
  `git diff --stat 285e1c45..origin/main` により
  `store.rs`・3 fixture・`Cargo.toml`・`Cargo.lock` が
  byte 同一であることを確認 — ubuntu/windows/macos/
  msrv leg で `store_publish` 16/16・`head_cas` 9/9・
  `controller_store` 11/11 が全て ok。rt-verify は
  `rt_` filter で本 fixture は選択外（0 selected・
  期待どおり）。
- **判断**: nearest-SHA 証拠として採用し、対象 SHA 自体は
  NotRun pending として分離記録する方針を採用（前例:
  `syn-io-02-os-l1-evidence` IR と同じ扱い）。

## 結論

採用: 実装変更なし・証拠/文書 slice として進める。
`docs/impl/ir/stagedfile-migration.md` に検証結果を記録し、
`evidence/sv84/case-ledger.md` に Case ID 別の
Expected/Observed/NotRun を台帳化する。
