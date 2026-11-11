---
id: "TASK-VB7LzgmqMRIfE31stZItL7"
type: "task"
status: "done"
title: "Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/issue-86-verified-import-wiring-plan.md"
  depends-on:
    - "docs/designs/issue-86-verified-import-wiring-design.md"
  verifies:
    - "docs/test-specs/issue-86-verified-import-wiring.md"
---

# Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決

- [x] `player::blob::open_blob_verified` — `AssetDigest` kind 分岐
      （`Sha256`→`blobs/sha256/<hex>`+sha2 verify、`Crc32Fold`→flat+crc
      fold、`Opaque`→`UnsupportedDigest` fail-closed）、同一 fd
      verify→decode
- [x] `resolver::open_verified` + `render::prepare::open_source` を
      helper へ移行（重複集約）
- [x] `error.rs`: `ResolveError::UnsupportedDigest` + `VerifiedImportError`
- [x] `verified_import::import_verified_local` — provider-bounded pin →
      `import_verified` → published blob decode 検証 → `SetSourceState`
      (`Verified` 止め)
- [x] `Cargo.toml`: `synvelis-library` + `sha2` 依存、`lib.rs` exports
- [x] tests: `crates/synvelis-player/tests/verified_import.rs` —
      digest-kind 3 分岐・chain 再生・undo 再 import 収束・save/load
      再生・中断 job resume
- [x] canonical `mise` 検証（fmt-check/check/lint/test/test-rt/test-msrv）
- [x] evidence + docs index 再生成 + impl record
