//! Catalog index cache (ALIB-03, Synvelis #87).
//!
//! `LibraryCache::browse`/`search` serve provider catalog pages from
//! a bounded, principal-isolated, TTL'd index snapshot stored under
//! the cache root — *derived* data that is never the catalog's
//! source of truth. A miss or expired entry triggers a bounded
//! provider walk (single-flight per key); provider failures degrade
//! honestly: `Offline` → `Coverage::OfflineCached`, other transient
//! failures → `Coverage::Stale`, and `PermissionDenied`/`AuthExpired`
//! retire the principal's private entries before propagating.
//!
//! Cache cursors are minted by the cache itself: they bind the
//! catalog key fingerprint, the index build id, and the entry
//! offset. Reusing a cursor after the query/principal/provider/
//! generation changed — or after the index was rebuilt — fails with
//! `CursorInvalid`, exactly like provider cursors.

use std::sync::Arc;
use std::time::SystemTime;

use ruviform_io_file::PublishMode;
use serde::{Deserialize, Serialize};

use crate::browse::{
    BrowseEntry, BrowsePage, BrowseRequest, BrowseScope, Coverage, FilterSpec, OpaquePageCursor,
    SearchQuery, SortSpec, hex_decode, hex_encode, scope_fingerprint,
};
use crate::cache::{
    CATALOG_PAYLOAD, CacheCategory, CacheCore, CacheError, EntryAux, EntryMeta, LibraryCache,
    SIDECAR_FORMAT, SIDECAR_NAME, SidecarMeta,
};
use crate::context::ProviderContext;
use crate::digest::short_fingerprint;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::ProviderId;
use crate::notify::LibraryEventKind;
use crate::observation::ObservedAt;
use crate::provider::AssetProvider;

/// Index payload wire version.
const INDEX_FORMAT: u32 = 1;

/// Caller-pinned catalog revision/generation token.
///
/// The cache never interprets it: a different generation is simply a
/// different cache key (a miss), and committing a new generation
/// sweeps older-generation indexes for the same
/// (provider, scope, principal, credential epoch). Pass the project
/// revision for [`crate::ProjectAssetsProvider`], an app catalog
/// epoch for live providers, or `CatalogGeneration(0)` when the
/// provider has no revision concept.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct CatalogGeneration(pub u64);

/// What a catalog index covers — browse sort/filter or a search text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CatalogQuery {
    /// A browse listing with its sort and filter.
    Browse {
        /// Sort specification.
        sort: SortSpec,
        /// Entry filter.
        filter: FilterSpec,
    },
    /// A provider-side text search.
    Search {
        /// Free-text query.
        text: String,
    },
}

/// The complete identity of one catalog index entry.
///
/// Every field participates in the key fingerprint: provider,
/// scope, caller-pinned generation, query, principal scope, and the
/// credential epoch. The fingerprint — never the key itself — is
/// what reaches disk paths, cursors, events, and stats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogCacheKey {
    /// Owning provider.
    pub provider: ProviderId,
    /// Browse scope the index covers.
    pub scope: BrowseScope,
    /// Caller-pinned generation.
    pub generation: CatalogGeneration,
    /// Browse sort/filter or search text.
    pub query: CatalogQuery,
    /// Principal scope — private indexes never cross principals.
    pub principal: crate::PrincipalId,
    /// Credential epoch the key was computed under.
    pub credential_epoch: u64,
}

impl CatalogCacheKey {
    /// Short fingerprint over the canonical key encoding.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        short_fingerprint(&bytes)
    }
}

/// Why an index is `Partial`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexPartialReason {
    /// The provider reported truncation (`Coverage::Partial` on the
    /// terminal page — its own walk budget stopped early).
    ProviderTruncated,
    /// The cache's `max_index_entries` bound stopped the walk.
    EntryBudget,
    /// The cache's `max_index_pages` bound stopped the walk.
    PageBudget,
}

