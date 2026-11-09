//! Bounded non-authoritative catalog/preview cache store (ALIB-03,
//! Synvelis #87).
//!
//! The cache is *derived* data: it never is the asset's true source
//! and never touches the project's verified blob store. Losing the
//! whole cache degrades to provider re-fetch — project save,
//! playback, and undo are unaffected by construction.
//!
//! Layout under the cache root:
//!
//! ```text
//! <root>/catalog/<principal_tag>/<key_fp>/index.json + meta.json
//! <root>/preview/<principal_tag>/<key_fp>/payload.bin + meta.json
//! ```
//!
//! Staging temporaries live beside their target inside the entry
//! directory (the `StagedFile` primitive's doing) and are removed
//! with it on failure or by the open-time scan.
//!
//! `principal_tag` is the SHA-256 hex of the principal scope — the
//! raw principal, credential keys, and secret material never appear
//! on disk, in events, or in stats. `key_fp` is a short fingerprint
//! of the canonical cache key.
//!
//! Entries carry `created_at`/`last_used_at`/`expires_at` and a lease
//! count. Eviction is LRU+TTL under `CachePolicy` budgets and never
//! removes a leased entry; when everything is leased the new entry
//! is simply not admitted (the operation still succeeds uncached).
//! Payload writes go through the ruviform staged-publish primitive,
//! same as the ALIB-02 import manifests.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

use ruviform_io_file::{Durability, PublishMode, PublishOptions, StagedFile};
use serde::{Deserialize, Serialize};

use crate::catalog::IndexCompleteness;
use crate::context::ProviderContext;
use crate::digest::full_fingerprint;
use crate::error::ProviderError;
use crate::id::PrincipalId;
use crate::notify::{EventHub, LibraryEventKind, LibraryGeneration, RetireReason};
use crate::observation::ObservedAt;
use crate::preview::PreviewMeta;

/// Sidecar metadata format version — bump on layout change.
pub(crate) const SIDECAR_FORMAT: u32 = 1;

/// Payload file names inside an entry directory.
pub(crate) const CATALOG_PAYLOAD: &str = "index.json";
pub(crate) const PREVIEW_PAYLOAD: &str = "payload.bin";
pub(crate) const SIDECAR_NAME: &str = "meta.json";
/// Tombstone marker: a retired entry is never resurrected by a
/// restart scan even when its files could not be deleted (e.g. an
/// open preview payload on Windows).
const RETIRED_MARKER: &str = ".retired";

/// Bounded cache policy — every limit is configurable. Defaults are
/// `PoC` values for the headless slice, not security guarantees.
#[derive(Clone, Debug)]
pub struct CachePolicy {
    /// Total payload+sidecar bytes retained on disk.
    pub max_disk_bytes: u64,
    /// Total retained entries (both categories).
    pub max_entries: usize,
    /// Catalog index TTL measured from `indexed_at`.
    pub catalog_ttl: Duration,
    /// Preview TTL measured from `stored_at`.
    pub preview_ttl: Duration,
    /// Maximum entries materialized into one catalog index.
    pub max_index_entries: usize,
    /// Maximum provider pages walked for one index build.
    pub max_index_pages: u32,
    /// Maximum serialized index payload admitted to cache.
    pub max_index_payload_bytes: u64,
    /// Maximum object size admitted as a preview fetch (ranged or
    /// consented full-object).
    pub max_preview_object_bytes: u64,
    /// Hard ceiling for a full-object preview even with explicit
    /// consent — beyond this the fetch is refused outright.
    pub max_full_object_bytes: u64,
    /// Global cap on simultaneously held cache leases.
    pub max_active_leases: u32,
    /// Wait-slice granularity for single-flight waiters so cancel and
    /// deadline stay responsive while a peer builds.
    pub flight_wait_slice: Duration,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            max_disk_bytes: 256 * 1024 * 1024,
            max_entries: 4096,
            catalog_ttl: Duration::from_secs(15 * 60),
            preview_ttl: Duration::from_secs(60 * 60),
            max_index_entries: 50_000,
            max_index_pages: 512,
            max_index_payload_bytes: 64 * 1024 * 1024,
            max_preview_object_bytes: 64 * 1024 * 1024,
            max_full_object_bytes: 512 * 1024 * 1024,
            max_active_leases: 64,
            flight_wait_slice: Duration::from_millis(25),
        }
    }
}

/// Observable cache counters — the bounded-budget evidence surface.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Retained catalog index entries.
    pub catalog_entries: usize,
    /// Retained preview entries.
    pub preview_entries: usize,
    /// Disk bytes accounted (payload + sidecar).
    pub disk_bytes: u64,
    /// Currently held leases.
    pub active_leases: u32,
    /// Lookup hits (provider call avoided).
    pub hits: u64,
    /// Lookup misses.
    pub misses: u64,
    /// Entries evicted by budget enforcement.
    pub evictions: u64,
    /// Entries found past TTL but still retained.
    pub expired_entries: u64,
    /// Entries removed by explicit retire/clear.
    pub retired_entries: u64,
    /// In-flight single-flight builds right now.
    pub flights_in_flight: usize,
}

