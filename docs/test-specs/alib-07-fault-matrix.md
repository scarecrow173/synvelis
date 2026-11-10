---
id: "TESTSPEC-alib07"
type: "test-spec"
status: "approved"
title: "ALIB-07 Asset Library 全経路 Fault Matrix — テスト仕様・証拠 matrix（headless slice）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  verifies:
    - "docs/specs/alib-07-fault-matrix.md"
    - "docs/designs/alib-07-fault-matrix-design.md"
    - "docs/specs/alib-07-import-cancel.md"
  verified-by:
    - "crates/synvelis-library/tests/git.rs"
    - "crates/synvelis-library/tests/webdav.rs"
    - "crates/synvelis-library/tests/import.rs"
    - "crates/synvelis-library/tests/import_recovery.rs"
    - "crates/synvelis-library/tests/cache.rs"
    - "crates/synvelis-library/tests/contract.rs"
    - "crates/synvelis-library/tests/local.rs"
    - "crates/synvelis-library/tests/project.rs"
    - "crates/synvelis-library/tests/rt_cache.rs"
    - "crates/synvelis-library/tests/containment_swap.rs"
    - "crates/synvelis-library/tests/common/git_fixture.rs"
    - "crates/synvelis-library/tests/common/webdav_fixture.rs"
    - "crates/synvelis-recording/tests/input_adapter.rs"
    - "crates/synvelis-recording/tests/input_adapter_rt_alloc.rs"
    - "crates/synvelis-recording/tests/monitor_gate.rs"
    - "crates/synvelis-project/tests/routing_sf01.rs"
    - "crates/synvelis-project/tests/vcs01_snapshot.rs"
    - "crates/synvelis-project/tests/vcs02_diff.rs"
    - "crates/synvelis-workspace/tests/preset.rs"
    - "crates/synvelis-player/tests/sf03_l1.rs"
---

# ALIB-07 Test Spec — Fault Matrix（headless slice）

Issue #91 固定の Case ID（LIB-ID 01〜04 / LIB-BROWSE 01〜04 /
LIB-PIN 01〜05 / LIB-IMPORT 01〜10 / LIB-MIG 01〜05 /
LIB-CACHE 01〜05 / LIB-SEC 01〜05 / LIB-QOS 01〜04、計 42 class）
に対し、headless で再現可能な fault/golden case の Expected /
Observed / Status を記録する。

## 実行環境（Baseline）

- Synvelis commit: `d867f88`（branch `scarecrow173/sv91-alib07`、
  `origin/main` と同一 HEAD から分岐）
- Slice 2 追記（branch `feature/91-alib07-cancel-slice`、base
  `ae5ddc4a` = PR #253 merge）: LIB-IMPORT-04 の publish/commit 相
  cancel と LIB-IMPORT-08 の実 subprocess crash を追加。先行
  evidence は PR #236（headless matrix 本体、
  https://github.com/scarecrow173/synvelis/pull/236）と PR #253
  （v5 digest migration + import_recovery fixture、
  https://github.com/scarecrow173/synvelis/pull/253）。
- 関連 Issue/PR（本 matrix の NotRun 根拠・別 slice の責務）:
  synvelis#84（File/FS publish/HEAD/crash/recovery cross-gate）、
  synvelis#97（VCS immutable history/missing LFS）、
  synvelis#89・#114（Browser/import UX・実 GPUI window）、
  synvelis#83（独立 CI）、Ruviform#219・#294（consumer 互換・
  独立 gate）。
- Ruviform resolved SHA（`Cargo.lock`）:
  `0f5b57372e9a43f84415f7964b2f9636774174f0`
- Toolchain: rustc 1.99.0（MSRV 1.89）
- OS: Windows（開発機）。Linux/macOS 実機差は `NotRun`
- Fixture: `GitFixture`（実 git CLI + bare `file://` remote +
  手置き LFS objects）、`webdav_fixture::MockServer`
  （in-process `TcpListener` scripted HTTP/1.1）、
  `ScriptedProvider`/`ScriptedPublisher`/`CancelOnReadProvider`/
  `ArmOnRead`（import fault 注入）
