---
id: "IMPL-qGhrNdq0yLEfcWCITQ0m0p"
type: "impl"
status: "completed"
title: "Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 実装記録"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/blob.rs"
      - type: "file"
        path: "crates/synvelis-player/src/verified_import.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/verified_import.rs"
      - type: "file"
        path: "docs/specs/issue-86-verified-import-wiring.md"
      - type: "file"
        path: "docs/designs/issue-86-verified-import-wiring-design.md"
      - type: "file"
        path: "docs/plans/issue-86-verified-import-wiring-plan.md"
      - type: "file"
        path: "docs/tasks/issue-86-task-verified-import-wiring.md"
      - type: "file"
        path: "docs/test-specs/issue-86-verified-import-wiring.md"
      - type: "file"
        path: "docs/impl/ir/issue-86-verified-import-wiring-slice.md"
      - type: "file"
        path: "evidence/issue-86-verified-import-wiring/verification-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-player/src/resolver.rs"
      - type: "file"
        path: "crates/synvelis-player/src/error.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-render/src/prepare.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "Cargo.lock"
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
    - "docs/tasks/issue-86-task-verified-import-wiring.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-player/src/blob.rs"
    - "crates/synvelis-player/src/verified_import.rs"
    - "crates/synvelis-player/tests/verified_import.rs"
    - "evidence/issue-86-verified-import-wiring/verification-ledger.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 — 実装記録

## 実装内容

- `synvelis-player/src/blob.rs`（新規）: digest-kind 解決の単一権威
  `open_blob_verified`。`AssetDigest::parse(digest).kind()` で分岐し、
  `Sha256`→`blobs/sha256/<hex>`+sha2 再計測、`Crc32Fold`→flat
  `blobs/<digest>`+crc fold、`Opaque`→`ResolveError::UnsupportedDigest`
  fail-closed。検証済み `File` handle を返し verify→open 間の
  path-swap ギャップを排除。
- `synvelis-player/src/verified_import.rs`（新規）: consumer 入口
  `import_verified_local`。
  `LocalFolderProvider::pin`（root-bounded）→ `ImportIntent{import_id:
  draw_raw_id() or reserved, expected_project_revision:
  live_revision(), source: pin, target}` →
  `synvelis_library::import_verified` → 公開済み blob を
  `open_blob_verified` で byte 検証 → Symphonia probe + PCM codec 確認
  → `ProjectOp::SetSourceState{Verified}` を `expected_project_revision`
  CAS で commit。library 側 commit は常に `Imported` のため live
  source state を確認してから前進させる。
- `synvelis-player/src/error.rs`: `ResolveError::UnsupportedDigest` と
  `VerifiedImportError`（Provider/Import/Resolve/CorruptMedia/
  UnsupportedCodec/CommitRejected/Io）を追加。
- `synvelis-player/src/resolver.rs`: flat-path+crc fold の重複実装を
  `open_blob_verified` へ集約。
- `synvelis-render/src/prepare.rs`: 同様の重複実装を
  `synvelis_player::blob::open_blob_verified` へ集約（render は既に
  player 依存のため依存方向不変）。
- `synvelis-player/src/lib.rs`: `blob`・`verified_import` module 公開 +
  `VerifiedBlob`/`open_blob_verified`/`VerifiedImportError`/
  `VERIFIED_IMPORT_PROVIDER`/`VerifiedImportOutcome`/
  `VerifiedImportRequest`/`import_verified_local` re-export。
- `synvelis-player/Cargo.toml`: `synvelis-library`（path dep）と
  `sha2`（workspace）追加。library→project 単方向なので player→library
  は非循環。
- テスト: `crates/synvelis-player/tests/verified_import.rs`（8 件）。

## 意図的な設計決定

- **Verified 止め**: `SetSourceState` は `Verified` まで。`Committed`
  まで進めると 2 command=2 undo step で旧経路と undo 粒度が変わるため
  spec で明示したとおり `Verified` で停止（playback gate は `>= Verified`）。
- **`import_local` 温存**: 旧経路（fs::read 全件・CRC fold・無条件
  commit）は変更せず 20 test call sites を非回帰で維持。
- **Opaque fail-closed**: 不明な digest 文字列は検証済み content
  address として解釈しない。legacy CRC は byte-level 検証のみで強
  識別子へ昇格しない。
- **manifest replay での target 無視**: resume では manifest 記録の
  target が優先されるため、clip 付き import は初回呼び出し時に
  target を渡す設計とした（テストもそれに合わせて構築）。

## 検証（canonical: mise）

- `mise run fmt` / `fmt-check`: pass。
- `mise run check`: pass（exit 0）。
- `mise run lint`: pass（`-D warnings`、exit 0）。
- `mise run test`: pass（workspace 全緑、exit 0）。
- `mise run test-rt`: pass（exit 0）。
- `mise run test-msrv`: 結果は PR 本文参照。
- `crates/synvelis-player/tests/verified_import.rs`: 8 passed
  （sha256 resolve+play / imported gating / crc fold / opaque
  fail-closed / sha256 missing+mismatch / undo→reimport reserved IDs /
  saved HEAD→load→play / interrupted inspect→resume）。
- evidence: `evidence/issue-86-verified-import-wiring/verification-ledger.md`。

## 残件

- RT thread・実 audio device・実 GUI・app 層配線: 対象外（別 Issue）。
- DirSync durability: Windows では Unsupported（spec に記録のみ）。
- Issue #86 には他 DoD が残存するため Issue は open 維持（PR 本文は
  `Refs #86`）。