/// Cache-layer error surface. Provider failures travel unchanged
/// inside [`CacheError::Provider`]; cache-specific refusals are typed
/// separately so callers never confuse them with provider faults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheError {
    /// A provider-layer failure, propagated with its typed kind.
    Provider(ProviderError),
    /// Cache-store I/O failure.
    Io {
        /// Operation tag.
        op: &'static str,
        /// Diagnostic detail.
        detail: String,
    },
    /// The requested preview would fetch the whole object while its
    /// size is unknown or over the preview budget — the caller must
    /// explicitly consent via `allow_full_object`.
    FullDownloadRequired {
        /// Known object size when the provider reported one.
        expected_size: Option<u64>,
    },
    /// The global lease cap is exhausted.
    LeaseExhausted,
    /// Internal cache failure (flight panic, corrupt state).
    Internal {
        /// Diagnostic detail.
        detail: String,
    },
}

impl CacheError {
    /// `true` when this is a provider-layer failure of `kind`.
    #[must_use]
    pub fn is_provider_kind(&self, kind: crate::ProviderErrorKind) -> bool {
        matches!(self, Self::Provider(e) if e.kind() == kind)
    }

    /// The wrapped provider error, when this is one.
    #[must_use]
    pub fn provider_error(&self) -> Option<&ProviderError> {
        match self {
            Self::Provider(e) => Some(e),
            _ => None,
        }
    }

    pub(crate) fn internal(detail: impl Into<String>) -> Self {
        Self::Internal {
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider(e) => write!(f, "provider: {e}"),
            Self::Io { op, detail } => write!(f, "cache io {op}: {detail}"),
            Self::FullDownloadRequired { expected_size } => write!(
                f,
                "preview requires explicit full-download consent (expected_size={expected_size:?})"
            ),
            Self::LeaseExhausted => f.write_str("cache lease limit exhausted"),
            Self::Internal { detail } => write!(f, "cache internal: {detail}"),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<ProviderError> for CacheError {
    fn from(e: ProviderError) -> Self {
        Self::Provider(e)
    }
}

fn io_err(op: &'static str, e: impl std::fmt::Display) -> CacheError {
    CacheError::Io {
        op,
        detail: e.to_string(),
    }
}

/// Cache entry category — selects the disk subtree and payload name.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum CacheCategory {
    Catalog,
    Preview,
}

impl CacheCategory {
    fn dir_name(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Preview => "preview",
        }
    }
}

/// Per-category typed metadata kept in memory and mirrored to the
/// `meta.json` sidecar (never containing raw principal material).
#[derive(Clone, Debug)]
pub(crate) enum EntryAux {
    /// Catalog index facts (from the sidecar).
    Catalog {
        /// Recorded index completeness.
        completeness: IndexCompleteness,
        /// When the index was built.
        indexed_at: ObservedAt,
        /// Fingerprint of the published `index.json` payload — binds
        /// cache cursors to the exact index instance, surviving
        /// restarts (unlike a runtime counter).
        index_fp: String,
    },
    /// Preview facts (from the sidecar).
    Preview(PreviewMeta),
}

/// In-memory metadata for one retained entry.
#[derive(Clone, Debug)]
pub(crate) struct EntryMeta {
    /// Root-relative entry directory (`<category>/<ptag>/<kfp>`).
    pub rel_dir: String,
    /// Entry category.
    pub category: CacheCategory,
    /// Payload + sidecar bytes accounted on disk.
    pub payload_bytes: u64,
    /// Last lookup/insert touch (LRU).
    pub last_used_at: SystemTime,
    /// Absolute expiry — entries past this are refresh candidates,
    /// not automatically deleted.
    pub expires_at: SystemTime,
    /// Active leases pinning this entry against eviction.
    pub leases: u32,
    /// Generation-sweep tag: catalog sweeps on
    /// (provider, scope, principal, credential epoch) when the
    /// caller-pinned generation changes; preview entries retire via
    /// the principal path instead.
    pub sweep_tag: String,
    /// Catalog generation this index was built against (0 for
    /// previews).
    pub generation: u64,
    /// Credential epoch the key was computed under.
    pub credential_epoch: u64,
    /// The raw principal — in-memory only, `None` for entries
    /// rebuilt by the open-time scan (the sidecar stores only the
    /// tag). Used for the commit-time epoch recheck.
    pub principal: Option<PrincipalId>,
    /// Category-typed facts mirrored from the sidecar.
    pub aux: EntryAux,
    /// Unique instance id assigned at commit/scan — lets a
    /// [`CacheLease`] verify it still pins *this* entry and not a
    /// same-`rel_dir` recommit (TTL refresh, clear+refetch).
    pub instance: u64,
}

impl EntryMeta {
    /// `true` when `now` is at or past `expires_at`.
    pub(crate) fn expired(&self, now: SystemTime) -> bool {
        now >= self.expires_at
    }
}

/// Sidecar (`meta.json`) wire format — one per entry directory.
/// Carries no raw principal material: identity lives in the
/// directory names (`<category>/<principal_tag>/<key_fp>`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SidecarMeta {
    /// Catalog index sidecar.
    Catalog {
        /// Wire version.
        format_version: u32,
        /// Generation-sweep tag.
        sweep_tag: String,
        /// Catalog generation.
        generation: u64,
        /// Credential epoch.
        credential_epoch: u64,
        /// Index build time (provider observation clock —
        /// informational only, never the TTL anchor).
        indexed_at: ObservedAt,
        /// Latest provider observation.
        observed_at: ObservedAt,
        /// Wall-clock commit time — the TTL anchor. Cache freshness
        /// is measured on the cache's own clock, not provider or
        /// fixture logical time.
        committed_at: SystemTime,
        /// Recorded completeness.
        completeness: IndexCompleteness,
        /// Entries in the index payload.
        entry_count: u64,
        /// Fingerprint of the `index.json` payload bytes — binds
        /// cache cursors to this exact index instance across
        /// restarts.
        index_fp: String,
    },
    /// Preview sidecar.
    Preview {
        /// Wire version.
        format_version: u32,
        /// Generation-sweep tag (unused for eviction, kept for
        /// symmetry/debugging).
        sweep_tag: String,
        /// Credential epoch.
        credential_epoch: u64,
        /// Cached payload facts.
        meta: PreviewMeta,
        /// Wall-clock commit time — the TTL anchor.
        committed_at: SystemTime,
    },
}

