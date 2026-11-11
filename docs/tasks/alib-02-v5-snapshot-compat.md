---
id: "TASK-alib02-v5snap-t1"
type: "task"
status: "done"
title: "ALIB-02 v5 asset-digest migration + recovery fixture"
created: "2026-11-10"
updated: "2026-11-11"
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

初回 slice（2026-11-10）:

- [x] `AssetDigest`/`DigestKind` + `validate_references` digest/receipt 検証
- [x] migration STEPS v10→v11 + `SCHEMA_VERSION=11`
- [x] `CommitVerifiedImport::validate` sha256 ゲート
- [x] project tests: v5 CRC fixture・分類・空 digest 拒否・collision・receipt integrity
- [x] library tests: save/load replay・save 失敗回復・undo 再 import・stale→target 消失・複数 track 配置・legacy namespace 共存
- [x] canonical `mise` 検証（fmt-check/check/lint/test）

Residual slice — Issue #86 DoD(1)（2026-11-11）:

- [x] 実 v5-era build（`a401e5c`）で実 fixture 生成 → `tests/fixtures/alib02-v5-legacy.{json,blob}`、generator/出所は `evidence/issue-86-v5-migration/`
- [x] `tests/alib02_v5_real.rs`（8 tests）: 実 v5 decode→STEPS 全 ladder→digest 保全→save→reload、revision +1・新 snapshot 名、旧 bytes/flat blob/`blobs/sha256/`/plugin-state opaque の read-only 保存、`resolve_saved` deterministic projection、Missing/Unknown/Degraded・UnsupportedSchema/Deserialize/Validation reject・CRC collision の分類
- [x] production code 追加なしで受入条件成立を実測確認（既存 STEPS ladder・`snapshot/` resolver・`AssetDigest` 分類で成立）
- [x] doc chain を実測に合わせて更新（`AssetDigest::parse` infallible 化・`SCHEMA_VERSION=13`・劣化語彙）
