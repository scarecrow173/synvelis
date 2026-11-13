---
id: "IMPL-034X5H2u7vD5H929MaHV8N"
type: "impl"
status: "completed"
title: "Issue #84 StagedFile Migration — 証拠統合・受入検証 slice 実装記録"
created: "2026-11-13"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added:
      - type: "file"
        path: "evidence/sv84/case-ledger.md"
      - type: "file"
        path: "docs/specs/stagedfile-migration.md"
      - type: "file"
        path: "docs/designs/stagedfile-migration-design.md"
      - type: "file"
        path: "docs/plans/stagedfile-migration.md"
      - type: "file"
        path: "docs/tasks/stagedfile-migration-task.md"
      - type: "file"
        path: "docs/test-specs/stagedfile-migration.md"
      - type: "file"
        path: "docs/impl/exp/stagedfile-migration.md"
      - type: "file"
        path: "docs/impl/ir/stagedfile-migration.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/stagedfile-migration-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/stagedfile-migration-task.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/stagedfile-migration-design.md"
    - "docs/plans/stagedfile-migration.md"
    - "docs/test-specs/stagedfile-migration.md"
    - "evidence/sv84/case-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/stagedfile-migration.md"
  verified-by: []
  references:
    - "docs/impl/exp/stagedfile-migration.md"
    - "evidence/sv84/case-ledger.md"
    - "docs/impl/ir/syn-io-02-store-stagedfile-migration.md"
    - "docs/impl/ir/syn-io-02-head-cas.md"
    - "docs/impl/ir/syn-io-02-os-l1-evidence.md"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/stagedfile-migration.md"
    rejected: []
---

# Issue #84 StagedFile Migration — 証拠統合・受入検証 slice 実装記録

## Summary

Issue #84「DirectoryBundleStore・Snapshot/HEAD 保存を Ruviform
StagedFile へ移行」の Issue-level 集約 slice を実装した。
本 slice のコード差分はゼロ — 実装本体は先行 slice
（PR #178 store 移行・PR #342 HEAD CAS・PR #382 2-process
CAS e2e）により main 統合済みであり、本 slice は
(a) Issue-level の spec/design/plan/task/test-spec 文書
チェーン、(b) `evidence/sv84/case-ledger.md` の Case 台帳、
(c) 実走検証と IR 記録を追加する。監査の結果、fixture の
gap は発見されず、追加 fixture は不要と判断した。

## Context

`docs/tasks/stagedfile-migration-task.md` /
`docs/plans/stagedfile-migration.md` /
`docs/test-specs/stagedfile-migration.md` に従う。
詳細な設計・検証契約は既存 SYN-IO-02 系文書
（`syn-io-02-snapshot-head-staged-publish` spec、
`syn-io-02-directory-bundle-store-stagedfile-design`
design、3 系統の test-spec、3 本の IR）が正本であり、
本 doc chain はそれらを Issue #84 の受入軸に束ねる
集約文書である。責務境界: Ruviform `ruviform-io-file` が
単一 file の staged publish/durability 分類/recovery
handle を所有し、Synvelis `synvelis-project` が
SnapshotEnvelope schema・immutable snapshot 名・HEAD
pointer・ExpectedRevision/CAS・collision policy・Recovery
採用を所有する。**Publish 成功 ≠ Project Commit**。

## Implementation

### Changes

- **コード変更なし**（監査の結果、`store.rs` は既に
  `StagedFile`/`PublishFailure` 経由であり、独自
  `.tmp`→`sync`→`rename` は残存しない）。
- 追加文書: `docs/specs/stagedfile-migration.md`、
  `docs/designs/stagedfile-migration-design.md`、
  `docs/plans/stagedfile-migration.md`、
  `docs/tasks/stagedfile-migration-task.md`、
  `docs/test-specs/stagedfile-migration.md`、
  本 IR、`docs/impl/exp/stagedfile-migration.md`。
- 追加証拠: `evidence/sv84/case-ledger.md` — Rule 1–8 を
  SV84-C01〜C13 に写像し、OS/実行環境別の Expected/
  Observed/NotRun を記録。

### Decisions During Implementation

- **as-built 記録として進める**: dispatch 時点で
  `store.rs`・3 fixture・`Cargo.toml`/`Cargo.lock` が
  先行 slice のまま main に存在したため、再実装せず
  監査・証拠統合に限定した（exp doc の仮説を実測で確認）。
