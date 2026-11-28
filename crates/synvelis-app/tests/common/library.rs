//! ALIB-05 shared fixture (synvelis#89) — the in-memory
//! `AssetProvider` fake and the `SynvelisApp` composition the
//! `library_browser` lane drives through `syn.library.*` ops.
//! Adapter-agnostic: no GPUI types appear here.
#![allow(dead_code)]

use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ruviform_ui::BackendReport;
use serde_json::{Value, json};

use synvelis_app::{LibraryImportServices, SynvelisApp};
use synvelis_library::{
    AccessMode, AssetKind, AssetObservation, AssetProvider, AssetStat, Availability, BrowseEntry,
    BrowsePage, BrowseRequest, Capability, Coverage, EntryFacts, EntryKind, FetchClaims,
    FetchCompletion, FetchSlot, MediaFacts, ObjectIdentity, ObservationOrigin, OpaqueLocator,
    OpaquePageCursor, PinCapability, PinProof, PinnedAssetRef, PinnedRead, PrincipalId,
    ProviderCapabilities, ProviderContext, ProviderDescriptor, ProviderError, ProviderErrorKind,
    ProviderId, ProviderKind, ProviderRegistry, RemoteAssetRef, RevisionKey, SearchQuery,
    UnavailableReason, VerificationPolicy,
};
use synvelis_workspace::{LibraryBackend, LibraryJobLedger};

/// Default fake provider id (`ProviderId::new(1)` tagged form).
pub const PROVIDER_TAG: &str = "provider-00000000000000000000000000000001";

/// Bytes the fake's `open_pinned` streams — the verified-import
/// pipeline SHA-256s them; content is opaque to the contract.
pub const ASSET_BYTES: &[u8] = b"fake-asset-bytes-for-import";

/// One-shot channel reader for fake fetch bytes.
struct ChanReader(std::sync::mpsc::Receiver<Vec<u8>>);

impl Read for ChanReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.0.try_recv() {
            Ok(b) => {
                let n = b.len().min(buf.len());
                buf[..n].copy_from_slice(&b[..n]);
                Ok(n)
            }
            Err(_) => Ok(0),
        }
    }
}

/// In-memory fake provider — browse/stat/pin/preview scripted,
/// search optional, `open_pinned` streams [`ASSET_BYTES`] so the
/// verified-import path can run end-to-end against temp-dir ports.
pub struct FakeProvider {
    id: ProviderId,
    kind: ProviderKind,
    caps: ProviderCapabilities,
    pages: Mutex<Vec<BrowsePage>>,
    stats: Mutex<Vec<(String, AssetStat)>>,
    search_results: Mutex<Option<BrowsePage>>,
    preview_bytes: Vec<u8>,
    preview_supported: bool,
    preview_gate: Option<Arc<Gate>>,
    pin_unsupported: bool,
    pinned_supported: bool,
}

impl FakeProvider {
    /// `searchable`/`pinnable`/`preview` script the capability tags;
    /// `pinned_supported` separately gates `open_pinned` (pin evidence
    /// vs fetchable bytes are distinct capabilities in the contract).
    pub fn new(id: u128, searchable: bool, pinnable: bool, preview: bool) -> Self {
        Self {
            id: ProviderId::new(id),
            kind: ProviderKind::LocalFolder,
            caps: ProviderCapabilities {
                browse: Capability::Supported,
                stat: Capability::Supported,
                search: if searchable {
                    Capability::Supported
                } else {
                    Capability::Unsupported {
                        reason: synvelis_library::UnsupportedReason::RequiresIndex,
                    }
                },
                pin: if pinnable {
                    PinCapability::ImmutableObject
                } else {
                    PinCapability::Unsupported {
                        reason: synvelis_library::UnsupportedReason::NotSupportedByBackend,
                    }
                },
                range_read: Capability::Supported,
                preview: if preview {
                    Capability::Supported
                } else {
                    Capability::Unsupported {
                        reason: synvelis_library::UnsupportedReason::NotImplemented,
                    }
                },
                fetch: Capability::Supported,
                checksum_hint: Capability::Unsupported {
                    reason: synvelis_library::UnsupportedReason::NotImplemented,
                },
                access: AccessMode::ReadOnly,
            },
            pages: Mutex::new(Vec::new()),
            stats: Mutex::new(Vec::new()),
            search_results: Mutex::new(None),
            preview_bytes: b"fake-preview".to_vec(),
            preview_supported: preview,
            preview_gate: None,
            pin_unsupported: !pinnable,
            pinned_supported: pinnable,
        }
    }

