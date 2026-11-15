# sv#458 verification evidence — variant_sets restore-as-copy provenance 修復

Slice: `validate_set_shape` を `ProvenanceRule`
（`SameProject`/`Persisted`）で境界分割し、persisted/load oracle
が restore-as-copy 産 project の source-project provenance を
verbatim 受理するよう修復。`fork_project` は不変、
`stable_id_counts` に `variant-set`/`branch` を追加、
VCS-RESTORE-10 pin を `Published` + envelope readback へ flip。

## Environment

| Field | Value |
| --- | --- |
| Synvelis commit | `dcac0c1dba7c1fa446f9d0a02c1e0f83e84b6a5d` (origin/main) + this change set |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1 (58cf8f9c3), git 2.54.0.windows.1 |
| Cargo.lock Ruviform SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` |
| Schema | `SCHEMA_VERSION = 14` |

## Commands run (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt-check` | 0 | pass（`cargo fmt -p synvelis-project` で整形後に check 成功） |
| `mise run check synvelis-project` | 0 | pass |
| `mise run lint synvelis-project` | 0 | pass |
| `mise run test synvelis-project` | 0 | pass — 全 test binary 成功。`vcs06_variant_matrix` 3/3（新 pin `restore_as_copy_preserves_variant_sets_verbatim` 含む）、`vcs07_variant_diff` 7/7（`variant_changes_stay_fail_closed_on_apply_surface` 非回帰）、`vcs05_restore` 7/7・`vcs06_restore_matrix` 11/11（variant_sets 空 project の restore 非回帰）を含む |
| `mise run test-rt` | 0 | pass（workspace `rt_` filter、全 binary 成功） |
| `mise run build synvelis-project` | 0 | pass |
| `mise run check-full` | 0 | pass（`cargo check --workspace --all-targets` — `synvelis-render`/`synvelis-gateway`/`synvelis-workspace` 等の `validate_references` consumer 非回帰） |
| `mise exec -- prek run --all-files` | 0 | pass |

## Auxiliary evidence (canonical = mise, cargo direct)

`cargo test -p synvelis-project --test vcs06_variant_matrix` —
3 passed / 0 failed（`mise run test synvelis-project` の一部として
も実行済み）。

## Readback 突合（VCS-RESTORE-10 期待形）

`restore_as_copy_preserves_variant_sets_verbatim` が assert:

- `receipt.outcome == RestoreOutcome::Published`、
  `new_project_id == ProjectId::new(777)`、
  `source_project_id == source project`。
- `receipt.stable_ids.counts["variant-set"] == 1`・
  `["branch"] == 2`（`stable_id_counts` 新規 entry）。
- HEAD → snapshot envelope readback:
  `envelope.project_id`/`envelope.project.project_id` が
  `new_project_id`、`envelope.project.variant_sets` が
  `c.live().variant_sets` と **map 全体 verbatim 一致**
  （`VariantSetId`/`BranchId` key・`revision`・
  `BranchContentRef`・`BranchLineage`・`last_adopt`・
  `origin_objects`・`scope`・`adopted`・`origin`/
  `created_from` 全 `SnapshotIdentity` field）。
- `restored_set.origin.project_id`・全 branch の
  `created_from.project_id` が **source project を指し続ける**
  — provenance は rewrite されない（設計意図）。
- `envelope.project.validate_references()` が `Ok` — load oracle
  が restore 産 project を受理（open/load で拒否されない）。
- source project・`live_revision` 非破壊。

## NotRun

- `*-msrv` 系 — Daily Full CI 委譲（ユーザー指定）。
- `mise run lint-full`/`test-full`/`build-full`/`verify-full` —
  タスク固有の必須検証に含まれず未実行。
- 実 GUI/実 OS durability/consumer/precision lanes —
  synvelis#97 統合 lane（ledger `VCS-RESTORE-REAL-*` は
  `NotRun` のまま）。

## Known limitation（修復しない・正直記録）

`BranchContentRef::Captured.payload_digest` の payload bytes は
`BranchContentStore`（project bundle 外）にあり、restore-as-copy
は fork 先へ bytes を複製しない（synvelis#457 Deferred・別設計）。
したがって復元 project の非採用 candidate branch は
`decode_candidate` で `CandidateUnavailable` になりうる。
live-mirror adopted branch（`BranchContentRef::Live`）と set
構造自体は verbatim 有効。ledger `VCS-RESTORE-10` の
Recovery/Rollback 欄に記録済み。
