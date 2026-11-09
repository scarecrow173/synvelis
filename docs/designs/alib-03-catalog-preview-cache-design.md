---
id: "DSN-alib03-catalog-preview-cache"
type: "design"
status: "approved"
title: "ALIB-03 — Catalog/Preview Cache 設計（synvelis-library）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-03-catalog-preview-cache.md"
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
  implements:
    - "docs/specs/alib-03-catalog-preview-cache.md"
  implemented-by: []
  depends-on:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
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

# ALIB-03 — Catalog/Preview Cache 設計（synvelis-library）

## 1. 位置づけ

ALIB-01 の provider 境界を包む**非正本 derived cache**。cache は
Asset の真の取得元ではなく Project の Verified Blob でもない —
独立した disk store であり、全 entry の喪失は「miss → provider
再取得」に落ちるだけで正式データに影響しない。

権威の分担:

- catalog の正本 = provider（live walk/search）。cache は
  `ObservedAt`/`IndexedAt`/`Completeness` 付きの derived snapshot。
- preview bytes の正本 = provider object（pin で固定）。cache は
  `ObjectIdentity` + principal + format + region で key した
  derived copy。測定 digest/len を meta に記録する。
- project media = `synvelis-project` の blob store。本層は
  一切触れない。

## 2. モジュール構成

```
src/cache.rs     CachePolicy/CacheStats/CacheLease/StoreState/
                 単一 Store・LRU+TTL eviction・single-flight・
                 atomic payload write・principal 退役・clear・
                 LibraryCache facade
src/catalog.rs   CatalogGeneration/CatalogQuery/CatalogCacheKey/
                 IndexCompleteness/CatalogIndex record/IndexStatus/
                 cache-minted cursor/browse・search・index_status
src/preview.rs   PreviewFormat/PreviewRequest/PreviewCacheKey/
                 PreviewMeta/PreviewLease(Read)/preview・
                 preview_plan・FullDownloadRequired
src/notify.rs    LibraryEvent/LibraryEventKind/RetireReason/
                 LibraryEventSink/SuppressingSink/RtEventQueue
```

`lib.rs` は module 宣言と re-export のみ（ALIB-01/02 と同型）。

## 3. Store 構造と key

```
<root>/catalog/<principal_tag>/<key_fp>/index.json
<root>/preview/<principal_tag>/<key_fp>/payload.bin + meta.json
<root>/.tmp-*          in-flight staging（成功時 rename、失敗時削除）
```

- `principal_tag` = principal 文字列の **SHA-256 全桁 hex**
  （`short_fingerprint` の 64bit ではなく全 256bit — principal
  分離は cache の正しさの根拠なので衝突耐性を優先）。
  raw principal・credential・secret は disk/log/event に出さない。
- `key_fp` = `short_fingerprint`（16 hex）over canonical JSON of
  the public key struct。lookup は fingerprint probe のみで、
  disk 上に key を復元する必要はない。
- `index.json`/`meta.json`+`payload.bin` は `StagedFile` +
  `PublishMode::Replace`/`CreateNew` + `FileSync` の atomic
  publish（import.rs と同じ primitive、durability を偽らない）。
- `StoreState`（`Mutex`）は entry metadata のみ保持
  （key triple・bytes・created/last_used・leases・sweep tag）。
  payload は disk のみ — memory は entry 数 × metadata で bounded。

### CatalogCacheKey

```rust
pub struct CatalogCacheKey {
    provider: ProviderId,
    scope: BrowseScope,
    generation: CatalogGeneration,   // caller 固定の revision/epoch
    query: CatalogQuery,             // Browse{sort,filter}|Search{text}
    principal: PrincipalId,
    credential_epoch: u64,
}
```

`CatalogGeneration(pub u64)` は caller が pin する opaque token
（project revision・app catalog epoch 等）。cache は解釈しない。
generation が変われば別 key = miss であり、insert 時に同一
(provider, scope, principal, credential_epoch) の旧 generation
entry を sweep する（「Revision が変われば index result は失効」）。

### PreviewCacheKey

```rust
pub struct PreviewCacheKey {
    provider: ProviderId,
    object_identity: ObjectIdentity, // pin 済み object
    principal: PrincipalId,
    format: PreviewFormat,           // decode/format/version
    region: Option<ByteRange>,
    credential_epoch: u64,
}
```

