---
id: "PLAN-alib02-v5snap"
type: "plan"
status: "completed"
title: "ALIB-02 v5 snapshot asset-digest 互換移行 — 実装計画"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/alib02_digest.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import_recovery.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
      - type: "file"
        path: "crates/synvelis-library/src/import.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
  implemented-by: []
  depends-on:
    - "docs/designs/alib-02-v5-snapshot-compat-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/alib-02-v5-snapshot-compat.md"
  references: []
  defers: []
  deferred-by: []
---

# ALIB-02 v5 snapshot asset-digest 互換移行 — 実装計画

## Scope split

単一 slice。`synvelis-project` の schema/digest 境界と
`synvelis-library` の commit 境界・recovery fixture のみ。

## File map

| File | 変更 |
| --- | --- |
| `crates/synvelis-project/src/project.rs` | `AssetDigest`/`DigestKind` 追加、`validate_references` に digest/receipt 検証 |
| `crates/synvelis-project/src/migration.rs` | STEPS に v10→v11 step、分類境界の適用 |
| `crates/synvelis-project/src/store.rs` | `SCHEMA_VERSION` 10→11、version history 更新 |
| `crates/synvelis-project/tests/alib02_digest.rs` | v5 fixture・分類・拒否・round-trip（新規） |
| `crates/synvelis-project/tests/md_a.rs` / `tim_02_smf.rs` | schema version 追従 |
| `crates/synvelis-library/src/import.rs` | `CommitVerifiedImport::validate` に sha256 ゲート |
| `crates/synvelis-library/tests/import_recovery.rs` | receipt/save/undo/stale/target-loss/namespace fixture（新規） |

## Dependency graph

`AssetDigest`（project）→ `validate_references` → migration step →
`CommitVerifiedImport::validate`（library が project 型を参照）。

## Risk register

- 旧 snapshot が空 digest を含む場合 load が新たに失敗し得る →
  仕様上「不正 digest は拒否」が受入条件であり、テストで固定。
- `validate_references` 強化が既存テスト fixture を壊す可能性 →
  workspace test で回帰確認。

## Verification matrix

- `cargo test -p synvelis-project` / `-p synvelis-library`（補助証拠）
- `mise run fmt-check` / `check` / `lint` / `test`（canonical）
