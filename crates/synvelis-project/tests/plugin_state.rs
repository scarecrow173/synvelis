//! TIM-04 (issue #128): plugin-state snapshot consistency — quality
//! classification, content-addressed store, save/restore receipts,
//! Strict fail-closed, indeterminate publish reconciliation and
//! copy-restore quality preservation, all over mock ports/stores.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use synvelis_project::plugin_state::{
    PLUGIN_STATE_SCHEMA_VERSION, PluginStateStoreOperation, PluginStateStorePublishEffect,
    plugin_state_manifest_name,
};
use synvelis_project::*;

// ── Fixtures ─────────────────────────────────────────────────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

/// Adds a track and one processor with `binding`; returns its id.
fn add_processor(c: &mut ProjectController, name: &str, binding: ProcessorBinding) -> ProcessorId {
    c.apply(cmd(c, ProjectOp::AddTrack { name: name.into() }))
        .unwrap();
    let track = *c.live().tracks.keys().next_back().unwrap();
    c.apply(cmd(c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    c.apply(cmd(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: name.into(),
            binding,
            sidechain_input: false,
        },
    ))
    .unwrap();
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn resolved(name: &str) -> ProcessorBinding {
    ProcessorBinding::Resolved(PluginRef {
        descriptor: format!("vendor.{name}"),
        version: "1.0.0".into(),
        capability_revision: 7,
    })
}

fn binding_for(pid: u128) -> PluginStateBinding {
    PluginStateBinding {
        descriptor: format!("vendor.p{pid}"),
        version: "1.0.0".into(),
        capability_revision: 7,
        host_runtime_instance: "mock-host".into(),
        graph_generation: 3,
    }
}

/// Restore call log shared between port and assertions.
type RestoredLog = Rc<RefCell<Vec<(ProcessorId, Vec<u8>)>>>;

/// Scripted port: per-processor capture results, restore outcomes
/// and a call log proving which processors the session asked.
#[derive(Default)]
struct MockPort {
    bindings: BTreeMap<ProcessorId, PluginStateBinding>,
    captures: BTreeMap<ProcessorId, Result<PluginStateCapture, PluginStateCaptureError>>,
    restore_error: BTreeMap<ProcessorId, PluginStateCaptureError>,
    calls: Rc<RefCell<Vec<String>>>,
    restored: RestoredLog,
}

impl MockPort {
    fn capture_ok(blob: &[u8], quality: StateCaptureQuality) -> PluginStateCapture {
        PluginStateCapture {
            blob: blob.to_vec(),
            achieved: quality,
            observation: CaptureObservation {
                processing: quality == StateCaptureQuality::BestEffort,
                stable_boundary: quality != StateCaptureQuality::BestEffort,
            },
            detail: None,
        }
    }

    fn bind_all(&mut self, ids: &[ProcessorId]) {
        for id in ids {
            self.bindings.insert(*id, binding_for(id.raw()));
        }
    }
}

impl PluginStatePort for MockPort {
    fn describe(&self, processor: ProcessorId) -> Option<PluginStateBinding> {
        self.bindings.get(&processor).cloned()
    }

    fn capture(
        &mut self,
        processor: ProcessorId,
        _mode: CaptureMode,
    ) -> Result<PluginStateCapture, PluginStateCaptureError> {
        self.calls.borrow_mut().push(format!("capture:{processor}"));
        self.captures
            .get(&processor)
            .cloned()
            .unwrap_or_else(|| Ok(Self::capture_ok(b"state", StateCaptureQuality::Coordinated)))
    }

    fn restore(
        &mut self,
        processor: ProcessorId,
        bytes: &[u8],
    ) -> Result<(), PluginStateCaptureError> {
        self.calls.borrow_mut().push(format!("restore:{processor}"));
        if let Some(error) = self.restore_error.get(&processor) {
            return Err(error.clone());
        }
        self.restored.borrow_mut().push((processor, bytes.to_vec()));
        Ok(())
    }
}

fn save_request(c: &ProjectController, policy: StateQualityPolicy) -> PluginStateSaveRequest<'_> {
    PluginStateSaveRequest {
        project: c.live(),
        project_revision: c.live_revision().0,
        project_head: c.saved().map(|s| s.head.clone()),
        mode: CaptureMode::Coordinated,
        policy,
        stored_at: "2026-11-09T00:00:00Z".into(),
    }
}

fn restore_request(
    c: &ProjectController,
    manifest: Option<String>,
) -> PluginStateRestoreRequest<'_> {
    PluginStateRestoreRequest {
        manifest,
        project: c.live(),
    }
}

/// Store wrapper injecting publish-effect faults while delegating
/// to a real inner store — exercises the session's reconcile paths.
struct FaultStore {
    inner: InMemoryPluginStateStore,
    /// `Some(true)` = publish then report Indeterminate;
    /// `Some(false)` = report Indeterminate without publishing.
    manifest_indeterminate: Option<bool>,
    blob_indeterminate: Option<bool>,
}

impl FaultStore {
    fn new() -> Self {
        Self {
            inner: InMemoryPluginStateStore::new(),
            manifest_indeterminate: None,
            blob_indeterminate: None,
        }
    }
}

impl PluginStateStore for FaultStore {
    fn publish_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateStoreError> {
        self.inner.publish_manifest(bytes)
    }

    fn update_head(&mut self, name: &str) -> Result<(), PluginStateStoreError> {
        self.inner.update_head(name)
    }