/// Mutable store state guarded by one mutex.
pub(crate) struct StoreState {
    /// `rel_dir` → metadata.
    entries: BTreeMap<String, EntryMeta>,
    /// Sum of `payload_bytes` over entries.
    disk_bytes: u64,
    /// Lookup hits.
    hits: u64,
    /// Lookup misses.
    misses: u64,
    /// Budget evictions.
    evictions: u64,
    /// Explicit retirements.
    retired: u64,
    /// Next entry-instance id — monotonic so a `rel_dir` recommit
    /// never reuses an id a live [`CacheLease`] still pins.
    next_instance: u64,
}

impl StoreState {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            disk_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            retired: 0,
            next_instance: 1,
        }
    }
}

/// One single-flight slot: the leader runs the build; waiters block
/// on the condvar with cancel/deadline checks.
#[derive(Default)]
pub(crate) struct Flight {
    state: Mutex<FlightState>,
    cv: Condvar,
}

#[derive(Default)]
struct FlightState {
    done: bool,
    /// Type-erased leader result — the slot key determines `T`, so
    /// the downcast on the waiter side cannot mismatch in practice.
    result: Option<Result<Arc<dyn std::any::Any + Send + Sync>, CacheError>>,
}

/// Shared cache core — always used through `Arc` so leases can
/// outlive a borrow of `LibraryCache`.
pub(crate) struct CacheCore {
    /// Cache root directory.
    pub root: PathBuf,
    /// Configured budgets.
    pub policy: CachePolicy,
    /// Entry metadata + counters.
    pub state: Mutex<StoreState>,
    /// Single-flight builds keyed by entry `rel_dir`.
    pub flights: Mutex<BTreeMap<String, Arc<Flight>>>,
    /// Event fan-out.
    pub events: EventHub,
    /// Per-principal credential epochs (bumped on credential change).
    pub credentials: Mutex<BTreeMap<PrincipalId, u64>>,
}

