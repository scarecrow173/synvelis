# Verification — synvelis#97 VCS-06 variant-domain matrix slice

Date: 2026-11-15. Slice: schema v14 `Project::variant_sets` の
VCS-06 fixture matrix 拡張（正直記録型 — `src/` 変更なし）。
Canonical 検証は `mise run` 経由。base: `origin/main` @
`3da82c5`（post-#459）、Cargo.lock Ruviform pin `fd1d4d5`。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `3da82c5` + 本 slice 差分（ledger/impl-ir に同記録） |
| OS / FS | Windows 11 x64 / NTFS |
| Toolchain | rustc 1.99.1（`58cf8f9c3`・2026-10-13）、git 2.54.0.windows.1 |
| Project schema | v14（`SCHEMA_VERSION = 14`、`Project::variant_sets` 永続化済み） |

## 実行結果（実測）

| コマンド | OS | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | Windows 11 | Pass（exit 0） |
| `mise run check synvelis-project` | Windows 11 | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-project` | Windows 11 | Pass（exit 0、clippy `-D warnings`） |
| `mise run test synvelis-project` | Windows 11 | Pass（exit 0 — 全 suite 緑。`vcs06_variant_matrix.rs` 3 tests を含み既存 vcs06 suite 非回帰） |
| `mise run test-rt` | Windows 11 | Pass（exit 0、約 6m54s） |
| `mise run build synvelis-project` | Windows 11 | Pass（exit 0） |
| `mise exec -- prek run --all-files` | Windows 11 | Pass（exit 0） |

## 補助証拠（canonical = mise）

- `cargo test -p synvelis-project --test vcs06_variant_matrix` —
  3 passed。
- `cargo clippy -p synvelis-project --all-targets -- -D warnings`
  — clean。
- `cargo fmt --all -- --check` — clean。
- `node .agents/skills/doc-status/scripts/doc_status.js lint
  --json` — blocking findings 693（変更前 baseline と同一。
  本 slice の新規 doc に blocking 0。non-blocking の
  reciprocal-relation warning 1 件は `docs/discovery/
  variant-sets-vcs-diff-gap.md` の `derives-from` →
  `plugin-instances-vcs-diff-gap.md` で、対象 file は本 slice
  の編集範囲外のため残置）。

## NotRun

- `mise run test-msrv` — **NotRun**（POLICIES.md §5・ユーザー
  指定どおり Daily CI（daily-ci→ci.yml）へ委譲。必須検証に
  含めない）。
- `*-full` / Feature Matrix / 3OS 横断 / 実 GUI・実 OS・実
  provider/remote・plugin host・RT・consumer・CI gate —
  タスク指定により対象外（外部 gate・後続 slice）。

## Case 判定（ledger 対応）

| Case | 判定 | 根拠 |
| --- | --- | --- |
| VCS-DIFF-11（variant-only diff） | `Blocked` | `variant_set_presence_and_mutation_are_invisible_to_diff` — `identity.digest` 差・`variant_sets` 実差があるのに `overall=Equal`・`completeness=Complete`・0 changes・0 unresolved。`ObjectKind` に variant kind 無し・`raw_kind_map` 未列挙の compare-surface gap（synvelis#457）。 |
| VCS-DIFF-11 mixed（variant + track rename） | `Blocked` | `mixed_diff_reports_track_change_but_drops_variant_set` — `Different`・changes len=1（Track `Modified` のみ）で variant 差分は silent drop。 |
| VCS-APPLY-10 | `Blocked` | diff が variant `SemanticChange` を emit しないため apply 対象の change object が存在しない（fixture 未実行・honest N/A）。 |
| VCS-RESTORE-10 | `Fail` | `restore_as_copy_fails_closed_on_variant_set_origin_mismatch` — `RestoreError::Validation`（`origin belongs to a different project`）で publish 前 fail-closed。staging 保持・dest 不在・source 不変を assert。`fork_project` が top-level `project_id` のみ再採番し nested `SnapshotIdentity.project_id` が source のまま残るため `validate_set_shape` が拒否（synvelis#458）。 |

silent `Equal`・silent drop・restore 拒否を Pass に繰り上げて
いない。envelope readback による verbatim 確認は restore 成功が
前提のため #458 修復後の follow-up。
