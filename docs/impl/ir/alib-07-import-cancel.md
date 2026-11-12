---
id: "IR-alib07-import-cancel"
type: "impl"
status: "completed"
title: "ALIB-07 slice 2 — import cancel checkpoint・subprocess crash fixture 実装記録（synvelis-library）"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-07-import-cancel.md"
    - "docs/designs/alib-07-import-cancel-design.md"
    - "docs/specs/alib-07-import-cancel.md"
  verified-by:
    - "docs/test-specs/alib-07-fault-matrix.md"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/designs/alib-07-import-cancel-design.md"
      - type: "file"
        path: "docs/plans/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/tasks/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/impl/ir/alib-07-import-cancel.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "docs/test-specs/alib-07-fault-matrix.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-07 slice 2 Implementation Record

## Summary

Synvelis #91 の残 headless sub-case（LIB-IMPORT-04 publish/commit 相
cancel、LIB-IMPORT-08 subprocess crash）を `synvelis-library` で
閉じた記録（PR #256 で merge 済み）。先行 slice
（`docs/impl/ir/alib-07-fault-matrix.md`、PR #236）が
「publish/commit 相 cancel の注入点: NotRun」と記録した gap を
解消する。`import_verified` の commit 直前に最後の
`cx.check_interrupts()?` を 1 箇所だけ追加し、`tests/import.rs`
に新規 4 case（verify/publish/commit 相 cancel + subprocess
crash の 2 crash point）を実装した。

## Context

- Issue: <https://github.com/scarecrow173/synvelis/issues/91>
  （ALIB-07。親・設計オーナー: #9。関連: #86 ALIB-02 import）。
- 上流 doc chain: `docs/specs|designs|plans|tasks|
  alib-07-import-cancel*`。検証の正本は
  `docs/test-specs/alib-07-fault-matrix.md` の LIB-IMPORT-04 /
  LIB-IMPORT-08 行。
- 先行 slice: `docs/impl/ir/alib-07-fault-matrix.md`
  （stage 相 cancel までを固定済み。本 slice はその残 gap を
  閉じる）。
- 境界: `crates/synvelis-library` の `src/import.rs` commit 相
  注入点・`tests/import.rs`・docs チェーンのみ。
  `synvelis-project` 他 crate、provider 実装本体、GUI、実
  network/device/OS credential store は対象外。

## Implementation

### 契約判断（spec に記録済み）

publish 完了〜`controller.apply` の間に cooperative cancel を観測
できないのは契約違反と判断し、option (a) を採用: `apply` 直前に
最後の `cx.check_interrupts()?` を 1 箇所だけ置く。`apply` 成功後は
commit が事実なので cancel を返さない（`Cancelled` は実状態を
偽る）。`apply`〜manifest 書込みの crash は既存 `applied_replay`
が reconcile する。

### `crates/synvelis-library/src/import.rs`

`import_verified` の commit 直前（blob re-stat の後・`commit_op` /
`controller.apply` の前）に `cx.check_interrupts()?` を追加。
これ以降に checkpoint は置かない。

### `crates/synvelis-library/tests/import.rs`（新規 4 case）

- `cancel_during_verify_is_typed_and_same_import_resumes`:
  `CancelOnEofProvider`/`CancelAtEof`（provider EOF で共有 token
  を cancel）で verify 相 cancel を注入。manifest `Staged` 保持・
  typed `Cancelled`・同一 `ImportId` resume で 1 回だけ commit。
- `cancel_during_publish_is_typed_and_same_import_resumes`:
  `ScriptedPublisher` の `BlobPublishError::Interrupted(Cancelled)`
  で publish 相 cancel を注入。manifest `Verified` 保持・resume で
  publish→commit が各 1 回。
- `cancel_at_commit_boundary_parks_at_published_and_resumes`:
  `CancelAfterPublish`（実 `StagedBlobPublisher` へ委譲後に token
  を cancel）が publish tail の cancel を再現。manifest
  `Published` 保持・blob 温存・resume は publisher 呼出 0 回で
  commit。
