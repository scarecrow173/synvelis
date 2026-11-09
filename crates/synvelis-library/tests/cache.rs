//! ALIB-03 catalog/preview cache tests (Synvelis #87).
//!
//! A scripted [`AssetProvider`] fixture drives miss/stale/offline/
//! truncation/denial cases deterministically; `LocalFolderProvider`
//! covers the real-provider path. Principal isolation, bounded
//! budgets, single-flight, cancellation, generation suppression, and
//! the RT queue are exercised through the public `LibraryCache` API.

mod common;

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use common::*;
use synvelis_library::{
    AccessMode, AssetProvider, Availability, BrowseEntry, BrowsePage, BrowseRequest, BrowseScope,
    ByteRange, CacheError, CachePolicy, CacheStats, Capability, CatalogGeneration, CatalogQuery,
    Coverage, EntryFacts, EntryKind, FetchClaims, FetchCompletion, FetchSlot, FilterSpec,
    IndexCompleteness, IndexPartialReason, IndexStatus, LibraryCache, LibraryEvent,
    LibraryEventKind, LibraryEventSink, LibraryGeneration, LocalFolderProvider, ObjectIdentity,
    ObservedAt, PinCapability, PinProof, PinnedAssetRef, PinnedRead, PreviewFormat, PreviewPlan,
    PreviewRequest, ProviderCapabilities, ProviderContext, ProviderError, ProviderErrorKind,
    ProviderId, ProviderKind, RemoteAssetRef, RetireReason, RevisionKey, RtEventQueue, SearchQuery,
    SortSpec, SuppressingSink, UnsupportedReason, VerificationPolicy,
};

// ---------------------------------------------------------------------------
// Scripted provider fixture
// ---------------------------------------------------------------------------

#[derive(Default)]
struct FixtureState {
    entries: Vec<BrowseEntry>,
    objects: BTreeMap<String, Vec<u8>>,
    /// Provider-side page size for browse/search walks.
    page_size: usize,
    /// Report `Coverage::Partial` on the terminal page (truncation).
    partial_terminal: bool,
    /// Injected browse/search fault.
    catalog_err: Option<ProviderErrorKind>,
    /// Injected `pin`/`open_preview` fault.
    preview_err: Option<ProviderErrorKind>,
    /// When true, pins report `expected_size: None`.
    unknown_size: bool,
    /// When true, `search` returns `UnsupportedCapability`.
    search_unsupported: bool,
    browse_calls: usize,
    search_calls: usize,
    pin_calls: usize,
    preview_calls: usize,
    /// Fires once when the first browse call starts (single-flight).
    browse_started: Option<std::sync::mpsc::Sender<()>>,
    /// When set, the browse leader blocks on `recv` — holds the
    /// single-flight slot open so waiters pile up deterministically.
    browse_release: Option<std::sync::mpsc::Receiver<()>>,
}

struct Fixture {
    id: ProviderId,
    state: Mutex<FixtureState>,
}

impl Fixture {
    fn new(raw: u128) -> Self {
        Self {
            id: ProviderId::new(raw),
            state: Mutex::new(FixtureState {
                // Effectively unbounded provider-side page: one
                // provider call per index walk unless a test shrinks
                // `page_size` to exercise page budgets.
                page_size: usize::MAX,
                ..FixtureState::default()
            }),
        }
    }

    fn with_entries(self, names: &[&str]) -> Self {
        let mut st = self.state.lock().unwrap();
        st.entries = names.iter().map(|n| entry(n)).collect();
        drop(st);
        self
    }

    fn object(&self, locator: &str, bytes: &[u8]) {
        self.state
            .lock()
            .unwrap()
            .objects
            .insert(locator.to_owned(), bytes.to_vec());
    }

    fn edit<R>(&self, f: impl FnOnce(&mut FixtureState) -> R) -> R {
        f(&mut self.state.lock().unwrap())
    }

    fn calls(&self) -> (usize, usize, usize, usize) {
        let st = self.state.lock().unwrap();
        (
            st.browse_calls,
            st.search_calls,
            st.pin_calls,
            st.preview_calls,
        )
    }
}

fn entry(name: &str) -> BrowseEntry {
    BrowseEntry {
        locator: loc(&format!("loc/{name}")),
        display_name: name.to_owned(),
        kind: EntryKind::Asset,
        availability: Availability::Available,
        facts: EntryFacts::all_unknown(),
        via_symlink: false,
    }
}

fn page_of(
    items: &[BrowseEntry],
    cursor: Option<&synvelis_library::OpaquePageCursor>,
    page_size: usize,
    partial_terminal: bool,
    observed: ObservedAt,
) -> BrowsePage {
    let offset = cursor
        .and_then(|c| c.as_str().strip_prefix("off:"))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let end = offset.saturating_add(page_size).min(items.len());
    let has_more = end < items.len();
    BrowsePage {
        entries: items[offset.min(items.len())..end].to_vec(),
        next_cursor: has_more.then(|| {
            synvelis_library::OpaquePageCursor::new(format!("off:{end}")).expect("cursor")
        }),
        coverage: if !has_more && partial_terminal {
            Coverage::Partial
        } else {
            Coverage::Complete
        },
        observed_at: observed,
    }
}