- Status 凡例: `Pass` = headless で Expected を観測し green、
  `Fail` = Expected 不成立、`NotRun` = headless slice では
  実施不能（実環境/実装未了）、`Unsupported` = provider/transport
  が能力を持たない、`NotImplemented` = provider 未実装。
  `NotRun`/`Unsupported`/`NotImplemented`/`Unknown` は Pass に
  昇格しない。

## Feature: LIB-ID — Source/Asset Identity

- Rule: 同一 byte 列を異 provider が供給しても blob は内容
  アドレスで共有されるが asset/source identity は provider 別に
  発行される。Remote rename/branch move は既存 import・pin を
  変えない。symlink/race は root 脱出も誤同定もしない。
  Project provider は read-only projection。
- Evidence: `tests/import.rs::same_bytes_across_providers_share_one_verified_blob`、
  `tests/git.rs::remote_rename_keeps_pin_and_fixed_commit_identity`、
  `tests/git.rs::pinned_commit_survives_later_remote_updates`、
  `tests/local.rs::renamed_or_deleted_file_cannot_open_pinned`、
  `tests/local.rs::symlink_escaping_root_is_listed_but_never_opened`、
  `tests/containment_swap.rs::local_*_component_swap_*`、
  `tests/project.rs::browse_projects_every_asset_with_blob_facts`。

## Feature: LIB-BROWSE — cursor/stale/partial/principal

- Rule: cursor は query/principal/generation に束縛され、
  stale/foreign/garbage は `CursorInvalid`。offline/partial は
  `Coverage::Partial` または typed error であり empty 偽装
  しない。search 非対応は `Unsupported`（empty ではない）。
- Evidence: `tests/git.rs::paging_walks_the_whole_tree_deterministically`、
  `tests/git.rs::browse_cursor_survives_remote_update_within_session`、
  `tests/git.rs::cursor_rejected_on_principal_query_or_garbage_change`、
  `tests/git.rs::search_is_typed_unsupported_not_fake_empty`、
  `tests/git.rs::deleted_paths_and_branches_fail_typed_after_refresh`、
  `tests/git.rs::offline_remote_typed_failures_mirror_stays_honest`、
  `tests/webdav.rs::browse_cursor_pages_listing`、
  `tests/webdav.rs::browse_partial_on_foreign_and_unaddressable_hrefs`、
  `tests/webdav.rs::offline_server_fails_typed_on_every_op`、
  `tests/cache.rs::catalog_cursor_rejects_foreign_and_stale`、
  `tests/cache.rs::catalog_principal_isolation_and_disk_layout`、
  `tests/cache.rs::catalog_offline_and_transient_degrade_honestly`、
  `tests/project.rs::cursor_dies_when_project_identity_changes`、
  `tests/project.rs::principal_change_invalidates_cursor`。

## Feature: LIB-PIN — fixed identity / conditional read

- Rule: Git pin は immutable object（commit/tree/blob oid）に
  束縛され LFS は実 byte の SHA-256 で検証する。WebDAV pin は
  strong ETag + `If-Match` のみ受理し weak/missing は fail
  closed。mutable branch 更新・auth revoke・permission denied は
  typed error。
- Evidence: `tests/git.rs::browse_root_lists_fixed_commit_tree`、
  `tests/git.rs::blob_pin_open_roundtrip_is_identity_bound`、
  `tests/git.rs::lfs_pointer_pin_fetches_and_verifies_real_object`、
  `tests/git.rs::lfs_missing_object_is_not_found_not_pointer`、
  `tests/git.rs::lfs_size_mismatch_and_corruption_fail_verification`、
  `tests/git.rs::refresh_makes_remote_updates_visible`、
  `tests/git.rs::force_push_keeps_pinned_objects_fetchable`、
  `tests/webdav.rs::pin_strong_etag_produces_if_match_proof`、
  `tests/webdav.rs::pin_rejects_weak_and_missing_etags`、
  `tests/webdav.rs::open_pinned_sends_if_match_and_verifies`、
  `tests/webdav.rs::open_pinned_412_is_precondition_failed`、
  `tests/webdav.rs::open_pinned_response_etag_mismatch_is_source_changed`、
  `tests/webdav.rs::auth_revoked_between_pin_and_conditional_get`、
  `tests/webdav.rs::status_mapping_table`、
  `tests/git.rs::credential_failures_are_typed_and_secrets_never_leak`。

