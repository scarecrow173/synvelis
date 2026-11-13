---
id: "IMPL-034WUav4G9Bn61vUaDoKv9"
type: "impl"
status: "completed"
title: "SYN-IO-02 OS-L1 evidence照合・残ゲート記録 (synvelis#84)"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added:
      - type: "file"
        path: "evidence/reports/syn-io-02-os-l1-evidence.md"
      - type: "file"
        path: "docs/impl/ir/syn-io-02-os-l1-evidence.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/syn-io-02-store-stagedfile-migration.md"
    - "docs/impl/ir/syn-io-02-head-cas.md"
    - "docs/test-specs/syn-io-02-store-publish-contract.md"
    - "docs/test-specs/syn-io-02-head-cas.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/syn-io-02-store-publish-contract.md"
    - "docs/test-specs/syn-io-02-head-cas.md"
  verified-by: []
  references:
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
    - ".github/workflows/ci.yml"
    - "tools/ci-summary.mjs"
  defers:
    - "対象 SHA c0b000b3 の ci.yml run（NotRun pending — 次回 daily-ci/手動 dispatch）"
    - "Windows NTFS 実機 Case・物理電断・Network/Cloud FS（Required/Deferred・NotRun）"
    - "Ruviform #305/#294/#306・Synvelis #28/#294・AudioLens #143 の統合・受入関係事項"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SYN-IO-02 OS-L1 evidence照合・残ゲート記録 (synvelis#84)

## Summary

synvelis#84 の OS-L1 証拠照合 slice。コード変更なし —
`evidence/reports/syn-io-02-os-l1-evidence.md` に StagedFile/
CAS/Recovery fixture の CI 実走証跡（SHA・Run URL・leg・
test 数）と残ゲートの Required/Deferred/NotRun 分類を記録
した。対象 SHA `c0b000b3`（main HEAD・PR #380 merge）の
ci.yml run は収集時点で未実施のため、祖先 SHA `7c206289`
の covering run 43428177865（全 14 job success）を、差分が
`synvelis-project/src` と syn-io-02 fixture にゼロである
ことを `git diff` で確認した上で nearest-SHA 証拠として採用。
本 slice は証拠記録のみのため doc chain は本 IR を正とし、
実装済みの spec/design/plan/task/test-spec/既存 IR を再利用
する（重複する完全チェーンは作成しない）。

## Context

Implements なし（証拠記録 slice — 既存 task は実装 slice
`docs/tasks/syn-io-02-task1-store-stagedfile-migration.md`・
`docs/tasks/syn-io-02-head-cas-task.md` で完了済み）。
`synvelis#84` の受入条件のうち本 slice 担当分:

- Windows/macOS/Linux 既存 CI の Native Build/Link・対象
  テストと SHA/実行 URL の記録（Linux/macOS で実走した
  通常 FS テストのみを Pass とする）。
- Snapshot/HEAD/CAS/ExpectedRevision/Concurrent Save/
  Crash Simulation/Recovery fixture の実走照合。
- StagedFile Publish と Project Commit の責務境界、
  Ruviform #288/#306・SV #28/#294 の Consumer Golden 関係の
  記録。
- Network/Cloud/特異 FS・物理電断・実 GUI/Device/Plugin は
  Deferred の正直な分類。

実装状態は `crates/synvelis-project/src/store.rs`
（`ruviform_io_file::StagedFile` publish・`save_cas`・
`HEAD.lock` mutex）と `tests/{store_publish,head_cas,
controller_store}.rs` を読み取り確認 — PR #178（StagedFile
移行）・PR #342（HEAD CAS、Issue #330）で main 統合済み。
`Cargo.lock` ruviform 解決 rev `fd1d4d5f…`。

## Implementation

### Changes

- `evidence/reports/syn-io-02-os-l1-evidence.md`（新規）—
  anchor（対象 SHA・実装 merge・ruviform rev）・CI run 選定
  （対象 SHA run 不在の正直な記録と nearest-SHA 採用根拠）・
  leg 別 fixture 結果（ubuntu/windows/macos/msrv で
  head_cas 8・store_publish 16・controller_store 11 全て
  `ok`、rt-verify は `rt_` filter で 0 selected）・
  fixture↔test-spec Rule↔実走 leg の照合表・責務境界・
  Consumer Golden 関係・残ゲート分類・成立/不成立の明記。
- `docs/impl/ir/syn-io-02-os-l1-evidence.md`（本書）。
- `docs/impl/ir/README.md` — `sync_impl_index.js --kind ir
  --apply` で再生成（managed index・手編集なし）。

### Decisions During Implementation

- **nearest-SHA 証拠の採用条件を明示**: 対象 SHA `c0b000b3`
  の ci.yml run が無い（merge 13:37Z・収集 13:41Z・次回
  daily-ci cron 21:29 UTC 未到来）ため、祖先 `7c206289` の
  run 43428177865 を採用。ただし `git diff --stat
  7c206289..c0b000b3` で差分が PR #379（pre-commit 設定）と
  PR #380（VCS-06 test/docs/evidence）のみ、`synvelis-
  project/src` と syn-io-02 fixture 3 file にゼロである
  ことを確認してから適用 — 「同じ code が走った」ことを
  差分照合で示し、run が c0b000b3 のものでないことも併記。
- **CI leg を OS-L1 Pass の根拠に限定**: ubuntu-latest・
  macos-latest leg の通常 FS（`std::env::temp_dir` 実
  ファイル）での `test result: ok` のみを Pass とし、
  windows-latest leg は「CI 証拠」として区別 — GitHub
  hosted runner は Windows NTFS 実機 Case（Issue #84
  「まず Windows 実機」条件）の代替にしない。rt-verify は
  fixture が `rt_` 契約外で 0 selected のため RT 証拠に
  計上しない。
- **「Orphan の禁止」の契約を正確に記述**: conflict は
  publish 前判定で orphan を生成しないが、crash window の
  orphan snapshot は recovery データとして保持・Satisfied
  再 save で再採用する — test-spec R1/R8 の設計どおりで
  あり、「orphan を残さない」とは記録しない。
- **doc chain は IR 集約**: spec/design/plan/task/test-spec
  は実装 slice で作成済みのため重複作成せず、本 slice の
  記録は IR + evidence report に集約（共通契約の
  「IR を正とし重複時は IR に集約」に従う）。
- **StagedFile の共有 consumer を明記**: `store.rs` 以外に
  `apply/journal.rs`・`plugin_state.rs`・`restore/mod.rs` も
  同一 `ruviform-io-file` API を使用 — publish 基盤は共有、
  Project Commit CAS は `store.rs` 固有として境界を記述。

## Related Experiments

### Adopted

- なし（experiment log なし — `metadata.experiments.adopted`
  は空、`docs/impl/exp/` に関連 experiment なし）。

### Rejected

- なし。

## Validation

- `gh run list`/`gh run view --log`/`gh api` による CI 証拠
  実測（2026-11-13T13:41Z）— run 43428177865 の 14 job 全
  success、leg 別 `test result` 行を binary 単位で転記。
- `git fetch origin main && git rev-parse origin/main` —
  対象 SHA `c0b000b3` が main HEAD であることを再確認。
- `git merge-base --is-ancestor 7c206289 c0b000b3` および
  `git diff --stat` — covering run の祖先関係と関連 code
  無差分を確認。
- `mise run fmt-check` — docs/証拠のみの変更に対する
  canonical 軽量検証（結果は PR 本文に記録）。
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js
  --json` — IR front matter/section 監査。

NotRun / Unsupported（証拠記録の範囲どおり）:

- 対象 SHA `c0b000b3` 自体の ci.yml run — NotRun
  （pending・次回 dispatch 待ち）。
- Windows NTFS 実機 Case・物理電断・Network/Cloud FS・
  実 GUI/Device/Plugin・AudioLens consumer — NotRun
  （evidence report の残ゲート表どおり）。
- `*-full`/`verify-full`/`*-msrv` ローカル必須検証 — 本
  slice は証拠記録のみでコード変更なしのため対象外
  （タスク指定どおり）。msrv-verify leg の CI 証拠は上記
  表に記録済み。

## Risks

- nearest-SHA 証拠は「run SHA と対象 SHA で関連 code が
  byte 同一」の照合に基づく — `c0b000b3` 自体の run が
  後続で fail した場合、本書の Pass は遡って無効になる
  （daily-ci/手動 dispatch の結果で更新要）。
- run 43428177865 は workflow_dispatch の手動 run であり、
  daily-ci の変更検知経路とは別トリガ — 統合受入の正式
  証拠は対象 SHA の日次/手動 CI 結果で最終確認が必要
  （POLICIES §5: ローカル/中間証拠を統合受入と混同しない）。
- Windows leg の Pass は GitHub-hosted windows-latest
  runner 上のもので、Issue #84 の「まず Windows 実機」
  条件（実 FS 状態遷移・Crash→Recovery）を満たさない —
  実機 lane 未整備の間は当該機能の最終受入は保留。

## Follow-ups

- `c0b000b3`（またはその後の main HEAD）の ci.yml run が
  実走次第、本書へ SHA/Run URL を追記して nearest-SHA
  証拠を exact-SHA 証拠へ置き換える — Coordinator/日次
  CI 運用に委譲。
- Windows NTFS 実機 Case（Snapshot CreateNew→HEAD CAS/
  Crash→Recovery・Failed/Indeterminate の HEAD 誤進行
  なし）の実機 lane — Issue #84 残件として別 slice。
- 物理電断・Network/Cloud FS・Ruviform #305/#294・SV
  #28/#294・RF #306・AL #143 の統合・受入関係事項 —
  Issue #84 open 継続・Coordinator 判断。
- `docs/impl/ir/README.md` index は `sync_impl_index.js
  --kind ir --apply` で本 slice 内に再生成済み。