    fn save_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateStoreError> {
        match self.manifest_indeterminate {
            Some(true) => {
                // Publish for real, then report the effect as
                // undetermined — the session must reconcile by
                // HEAD readback.
                self.inner.save_manifest(bytes)?;
                Err(PluginStateStoreError::Indeterminate {
                    operation: PluginStateStoreOperation::HeadUpdate,
                })
            }
            Some(false) => Err(PluginStateStoreError::Indeterminate {
                operation: PluginStateStoreOperation::HeadUpdate,
            }),
            None => self.inner.save_manifest(bytes),
        }
    }

    fn head(&self) -> Result<Option<String>, PluginStateStoreError> {
        self.inner.head()
    }

    fn read_manifest(&self, name: &str) -> Result<Vec<u8>, PluginStateStoreError> {
        self.inner.read_manifest(name)
    }

    fn put_blob(
        &mut self,
        digest: &ContentDigest,
        bytes: &[u8],
    ) -> Result<(), PluginStateStoreError> {
        match self.blob_indeterminate {
            Some(publish) => {
                if publish {
                    self.inner.put_blob(digest, bytes)?;
                }
                Err(PluginStateStoreError::Indeterminate {
                    operation: PluginStateStoreOperation::BlobPublish,
                })
            }
            None => self.inner.put_blob(digest, bytes),
        }
    }

    fn read_blob(&self, digest: &ContentDigest) -> Result<Vec<u8>, PluginStateStoreError> {
        self.inner.read_blob(digest)
    }
}

// ── Permission-denial matrix store ───────────────────────────────

fn injected_permission_denied(operation: PluginStateStoreOperation) -> PluginStateStoreError {
    let effect = match operation {
        PluginStateStoreOperation::BlobPublish
        | PluginStateStoreOperation::ManifestPublish
        | PluginStateStoreOperation::HeadUpdate => PluginStateStorePublishEffect::NotPublished,
        PluginStateStoreOperation::HeadRead
        | PluginStateStoreOperation::ManifestRead
        | PluginStateStoreOperation::BlobRead => PluginStateStorePublishEffect::NotApplicable,
    };
    PluginStateStoreError::PermissionDenied {
        operation,
        effect,
        detail: format!("deterministic fault injection at {operation}"),
    }
}

/// Deterministic store-boundary fault seam for the permission
/// matrix. Unlike an OS permission probe, every denial is injected
/// before the denied write takes effect; the indeterminate cases
/// separately control whether the bytes/HEAD effect was applied.
/// The store keeps its objects in memory so tests can inspect the
/// exact manifest/blob set without filesystem or OS-account
/// dependencies.
struct MatrixStore {
    manifests: BTreeMap<String, Vec<u8>>,
    blobs: BTreeMap<String, Vec<u8>>,
    head_name: Option<String>,
    denied: BTreeSet<PluginStateStoreOperation>,
    calls: Rc<RefCell<Vec<PluginStateStoreOperation>>>,
    blob_indeterminate: Option<bool>,
    head_indeterminate: Option<bool>,
}

impl MatrixStore {
    fn new() -> Self {
        Self {
            manifests: BTreeMap::new(),
            blobs: BTreeMap::new(),
            head_name: None,
            denied: BTreeSet::new(),
            calls: Rc::new(RefCell::new(Vec::new())),
            blob_indeterminate: None,
            head_indeterminate: None,
        }
    }

    fn deny(&mut self, operation: PluginStateStoreOperation) {
        self.denied.insert(operation);
    }

    fn check(&mut self, operation: PluginStateStoreOperation) -> Result<(), PluginStateStoreError> {
        self.calls.borrow_mut().push(operation);
        if self.denied.contains(&operation) {
            return Err(injected_permission_denied(operation));
        }
        Ok(())
    }

    fn manifest_set(&self) -> BTreeMap<String, Vec<u8>> {
        self.manifests.clone()
    }

    fn blob_set(&self) -> BTreeMap<String, Vec<u8>> {
        self.blobs.clone()
    }
}

impl PluginStateStore for MatrixStore {
    fn publish_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateStoreError> {
        self.check(PluginStateStoreOperation::ManifestPublish)?;
        let name = plugin_state_manifest_name(bytes);
        self.manifests.insert(name.clone(), bytes.to_vec());
        Ok(name)
    }

    fn update_head(&mut self, name: &str) -> Result<(), PluginStateStoreError> {
        self.check(PluginStateStoreOperation::HeadUpdate)?;
        if !is_manifest_name(name) {
            return Err(PluginStateStoreError::Failed(format!(
                "invalid manifest name {name:?}"
            )));
        }
        self.head_name = Some(name.to_owned());
        Ok(())
    }

    fn save_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateStoreError> {
        // Direct denial is an admission-control failure injected
        // before any write. This is deliberate: the protection
        // oracle must not depend on whether a real OS happens to
        // deny a rename early or late. Partial effects are covered
        // by the separate indeterminate injection below.
        if self
            .denied
            .contains(&PluginStateStoreOperation::ManifestPublish)
        {
            self.calls
                .borrow_mut()
                .push(PluginStateStoreOperation::ManifestPublish);
            return Err(injected_permission_denied(
                PluginStateStoreOperation::ManifestPublish,
            ));
        }
        if self.denied.contains(&PluginStateStoreOperation::HeadUpdate) {
            self.calls
                .borrow_mut()
                .push(PluginStateStoreOperation::HeadUpdate);
            return Err(injected_permission_denied(
                PluginStateStoreOperation::HeadUpdate,
            ));
        }

        self.calls
            .borrow_mut()
            .push(PluginStateStoreOperation::ManifestPublish);
        let name = plugin_state_manifest_name(bytes);
        self.manifests.insert(name.clone(), bytes.to_vec());
        self.calls
            .borrow_mut()
            .push(PluginStateStoreOperation::HeadUpdate);
        if let Some(applied) = self.head_indeterminate {
            if applied {
                self.head_name = Some(name.clone());
            }
            return Err(PluginStateStoreError::Indeterminate {
                operation: PluginStateStoreOperation::HeadUpdate,
            });
        }
        self.head_name = Some(name.clone());
        Ok(name)
    }