## Feature: LIB-IMPORT/MIG — verified import・migration

- Rule: chunked stream を bounded memory で SHA-256 計測し、
  staged readback・publish trichotomy・atomic commit を経る。
  同一 ImportId は冪等/conflict、cancel/crash は manifest から
  resume、revision/target 変更は typed reject。v5 snapshot と
  SHA-256 namespace の共存を確認し、未移行の CRC 昇格を
  行わない。
- Evidence: `tests/import.rs` 全件 + `cancel_mid_stream_is_typed_and_same_import_resumes`、
  `same_bytes_across_providers_share_one_verified_blob`、
  `tests/git.rs::import_verified_consumes_git_blob_and_lfs_pins`、
  `tests/project.rs`（missing blob typed）、
  `synvelis-project` v5 migration tests、
  `synvelis-recording`/`synvelis-player` golden tests。

## Feature: LIB-CACHE/SEC — cache 分離・安全性

- Rule: principal/credential で cache を分離し、TTL/LRU/lease で
  bounded eviction、auth revoke/clear は typed retire。
  traversal/symlink/redirect/SSRF/credential forwarding/
  malformed response/log redaction/git argv を fail closed で
  拒否する。
- Evidence: `tests/cache.rs` 全件、`tests/webdav.rs` の
  security 系 test、`tests/git.rs::hostile_locator_and_url_inputs_fail_closed`、
  `tests/local.rs` traversal/symlink、`tests/containment_swap.rs`、
  `tests/types.rs::opaque_credential_debug_redacts_context`。

## Feature: LIB-QOS — RT callback 予算

- Rule: RT callback 経路は alloc/dealloc/network/disk/block を
  行わない。headless では allocator 計測と queue/drop 会計で
  証明し、実 device/OS/GUI は別記録とする。
- Evidence: `tests/rt_cache.rs::rt_event_enqueue_is_alloc_and_dealloc_free`、
  `tests/rt_cache.rs::rt_queue_is_bounded_and_accounts_drops`、
  `tests/rt_cache.rs::suppressing_sink_drops_stale_generations`、
  `synvelis-recording` `rt_callback_is_alloc_and_drop_free` 等。

## 証拠 matrix（Case ID 別 Expected/Observed/Status）

凡例の Status は spec 記載の通り。Observed の test 名は
`<file>::<test>` 形式（`tests/` 配下は `crates/synvelis-library/`）。
workspace 横断の証拠は crate 名を前置する。

### LIB-ID（Source/Asset Identity）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| LIB-ID-01 同Byte/異Provider→別AssetId・Blob共有 | byte 同一・provider 相違の 2 import が同一 sha256 blob を `Reused` で共有し asset/source は別 id | `import.rs::same_bytes_across_providers_share_one_verified_blob`（Pass: Reused・digest/identity 一致・id 相違） | Pass |
| LIB-ID-02 Remote rename/branch move→既Import不変 | remote rename 後に旧 path は NotFound、新 path 解決、既存 pin/fixed-commit locator は不変 byte | `git.rs::remote_rename_keeps_pin_and_fixed_commit_identity`、`git.rs::pinned_commit_survives_later_remote_updates`、`local.rs::renamed_or_deleted_file_cannot_open_pinned`（Pass） | Pass |
| LIB-ID-03 Local path symlink/race | symlink は root 内のみ辿り、escape/loop/祖先経由脱出を typed に拒否。実 FS race window は headless 範囲外 | `local.rs::symlink_into_root_is_browsable_and_marked`、`symlink_escaping_root_is_listed_but_never_opened`、`ancestor_symlink_cannot_leak_out_of_locator_scope`、`containment_swap.rs::local_*_component_swap_*`（Pass: 差替え race は component swap で注入）。OS レベル TOCTOU 実 race は NotRun | Pass（一部 NotRun 注記） |
| LIB-ID-04 Project Asset read-only projection | project provider は blob facts/provenance を投影し write 経路を持たない | `project.rs::browse_projects_every_asset_with_blob_facts`、`stat_reports_provenance_and_blob_facts`、`imported_via_provenance_is_projected`（Pass） | Pass |