    /// Gates `open_preview`'s byte stream on `gate` — the reader
    /// blocks until `gate.open()` while still observing the fetch
    /// context's cooperative cancel (the deterministic mid-flight
    /// point for `preview.cancel`/`import.cancel` job tests).
    pub fn with_preview_gate(mut self, gate: Arc<Gate>) -> Self {
        self.preview_gate = Some(gate);
        self
    }

    /// Whether `open_pinned` serves bytes (verified-import fetch).
    pub fn with_pinned_bytes(mut self, supported: bool) -> Self {
        self.pinned_supported = supported;
        self
    }

    /// Appends a scripted browse page (cursor = page index, matching
    /// the workspace fixture's opaque-token convention).
    pub fn with_page(self, page: BrowsePage) -> Self {
        self.pages.lock().unwrap().push(page);
        self
    }

    /// Scripts the provider-side search result page.
    pub fn with_search_page(self, page: BrowsePage) -> Self {
        *self.search_results.lock().unwrap() = Some(page);
        self
    }

    /// Scripts one stat observation keyed by locator — availability
    /// is the field under test; facts stay all-unknown (the contract:
    /// unknown metadata stays unknown).
    pub fn with_stat(self, locator: &str, availability: Availability) -> Self {
        self.stats.lock().unwrap().push((
            locator.to_string(),
            AssetStat {
                asset: RemoteAssetRef {
                    provider: self.id,
                    locator: OpaqueLocator::new(locator).unwrap(),
                },
                availability,
                facts: EntryFacts::all_unknown(),
                provenance: Vec::new(),
            },
        ));
        self
    }
}