- `subprocess_crash_during_publish_or_commit_resumes_across_processes`:
  self-exec 子 process（`SYNVELIS_LIB_IMPORT_CRASH_DIR`/
  `_MODE` env guard）で 2 crash point を実再現 — `ack-lost`
  （実 publish 後 receipt 返却前に `exit(1)`）と `pre-commit`
  （`StatKillStore` が commit 前 re-stat の stat#2 で `exit(1)`）。
  親 process は manifest + on-disk blob のみから resume し、
  `ack-lost`→`BlobMaterialization::Reused`、`pre-commit`→
  `Created`、子 process の予約 ID がそのまま commit されること、
  project への登録が 1 件だけであることを assert。

## Related Experiments

### Adopted

なし（本 slice は実験を伴わない test/実装作業。
`metadata.experiments` は adopted/rejected とも空で、
`docs/impl/exp/` に関連する実験ログは存在しない）。

### Rejected

なし。

## Validation

- `cargo test -p synvelis-library --test import`（補助証拠、
  canonical = mise）: 21 passed / 0 failed（新規 4 case 含む）。
- `cargo clippy -p synvelis-library --all-targets -- -D warnings`
  （補助証拠）: exit 0。
- `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` / `mise run test-rt` / `mise run test-msrv`:
  canonical 検証 lane。結果は `docs/tasks/alib-07-import-cancel.md`
  Task 5 と PR 本文の検証欄を正本とする。

## Risks

- **pre-commit checkpoint が既存 test を壊す可能性**:
  `check_interrupts` は cancel token を立てない既存 test では
  no-op であり、deadline 未設定のため `BudgetExceeded` も
  発火しない設計。`mise run test` で全件確認済み
  （plan の risk register どおり）。
- **subprocess test の再帰・ハング**: env guard
  （`SYNVELIS_LIB_IMPORT_CRASH_DIR`/`_MODE`）で child 経路を
  分岐し、child は必ず `process::exit` で終端する（fault 注入が
  機能しなかった場合も非 0 終了で fail-closed）。親は
  `output()` で exit status を assert する。
- **child/parent の intent 不一致**: `LocalFolderProvider::pin`
  は内容 digest 由来の `object_identity` を返し process 間で
  決定的。同一 fixture file・同一 locator・同一 ImportId を
  使い、manifest 整合検査（`object_identity`/`asset`）を両側で
  一致させている。
- **subprocess/Mock 成功の非昇格**: subprocess `exit(1)` は
  real process crash の headless 証拠であり、OS レベル
  durability 保証・実 OS crash とは区別する（synvelis#84
  cross-gate）。Mock 結果を実 provider・実 durability の
  Pass に読み替えない。

## Follow-ups

- OS クラッシュ/電源断の実 durability（fsync 実機保証）: NotRun
  （実環境。subprocess `exit(1)` は real process crash の headless
  証拠であり、OS レベル durability 保証とは区別する — synvelis#84
  cross-gate）。
- OS signal/実 cancel 経路（GUI cancel、SIGINT）: NotRun
  （consumer/OS 範囲。library 契約は cooperative token）。
- LIB-IMPORT-07 multiple windows、LIB-QOS-04 実 device/GUI:
  NotRun（#89/#114 の責務、本 slice 対象外）。
- LIB-MIG-02 CRC→SHA-256 移行本体: #86 で main 済み・#91 は吸収
  しない（namespace 共存のみ Pass のまま）。
- SVN/P4/SMB/S3 provider: NotImplemented（matrix に行を設けない）。

上記の残存受入行が Issue #91 を open 維持する根拠である
（本 slice は PR #256 で merge 済みだが、実環境 lane・
cross-gate・#89/#114 所有行は未消化）。

詳細は `docs/test-specs/alib-07-fault-matrix.md` の LIB-IMPORT-04 /
LIB-IMPORT-08 行を参照。