impl CacheCore {
    fn lock_state(&self) -> MutexGuard<'_, StoreState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_flights(&self) -> MutexGuard<'_, BTreeMap<String, Arc<Flight>>> {
        self.flights.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_credentials(&self) -> MutexGuard<'_, BTreeMap<PrincipalId, u64>> {
        self.credentials
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The on-disk principal scope tag — full SHA-256 hex so the
    /// namespace separation is not a 64-bit collision surface.
    pub(crate) fn principal_tag(principal: &PrincipalId) -> String {
        full_fingerprint(principal.as_str().as_bytes())
    }

    /// Current credential epoch for `principal` (0 = never changed).
    pub(crate) fn credential_epoch(&self, principal: &PrincipalId) -> u64 {
        self.lock_credentials().get(principal).copied().unwrap_or(0)
    }

    /// Absolute path of an entry directory.
    pub(crate) fn entry_dir(&self, rel_dir: &str) -> PathBuf {
        self.root.join(rel_dir)
    }

    /// Absolute path of the entry's payload file.
    pub(crate) fn payload_path(&self, meta: &EntryMeta) -> PathBuf {
        let name = match meta.category {
            CacheCategory::Catalog => CATALOG_PAYLOAD,
            CacheCategory::Preview => PREVIEW_PAYLOAD,
        };
        self.entry_dir(&meta.rel_dir).join(name)
    }

    /// Absolute path of the entry's sidecar.
    #[allow(dead_code)]
    pub(crate) fn sidecar_path(&self, meta: &EntryMeta) -> PathBuf {
        self.entry_dir(&meta.rel_dir).join(SIDECAR_NAME)
    }

    /// `rel_dir` for an identity triple.
    pub(crate) fn rel_dir(category: CacheCategory, principal_tag: &str, key_fp: &str) -> String {
        format!("{}/{principal_tag}/{key_fp}", category.dir_name())
    }

    /// Looks up an entry by its identity triple. On hit, touches
    /// `last_used_at`; expiry is *not* hidden — callers decide
    /// whether an expired entry is servable (stale/offline) or a
    /// refresh candidate.
    pub(crate) fn lookup(
        &self,
        category: CacheCategory,
        principal_tag: &str,
        key_fp: &str,
        now: SystemTime,
    ) -> Option<EntryMeta> {
        let rel_dir = Self::rel_dir(category, principal_tag, key_fp);
        let mut state = self.lock_state();
        let meta = state.entries.get_mut(&rel_dir)?;
        meta.last_used_at = now;
        Some(meta.clone())
    }

    /// Records a hit for stats.
    pub(crate) fn note_hit(&self) {
        self.lock_state().hits += 1;
    }

    /// Records a miss for stats.
    pub(crate) fn note_miss(&self) {
        self.lock_state().misses += 1;
    }

    /// Commits an entry whose payload+sidecar were already published.
    ///
    /// Verifies the credential epoch is still current (a concurrent
    /// `credentials_changed`/`revoke` between key computation and
    /// commit must not resurrect retired data), assigns a fresh
    /// build id, upserts the metadata, then enforces budgets.
    /// Returns the admitted meta — or `None` when the epoch moved on
    /// or budget enforcement evicted the entry itself (caller must
    /// delete the just-published payload / serve uncached).
    pub(crate) fn commit_entry(&self, mut meta: EntryMeta, now: SystemTime) -> Option<EntryMeta> {
        if let Some(principal) = &meta.principal {
            let current = self.credential_epoch(principal);
            if current != meta.credential_epoch {
                return None;
            }
        }
        meta.last_used_at = now;
        let rel_dir = meta.rel_dir.clone();
        let bytes = meta.payload_bytes;
        let (evicted_count, evicted_bytes, admitted) = {
            let mut state = self.lock_state();
            meta.instance = state.next_instance;
            state.next_instance = state.next_instance.wrapping_add(1);
            if let Some(old) = state.entries.insert(rel_dir.clone(), meta.clone()) {
                state.disk_bytes = state
                    .disk_bytes
                    .saturating_sub(old.payload_bytes)
                    .saturating_add(bytes);
            } else {
                state.disk_bytes = state.disk_bytes.saturating_add(bytes);
            }
            let (count, evicted) = Self::enforce_budget_locked(self, &mut state, now);
            // Budget enforcement may evict the entry just admitted
            // (e.g. it is the only unleased entry and still over
            // budget). That is an honest "cannot cache" outcome —
            // report `None` so callers serve uncached rather than
            // erroring on a vanished store.
            (count, evicted, state.entries.contains_key(&rel_dir))
        };
        if evicted_count > 0 {
            let generation = self.events.generation();
            self.events.emit(
                generation,
                LibraryEventKind::EntriesEvicted {
                    count: evicted_count,
                    bytes: evicted_bytes,
                },
                ObservedAt(now),
            );
        }
        admitted.then_some(meta)
    }

    /// LRU+TTL eviction. Expired entries are preferred victims, then
    /// oldest `last_used_at`; leased entries are never removed. When
    /// only leased entries remain the loop stops — callers tolerate
    /// temporary over-admission by not caching.
    ///
    /// Returns `(count, bytes)` evicted. Caller holds the state lock.
    fn enforce_budget_locked(
        core: &CacheCore,
        state: &mut StoreState,
        now: SystemTime,
    ) -> (u64, u64) {
        let mut count = 0u64;
        let mut bytes = 0u64;
        loop {
            let over_bytes = state.disk_bytes > core.policy.max_disk_bytes;
            let over_count = state.entries.len() > core.policy.max_entries;
            if !over_bytes && !over_count {
                break;
            }
            // Victim: expired-and-unleased first, then oldest use.
            let victim = state
                .entries
                .iter()
                .filter(|(_, m)| m.leases == 0)
                .min_by_key(|(_, m)| (u8::from(!m.expired(now)), m.last_used_at))
                .map(|(k, _)| k.clone());
            let Some(victim) = victim else {
                break;
            };
            if let Some(meta) = state.entries.remove(&victim) {
                state.disk_bytes = state.disk_bytes.saturating_sub(meta.payload_bytes);
                count += 1;
                bytes += meta.payload_bytes;
                // Best-effort file removal — a failure leaves an
                // orphan the open-scan reconciles.
                let _ = fs::remove_dir_all(core.entry_dir(&victim));
            }
        }
        if count > 0 {
            state.evictions += count;
        }
        (count, bytes)
    }

    /// Deletes one entry directory: writes the `.retired` tombstone
    /// first so a failed delete (open file on Windows) can never be
    /// resurrected by a later scan, then removes state + files.
    pub(crate) fn remove_entry(&self, rel_dir: &str) {
        let dir = self.entry_dir(rel_dir);
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(dir.join(RETIRED_MARKER), b"retired\n");
        let _ = fs::remove_dir_all(&dir);
        let mut state = self.lock_state();
        if let Some(meta) = state.entries.remove(rel_dir) {
            state.disk_bytes = state.disk_bytes.saturating_sub(meta.payload_bytes);
            state.retired += 1;
        }
    }

    /// All `rel_dir`s belonging to one principal tag (both
    /// categories) — the unit of principal retirement.
    pub(crate) fn entries_for_principal(&self, principal_tag: &str) -> Vec<String> {
        let state = self.lock_state();
        let needle = format!("/{principal_tag}/");
        state
            .entries
            .keys()
            .filter(|k| k.contains(&needle))
            .cloned()
            .collect()
    }

    /// Removes every catalog entry whose sweep tag matches but whose
    /// generation differs — the "revision changed ⇒ index results
    /// expire" rule.
    pub(crate) fn sweep_catalog_generation(&self, sweep_tag: &str, keep_generation: u64) {
        let victims: Vec<String> = {
            let state = self.lock_state();
            state
                .entries
                .iter()
                .filter(|(_, m)| {
                    m.category == CacheCategory::Catalog
                        && m.sweep_tag == sweep_tag
                        && m.generation != keep_generation
                })
                .map(|(k, _)| k.clone())
                .collect()
        };
        for rel in victims {
            self.remove_entry(&rel);
        }
    }

    /// Retires every entry of one principal (both categories), bumps
    /// the credential epoch so late commits are rejected, and emits
    /// `PrincipalRetired` under a fresh generation.
    pub(crate) fn retire_principal(&self, principal: &PrincipalId, reason: RetireReason) {
        // Bump first: any in-flight key computation still holding the
        // old epoch will fail the commit-time recheck.
        {
            let mut creds = self.lock_credentials();
            let entry = creds.entry(principal.clone()).or_insert(0);
            *entry += 1;
        }
        let generation = self.events.bump();
        let tag = Self::principal_tag(principal);
        for rel in self.entries_for_principal(&tag) {
            self.remove_entry(&rel);
        }
        self.events.emit(
            generation,
            LibraryEventKind::PrincipalRetired {
                principal_fp: tag,
                reason,
            },
            ObservedAt(SystemTime::now()),
        );
    }

    /// Removes every entry (both categories) — explicit clear.
    pub(crate) fn clear_all(&self) {
        let generation = self.events.bump();
        let rels: Vec<String> = {
            let state = self.lock_state();
            state.entries.keys().cloned().collect()
        };
        for rel in rels {
            self.remove_entry(&rel);
        }
        self.events.emit(
            generation,
            LibraryEventKind::CacheCleared,
            ObservedAt(SystemTime::now()),
        );
    }

    /// Acquires a lease on an existing entry. `Ok(None)` when the
    /// entry is gone (evicted/retired); `Err(LeaseExhausted)` when
    /// the global lease cap is hit.
    pub(crate) fn acquire_lease(
        self: &Arc<Self>,
        rel_dir: &str,
    ) -> Result<Option<CacheLease>, CacheError> {
        let mut state = self.lock_state();
        if !state.entries.contains_key(rel_dir) {
            return Ok(None);
        }
        let total_leases: u32 = state.entries.values().map(|m| m.leases).sum();
        if total_leases >= self.policy.max_active_leases {
            return Err(CacheError::LeaseExhausted);
        }
        let meta = state
            .entries
            .get_mut(rel_dir)
            .expect("entry presence checked above");
        meta.leases += 1;
        let instance = meta.instance;
        Ok(Some(CacheLease {
            core: Arc::clone(self),
            rel_dir: rel_dir.to_owned(),
            instance,
        }))
    }

    /// Runs `f` as the single-flight leader for `slot`, or waits for
    /// the in-flight leader and returns a clone of its result.
    /// Waiters re-check cancellation/deadline every
    /// `flight_wait_slice`.
    ///
    /// The leader's `Result` is shared with all waiters — a failed
    /// build fails them all identically (they may then retry as a
    /// new leader on the next call).
    pub(crate) fn run_flight<T>(
        self: &Arc<Self>,
        slot: &str,
        cx: &ProviderContext,
        f: impl FnOnce() -> Result<T, CacheError>,
    ) -> Result<T, CacheError>
    where
        T: Clone + Send + Sync + 'static,
    {
        let (flight, leader) = {
            let mut flights = self.lock_flights();
            match flights.entry(slot.to_owned()) {
                std::collections::btree_map::Entry::Occupied(e) => (Arc::clone(e.get()), false),
                std::collections::btree_map::Entry::Vacant(v) => {
                    let flight = Arc::new(Flight::default());
                    v.insert(Arc::clone(&flight));
                    (flight, true)
                }
            }
        };
        if leader {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
                .unwrap_or_else(|_| Err(CacheError::internal("cache flight panicked")));
            let erased = match &result {
                Ok(t) => Ok(Arc::new(t.clone()) as Arc<dyn std::any::Any + Send + Sync>),
                Err(e) => Err(e.clone()),
            };
            {
                let mut st = flight.state.lock().unwrap_or_else(PoisonError::into_inner);
                st.result = Some(erased);
                st.done = true;
            }
            flight.cv.notify_all();
            self.lock_flights().remove(slot);
            return result;
        }
        // Waiter: poll the flight state with a bounded slice so
        // cancel/deadline stay responsive.
        loop {
            cx.check_interrupts()?;
            let st = flight.state.lock().unwrap_or_else(PoisonError::into_inner);
            if st.done {
                return match st.result.clone() {
                    Some(Ok(value)) => value
                        .downcast::<T>()
                        .map(|arc| arc.as_ref().clone())
                        .map_err(|_| CacheError::internal("flight result type mismatch")),
                    Some(Err(e)) => Err(e),
                    None => Err(CacheError::internal("flight completed without result")),
                };
            }
            let (_guard, _) = flight
                .cv
                .wait_timeout(st, self.policy.flight_wait_slice)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Publishes small bytes atomically via the staged-publish
    /// primitive (`Replace` + file sync).
    pub(crate) fn publish_file(
        path: &Path,
        bytes: &[u8],
        mode: PublishMode,
    ) -> Result<(), CacheError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io_err("create entry dir", e))?;
        }
        let mut staged = StagedFile::create(path).map_err(|e| io_err("stage cache file", e))?;
        staged
            .write_all(bytes)
            .and_then(|()| staged.flush())
            .map_err(|e| io_err("write cache file", e))?;
        staged
            .seal()
            .publish(PublishOptions::new(mode, Durability::FileSync))
            .map_err(|e| io_err("publish cache file", e))?;
        Ok(())
    }