impl AssetProvider for FakeProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn kind(&self) -> ProviderKind {
        self.kind.clone()
    }

    fn capabilities(&self, _asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
        self.caps.clone()
    }

    fn browse(
        &self,
        req: &BrowseRequest,
        _cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        let pages = self.pages.lock().unwrap();
        if pages.is_empty() {
            return Ok(BrowsePage {
                entries: Vec::new(),
                next_cursor: None,
                coverage: Coverage::Complete,
                observed_at: synvelis_library::ObservedAt::now(),
            });
        }
        let idx = req
            .cursor
            .as_ref()
            .and_then(|c| c.as_str().parse::<usize>().ok())
            .unwrap_or(0);
        if idx >= pages.len() {
            return Err(ProviderError::new(
                ProviderErrorKind::CursorInvalid,
                "cursor out of range",
            ));
        }
        Ok(pages[idx].clone())
    }

    fn stat(
        &self,
        asset: &RemoteAssetRef,
        _cx: &ProviderContext,
    ) -> Result<AssetStat, ProviderError> {
        let stats = self.stats.lock().unwrap();
        for (loc, stat) in stats.iter() {
            if loc == asset.locator.as_str() {
                return Ok(stat.clone());
            }
        }
        Err(ProviderError::new(
            ProviderErrorKind::NotFound,
            "no stat scripted",
        ))
    }

    fn pin(
        &self,
        asset: &RemoteAssetRef,
        _cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError> {
        if self.pin_unsupported {
            return Err(ProviderError::unsupported("pin", "fake"));
        }
        Ok(PinnedAssetRef {
            asset: asset.clone(),
            proof: PinProof::ImmutableObject {
                revision: RevisionKey::new("commit:abc123").unwrap(),
            },
            object_identity: ObjectIdentity::new("blob:test-asset").unwrap(),
            expected_size: Some(ASSET_BYTES.len() as u64),
            digest_hint: None,
        })
    }

    fn open_pinned(
        &self,
        pin: &PinnedAssetRef,
        _cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        if !self.pinned_supported {
            return Err(ProviderError::unsupported("open_pinned", "fake"));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(ASSET_BYTES.to_vec()).unwrap();
        drop(tx);
        let slot = Arc::new(FetchSlot::new());
        Ok(PinnedRead {
            reader: Box::new(ChanReader(rx)),
            claims: FetchClaims {
                object_identity: pin.object_identity.clone(),
                expected_size: pin.expected_size,
                verification: VerificationPolicy::IdentityBound {
                    scheme: "fake-blob".to_string(),
                },
            },
            completion: FetchCompletion::new(slot),
        })
    }

    fn search(
        &self,
        _query: &SearchQuery,
        _cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        self.search_results
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| ProviderError::unsupported("search", "fake"))
    }

    fn open_preview(
        &self,
        _asset: &RemoteAssetRef,
        _range: Option<synvelis_library::ByteRange>,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        if !self.preview_supported {
            return Err(ProviderError::unsupported("open_preview", "fake"));
        }
        let bytes = self.preview_bytes.clone();
        let slot = Arc::new(FetchSlot::new());
        if let Some(gate) = &self.preview_gate {
            return Ok(PinnedRead {
                reader: Box::new(GatedReader {
                    bytes,
                    gate: gate.clone(),
                    cancel: cx.cancel.clone(),
                    delivered: false,
                }),
                claims: FetchClaims {
                    object_identity: ObjectIdentity::new("prev:test").unwrap(),
                    expected_size: None,
                    verification: VerificationPolicy::Unverified,
                },
                completion: FetchCompletion::new(slot),
            });
        }
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(bytes).unwrap();
        drop(tx);
        Ok(PinnedRead {
            reader: Box::new(ChanReader(rx)),
            claims: FetchClaims {
                object_identity: ObjectIdentity::new("prev:test").unwrap(),
                expected_size: None,
                verification: VerificationPolicy::Unverified,
            },
            completion: FetchCompletion::new(slot),
        })
    }
}

/// A browse entry with observed facts — mirrors the workspace
/// fixture's `entry()` helper.
pub fn entry(name: &str, locator: &str, kind: EntryKind) -> BrowseEntry {
    BrowseEntry {
        locator: OpaqueLocator::new(locator).unwrap(),
        display_name: name.to_string(),
        kind,
        availability: Availability::Available,
        facts: EntryFacts {
            byte_len: AssetObservation::observed(10u64, ObservationOrigin::Provider),
            modified: AssetObservation::unknown(UnavailableReason::NotProvided),
            kind: AssetObservation::observed(AssetKind::File, ObservationOrigin::Provider),
            checksum_hint: AssetObservation::unknown(UnavailableReason::NotProvided),
            media: MediaFacts::all_unknown(),
        },
        via_symlink: false,
    }
}

/// A browse page — cursor is the opaque next-page token.
pub fn page(entries: Vec<BrowseEntry>, coverage: Coverage, cursor: Option<&str>) -> BrowsePage {
    BrowsePage {
        entries,
        next_cursor: cursor.map(|c| OpaquePageCursor::new(c).unwrap()),
        coverage,
        observed_at: synvelis_library::ObservedAt::now(),
    }
}

/// A registry holding one [`FakeProvider`] under `PROVIDER_TAG`.
pub fn registry_with(provider: FakeProvider) -> Arc<ProviderRegistry> {
    let mut registry = ProviderRegistry::new();
    registry
        .register(
            ProviderDescriptor {
                id: provider.id,
                kind: provider.kind.clone(),
                display_name: "fake".to_string(),
            },
            Arc::new(provider),
        )
        .expect("register fake provider");
    Arc::new(registry)
}