    fn head(&self) -> Result<Option<String>, PluginStateStoreError> {
        self.calls
            .borrow_mut()
            .push(PluginStateStoreOperation::HeadRead);
        if self.denied.contains(&PluginStateStoreOperation::HeadRead) {
            return Err(injected_permission_denied(
                PluginStateStoreOperation::HeadRead,
            ));
        }
        Ok(self.head_name.clone())
    }

    fn read_manifest(&self, name: &str) -> Result<Vec<u8>, PluginStateStoreError> {
        self.calls
            .borrow_mut()
            .push(PluginStateStoreOperation::ManifestRead);
        if self
            .denied
            .contains(&PluginStateStoreOperation::ManifestRead)
        {
            return Err(injected_permission_denied(
                PluginStateStoreOperation::ManifestRead,
            ));
        }
        if !is_manifest_name(name) {
            return Err(PluginStateStoreError::Failed(format!(
                "invalid manifest name {name:?}"
            )));
        }
        self.manifests
            .get(name)
            .cloned()
            .ok_or(PluginStateStoreError::NotFound)
    }

    fn put_blob(
        &mut self,
        digest: &ContentDigest,
        bytes: &[u8],
    ) -> Result<(), PluginStateStoreError> {
        self.check(PluginStateStoreOperation::BlobPublish)?;
        if let Some(applied) = self.blob_indeterminate {
            if applied {
                self.blobs.insert(digest_hex(digest), bytes.to_vec());
            }
            return Err(PluginStateStoreError::Indeterminate {
                operation: PluginStateStoreOperation::BlobPublish,
            });
        }
        self.blobs.insert(digest_hex(digest), bytes.to_vec());
        Ok(())
    }

    fn read_blob(&self, digest: &ContentDigest) -> Result<Vec<u8>, PluginStateStoreError> {
        self.calls
            .borrow_mut()
            .push(PluginStateStoreOperation::BlobRead);
        if self.denied.contains(&PluginStateStoreOperation::BlobRead) {
            return Err(injected_permission_denied(
                PluginStateStoreOperation::BlobRead,
            ));
        }
        self.blobs
            .get(&digest_hex(digest))
            .cloned()
            .ok_or(PluginStateStoreError::NotFound)
    }
}

fn captured_matrix_fixture(
    blob: &[u8],
    quality: StateCaptureQuality,
) -> (ProjectController, ProcessorId, MockPort) {
    let mut c = controller();
    let processor = add_processor(&mut c, "matrix", resolved("matrix"));
    let mut port = MockPort::default();
    port.bind_all(&[processor]);
    port.captures
        .insert(processor, Ok(MockPort::capture_ok(blob, quality)));
    (c, processor, port)
}

fn save_matrix_fixture(
    blob: &[u8],
) -> (
    ProjectController,
    ProcessorId,
    PluginStateSession<MockPort, MatrixStore>,
) {
    let (c, processor, port) = captured_matrix_fixture(blob, StateCaptureQuality::Coordinated);
    let mut session = PluginStateSession::new(port, MatrixStore::new());
    session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();
    (c, processor, session)
}

// ── Save: quality classes & policy ───────────────────────────────

#[test]
fn standard_save_records_all_quality_classes_and_publishes() {
    let mut c = controller();
    let coord = add_processor(&mut c, "coord", resolved("coord"));
    let quiet = add_processor(&mut c, "quiet", resolved("quiet"));
    let best = add_processor(&mut c, "best", resolved("best"));
    let unsup = add_processor(&mut c, "unsup", resolved("unsup"));

    let mut port = MockPort::default();
    port.bind_all(&[coord, quiet, best, unsup]);
    port.captures.insert(
        coord,
        Ok(MockPort::capture_ok(
            b"c-state",
            StateCaptureQuality::Coordinated,
        )),
    );
    port.captures.insert(
        quiet,
        Ok(MockPort::capture_ok(
            b"q-state",
            StateCaptureQuality::Quiescent,
        )),
    );
    port.captures.insert(
        best,
        Ok(MockPort::capture_ok(
            b"b-state",
            StateCaptureQuality::BestEffort,
        )),
    );
    port.captures
        .insert(unsup, Err(PluginStateCaptureError::Unsupported));

    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();

    assert_eq!(receipt.summary.processors, 4);
    assert_eq!(receipt.summary.exact, 2);
    assert_eq!(receipt.summary.degraded, 1);
    assert_eq!(receipt.summary.unsupported, 1);
    assert_eq!(receipt.summary.failed, 0);
    assert!(!receipt.summary.exact_eligible());
    assert!(!receipt.reconciled);
    assert_eq!(receipt.records.len(), 4);
    // Records carry binding + revision + stored_at evidence.
    for record in &receipt.records {
        assert!(record.binding.is_some());
        assert_eq!(record.project_revision, c.live_revision().0);
        assert_eq!(record.stored_at, "2026-11-09T00:00:00Z");
    }
    // HEAD points at the published manifest; blobs are content-addressed.
    let name = receipt.manifest_name.clone().unwrap();
    assert_eq!(session.store().head().unwrap(), Some(name.clone()));
    let manifest_bytes = session.store().read_manifest(&name).unwrap();
    let manifest: PluginStateManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest.schema_version, PLUGIN_STATE_SCHEMA_VERSION);
    assert_eq!(manifest.project_revision, c.live_revision().0);
    assert_eq!(
        receipt.manifest_digest,
        Some(ContentDigest::sha256(&manifest_bytes))
    );
    // Unsupported record has no blob reference.
    let unsup_record = receipt
        .records
        .iter()
        .find(|r| r.processor_id == unsup)
        .unwrap();
    assert!(matches!(
        unsup_record.outcome,
        PluginStateOutcome::Unsupported
    ));
}

