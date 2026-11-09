---
id: "IMPL-Vcs01ImmutableSnapshotResolver"
type: "impl"
status: "completed"
title: "VCS-01 Immutable Snapshot Resolver — Live/Saved/Recovery/VCS read-only projection"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/7"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/snapshot/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/snapshot/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/snapshot/digest.rs"
      - type: "file"
        path: "crates/synvelis-project/src/snapshot/vcs.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs01_snapshot.rs"
      - type: "file"
        path: "docs/specs/vcs-01-immutable-snapshot-resolver.md"
      - type: "file"
        path: "docs/designs/vcs-01-immutable-snapshot-resolver-design.md"
      - type: "file"
        path: "docs/plans/vcs-01-immutable-snapshot-resolver.md"
      - type: "file"
        path: "docs/test-specs/vcs-01-snapshot-resolver-fixtures.md"
      - type: "file"
        path: "docs/tasks/vcs-01-task1-immutable-snapshot-resolver-headless-slice.md"
      - type: "file"
        path: "docs/impl/ir/vcs-01-immutable-snapshot-resolver.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-project/src/smf.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-01-immutable-snapshot-resolver.md"
    - "docs/designs/vcs-01-immutable-snapshot-resolver-design.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/tasks/vcs-01-task1-immutable-snapshot-resolver-headless-slice.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-01-immutable-snapshot-resolver-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-01-immutable-snapshot-resolver.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-01-snapshot-resolver-fixtures.md"
  verified-by: []
  references: []
  defers:
    - "VCS-02 Semantic Diff ChangeSet（Issue #93）"
    - "VCS-03 Git Read Provider（Issue #94）"
    - "実 Git remote / 実 OS filesystem atomicity / GUI / 実 plugin host probe（VCS-04/05/06, Issue #97）"
  deferred-by: []
---

# VCS-01 Immutable Snapshot Resolver — Implementation Record

## What was built

`synvelis-project` に VCS 非依存の Immutable Snapshot Resolver を
additive module `src/snapshot/` として実装した。`SnapshotResolver`
は Live / Saved HEAD / Recovery checkpoint / VCS staged /
VCS committed の 5 locator 系統を、厳密 `SnapshotEnvelope` parse →
既存 migration ladder（`migration::migrate_to_current`）→
`validate_references` + `check_id_watermark`（`load` と同一
oracle）→ exact-bytes SHA-256 digest → read-only projection の
単一 pipeline で `ImmutableSnapshot` へ解決する。

- `digest.rs`: crate 内 SHA-256（新規依存なし — 本番依存追加は
  タスク制約で禁止。`sha2` は workspace 依存だが
  `synvelis-project` 未使用のため manifest 変更なしで内製化）。
  FIPS 既知解答ベクタで unit test 済み。`ContentDigest` は
  `"sha256:<64hex>"` の algorithm-tagged 文字列で、Git object ID
  とは別名前空間。
- `types.rs`: `SnapshotLocator`/`FixedSnapshotLocator`/
  `SnapshotIdentity`/`SnapshotOrigin`/`ImmutableSnapshot`/
  `ReadOnlyProjectProjection`（`Arc<Project>`・mutable 経路なし）/
  `AssetAvailabilityManifest`/`OpaqueStateRef`/
  `SnapshotCompleteness`/`SnapshotValidation`/`ContentStatus`。
- `vcs.rs`: `VcsBytesPort`（`index_generation`/`read_file`/
  `read_blob` の read-only 契約）・`VcsLocator`/`VcsFileRead`/
  `BlobRead`/`VcsPortError`・`BlobProbe`。write method を持たない
  ため worktree/index 変更は構造上不可能。
- `mod.rs`: `SnapshotResolver`（`capture_live`/`resolve_saved`/
  `resolve_saved_head`/`resolve_recovery`/`resolve_vcs`）・
  `SnapshotError`（11 variant の明示 Failure）・`SnapshotCancel`・
  `SnapshotLimits`。
- `controller.rs`: `snapshot_id_watermark()` read-only accessor のみ
  追加（`save` が stamp するのと同じ `generator.peek()` を返す。
  Single Writer/Undo/Redo/load/save の semantics は不変）。
- `lib.rs`: `pub mod snapshot` + 全公開型の re-export。

### Race / integrity semantics

- Saved: HEAD→bytes→HEAD→bytes の 4 read window。HEAD 変化は
  `HeadConflict`、同名 bytes 変化は `SourceChanged`、`head_ref`
  pin 不一致も `HeadConflict`。
- Recovery: recovery-rooted store の checkpoint 名を 2 read 比較。
  saved HEAD を参照も変更もしない。
- VCS staged: `index_generation` を window 前後で比較
  （`IndexChanged`）、locator pin との不一致も `IndexChanged`、
  index が運ばない project file/blob は `PartialStage`。
- VCS committed: `fixed_revision` pin。absent blob は Failure で
  はなく `Missing` 記録（degraded）。
- asset `Absent`/`Unknown`・`ProcessorBinding::Unresolved`・
  `Resolved`（probe なし opaque state = `Unknown` 固定）は
  manifest/opaque_state に記録され `Partial`/`Degraded`。
  Equal 判定は resolver の責務外（VCS-02）。

## Verification（実施済み）

| Task | Result |
| --- | --- |
| `mise run fmt` / `fmt-check` | Pass |
| `mise run check` | Pass（workspace --all-targets） |
| `mise run lint` | Pass（-D warnings。事前に main 由来の既存警告 2 件を style commit で解消） |
| `mise run test` | Pass（workspace --locked。vcs01_snapshot 28 tests + digest FIPS 2 tests 含む） |
| `mise run test-rt` | Pass（rt_ contract tests） |
| `mise run test-msrv` | Pass（rust 1.89.0 toolchain で workspace test） |
| `mise run check-app-native` | Pass |
| `mise run lint-app-native` | Pass |
| `mise run build-app-native` | Pass |

Headless fixture coverage: test-spec SNAP-FIX-01〜10 を
`tests/vcs01_snapshot.rs` 28 test で網羅。read-only oracle
（controller revision/live/saved・store HEAD/bytes・VCS port
call journal が resolve 前後で不変）を assert 済み。

### NotRun / Unsupported（明示）

- 実 OS filesystem の atomicity（directory bundle store の実 FS
  挙動）: **NotRun** — headless `FakeStore` fixture のみ。
- 実 Git remote/provider・実 index/worktree: **Unsupported** —
  `VcsBytesPort` 境界のみ。実装は VCS-03（Issue #94）。
- GUI / 実 plugin host probe: **Unsupported** — opaque state は
  `Resolved`→`Unknown`・`Unresolved`→`Missing` 固定。
- Semantic Diff/ChangeSet/comparison: **対象外** — VCS-02
  （Issue #93）。
- 実 transport/engine プロセス: **対象外** — 本 crate に該当
  runtime は存在せず、resolver は controller への参照を保持
  しないため構造上影響しない（undo/edit/save 後の snapshot
  不変性は fixture で検証済み）。

## Notes / follow-ups

- `Cargo.toml`/`Cargo.lock`/`mise.toml` 無変更。依存追加なし。
- `sha2` crate-local 実装は FIPS ベクタで検証済みだが、将来
  `synvelis-project` が `sha2` に依存する変更が入った場合は
  置き換えを検討する（DRY）。
- duplicate ID は `BTreeMap` 構造上 JSON からは生成不能のため、
  integrity failure は invalid reference / forged watermark /
  project-id mismatch / corrupt bytes の各経路で `InvalidSnapshot`
  として固定した。