/// `SynvelisApp` with a caller-scripted [`FakeProvider`] bound (no
/// import services).
pub fn app_with(provider: FakeProvider) -> SynvelisApp {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    app.set_library_backend(LibraryBackend {
        registry: registry_with(provider),
        principal: PrincipalId::new("test").unwrap(),
    });
    app
}

/// `SynvelisApp` with the library provider backend bound (no import
/// services — import actions land the typed fault). The default
/// provider lists one `loc-a` browse entry so selection/pin/preview/
/// drag flows have a target.
pub fn app() -> SynvelisApp {
    app_with(FakeProvider::new(1, true, true, true).with_page(page(
        vec![entry("a.wav", "loc-a", EntryKind::Asset)],
        Coverage::Complete,
        None,
    )))
}

/// `SynvelisApp` with provider backend + verified-import services over
/// a fresh temp dir (`ImportStore` staging/media + `LocalBlobDir` +
/// `StagedBlobPublisher`). Returns the app and the temp root for
/// assertions/cleanup.
pub fn app_with_import(tag: &str) -> (SynvelisApp, std::path::PathBuf) {
    app_with_import_registry(tag, vec![FakeProvider::new(1, true, true, true)])
}

/// `SynvelisApp` with the *product* provider kind — a real
/// [`synvelis_library::LocalFolderProvider`] rooted at a fresh temp
/// dir — plus verified-import services over a sibling project dir,
/// mirroring `synvelis-native`'s `SYNVELIS_LIBRARY_DIR` /
/// `SYNVELIS_PROJECT_DIR` composition (ALIB-05 native mount,
/// synvelis#89). `files` are `(name, bytes)` fixtures written under
/// the library root so browse/stat/pin/preview/import hit real
/// filesystem state. Returns `(app, library_root, project_dir)`.
pub fn app_with_local_provider(
    tag: &str,
    files: &[(&str, &[u8])],
) -> (SynvelisApp, std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "sv89-local-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let lib_root = dir.join("library");
    let project_dir = dir.join("project");
    std::fs::create_dir_all(&lib_root).expect("library root");
    std::fs::create_dir_all(&project_dir).expect("project dir");
    for (name, bytes) in files {
        let path = lib_root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("fixture parent");
        }
        std::fs::write(path, bytes).expect("fixture file");
    }
    let provider = synvelis_library::LocalFolderProvider::new(ProviderId::new(1), &lib_root)
        .expect("local folder provider");
    let mut registry = ProviderRegistry::new();
    registry
        .register(
            ProviderDescriptor {
                id: provider.id(),
                kind: provider.kind(),
                display_name: "local folder".to_string(),
            },
            Arc::new(provider),
        )
        .expect("register local provider");
    let store =
        synvelis_library::ImportStore::new(project_dir.join("staging"), project_dir.join("media"));
    let blobs = synvelis_library::LocalBlobDir::new(store.blob_dir());
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    app.set_library_backend(LibraryBackend {
        registry: Arc::new(registry),
        principal: PrincipalId::new("test").unwrap(),
    });
    app.set_library_import(LibraryImportServices {
        store,
        blobs: Arc::new(blobs),
        publisher: Arc::new(synvelis_library::StagedBlobPublisher),
    });
    (app, lib_root, project_dir)
}

/// [`app_with_import`] over a caller-composed provider set — each
/// [`FakeProvider`] registers under its own tagged id so
/// provider-switch flows can span connections.
pub fn app_with_import_registry(
    tag: &str,
    providers: Vec<FakeProvider>,
) -> (SynvelisApp, std::path::PathBuf) {
    app_with_publisher(
        tag,
        Arc::new(synvelis_library::StagedBlobPublisher),
        providers,
    )
}