### LIB-BROWSE（cursor/stale/partial/principal）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| LIB-BROWSE-01 cursor pagination/continuation | page_limit 分割・next_cursor で全件一度ずつ、remote 更新後も cursor は旧 epoch に束縛 | `git.rs::paging_walks_the_whole_tree_deterministically`、`git.rs::browse_cursor_survives_remote_update_within_session`、`webdav.rs::browse_cursor_pages_listing`、`local.rs::paging_walks_all_entries_once_without_dupes`（Pass） | Pass |
| LIB-BROWSE-02 search Unsupported vs empty | index 非対応 provider は `Unsupported` typed であり empty 結果を偽装しない | `git.rs::search_is_typed_unsupported_not_fake_empty`、`project.rs::search_is_typed_unsupported_not_empty`、`webdav.rs::capabilities_declare_contract`（Pass） | Pass |
| LIB-BROWSE-03 stale/offline partial/never indexed | offline は typed `Offline`/mirror の正直な stale、partial は `Coverage::Partial`、未 index を Complete と偽らない | `git.rs::offline_remote_typed_failures_mirror_stays_honest`、`git.rs::deleted_paths_and_branches_fail_typed_after_refresh`、`webdav.rs::offline_server_fails_typed_on_every_op`、`webdav.rs::browse_partial_on_foreign_and_unaddressable_hrefs`、`webdav.rs::browse_partial_when_entry_cap_truncates`、`cache.rs::catalog_offline_and_transient_degrade_honestly`、`cache.rs::catalog_partial_and_page_budget_completeness`（Pass） | Pass |
| LIB-BROWSE-04 principal scope 変更→cursor invalidation | principal/query/scope/generation 変更で cursor は `CursorInvalid` | `git.rs::cursor_rejected_on_principal_query_or_garbage_change`、`cache.rs::catalog_cursor_rejects_foreign_and_stale`、`cache.rs::catalog_principal_isolation_and_disk_layout`、`project.rs::cursor_dies_when_project_identity_changes`、`project.rs::principal_change_invalidates_cursor`（Pass） | Pass |

### LIB-PIN（fixed identity / conditional read）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| LIB-PIN-01 Git fixed commit/tree | pin は commit/tree/blob oid の immutable identity、locator `<sha>:<path>` で固定 commit を解決 | `git.rs::browse_root_lists_fixed_commit_tree`、`git.rs::blob_pin_open_roundtrip_is_identity_bound`、`git.rs::browse_subtree_scopes_and_odd_names`（Pass: file:// offline fixture。実 git remote/SSH/HTTP transport は NotRun） | Pass |
| LIB-PIN-02 Git LFS 実 Byte | LFS pointer は内容として返さず、remote `lfs/objects` の実 byte を SHA-256 で検証。missing/corrupt は typed | `git.rs::lfs_pointer_pin_fetches_and_verifies_real_object`、`lfs_missing_object_is_not_found_not_pointer`、`lfs_size_mismatch_and_corruption_fail_verification`、`import.rs` 経由 `git.rs::import_verified_consumes_git_blob_and_lfs_pins`（Pass: file:// store 直読み。LFS batch API/HTTP download endpoint は Unsupported/NotRun） | Pass |
| LIB-PIN-03 WebDAV If-Match strong/weak | strong ETag のみ pin 可、`If-Match` conditional GET、412→PreconditionFailed、response ETag/CL 不一致→SourceChanged、weak/missing ETag は fail closed | `webdav.rs::pin_strong_etag_produces_if_match_proof`、`pin_rejects_weak_and_missing_etags`、`pin_rejects_collection`、`open_pinned_sends_if_match_and_verifies`、`open_pinned_412_is_precondition_failed`、`open_pinned_response_etag_mismatch_is_source_changed`、`open_pinned_rejects_foreign_proof`（Pass: scripted mock。実 WebDAV server は NotRun） | Pass |
| LIB-PIN-04 mutable branch update | refresh で remote 更新が見え、pin は旧 immutable bytes を保持、force-push 後も取得済み object は fetchable | `git.rs::refresh_makes_remote_updates_visible`、`git.rs::force_push_keeps_pinned_objects_fetchable`、`git.rs::remote_rename_keeps_pin_and_fixed_commit_identity`（Pass） | Pass |
| LIB-PIN-05 auth revoke / permission denied | 401→AuthExpired、403→PermissionDenied、credential 解決失敗は typed、secret は error/log に漏れない | `webdav.rs::auth_revoked_between_pin_and_conditional_get`、`webdav.rs::status_401_maps_auth_expired`、`webdav.rs::status_mapping_table`、`webdav.rs::missing_resolver_is_auth_expired`、`git.rs::credential_failures_are_typed_and_secrets_never_leak`（Pass: mock/scripted resolver。OS credential store 実機は NotRun） | Pass |

