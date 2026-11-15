# sv#457 verification evidence — variant_sets VCS semantic diff

Slice: `variant_sets` を VCS-02 semantic diff の分類対象に追加
（`ObjectKind::VariantSet`/`VariantBranch`・`ComparisonDomain::
Variants`・`VariantsAdapter`・`raw_kind_map` 登録・policy v4）。

## Environment

| Field | Value |
| --- | --- |
| Synvelis commit | `34387c4e31afb90d7e0d5266d4e87d81d4e8b329` (origin/main) + this change set |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1 (58cf8f9c3), git 2.54.0.windows.1 |
| Cargo.lock Ruviform SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38` |

## Commands run (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt-check` | 0 | pass（`mise run fmt` で整形後に check 成功） |
| `mise run check synvelis-project` | 0 | pass |
| `mise run lint synvelis-project` | 0 | pass（初回 `clippy::too_many_lines`/`doc_markdown`/`needless_lifetimes`/`match_wildcard_for_single_variants`/`assert_is_empty` を修正して再実行） |
| `mise run test synvelis-project` | 0 | pass — 全 test binary 成功。`vcs07_variant_diff` 7/7、`vcs06_variant_matrix` 3/3、`vcs02_diff` 36/36 を含む |
| `mise run build synvelis-project` | 0 | pass |
| `mise run test-rt` | 0 | pass（workspace `rt_` filter、全 binary 成功） |
| `mise run check-full` | 0 | pass（`cargo check --workspace --all-targets` — `synvelis-render`/`synvelis-workspace` 等の `ObjectKind`/`ComparisonDomain` consumer 非回帰） |
| `mise exec -- prek run --all-files` | 0 | pass |

## Auxiliary evidence (canonical = mise, cargo direct)

`cargo test -p synvelis-project --test vcs07_variant_diff --test
vcs06_variant_matrix` — vcs07 7 passed / vcs06 3 passed
（`mise run test synvelis-project` の一部としても実行済み）。

## NotRun

- `*-msrv` 系 — Daily Full CI 委譲（ユーザー指定）。
- 実 GUI/実 OS/consumer/precision lanes — ledger の
  NotRun/Unsupported 記載どおり。
- `mise run lint-full`/`test-full`/`build-full`/`verify-full` —
  タスク固有の必須検証に含まれず未実行。