#[test]
fn strict_save_rejects_degraded_and_publishes_nothing() {
    let mut c = controller();
    let coord = add_processor(&mut c, "coord", resolved("coord"));
    let best = add_processor(&mut c, "best", resolved("best"));

    let mut port = MockPort::default();
    port.bind_all(&[coord, best]);
    port.captures.insert(
        best,
        Ok(MockPort::capture_ok(b"b", StateCaptureQuality::BestEffort)),
    );

    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    let error = session
        .save(&save_request(&c, StateQualityPolicy::Strict))
        .unwrap_err();
    match error {
        PluginStateError::QualityRejected { summary } => {
            assert_eq!(summary.degraded, 1);
            assert!(!summary.exact_eligible());
        }
        other => panic!("expected QualityRejected, got {other}"),
    }
    // Nothing published: no HEAD, no manifests, no blobs.
    assert_eq!(session.store().head().unwrap(), None);
}

#[test]
fn quality_summary_requires_consistent_counts_and_expected_coverage() {
    let exact_two = PluginStateQualitySummary {
        processors: 2,
        exact: 2,
        degraded: 0,
        unsupported: 0,
        failed: 0,
    };
    assert!(exact_two.exact_eligible());
    assert!(exact_two.exact_eligible_for_processors(2));
    assert!(!exact_two.exact_eligible_for_processors(1));

    let missing_coverage = PluginStateQualitySummary {
        processors: 2,
        exact: 1,
        degraded: 0,
        unsupported: 0,
        failed: 0,
    };
    assert!(!missing_coverage.internally_consistent());
    assert!(!missing_coverage.exact_eligible());

    let vacuous = PluginStateQualitySummary {
        processors: 0,
        exact: 0,
        degraded: 0,
        unsupported: 0,
        failed: 0,
    };
    assert!(vacuous.exact_eligible());
    assert!(vacuous.exact_eligible_for_processors(0));
    assert!(!vacuous.exact_eligible_for_processors(1));
}

#[test]
fn strict_save_accepts_exact_quality() {
    let mut c = controller();
    let coord = add_processor(&mut c, "coord", resolved("coord"));
    let quiet = add_processor(&mut c, "quiet", resolved("quiet"));

    let mut port = MockPort::default();
    port.bind_all(&[coord, quiet]);

    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Strict))
        .unwrap();
    assert!(receipt.summary.exact_eligible());
    assert!(receipt.manifest_name.is_some());
}

#[test]
fn typed_failures_record_without_blobs() {
    let mut c = controller();
    let crash = add_processor(&mut c, "crash", resolved("crash"));
    let timeout = add_processor(&mut c, "timeout", resolved("timeout"));
    let denied = add_processor(&mut c, "denied", resolved("denied"));

    let mut port = MockPort::default();
    port.bind_all(&[crash, timeout, denied]);
    for (id, kind) in [
        (crash, PluginStateFailureKind::Crashed),
        (timeout, PluginStateFailureKind::Timeout),
        (denied, PluginStateFailureKind::PermissionDenied),
    ] {
        port.captures
            .insert(id, Err(PluginStateCaptureError::Failed(kind)));
    }

    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();
    assert_eq!(receipt.summary.failed, 3);
    for record in &receipt.records {
        let PluginStateOutcome::Failed { kind } = record.outcome else {
            panic!("expected Failed outcome");
        };
        assert!(
            [
                PluginStateFailureKind::Crashed,
                PluginStateFailureKind::Timeout,
                PluginStateFailureKind::PermissionDenied
            ]
            .contains(&kind)
        );
    }
    // Strict rejects the same evidence.
    let error = session
        .save(&save_request(&c, StateQualityPolicy::Strict))
        .unwrap_err();
    assert!(matches!(error, PluginStateError::QualityRejected { .. }));
}

#[test]
fn unresolved_binding_and_missing_instance_fail_unavailable_without_port_call() {
    let mut c = controller();
    add_processor(
        &mut c,
        "missing",
        ProcessorBinding::Unresolved {
            descriptor: "vendor.gone".into(),
        },
    );
    add_processor(&mut c, "noinst", resolved("noinst"));

    let port = MockPort::default();
    let calls = Rc::clone(&port.calls);
    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();
    assert_eq!(receipt.summary.failed, 2);
    for record in &receipt.records {
        assert!(matches!(
            record.outcome,
            PluginStateOutcome::Failed {
                kind: PluginStateFailureKind::Unavailable
            }
        ));
        // No live instance was described — no binding evidence.
        assert!(record.binding.is_none());
    }
    let calls = calls.borrow();
    // Neither processor reached the port's capture — there is no
    // live instance to ask (unresolved binding / unknown instance).
    assert!(calls.iter().all(|c| !c.starts_with("capture:")));
}

