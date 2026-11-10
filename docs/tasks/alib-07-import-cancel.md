---
id: "TASK-alib07-import-cancel"
type: "task"
status: "done"
title: "ALIB-07 slice 2 Task 1 — cancel checkpoint・fault case・crash fixture・matrix 簿記"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-07-import-cancel.md"
  depends-on:
    - "docs/designs/alib-07-import-cancel-design.md"
  verifies:
    - "docs/test-specs/alib-07-fault-matrix.md"
---

# ALIB-07 slice 2 Tasks

## Task 1 — docs チェーン

- [x] spec/design/plan/task 作成（cancel 契約の決定を spec に記録）

## Task 2 — pre-commit checkpoint + cancel case

- [x] `src/import.rs`: `import_verified` pre-commit 区間に
      `cx.check_interrupts()?` 追加（`apply` 以降に checkpoint なし）
- [x] `tests/import.rs`: publish 相 cancel（typed + Verified 保持 +
      resume）、commit 相 cancel（typed + Published 保持 + blob 温存
      + 再 publish なし resume）、verify 相 cancel（EOF token 注入で
      Staged 保持 + resume）

## Task 3 — subprocess crash fixture（LIB-IMPORT-08）

- [x] self-exec 子 process で `ack-lost`（publish 後 receipt 前
      exit）と `pre-commit`（commit 前 re-stat で exit）の 2 crash
      point を再現
- [x] 親 process が manifest + on-disk blob のみから resume し、
      ID 予約一致・blob 再利用/skip・commit 収束を assert

## Task 4 — matrix 簿記 + IR + index

- [x] `docs/test-specs/alib-07-fault-matrix.md`: baseline SHA・
      PR #236/#253・cross-link・LIB-IMPORT-04/08・LIB-MIG-02 行更新
- [x] `docs/impl/ir/alib-07-import-cancel.md` 作成
- [x] `doc_maintenance.js apply` で index 再生成、
      `doc_status.js lint/audit` で新規 finding なし確認
      （残存は id-format migration・非 blocking reciprocal のみ、
      既存 alib docs と同一 category）

## Task 5 — 検証 lane

- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `test-msrv` 全て exit 0（`test-msrv` 初回は
      無関係の `webdav::status_401_maps_auth_expired` が flake
      失敗、単独・file 単位・lane 再実行で green を確認）
- [x] commit → push → PR（`Refs #91`、close しない）