impl AssetProvider for Fixture {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::LocalFolder
    }

    fn capabilities(&self, _asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
        ProviderCapabilities {
            browse: Capability::Supported,
            stat: Capability::Supported,
            search: if self.state.lock().unwrap().search_unsupported {
                Capability::Unsupported {
                    reason: UnsupportedReason::RequiresIndex,
                }
            } else {
                Capability::Supported
            },
            pin: PinCapability::ImmutableObject,
            range_read: Capability::Supported,
            preview: Capability::Supported,
            fetch: Capability::Supported,
            checksum_hint: Capability::Unsupported {
                reason: UnsupportedReason::NotSupportedByBackend,
            },
            access: AccessMode::ReadOnly,
        }
    }

    fn browse(
        &self,
        req: &BrowseRequest,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        cx.check_interrupts()?;
        let (page_size, partial, err, release, started) = {
            let mut st = self.state.lock().unwrap();
            st.browse_calls += 1;
            (
                st.page_size,
                st.partial_terminal,
                st.catalog_err,
                st.browse_release.take(),
                st.browse_started.take(),
            )
        };
        if let Some(tx) = started {
            let _ = tx.send(());
        }
        if let Some(kind) = err {
            return Err(ProviderError::new(kind, "injected catalog fault"));
        }
        if let Some(rx) = release {
            let _ = rx.recv();
        }
        let st = self.state.lock().unwrap();
        Ok(page_of(
            &st.entries,
            req.cursor.as_ref(),
            page_size,
            partial,
            cx.now(),
        ))
    }

    fn stat(
        &self,
        _asset: &RemoteAssetRef,
        _cx: &ProviderContext,
    ) -> Result<synvelis_library::AssetStat, ProviderError> {
        Err(ProviderError::unsupported("stat", "fixture"))
    }

    fn pin(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError> {
        cx.check_interrupts()?;
        let (bytes, unknown) = {
            let mut st = self.state.lock().unwrap();
            st.pin_calls += 1;
            let b = st.objects.get(asset.locator.as_str()).cloned();
            (b, st.unknown_size)
        };
        let expected_size = if unknown {
            None
        } else {
            bytes.as_ref().map(|b| b.len() as u64)
        };
        Ok(PinnedAssetRef {
            asset: asset.clone(),
            proof: PinProof::ImmutableObject {
                revision: RevisionKey::new(format!("rev:{}", asset.locator.as_str()))
                    .expect("revision"),
            },
            object_identity: ObjectIdentity::new(format!("obj:{}", asset.locator.as_str()))
                .expect("identity"),
            expected_size,
            digest_hint: None,
        })
    }

    fn open_pinned(
        &self,
        _pin: &PinnedAssetRef,
        _cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        Err(ProviderError::unsupported("open_pinned", "fixture"))
    }

    fn search(
        &self,
        query: &SearchQuery,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        cx.check_interrupts()?;
        let (page_size, partial, err, unsupported) = {
            let mut st = self.state.lock().unwrap();
            st.search_calls += 1;
            (
                st.page_size,
                st.partial_terminal,
                st.catalog_err,
                st.search_unsupported,
            )
        };
        if unsupported {
            return Err(ProviderError::unsupported("search", "fixture"));
        }
        if let Some(kind) = err {
            return Err(ProviderError::new(kind, "injected search fault"));
        }
        let needle = query.text.to_lowercase();
        let st = self.state.lock().unwrap();
        let hits: Vec<BrowseEntry> = st
            .entries
            .iter()
            .filter(|e| e.display_name.to_lowercase().contains(&needle))
            .cloned()
            .collect();
        Ok(page_of(
            &hits,
            query.cursor.as_ref(),
            page_size,
            partial,
            cx.now(),
        ))
    }

    fn open_preview(
        &self,
        asset: &RemoteAssetRef,
        range: Option<ByteRange>,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        cx.check_interrupts()?;
        let (bytes, err) = {
            let mut st = self.state.lock().unwrap();
            st.preview_calls += 1;
            (
                st.objects.get(asset.locator.as_str()).cloned(),
                st.preview_err,
            )
        };
        if let Some(kind) = err {
            return Err(ProviderError::new(kind, "injected preview fault"));
        }
        let bytes = bytes.unwrap_or_default();
        let (slice, claims_size) = match range {
            Some(r) => {
                let start = usize::try_from(r.offset)
                    .unwrap_or(usize::MAX)
                    .min(bytes.len());
                let end = start
                    .saturating_add(usize::try_from(r.len).unwrap_or(usize::MAX))
                    .min(bytes.len());
                (&bytes[start..end], Some((end - start) as u64))
            }
            None => (&bytes[..], Some(bytes.len() as u64)),
        };
        let identity =
            ObjectIdentity::new(format!("obj:{}", asset.locator.as_str())).expect("identity");
        Ok(PinnedRead {
            reader: Box::new(Cursor::new(slice.to_vec())),
            claims: FetchClaims {
                object_identity: identity.clone(),
                expected_size: claims_size,
                verification: VerificationPolicy::Unverified,
            },
            completion: FetchCompletion::new(Arc::new(FetchSlot::new())),
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn open_cache(tag: &str, policy: CachePolicy) -> (TempTree, LibraryCache) {
    let tree = TempTree::new(tag);
    let cache = LibraryCache::open(tree.join("cache"), policy).expect("open cache");
    (tree, cache)
}

fn browse_req_limit(limit: u32) -> BrowseRequest {
    let mut req = browse_req(limit);
    req.cursor = None;
    req
}

fn preview_request(region: Option<ByteRange>) -> PreviewRequest {
    PreviewRequest {
        format: PreviewFormat {
            decoder: "test".to_owned(),
            version: 1,
        },
        region,
        allow_full_object: false,
    }
}

fn read_all(lease: &mut synvelis_library::PreviewLease) -> Vec<u8> {
    let mut out = Vec::new();
    lease.read_to_end(&mut out).expect("read preview");
    out
}

fn stats(cache: &LibraryCache) -> CacheStats {
    cache.stats()
}

// ---------------------------------------------------------------------------
// Catalog tests
// ---------------------------------------------------------------------------

#[test]
fn catalog_miss_build_hit_and_cache_pages() {
    let fixture = Arc::new(Fixture::new(11).with_entries(&["a", "b", "c", "d", "e"]));
    let (_tree, cache) = open_cache("cat-basic", CachePolicy::default());
    let cx = cx();

    let page1 = cache
        .browse(&*fixture, &browse_req_limit(2), CatalogGeneration(0), &cx)
        .expect("page1");
    assert_eq!(page1.entries.len(), 2);
    // A live continuation cursor means a partial view — the same
    // coverage contract real providers return.
    assert_eq!(page1.coverage, Coverage::Partial);
    let cursor = page1.next_cursor.clone().expect("cursor");
    // The whole index was built in one provider walk; serving pages
    // costs no provider calls.
    assert_eq!(fixture.calls().0, 1);

    let mut req2 = browse_req_limit(2);
    req2.cursor = Some(cursor);
    let page2 = cache
        .browse(&*fixture, &req2, CatalogGeneration(0), &cx)
        .expect("page2");
    assert_eq!(page2.entries.len(), 2);
    assert_eq!(fixture.calls().0, 1, "cursor page must not rebuild");

    let cursor2 = page2.next_cursor.clone().expect("cursor2");
    let mut req3 = browse_req_limit(2);
    req3.cursor = Some(cursor2);
    let page3 = cache
        .browse(&*fixture, &req3, CatalogGeneration(0), &cx)
        .expect("page3");
    assert_eq!(page3.entries.len(), 1);
    assert!(page3.next_cursor.is_none());
    assert_eq!(page3.coverage, Coverage::Complete);
    assert_eq!(fixture.calls().0, 1);

    // A fresh first page is now a cache hit — still no provider call.
    let _ = cache
        .browse(&*fixture, &browse_req_limit(2), CatalogGeneration(0), &cx)
        .expect("hit");
    assert_eq!(fixture.calls().0, 1);
    let st = stats(&cache);
    assert!(st.hits >= 1);
    assert_eq!(st.catalog_entries, 1);
}

#[test]
fn catalog_cursor_rejects_foreign_and_stale() {
    let fixture = Arc::new(Fixture::new(12).with_entries(&["a", "b", "c"]));
    let (_tree, cache) = open_cache("cat-cursor", CachePolicy::default());
    let cx = cx();
    let other = cx_named("other-principal");

    let page = cache
        .browse(&*fixture, &browse_req_limit(1), CatalogGeneration(0), &cx)
        .expect("page");
    let cursor = page.next_cursor.clone().expect("cursor");

    // Same cursor under a different principal → different key → invalid.
    let mut req = browse_req_limit(1);
    req.cursor = Some(cursor.clone());
    let err = cache
        .browse(&*fixture, &req, CatalogGeneration(0), &other)
        .expect_err("foreign principal cursor");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::CursorInvalid
    ));

    // Forged/garbage cursor → invalid, never a provider call.
    let mut req = browse_req_limit(1);
    req.cursor =
        Some(synvelis_library::OpaquePageCursor::new("not-a-cache-cursor").expect("cursor"));
    let err = cache
        .browse(&*fixture, &req, CatalogGeneration(0), &cx)
        .expect_err("forged cursor");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::CursorInvalid
    ));
    assert_eq!(fixture.calls().0, 1);

    // After explicit clear the index is gone → cursor invalid.
    cache.clear().expect("clear");
    let mut req = browse_req_limit(1);
    req.cursor = Some(cursor);
    let err = cache
        .browse(&*fixture, &req, CatalogGeneration(0), &cx)
        .expect_err("cleared cursor");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::CursorInvalid
    ));
}

