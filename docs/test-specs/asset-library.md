---
id: "TSPEC-034X9CA5G6nlvnqAHaPPqf"
type: "test-spec"
status: "approved"
title: "Asset Library LIB-ID/BROWSE/PIN/IMPORT/MIG/CACHE/SEC/QOS 保証仕様 (synvelis#9)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/91"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/asset-library-design.md"
    - "docs/specs/asset-library.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/asset-library-design.md"
    - "docs/specs/asset-library.md"
  derived-by: []
  verifies:
    - "docs/designs/asset-library-design.md"
    - "docs/specs/asset-library.md"
  verified-by: []
  references:
    - "evidence/sv9/asset-library-case-ledger.md"
    - "evidence/sv9/asset-library-verification.md"
  defers: []
  deferred-by: []
---

# Asset Library LIB-* 保証仕様 (synvelis#9)

## Purpose

Issue #9 の Asset Library 要件が、実装変更や redesign を経ても
保持されなければならない保証を記録する。特に「metadata と
verified asset の区別」「coverage/capability の typed 表現」
「pin/verify/commit の状態分離」「offline/permission の正直な
表示」「headless/mock Pass を real 受入へ昇格しない」規則は、
将来の変更で静かに失われやすい。実装 case の実行結果は
`docs/impl/` の experiment log と `evidence/sv9/` の ledger
に記録し、本書は実行結果を持たない。

## Feature

Remote-aware Asset Library の Browser/Import UX とその背後の
provider/import/cache/Project 契約。Local/Project/Remote
source を metadata-first で閲覧し、immutable pin・verified
import・offline/permission typed state を提供する。

## Rules

### AL-T-R-01 識別・来歴（LIB-ID）

- Rule: Provider/Remote source/Project Asset/Blob は別
  identity。content identity は検証済み byte の exact
  SHA-256。同 byte 別来歴は別 AssetId、Blob は共有可能。
- Rule: Remote rename・branch move・Local rename は既存
  pin/import を不変に保つ。Project Asset は read-only
  projection。
- Example: `same_bytes_across_providers_share_one_verified_blob`
  — 同一 byte が 2 provider から来ても verified blob は 1 つ、
  AssetId は来歴別。

### AL-T-R-02 Browse・coverage（LIB-BROWSE）

- Rule: catalog は metadata-first paging。cursor+Principal+
  Revision の一致を要求。未選択本体を全量同期しない。
- Rule: No Results・Partial・Offline・Stale・Unsupported・
  IndexIncomplete・Unauthorized・PermissionDenied・Empty を
  相互に区別する。検索非対応は `Unsupported` であり
  `Empty` ではない。索引未完了を検索完了と誤認させない。
- Example: `search_is_typed_unsupported_not_fake_empty` —
  search 非対応 provider で empty ではなく `Unsupported`。
- Example: `offline_remote_typed_failures_mirror_stays_honest`
  — offline で cache mirror が stale/offline を typed 表示。

### AL-T-R-03 Pin（LIB-PIN）

- Rule: Git mutable ref は固定 Commit/Tree/blob へ解決。
  LFS pointer text は media ではなく実 Byte のみ verified。
- Rule: WebDAV は strong ETag + `If-Match`。weak/missing
  ETag は fail-closed。pin 後の source 変化は `SourceChanged`。
- Example: `lfs_pointer_pin_fetches_and_verifies_real_object`
  — LFS pointer pin が実 object を fetch・verify。
- Example: `pin_rejects_weak_and_missing_etags` — weak/missing
  ETag を拒否。
- Example: `open_pinned_412_is_precondition_failed` — pin 後
  の `412` を `SourceChanged` として typed 化。

### AL-T-R-04 Import（LIB-IMPORT）

- Rule: bounded streaming・SHA-256・decode/metadata・staged
  readback・atomic publish・Project CAS・manifest/receipt
  recovery。部分 file・偽 manifest を残さない。
- Rule: `ImportIntent` は import_id・pinned_asset・
  expected_project_revision・target・copy_policy・
  source_principal_ref を保持。同一 import_id replay は
  digest 一致時冪等、相違時拒否。
