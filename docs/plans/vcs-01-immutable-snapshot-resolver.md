---
id: "PLAN-034UAzWU1ltpUXPw8mRuWd"
type: "plan"
status: "approved"
title: "VCS-01 Immutable Snapshot Resolver implementation plan"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/92"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-01-immutable-snapshot-resolver-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/vs-a-project-recording-foundation.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/designs/vcs-01-immutable-snapshot-resolver-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-01-snapshot-resolver-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-01 Immutable Snapshot Resolver implementation plan

## Goal

`synvelis-project` に VCS 非依存の Immutable Snapshot Resolver
（Live/Saved/Recovery/VCS Bytes Port・read-only schema
projection）を additive module として実装し、Issue #92 の
headless fixture 受入を固定する。

## Scope

単一 implementation stream。file map は `synvelis-project`
中心。`synvelis-app`/`workspace`/`library`/`gateway`/`player`/
`recording`/`render`/`theme`/poc・Ruviform・実 Git/実 OS/GUI
は対象外（spec の Out of Scope）。

## Upstream Documents

- `docs/specs/vcs-01-immutable-snapshot-resolver.md`
- Issue #92 / Issue #7（設計正本）

## Design Inputs

- `docs/designs/vcs-01-immutable-snapshot-resolver-design.md`

## File Map

- `crates/synvelis-project/src/snapshot/mod.rs` — resolver
  orchestration・`SnapshotError`・`SnapshotCancel`・
  `SnapshotLimits`
- `crates/synvelis-project/src/snapshot/types.rs` — locator/
  identity/snapshot/projection/validation/manifest 型
- `crates/synvelis-project/src/snapshot/digest.rs` — SHA-256 +
  `ContentDigest`
- `crates/synvelis-project/src/snapshot/vcs.rs` — `VcsBytesPort`
  契約と関連型
- `crates/synvelis-project/src/lib.rs` — module 公開 + re-export
- `crates/synvelis-project/src/controller.rs` —
  `snapshot_id_watermark` accessor のみ追加
- `crates/synvelis-project/tests/vcs01_snapshot.rs` — 受入
  fixture tests

## Implementation Sequence

- [x] doc chain（spec/design/plan/test-spec/task）
- [ ] `digest.rs`: SHA-256 compression + `ContentDigest` parse/
  format + FIPS unit test
- [ ] `types.rs` + `vcs.rs`: 全公開型・port trait
- [ ] `mod.rs`: resolver pipeline（capture_live/resolve_saved/
  resolve_recovery/resolve_vcs）+ `SnapshotError`
- [ ] `lib.rs`/`controller.rs`: 公開配線
- [ ] `tests/vcs01_snapshot.rs`: 受入 fixture
- [ ] `mise run fmt` → `fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`test-msrv` + app-native lanes
- [ ] impl record + 最終検証・merge・PR

## Dependencies and Risks

- `migration::migrate_to_current` は `pub(crate)` — 同一 crate
  内のためそのまま利用可。`validate_references`/
  `check_id_watermark` は `pub(crate)`/`pub` で load と同一
  oracle を共有。
- 依存追加なし（SHA-256 は crate 内実装）。`Cargo.toml`/
  `Cargo.lock`/`mise.toml` 不変。
- `InMemoryStore`/`DirectoryBundleStore` は変更しない —
  resolver は `&dyn ProjectStore` の read method のみ使用。

## Task Breakdown

- `docs/tasks/vcs-01-task1-immutable-snapshot-resolver-headless-slice.md`
  単一 task（file map が小さく依存が直列のため分割しない）。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `check-app-native` /
  `lint-app-native` / `build-app-native`
- [ ] test-spec `vcs-01-snapshot-resolver-fixtures` の全
  Feature/Rule/Example を `tests/vcs01_snapshot.rs` で網羅
- [ ] read-only oracle: resolve 前後で controller
  revision/undo/saved・store HEAD/snapshot bytes・VCS fake
  port の呼出し記録が不変であることを assert

## Open Questions / Gaps

- 実 Git provider・実 FS atomicity・実 plugin host は
  NotRun/Unsupported（VCS-03 #94 / VCS-06 #97 の責務）。
- Plugin opaque state の `Known` 昇格には将来の probe が必要
  （本 slice では `Resolved` → `Unknown` 固定）。