#[test]
fn catalog_principal_isolation_and_disk_layout() {
    let fixture = Arc::new(Fixture::new(13).with_entries(&["shared"]));
    let (tree, cache) = open_cache("cat-iso", CachePolicy::default());
    let cx_a = cx_named("principal-a");
    let cx_b = cx_named("principal-b");

    cache
        .browse(
            &*fixture,
            &browse_req_limit(10),
            CatalogGeneration(0),
            &cx_a,
        )
        .expect("a");
    cache
        .browse(
            &*fixture,
            &browse_req_limit(10),
            CatalogGeneration(0),
            &cx_b,
        )
        .expect("b");
    // Two principals → two independent index builds.
    assert_eq!(fixture.calls().0, 2);
    assert_eq!(stats(&cache).catalog_entries, 2);

    // Disk layout: <cat>/<64-hex principal tag>/<16-hex key fp> —
    // no raw principal material anywhere under the cache root.
    let cat_dir = tree.join("cache/catalog");
    let ptag_dirs: Vec<_> = std::fs::read_dir(&cat_dir)
        .expect("catalog dir")
        .flatten()
        .collect();
    assert_eq!(ptag_dirs.len(), 2);
    for d in &ptag_dirs {
        assert_eq!(d.file_name().to_string_lossy().len(), 64);
        for kfp in std::fs::read_dir(d.path()).expect("kfp dir").flatten() {
            assert_eq!(kfp.file_name().to_string_lossy().len(), 16);
        }
    }
    // No path under the root contains either raw principal.
    let mut stack = vec![tree.join("cache")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("walk").flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            assert!(!name.contains("principal-a"));
            assert!(!name.contains("principal-b"));
            if e.file_type().expect("ft").is_dir() {
                stack.push(e.path());
            }
        }
    }
}

