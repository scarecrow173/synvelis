---
id: "IMPL-034WVJPwDQqM213ZktQskq"
type: "impl"
status: "completed"
title: "SYN-IO-02 HEAD CAS 実 2-process E2E 検証記録 (synvelis#330)"
created: "2026-11-13"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/330"
  changes:
    added:
      - type: "file"
        path: "docs/specs/syn-io-02-head-cas-e2e.md"
      - type: "file"
        path: "docs/designs/syn-io-02-head-cas-e2e-design.md"
      - type: "file"
        path: "docs/plans/syn-io-02-head-cas-e2e-plan.md"
      - type: "file"
        path: "docs/tasks/syn-io-02-head-cas-e2e-task.md"
      - type: "file"
        path: "docs/test-specs/syn-io-02-head-cas-e2e.md"
      - type: "file"
        path: "docs/impl/ir/syn-io-02-head-cas-e2e.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/tests/head_cas.rs"
      - type: "file"
        path: "docs/designs/syn-io-02-head-cas-design.md"
      - type: "file"
        path: "docs/test-specs/syn-io-02-head-cas.md"
      - type: "file"
        path: "docs/specs/syn-io-02-head-cas.md"
      - type: "file"
        path: "docs/plans/syn-io-02-head-cas-plan.md"
      - type: "file"
        path: "docs/tasks/syn-io-02-head-cas-task.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/syn-io-02-head-cas-e2e-task.md"
    - "docs/plans/syn-io-02-head-cas-e2e-plan.md"
    - "docs/designs/syn-io-02-head-cas-e2e-design.md"
    - "docs/specs/syn-io-02-head-cas-e2e.md"
  implemented-by: []
  depends-on:
    - "docs/designs/syn-io-02-head-cas-e2e-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "IR-SV330-syn-io-02-head-cas"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/syn-io-02-head-cas-e2e-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/syn-io-02-head-cas-e2e.md"
    - "TSPEC-034WVJHbBO657gxk3KnLP1"
  verified-by:
    - "docs/test-specs/syn-io-02-head-cas-e2e.md"
  references:
    - "crates/synvelis-project/tests/head_cas.rs"
    - "crates/synvelis-project/src/store.rs"
    - "crates/synvelis-library/tests/import.rs"
  defers:
    - "macOS/Linux での実 2-process/HEAD.lock 実走 — portable code のため CI 実走見込み、merge 後の対象 SHA の CI 証拠で確認（NotRun）"
    - "電源断・実 durability fault・stale break の process 間実測 — 外部 gate（NotRun）"
    - "非協調 writer への強制排他・kernel file lock 化 — ruviform 境界上の別 Issue 事項"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SYN-IO-02 HEAD CAS 実 2-process E2E 検証記録 (synvelis#330)

## Summary

Issue #330 の残件 slice: PR #342（merge `d653062`）で統合済みの
`ProjectStore::save_cas` / `HeadCasError` / `DirectoryBundleStore`
`HEAD.lock` advisory mutex CAS について、**実 OS process をまたいだ**
協調 writer の検証を追加した。`crates/synvelis-project/tests/
head_cas.rs` に test binary self re-exec 方式の 2-process テスト
`two_process_save_cas_single_winner_typed_conflict` を追加し、
Windows 実機で exactly one winner + loser typed `Conflict` +
最終 HEAD=勝者 + 敗者 snapshot 非 publish + `HEAD.lock` 非残滓を
複数 round で pin した。`src/` 変更・新規 dependency なし。

## Context

- Issue: https://github.com/scarecrow173/synvelis/issues/330
- Branch: `feat/330-head-cas-e2e`
- Base: `origin/main` `82d27acb78ef93f7eeb607b839b6a20abb6ab012`
  （検証前に `git fetch origin main && git merge origin/main`
  で取り込み済み — Already up to date）
- 上流 doc chain（本 slice で新規作成）:
  `docs/specs/syn-io-02-head-cas-e2e.md` /
  `docs/designs/syn-io-02-head-cas-e2e-design.md` /
  `docs/plans/syn-io-02-head-cas-e2e-plan.md` /
  `docs/tasks/syn-io-02-head-cas-e2e-task.md` /
  `docs/test-specs/syn-io-02-head-cas-e2e.md`。
- 親 chain `syn-io-02-head-cas` の残件記述（design Verification
  Notes・test-spec Non-goals・spec/plan/task の NotRun 記述）を
  実測結果へ整合済み。

## Implementation

### Changes

- `crates/synvelis-project/tests/head_cas.rs` — 既存 8 contract
  test に加え、実 2-process 協調テストを追加（test-spec
  `syn-io-02-head-cas-e2e` Rules 1–4 対応）:
  - 親 test `two_process_save_cas_single_winner_typed_conflict`:
    bundle root を `workdir`/`BundleGuard` で用意し、3 round
    にわたり各 round で `probe.head()` の現在 HEAD を expected
    として 2 つの子 process を spawn。go-file barrier で同時
    開始を合わせ、結果を marker protocol で検証する。
  - worker 分岐: env guard `SYNVELIS_HEAD_CAS_E2E_*` が設定
    された再実行 process は `e2e_worker()` へ分岐し、go-file
    出現を timeout 付きで待って `DirectoryBundleStore::create`
    → `save_cas` を 1 回実行、`HEAD_CAS_CHILD_RESULT
    ok|conflict|error` marker を stdout に出力。
  - spawn は `std::env::current_exe()` + test fn 名 +
    `--exact --nocapture`（`synvelis-library/tests/import.rs`
    の self re-exec 先例と同一 pattern）。新規 dependency・
    OS 固有 API・`#[cfg]` 分岐なし。
  - 検証: exactly one `Ok(snapshot_name)` + one
    `Conflict{expected, actual}`（expected=seed HEAD、actual=
    勝者名）。最終 HEAD=勝者 snapshot、勝者 snapshot bytes
    一致、敗者 snapshot 非存在、`HEAD.lock`/`HEAD.lock.stale-*`
    残滓なし。marker 欠落・非 0 終了・`error` marker・
    timeout は全て test failure（typed outcome のみ pass）。
- `docs/designs/syn-io-02-head-cas-design.md` — Verification
  Notes の実 2-process 項目を Windows 実測済みへ更新。
- `docs/test-specs/syn-io-02-head-cas.md` — Non-goals を
  後続 slice カバー済みの記述へ整合。
- `docs/specs|plans|tasks/syn-io-02-head-cas*.md` — 実 2-process
  NotRun 記述に後続 slice 実測済みの注記を追加（macOS/Linux・
  電源断は引き続き外部 gate）。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  syn-io-02-head-cas-e2e*.md` — 本 chain 6 file。
- `docs/*/README.md` — doc-maintenance `index apply` で再生成。

### Decisions During Implementation

- **self re-exec（test binary 再実行）を採用**: 専用 fixture
  bin 追加は crate 構成変更を伴い、shell orchestrator は
  `mise run test` の正規経路から外れるため棄却
  （design「検討した代替案」どおり）。`import.rs` 先例の
  env guard + `--exact` filter 方式で追加物を test code のみに
  留めた。
- **marker protocol は stdout prefix 行**: `--nocapture` で
  libtest capture を回避し、`HEAD_CAS_CHILD_RESULT ` prefix
  で他出力と区別。typed outcome 以外（`error`・marker 欠落・
  非 0 終了）は全て panic = test failure とし、Store error を
  Conflict と誤分類しない。
- **go-file barrier は best-effort**: 「同時 critical section
  突入」の保証ではないが、`HEAD.lock` 直列化により結果
  （exactly one winner）は timing 非依存で成立するため十分。
- **round 反復**: round 1 は `expected=None`（absent assert）、
  以降は前 round の勝者 HEAD を expected とする moved-head
  assert を非空 store 上でも exercise。revision は round・子
  ごとに一意採番し snapshot 名衝突を回避。
- **timeout は failure 変換**: 子の go-file 待機・親の子待機
  とも 60s 上限。hang は無期限 wait ではなく test failure。

## Related Experiments

### Adopted

- なし（experiment log なし — `metadata.experiments.adopted`
  は空、`docs/impl/exp/` に関連 experiment なし）。

### Rejected

- なし。

## Validation

実行環境: Windows 11（MINGW64_NT-10.0-26300）、rustc/cargo
1.99.1（mise rust 1.99.1）。検証前に `git fetch origin main
&& git merge origin/main` 実施（Already up to date — base
`82d27ac` が最新 main）。以下の canonical コマンドは全て
exit 0 で pass。

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | pass（exit 0） |
| `mise run check synvelis-project` | pass（exit 0） |
| `mise run lint synvelis-project` | pass（exit 0） |
| `mise run test synvelis-project` | pass（exit 0・全 suite green、`head_cas.rs` 9 tests 含む） |

補助証拠（canonical = mise）: `cargo test -p synvelis-project
--test head_cas` pass・9 tests（`two_process_save_cas_
single_winner_typed_conflict` を含む）。同 test を個別に
5 回連続実行して flake なしを確認。

実測で確認した保証境界（Windows/NTFS）:

- `HEAD.lock` の `CreateNew` acquire・token 照合 release が
  **process をまたいで**機能し、協調する `save_cas` caller
  間で exactly one winner + loser typed `Conflict` が成立。
- conflict は publish 前判定のため敗者の divergent snapshot
  は残らず、`HEAD.lock` 残滓もない。
- portable code（OS 固有 API・`#[cfg]` なし）のため CI で
  各 OS 実走見込み — macOS/Linux は本 slice では NotRun
  （CI 実走または merge 後の対象 SHA の CI 証拠で確認）。

## Risks

以下は本 slice で pin していない領域・残存リスクの正直記録。

- macOS/Linux での `HEAD.lock`/2-process 挙動 — NotRun
  （Windows 実測のみ。portable code のため CI 実走見込みだが
  merge 後の対象 SHA の CI 証拠が統合検証）。
- 電源断・実 durability fault — 外部 gate（ruviform-io-file
  責務、親 chain と同じ委譲先）。
- 非協調 writer（`save` 直接・外部 HEAD 書換え）への強制
  排他 — advisory mutex の設計限界。次回 CAS の conflict
  で観測される契約まで。
- `HEAD_LOCK_STALE_AFTER`（20s）超過 hold の residual race —
  post-publish fencing で誤成功は塞ぐが物理的余地は残存。
  stale break の process 間実測も外部 gate。
- FS 依存の限界（network FS・case 規則差・dirsync 非対応
  FS での等価性）— 親 design 記載どおり未実測。

## Follow-ups

- macOS/Linux CI 実走結果の確認 — merge 後の対象 SHA の CI
  証拠（Coordinator が統合検証で確認）。
- 電源断・実機 durability fault・stale break の process 間
  実測 — 外部 gate として NotRun 記録済み。必要なら別 Issue。
- kernel file lock 化・非協調 writer 排他 — ruviform 境界上
  の別 Issue 事項（親 design Deferred Design Concerns）。