preview は `provider.pin(asset)` で `object_identity` を得てから
key を作る — pinned object identity が preview の同一性。
`ByteRange`/`PreviewFormat`/principal/epoch が違えば別 entry で、
同一 object でも principal 横断の共有は構造的に起きない。

## 4. Catalog cache 意味論

### build（miss/refresh）

1. `provider.browse`/`provider.search` を page walk
   （`next_cursor` を辿る）。各 page で `cx.check_interrupts()`、
   `max_index_entries`/`max_index_pages`/deadline/cancel で打切り。
2. 集めた entries を `index.json` として atomic publish。
   `completeness` = 最終 page が `Coverage::Complete` かつ
   walk が budget で打ち切られていない場合のみ `Complete`、
   それ以外は `Partial{reason}`。
3. single-flight: 同一 key の同時 build は `InFlight` slot で
   合体し、待機側は condvar + cancel/deadline check で結果を
   受け取る（または `Cancelled`/`BudgetExceeded`）。

### serve（hit）

- fresh（TTL 内）: snapshot を読み page slice を返す。
  coverage は snapshot completeness を写し、継続 page は
  provider と同じく `Partial`、最終 page は
  `Complete`/`Partial` をそのまま返す。
- 期限切れ（TTL 超過・未 eviction）: provider が生きていれば
  **refresh を試み**、成功なら新 snapshot で serve。provider が
  `Offline`/`TransientIo`/`RateLimited` なら期限切れ snapshot を
  `OfflineCached`（Offline）/`Stale`（その他 transient）で返す。
  `PermissionDenied`/`AuthExpired` は当該 principal の全 entry を
  退役させて error を返す（cache は絶対に権限を迂回しない）。
- cursor page: cache-minted cursor（`key_fp + build_id + offset`
  の hex JSON）を decode・照合。index が無い/再構築済みなら
  `CursorInvalid` — client は先頭から paging し直す。
  TTL 期限切れ snapshot 上の cursor 継続は `Stale` coverage で
  正直に返す（page 途中で snapshot が入れ替わらない）。
- `search`: provider.search が `UnsupportedCapability` なら
  そのまま伝播（cache は検索結果を捏造しない）。provider page
  の `Coverage::Partial` は snapshot `Partial` に写り、
  部分 index を `Complete` に見せない。

`index_status(provider, scope, query, generation, cx)` は
`NeverIndexed`/`Fresh{completeness}`/`Stale{completeness}` を返す
— 「結果 0 件」と「未索引」を区別する読み取り専用 API。

## 5. Preview cache 意味論

### `preview_plan`（fetch 前の明示）

`PreviewPlan::{Bounded{bytes}, FullDownload{bytes:Option<u64>},
Unavailable{reason}}` を返す。`FullDownload` は
`allow_full_object=false` の `preview()` を
`CacheError::FullDownloadRequired` で拒否する根拠 —
「Duration/Codec/size 不明で GB 級の全取得を勝手に始めない」
を型で強制する。

### `preview`

1. `provider.pin(asset)` → `object_identity`/`expected_size`/
   `digest_hint`。pin 非対応は error 伝播。
2. key 構成 → hit なら `PreviewLease`（open file + RAII lease）。
   期限切れ hit は refetch（object identity で内容同一性は
   保証されるが TTL/privacy のため再取得）。
3. miss → `open_preview(asset, range)` を 64KiB chunk で
   staging file へ stream。chunk 毎に `check_interrupts`、
   `max_preview_fetch_bytes` 上限、測定 SHA-256/len を記録。
   期待 len（range.len or expected_size）と不一致なら
   `IntegrityMismatch` で staging を捨てる — 切詰め bytes を
   完全 entry として残さない。
4. staging → `payload.bin` atomic publish + `meta.json`。
   失敗/cancel は staging 削除（FD は drop で close）。
5. `PreviewLease` は entry の lease を保持したまま payload を
   sequential read する `Read` impl。cancel token を保持し、
   read 毎に `is_cancelled` → `Interrupted`。drop で file close
   + lease 解放。

### lease / FD / stream leak 防止

- `CacheLease` は `leases += 1` の RAII。eviction は
  `leases == 0` の entry のみ対象。lease 中 entry が budget を
  超えても追い出さない（serve は継続、retire は lease 解放後）。
- `StoreState.active_leases`/`stats().active_leases` で
  rapid cancel 後に 0 へ戻ることを計測。provider 側の open
  stream 数（fake provider の counter）も 0 へ戻ることを確認。