#[test]
fn catalog_generation_change_sweeps_old_index() {
    let fixture = Arc::new(Fixture::new(14).with_entries(&["x"]));
    let (_tree, cache) = open_cache("cat-gen", CachePolicy::default());
    let cx = cx();

    cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(1), &cx)
        .expect("gen1");
    assert_eq!(stats(&cache).catalog_entries, 1);
    cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(2), &cx)
        .expect("gen2");
    // Same sweep tag, newer generation committed → gen-1 index gone.
    assert_eq!(stats(&cache).catalog_entries, 1);
    assert_eq!(fixture.calls().0, 2);
    // Generation 1 is now a miss again (its entry was swept).
    cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(1), &cx)
        .expect("gen1 again");
    assert_eq!(fixture.calls().0, 3);
    assert_eq!(stats(&cache).catalog_entries, 1);
}

#[test]
fn catalog_search_cached_and_unsupported_propagates() {
    let fixture = Arc::new(Fixture::new(15).with_entries(&["kick.wav", "snare.wav", "hat.wav"]));
    let (_tree, cache) = open_cache("cat-search", CachePolicy::default());
    let cx = cx();
    let query = |text: &str| SearchQuery {
        text: text.to_owned(),
        scope: BrowseScope::Root,
        page_limit: 10,
        cursor: None,
    };

    let page = cache
        .search(&*fixture, &query("wav"), CatalogGeneration(0), &cx)
        .expect("search");
    assert_eq!(page.entries.len(), 3);
    let page2 = cache
        .search(&*fixture, &query("kick"), CatalogGeneration(0), &cx)
        .expect("search2");
    assert_eq!(page2.entries.len(), 1);
    assert_eq!(page2.entries[0].display_name, "kick.wav");
    // Two distinct queries → two index entries; hits on repeat.
    assert_eq!(fixture.calls().1, 2);
    assert_eq!(stats(&cache).catalog_entries, 2);

    // Unsupported search propagates the typed provider error — the
    // cache never fabricates an empty result.
    fixture.edit(|st| st.search_unsupported = true);
    let err = cache
        .search(&*fixture, &query("zzz"), CatalogGeneration(0), &cx)
        .expect_err("unsupported");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::UnsupportedCapability
    ));
}

#[test]
fn catalog_partial_and_page_budget_completeness() {
    // Provider-truncated terminal page → Partial{ProviderTruncated}.
    let fixture = Arc::new(Fixture::new(16).with_entries(&["a", "b", "c"]));
    fixture.edit(|st| st.partial_terminal = true);
    let (_tree, cache) = open_cache("cat-partial", CachePolicy::default());
    let cx = cx();
    let page = cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("page");
    assert_eq!(page.coverage, Coverage::Partial);
    let status = cache
        .index_status(
            fixture.id,
            &BrowseScope::Root,
            &CatalogQuery::Browse {
                sort: SortSpec::default(),
                filter: FilterSpec::default(),
            },
            CatalogGeneration(0),
            &cx,
        )
        .expect("status");
    assert!(matches!(
        status,
        IndexStatus::Fresh {
            completeness: IndexCompleteness::Partial {
                reason: IndexPartialReason::ProviderTruncated
            },
            ..
        }
    ));

    // Page budget → Partial{PageBudget}.
    let fixture2 = Arc::new(Fixture::new(17).with_entries(&["a", "b", "c", "d"]));
    fixture2.edit(|st| st.page_size = 1);
    let policy = CachePolicy {
        max_index_pages: 2,
        ..CachePolicy::default()
    };
    let (_t2, cache2) = open_cache("cat-pagebudget", policy);
    let page = cache2
        .browse(&*fixture2, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("page2");
    assert_eq!(page.entries.len(), 2, "walk stops at page budget");
    assert_eq!(page.coverage, Coverage::Partial);
}

#[test]
fn catalog_offline_and_transient_degrade_honestly() {
    let fixture = Arc::new(Fixture::new(18).with_entries(&["a", "b"]));
    // Zero TTL → every first-page request rebuilds; failures then
    // fall back to the aged snapshot.
    let policy = CachePolicy {
        catalog_ttl: Duration::ZERO,
        ..CachePolicy::default()
    };
    let (_tree, cache) = open_cache("cat-offline", policy.clone());
    let cx = cx();

    cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("initial");
    fixture.edit(|st| st.catalog_err = Some(ProviderErrorKind::Offline));
    let page = cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("offline serve");
    assert_eq!(page.coverage, Coverage::OfflineCached);
    assert_eq!(page.entries.len(), 2);

    fixture.edit(|st| st.catalog_err = Some(ProviderErrorKind::TransientIo));
    let page = cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("stale serve");
    assert_eq!(page.coverage, Coverage::Stale);

    // With no snapshot at all the provider error propagates.
    let fixture2 = Arc::new(Fixture::new(19).with_entries(&["a"]));
    fixture2.edit(|st| st.catalog_err = Some(ProviderErrorKind::Offline));
    let (_t2, cache2) = open_cache("cat-offline-empty", policy.clone());
    let err = cache2
        .browse(&*fixture2, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect_err("offline no cache");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::Offline
    ));
}

#[test]
fn catalog_permission_denied_retires_principal() {
    let fixture = Arc::new(Fixture::new(20).with_entries(&["a"]));
    let policy = CachePolicy {
        catalog_ttl: Duration::ZERO,
        ..CachePolicy::default()
    };
    let (_tree, cache) = open_cache("cat-deny", policy);
    let cx = cx();
    cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("initial");
    assert_eq!(stats(&cache).catalog_entries, 1);

    fixture.edit(|st| st.catalog_err = Some(ProviderErrorKind::PermissionDenied));
    let err = cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect_err("denied");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::PermissionDenied
    ));
    // Defensive retirement: the principal's private entries are gone.
    assert_eq!(stats(&cache).catalog_entries, 0);
}

