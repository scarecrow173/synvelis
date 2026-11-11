---
id: "IMPL-alib02-streaming-verified-import"
type: "impl"
status: "completed"
title: "ALIB-02 Streaming Verified Import — SHA-256 namespace（synvelis-library slice）"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/src/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "docs/specs/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/specs/alib-02-v5-snapshot-compat.md"
      - type: "file"
        path: "docs/designs/alib-02-streaming-verified-import-design.md"
      - type: "file"
        path: "docs/plans/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/tasks/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/test-specs/alib-02-streaming-verified-import.md"
      - type: "file"
        path: "docs/impl/ir/alib-02-streaming-verified-import.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/src/project.rs"
      - type: "file"
        path: "docs/specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-02-streaming-verified-import.md"
    - "docs/tasks/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/designs/alib-02-streaming-verified-import-design.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/alib-02-v5-snapshot-compat.md"
    - "docs/impl/ir/alib-02-v5-snapshot-compat-slice.md"
    - "docs/impl/ir/alib-02-residual-closeout.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/specs/alib-02-residual-closeout.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/alib-02-streaming-verified-import-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/alib-02-streaming-verified-import.md"
  verified-by:
    - "docs/test-specs/alib-02-streaming-verified-import.md"
  references:
    - "https://github.com/scarecrow173/synvelis/pull/220"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-02 Streaming Verified Import — 実装記録（PR #220 backfill）

## Summary

PR #220（`Refs #86`、2026-11-09 merge、squash commit `929a5e9`）で
`synvelis-library` に実装された bounded-memory streaming verified
import パイプラインの実装記録。merge 時に spec/design/plan/task/
test-spec の doc chain は作成されたが Implementation Record が未作成
だったため、merge 済みという実態に合わせて後付けで正規化する
backfill である。

実装は ALIB-01 の `AssetProvider`/`PinnedAssetRef` 契約の上に、
staged streaming + 逐次 SHA-256 計測、staged readback verify、
`blobs/sha256/<64hex>` への atomic publish、manifest 固定
`expected_project_revision` での単一 atomic project commit、
crash-safe manifest による復旧経路を library 層で完結させる。

## Context

- Issue: <https://github.com/scarecrow173/synvelis/issues/86>（ALIB-02）
- PR: <https://github.com/scarecrow173/synvelis/pull/220>（merge
  commit `929a5e9`）
- Spec: `docs/specs/alib-02-streaming-verified-import.md`
- Design: `docs/designs/alib-02-streaming-verified-import-design.md`
- Plan: `docs/plans/alib-02-streaming-verified-import.md`
- Task: `docs/tasks/alib-02-streaming-verified-import.md`
- Test-spec: `docs/test-specs/alib-02-streaming-verified-import.md`
- 後続 slice: `docs/impl/ir/alib-02-v5-snapshot-compat.md`（PR #253、
  v5 asset-digest 分類・v10→v11 migration・import_recovery fixture）

## Implementation

### Changes

- `crates/synvelis-library/src/import.rs`（新規、約 1.8k 行）:
  - `ImportIntent`/`ImportTarget`/`ImportStore`/`ImportManifest`
    （serde・`format_version`・`StagedFile`+`PublishMode::Replace`
    による atomic manifest write）。
  - Stage machine `Started → Staged → Verified → Published →
    Committed` を各遷移で永続化。
  - `stage_source`: 64 KiB 固定 chunk で bounded streaming、
    incremental SHA-256 + byte-length 計測、chunk 毎の
    `check_interrupts`・byte budget・pin 期待 digest/size 照合・
    provider completion-slot 交差検査。
  - `verify_staged`: staged bytes の readback 再計測を manifest と
    照合。
  - `BlobPublisher` port + `StagedBlobPublisher`
    （`PublishMode::CreateNew`、`FileAndDirectorySync` の正直な
    `Unsupported` fallback、publish-temp readback verify、seal 前の
    integrity gate）。
  - Publish 三値: `Published`（post-failure verify）、
    `Indeterminate`（stat reconcile + 単一 retry →
    `PublishIndeterminate`）、`NotPublished`。
    `DirectorySync::{Supported,Unsupported,NotRequested,Failed,
    Unknown}` は観測値のみ記録。
  - `CommitVerifiedImport` `Operation`: manifest 固定
    `expected_project_revision` で asset+source+applied-import
    （+任意 clip）を単一 atomic apply。予約 ID の
    reseed/衝突検査・lane/track/clip 契約検証。
  - `rebase_import`（stale 解決のみ）、`inspect_imports`、
    `gc_orphans`（project+manifest reachability、foreign/legacy
    非触）。
- `crates/synvelis-library/src/project.rs`:
  `LocalBlobDir::object_rel` に additive な `sha256:<64hex>` →
  `sha256/<hex>` 写像を追加。旧 flat 名前空間は不変・衝突不可。
