---
id: "TSPEC-Zb0ctcQ2s7S0lDwxkjXsiu"
type: "test-spec"
status: "approved"
title: "Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 — テスト仕様"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  verifies:
    - "docs/specs/issue-86-verified-import-wiring.md"
    - "docs/designs/issue-86-verified-import-wiring-design.md"
  verified-by:
    - "crates/synvelis-player/tests/verified_import.rs"
    - "docs/tasks/issue-86-task-verified-import-wiring.md"
---

# Issue #86 DoD(2) — verified import consumer 配線 + digest-kind blob 解決 — テスト仕様

## Feature: digest-kind blob 解決

### Rule: `sha256:` digest は `blobs/sha256/<hex>` から解決する

- Example: import_verified で commit した source（digest =
  `"sha256:<64hex>"`）が `resolver::prepare` で解決され、
  `HeadlessDriver` で fixture_sample 一致の PCM が得られる。
- Example: blob bytes を改竄すると `HashMismatch`（sha256 再検証）。

### Rule: `Crc32Fold` 旧経路は非回帰

- Example: `import_local`（flat `blobs/<crc16>`）で commit した source
  が従来どおり `resolver::prepare` で解決される（`l1_e2e` 既存 lane）。

### Rule: `Opaque` digest は fail-closed

- Example: `SetSourceState` で `Verified` へ無理やり進めた source が
  opaque digest（例: `"x"`, `sha256:` 形式不正）を持つ場合、
  `prepare` は `UnsupportedDigest` を返す — 再生経路に乗らない。

## Feature: consumer 入口 chain（DoD(2) 実結合）

### Rule: pin → import_verified → Verified → 再生

- Example: `import_verified_local` が `Imported`→`Verified` へ進め、
  receipt（`import_id`/`asset_id`/`source_id`/digest/byte_len）が
  project に反映され、`resolver::prepare` + `HeadlessDriver` で
  fixture sample 一致する。clip は `ImportTarget` 経由で同時 commit
  される。

### Rule: undo → 再 import は manifest 予約 ID で収束

- Example: import + state 前進の 2 undo で project から import が
  消え、同一 `import_id` での再 import は persisted revision
  expectation が stale → `StaleRevision` → `rebase_import` →
  commit が同じ `asset_id`/`source_id` に収束し `applied_imports`
  は 1 entry のまま。

### Rule: Saved HEAD → load → 再生

- Example: `DirectoryBundleStore` save の Saved HEAD を新
  `ProjectController::load` で復元し、`sha256:` source が
  Verified のまま解決・再生される（snapshot に source state と
  digest が乗る）。

### Rule: 中断 job は inspect → resume → commit → 再生

- Example: publish 直後 commit 前に中断した job が
  `inspect_imports` で `stage=Published`/`applied=false` と観測され、
  同一 `import_id` での entry 再実行が manifest resume で
  commit → `Verified` → 再生まで完走する。

### Rule: DirectorySync 証拠は正直に記録

- Example: Windows で `directory_sync` が `Unsupported` の場合、
  `VerifiedImportOutcome.media.durability` にそのラベルが残る
  （失敗扱いしない・Unsupported として正直に記録）。

## 対象外（Pass 扱いしない）

- RT engine / 実 audio device / 実 GUI 経路
- `import_local` 本体の検証（既存 `l1_e2e` lane が非回帰証拠）
- app 層（Service op / UI）配線
