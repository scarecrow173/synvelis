---
id: "TESTSPEC-alib06"
type: "test-spec"
status: "approved"
title: "ALIB-06 WebDAV File-Server Provider — テスト仕様"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  verifies:
    - "docs/specs/alib-06-webdav-provider.md"
    - "docs/designs/alib-06-webdav-provider-design.md"
  verified-by:
    - "crates/synvelis-library/tests/webdav.rs"
    - "crates/synvelis-library/tests/common/webdav_fixture.rs"
---

# ALIB-06 Test Spec

`crates/synvelis-library/tests/common/webdav_fixture.rs` が
`TcpListener` の in-process server で in-memory tree に対し
PROPFIND/GET/HEAD を提供し、auth・redirect・chunked・
stall・disconnect・ETag 変種・大 collection を再現する。
`tests/webdav.rs` が `AssetProvider` 契約・ETag/If-Match・
Range・bounded listing・failure 分類・redaction・import
接続を検証する。実 WebDAV server・実 network・TLS・実 OS・
GUI は対象外（NotRun/Unsupported）。

## Feature: Metadata-first browse/stat

- Rule: PROPFIND Depth:1/0 で collection を列挙・観測。
  paging 非対応のため bounded listing であり、打ち切りは
  `Coverage::Partial`、全件列挙を偽装しない。
- Example: `browse_root_lists_collection_entries`（dir/
  file 分類・facts 観測）、`browse_subtree_via_locator`、
  `stat_reports_file_and_collection_facts`、
  `browse_large_collection_is_bounded_partial`（
  `max_entries` 超 flat dir → Partial + window cursor）、
  `browse_rejects_file_scope_and_escape_locator`。

## Feature: Strong ETag pin / fail-closed

- Rule: strong ETag（`W/` なし・ASCII entity-tag）のみ
  `ConditionalStrong{StrongValidator{IfMatch}}` で pin。
  weak/なし/Last-Modified のみは `UnsupportedCapability`。
- Example: `pin_strong_etag_produces_if_match_proof`、
  `pin_weak_etag_is_unsupported`、`pin_missing_etag_is_
  unsupported`、`pin_uses_getcontentlength_as_expected_
  size`、`pin_rejects_container`、`etag_grammar_rejects_
  ctl_and_obs_text`（unit）。

## Feature: Conditional read（If-Match）

- Rule: `open_pinned` は `GET`+`If-Match`。412→
  `PreconditionFailed`、応答 ETag 不一致→`SourceChanged`、
  pin 後の resource 変更は Verified に昇格しない。
- Example: `open_pinned_sends_if_match_header`（wire 記録）、
  `etag_changed_after_pin_returns_412`、
  `response_etag_mismatch_is_source_changed`、
  `pinned_fetch_verifies_size_and_reports_digest`。

## Feature: Range / Preview

- Rule: Range 対応 server は 206、非対応は 200 → bounded
  discard。byte budget を超えない。preview は Unverified。
- Example: `preview_range_206`、`preview_range_ignored_
  bounded_discard`、`preview_without_range_bounded`、
  `preview_range_over_budget_rejected`。

## Feature: Failure 分類

- Rule: auth revoke・offline・HTTP error・timeout・
  mid-transfer disconnect・cancel が typed error。
  NotFound に隠さない。
- Example: `auth_revoked_returns_auth_expired`、
  `forbidden_returns_permission_denied`、`not_found_is_
  not_hidden`、`server_5xx_is_transient`、
  `stall_times_out_transient`、`mid_transfer_disconnect_
  fails_typed`、`cancel_during_fetch_is_cancelled`、
  `cancel_during_stall_is_cancelled`。

## Feature: Security / redaction

- Rule: credential を URL/locator/provenance/log/error
  detail に残さない。cross-origin redirect で
  `Authorization` を転送しない。path traversal・
  confinement escape を拒否。
- Example: `credential_not_in_locator_or_error`、
  `cross_origin_redirect_strips_authorization`、
  `same_origin_redirect_keeps_authorization`、
  `https_target_is_not_downgraded`、
  `path_traversal_and_encoded_dotdot_rejected`、
  `locator_outside_root_path_rejected`、
  `url_rejects_userinfo_query_fragment`。

## Feature: Import 接続・capability

- Rule: strong pin→`import_verified`→SHA-256 blob→
  offline 再読。capability は正直に宣言
  （search=Unsupported、checksum_hint=Unsupported、
  pin=ConditionalStrong、access=ReadOnly）。
- Example: `import_verified_consumes_webdav_pin`、
  `capabilities_declared`、`webdav_provider_info_reports_
  scheme_and_auth`、`search_is_typed_unsupported`。

## Feature: 非回帰

- Rule: 既存 ALIB-01/02/03/04 test が全て pass。
  AudioLens/RT/Golden に影響しない。
- Example: `mise run test`/`test-rt`/`test-msrv` 全 pass、
  `tests/contract.rs`/`cache.rs`/`import.rs`/`local.rs`/
  `project.rs`/`git.rs`/`types.rs`/`containment_swap.rs`
  非退行。
