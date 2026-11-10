---
id: "TASK-034UAAcPSS47j6GIlPGPrX"
type: "task"
status: "in-progress"
title: "VCS-01 Task1 Immutable Snapshot Resolver headless slice"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-01-immutable-snapshot-resolver.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vcs-01-immutable-snapshot-resolver.md"
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
    - "docs/test-specs/vcs-01-snapshot-resolver-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-01 Task1 Immutable Snapshot Resolver headless slice

## Work

plan `vcs-01-immutable-snapshot-resolver` /
design `vcs-01-immutable-snapshot-resolver-design` /
test-spec `vcs-01-snapshot-resolver-fixtures` に従い、
`synvelis-project` に Immutable Snapshot Resolver を実装する。

1. `src/snapshot/digest.rs`: SHA-256 compression +
   `ContentDigest`（`"sha256:<hex>"` tag 付き）+ FIPS unit test。
2. `src/snapshot/types.rs` + `src/snapshot/vcs.rs`:
   `SnapshotLocator`/`FixedSnapshotLocator`/`SnapshotIdentity`/
   `SnapshotOrigin`/`ImmutableSnapshot`/`ReadOnlyProjectProjection`/
   `SnapshotValidation`/`AssetAvailabilityManifest`/
   `OpaqueStateRef`/`SnapshotCompleteness`/`ContentStatus`/
   `IndexGeneration`/`BlobRead`/`VcsFileRead`/`VcsLocator`/
   `VcsPortError`/`VcsBytesPort`/`BlobProbe`。
3. `src/snapshot/mod.rs`: `SnapshotResolver`
   （`capture_live`/`resolve_saved`/`resolve_recovery`/
   `resolve_vcs`/`resolve_saved_head`）・`SnapshotError`・
   `SnapshotCancel`・`SnapshotLimits`。
4. `src/lib.rs`: `pub mod snapshot` + re-export。
5. `src/controller.rs`: `snapshot_id_watermark` accessor のみ。
6. `tests/vcs01_snapshot.rs`: test-spec SNAP-FIX-01〜10 の
   fixture tests。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/vcs-01-immutable-snapshot-resolver.md`
- `docs/specs/vcs-01-immutable-snapshot-resolver.md`
- `docs/designs/vcs-01-immutable-snapshot-resolver-design.md`
- Issue #92 / Issue #7

## Dependencies

- なし（着手条件 Blocked by なし）。`migration` ladder・
  `ProjectStore`・`ProjectController` の既存契約を再利用。

## Affected Files

- `crates/synvelis-project/src/snapshot/`（新規 dir）
- `crates/synvelis-project/src/lib.rs`
- `crates/synvelis-project/src/controller.rs`（accessor のみ）
- `crates/synvelis-project/tests/vcs01_snapshot.rs`（新規）
- `docs/` chain（spec/design/plan/test-spec/task/impl）

## Done When

- [ ] test-spec の全 Rule/Example が `tests/vcs01_snapshot.rs`
  で Pass。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv` + app-native lanes が Pass。
- [ ] 依存追加・`Cargo.toml`/`Cargo.lock`/`mise.toml` 変更なし。
- [ ] read-only oracle（controller/store/VCS port 不変）を
  test で assert。

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check`
- [ ] `mise run lint`
- [ ] `mise run test`
- [ ] `mise run test-rt`
- [ ] `mise run test-msrv`
- [ ] `mise run check-app-native` / `lint-app-native` /
  `build-app-native`