// ── Restore ──────────────────────────────────────────────────────

#[test]
fn restore_roundtrip_restores_captured_and_reports_rest() {
    let mut c = controller();
    let a = add_processor(&mut c, "a", resolved("a"));
    let b = add_processor(&mut c, "b", resolved("b"));
    let unsup = add_processor(&mut c, "u", resolved("u"));

    let mut port = MockPort::default();
    port.bind_all(&[a, b, unsup]);
    port.captures
        .insert(unsup, Err(PluginStateCaptureError::Unsupported));
    let restored = Rc::clone(&port.restored);

    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();
    let name = receipt.manifest_name.unwrap();

    // Mutate the "plugin" state, then restore from HEAD.
    let restore = session.restore(&restore_request(&c, None)).unwrap();
    assert_eq!(restore.manifest_name, name);
    assert_eq!(restore.records.len(), 3);
    for record in &restore.records {
        match record.processor_id {
            p if p == unsup => assert!(matches!(
                record.outcome,
                PluginStateRestoreOutcome::Unsupported
            )),
            _ => assert!(matches!(
                record.outcome,
                PluginStateRestoreOutcome::Restored
            )),
        }
    }
    // Both captured blobs reached the port verbatim.
    let restored = restored.borrow();
    assert_eq!(restored.len(), 2);
    assert!(restored.iter().all(|(_, b)| b == b"state"));
}

#[test]
fn restore_detects_blob_corruption_and_missing_blob() {
    let mut c = controller();
    let a = add_processor(&mut c, "a", resolved("a"));
    let b = add_processor(&mut c, "b", resolved("b"));

    let mut port = MockPort::default();
    port.bind_all(&[a, b]);
    port.captures.insert(
        a,
        Ok(MockPort::capture_ok(
            b"aaa",
            StateCaptureQuality::Coordinated,
        )),
    );
    port.captures.insert(
        b,
        Ok(MockPort::capture_ok(
            b"bbb",
            StateCaptureQuality::Coordinated,
        )),
    );

    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();

    // Tamper: overwrite a's blob bytes in the store's inner map via
    // a second store sharing nothing — instead corrupt through the
    // public surface: delete is impossible, so rebuild a store with
    // tampered bytes by hand using the manifest's digest.
    let manifest_name = session.store().head().unwrap().unwrap();
    let manifest_bytes = session.store().read_manifest(&manifest_name).unwrap();
    let manifest: PluginStateManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    let a_digest = manifest
        .entries
        .iter()
        .find(|e| e.processor_id == a)
        .map(|e| match e.outcome {
            PluginStateOutcome::Captured { blob_digest, .. } => blob_digest,
            _ => panic!(),
        })
        .unwrap();
    // b's blob intentionally absent → Missing (no digest lookup needed).

    // Corrupt store: same manifest, wrong blob bytes for a, absent for b.
    let mut corrupt = InMemoryPluginStateStore::new();
    corrupt.save_manifest(&manifest_bytes).unwrap();
    corrupt.put_blob(&a_digest, b"tampered").unwrap();
    // b's blob intentionally absent → Missing.

    // Swap the session's store: build a fresh session over the same port.
    let (port, _) = session.into_parts();
    let mut session2 = PluginStateSession::new(port, corrupt);
    let receipt = session2
        .restore(&PluginStateRestoreRequest {
            manifest: Some(manifest_name),
            project: c.live(),
        })
        .unwrap();
    let a_out = receipt
        .records
        .iter()
        .find(|r| r.processor_id == a)
        .unwrap();
    let b_out = receipt
        .records
        .iter()
        .find(|r| r.processor_id == b)
        .unwrap();
    assert!(matches!(
        a_out.outcome,
        PluginStateRestoreOutcome::Failed {
            kind: PluginStateFailureKind::Corrupt
        }
    ));
    assert!(matches!(
        b_out.outcome,
        PluginStateRestoreOutcome::Failed {
            kind: PluginStateFailureKind::Missing
        }
    ));
    // The port's restore log stays empty — corrupt/missing blobs are
    // rejected before the plugin sees any bytes.
    let (port, _) = session2.into_parts();
    assert!(port.restored.borrow().is_empty());
}

#[test]
fn restore_reports_missing_states_for_processors_absent_from_manifest() {
    let mut c = controller();
    let a = add_processor(&mut c, "a", resolved("a"));

    let mut port = MockPort::default();
    port.bind_all(&[a]);
    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();

    // Add a second processor after the save — it is absent from the
    // manifest and must be reported, not skipped.
    let b = add_processor(&mut c, "b", resolved("b"));
    let receipt = session.restore(&restore_request(&c, None)).unwrap();
    assert_eq!(receipt.missing_states, vec![b]);
}