- Rule: preview 成功は verified import・save 完了を意味
  しない。`CommittedLive` と saved HEAD/SavedRevision は
  別状態。
- Example: `cancel_mid_stream_is_typed_and_same_import_resumes`
  — stream 途中 cancel が typed で同一 import が resume。
- Example: `subprocess_crash_during_publish_or_commit_resumes_across_processes`
  — publish/commit 中 crash が manifest/receipt で復旧。

### AL-T-R-05 Migration（LIB-MIG）

- Rule: v5 旧 CRC/opaque digest・Blob Path・SourceId/AssetId・
  Recording/PCM/Plugin Opaque Bytes を lossless に読み、
  byte-level 検証なしに verified SHA-256 identity へ昇格
  しない。schema 不明/欠落/不正は degraded/typed error。

### AL-T-R-06 Cache/Security/QoS（LIB-CACHE/SEC/QOS）

- Rule: catalog/preview cache は非権威的・principal 分離・
  TTL/LRU/byte budget・generation-aware 通知。eviction は
  正式 Project Asset を削除しない。
- Rule: path traversal・symlink・redirect・SSRF・credential
  forwarding・malformed length/compression bomb・log token・
  git argv を境界で検証。
- Rule: network/disk/hash/blocking/allocation は non-RT job
  のみ。audio callback からは enqueue のみ。
- Example: `rt_event_enqueue_is_alloc_and_dealloc_free` —
  RT enqueue が alloc/dealloc free。
- Example: `rt_queue_is_bounded_and_accounts_drops` — RT queue
  が bounded で drop を accounting。
- Example: `opaque_credential_debug_redacts_context` —
  credential debug が redaction。

### AL-T-R-07 Evidence Honesty

- Rule: 要求状態（Required/Deferred/NotApplicable）と実行
  結果（Pass/Fail/NotRun/Unsupported/NotImplemented/Unknown）
  を別軸で管理。headless/mock/fixture の Pass を real
  provider/GUI/OS/device 受入へ昇格しない。
- Rule: 各 case は repo/commit SHA・Ruviform resolved SHA・
  OS/FS/backend・input/fixture/capability・command/lane・
  expected/observed・status・diagnostic/recovery/rollback・
  link を持つ。

### AL-T-R-08 文書鎖整合

- Rule: spec→design→plan→task→test-spec→impl-ir/exp の鎖が
  relations で双方向に link され、index が `doc-maintenance`
  で再生成されている。

## Case Map（概要 — 詳細は evidence/sv9/asset-library-case-ledger.md）

| 分類 | case 例 | fixture/oracle | Owner | mode/layer | status |
|------|---------|----------------|-------|------------|--------|
| LIB-ID-01〜04 | same_bytes_across_providers・remote_rename_keeps_pin・local_rename_stable・symlink_race | ALIB-01 contract test・#91 oracle | #85/#86 | headless fixture L1 | NotRun（実装は #85/#86） |
| LIB-BROWSE-01〜04 | cursor_pagination・search_unsupported・stale_offline_partial・principal_cursor_invalidation | ALIB-03 cache/index fixture・#91 oracle | #85/#87 | headless fixture L1 | NotRun |
| LIB-PIN-01〜05 | git_fixed_commit・lfs_real_bytes・webdav_strong_etag・mutable_branch・auth_revoke | ALIB-04/06 provider fixture・#91 oracle | #88/#90/#331 | headless/mock L1・real provider L2 | NotRun |
| LIB-IMPORT-01〜10 | bounded_stream・sha256_decode・cancel_each_phase・dup_import_id・stale_revision・target_gone・publish_ack_lost・save_head_separate・offline_playback | ALIB-02 importer fixture・#91 oracle・#86 CAS | #86/#89 | headless fixture L1・real FS L2 | NotRun |
| LIB-MIG-01〜05 | v5_read_upgrade・crc_vs_sha256・missing_blob・collision_no_promotion・opaque_bytes_untouched | ALIB-02-v5 fixture・#91 oracle | #86 | headless fixture L1 | NotRun |
| LIB-CACHE-01〜05 | principal_isolation・ttl_lru_lease・auth_revoke_clear・preview_full_fetch・huge_catalog_budget | ALIB-03 cache fixture・#91 oracle | #87 | headless fixture L1 | NotRun |
| LIB-SEC-01〜05 | traversal_symlink_redirect・ssrf_credential_forwarding・malformed_length_bomb・log_redaction・git_argv | ALIB-04/06/07 security fixture・#91 oracle | #87/#90 | headless/mock L1・real server L2 | NotRun |
| LIB-QOS-01〜04 | rt_no_block_alloc・capture_prefetch_priority・f64_pcm_golden・os_gui_distinction | ALIB-07 RT fixture・#91 oracle・#97 | #87/#91 | headless L1・real device L2 | NotRun |
| DOC-01〜03 | 文書鎖 relations・index 再生成・fmt-check | doc-status/doc-maintenance/mise | 本 slice | docs L1 | Pass（実行後に ledger へ記録） |