### LIB-IMPORT（chunked verified import）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| LIB-IMPORT-01 chunked huge bounded memory | provider chunk サイズに依らず固定 buffer のみで stream、1MiB を 7B ずつ供給して byte-exact | `import.rs::bounded_memory_streaming_with_tiny_chunks`、`import.rs::import_streams_verifies_publishes_and_commits`（Pass。GB 級の実サイズは headless 範囲外として NotRun 注記） | Pass |
| LIB-IMPORT-02 verify/decode/metadata | staged bytes を readback 再計測し digest+size を照合。media decode 検証は consumer の別段階 | `import.rs::digest_mismatch_is_integrity_error`、`import.rs::byte_budget_is_enforced_during_staging`（byte verify Pass。decode/metadata は library slice 対象外 → NotRun） | Pass（一部 NotRun） |
| LIB-IMPORT-03 SHA-256 mismatch | pin 期待 digest と実測の不一致は `IntegrityMismatch`、既存 blob との不一致も同様 | `import.rs::digest_mismatch_is_integrity_error`、`git.rs::lfs_size_mismatch_and_corruption_fail_verification`、`import.rs::same_import_id_different_content_is_conflict`（Pass） | Pass |
| LIB-IMPORT-04 cancellation at each phase | stage/verify/publish/commit 各相の cancel は typed `Cancelled` で manifest が直前の durable stage に留まり resume 可能。commit は `apply` 直前の checkpoint のみ — commit 後に cancel を返さない | `import.rs::cancel_mid_stream_is_typed_and_same_import_resumes`（stage→`Started` 保持）、`import.rs::cancel_during_verify_is_typed_and_same_import_resumes`（verify→`Staged` 保持）、`import.rs::cancel_during_publish_is_typed_and_same_import_resumes`（publish→`Verified` 保持）、`import.rs::cancel_at_commit_boundary_parks_at_published_and_resumes`（commit 直前→`Published` 保持・blob 温存・resume は再 publish なしで commit）、`git.rs::cancellation_is_typed_before_and_during_fetch`、`webdav.rs::cancelled_token_fails_before_io`、`cache.rs::catalog_waiter_honours_cancellation`、`local.rs::cancel_during_directory_enumeration_fails_the_page`、`cache.rs::preview_cancel_and_typed_errors`（Pass: headless cooperative token。OS signal 等の実 cancel 経路は consumer/OS 範囲） | Pass |
| LIB-IMPORT-05 duplicate ImportId same/different digest | same ImportId+same content は冪等 reuse、different content は `IntegrityConflict` で二重登録なし | `import.rs::replay_same_import_is_idempotent`、`import.rs::same_import_id_different_content_is_conflict`（Pass） | Pass |
| LIB-IMPORT-06 Project Revision/Target changed | stale `expected_project_revision` は `StaleRevision` で `rebase_import` のみが解決。存在しない lane/誤 track target は commit 前に reject | `import.rs::stale_revision_rejected_then_explicit_rebase_converges`、`import.rs::missing_lane_target_is_rejected_without_clip`、`import.rs::wrong_track_lane_target_is_rejected`（Pass） | Pass |
| LIB-IMPORT-07 multiple windows | 複数 window/consumer からの同時 import | GUI/consumer slice（#89/#114）の範囲。headless library slice では NotRun | NotRun |
| LIB-IMPORT-08 Blob Published/ACK lost/manifest crash | publish 失敗・indeterminate・dirsync 失敗は typed 記録され manifest から resume、published-uncommitted blob は orphan 保護。実 process crash（publish ACK lost / pre-commit）から別 process が manifest + on-disk blob のみで resume | `import.rs::publish_failure_resumes_from_manifest`、`import.rs::indeterminate_publish_is_reconciled_not_misreported`、`import.rs::directory_sync_failure_is_recorded_not_hidden`、`import.rs::incomplete_job_and_blob_are_protected_from_gc`、`import.rs::subprocess_crash_during_publish_or_commit_resumes_across_processes`（self-exec 子 process が publish ACK lost / pre-commit で `exit` 死亡、親 process が `ack-lost`→`Reused` / `pre-commit`→`Created` で commit 収束・子の予約 ID 一致を assert。実 subprocess crash 証拠。OS クラッシュ/電源断の実 durability は実環境 NotRun — synvelis#84 cross-gate） | Pass |
| LIB-IMPORT-09 Save/HEAD separate | commit は CommittedLive のみで saved()/HEAD を意味しない | `import.rs::import_streams_verifies_publishes_and_commits`（`c.saved().is_none()`・live≠saved を assert、Pass） | Pass |
| LIB-IMPORT-10 offline playback after Cache eviction | cache eviction/clear は project blob（source of truth）を削除しない。実再生は consumer | `cache.rs::cache_clear_and_eviction_preserve_source_of_truth`、`cache.rs::cache_budget_evicts_but_leases_pin`、`cache.rs::cache_open_scan_drops_corrupt_and_half_published`（library slice Pass。実 playback は NotRun） | Pass（一部 NotRun） |

