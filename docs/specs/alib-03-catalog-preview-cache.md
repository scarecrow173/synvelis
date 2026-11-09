---
id: "SPEC-alib03-catalog-preview-cache"
type: "spec"
status: "approved"
title: "ALIB-03 — Catalog/Preview Cache・Principal 隔離・bounded cache（synvelis-library slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/87"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-library/src"
      - type: "file"
        path: "crates/synvelis-library/tests/cache.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/rt_cache.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ALIB-03 — Catalog/Preview Cache・Principal 隔離・bounded cache（synvelis-library slice）

## What Should Be Built

`synvelis-library` に、ALIB-01 の `AssetProvider` 契約の上に載る
**非正本の Catalog/Preview cache・lease・通知層**を実装する。
Provider の browse/search/preview を包み、明示的な cache key・
TTL/LRU/byte budget・principal 分離・Generation 付き非同期通知を
library 層で完結させる。Cache は Asset の真の取得元でも Project の
Verified Blob でもなく、eviction が正式 Project の保存・再生・
Undo を壊さない独立 store とする。

対象は library 契約と実装のみ（fake/scripted provider + fixture）。
`synvelis-app`/`synvelis-player`/`synvelis-workspace` への配線、
GUI Import（#89）、ALIB Fault Matrix（#91）への coverage/Unknown
受け渡しは本 slice では実装しない。

## Why Now

ALIB-01（#85）で provider 境界・`Coverage` 区別・cursor 契約が、
ALIB-02（#86, PR #220 merged）で verified import・SHA-256
namespace・manifest 駆動の復旧が揃った。次の垂直スライス
（Issue #87）は「provider の再走査を避ける非正本 cache」を、
miss/stale/partial/offline/index-incomplete/permission-revoked/
credential-change の各状態を**正直に区別したまま**、principal
横断で private preview を共有しない形で bounded に提供すること。
後続の ALIB-04/05/06（#89 GUI Import 等）がこの層を利用する。

## Users and Value

- **後続 slice の実装者**（#89/#91 等）: provider 呼び出しを包む
  cache/lease/通知 port を再利用できる。cache hit/miss・
  coverage の意味を型で受け取れる。
- **Project 利用者**: paging/scroll/preview の反復で provider
  walk や byte stream が再実行されず、offline 時も最後の index /
  preview が `OfflineCached`/`Stale` と明示されて使える。
- **セキュリティレビュア**: 同一 Blob/Digest でも principal が
  異なれば private preview を共有しないこと、credential 変更・
  revoke 後に当該 principal の非公開 cache が退役することを
  型と fixture で確認できる。

## Scope

### In Scope

- `CatalogCacheKey`（provider + locator scope + caller 固定の
  `CatalogGeneration` + query/filter/sort + principal scope +
  credential epoch）と cache-minted page cursor（key fingerprint +
  index build id + offset に結合し、変更後の再利用を
  `CursorInvalid` で拒否）。
- Catalog index の materialize（provider browse/search page walk
  を bounded に辿り `IndexCompleteness::{Complete,Partial}` を
  記録）、`ObservedAt`/`IndexedAt`/`Completeness`/`Availability`/
  `StaleReason`/`Derivation` を持つ cache value、TTL/LRU/
  byte budget による退役、明示 `clear`、principal 単位の退役。
- `browse`/`search`/`index_status` の cache 経路: hit は
  `Coverage` を正直に写し、期限切れは `Stale`、provider
  `Offline` 時は `OfflineCached` で最後の snapshot を返し、
  `PermissionDenied`/`AuthExpired` は cache を退役させて error を
  そのまま返す。provider `search` 非対応は
  `UnsupportedCapability` を伝播し、部分 index を完全検索に
  見せない。
- `PreviewCacheKey`（provider + pinned `ObjectIdentity` +
  principal scope + decode/format/version + requested region +
  credential epoch）、`PreviewRequest`（range/format/
  `allow_full_object`）、full-download-required の明示、
  bounded streaming → staging → atomic publish、測定 digest/len
  の記録、`PreviewLease`（RAII の entry 保護 + sequential read +
  cancel 対応）。
- 単一 key あたりの single-flight（build/fetch の重複を
  condvar で合体、cancel/deadline は待機側にも効く）。
- `CachePolicy`（disk bytes/entries/TTL/index 上限/preview 上限/
  lease 上限、全て構成可能）と `CacheStats`（entries/bytes/
  leases/evictions/hits/misses/provider calls avoided）。
- `LibraryEvent`/`LibraryEventSink` port、generation stamp、
  `SuppressingSink`（stale generation の drop）、
  `RtEventQueue`（bounded・try_lock・queue 投入のみの RT-safe
  sink。callback から network/disk/hashing/blocking/alloc を
  行わない）。
- `stats_alloc` dev-dependency（`rt_` alloc 計測、他 crate と
  同一手法）と `tests/cache.rs`/`tests/rt_cache.rs`。
- docs チェーン（spec/design/plan/task/test-spec）。

### Out of Scope

- `synvelis-app`/`synvelis-player`/`synvelis-workspace`/
  `synvelis-render`/`synvelis-gateway`/`synvelis-theme` への配線、
  実 Provider（remote/WebDAV 等）実装、OS Credential Store
  adapter、secret の永続化。