## Examples

### EX-1: 検索非対応 provider で empty にしない（LIB-BROWSE-03）

Given search capability を持たない provider の catalog
When UI が search を実行
Then `Unsupported{capability=search}` を typed 表示し、
`Empty`（0 件）として扱わない。

### EX-2: LFS pointer を verified asset にしない（LIB-PIN-02）

Given Git repo の LFS pointer のみ存在する entry
When user が pin/import を選択
Then pointer text を media として import せず、実 object を
fetch して SHA-256 verify する。object 不在は `NotFound`
であり pointer を verified asset にしない。

### EX-3: 同一 ImportId replay（LIB-IMPORT-05）

Given 同一 `import_id`・同一 intent digest の再送
When importer が replay を受ける
Then `AlreadyApplied` または resume し、二重 commit しない。
digest 相違は `Failed{ImportIdConflict}`。

### EX-4: preview 成功 ≠ verified import（LIB-IMPORT）

Given preview fetch が成功した remote entry
When user が import を実行
Then preview byte を verified asset として扱わず、別途
bounded streaming + SHA-256 verify + staged readback + atomic
publish + Project CAS を経る。preview 成功だけでは
`CommittedLive` に遷移しない。

### EX-5: offline で Project 再生継続（LIB-IMPORT-10）

Given 特定 revision を import 済みの Project
When network が切断される
Then copy-into-project 済み byte で Project 再生が継続し、
remote catalog は `Offline`/`Stale` を typed 表示する。

## Guarantees

- metadata-first browse・coverage/capability typed state・
  immutable pin・verified import・offline/permission UX の
  各保証が spec AL-R-01〜17 と対応する。
- LIB-* 全分類が fixture/oracle/Owner/mode/layer/status を
  ledger に持ち、実装未追加は `NotRun` のまま。
- headless/mock/fixture Pass を real provider/GUI/OS 受入へ
  昇格しない。

## Non-goals

- 実 Provider（実 Git remote・実 WebDAV・実 credential
  store）との E2E — #88/#90/#331 の別 gate。
- 実 GPUI window の操作受入 — #114。
- OS 別実機・実 device・実 network の L2/L3 — POLICIES.md
  に従い利用時まで Deferred。
- SVN/P4/SMB/S3 等の追加 Provider の能力保証 — typed
  `Unsupported`/`NotImplemented` のまま段階化。
- Ruviform Rooted File/IO/Security 共通契約の変更 —
  Ruviform 側 Issue Owner。
- 実行結果の記録 — 本書は保証のみ。結果は impl-doc/ledger。

## Risk

- coverage state の複合性により「No Results」への退行が
  起きやすい → AL-T-R-02/AL-T-R-07 で typed 区別を保証。
- preview/verified import の混同 → AL-T-R-04/EX-4 で分離を
  保証。
- ImportId replay の digest 設計ミス → AL-T-R-04/EX-3 で
  canonical digest を要求。
- real 受入への誤昇格 → AL-T-R-07 で mode/layer 分離を保証。
