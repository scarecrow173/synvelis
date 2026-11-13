---
id: "SPEC-034WVJji6Ejtn5EnEV5RkC"
type: "spec"
status: "approved"
title: "SYN-IO-02 HEAD CAS 実 2-process E2E 検証と保証境界確定 (synvelis#330)"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/330"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "SPEC-034T7fQpLmNvRwXk2jHs9"
    - "DESIGN-034Tk3mXw7LpQ2vRnB8y"
    - "TSPEC-034Th7fQpLmNvRwXk2jHs"
    - "IR-SV330-syn-io-02-head-cas"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/330"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-project/src/store.rs"
    - "crates/synvelis-project/tests/head_cas.rs"
    - "crates/synvelis-library/tests/import.rs"
    - "POLICIES.md"
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS 実 2-process E2E 検証と保証境界確定 (synvelis#330)

## What Should Be Built

PR #342（merge `d653062`）で統合済みの `ProjectStore::save_cas` /
`HeadCasError` / `DirectoryBundleStore` `HEAD.lock` advisory mutex
CAS の残件 slice — Issue #330 の未達受入条件のうちローカルで
検証可能な分を消化する test + docs slice。`src/` 変更を伴わず、
`save_cas` API・契約・`HEAD.lock` 機構は変更しない。

1. **実 2-process 再現テスト**: `crates/synvelis-project/tests/`
   に、test binary を self re-exec した 2 つの実 OS process が
   同一 bundle root へ同時 `save_cas` する協調テストを追加する。
   `mise run test synvelis-project` で自動実行可能とする。
2. **typed error の実 2-process 検証**: 敗者 process が
   `HeadCasError::Conflict`（成功誤報なし）を受け取ることを
   実 process 経路で観測する。
3. **保証境界の確定記録**: rename atomicity・FS 依存の限界・
   advisory mutex の範囲について、実測で確認した範囲と未検証・
   設計上の限界を区別して関連 docs に記録する。

## Why Now

Issue #330 の受入条件のうち PR #342 で未達だった残件:

- 「複数 process からの同時 save で lost-update しないことを
  再現テスト（実 2-process 協調）で検証」— PR #342 は同一
  process 内 thread 協調（`tests/head_cas.rs` Rule 2）までを
  pin し、実 2-process は外部 gate として NotRun のまま。
- 「競合時に caller が typed error（成功誤報なし）を受け取る
  こと」— 同上、実 process 経路での観測が残っていた。
- 「HEAD CAS の保証境界の文書化」— `syn-io-02-head-cas-design`
  に保証境界節はあるが、実 2-process 実測に基づく確定記録が
  ない。

## Users and Value

- Issue #330 の受入担当者・Coordinator: 実 process 協調での
  single-winner + typed conflict の実測証拠（OS-L1）を得る。
- 将来の複数 writer 構成の設計者: 保証境界（どこまでが契約で
  どこからが FS 依存・協調前提か）が実測付きで確定する。

## Scope

### In Scope

- `crates/synvelis-project/tests/head_cas.rs` への実 2-process
  協調テスト追加（self re-exec 方式・新規 dependency なし）。
- 本 doc chain（`syn-io-02-head-cas-e2e`）6 file
  （spec/design/plan/task/test-spec/impl-ir）。
- 既存 `syn-io-02-head-cas` design の Verification Notes と
  test-spec の Non-goals の実測整合（追記・更新）。
- docs index の doc-maintenance 再生成。

### Out of Scope

- `crates/*/src/` の変更 — `save_cas` 契約・`HeadCasError`・
  `HEAD.lock` 機構は PR #342 で確定済みであり、本 slice は
  その検証と文書化のみ（契約再設計は対象外）。
- `synvelis-recording` staging writer・他 Consumer 固有コード、
  `Cargo.lock`（依存追加なし）。
- macOS/Linux での手動実機レーン・電源断・実 durability
  fault — CI 実走または NotRun 記録（POLICIES.md の
  OS-L1/外部 gate 方針）。
- 非協調 writer への強制排他・kernel file lock 化 — 親
  design の Deferred Design Concerns どおり対象外。

## Requirements

- 2-process テストは `cargo test` / `mise run test
  synvelis-project` で自動実行され、外部バイナリ・手動手順・
  opt-in feature を必要としない（test binary の self re-exec）。
- 同時 `save_cas` の結果は exactly one winner — 勝者は `Ok`、
  敗者は `HeadCasError::Conflict { expected, actual }` で、
  最終 HEAD は勝者の snapshot、敗者の snapshot は publish
  されず、`HEAD.lock` 残滓がない。
- 実 2-process 経路で `HEAD.lock` が process 間で機能する
  こと（敗者が勝者の commit を lock 内で観測して conflict
  する）を assert する。
- テストは対象 OS 全てで portable に動作する — OS 固有 API・
  `#[cfg]` 分岐に依存しない。
- 保証境界の記述は「実測で確認したこと」と「設計上の限界・
  未検証事項」を区別して記録する。

## Acceptance Criteria

- [ ] 実 2-process 同時 `save_cas` テストが追加され、
  `mise run test synvelis-project` で pass する。
- [ ] 敗者 process が typed `HeadCasError::Conflict` を受け取り、
  成功誤報がないことが実 2-process 経路で検証される。
- [ ] HEAD CAS の保証境界（rename atomicity・FS 依存の限界・
  advisory mutex の範囲）が関連 docs に記録される。
- [ ] `mise run fmt-check` / `check` / `lint` / `test`
  （synvelis-project）を canonical 検証として実行し結果を
  impl-ir に記録する。macOS/Linux は CI 実走または NotRun
  として記録する。

## Deferred Design Concerns

- 電源断・実 durability fault・非協調 writer への強制排他 —
  外部 gate / 親 design Deferred のまま。
- macOS/Linux での CI 実走結果の確認 — merge 後の対象 SHA
  の CI 証拠（Coordinator が統合検証で確認）。

## Sources and Relations

- synvelis#330（本 Issue。PR #342 `d653062` で契約・実装・
  同一 process 契約テストは統合済み、本 slice は残件）。
- `docs/specs/syn-io-02-head-cas.md` /
  `docs/designs/syn-io-02-head-cas-design.md` /
  `docs/test-specs/syn-io-02-head-cas.md` /
  `docs/impl/ir/syn-io-02-head-cas.md`（親 chain）。
- `crates/synvelis-project/src/store.rs`（`save_cas`・
  `HeadCasError`・`HEAD.lock` 実装の正本）。
- `crates/synvelis-project/tests/head_cas.rs`（同一 process
  thread 協調の先行契約テスト — Rule 2 が本 slice の土台）。
- 先例: `crates/synvelis-library/tests/import.rs` の
  `subprocess_crash_during_publish_or_commit_resumes_across_processes`
  （test binary self re-exec による実 process 検証 pattern）。
