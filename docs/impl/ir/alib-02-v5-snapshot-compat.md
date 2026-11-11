---
id: "IMPL-alib02-v5snap"
type: "impl"
status: "completed"
title: "ALIB-02 v5 snapshot asset-digest 互換移行 + recovery fixture"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/plans/alib-02-v5-snapshot-compat.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/alib02_digest.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import_recovery.rs"
      - type: "file"
        path: "docs/specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/designs/alib-02-v5-snapshot-compat-design.md"
      - type: "file"
        path: "docs/plans/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/tasks/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/test-specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
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
    - "docs/plans/alib-02-v5-snapshot-compat.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    - "docs/impl/ir/alib-02-residual-closeout.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ALIB-02 v5 snapshot asset-digest 互換移行 + recovery fixture — 実装記録

## 実装内容

- `synvelis-project/src/project.rs`: `AssetDigest`/`DigestKind` 型付き
  分類層を追加し、`validate_references` で全 asset/source/applied-import
  の digest 文字列（空・不正形式拒否）と receipt→live entity 参照を検証。
- `synvelis-project/src/migration.rs`: STEPS ladder に v10→v11 step を
  追加。digest 文字列は lossless（書き換えなし）、分類境界は
  `validate_references` に集約。
- `synvelis-project/src/store.rs`: `SCHEMA_VERSION` 10→11、version
  history 更新。
- `synvelis-library/src/import.rs`: `CommitVerifiedImport::validate` に
  `AssetDigest::is_content_addressed()` ゲートを追加し、非 SHA-256 の
  content digest による verified commit を拒否。
- テスト: `crates/synvelis-project/tests/alib02_digest.rs`（5 件）、
  `crates/synvelis-library/tests/import_recovery.rs`（6 件）。
  `md_a.rs`/`tim_02_smf.rs` は schema version 追従のみ。

## 検証（補助証拠: cargo / canonical: mise）

- `cargo test -p synvelis-project --test alib02_digest`: 5 passed。
- `cargo test -p synvelis-project`（workspace project crate）: 全 pass。
- `cargo test -p synvelis-library --test import_recovery`: 6 passed。
- `cargo test -p synvelis-library --test import`: 17 passed（回帰なし）。
- `mise run fmt-check` / `check` / `lint` / `test`: 実施結果は PR 本文参照。

## 残件

- Linux/macOS 実 FS durability・directory sync: NotRun（Issue #91）。
- app/player 配線: PR #326（`import_verified_local` +
  digest-kind blob 解決）で headless 結合済み。実 device/GUI・
  app 層配線は synvelis#114・#97 の外部 gate。
- Issue #86 の DoD 照合・canonical lane 再実測・残存 gate 棚卸しは
  `docs/impl/ir/alib-02-residual-closeout.md` に集約。