#[test]
fn catalog_credential_change_invalidates() {
    let fixture = Arc::new(Fixture::new(21).with_entries(&["a"]));
    let (_tree, cache) = open_cache("cat-cred", CachePolicy::default());
    let cx = cx();
    cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("initial");
    assert_eq!(fixture.calls().0, 1);

    cache
        .credentials_changed(&cx.principal)
        .expect("cred change");
    // Old-epoch entry retired; the same query now misses and rebuilds.
    cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("rebuild");
    assert_eq!(fixture.calls().0, 2);
    assert_eq!(stats(&cache).catalog_entries, 1);

    // revoke_principal retires everything for the principal.
    cache.revoke_principal(&cx.principal).expect("revoke");
    assert_eq!(stats(&cache).catalog_entries, 0);
    assert_eq!(stats(&cache).preview_entries, 0);
}

#[test]
fn catalog_single_flight_dedupes_concurrent_builds() {
    let fixture = Arc::new(Fixture::new(22).with_entries(&["a", "b", "c"]));
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    fixture.edit(|st| {
        st.browse_started = Some(started_tx);
        st.browse_release = Some(release_rx);
    });
    let (_tree, cache) = open_cache("cat-flight", CachePolicy::default());
    let cache = Arc::new(cache);
    let cx = cx();

    let f1 = {
        let cache = Arc::clone(&cache);
        let fixture = Arc::clone(&fixture);
        let cx = cx.clone();
        thread::spawn(move || {
            cache.browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        })
    };
    // The leader is inside the provider walk holding the flight
    // slot; a second caller must join as a waiter, not rebuild.
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("browse started");
    let f2 = {
        let cache = Arc::clone(&cache);
        let fixture = Arc::clone(&fixture);
        let cx = cx.clone();
        thread::spawn(move || {
            cache.browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        })
    };
    // Let the waiter park, then release the leader.
    thread::sleep(Duration::from_millis(150));
    let _ = release_tx.send(());
    let p1 = f1.join().expect("join1").expect("page1");
    let p2 = f2.join().expect("join2").expect("page2");
    assert_eq!(p1.entries.len(), p2.entries.len());
    // Exactly one provider walk served both callers.
    assert_eq!(fixture.calls().0, 1);
}

// ---------------------------------------------------------------------------
// Preview tests
// ---------------------------------------------------------------------------

#[test]
fn preview_range_fetch_cached_and_principal_isolated() {
    let fixture = Arc::new(Fixture::new(30));
    fixture.object("obj/a.wav", b"0123456789abcdef");
    let (_tree, cache) = open_cache("prev-basic", CachePolicy::default());
    let cx_a = cx_named("pa");
    let cx_b = cx_named("pb");
    let asset = aref(fixture.id, "obj/a.wav");
    let region = Some(ByteRange { offset: 2, len: 4 });

    let mut lease = cache
        .preview(&*fixture, &asset, &preview_request(region), &cx_a)
        .expect("preview a");
    assert!(lease.is_cached());
    assert_eq!(read_all(&mut lease), b"2345");
    assert_eq!(fixture.calls().3, 1);
    drop(lease);
    assert_eq!(stats(&cache).active_leases, 0, "lease released on drop");

    // Same principal+key → cache hit, no provider call.
    let mut lease = cache
        .preview(&*fixture, &asset, &preview_request(region), &cx_a)
        .expect("preview a hit");
    assert!(lease.is_cached());
    assert_eq!(read_all(&mut lease), b"2345");
    assert_eq!(fixture.calls().3, 1);

    // Different principal → different cache key → new fetch; the
    // private preview is never shared across principals.
    let mut lease = cache
        .preview(&*fixture, &asset, &preview_request(region), &cx_b)
        .expect("preview b");
    assert_eq!(read_all(&mut lease), b"2345");
    assert_eq!(fixture.calls().3, 2);
    assert_eq!(stats(&cache).preview_entries, 2);
}

#[test]
fn preview_full_download_requires_consent() {
    let fixture = Arc::new(Fixture::new(31));
    fixture.object("obj/big.wav", &[7u8; 100]);
    let policy = CachePolicy {
        max_preview_object_bytes: 50,
        max_full_object_bytes: 1_000,
        ..CachePolicy::default()
    };
    let (_tree, cache) = open_cache("prev-consent", policy);
    let cx = cx();
    let asset = aref(fixture.id, "obj/big.wav");

    // Plan reports the full-download requirement explicitly.
    let plan = cache
        .preview_plan(&*fixture, &asset, &preview_request(None), &cx)
        .expect("plan");
    assert_eq!(plan, PreviewPlan::FullDownload { bytes: Some(100) });

    // Without consent → typed refusal, no fetch.
    let err = cache
        .preview(&*fixture, &asset, &preview_request(None), &cx)
        .expect_err("no consent");
    assert!(matches!(err, CacheError::FullDownloadRequired { .. }));
    assert_eq!(fixture.calls().3, 0);

    // With consent → fetches and caches the whole object.
    let mut req = preview_request(None);
    req.allow_full_object = true;
    let mut lease = cache
        .preview(&*fixture, &asset, &req, &cx)
        .expect("consented");
    assert_eq!(read_all(&mut lease).len(), 100);
    assert_eq!(fixture.calls().3, 1);

    // Beyond the hard ceiling → refused even with consent.
    let fixture2 = Arc::new(Fixture::new(32));
    fixture2.object("obj/huge.wav", &vec![1u8; 2_000]);
    let policy2 = CachePolicy {
        max_preview_object_bytes: 50,
        max_full_object_bytes: 1_000,
        ..CachePolicy::default()
    };
    let (_t2, cache2) = open_cache("prev-ceiling", policy2);
    let mut req = preview_request(None);
    req.allow_full_object = true;
    let err = cache2
        .preview(&*fixture2, &aref(fixture2.id, "obj/huge.wav"), &req, &cx)
        .expect_err("ceiling");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::BudgetExceeded
    ));
}