/// [`app_with_import_registry`] with a caller-chosen
/// [`synvelis_library::BlobPublisher`] — gated/failing publisher
/// fixtures drive the job lane's deterministic mid-flight and
/// recovery points.
pub fn app_with_publisher(
    tag: &str,
    publisher: Arc<dyn synvelis_library::BlobPublisher>,
    providers: Vec<FakeProvider>,
) -> (SynvelisApp, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "sv89-libapp-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = synvelis_library::ImportStore::new(dir.join("staging"), dir.join("media"));
    let blobs = synvelis_library::LocalBlobDir::new(store.blob_dir());
    let mut registry = ProviderRegistry::new();
    for provider in providers {
        registry
            .register(
                ProviderDescriptor {
                    id: provider.id,
                    kind: provider.kind.clone(),
                    display_name: "fake".to_string(),
                },
                Arc::new(provider),
            )
            .expect("register fake provider");
    }
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    app.set_library_backend(LibraryBackend {
        registry: Arc::new(registry),
        principal: PrincipalId::new("test").unwrap(),
    });
    app.set_library_import(LibraryImportServices {
        store,
        blobs: Arc::new(blobs),
        publisher,
    });
    (app, dir)
}

/// `syn.library.visual{action:"open"}` → wire `view_id`.
pub fn open_view(app: &mut SynvelisApp) -> String {
    let v = app
        .op_library_visual(&json!({ "action": "open" }))
        .expect("syn.library.visual open");
    v["view_id"].as_str().unwrap().to_string()
}

/// `syn.library.view` projection for `view_id` — named `proj` so it
/// never shadows the `view_id` locals tests bind.
pub fn proj(app: &mut SynvelisApp, view_id: &str) -> Value {
    app.op_library_view(&json!({ "view_id": view_id }))
        .expect("syn.library.view")
}

/// `syn.library.action` — `params` carries the action tag + fields;
/// `view_id` is merged in.
pub fn action(app: &mut SynvelisApp, view_id: &str, params: Value) {
    let mut p = params;
    p.as_object_mut()
        .unwrap()
        .insert("view_id".to_string(), json!(view_id));
    app.op_library_action(&p).expect("syn.library.action");
}

/// `syn.library.providers` catalog.
pub fn providers(app: &SynvelisApp) -> Value {
    app.op_library_providers(&json!({}))
        .expect("syn.library.providers")
}

// ── job lane (ALIB-05) — drain helpers + gate fixtures ─────────────

/// `syn.library.action` returning the ack verbatim — job actions ack
/// `{ok, view_id, job_key, queued: true}`; everything else keeps the
/// `{ok, view_id}` shape.
pub fn action_json(app: &mut SynvelisApp, view_id: &str, params: Value) -> Value {
    let mut p = params;
    p.as_object_mut()
        .unwrap()
        .insert("view_id".to_string(), json!(view_id));
    app.op_library_action(&p).expect("syn.library.action")
}

/// `syn.library.jobs` — the whole ledger projection.
pub fn jobs(app: &mut SynvelisApp) -> Vec<Value> {
    let v = app.op_library_jobs(&json!({})).expect("syn.library.jobs");
    v["jobs"].as_array().cloned().unwrap_or_default()
}

/// The ledger entry for `key`, when registered.
pub fn job(app: &mut SynvelisApp, key: &str) -> Option<Value> {
    jobs(app)
        .into_iter()
        .find(|j| j["job_key"].as_str() == Some(key))
}

/// The ledger key an `import_id` (u64) registers under —
/// `import:<tagged>`, where tagged is `import-<32hex>`.
pub fn import_key(import_id: u64) -> String {
    format!("import:import-{import_id:032x}")
}

/// The ledger key a view's preview fetch registers under —
/// `preview:<vN>` where `vN` is the `ViewInstanceId` display form.
pub fn preview_key(view_id: &str) -> String {
    format!("preview:v{}", view_id.trim_start_matches("view-"))
}

/// Upper bound shared by every ALIB-05 harness wait. Local headless
/// imports complete in well under a second; the bound exists only for
/// Windows CI durable-I/O and scheduling latency, not for a worker that
/// is truly stuck.
pub const JOB_WAIT_TIMEOUT: Duration = Duration::from_secs(15);