/// Whether the cached index covers the whole query result.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum IndexCompleteness {
    /// The provider walk ran to its end un-truncated.
    Complete,
    /// The index is real but does not cover everything — never
    /// presented to callers as `Complete`.
    Partial {
        /// Why the index is incomplete.
        reason: IndexPartialReason,
    },
}

/// Read-only index status — distinguishes "no results" from
/// "never indexed" and "indexed but aged out".
#[derive(Clone, Debug, PartialEq)]
pub enum IndexStatus {
    /// No index exists for this key.
    NeverIndexed,
    /// A usable (TTL-fresh) index exists.
    Fresh {
        /// Recorded completeness.
        completeness: IndexCompleteness,
        /// When the index was built.
        indexed_at: ObservedAt,
    },
    /// An index exists but is past TTL — a refresh candidate, still
    /// servable as `Stale`/`OfflineCached` when the provider is down.
    Stale {
        /// Recorded completeness.
        completeness: IndexCompleteness,
        /// When the index was built.
        indexed_at: ObservedAt,
    },
}

/// The on-disk index payload (`index.json`). Entries are stored in
/// provider order — sort/filter were applied during the walk, so
/// slices preserve the requested ordering.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogIndexRecord {
    /// Wire version.
    format_version: u32,
    /// When the cache built this index.
    indexed_at: ObservedAt,
    /// Latest provider observation carried by the walked pages.
    observed_at: ObservedAt,
    /// Recorded completeness.
    completeness: IndexCompleteness,
    /// Materialized entries.
    entries: Vec<BrowseEntry>,
}

/// Cache-minted page cursor payload — binds key fingerprint, index
/// instance fingerprint, and offset. Versioned JSON, hex-encoded.
#[derive(Serialize, Deserialize)]
struct CacheCursorPayload {
    /// Wire version — only `1` is understood.
    v: u8,
    /// Catalog key fingerprint the cursor belongs to.
    key_fp: String,
    /// Fingerprint of the exact `index.json` instance — invalidated
    /// when the index is rebuilt (survives restarts via the
    /// sidecar).
    index_fp: String,
    /// Entries already emitted.
    offset: u64,
}

fn cursor_invalid(detail: impl Into<String>) -> CacheError {
    CacheError::Provider(ProviderError::new(
        ProviderErrorKind::CursorInvalid,
        detail.into(),
    ))
}

fn encode_cache_cursor(
    key_fp: &str,
    index_fp: &str,
    offset: u64,
) -> Result<OpaquePageCursor, CacheError> {
    let payload = CacheCursorPayload {
        v: 1,
        key_fp: key_fp.to_owned(),
        index_fp: index_fp.to_owned(),
        offset,
    };
    let bytes = serde_json::to_vec(&payload)
        .map_err(|e| CacheError::internal(format!("cursor encode: {e}")))?;
    OpaquePageCursor::new(hex_encode(&bytes)).map_err(CacheError::Provider)
}

fn decode_cache_cursor(
    cursor: &OpaquePageCursor,
    key_fp: &str,
) -> Result<CacheCursorPayload, CacheError> {
    let bytes = hex_decode(cursor.as_str()).map_err(CacheError::Provider)?;
    let payload: CacheCursorPayload =
        serde_json::from_slice(&bytes).map_err(|_| cursor_invalid("undecodable cache cursor"))?;
    if payload.v != 1 {
        return Err(cursor_invalid("unsupported cache cursor version"));
    }
    if payload.key_fp != key_fp {
        return Err(cursor_invalid("cursor belongs to another catalog key"));
    }
    Ok(payload)
}

/// How a built/served index relates to freshness — drives the
/// `Coverage` overlay on stale serves.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum IndexResolution {
    /// A fresh index was built (or a fresh hit was served).
    Fresh,
    /// Provider unreachable — serve the aged snapshot as
    /// `OfflineCached`.
    OfflineCached,
    /// Provider transiently failed — serve the aged snapshot as
    /// `Stale`.
    StaleServe,
}