### LIB-MIG（v5 snapshot / namespace 移行）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| LIB-MIG-01 Snapshot v5 read/upgrade | v5 snapshot が現行 schema へ決定的に migrate される | `synvelis-project::routing_sf01::v5_snapshot_migrates_to_legacy_clip_to_master_projection`、`synvelis-project::vcs01_snapshot::v5_snapshot_projects_read_only_into_current_schema`、`synvelis-workspace::preset::legacy_snapshot_shape_migrates_to_preset`（Pass: workspace test lane） | Pass |
| LIB-MIG-02 旧 hex16 CRC vs SHA-256 namespace | 旧 CRC 参照と新 SHA-256 namespaced key が衝突せず、未検証 hash の昇格をしない。移行・分類の実装権威は #86（PR #253）— 本 Issue は吸収せず完了証跡を参照する | `import.rs::sha256_namespace_cannot_collide_with_legacy_flat`（namespace 共存）、`import_recovery.rs::legacy_crc_blob_coexists_with_sha256_namespace`（flat CRC blob が `DigestKind::Crc32Fold`=weak に留まり flat のまま読め、新規 import は sha256 namespace に着地・flat 非破壊 — 未検証 hash の sha256 昇格なし）（Pass。#86/PR #253 の lane を参照し本 slice では再実装しない） | Pass |
| LIB-MIG-03 旧 missing blob/degraded | 参照先 blob 欠落は typed `NotFound` で degraded 扱い、panic/誤 bytes なし | `project.rs::missing_blob_is_typed_missing_not_error`（Pass） | Pass |
| LIB-MIG-04 legacy collision/no unverified hash promotion | flat 名が sha256 namespaced identity を shadow しない | `import.rs::sha256_namespace_cannot_collide_with_legacy_flat`（shadow file case 含む、Pass） | Pass |
| LIB-MIG-05 Plugin Opaque Bytes/ID/Undo/Source/Recording/PCM Golden untouched | 移行で plugin opaque・Undo・source・recording・PCM golden が非回帰 | `synvelis-recording` `mono_stereo_multichannel_pcm_fidelity`、`raw_capture_bytes_are_identical_across_monitor_states`、`synvelis-player::sf03_l1::sf03_02_semantic_audio_matches_legacy_bitexact`、`recording-poc` vsc_process tests（Pass: workspace lane で非回帰確認） | Pass |