/// Poll spacing shared by every ALIB-05 harness wait. Twenty
/// milliseconds keeps the observation well below the durable-I/O time
/// scale without the several-hundred-Hz polling the old helpers used.
pub const JOB_WAIT_INTERVAL: Duration = Duration::from_millis(20);

/// Runs `probe` immediately, then every [`JOB_WAIT_INTERVAL`] until it
/// yields a value or [`JOB_WAIT_TIMEOUT`] elapses. The diagnostic is
/// evaluated only on timeout so passing waits do not serialize extra
/// ledger state.
///
/// # Panics
/// Panics with `timeout_diagnostic` when the deadline elapses.
pub fn bounded_wait<T>(
    description: &str,
    mut probe: impl FnMut() -> Option<T>,
    timeout_diagnostic: impl FnOnce() -> String,
) -> T {
    let started = Instant::now();
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(
            started.elapsed() < JOB_WAIT_TIMEOUT,
            "timed out after {JOB_WAIT_TIMEOUT:?} waiting for {description} (poll interval {JOB_WAIT_INTERVAL:?}): {}",
            timeout_diagnostic()
        );
        std::thread::sleep(JOB_WAIT_INTERVAL);
    }
}

/// Shared handle to the app-owned raw job ledger. Reads through this
/// handle do not run `reconcile_jobs`, so they can observe
/// `commit-pending` without consuming it.
pub fn shared_job_ledger(app: &SynvelisApp) -> Arc<Mutex<LibraryJobLedger>> {
    app.workspace().library().jobs()
}

fn raw_ledger_values(ledger: &Arc<Mutex<LibraryJobLedger>>) -> Option<Vec<Value>> {
    Some(
        ledger
            .lock()
            .ok()?
            .snapshot()
            .into_iter()
            .map(|job| serde_json::to_value(job).expect("library job wire serializes"))
            .collect(),
    )
}

/// The raw ledger entry for `key`, when registered. Unlike [`job`],
/// this read never reconciles a parked commit first.
pub fn raw_job(ledger: &Arc<Mutex<LibraryJobLedger>>, key: &str) -> Option<Value> {
    raw_ledger_values(ledger)?
        .into_iter()
        .find(|j| j["job_key"].as_str() == Some(key))
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unserializable>".to_string())
}

fn job_field(job: &Value, field: &str) -> String {
    job.get(field)
        .map_or_else(|| "<absent>".to_string(), compact_json)
}

fn summarize_raw_job(job: Option<&Value>) -> String {
    job.map_or_else(
        || "<missing>".to_string(),
        |j| {
            format!(
                "state={} stage={} progress={} pending_commit={} runtime_job={} terminal={} cancel_requested={} epoch={}",
                job_field(j, "state"),
                job_field(j, "stage"),
                job_field(j, "progress"),
                job_field(j, "pending_commit"),
                job_field(j, "runtime_job"),
                job_field(j, "terminal"),
                job_field(j, "cancel_requested"),
                job_field(j, "epoch"),
            )
        },
    )
}

fn timeout_classification(job: Option<&Value>) -> &'static str {
    let Some(job) = job else {
        return "ledger-entry-missing: key registration, persistence, or correlation is suspect";
    };
    if job["pending_commit"].as_bool() == Some(true) {
        return "commit-pending-visible: the worker reached its park boundary; direct ledger waits implicate reconcile/replay, while runtime event waits implicate terminal-event delivery";
    }
    match job["state"].as_str() {
        Some("done") => {
            "terminal-present: the ledger reached done, so the reconciled observation or test expectation is the boundary"
        }
        Some("queued") => {
            "worker-not-started: the entry stayed queued without progress, so runtime worker startup or scheduling is suspect"
        }
        Some("running" | "cancel-requested") => {
            "worker-in-flight: compare stage/progress with durable-I/O latency to distinguish a slow import from a true worker stall"
        }
        _ => "state-unknown: inspect the raw ledger fields",
    }
}