#[test]
fn copy_restore_preserves_records_quality_and_digests() {
    let mut c = controller();
    let a = add_processor(&mut c, "a", resolved("a"));
    let best = add_processor(&mut c, "b", resolved("b"));

    let mut port = MockPort::default();
    port.bind_all(&[a, best]);
    port.captures.insert(
        best,
        Ok(MockPort::capture_ok(
            b"degraded",
            StateCaptureQuality::BestEffort,
        )),
    );
    let restored = Rc::clone(&port.restored);

    let mut session = PluginStateSession::new(port, InMemoryPluginStateStore::new());
    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();
    let name = receipt.manifest_name.clone().unwrap();

    // "Copy restore": clone the whole store (immutable bytes) into a
    // fresh session — records, quality and digests must be
    // identical (undo/history/copy-restore quality preservation).
    let store_copy = session.store().clone();
    let (port, _) = session.into_parts();
    let mut session2 = PluginStateSession::new(port, store_copy);
    let receipt2 = session2
        .restore(&PluginStateRestoreRequest {
            manifest: Some(name),
            project: c.live(),
        })
        .unwrap();
    assert_eq!(receipt2.manifest_digest, receipt.manifest_digest.unwrap());
    assert_eq!(receipt2.records.len(), 2);
    assert!(
        receipt2
            .records
            .iter()
            .all(|r| matches!(r.outcome, PluginStateRestoreOutcome::Restored))
    );
    assert_eq!(restored.borrow().len(), 2);
}

// ── Indeterminate publish reconciliation ─────────────────────────

#[test]
fn indeterminate_manifest_publish_reconciles_via_head_readback() {
    let mut c = controller();
    let a = add_processor(&mut c, "a", resolved("a"));

    let mut port = MockPort::default();
    port.bind_all(&[a]);
    // Inject before session construction: publish for real, then
    // report Indeterminate — the session must reconcile by readback.
    let mut store = FaultStore::new();
    store.manifest_indeterminate = Some(true);
    let mut session = PluginStateSession::new(port, store);

    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();
    assert!(receipt.reconciled);
    assert!(receipt.manifest_name.is_some());
    assert_eq!(
        session.store().head().unwrap(),
        receipt.manifest_name.clone()
    );
}

#[test]
fn indeterminate_manifest_publish_without_effect_stays_error() {
    let mut c = controller();
    let a = add_processor(&mut c, "a", resolved("a"));

    let mut port = MockPort::default();
    port.bind_all(&[a]);
    let mut store = FaultStore::new();
    store.manifest_indeterminate = Some(false); // report Indeterminate, publish nothing
    let mut session = PluginStateSession::new(port, store);

    let error = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap_err();
    assert!(matches!(
        error,
        PluginStateError::Store(PluginStateStoreError::Indeterminate { operation: _ })
    ));
    // HEAD was never repointed — previous state (none) intact.
    assert_eq!(session.store().head().unwrap(), None);
}

#[test]
fn indeterminate_blob_put_reconciles_and_unreconciled_aborts_before_manifest() {
    let mut c = controller();
    let a = add_processor(&mut c, "a", resolved("a"));

    // Case 1: blob put publishes then reports Indeterminate → readback
    // proves it → save proceeds.
    let mut port = MockPort::default();
    port.bind_all(&[a]);
    port.captures.insert(
        a,
        Ok(MockPort::capture_ok(
            b"blob",
            StateCaptureQuality::Coordinated,
        )),
    );
    let mut store = FaultStore::new();
    store.blob_indeterminate = Some(true);
    let mut session = PluginStateSession::new(port, store);
    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();
    assert!(!receipt.reconciled); // blob reconcile is silent; manifest was definite
    assert!(receipt.manifest_name.is_some());

    // Case 2: blob put reports Indeterminate without publishing →
    // readback fails → save aborts BEFORE manifest publish; HEAD
    // stays empty.
    let mut port2 = MockPort::default();
    port2.bind_all(&[a]);
    port2.captures.insert(
        a,
        Ok(MockPort::capture_ok(
            b"blob",
            StateCaptureQuality::Coordinated,
        )),
    );
    let mut store2 = FaultStore::new();
    store2.blob_indeterminate = Some(false);
    let mut session2 = PluginStateSession::new(port2, store2);
    let error = session2
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap_err();
    assert!(matches!(
        error,
        PluginStateError::Store(PluginStateStoreError::Indeterminate { operation: _ })
    ));
    assert_eq!(session2.store().head().unwrap(), None);
}

// ── Permission denial matrix ─────────────────────────────────────

#[test]
fn permission_denied_save_matrix_protects_previous_state() {
    for operation in [
        PluginStateStoreOperation::BlobPublish,
        PluginStateStoreOperation::ManifestPublish,
        PluginStateStoreOperation::HeadUpdate,
    ] {
        for policy in [StateQualityPolicy::Strict, StateQualityPolicy::Standard] {
            let (c, processor, session) = save_matrix_fixture(b"exact-state-v1");
            let (mut port, mut store) = session.into_parts();
            let before_head = store.head().unwrap();
            let before_manifests = store.manifest_set();
            let before_blobs = store.blob_set();
            port.captures.insert(
                processor,
                Ok(MockPort::capture_ok(
                    b"exact-state-v2",
                    StateCaptureQuality::Coordinated,
                )),
            );
            store.deny(operation);
            let mut session = PluginStateSession::new(port, store);

            let error = session.save(&save_request(&c, policy)).unwrap_err();
            let PluginStateError::Store(PluginStateStoreError::PermissionDenied {
                operation: actual,
                effect,
                ..
            }) = error
            else {
                panic!("expected typed permission denial at {operation}, got {error:?}");
            };
            assert_eq!(actual, operation);
            assert_eq!(effect, PluginStateStorePublishEffect::NotPublished);

            assert_eq!(session.store().head().unwrap(), before_head);
            assert_eq!(session.store().manifest_set(), before_manifests);
            // Content-addressed blobs are immutable: every pre-existing
            // blob must keep the same bytes under the same digest. A
            // denied later stage may leave an unreferenced new blob as
            // garbage, but it is not reachable from the unchanged HEAD
            // and is therefore not a valid saved state.
            let after_blobs = session.store().blob_set();
            for (digest, bytes) in &before_blobs {
                assert_eq!(after_blobs.get(digest), Some(bytes));
            }
            let empty_digest = digest_hex(&ContentDigest::sha256(b""));
            assert!(!after_blobs.contains_key(&empty_digest));
        }
    }
}