### LIB-CACHE（TTL/lease/principal/予算）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| LIB-CACHE-01 account/principal private cache isolation | principal 毎に cache/index を分離、cursor/credential 変更で invalidate | `cache.rs::catalog_principal_isolation_and_disk_layout`、`cache.rs::preview_range_fetch_cached_and_principal_isolated`、`cache.rs::catalog_credential_change_invalidates`（Pass） | Pass |
| LIB-CACHE-02 TTL/LRU/active read lease | budget 超過は LRU evict するが lease 中 entry は保持、corrupt/half-published は scan で drop | `cache.rs::cache_budget_evicts_but_leases_pin`、`cache.rs::cache_clear_and_eviction_preserve_source_of_truth`、`cache.rs::cache_open_scan_drops_corrupt_and_half_published`（Pass） | Pass |
| LIB-CACHE-03 auth revocation & clear | permission denied は principal の entry を retire、credential 変更は invalidate | `cache.rs::catalog_permission_denied_retires_principal`、`cache.rs::catalog_credential_change_invalidates`（Pass） | Pass |
| LIB-CACHE-04 Preview full-fetch warning | full download/unknown size は consent 必須、ranged over-budget は拒否 | `cache.rs::preview_full_download_requires_consent`、`preview_unknown_size_requires_consent`、`preview_ranged_over_budget_refused`（Pass） | Pass |
| LIB-CACHE-05 巨大 catalog/rapid cancel/resource budget | single-flight dedupe、waiter cancel、entry/depth/byte budget で bounded truncate | `cache.rs::catalog_single_flight_dedupes_concurrent_builds`、`cache.rs::catalog_waiter_honours_cancellation`、`cache.rs::catalog_partial_and_page_budget_completeness`、`local.rs::entry_budget_truncates_honestly`、`local.rs::depth_budget_marks_truncation`、`rt_cache.rs::rt_queue_is_bounded_and_accounts_drops`（Pass: headless 規模。実運用規模の巨大 catalog は NotRun 注記） | Pass |

### LIB-SEC（traversal/SSRF/redaction/argv）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| LIB-SEC-01 Path traversal/symlink/redirect | locator/redirect の `..`/root 脱出/symlink escape を IO 前に拒否、cross-origin redirect 拒否 | `local.rs::traversal_locators_are_rejected_before_any_io`、`webdav.rs::locator_outside_root_rejected`、`webdav.rs::cross_origin_redirect_refused`、`webdav.rs::cross_origin_redirect_never_reaches_other_origin`、`git.rs::hostile_locator_and_url_inputs_fail_closed`、`containment_swap.rs`（Pass） | Pass |
| LIB-SEC-02 SSRF/credential forwarding | Authorization は configured origin のみに送信、cross-origin redirect は接続前に拒否され foreign origin に request が届かない。実内部 network への SSRF probe は headless 範囲外 | `webdav.rs::cross_origin_redirect_never_reaches_other_origin`（foreign mock への hit=0 を assert）、`webdav.rs::same_origin_redirect_keeps_credentials_in_origin`、`webdav.rs::credentials_sent_only_as_authorization_header`、`git.rs::hostile_locator_and_url_inputs_fail_closed`（ext::/fd::/userinfo URL 拒否）（Pass: mock 証拠。実 SSRF 対象への到達試行は NotRun） | Pass（一部 NotRun） |
| LIB-SEC-03 malformed Content-Length/compression bomb | 壊れた head/status/UTF-8/header 過多/oversize head は `TransientIo`、CL 過大の truncated body と malformed chunked は stream 途中で typed error、`accept-encoding: identity` で圧縮経路を閉じる | `webdav.rs::malformed_response_head_is_typed_transient`、`truncated_body_fails_mid_stream_not_silent`、`malformed_chunked_body_fails_typed`、`max_bytes_enforced_on_pinned_read`、`requests_declare_identity_encoding`（Pass） | Pass |
| LIB-SEC-04 log token/redaction | credential/secret が error detail・debug 表示・locator/URL に漏れない | `git.rs::credential_failures_are_typed_and_secrets_never_leak`（"SECRET-TOKEN-xyz" 非含有 assert）、`types.rs::opaque_credential_debug_redacts_context`（Pass: typed error 経路。app 層の実 log pipeline は consumer 範囲で NotRun） | Pass（一部 NotRun） |
| LIB-SEC-05 Git process argv safety | option/reflog/traversal を含む rev・path は argv 構築前に `InvalidRequest`、`-` 始まり path はデータとして解決、`ext::`/`fd::`/userinfo URL は parse 拒否、prompt/credential helper 無効化 | `git.rs::hostile_locator_and_url_inputs_fail_closed`（`-x:`/`--upload-pack=`/`HEAD@{1}`/`..`/`\`/`//`/空 rev を InvalidRequest、`-dash.txt` を実 file として pin+read、`ext::`/`fd::`/`user:pass@`/`user@` 非 SSH を parse 拒否）（Pass） | Pass |