/// The result of a catalog build flight — shared with waiters.
#[derive(Clone)]
struct BuiltIndex {
    /// The walked record (served from memory; also what was
    /// published when `meta` is `Some`).
    record: Arc<CatalogIndexRecord>,
    /// The committed entry metadata — `None` when the index was
    /// built but not admitted (oversized payload or store failure);
    /// the record is still served, just without cache cursors.
    meta: Option<EntryMeta>,
    /// Freshness resolution for coverage mapping.
    resolution: IndexResolution,
}

/// Sweep tag grouping all catalog indexes of one
/// (provider, scope, principal, credential epoch) across queries and
/// generations — a generation change sweeps them together.
fn sweep_tag_for(key: &CatalogCacheKey, principal_tag: &str) -> String {
    short_fingerprint(
        format!(
            "catalog|{}|{}|{}|{}",
            key.provider.to_tagged(),
            scope_fingerprint(&key.scope),
            principal_tag,
            key.credential_epoch,
        )
        .as_bytes(),
    )
}

/// The index-instance fingerprint recorded for `meta` — `None` for
/// non-catalog entries (impossible in practice; treated as no cursor
/// support).
fn index_fp_of(meta: &EntryMeta) -> Option<&str> {
    match &meta.aux {
        EntryAux::Catalog { index_fp, .. } => Some(index_fp),
        EntryAux::Preview(_) => None,
    }
}

/// `Some(coverage)` when the served index is past TTL — the caller
/// decides whether the resolution was an offline or generic stale
/// serve. Fresh entries pass `None`.
fn freshness_override(
    meta: Option<&EntryMeta>,
    resolution: IndexResolution,
    now: SystemTime,
) -> Option<Coverage> {
    match meta {
        Some(meta) if meta.expired(now) => Some(match resolution {
            IndexResolution::OfflineCached => Coverage::OfflineCached,
            IndexResolution::Fresh | IndexResolution::StaleServe => Coverage::Stale,
        }),
        _ => None,
    }
}

