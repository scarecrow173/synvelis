---
id: "TASK-alib02-v5snap-t1"
type: "task"
status: "completed"
title: "ALIB-02 v5 asset-digest migration + recovery fixture"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-02-v5-snapshot-compat.md"
  depends-on:
    - "docs/designs/alib-02-v5-snapshot-compat-design.md"
  verifies:
    - "docs/test-specs/alib-02-v5-snapshot-compat.md"
---

# ALIB-02 v5 asset-digest migration + recovery fixture

- [x] `AssetDigest`/`DigestKind` + `validate_references` digest/receipt 検証
- [x] migration STEPS v10→v11 + `SCHEMA_VERSION=11`
- [x] `CommitVerifiedImport::validate` sha256 ゲート
- [x] project tests: v5 CRC fixture・分類・空 digest 拒否・collision・receipt integrity
- [x] library tests: save/load replay・save 失敗回復・undo 再 import・stale→target 消失・複数 track 配置・legacy namespace 共存
- [x] canonical `mise` 検証（fmt-check/check/lint/test）