    /// Opens a fresh `StagedFile` at `path` for streaming writes —
    /// the caller seals+publishes after the stream completes.
    pub(crate) fn begin_payload(path: &Path) -> Result<StagedFile, CacheError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io_err("create entry dir", e))?;
        }
        StagedFile::create(path).map_err(|e| io_err("stage payload", e))
    }

    /// Stats snapshot for tests and diagnostics — `now` is the
    /// caller's observation clock so expiry accounting stays in the
    /// same time domain the entries were written under.
    #[must_use]
    pub fn stats_at(&self, now: SystemTime) -> CacheStats {
        let state = self.lock_state();
        let flights = self.lock_flights();
        CacheStats {
            catalog_entries: state
                .entries
                .values()
                .filter(|m| m.category == CacheCategory::Catalog)
                .count(),
            preview_entries: state
                .entries
                .values()
                .filter(|m| m.category == CacheCategory::Preview)
                .count(),
            disk_bytes: state.disk_bytes,
            active_leases: state.entries.values().map(|m| m.leases).sum(),
            hits: state.hits,
            misses: state.misses,
            evictions: state.evictions,
            expired_entries: state.entries.values().filter(|m| m.expired(now)).count() as u64,
            retired_entries: state.retired,
            flights_in_flight: flights.len(),
        }
    }

    /// Stats under the system clock.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        self.stats_at(SystemTime::now())
    }

    /// Rebuilds in-memory state from the on-disk layout. Entries
    /// with a `.retired` marker or unreadable/corrupt sidecars or a
    /// missing payload are deleted rather than trusted.
    fn scan(&self) {
        let mut state = self.lock_state();
        for category in [CacheCategory::Catalog, CacheCategory::Preview] {
            let cat_dir = self.root.join(category.dir_name());
            let Ok(ptag_dirs) = fs::read_dir(&cat_dir) else {
                continue;
            };
            for ptag_dir in ptag_dirs.flatten() {
                let Ok(kfp_dirs) = fs::read_dir(ptag_dir.path()) else {
                    continue;
                };
                for kfp_dir in kfp_dirs.flatten() {
                    let dir = kfp_dir.path();
                    let rel = dir
                        .strip_prefix(&self.root)
                        .map(|r| r.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_default();
                    if dir.join(RETIRED_MARKER).exists() {
                        let _ = fs::remove_dir_all(&dir);
                        continue;
                    }
                    match self.scan_entry(&rel, &dir) {
                        Ok(Some(mut meta)) => {
                            meta.instance = state.next_instance;
                            state.next_instance = state.next_instance.wrapping_add(1);
                            state.disk_bytes = state.disk_bytes.saturating_add(meta.payload_bytes);
                            state.entries.insert(rel, meta);
                        }
                        // Any directory under an entry slot that is
                        // not a complete, trusted entry is removed —
                        // half-published writes never resurrect.
                        _ => {
                            let _ = fs::remove_dir_all(&dir);
                        }
                    }
                }
            }
        }
        // Entries beyond max_entries are trimmed by oldest use;
        // disk-byte overage is left to lazy eviction on next insert.
        while state.entries.len() > self.policy.max_entries {
            let victim = state
                .entries
                .iter()
                .filter(|(_, m)| m.leases == 0)
                .min_by_key(|(_, m)| m.last_used_at)
                .map(|(k, _)| k.clone());
            let Some(victim) = victim else { break };
            if let Some(meta) = state.entries.remove(&victim) {
                state.disk_bytes = state.disk_bytes.saturating_sub(meta.payload_bytes);
                let _ = fs::remove_dir_all(self.entry_dir(&victim));
            }
        }
    }

    /// Parses one on-disk entry into memory metadata. `Ok(None)`
    /// for directories that are not complete entries (e.g. a stray
    /// staging dir).
    fn scan_entry(&self, rel: &str, dir: &Path) -> Result<Option<EntryMeta>, CacheError> {
        let sidecar_path = dir.join(SIDECAR_NAME);
        let bytes = match fs::read(&sidecar_path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_err("read sidecar", e)),
        };
        let sidecar: SidecarMeta =
            serde_json::from_slice(&bytes).map_err(|e| io_err("parse sidecar", e))?;
        if matches!(sidecar, SidecarMeta::Catalog { format_version, .. } | SidecarMeta::Preview { format_version, .. } if format_version != SIDECAR_FORMAT)
        {
            return Err(CacheError::internal("unsupported sidecar format"));
        }
        // The payload must exist for the entry to be admitted.
        let (payload_name, aux, sweep_tag, generation, credential_epoch, committed_at) =
            match &sidecar {
                SidecarMeta::Catalog {
                    sweep_tag,
                    generation,
                    credential_epoch,
                    indexed_at,
                    committed_at,
                    completeness,
                    index_fp,
                    ..
                } => (
                    CATALOG_PAYLOAD,
                    EntryAux::Catalog {
                        completeness: *completeness,
                        indexed_at: *indexed_at,
                        index_fp: index_fp.clone(),
                    },
                    sweep_tag.clone(),
                    *generation,
                    *credential_epoch,
                    *committed_at,
                ),
                SidecarMeta::Preview {
                    sweep_tag,
                    credential_epoch,
                    committed_at,
                    meta,
                    ..
                } => (
                    PREVIEW_PAYLOAD,
                    EntryAux::Preview(meta.clone()),
                    sweep_tag.clone(),
                    0,
                    *credential_epoch,
                    *committed_at,
                ),
            };
        let payload_path = dir.join(payload_name);
        let payload_len = fs::metadata(&payload_path)
            .map_err(|e| io_err("stat payload", e))?
            .len();
        let category = if payload_name == CATALOG_PAYLOAD {
            CacheCategory::Catalog
        } else {
            CacheCategory::Preview
        };
        let ttl = match category {
            CacheCategory::Catalog => self.policy.catalog_ttl,
            CacheCategory::Preview => self.policy.preview_ttl,
        };
        let parts: Vec<&str> = rel.split('/').collect();
        if parts.len() != 3 || parts[0] != category.dir_name() {
            return Err(CacheError::internal("malformed entry path"));
        }
        let sidecar_len = bytes.len() as u64;
        let now = SystemTime::now();
        let expires_at = committed_at.checked_add(ttl).unwrap_or(committed_at);
        Ok(Some(EntryMeta {
            rel_dir: rel.to_owned(),
            category,
            payload_bytes: payload_len.saturating_add(sidecar_len),
            last_used_at: now,
            expires_at,
            leases: 0,
            sweep_tag,
            generation,
            credential_epoch,
            principal: None,
            aux,
            instance: 0,
        }))
    }
}