impl LibraryCache {
    /// Serves one page of `req`'s catalog from the index cache,
    /// building or refreshing the index on miss/expiry.
    ///
    /// # Errors
    /// [`CacheError::Provider`] for provider failures (denials also
    /// retire the principal's entries); [`CacheError::CursorInvalid`]
    /// for stale/foreign cursors; cache-internal errors otherwise.
    pub fn browse(
        &self,
        provider: &dyn AssetProvider,
        req: &BrowseRequest,
        generation: CatalogGeneration,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, CacheError> {
        let key = self.key_for(
            provider.id(),
            req.scope.clone(),
            CatalogQuery::Browse {
                sort: req.sort,
                filter: req.filter.clone(),
            },
            generation,
            cx,
        );
        self.catalog_page(provider, &key, req.page_limit, req.cursor.as_ref(), cx)
    }

    /// Serves one page of `query`'s provider-side search from the
    /// index cache. `UnsupportedCapability` propagates unchanged —
    /// the cache never fabricates search results.
    ///
    /// # Errors
    /// Same surface as [`Self::browse`].
    pub fn search(
        &self,
        provider: &dyn AssetProvider,
        query: &SearchQuery,
        generation: CatalogGeneration,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, CacheError> {
        let key = self.key_for(
            provider.id(),
            query.scope.clone(),
            CatalogQuery::Search {
                text: query.text.clone(),
            },
            generation,
            cx,
        );
        self.catalog_page(provider, &key, query.page_limit, query.cursor.as_ref(), cx)
    }

    /// Read-only index status — no provider call, no build.
    ///
    /// # Errors
    /// [`CacheError`] on store failures.
    pub fn index_status(
        &self,
        provider: ProviderId,
        scope: &BrowseScope,
        query: &CatalogQuery,
        generation: CatalogGeneration,
        cx: &ProviderContext,
    ) -> Result<IndexStatus, CacheError> {
        let key = self.key_for(provider, scope.clone(), query.clone(), generation, cx);
        let key_fp = key.fingerprint();
        let ptag = CacheCore::principal_tag(&cx.principal);
        let meta = self
            .core
            .lookup(CacheCategory::Catalog, &ptag, &key_fp, SystemTime::now());
        let Some(meta) = meta else {
            return Ok(IndexStatus::NeverIndexed);
        };
        let EntryAux::Catalog {
            completeness,
            indexed_at,
            ..
        } = &meta.aux
        else {
            // Category mismatch cannot happen (key_fp collision is
            // cryptographically implausible) — treat as absent.
            return Ok(IndexStatus::NeverIndexed);
        };
        if meta.expired(SystemTime::now()) {
            Ok(IndexStatus::Stale {
                completeness: *completeness,
                indexed_at: *indexed_at,
            })
        } else {
            Ok(IndexStatus::Fresh {
                completeness: *completeness,
                indexed_at: *indexed_at,
            })
        }
    }

    /// Builds the cache key for one catalog query under `cx`'s
    /// principal and the current credential epoch.
    fn key_for(
        &self,
        provider: ProviderId,
        scope: BrowseScope,
        query: CatalogQuery,
        generation: CatalogGeneration,
        cx: &ProviderContext,
    ) -> CatalogCacheKey {
        CatalogCacheKey {
            provider,
            scope,
            generation,
            query,
            principal: cx.principal.clone(),
            credential_epoch: self.credential_epoch(&cx.principal),
        }
    }

    /// Shared page path for `browse`/`search`: fresh hit → serve;
    /// miss/expired → single-flight build then serve.
    fn catalog_page(
        &self,
        provider: &dyn AssetProvider,
        key: &CatalogCacheKey,
        page_limit: u32,
        cursor: Option<&OpaquePageCursor>,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, CacheError> {
        if page_limit == 0 {
            return Err(CacheError::Provider(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "page_limit must be >= 1",
            )));
        }
        let key_fp = key.fingerprint();
        let ptag = CacheCore::principal_tag(&cx.principal);
        let now = SystemTime::now();
        let meta = self
            .core
            .lookup(CacheCategory::Catalog, &ptag, &key_fp, now);

        // Cursor pages never refresh — an aged snapshot continues
        // honestly as `Stale`.
        if let Some(cursor) = cursor {
            return self.serve_cursor_page(&key_fp, meta, cursor, page_limit);
        }

        // Fresh hit — no provider call. A corrupt payload already
        // retired the entry inside `load_index`; fall through to a
        // rebuild.
        if let Some(meta) = meta.as_ref().filter(|m| !m.expired(now))
            && let Some(record) = self.load_index(meta)
        {
            self.core.note_hit();
            return Self::serve_page(
                &key_fp,
                Some(meta),
                &record,
                0,
                page_limit,
                freshness_override(Some(meta), IndexResolution::Fresh, SystemTime::now()),
            );
        }

        // Miss or expired — single-flight build/refresh.
        self.core.note_miss();
        let rel_dir = CacheCore::rel_dir(CacheCategory::Catalog, &ptag, &key_fp);
        let built = self.core.run_flight(&rel_dir, cx, || {
            self.build_catalog(provider, key, &ptag, &key_fp, cx)
        })?;
        let freshness =
            freshness_override(built.meta.as_ref(), built.resolution, SystemTime::now());
        Self::serve_page(
            &key_fp,
            built.meta.as_ref(),
            &built.record,
            0,
            page_limit,
            freshness,
        )
    }

    /// Walks the provider for `key`'s query and commits the index —
    /// the single-flight leader body.
    fn build_catalog(
        &self,
        provider: &dyn AssetProvider,
        key: &CatalogCacheKey,
        principal_tag: &str,
        key_fp: &str,
        cx: &ProviderContext,
    ) -> Result<BuiltIndex, CacheError> {
        let record = match self.walk_catalog(provider, key, cx) {
            Ok(record) => record,
            Err(err) => return self.catalog_fallback(err, principal_tag, key_fp, cx),
        };
        let record = Arc::new(record);
        // Commit best-effort: a store failure must not fail the user
        // operation — the freshly walked record is still served, it
        // just gets no cache cursors.
        // `Ok(None)` = credential epoch moved mid-build — drop the
        // orphaned payload and serve uncached rather than resurrect
        // retired-authority data.
        let meta = self
            .commit_catalog_index(key, principal_tag, key_fp, &record)
            .unwrap_or_default();
        if meta.is_none() {
            self.core.remove_entry(&CacheCore::rel_dir(
                CacheCategory::Catalog,
                principal_tag,
                key_fp,
            ));
        }
        if meta.is_some() {
            self.core
                .sweep_catalog_generation(&sweep_tag_for(key, principal_tag), key.generation.0);
            let generation = self.core.events.generation();
            self.core.events.emit(
                generation,
                LibraryEventKind::CatalogIndexed {
                    key_fp: key_fp.to_owned(),
                    completeness: record.completeness,
                },
                record.indexed_at,
            );
        }
        Ok(BuiltIndex {
            record,
            meta,
            resolution: IndexResolution::Fresh,
        })
    }

    /// Provider-failure path of a build: denials retire the
    /// principal; `Offline`/transient errors serve the aged snapshot
    /// when one exists.
    fn catalog_fallback(
        &self,
        err: CacheError,
        principal_tag: &str,
        key_fp: &str,
        cx: &ProviderContext,
    ) -> Result<BuiltIndex, CacheError> {
        match err.provider_error().map(ProviderError::kind) {
            Some(ProviderErrorKind::PermissionDenied | ProviderErrorKind::AuthExpired) => {
                self.retire_for_provider_denial(cx);
                return Err(err);
            }
            Some(ProviderErrorKind::Offline) => {
                if let Some(built) =
                    self.aged_snapshot(key_fp, principal_tag, IndexResolution::OfflineCached)
                {
                    return Ok(built);
                }
            }
            Some(ProviderErrorKind::TransientIo | ProviderErrorKind::RateLimited) => {
                if let Some(built) =
                    self.aged_snapshot(key_fp, principal_tag, IndexResolution::StaleServe)
                {
                    return Ok(built);
                }
            }
            _ => {}
        }
        Err(err)
    }

    /// Loads the aged snapshot for `key_fp` as a fallback serve.
    /// `None` when no readable index exists.
    fn aged_snapshot(
        &self,
        key_fp: &str,
        principal_tag: &str,
        resolution: IndexResolution,
    ) -> Option<BuiltIndex> {
        let meta = self.core.lookup(
            CacheCategory::Catalog,
            principal_tag,
            key_fp,
            SystemTime::now(),
        )?;
        self.load_index(&meta).map(|record| BuiltIndex {
            record: Arc::new(record),
            meta: Some(meta),
            resolution,
        })
    }

    /// Pages through the provider building the full index record.
    /// Honors cancel/deadline per page and the configured entry/page
    /// budgets; the result's `completeness` is `Complete` only when
    /// the provider walk finished un-truncated.
    fn walk_catalog(
        &self,
        provider: &dyn AssetProvider,
        key: &CatalogCacheKey,
        cx: &ProviderContext,
    ) -> Result<CatalogIndexRecord, CacheError> {
        let policy = &self.core.policy;
        let max_entries = policy.max_index_entries;
        let page_limit = u32::try_from(max_entries).unwrap_or(u32::MAX);
        let mut entries: Vec<BrowseEntry> = Vec::new();
        let mut provider_cursor: Option<OpaquePageCursor> = None;
        let mut pages = 0u32;
        let mut done = false;
        let mut partial_reason = None;
        let mut last_coverage = Coverage::Complete;
        let mut last_observed = cx.now();

        loop {
            cx.check_interrupts()?;
            if pages >= policy.max_index_pages {
                partial_reason = Some(IndexPartialReason::PageBudget);
                break;
            }
            pages += 1;
            let page = match &key.query {
                CatalogQuery::Browse { sort, filter } => provider.browse(
                    &BrowseRequest {
                        scope: key.scope.clone(),
                        page_limit,
                        cursor: provider_cursor.clone(),
                        sort: *sort,
                        filter: filter.clone(),
                    },
                    cx,
                )?,
                CatalogQuery::Search { text } => provider.search(
                    &SearchQuery {
                        text: text.clone(),
                        scope: key.scope.clone(),
                        page_limit,
                        cursor: provider_cursor.clone(),
                    },
                    cx,
                )?,
            };
            last_coverage = page.coverage;
            last_observed = page.observed_at;
            let incoming = page.entries.len();
            if entries.len() + incoming > max_entries {
                let room = max_entries - entries.len();
                entries.extend(page.entries.into_iter().take(room));
                partial_reason = Some(IndexPartialReason::EntryBudget);
                break;
            }
            entries.extend(page.entries);
            provider_cursor = page.next_cursor;
            if provider_cursor.is_none() {
                done = true;
                break;
            }
            if entries.len() >= max_entries {
                partial_reason = Some(IndexPartialReason::EntryBudget);
                break;
            }
        }

        let completeness =
            if done && last_coverage == Coverage::Complete && partial_reason.is_none() {
                IndexCompleteness::Complete
            } else {
                IndexCompleteness::Partial {
                    reason: partial_reason.unwrap_or(IndexPartialReason::ProviderTruncated),
                }
            };
        Ok(CatalogIndexRecord {
            format_version: INDEX_FORMAT,
            indexed_at: cx.now(),
            observed_at: last_observed,
            completeness,
            entries,
        })
    }

    /// Publishes `record` as `index.json` + `meta.json` and commits
    /// the entry. `Ok(None)` when the payload exceeds
    /// `max_index_payload_bytes` (serve uncached) or the credential
    /// epoch moved mid-build — store I/O errors propagate; the caller
    /// serves uncached either way.
    fn commit_catalog_index(
        &self,
        key: &CatalogCacheKey,
        principal_tag: &str,
        key_fp: &str,
        record: &CatalogIndexRecord,
    ) -> Result<Option<EntryMeta>, CacheError> {
        let payload = serde_json::to_vec(record)
            .map_err(|e| CacheError::internal(format!("index encode: {e}")))?;
        if payload.len() as u64 > self.core.policy.max_index_payload_bytes {
            return Ok(None);
        }
        let sweep_tag = sweep_tag_for(key, principal_tag);
        // The cursor instance token is the payload fingerprint —
        // persisted in the sidecar so it survives restarts and any
        // rebuild produces a different token.
        let index_fp = short_fingerprint(&payload);
        // TTL anchors on the cache's wall clock, not the provider's
        // observation clock — a skewed or fixed logical clock must
        // never make a fresh entry born-expired.
        let committed_at = SystemTime::now();
        let sidecar = SidecarMeta::Catalog {
            format_version: SIDECAR_FORMAT,
            sweep_tag: sweep_tag.clone(),
            generation: key.generation.0,
            credential_epoch: key.credential_epoch,
            indexed_at: record.indexed_at,
            committed_at,
            observed_at: record.observed_at,
            completeness: record.completeness,
            entry_count: record.entries.len() as u64,
            index_fp: index_fp.clone(),
        };
        let sidecar_bytes = serde_json::to_vec(&sidecar)
            .map_err(|e| CacheError::internal(format!("sidecar encode: {e}")))?;
        let rel_dir = CacheCore::rel_dir(CacheCategory::Catalog, principal_tag, key_fp);
        let dir = self.core.entry_dir(&rel_dir);
        CacheCore::publish_file(&dir.join(CATALOG_PAYLOAD), &payload, PublishMode::Replace)?;
        CacheCore::publish_file(
            &dir.join(SIDECAR_NAME),
            &sidecar_bytes,
            PublishMode::Replace,
        )?;
        let expires_at = committed_at
            .checked_add(self.core.policy.catalog_ttl)
            .unwrap_or(committed_at);
        let meta = EntryMeta {
            rel_dir,
            category: CacheCategory::Catalog,
            payload_bytes: payload.len() as u64 + sidecar_bytes.len() as u64,
            last_used_at: SystemTime::now(),
            expires_at,
            leases: 0,
            sweep_tag,
            generation: key.generation.0,
            credential_epoch: key.credential_epoch,
            principal: Some(key.principal.clone()),
            aux: EntryAux::Catalog {
                completeness: record.completeness,
                indexed_at: record.indexed_at,
                index_fp,
            },
            instance: 0,
        };
        // `None` = credential epoch moved mid-build or the entry was
        // evicted by its own budget enforcement — serve uncached.
        Ok(self.core.commit_entry(meta, SystemTime::now()))
    }

    /// Reads and parses an entry's `index.json`. Corrupt or missing
    /// payloads retire the entry and return `None` — a damaged
    /// cache degrades to a rebuild, never a user-facing failure.
    fn load_index(&self, meta: &EntryMeta) -> Option<CatalogIndexRecord> {
        let path = self.core.payload_path(meta);
        let result = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CatalogIndexRecord>(&bytes).ok());
        match result {
            Some(record) if record.format_version == INDEX_FORMAT => Some(record),
            _ => {
                self.core.remove_entry(&meta.rel_dir);
                None
            }
        }
    }

    /// Serves a cursor continuation page. Cursors never trigger a
    /// refresh — an expired snapshot continues honestly as `Stale`.
    fn serve_cursor_page(
        &self,
        key_fp: &str,
        meta: Option<EntryMeta>,
        cursor: &OpaquePageCursor,
        page_limit: u32,
    ) -> Result<BrowsePage, CacheError> {
        let Some(meta) = meta else {
            return Err(cursor_invalid("catalog index is no longer cached"));
        };
        let payload = decode_cache_cursor(cursor, key_fp)?;
        let Some(index_fp) = index_fp_of(&meta) else {
            return Err(cursor_invalid("catalog index is no longer cached"));
        };
        if payload.index_fp != index_fp {
            return Err(cursor_invalid("index was rebuilt — restart paging"));
        }
        let Some(record) = self.load_index(&meta) else {
            return Err(cursor_invalid("catalog index is unavailable"));
        };
        let freshness =
            freshness_override(Some(&meta), IndexResolution::StaleServe, SystemTime::now());
        Self::serve_page(
            key_fp,
            Some(&meta),
            &record,
            payload.offset,
            page_limit,
            freshness,
        )
    }

    /// Slices `record` at `offset` into a page, minting a cache
    /// cursor when `meta` is present and entries remain.
    fn serve_page(
        key_fp: &str,
        meta: Option<&EntryMeta>,
        record: &CatalogIndexRecord,
        offset: u64,
        page_limit: u32,
        freshness: Option<Coverage>,
    ) -> Result<BrowsePage, CacheError> {
        let entries = &record.entries;
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(entries.len());
        let end = start
            .saturating_add(usize::try_from(page_limit).unwrap_or(usize::MAX))
            .min(entries.len());
        let page_entries: Vec<BrowseEntry> = entries[start..end].to_vec();
        let has_more = end < entries.len();
        let next_cursor = match (has_more, meta.and_then(index_fp_of)) {
            (true, Some(index_fp)) => Some(encode_cache_cursor(key_fp, index_fp, end as u64)?),
            _ => None,
        };
        let coverage = match freshness {
            Some(coverage) => coverage,
            None => {
                if matches!(record.completeness, IndexCompleteness::Partial { .. }) || has_more {
                    Coverage::Partial
                } else {
                    Coverage::Complete
                }
            }
        };
        Ok(BrowsePage {
            entries: page_entries,
            next_cursor,
            coverage,
            observed_at: record.observed_at,
        })
    }
}
