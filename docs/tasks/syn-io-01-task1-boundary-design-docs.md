---
id: "TASK-034UvJH1gMKAHU5UZUrzNN"
type: "task"
status: "done"
title: "SYN-IO-01 task1 境界 Owner・失敗写像・Backup/Import 設計文書"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/82"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/syn-io-01-ruviform-io-boundary-design.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-io-01-ruviform-io-boundary-design.md"
  blocks:
    - "docs/tasks/syn-io-01-task2-verification-gate-docs.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md"
    - "docs/discovery/syn-io-01-io-boundary-inventory.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-01-io-boundary-verification.md"
  references: []
  defers: []
  deferred-by: []
---

# SYN-IO-01 task1 境界 Owner・失敗写像・Backup/Import 設計文書

## Work

Issue #82 の AC-1〜AC-4・AC-8 に対応する設計文書を作成する。

- `docs/discovery/syn-io-01-io-boundary-inventory.md`:
  Synvelis 現行 Project I/O（`DirectoryBundleStore`・
  `ProjectController`・gateway journal・各 crate 利用面）と
  Ruviform 公開 I/O 契約（`ruviform-io-file`/`structured`/
  `archive`/`compression`/rooted FS）の棚卸し。
- `docs/specs/syn-io-01-ruviform-common-io-boundary.md`:
  境界の要求事項と AC-1..8。
- `docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md`:
  Backup/Import archive 初期採用範囲（ZIP コンテナ・
  Stored/Deflate・limits/path/link/manifest 方針）の決定。
- `docs/designs/syn-io-01-ruviform-io-boundary-design.md`:
  責務分担表・Owner 固定表・失敗写像表・状態遷移図・
  Backup/Import 設計・非退行/影響文書化・gate 証拠項目。

## Acceptance

- AC-1: 責務・データ/状態遷移差が図示されている。
- AC-2: ExpectedRevision/HEAD/immutable Snapshot/外部変更/
  Crash 後 Reopen の Owner が Synvelis に固定。
- AC-3: 置換/Sync/indeterminate/rollback/HA storage が
  `ProjectStore` へマッピング。
- AC-4: Backup/Import の ZIP/TAR/Compression 初期採用範囲・
  Limits/Link/Path/保存形式が設計。
- AC-8: consumer 固有意味論維持・旧 API 互換層/恒久 SHA
  固定非依存が明記。

## Verification

- `doc_status`/`doc_maintenance` lint で front matter・relation
  整合を確認。
- review-protocol で blocker なし。