/// RAII guard pinning one cache entry against eviction. Drop
/// releases the lease; the underlying file may already be gone
/// (explicit retire) — release is then a no-op.
pub(crate) struct CacheLease {
    core: Arc<CacheCore>,
    rel_dir: String,
    /// Instance id captured at acquire — the release only applies
    /// when the entry at `rel_dir` is still that same instance.
    instance: u64,
}

impl CacheLease {
    /// The entry's metadata snapshot.
    pub(crate) fn meta(&self) -> Option<EntryMeta> {
        self.core.lookup_meta(&self.rel_dir)
    }

    /// Absolute payload path for reading — valid while the lease is
    /// held (the entry cannot be evicted, though explicit retire may
    /// still remove files; readers must tolerate `NotFound`).
    pub(crate) fn payload_path(&self) -> Option<PathBuf> {
        self.meta().map(|m| self.core.payload_path(&m))
    }
}

impl Drop for CacheLease {
    fn drop(&mut self) {
        let mut state = self.core.lock_state();
        // Only release the instance this lease actually pinned — a
        // same-`rel_dir` recommit (TTL refresh, clear+refetch) is a
        // different entry whose lease count this guard never held.
        if let Some(meta) = state.entries.get_mut(&self.rel_dir)
            && meta.instance == self.instance
        {
            meta.leases = meta.leases.saturating_sub(1);
        }
    }
}

