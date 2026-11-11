---
id: "TASK-alib07-t1"
type: "task"
status: "done"
title: "ALIB-07 Task 1 — fault case 追加・webdav fixture 抽出・証拠 matrix"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-07-fault-matrix.md"
  depends-on:
    - "docs/designs/alib-07-fault-matrix-design.md"
  verifies:
    - "docs/test-specs/alib-07-fault-matrix.md"
---

# ALIB-07 Tasks

## Task 1 — webdav fixture 抽出

- [x] `tests/common/webdav_fixture.rs` 新設（MockServer/
      MockRequest/MockResponse/Handler + DAV builder 移動、
      `MockResponse::raw` 追加）
- [x] `tests/common/mod.rs` に module 登録
- [x] `tests/webdav.rs` を fixture import へ置換（既存 33 test
      無変更で green）

## Task 2 — provider fault case

- [x] `tests/git.rs`: offline remote（refresh typed・mirror
      正直な stale・未取得 pin typed・LFS NotFound）、
      remote rename + pin/fixed-commit 不変、hostile
      locator/URL fail-closed
- [x] `tests/webdav.rs`: offline typed、auth revoke、
      malformed head/body/chunked、identity encoding、
      same/cross-origin redirect credential 境界

## Task 3 — import fault case

- [x] `tests/import.rs`: stage 相 cancel → typed + 同一
      ImportId resume、cross-provider 同一 bytes の blob 共有

## Task 4 — 証拠 matrix + docs チェーン

- [x] `docs/test-specs/alib-07-fault-matrix.md` に Case ID
      別 Expected/Observed/Status matrix
- [x] spec/design/plan/task/ir docs 作成
- [x] `doc_maintenance.js apply` で index 再生成、
      `doc_status.js lint/audit` で新規 finding なし確認
      （alib-06 と同一の既存 category のみ）

## Task 5 — 検証 lane

- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `test-msrv` 全て exit 0
- [x] commit → push → PR（`Refs #91`、close しない。PR #236・
      slice-2 PR #256 ともに merge 済み。Issue は残受入行
      （real-env lane・cross-gate・deferred・#89/#114 所有分）
      のため open 維持）