- `max_active_leases` 超過は `CacheError::LeaseExhausted` —
  bounded concurrency の明示。

## 6. Bounded QoS

`CachePolicy`（全て構成可能、default は PoC 値であり
security保証として宣言しない）:

```rust
pub struct CachePolicy {
    pub max_disk_bytes: u64,        // default 256 MiB
    pub max_entries: usize,         // default 4096
    pub catalog_ttl: Duration,      // default 15 min
    pub preview_ttl: Duration,      // default 60 min
    pub max_index_entries: usize,   // default 50_000
    pub max_index_pages: u32,       // default 512
    pub max_preview_object_bytes: u64,  // default 64 MiB
    pub max_full_object_bytes: u64,     // default 512 MiB
    pub max_active_leases: u32,         // default 64
    pub max_concurrent_builds: usize,   // single-flight wait 上限
}
```

- eviction: insert 毎に `disk_bytes > max_disk_bytes` または
  `entries > max_entries` なら LRU（`last_used` 最古・
  `leases==0`・期限切れ優先）から削除。lease 中しか evict
  できない場合は entry を格納せず serve のみ（cache write
  失敗で user op を fail させない）。
- TTL: access 時に期限切れを検査し、期限切れ entry は
  refresh 経路（catalog）または refetch（preview）で更新、
  provider 不通時のみ stale serve。
- `CacheStats`: `entries/bytes/hits/misses/evictions/
  expired/active_leases/provider_calls_avoided` を公開し、
  bounded 計測 fixture が参照する。

## 7. 通知と RT 契約

```rust
pub enum LibraryEventKind {
    CatalogIndexed { key_fp, completeness },
    PreviewStored { key_fp, byte_len },
    EntriesEvicted { count, bytes },
    PrincipalRetired { principal_fp, reason: RetireReason },
    CacheCleared,
}
pub struct LibraryEvent { pub generation: u64, pub kind, pub at: ObservedAt }
pub trait LibraryEventSink: Send + Sync { fn emit(&self, event: LibraryEvent); }
```

- `EventHub`（cache 内部）は `generation: AtomicU64` を持ち、
  invalidate 系操作（retire/credential change/clear/sweep）の
  冒頭で `fetch_add(1)` して新 generation を event に stamp。
  通常の indexed/stored event は現行 generation を stamp。
- `SuppressingSink<S>`: consumer 側 wrapper。`emit` で
  `latest.fetch_max(event.generation)` を行い、既知より古い
  generation の event を drop（stale suppression）。
- `RtEventQueue`: `Mutex<Vec<Option<LibraryEvent>>>`（capacity
  事前確保）+ `AtomicUsize dropped`。`emit` は `try_lock` →
  空きがあれば move して push、満杯/lock 失敗時は `dropped`
  を increment して return — **block も alloc もしない**。
  `rt_` test が warm 後の `emit` を `stats_alloc` Region で
  計測し alloc/dealloc 0 を確認する（他 crate の
  `rt_*_alloc` test と同手法）。
- 本層は blocking worker 前提のまま — RT thread からは
  `RtEventQueue` の `emit` のみを呼ぶ契約。cache API
  （browse/search/preview）は非 RT で呼ぶ。

## 8. Error / 境界

```rust
pub enum CacheError {
    Provider(ProviderError),       // provider 層の typed error を保持
    Io { op: &'static str, detail: String },
    FullDownloadRequired { expected_size: Option<u64> },
    LeaseExhausted,
}
```

`browse`/`search`/`index_status`/`stats`/`clear`/`revoke`/
`credentials_changed` は `CacheError`（または `ProviderError`
を `CacheError::Provider` で包む）を返す。`preview`/`preview_plan`
も同じ。corrupt cache payload は miss 扱い + entry 退役
（cache の破損で user op を fail させない）。

## 9. 非目標・境界

- `synvelis-player`/`synvelis-app`/`synvelis-workspace`/
  `synvelis-project`/`synvelis-recording`/Ruviform は変更しない。
- RT I/O なし・Tokio/HTTP/Git SDK を public API に出さない
  （ALIB-01/02 踏襲）。
- 実 Provider の credential/secret 解決は consumer の
  `CredentialResolver` port — 本層は `credential_epoch` の
  bump/退役 API のみ提供し secret を保持しない。
- `Cargo.toml`/`Cargo.lock` の変更は `stats_alloc` dev-dep
  追加のみ（Issue の共有ファイル条項に従い PR に記録）。