/// Timeout diagnostic keyed by the raw ledger — state, stage, progress,
/// `pending_commit`, runtime correlation, and terminal receipt — plus a
/// cause-family hint for triage.
pub fn raw_job_timeout_diagnostic(
    ledger: &Arc<Mutex<LibraryJobLedger>>,
    key: &str,
    waiting_for: &str,
) -> String {
    let Some(jobs) = raw_ledger_values(ledger) else {
        return format!(
            "waiting_for={waiting_for}; key={key}; raw ledger lock unavailable; classification=ledger-observation-failure"
        );
    };
    let raw = jobs.iter().find(|j| j["job_key"].as_str() == Some(key));
    let keys = jobs
        .iter()
        .filter_map(|j| j["job_key"].as_str())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "waiting_for={waiting_for}; key={key}; raw_ledger={}; classification={}; ledger_keys=[{keys}]",
        summarize_raw_job(raw),
        timeout_classification(raw),
    )
}

/// The headless job drain for the direct `op_*` surface: polls
/// `syn.library.jobs` until `key` reaches `done`. The jobs read
/// reconciles a parked commit first, so a returned `committed-live`
/// terminal is already project truth — never a scratch-only receipt.
pub fn wait_job(app: &mut SynvelisApp, key: &str) -> Value {
    let ledger = shared_job_ledger(app);
    let description = format!("job {key} to reach done");
    bounded_wait(
        &description,
        || match job(app, key) {
            Some(j) if j["state"].as_str() == Some("done") => Some(j),
            _ => None,
        },
        || raw_job_timeout_diagnostic(&ledger, key, "done"),
    )
}

/// Polls the ledger until `key` reports `stage` — the deterministic
/// mid-flight observation point for cancel/dedup tests (`queued`,
/// `fetching`, `verifying`, `materializing`; `commit-pending` is
/// transient because the same read reconciles it).
pub fn wait_stage(app: &mut SynvelisApp, key: &str, stage: &str) -> Value {
    let ledger = shared_job_ledger(app);
    let description = format!("job {key} to reach stage {stage}");
    bounded_wait(
        &description,
        || match job(app, key) {
            Some(j) if j["stage"].as_str() == Some(stage) => Some(j),
            _ => None,
        },
        || raw_job_timeout_diagnostic(&ledger, key, &format!("stage {stage}")),
    )
}

/// Polls until the job is visible on the reconcile-first ledger read.
pub fn wait_job_registered(app: &mut SynvelisApp, key: &str) -> Value {
    let ledger = shared_job_ledger(app);
    let description = format!("job {key} to register");
    bounded_wait(
        &description,
        || job(app, key),
        || raw_job_timeout_diagnostic(&ledger, key, "registration"),
    )
}

/// Waits on the raw ledger until `key` either parks at
/// `commit-pending` (`pending_commit=true`) or records a terminal.
/// This read deliberately skips the reconcile-first `syn.library.jobs`
/// surface so restart tests can save the still-owed commit.
pub fn wait_raw_commit_boundary(app: &SynvelisApp, key: &str) -> Value {
    let ledger = shared_job_ledger(app);
    let probe_ledger = Arc::clone(&ledger);
    let description = format!("job {key} to reach pending_commit or terminal");
    bounded_wait(
        &description,
        || {
            raw_job(&probe_ledger, key).and_then(|j| {
                (j["pending_commit"].as_bool() == Some(true) || j["state"].as_str() == Some("done"))
                    .then_some(j)
            })
        },
        || raw_job_timeout_diagnostic(&ledger, key, "pending_commit or terminal"),
    )
}

/// Shared release gate — a fixture parks a job on it, the test
/// releases it deterministically mid-flight.
#[derive(Debug, Default)]
pub struct Gate {
    open: std::sync::atomic::AtomicBool,
}