#[test]
fn strict_quality_gate_precedes_store_denial_while_standard_reaches_store() {
    let mut c = controller();
    let exact = add_processor(&mut c, "exact", resolved("exact"));
    let degraded = add_processor(&mut c, "degraded", resolved("degraded"));
    let unsupported = add_processor(&mut c, "unsupported", resolved("unsupported"));
    let failed = add_processor(&mut c, "failed", resolved("failed"));

    let mut port = MockPort::default();
    port.bind_all(&[exact, degraded, unsupported, failed]);
    port.captures.insert(
        degraded,
        Ok(MockPort::capture_ok(
            b"degraded",
            StateCaptureQuality::BestEffort,
        )),
    );
    port.captures
        .insert(unsupported, Err(PluginStateCaptureError::Unsupported));
    port.captures.insert(
        failed,
        Err(PluginStateCaptureError::Failed(
            PluginStateFailureKind::CaptureFailed,
        )),
    );

    let mut strict_store = MatrixStore::new();
    strict_store.deny(PluginStateStoreOperation::ManifestPublish);
    let strict_calls = Rc::clone(&strict_store.calls);
    let mut strict_session = PluginStateSession::new(port, strict_store);
    let strict_error = strict_session
        .save(&save_request(&c, StateQualityPolicy::Strict))
        .unwrap_err();
    assert!(matches!(
        strict_error,
        PluginStateError::QualityRejected { .. }
    ));
    assert!(strict_calls.borrow().is_empty());

    let (port, _) = strict_session.into_parts();
    let mut standard_store = MatrixStore::new();
    standard_store.deny(PluginStateStoreOperation::ManifestPublish);
    let standard_calls = Rc::clone(&standard_store.calls);
    let mut standard_session = PluginStateSession::new(port, standard_store);
    let standard_error = standard_session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap_err();
    assert!(matches!(
        standard_error,
        PluginStateError::Store(PluginStateStoreError::PermissionDenied {
            operation: PluginStateStoreOperation::ManifestPublish,
            ..
        })
    ));
    let calls = standard_calls.borrow();
    assert!(calls.contains(&PluginStateStoreOperation::BlobPublish));
    assert!(calls.contains(&PluginStateStoreOperation::ManifestPublish));
}

#[test]
fn permission_denied_restore_matrix_is_typed_and_fail_closed() {
    for operation in [
        PluginStateStoreOperation::HeadRead,
        PluginStateStoreOperation::ManifestRead,
    ] {
        let (c, _, session) = save_matrix_fixture(b"restore-state");
        let (_, mut store) = session.into_parts();
        store.deny(operation);
        let manifest = store.head().unwrap_or(None);
        let mut session = PluginStateSession::new(MockPort::default(), store);
        let request = PluginStateRestoreRequest {
            manifest: if operation == PluginStateStoreOperation::ManifestRead {
                manifest
            } else {
                None
            },
            project: c.live(),
        };
        let error = session.restore(&request).unwrap_err();
        assert!(matches!(
            error,
            PluginStateError::Store(PluginStateStoreError::PermissionDenied {
                operation: actual,
                effect: PluginStateStorePublishEffect::NotApplicable,
                ..
            }) if actual == operation
        ));
    }

    let (c, _processor, session) = save_matrix_fixture(b"restore-state");
    let (port, mut store) = session.into_parts();
    let restored = Rc::clone(&port.restored);
    store.deny(PluginStateStoreOperation::BlobRead);
    let mut session = PluginStateSession::new(port, store);
    let receipt = session.restore(&restore_request(&c, None)).unwrap();
    assert!(matches!(
        receipt.records[0].outcome,
        PluginStateRestoreOutcome::StoreFailed {
            operation: PluginStateStoreOperation::BlobRead,
            kind: PluginStateFailureKind::PermissionDenied,
        }
    ));
    assert!(restored.borrow().is_empty());

    // A plugin-port refusal is intentionally not the same outcome
    // shape as host/store-side blob read denial. This mock only
    // validates the session type boundary; it is not evidence that a
    // real ABI plugin can originate PermissionDenied.
    let (c, processor, session) = save_matrix_fixture(b"restore-state");
    let (mut port, store) = session.into_parts();
    port.restore_error.insert(
        processor,
        PluginStateCaptureError::Failed(PluginStateFailureKind::PermissionDenied),
    );
    let mut session = PluginStateSession::new(port, store);
    let receipt = session.restore(&restore_request(&c, None)).unwrap();
    assert!(matches!(
        receipt.records[0].outcome,
        PluginStateRestoreOutcome::Failed {
            kind: PluginStateFailureKind::PermissionDenied,
        }
    ));
}