- `crates/synvelis-library/src/lib.rs`: module 登録・export・doc。
- `crates/synvelis-library/tests/import.rs`（新規・15 tests）:
  bounded streaming（1 MiB @ 7 byte chunks）、opaque bytes、replay
  冪等/conflict、digest/size/budget integrity、stale+明示 rebase、
  missing/wrong-track lane 拒否、publish 失敗 resume、
  indeterminate reconcile、DirSync 失敗記録、orphan GC、
  namespace 分離、live≠saved。
- docs チェーン: spec/design/plan/task/test-spec を同 PR で作成
  （v5 snapshot 互換は deferred stub `alib-02-v5-snapshot-compat`
  として分離）。

### Decisions During Implementation

- `Source` は `SourceState::Imported` で commit — byte integrity のみ
  検証し、media decode 検証は consumer の `SetSourceState` 責務
  （spec の Deferred Design Concerns 通り）。
- scripted `BlobPublisher`/`AssetProvider` double は fault *分類* の
  証拠であり、実 durability 証拠ではない。実 publish は ruviform
  `StagedFile`/`SealedFile` primitive 経由で独自 atomic rename/
  durability の再実装はしない。
- `import_id` 予約は durable reservation floor 超過が必要
  （manifest-less dir 保護のため）。
- `Indeterminate` reconcile は stat + 単一 retry とし、2 連続
  indeterminate は `PublishIndeterminate` で停止・手動 inspect を
  要求する。
- `CommittedLive` は `SavedRevision`/`HEAD` を意味しない
  （`saved()==None` を test で assert）。

## Related Experiments

### Adopted

<!-- なし（experiment log は記録されていない） -->

### Rejected

<!-- なし -->

## Validation

PR #220 本文に記録された検証（canonical = mise、Windows 実機）:

- `mise run fmt-check`: exit 0。
- `mise run check`: exit 0（merge 後再実行も exit 0）。
- `mise run lint`: exit 0（`-D warnings`、警告なし）。
- `mise run test`: exit 0（workspace 全 pass、`tests/import.rs`
  15/15 pass、merge 後も 15/15）。
- 補助証拠（non-canonical）: `cargo test -p synvelis-library
  --test import` 15/15、`cargo clippy -p synvelis-library
  --all-targets` 警告 0。
- review-protocol: 1 サイクル実施、blocker なし（非 blocker 指摘は
  spec/design の意図通りと確認）。

PR #220 時点の NotRun / Unsupported:

- `mise run test-msrv` / `test-rt` / `build`: NotRun（当該 PR の
  時間切れ未実施。後続 lane で workspace 全体 green を確認）。
- Linux/macOS 実機 FS 差・DirSync 可否: Unsupported（本環境は
  Windows のみ。platform 差は `DirectorySync::Unsupported` 記録の
  設計）。
- Mock FS による実 durability PASS: 実施せず（scripted double は
  分類証拠のみ）。

## Risks

- 本 IR は merge 後の backfill であり、実装時点の逐語的記録では
  ない。一次根拠は PR #220 本文・merge commit `929a5e9`・task/
  test-spec の当時記録であり、検証結果は同 PR 本文の転記（再測定
  ではない）。
- `PublishIndeterminate` 2 連続時は自動収束せず手動
  `inspect_imports` が必要（設計上の意図的停止点）。

## Follow-ups

Issue #86 の残実装 DoD は後続 slice で統合済み（2026-11-11 時点）:

- v5 旧 Asset/Source/Clip ID・Blob path/旧 CRC の versioned reader
  共用・read-only 原本保存・安全移行・再読込 — PR #253
  （`docs/impl/ir/alib-02-v5-snapshot-compat.md`）+ PR #328
  （`docs/impl/ir/alib-02-v5-snapshot-compat-slice.md`、実 v5 fixture
  `tests/alib02_v5_real.rs` 8 tests）で統合済み。
- Import Receipt → `ProjectController` CAS/Undo → Saved HEAD/
  Recovery → Offline Playback の実結合 — PR #326
  （`docs/impl/ir/issue-86-verified-import-wiring-slice.md`、
  `tests/verified_import.rs` 8 tests）で headless 結合済み。
- Duplicate ImportId/StaleRevision/Plugin Opaque/欠落 Asset/
  Cancel/Crash の独立 fixture — `tests/import.rs`（21 tests）・
  `tests/import_recovery.rs`（6 tests）・`tests/alib02_v5_real.rs`・
  `tests/verified_import.rs` に収録済み。
- 新旧 schema round trip・Ruviform staged publish/RT 非回帰 —
  `alib02_digest.rs`/`alib02_v5_real.rs`/`saved_head_load_replays`・
  `mise run test-rt` で実測済み。

Issue #86 は外部 gate（実 OS/FS=#91、統合試験=#97、実 GPUI/GUI=
#114、real provider/remote LFS=#331、cross-consumer/AudioLens=
ruviform#294）が残るため open 継続。DoD 行 ↔ 証拠の 1:1 照合と
残存 gate 棚卸しは `docs/impl/ir/alib-02-residual-closeout.md` と
`docs/specs/alib-02-residual-closeout.md` を正本とする。
本 IR は PR #220 merge 済みという実態の簿記であり、Issue の close
は行わない。