- `synvelis-project`/`synvelis-recording` の変更。Project 保存・
  再生・Undo への cache 結合は行わない（cache 削除耐性は
  「触れていないこと」の証明として検証する）。
- GUI Import（#89）・ALIB Fault Matrix（#91）への coverage/
  Unknown 受け渡し — 後続 slice。
- 実 Provider 負荷・長尺・OS 別の FD/実測 — **NotRun/Unsupported**。
- AudioLens baseline 比較 — #294 の別 Gate。
- 既存 ALIB-01/02 API・挙動の破壊的変更（全て additive）。

## Requirements

- Cache miss/stale/partial/offline/index-incomplete/
  permission-revoked/credential-change を fixture で再現し、
  検索・browse 結果の `Coverage`/error の意味が正確であること
  （部分 index を `Complete` に見せない、0 件と未索引を混同
  しない、権限剥奪を offline 扱いしない）。
- 同一 Blob/Digest でも principal が異なれば private preview を
  横断共有しない（cache key・disk path の両方で principal scope
  を分離）。disk cache 削除後も正式 Project 保存・blob store・
  Undo が影響を受けない。
- Paging/scroll/preview rapid cancel で FD/byte stream/cache
  lease の leak がない（provider open stream 数・active lease
  数・staging 残骸で計測）。長時間素材相当・高件数で
  memory/disk/response time が policy budget 内に収まる。
- Provider→Player/Workspace 向け port として generation 付き
  非同期通知を提供し、stale generation を抑制する。RT callback
  経路は queue 投入のみで network/disk/hashing/blocking/alloc
  を行わない（`test-rt` の `stats_alloc` 計測で証明）。
- secret・生 token・credential 値を cache key・disk・log・
  event・job evidence に含めない。principal 識別は fingerprint
  （SHA-256）のみを disk path に使う。
- Ruviform 共通 API（`ruviform-io-file` staged publish 等）への
  依存は既存利用に留め、重複実装しない。既存型・test の
  非退行を確認する。
- 既存 ALIB-01/02 API・挙動を破壊しない。

## Acceptance Criteria

- [ ] miss/stale/partial/offline/index-incomplete/
  permission-revoked/credential-change を再現し、検索結果の
  `Coverage`・error kind が正確（部分 index≠完全検索、
  権限剥奪≠offline cached）。
- [ ] 同一 object identity で principal A/B の preview が
  cache・disk path とも分離される。`revoke_principal`/
  `credentials_changed` 後に当該 principal の entry が退役し、
  他 principal は影響を受けない。
- [ ] disk cache root 削除後も Project の save/blob/Undo が
  非影響であることを test で示す。
- [ ] paging 中 cursor が key/build 変更で `CursorInvalid`、
  index 再構築後の旧 cursor が拒否される。
- [ ] rapid cancel で provider open stream・active lease・
  staging file が残らない。小さい `CachePolicy` で disk bytes/
  entries が budget を超えず、eviction が lease 中 entry を
  追い出さない。
- [ ] 高件数 fixture で 2 ページ目以降の provider 呼び出しが
  0（cache hit）で応答し、index build の walk が
  `max_index_entries`/deadline/cancel で bounded。
- [ ] `LibraryEvent` が generation stamp 付きで emit され、
  `SuppressingSink` が古い generation を drop、`RtEventQueue`
  の `emit` が warm 後 alloc/dealloc 0・非 blocking・
  bounded（溢れは drop 計数）であることを `rt_` test で示す。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`build` が全て pass。既存 test の非退行。

## Deferred Design Concerns

- peaks/thumbnail 派生物の decode pipeline — 本 slice は
  `PreviewFormat` を key に含める枠組みのみ。decode 実装は
  consumer slice。| 後続 slice | decode 未検証 bytes を
  verified 扱いする誤解を防ぐ。
- cache の永続 index rebuild（app restart 後の warm start）|
  disk entry は restart を跨ぐが index rebuild は lazy。|
  実利用で restart latency が問題になった時点 | 再起動直後の
  初回 browse が全件 walk になる。
- OS Credential Store / secret adapter | 本 slice は
  `credential_epoch` の bump API のみ。secret 解決は
  `CredentialResolver` port の consumer 責務 | remote provider
  slice | Provider lib に secret 永続化を持たない。
- cache 圧縮・peer sharing・分散 cache | 対象外 | 要件発生時 |
  YAGNI。
- 実 Provider での負荷・長尺・OS 別 FD 実測 | 本環境は
  scripted provider + Windows FS | 対象環境を持つ検証 slice |
  Unsupported を Supported と誤記しない。

## Sources and Relations

- Issue #87（本 spec の直接の根拠）、#9（Asset Library 親）、
  #85/ALIB-01（provider 契約・`Coverage`/cursor）、
  #86/ALIB-02（verified import・staged publish・manifest）。
- `docs/specs/alib-01-asset-provider-contract.md`、
  `docs/specs/alib-02-streaming-verified-import.md`、
  `docs/designs/alib-01-asset-provider-contract-design.md`、
  `docs/designs/alib-02-streaming-verified-import-design.md`。
- `ruviform-io-file` `StagedFile`/`PublishMode`/`Durability`
  （cache payload の atomic publish に再利用）。