#[test]
fn preview_unknown_size_requires_consent() {
    let fixture = Arc::new(Fixture::new(33));
    fixture.object("obj/mystery.wav", b"abc");
    fixture.edit(|st| st.unknown_size = true);
    let (_tree, cache) = open_cache("prev-unknown", CachePolicy::default());
    let cx = cx();
    let asset = aref(fixture.id, "obj/mystery.wav");

    let plan = cache
        .preview_plan(&*fixture, &asset, &preview_request(None), &cx)
        .expect("plan");
    assert_eq!(plan, PreviewPlan::FullDownload { bytes: None });
    let err = cache
        .preview(&*fixture, &asset, &preview_request(None), &cx)
        .expect_err("unknown no consent");
    assert!(matches!(
        err,
        CacheError::FullDownloadRequired {
            expected_size: None
        }
    ));
    let mut req = preview_request(None);
    req.allow_full_object = true;
    let mut lease = cache
        .preview(&*fixture, &asset, &req, &cx)
        .expect("consented unknown");
    assert_eq!(read_all(&mut lease), b"abc");
}

#[test]
fn preview_ranged_over_budget_refused() {
    let fixture = Arc::new(Fixture::new(34));
    fixture.object("obj/r.wav", &vec![0u8; 1_000]);
    let policy = CachePolicy {
        max_preview_object_bytes: 100,
        ..CachePolicy::default()
    };
    let (_tree, cache) = open_cache("prev-rangecap", policy);
    let cx = cx();
    let asset = aref(fixture.id, "obj/r.wav");
    // A range larger than the object budget is refused outright.
    let err = cache
        .preview(
            &*fixture,
            &asset,
            &preview_request(Some(ByteRange {
                offset: 0,
                len: 500,
            })),
            &cx,
        )
        .expect_err("range over budget");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::BudgetExceeded
    ));
    // The pre-flight plan reports the same refusal honestly.
    let plan = cache
        .preview_plan(
            &*fixture,
            &asset,
            &preview_request(Some(ByteRange {
                offset: 0,
                len: 500,
            })),
            &cx,
        )
        .expect("plan");
    assert_eq!(plan, PreviewPlan::OverBudget { bytes: 500 });
    // Within budget works — and the plan agrees it is bounded.
    let plan = cache
        .preview_plan(
            &*fixture,
            &asset,
            &preview_request(Some(ByteRange {
                offset: 10,
                len: 50,
            })),
            &cx,
        )
        .expect("plan");
    assert_eq!(plan, PreviewPlan::Bounded { bytes: 50 });
    let mut lease = cache
        .preview(
            &*fixture,
            &asset,
            &preview_request(Some(ByteRange {
                offset: 10,
                len: 50,
            })),
            &cx,
        )
        .expect("range ok");
    assert_eq!(read_all(&mut lease).len(), 50);
}

#[test]
fn preview_cancel_and_typed_errors() {
    let fixture = Arc::new(Fixture::new(35));
    fixture.object("obj/c.wav", b"data");
    let (_tree, cache) = open_cache("prev-cancel", CachePolicy::default());
    let cx = cx();
    let asset = aref(fixture.id, "obj/c.wav");

    // Provider denial propagates AND retires the principal.
    cache
        .preview(
            &*fixture,
            &asset,
            &preview_request(Some(ByteRange { offset: 0, len: 2 })),
            &cx,
        )
        .expect("seed");
    assert_eq!(stats(&cache).preview_entries, 1);
    // A different region is a different cache key → miss → fetch.
    // The provider's auth failure must propagate AND retire every
    // entry this principal holds (including the seeded one).
    fixture.edit(|st| st.preview_err = Some(ProviderErrorKind::AuthExpired));
    let err = cache
        .preview(
            &*fixture,
            &asset,
            &preview_request(Some(ByteRange { offset: 0, len: 3 })),
            &cx,
        )
        .expect_err("auth expired");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::AuthExpired
    ));
    assert_eq!(stats(&cache).preview_entries, 0, "denial retires entries");

    // Cancelled context → typed Cancelled before any fetch.
    fixture.edit(|st| st.preview_err = None);
    cx.cancel.cancel();
    let err = cache
        .preview(
            &*fixture,
            &asset,
            &preview_request(Some(ByteRange { offset: 0, len: 2 })),
            &cx,
        )
        .expect_err("cancelled");
    assert!(matches!(
        err,
        CacheError::Provider(e) if e.kind() == ProviderErrorKind::Cancelled
    ));
}

#[test]
fn preview_truncated_source_fails() {
    let fixture = Arc::new(Fixture::new(36));
    fixture.object("obj/short.wav", b"ab");
    let (_tree, cache) = open_cache("prev-trunc", CachePolicy::default());
    let cx = cx();
    // Request 10 bytes from a 2-byte object → the provider delivers
    // fewer → typed failure, nothing cached.
    let err = cache
        .preview(
            &*fixture,
            &aref(fixture.id, "obj/short.wav"),
            &preview_request(Some(ByteRange { offset: 0, len: 10 })),
            &cx,
        )
        .expect_err("truncated");
    assert!(matches!(
        err,
        CacheError::Provider(e)
            if e.kind() == ProviderErrorKind::SourceChanged
                || e.kind() == ProviderErrorKind::IntegrityMismatch
    ));
    assert_eq!(stats(&cache).preview_entries, 0);
}