- **nearest-SHA CI 証拠の採用**: dispatch 時点の対象
  コード SHA `e0fbbe8`（当時の `origin/main` HEAD）の
  ci.yml run は未実施（dispatch-only）。その後
  `origin/main` は docs-only 2 commit 進み `1947623` —
  `git diff --stat e0fbbe8..origin/main` でコード差分
  ゼロ、`git diff --stat 285e1c45..origin/main` で
  store.rs・3 fixture・crate manifest・`Cargo.lock` が
  byte 同一であることを確認した。直近 covering run
  `43457068626`（head_sha `285e1c45`）の leg 別
  `test result` 行を fixture 名まで降りて記録し、
  対象コード SHA 自体は NotRun pending として分離。
- **fixture 名の byte 同一範囲**: fixture binary は crate
  全体を link するため、「byte 同一」の主張は
  store.rs + 3 fixture + `crates/synvelis-project/
  Cargo.toml` に限定して記録した（lib.rs の re-export
  差分は code path に影響しないが binary 全体の同一性は
  主張しない）。
- **rt-verify の非計上**: `rt_` filter で syn-io-02
  fixture は選択外（0 selected）のため RT 証拠として
  計上せず、workspace `rt_` contract suite の leg
  success のみ記録した。
- **Windows 実機 = L2 相当の通常 FS**: fixture は
  `std::env::temp_dir`（ローカル NTFS）上の実ファイルで
  実行されており、Project 保存先/通常 FS の状態遷移・
  Readback/再 Open・衝突/失敗復旧を対象機能どおり検証
  した。network/cloud FS・物理電断・実 GUI/Device/
  Plugin は NotRun/Deferred/別 Owner として分類した。
- **doc chain の集約性**: spec/design/plan/task/
  test-spec は詳細を複製せず、SYN-IO-02 系正本への
  関係と Issue #84 受入軸のみを記述した。

## Related Experiments

### Adopted

- `docs/impl/exp/stagedfile-migration.md` — dispatch 時点
  での実装済み状態の確認・fixture 監査・CI 証拠収集方針・
  nearest-SHA 判定の実測記録。採用理由: as-built 判定と
  証拠分類の根拠を後続レビューで再現可能にするため。

### Rejected

- なし。

## Validation

全て `mise.toml` の canonical task 経由で実施
（2026-11-14・Windows 実機・対象コード = `e0fbbe8`、
その後 docs-only merge で `origin/main` `1947623` を
取込済み — コード状態同一）。

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | exit 0・Pass |
| `mise run check synvelis-project` | exit 0・Pass（dev profile Finished） |
| `mise run lint synvelis-project` | exit 0・Pass（警告なし） |
| `mise run test synvelis-project` | exit 0・Pass — 41 test binaries 全て `test result: ok`・FAILED 0。`store_publish` 16/16・`head_cas` 9/9（2-process e2e 含む）・`controller_store` 11/11・Doc-tests 2/2 |

CI（nearest-SHA・run 43457068626・head_sha `285e1c45`・
全 14 job success）leg 別 `test result` 実測:

- ubuntu-latest job `132006742268`: 3 fixture 全 Pass
- windows-latest job `132006670018`: 3 fixture 全 Pass
- macos-latest job `132006666935`: 3 fixture 全 Pass
- msrv-verify job `132006754106`（rust 1.89）: 3 fixture
  全 Pass
- rt-verify job `132006741582`: `rt_` filter で本 fixture
  選択外（0 selected）— RT 証拠としては非計上

対象コード SHA（`e0fbbe8`/`1947623`）の ci.yml run:
**NotRun**（両 commit とも 0 件・dispatch-only CI）。
統合受入は merge 後の対象 SHA CI 証拠に委ねる。

## Risks

- nearest-SHA 証拠は対象コード SHA と byte 同一範囲を
  限定して主張しており、対象コード SHA 自体の CI は
  未実施。統合受入で daily-ci/手動 dispatch の結果を
  確認する必要がある。
- `lib.rs` 等の store 無関係差分により fixture binary
  全体は byte 同一ではない — 影響は re-export 追加のみ
  だが、厳密な binary 同一性は主張していない。
- 物理電断・network/cloud FS・実 GUI/Device/Plugin・
  他 Consumer（AudioLens）受入・Ruviform #305/#306・
  SV #28/#294/#86 は本 slice 範囲外の未検証項目として
  残る。

## Follow-ups

- merge 後の対象 SHA に対する ci.yml run（daily-ci
  21:29 UTC cron または手動 dispatch）の結果確認 —
  Coordinator/統合受入側の責務。
- Ruviform #305 撤去候補との candidate-revision 組合せ
  検証、Ruviform #306・SV #28・#294・#86・AudioLens
  #143 の統合・受入時の関係事項（`evidence/sv84/
  case-ledger.md` の Deferred 表を参照）。
- 本 doc chain の `status` を review-protocol 通過後に
  `completed` 相当へ更新するかは Coordinator の受入
  判断に委ねる。