#[test]
fn indeterminate_blob_publish_with_denied_readback_is_unreconciled_and_fail_closed() {
    let (c, processor, session) = save_matrix_fixture(b"old-exact-state");
    let (mut port, mut store) = session.into_parts();
    let before_head = store.head().unwrap();
    let before_manifests = store.manifest_set();
    let before_blobs = store.blob_set();
    port.captures.insert(
        processor,
        Ok(MockPort::capture_ok(
            b"new-exact-state",
            StateCaptureQuality::Coordinated,
        )),
    );
    store.blob_indeterminate = Some(true);
    store.deny(PluginStateStoreOperation::BlobRead);
    let mut session = PluginStateSession::new(port, store);

    let error = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap_err();
    let PluginStateError::ReadbackFailed {
        publish_error,
        readback_error,
    } = error
    else {
        panic!("expected readback failure, got {error:?}");
    };
    assert!(matches!(
        publish_error,
        PluginStateStoreError::Indeterminate {
            operation: PluginStateStoreOperation::BlobPublish,
        }
    ));
    assert!(matches!(
        readback_error,
        PluginStateStoreError::PermissionDenied {
            operation: PluginStateStoreOperation::BlobRead,
            ..
        }
    ));

    assert_eq!(session.store().head().unwrap(), before_head);
    assert_eq!(session.store().manifest_set(), before_manifests);
    let after_blobs = session.store().blob_set();
    assert!(after_blobs.len() > before_blobs.len());
    let new_digest = ContentDigest::sha256(b"new-exact-state");
    assert!(after_blobs.contains_key(&digest_hex(&new_digest)));
}

#[test]
fn indeterminate_head_update_with_denied_readback_never_becomes_success() {
    for readback_operation in [
        PluginStateStoreOperation::HeadRead,
        PluginStateStoreOperation::ManifestRead,
    ] {
        let (c, processor, session) = save_matrix_fixture(b"old-head-state");
        let (mut port, mut store) = session.into_parts();
        let before_head = store.head().unwrap();
        let before_manifests = store.manifest_set();
        port.captures.insert(
            processor,
            Ok(MockPort::capture_ok(
                b"new-head-state",
                StateCaptureQuality::Coordinated,
            )),
        );
        store.head_indeterminate = Some(true);
        store.deny(readback_operation);
        let mut session = PluginStateSession::new(port, store);

        let error = session
            .save(&save_request(&c, StateQualityPolicy::Standard))
            .unwrap_err();
        let PluginStateError::ReadbackFailed {
            publish_error,
            readback_error,
        } = error
        else {
            panic!("expected readback failure, got {error:?}");
        };
        assert!(matches!(
            publish_error,
            PluginStateStoreError::Indeterminate {
                operation: PluginStateStoreOperation::HeadUpdate,
            }
        ));
        assert!(matches!(
            readback_error,
            PluginStateStoreError::PermissionDenied {
                operation: actual,
                ..
            } if actual == readback_operation
        ));

        // The injected effect was applied, so HEAD/advanced manifest
        // objects may exist, but the save is neither a success nor a
        // proven non-publish. Evidence keeps it unreconciled.
        assert_ne!(session.store().head_name, before_head);
        assert!(session.store().manifest_set().len() > before_manifests.len());
    }
}

// ── Directory store ──────────────────────────────────────────────

#[test]
fn directory_store_roundtrip_and_head_file() {
    let root = std::env::temp_dir().join(format!("synvelis-tim04-ps-{}-dir", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);

    let mut c = controller();
    let a = add_processor(&mut c, "a", resolved("a"));
    let mut port = MockPort::default();
    port.bind_all(&[a]);
    port.captures.insert(
        a,
        Ok(MockPort::capture_ok(
            b"dir-state",
            StateCaptureQuality::Coordinated,
        )),
    );
    let restored = Rc::clone(&port.restored);

    let store = DirectoryPluginStateStore::create(&root).unwrap();
    let mut session = PluginStateSession::new(port, store);
    let receipt = session
        .save(&save_request(&c, StateQualityPolicy::Standard))
        .unwrap();
    let name = receipt.manifest_name.clone().unwrap();

    // HEAD file on disk names the manifest; manifest + blob exist.
    let head = std::fs::read_to_string(root.join("HEAD")).unwrap();
    assert_eq!(head.trim(), name);
    assert!(root.join("manifests").join(&name).exists());
    let manifest: PluginStateManifest =
        serde_json::from_slice(&std::fs::read(root.join("manifests").join(&name)).unwrap())
            .unwrap();
    let digest = match &manifest.entries[0].outcome {
        PluginStateOutcome::Captured { blob_digest, .. } => *blob_digest,
        _ => panic!(),
    };
    assert!(root.join("blobs").join(digest_hex(&digest)).exists());

    // Restore through the same directory store.
    session
        .restore(&PluginStateRestoreRequest {
            manifest: None,
            project: c.live(),
        })
        .unwrap();
    assert_eq!(restored.borrow().len(), 1);
    assert_eq!(restored.borrow()[0].1, b"dir-state");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn manifest_name_is_content_derived_and_validated() {
    let bytes = br#"{"schema_version":1}"#;
    let name = plugin_state_manifest_name(bytes);
    assert!(is_manifest_name(&name));
    assert!(name.starts_with("ps-"));
    assert_eq!(
        std::path::Path::new(&name)
            .extension()
            .and_then(|e| e.to_str()),
        Some("json")
    );
    assert!(!is_manifest_name("../evil"));
    assert!(!is_manifest_name("ps-short.json"));
    // Identical bytes → identical name (idempotent re-save).
    assert_eq!(name, plugin_state_manifest_name(bytes));
}