#[test]
fn cache_clear_and_eviction_preserve_source_of_truth() {
    // Real provider: cache deletion must not touch the source files.
    let tree = TempTree::new("prev-real");
    tree.write("media/tone.wav", b"RIFF-fake-audio-bytes");
    let provider = LocalFolderProvider::new(pid(99), tree.path()).expect("provider");
    let (_t2, cache) = open_cache("prev-real-cache", CachePolicy::default());
    let cx = cx();
    let asset = aref(provider.id(), "media/tone.wav");

    let mut req = preview_request(Some(ByteRange { offset: 0, len: 4 }));
    req.allow_full_object = true;
    let mut lease = cache
        .preview(&provider, &asset, &req, &cx)
        .expect("preview");
    assert_eq!(read_all(&mut lease), b"RIFF");
    assert!(stats(&cache).preview_entries >= 1);

    // Wipe the entire cache — the provider's source is untouched.
    cache.clear().expect("clear");
    assert_eq!(stats(&cache).preview_entries, 0);
    assert!(tree.path().join("media/tone.wav").exists());
    // Provider browse/stat still work; preview refetches cleanly.
    let page = provider
        .browse(&browse_req_limit(10), &cx)
        .expect("browse after clear");
    assert_ne!(page.entries, []);
    let mut lease = cache
        .preview(&provider, &asset, &req, &cx)
        .expect("preview after clear");
    assert_eq!(read_all(&mut lease), b"RIFF");
}

#[test]
fn cache_budget_evicts_but_leases_pin() {
    let fixture = Arc::new(Fixture::new(40));
    for i in 0u8..4 {
        fixture.object(&format!("obj/{i}.bin"), &[i; 1_000]);
    }
    // Disk budget fits ~2 entries; the held lease must survive.
    let policy = CachePolicy {
        max_disk_bytes: 2_500,
        ..CachePolicy::default()
    };
    let disk_budget = policy.max_disk_bytes;
    let (_tree, cache) = open_cache("prev-budget", policy);
    let cx = cx();

    let mut held = cache
        .preview(
            &*fixture,
            &aref(fixture.id, "obj/0.bin"),
            &preview_request(Some(ByteRange {
                offset: 0,
                len: 1_000,
            })),
            &cx,
        )
        .expect("hold");
    assert!(held.is_cached());
    // Fill the budget with more previews — eviction must skip the
    // leased entry.
    for i in 1..4 {
        let lease = cache
            .preview(
                &*fixture,
                &aref(fixture.id, &format!("obj/{i}.bin")),
                &preview_request(Some(ByteRange {
                    offset: 0,
                    len: 1_000,
                })),
                &cx,
            )
            .expect("fill");
        drop(lease);
    }
    let st = stats(&cache);
    assert!(st.evictions > 0, "budget eviction ran");
    assert!(st.disk_bytes <= disk_budget + 2_000);
    assert_eq!(st.active_leases, 1);
    // The leased payload is still fully readable.
    assert_eq!(read_all(&mut held).len(), 1_000);
    drop(held);
    assert_eq!(stats(&cache).active_leases, 0);
}

#[test]
fn cache_open_scan_drops_corrupt_and_half_published() {
    let fixture = Arc::new(Fixture::new(41));
    fixture.object("obj/x.bin", b"payload");
    let tree = TempTree::new("scan");
    let cache_dir = tree.join("cache");
    {
        let cache = LibraryCache::open(&cache_dir, CachePolicy::default()).expect("open1");
        let cx = cx();
        cache
            .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
            .expect("catalog");
        cache
            .preview(
                &*fixture,
                &aref(fixture.id, "obj/x.bin"),
                &preview_request(Some(ByteRange { offset: 0, len: 3 })),
                &cx,
            )
            .expect("preview");
    }
    // Corrupt one catalog sidecar and leave a half-published dir.
    let cat_dir = cache_dir.join("catalog");
    let ptag = std::fs::read_dir(&cat_dir)
        .expect("ptags")
        .next()
        .expect("ptag")
        .expect("ptag entry");
    let kfp_dir = std::fs::read_dir(ptag.path())
        .expect("kfp dirs")
        .next()
        .expect("kfp")
        .expect("kfp entry");
    std::fs::write(kfp_dir.path().join("meta.json"), b"{corrupt").expect("corrupt");
    let stray = cache_dir.join("preview").join("deadbeef").join("cafe");
    std::fs::create_dir_all(&stray).expect("stray");
    std::fs::write(stray.join("payload.bin"), b"orphan").expect("orphan payload");

    // Reopen: corrupt + stray entries are removed, valid preview stays.
    let cache = LibraryCache::open(&cache_dir, CachePolicy::default()).expect("open2");
    let st = stats(&cache);
    assert_eq!(st.catalog_entries, 0, "corrupt catalog entry dropped");
    assert_eq!(st.preview_entries, 1, "valid preview survives scan");
    assert!(!stray.exists(), "half-published dir removed");
    // The surviving preview still serves.
    let cx = cx();
    let mut lease = cache
        .preview(
            &*fixture,
            &aref(fixture.id, "obj/x.bin"),
            &preview_request(Some(ByteRange { offset: 0, len: 3 })),
            &cx,
        )
        .expect("preview after reopen");
    assert_eq!(read_all(&mut lease), b"pay");
    assert_eq!(fixture.calls().3, 1, "served from disk cache, no refetch");
}

