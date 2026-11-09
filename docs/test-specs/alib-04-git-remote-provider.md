---
id: "TESTSPEC-alib04"
type: "test-spec"
status: "approved"
title: "ALIB-04 Git Remote Provider — テスト仕様"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  verifies:
    - "docs/specs/alib-04-git-remote-provider.md"
    - "docs/designs/alib-04-git-remote-provider-design.md"
  verified-by:
    - "crates/synvelis-library/tests/git.rs"
    - "crates/synvelis-library/tests/common/git_fixture.rs"
---

# ALIB-04 Test Spec

`crates/synvelis-library/tests/git_fixture.rs` が temp dir に
work repo + bare remote + `file://` mirror を `git`/`git-lfs`
CLI で構築する（remote に `uploadpack.allowFilter`/
`allowAnySHA1InWant` を設定し、file:// transport で
partial clone + lazy fetch を有効化）。`tests/git.rs` が
`AssetProvider` 契約・selective fetch・LFS・pin 固定・
failure 分類・redaction・import 接続を検証する。fixture は
実 git binary を使うため実装経路の証拠になるが、実
Git server/network・実 OS・GUI は対象外（NotRun/
Unsupported）。`git`/`git-lfs` 非存在環境では fixture が
`None` を返し test は明示 skip する。

## Feature: 固定 Commit/Tree browse

- Rule: Mutable branch/ref は pin で実 Commit SHA+OID に
  固定。browse は固定 commit の tree を 1 level 列挙し、
  paging 中に remote が動いても対象 commit は変わらない。
- Example: `browse_root_lists_tree_entries`（tree/blob/
  gitlink/symlink 分類）、`browse_subtree_via_locator`、
  `paging_cursor_pins_commit`（page1 後に force-push→
  page2 が旧 commit のまま）、`branch_update_requires_
  refresh`（push 後は refresh まで旧 sha）。

## Feature: Selective fetch

- Rule: 選択した object のみ取得。repo 全量・選択外 media の
  無断同期をしない。
- Example: `selective_fetch_fetches_only_pinned_objects`
  （mirror の local object 列挙で選択 blob のみ存在・
  未選択 blob は promisor 欠落のまま）、`non_selected_media_
  not_fetched`。

## Feature: Git LFS 実 Byte

- Rule: LFS Pointer は本体ではない。oid(SHA-256)+size と
  実 Download Byte を照合し、欠落/不一致/認証不可を
  Verified に昇格しない。
- Example: `lfs_pointer_resolves_to_real_bytes`（pointer→
  `git-lfs:<oid>` pin→`lfs/objects` file→sha256+size 検証
  →import）、`lfs_missing_object_fails`、`lfs_corrupt_object_
  integrity_mismatch`、`lfs_size_field_mismatch`、
  `pointer_only_is_not_verified`。

## Feature: Pin 固定性と Import 接続

- Rule: pin は実 Commit SHA+Blob/LFS OID を保持し、
  `open_pinned`/`import_verified` は remote 更新後も同じ
  byte を返す。Project/Working Tree/Index/HEAD は不変。
- Example: `pin_survives_branch_update_and_force_push`、
  `same_name_different_object_distinguished`（同 path 別
  commit で oid/bytes が異なる）、`import_verified_round_
  trip_offline`（pin→ImportIntent→staging→verify→
  publish→commit→blob 再読）、`deletion_makes_rev_unresolvable`。

## Feature: Failure 分類

- Rule: LFS missing・SHA/size mismatch・auth expired・
  切断・cancel・retry の結果が typed error として正確。
- Example: `auth_expired_via_credential_key`（resolver 失敗
  →`AuthExpired`/`AfterAuth`）、`cancel_returns_cancelled`、
  `missing_rev_is_not_found`、`unsupported_filter_policy_
  reject`/`allow_full_mirror`、`stderr_classification_table`
  （純粋関数の unit test で offline/ratelimited/auth/
  notfound/transient を分類）。

## Feature: Security / redaction

- Rule: token を URL・argv・Provenance・log・error detail に
  残さない。任意 git command を shell 連結しない。
- Example: `url_rejects_userinfo_and_bad_scheme`、
  `credential_not_in_argv_or_locator`（resolved 値が
  RemoteAssetRef/PinnedAssetRef/manifest/log に出ない）、
  `git_argv_is_fixed_array`（構築される argv に shell
  metachar を含まない・`--` separator 使用）、
  `path_traversal_rejected`（`..`/絶対/NUL/`:` を含む
  locator を `InvalidRequest`）。

## Feature: Capability / partiality

- Rule: Provider Capability と検索結果の部分性、
  `GitProviderInfo` の mirror mode/object format/LFS
  backend/filter 対応を正直に宣言。
- Example: `capabilities_declared`（search=Unsupported
  RequiresIndex・pin=ImmutableObject・access=ReadOnly）、
  `git_provider_info_reports_mirror_mode`、
  `browse_partial_on_budget`（小さい `max_entries` で
  `Coverage::Partial`）。

## Feature: 非回帰

- Rule: 既存 ALIB-01/02/03 test が全て pass。AudioLens/RT/
  Golden に影響しない。
- Example: `mise run test`/`test-rt`/`test-msrv` 全 pass、
  `tests/contract.rs`/`cache.rs`/`import.rs`/`local.rs`/
  `project.rs`/`types.rs`/`containment_swap.rs` 非退行。