impl CacheCore {
    /// Metadata lookup by `rel_dir` (no LRU touch — used by leases).
    fn lookup_meta(&self, rel_dir: &str) -> Option<EntryMeta> {
        self.lock_state().entries.get(rel_dir).cloned()
    }
}

/// The user-facing cache handle — owns the shared [`CacheCore`] and
/// exposes the catalog/preview/event API implemented in the sibling
/// modules.
pub struct LibraryCache {
    /// Shared core (Arc so leases can hold it).
    pub(crate) core: Arc<CacheCore>,
}

impl LibraryCache {
    /// Opens (or creates) the cache under `root` with `policy`,
    /// scanning existing entries into memory.
    ///
    /// # Errors
    /// [`CacheError::Io`] when the root cannot be created or scanned.
    pub fn open(root: impl AsRef<Path>, policy: CachePolicy) -> Result<Self, CacheError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root).map_err(|e| io_err("create cache root", e))?;
        for cat in [CacheCategory::Catalog, CacheCategory::Preview] {
            fs::create_dir_all(root.join(cat.dir_name()))
                .map_err(|e| io_err("create cache category dir", e))?;
        }
        let core = Arc::new(CacheCore {
            root,
            policy,
            state: Mutex::new(StoreState::new()),
            flights: Mutex::new(BTreeMap::new()),
            events: EventHub::new(),
            credentials: Mutex::new(BTreeMap::new()),
        });
        core.scan();
        Ok(Self { core })
    }

    /// The cache root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.core.root
    }

    /// Current event generation.
    #[must_use]
    pub fn generation(&self) -> LibraryGeneration {
        self.core.events.generation()
    }

    /// Registers an event sink (Player/Workspace adapter later).
    pub fn subscribe(&self, sink: Arc<dyn crate::LibraryEventSink>) {
        self.core.events.add_sink(sink);
    }

    /// Cache counters under the system clock.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        self.core.stats()
    }

    /// Cache counters evaluated at `now` — deterministic under an
    /// injected observation clock.
    #[must_use]
    pub fn stats_at(&self, now: SystemTime) -> CacheStats {
        self.core.stats_at(now)
    }

    /// Explicitly clears the entire cache (both categories).
    ///
    /// # Errors
    /// Currently infallible in practice; returns `CacheError` for
    /// forward compatibility with strict stores.
    pub fn clear(&self) -> Result<(), CacheError> {
        self.core.clear_all();
        Ok(())
    }

    /// Retires every cached entry of `principal` — used on explicit
    /// permission revocation. Bumps the credential epoch so late
    /// commits cannot resurrect retired data.
    ///
    /// # Errors
    /// [`CacheError`] — currently infallible; kept fallible for
    /// forward compatibility.
    pub fn revoke_principal(&self, principal: &PrincipalId) -> Result<(), CacheError> {
        self.core
            .retire_principal(principal, RetireReason::PermissionRevoked);
        Ok(())
    }

    /// Signals that `principal`'s credentials changed — bumps the
    /// credential epoch (all keys computed under the old epoch miss)
    /// and retires the principal's private entries.
    ///
    /// # Errors
    /// [`CacheError`] — currently infallible; kept fallible for
    /// forward compatibility.
    pub fn credentials_changed(&self, principal: &PrincipalId) -> Result<(), CacheError> {
        self.core
            .retire_principal(principal, RetireReason::CredentialChanged);
        Ok(())
    }

    /// Retires `cx.principal`'s entries after the provider reported
    /// `PermissionDenied`/`AuthExpired` — defensive invalidation so
    /// private cached data is never served under stale authority.
    pub(crate) fn retire_for_provider_denial(&self, cx: &ProviderContext) {
        self.core
            .retire_principal(&cx.principal, RetireReason::ProviderDenied);
    }

    /// Current credential epoch for `principal`.
    pub(crate) fn credential_epoch(&self, principal: &PrincipalId) -> u64 {
        self.core.credential_epoch(principal)
    }
}