impl Gate {
    /// A closed gate.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Releases whatever is parked on the gate.
    pub fn open(&self) {
        self.open.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Whether the gate is released.
    pub fn is_open(&self) -> bool {
        self.open.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Preview reader parked on a [`Gate`] — yields all bytes once the
/// gate opens; EOFs early when the fetch context's cancel token is
/// raised (the cooperative-cancel contract `import.cancel` relies on).
struct GatedReader {
    bytes: Vec<u8>,
    gate: Arc<Gate>,
    cancel: synvelis_library::CancellationToken,
    delivered: bool,
}

impl Read for GatedReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while !self.gate.is_open() && !self.cancel.is_cancelled() {
            std::thread::sleep(JOB_WAIT_INTERVAL);
        }
        if self.delivered || self.cancel.is_cancelled() {
            return Ok(0);
        }
        let n = self.bytes.len().min(buf.len());
        buf[..n].copy_from_slice(&self.bytes[..n]);
        self.delivered = true;
        Ok(n)
    }
}

/// [`synvelis_library::BlobPublisher`] parked on a [`Gate`] — blocks
/// inside `publish` until the gate opens while still honoring
/// `cx.check_interrupts` (publish-tail cancel lands the typed
/// `Interrupted(ProviderError)`, which `import_verified` maps to
/// `ImportError::Provider`). The deterministic mid-flight point for
/// `import.cancel` tests — the durable manifest sits at `verified`.
pub struct GatePublisher {
    gate: Arc<Gate>,
}

impl GatePublisher {
    /// A publisher parked on `gate`.
    pub fn new(gate: Arc<Gate>) -> Self {
        Self { gate }
    }
}

impl synvelis_library::BlobPublisher for GatePublisher {
    fn publish(
        &self,
        staged_source: &std::path::Path,
        expected: &synvelis_library::ContentDigest,
        expected_len: u64,
        destination: &std::path::Path,
        durability: synvelis_library::PublishDurability,
        cx: &ProviderContext,
    ) -> Result<synvelis_library::BlobPublishReceipt, synvelis_library::BlobPublishError> {
        while !self.gate.is_open() {
            cx.check_interrupts()
                .map_err(synvelis_library::BlobPublishError::Interrupted)?;
            std::thread::sleep(JOB_WAIT_INTERVAL);
        }
        synvelis_library::StagedBlobPublisher.publish(
            staged_source,
            expected,
            expected_len,
            destination,
            durability,
            cx,
        )
    }
}

/// [`synvelis_library::BlobPublisher`] that fails its first call with
/// an unclassified [`synvelis_library::BlobPublishError::Io`] — the
/// durable manifest sits at `verified` when it lands, so the import
/// job's terminal is `recoverable` and `import.retry` converges once
/// the next publish succeeds through `StagedBlobPublisher`.
#[derive(Debug)]
pub struct FailOncePublisher {
    spent: std::sync::atomic::AtomicBool,
}

impl FailOncePublisher {
    /// A publisher with one scripted failure left.
    pub fn new() -> Self {
        Self {
            spent: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl synvelis_library::BlobPublisher for FailOncePublisher {
    fn publish(
        &self,
        staged_source: &std::path::Path,
        expected: &synvelis_library::ContentDigest,
        expected_len: u64,
        destination: &std::path::Path,
        durability: synvelis_library::PublishDurability,
        cx: &ProviderContext,
    ) -> Result<synvelis_library::BlobPublishReceipt, synvelis_library::BlobPublishError> {
        if self.spent.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return synvelis_library::StagedBlobPublisher.publish(
                staged_source,
                expected,
                expected_len,
                destination,
                durability,
                cx,
            );
        }
        Err(synvelis_library::BlobPublishError::Io(
            std::io::Error::other("scripted first-publish failure"),
        ))
    }
}

/// [`app_with_import_registry`] with a [`GatePublisher`] injected —
/// the import job parks mid-publish until `gate.open()`.
pub fn app_with_import_gate(
    tag: &str,
    gate: Arc<Gate>,
    providers: Vec<FakeProvider>,
) -> (SynvelisApp, std::path::PathBuf) {
    app_with_publisher(tag, Arc::new(GatePublisher::new(gate)), providers)
}