// ---------------------------------------------------------------------------
// Notification / generation / RT-queue tests
// ---------------------------------------------------------------------------

#[derive(Default)]
struct RecSink {
    events: Mutex<Vec<LibraryEvent>>,
}

impl LibraryEventSink for RecSink {
    fn emit(&self, event: LibraryEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl LibraryEventSink for &RecSink {
    fn emit(&self, event: LibraryEvent) {
        self.events.lock().unwrap().push(event);
    }
}

#[test]
fn events_carry_generation_and_invalidations_bump() {
    let fixture = Arc::new(Fixture::new(50).with_entries(&["a"]));
    fixture.object("obj/e.bin", b"e");
    let (_tree, cache) = open_cache("evt", CachePolicy::default());
    let sink = Arc::new(RecSink::default());
    cache.subscribe(Arc::clone(&sink) as Arc<dyn LibraryEventSink>);
    let cx = cx();
    let gen0 = cache.generation();

    cache
        .browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        .expect("browse");
    cache
        .preview(
            &*fixture,
            &aref(fixture.id, "obj/e.bin"),
            &preview_request(Some(ByteRange { offset: 0, len: 1 })),
            &cx,
        )
        .expect("preview");

    let events = sink.events.lock().unwrap().clone();
    assert!(
        events
            .iter()
            .any(|e| matches!(e.kind, LibraryEventKind::CatalogIndexed { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e.kind, LibraryEventKind::PreviewStored { .. }))
    );
    // Non-invalidating events carry the current generation.
    assert!(
        events
            .iter()
            .all(|e| e.generation == gen0 || e.generation == cache.generation())
    );

    // Invalidation bumps the generation and emits the retire event.
    cache.credentials_changed(&cx.principal).expect("cred");
    let gen1 = cache.generation();
    assert!(gen1 > gen0);
    let events = sink.events.lock().unwrap().clone();
    assert!(events.iter().any(|e| matches!(
        e.kind,
        LibraryEventKind::PrincipalRetired {
            reason: RetireReason::CredentialChanged,
            ..
        }
    ) && e.generation == gen1));
}

#[test]
fn suppressing_sink_drops_stale_generations() {
    let inner = RecSink::default();
    let sink = SuppressingSink::new(&inner, LibraryGeneration(5));
    let at = ObservedAt(std::time::SystemTime::UNIX_EPOCH);
    sink.emit(LibraryEvent {
        generation: LibraryGeneration(4),
        kind: LibraryEventKind::CacheCleared,
        at,
    });
    sink.emit(LibraryEvent {
        generation: LibraryGeneration(6),
        kind: LibraryEventKind::CacheCleared,
        at,
    });
    sink.emit(LibraryEvent {
        generation: LibraryGeneration(5),
        kind: LibraryEventKind::CacheCleared,
        at,
    });
    let got = inner.events.lock().unwrap();
    // gen4 dropped (older than watermark 5), gen6 passes and
    // advances the watermark, gen5 now stale → dropped.
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].generation, LibraryGeneration(6));
}

#[test]
fn rt_queue_is_bounded_and_accounts_drops() {
    let queue = RtEventQueue::new(2);
    let at = ObservedAt(std::time::SystemTime::UNIX_EPOCH);
    let ev = |n: u64| LibraryEvent {
        generation: LibraryGeneration(n),
        kind: LibraryEventKind::CacheCleared,
        at,
    };
    assert!(queue.try_emit(ev(1)));
    assert!(queue.try_emit(ev(2)));
    // Full → dropped, counted, never blocks.
    assert!(!queue.try_emit(ev(3)));
    assert_eq!(queue.dropped(), 1);
    assert_eq!(queue.len(), 2);
    let drained = queue.drain();
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].generation, LibraryGeneration(1));
    assert_eq!(queue.len(), 0);
    // After drain there is room again.
    assert!(queue.try_emit(ev(4)));
    assert_eq!(queue.dropped(), 1);
}

// ---------------------------------------------------------------------------
// Deadline / cancellation responsiveness
// ---------------------------------------------------------------------------

#[test]
fn catalog_waiter_honours_cancellation() {
    let fixture = Arc::new(Fixture::new(60).with_entries(&["a", "b", "c"]));
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    fixture.edit(|st| {
        st.browse_started = Some(started_tx);
        st.browse_release = Some(release_rx);
    });
    let (_tree, cache) = open_cache("cat-waitcancel", CachePolicy::default());
    let cache = Arc::new(cache);
    let cx_leader = cx();
    let cx_waiter = cx();

    // Leader enters the provider walk holding the flight slot.
    let leader = {
        let cache = Arc::clone(&cache);
        let fixture = Arc::clone(&fixture);
        let cx = cx_leader.clone();
        thread::spawn(move || {
            cache.browse(&*fixture, &browse_req_limit(10), CatalogGeneration(0), &cx)
        })
    };
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("browse started");
    // A caller whose context is already cancelled must fail fast as
    // a flight waiter rather than joining the in-flight build.
    cx_waiter.cancel.cancel();
    let waiter = cache.browse(
        &*fixture,
        &browse_req_limit(10),
        CatalogGeneration(0),
        &cx_waiter,
    );
    assert!(matches!(
        waiter,
        Err(CacheError::Provider(e)) if e.kind() == ProviderErrorKind::Cancelled
    ));
    // Release the leader and let it finish normally.
    let _ = release_tx.send(());
    leader.join().expect("leader join").expect("leader page");
}
