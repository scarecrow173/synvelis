---
id: "IMPL-alib02-v5snap-slice"
type: "impl"
status: "completed"
title: "ALIB-02 v5 snapshot 互換移行 — 実 v5 fixture residual slice（Issue #86 DoD(1)）"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/alib02_v5_real.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/fixtures/alib02-v5-legacy.json"
      - type: "file"
        path: "crates/synvelis-project/tests/fixtures/alib02-v5-legacy.blob"
      - type: "file"
        path: "evidence/issue-86-v5-migration/README.md"
      - type: "file"
        path: "evidence/issue-86-v5-migration/emit_v5_fixture.rs"
      - type: "file"
        path: "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    modified:
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
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-02-v5-snapshot-compat.md"
    - "docs/tasks/alib-02-v5-snapshot-compat.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/designs/alib-02-v5-snapshot-compat-design.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/alib-02-v5-snapshot-compat.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/alib-02-streaming-verified-import.md"
    - "docs/impl/ir/alib-02-residual-closeout.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/alib-02-v5-snapshot-compat-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/alib-02-v5-snapshot-compat.md"
  verified-by:
    - "crates/synvelis-project/tests/alib02_v5_real.rs"
  references:
    - "evidence/issue-86-v5-migration/README.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-02 v5 snapshot 互換移行 — 実 v5 fixture residual slice（Issue #86 DoD(1)）

## Summary

Issue #86 DoD(1) の残件: 「実 v5 形式ファイルの decode→移行→digest
変換の本体は未実装（deferred stub）」という close 判定への応答。
v5-era build（commit `a401e5c`、`SCHEMA_VERSION=5`）の実 serializer
が出力した snapshot + blob を fixture 化し、現行コードで
decode→STEPS 全 ladder→digest 保全→save→reload→read-only 原本保存→
deterministic projection→劣化/reject 分類の連鎖を
`tests/alib02_v5_real.rs`（8 tests）で固定した。

実測の結論: **production code の追加は不要だった**。既存の
`migration.rs` STEPS ladder・`snapshot/` resolver・
`AssetDigest`/`DigestKind` 分類・controller の revision bump が、
Issue が要求する受入条件をすべて満たしていた。不足していたのは
「実 v5 bytes による証明」だけであり、本 slice はその証拠と
doc chain の実装同期を提供する。

## Context

- Issue: <https://github.com/scarecrow173/synvelis/issues/86>（DoD(1)）
- 先行 slice: `docs/impl/ir/alib-02-v5-snapshot-compat.md`
  （digest 分類・v10→v11 step・import_recovery fixture、合成 v5 envelope）
- Spec/Design/Plan/Task/Test-spec: `alib-02-v5-snapshot-compat` chain
- Evidence: `evidence/issue-86-v5-migration/README.md`
  （fixture provenance・generator・実測値）
- 現行 `SCHEMA_VERSION=13`（v11 digest・v12 monitoring・v13 plugin instances）

## Implementation

### Changes（test + evidence + docs only。src 変更なし）

- `crates/synvelis-project/tests/alib02_v5_real.rs`（新規、8 tests）:
  - `real_v5_snapshot_decodes_migrates_and_reloads` — 実 bytes の
    decode→全 ladder→digest verbatim→receipt 整合→save→reload。
    routing 投影（track strip + `Main` connection のみ、send/processor
    合成なし）、`scope` owner の typed 投影、revision `22→23`、
    旧 bytes 不変を検証。
  - `real_v5_read_only_projection_is_deterministic` —
    `resolve_saved`×2 で `PartialEq` 等価、`schema_version 5 →
    projection_version 13`、`Valid`/`Complete`。
  - `migrated_save_preserves_original_and_sibling_blobs` —
    `DirectoryBundleStore` で v5 原本・flat `blobs/<crc>`・
    `blobs/sha256/<hex>`・`plugin-state/` opaque が byte-identical。
  - `missing_blob_classifies_degraded_not_rejected` /
    `unprobed_blob_never_reports_present` — `Missing`/`Unknown` →
    `Degraded`+`Partial`（`Present` 捏造なし）。
  - `unknown_schema_is_rejected_not_degraded` — schema 4/99 は両
    reader で `UnsupportedSchema` reject、source 不変。
  - `corrupt_or_foreign_state_is_rejected` — 切断 bytes→`Deserialize`、
    非 default `RoutingModel` 混入→`Validation`（foreign reject）。
  - `crc_collision_assets_stay_separate_and_weak` — 同一 CRC の
    2 asset が別 identity・非 content-addressed のまま。
- `crates/synvelis-project/tests/fixtures/alib02-v5-legacy.{json,blob}`:
  v5 serializer 実出力（6,328B JSON / 9,600B blob、CRC fold
  `5fea10b959fe82a9`）。
- `evidence/issue-86-v5-migration/`: generator ソース
  （`emit_v5_fixture.rs`、detached worktree でのみビルド）+ 出所と
  実測結果の README。
- doc chain 5 件: `AssetDigest::parse` を infallible 分類へ訂正、
  `SCHEMA_VERSION=13` へ追従、劣化語彙を `snapshot/` 型へ正規化、
  本 slice の file map / 検証項目を追加。

### 判断と根拠

- **fail-closed 分類の追加は不採用**: `AssetDigest::parse` は
  CRC16-hex を受理する（fail しない）。`kind()` が `Crc32Fold` を返し
  `is_content_addressed()==false` で verified 昇格を構造的に防ぐため、
  「受理して弱く分類」が設計どおり正しい挙動。空文字列のみ
  `validate_digests` が拒否する。
- **Missing/IntegrityDegraded/ReadOnly/Reject の新 enum は不採用**:
  `snapshot/` モジュールの `ContentStatus`/`SnapshotValidation`/
  `SnapshotCompleteness`/`SnapshotError` が同じ意味論を既に提供して
  いた。Issue の「既存 Error 型の語彙に合わせる」に従い、重複語彙を
  増やさず既存型を fixture で検証した。
- **`migrate_v5_routing` の意味は変更していない**（タスク制約遵守）。

## Verification

| Check | Result |
| --- | --- |
| `cargo test -p synvelis-project --test alib02_v5_real` | 8 passed（補助証拠・開発ループ） |
| `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` / `test-msrv` | PR #328 時点で全 lane Pass。closeout 再実測（2026-11-11、origin/main `36c2254` + docs-only 差分）の結果は `docs/impl/ir/alib-02-residual-closeout.md` Validation と `evidence/issue-86-v5-migration/closeout-verification.md` を参照 |

## Remaining

- DoD(2)（consumer 実結合）は PR #326 で統合済み
  （`docs/impl/ir/issue-86-verified-import-wiring-slice.md`）。
  RT・実 device・実 GUI の外部 gate は synvelis#114・#97 が所有。
- 実 FS durability・OS 差（Issue #91）は対象外のまま。
- Issue #86 全体の DoD 照合・残存 gate 棚卸しは
  `docs/impl/ir/alib-02-residual-closeout.md` に集約。