### LIB-QOS（RT callback 予算）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| LIB-QOS-01 RT callback no blocking/disk/network/alloc | RT enqueue 経路は alloc/dealloc 0、queue は bounded で drop を会計。blocking/disk/network は構造上 RT path に IO を持たない | `rt_cache.rs::rt_event_enqueue_is_alloc_and_dealloc_free`（計測 allocator で alloc/dealloc 0 を assert）、`rt_cache.rs::rt_queue_is_bounded_and_accounts_drops`、`rt_cache.rs::suppressing_sink_drops_stale_generations`、`synvelis-recording` `rt_callback_is_alloc_and_drop_free`（Pass: headless alloc 計測。実 device での blocking 計測は NotRun） | Pass |
| LIB-QOS-02 Capture writer/Prefetch priority | capture/monitor/punch の writer 生成・queue 満杯 fault・placement が headless fake port で契約通り | `synvelis-recording` tests: `arm_off_monitor_on_creates_no_writer_and_captures_nothing`、`work_queue_full_faults`、`filled_queue_full_faults_through_adapter`、`monitor_probe_*` 系（Pass: fake port headless。実 device priority は NotRun） | Pass |
| LIB-QOS-03 f64/PCM time/Golden unchanged | PCM fidelity・semantic audio bitexact・capture bytes が monitor 状態に依らず不変 | `synvelis-recording` `mono_stereo_multichannel_pcm_fidelity`、`raw_capture_bytes_are_identical_across_monitor_states`、`synvelis-player::sf03_l1::sf03_02_semantic_audio_matches_legacy_bitexact`、`recording-poc` tests（Pass: workspace lane） | Pass |
| LIB-QOS-04 実 device/OS/GUI 区別 | Linux/macOS/Windows 実機・実 FS capability・実 GUI の証拠を headless と分離して記録 | 本 matrix では headless 証拠のみを Pass とし、実機/GUI 行は `NotRun` として分離記録（Case 自体の要件は「区別して記録」であり、本書がその区別を実施） | NotRun（区別記録は実施済み） |

## 集計

| Status | 件数 |
| --- | --- |
| Pass | 40（うち 5 件は一部 sub-case が NotRun 注記付き） |
| NotRun | 2（LIB-IMPORT-07、LIB-QOS-04） |
| Fail | 0 |
| Unsupported | 0（能力不在は個別 test で typed 確認済み） |
| NotImplemented | SVN/P4/SMB/S3 provider — 行自体を設けず暗黙 PASS にしない |

※ Case ID は class 単位の集約。1 Case 内に headless Pass と
実環境 NotRun が混在する場合は Observed 欄に注記し、Status は
保守的に「Pass（一部 NotRun）」または「NotRun」とした。
