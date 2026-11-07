//! ALIB-01 `ProjectAssetsProvider` tests: read-only projection of the
//! live `ProjectController`, catalog → blob resolution, typed
//! provenance, revision-bound cursors, immutable pinning.

mod common;

use std::fs;
use std::io::Read;
use std::sync::{Arc, Mutex};

use synvelis_library::*;
use synvelis_project::{
    Command, CounterIdGenerator, ImportId, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, SourceId,
};

use common::{TempTree, aref, browse_req, cx, cx_named, pid};

/// The blob-store file name convention: `<dir>/<digest>`.
const DIGEST_A: &str = "aaaa0000bbbb1111";
const DIGEST_B: &str = "cccc2222dddd3333";

fn blobs(tree: &TempTree) -> LocalBlobDir {
    LocalBlobDir::new(tree.join("blobs"))
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn register_asset(c: &mut ProjectController, digest: &str) -> synvelis_project::AssetId {
    c.apply(cmd(
        c,
        ProjectOp::RegisterAsset {
            digest: digest.into(),
        },
    ))
    .unwrap();
    *c.live().assets.keys().next_back().unwrap()
}

fn register_source(c: &mut ProjectController, asset: synvelis_project::AssetId, digest: &str) {
    c.apply(cmd(
        c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: digest.into(),
        },
    ))
    .unwrap();
}

type ProjectFixture = (
    TempTree,
    Arc<Mutex<ProjectController>>,
    ProjectAssetsProvider<Arc<Mutex<ProjectController>>, LocalBlobDir>,
);

/// Controller-backed provider with two assets + matching blobs.
fn fixture() -> ProjectFixture {
    let tree = TempTree::new("project");
    let shared = Arc::new(Mutex::new(controller()));
    {
        let mut c = shared.lock().unwrap();
        register_asset(&mut c, DIGEST_A);
        register_asset(&mut c, DIGEST_B);
        let a = *c.live().assets.keys().next().unwrap();
        register_source(&mut c, a, DIGEST_A);
    }
    tree.write(&format!("blobs/{DIGEST_A}"), b"alpha-blob-bytes");
    tree.write(&format!("blobs/{DIGEST_B}"), b"beta");
    let provider = ProjectAssetsProvider::new(pid(1), Arc::clone(&shared), blobs(&tree));
    (tree, shared, provider)
}

fn asset_locator(c: &Arc<Mutex<ProjectController>>, digest: &str) -> String {
    c.lock()
        .unwrap()
        .live()
        .assets
        .values()
        .find(|a| a.digest == digest)
        .unwrap()
        .id
        .to_tagged()
}

// ── browse / stat ────────────────────────────────────────────────

#[test]
fn browse_projects_every_asset_with_blob_facts() {
    let (_tree, _shared, provider) = fixture();
    let page = provider.browse(&browse_req(10), &cx()).unwrap();

    assert_eq!(page.coverage, Coverage::Complete);
    assert_eq!(page.entries.len(), 2);
    for entry in &page.entries {
        assert_eq!(entry.kind, EntryKind::Asset);
        assert!(!entry.via_symlink);
        assert_eq!(entry.availability, Availability::Available);
        // The locator is the tagged project AssetId — the stable
        // identity that never leaves the project namespace.
        assert!(entry.locator.as_str().starts_with("asset-"));
        assert_eq!(entry.facts.kind.value(), Some(&AssetKind::Blob));
        assert_eq!(
            entry
                .facts
                .checksum_hint
                .value()
                .map(ObjectIdentity::as_str),
            Some(entry.display_name.as_str())
        );
        assert_eq!(entry.facts.media.duration_ms.value(), None);
    }
    // Sorted by locator (tagged id) ascending; byte_len comes from the
    // blob store (16 vs 4 bytes → distinct values prove real stat).
    let sizes: Vec<_> = page
        .entries
        .iter()
        .map(|e| *e.facts.byte_len.value().unwrap())
        .collect();
    let mut expected = vec![16u64, 4];
    expected.sort_unstable();
    let mut sizes_sorted = sizes.clone();
    sizes_sorted.sort_unstable();
    assert_eq!(sizes_sorted, expected);
}

#[test]
fn stat_reports_provenance_and_blob_facts() {
    let (_tree, shared, provider) = fixture();
    let locator = asset_locator(&shared, DIGEST_A);
    let stat = provider.stat(&aref(pid(1), &locator), &cx()).unwrap();

    assert_eq!(stat.availability, Availability::Available);
    assert_eq!(stat.facts.byte_len.value(), Some(&16));
    assert_eq!(
        stat.facts.checksum_hint.value().map(ObjectIdentity::as_str),
        Some(DIGEST_A)
    );
    assert_eq!(stat.facts.kind.value(), Some(&AssetKind::Blob),);
    // Provenance: the registered source is recorded, not guessed.
    let source_id = shared
        .lock()
        .unwrap()
        .live()
        .sources
        .keys()
        .next()
        .unwrap()
        .to_tagged();
    assert!(
        stat.provenance
            .iter()
            .any(|p| p.kind == "source" && p.id == source_id && p.state.is_some())
    );
}

#[test]
fn imported_via_provenance_is_projected() {
    // Snapshot-shaped catalogs (headless fixtures) carry import
    // provenance through the same projection.
    let tree = TempTree::new("project-import");
    tree.write(&format!("blobs/{DIGEST_A}"), b"x");
    let asset_id = synvelis_project::AssetId::new(9);
    let import_id = ImportId::new(7);
    let snapshot = ProjectCatalogSnapshot {
        project_id: ProjectId::new(1),
        revision: ProjectRevision(4),
        assets: vec![ProjectAssetRecord {
            asset_id,
            digest: DIGEST_A.to_owned(),
            sources: vec![ProjectSourceRef {
                source_id: SourceId::new(5),
                state: synvelis_project::SourceState::Committed,
            }],
            imported_via: vec![import_id],
        }],
    };
    let provider = ProjectAssetsProvider::new(pid(1), snapshot, blobs(&tree));
    let stat = provider
        .stat(&aref(pid(1), &asset_id.to_tagged()), &cx())
        .unwrap();
    assert!(
        stat.provenance
            .iter()
            .any(|p| p.kind == "import" && p.id == import_id.to_tagged())
    );
    assert!(
        stat.provenance
            .iter()
            .any(|p| p.kind == "source" && p.state.as_deref() == Some("committed"))
    );
}

#[test]
fn missing_blob_is_typed_missing_not_error() {
    let (tree, shared, provider) = fixture();
    fs::remove_file(tree.join(format!("blobs/{DIGEST_B}"))).unwrap();
    let locator_b = asset_locator(&shared, DIGEST_B);

    // Browse still lists the catalog entry — the catalog record
    // exists; only its bytes are gone.
    let page = provider.browse(&browse_req(10), &cx()).unwrap();
    let entry = page
        .entries
        .iter()
        .find(|e| e.locator.as_str() == locator_b)
        .unwrap();
    assert!(matches!(entry.availability, Availability::Missing { .. }));

    // stat reports the same typed state, not an error.
    let stat = provider.stat(&aref(pid(1), &locator_b), &cx()).unwrap();
    assert!(matches!(stat.availability, Availability::Missing { .. }));

    // No bytes → no pin: the import path is blocked honestly.
    assert_eq!(
        provider
            .pin(&aref(pid(1), &locator_b), &cx())
            .unwrap_err()
            .kind(),
        ProviderErrorKind::NotFound
    );
}

// ── pin / open ───────────────────────────────────────────────────

#[test]
fn pin_yields_immutable_object_and_verified_stream() {
    let (_tree, shared, provider) = fixture();
    let locator = asset_locator(&shared, DIGEST_A);
    let pin = provider.pin(&aref(pid(1), &locator), &cx()).unwrap();

    assert!(matches!(pin.proof, PinProof::ImmutableObject { .. }));
    assert_eq!(pin.object_identity.as_str(), DIGEST_A);
    assert_eq!(pin.expected_size, Some(16));
    assert!(pin.digest_hint.is_some());

    let mut read = provider.open_pinned(&pin, &cx()).unwrap();
    assert_eq!(read.claims.object_identity.as_str(), DIGEST_A);
    assert!(matches!(
        read.claims.verification,
        VerificationPolicy::StreamVerified { .. }
    ));
    let mut bytes = Vec::new();
    read.reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"alpha-blob-bytes");
    let receipt = read.completion.take().unwrap().unwrap();
    assert!(receipt.verified);
    assert_eq!(receipt.bytes_read, 16);
}

#[test]
fn tampered_blob_fails_integrity_not_serves() {
    let (tree, shared, provider) = fixture();
    let locator = asset_locator(&shared, DIGEST_A);
    let pin = provider.pin(&aref(pid(1), &locator), &cx()).unwrap();
    // Same-length tamper: the digest_hint catches it at EOF.
    fs::write(tree.join(format!("blobs/{DIGEST_A}")), b"XXXX-XXXX-XXXXXX").unwrap();
    assert_eq!(
        fs::metadata(tree.join(format!("blobs/{DIGEST_A}")))
            .unwrap()
            .len(),
        16
    );
    let mut read = provider.open_pinned(&pin, &cx()).unwrap();
    let mut buf = Vec::new();
    let err = read.reader.read_to_end(&mut buf).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    let outcome = read.completion.take().unwrap().unwrap_err();
    assert_eq!(outcome.kind(), ProviderErrorKind::IntegrityMismatch);
}

#[test]
fn open_pinned_rejects_foreign_or_mismatched_identity() {
    let (_tree, shared, provider) = fixture();
    let locator_b = asset_locator(&shared, DIGEST_B);
    let pin_a = provider
        .pin(&aref(pid(1), &asset_locator(&shared, DIGEST_A)), &cx())
        .unwrap();
    // A pin naming asset A but carried on asset B's locator is
    // rejected — the object identity is bound to the asset.
    let crossed = PinnedAssetRef {
        asset: aref(pid(1), &locator_b),
        ..pin_a
    };
    // The catalog no longer maps that locator to the pinned object —
    // typed SourceChanged, not a blanket request error.
    assert_eq!(
        provider.open_pinned(&crossed, &cx()).unwrap_err().kind(),
        ProviderErrorKind::SourceChanged
    );
    // A ConditionalStrong proof is foreign to the CAS provider.
    let foreign = PinnedAssetRef {
        asset: aref(pid(1), &locator_b),
        proof: PinProof::ConditionalStrong {
            token: StrongRevisionToken::ContentHash {
                digest: ContentDigest::new(DigestAlgorithm::Sha256, &[1u8; 32]),
            },
        },
        object_identity: ObjectIdentity::new(DIGEST_B).unwrap(),
        expected_size: None,
        digest_hint: None,
    };
    assert_eq!(
        provider.open_pinned(&foreign, &cx()).unwrap_err().kind(),
        ProviderErrorKind::InvalidRequest
    );
}

// ── cursors / revision binding ───────────────────────────────────

#[test]
fn cursors_are_bound_to_project_revision() {
    let (_tree, shared, provider) = fixture();
    // Two assets, page size 1 → a cursor minted at revision R.
    let first = provider.browse(&browse_req(1), &cx()).unwrap();
    let cursor = first.next_cursor.unwrap();

    // Same revision → accepted.
    let mut req = browse_req(1);
    req.cursor = Some(cursor.clone());
    assert!(provider.browse(&req, &cx()).is_ok());

    // Apply any op → revision moves → the cursor is dead.
    {
        let mut c = shared.lock().unwrap();
        let command = cmd(
            &c,
            ProjectOp::RegisterAsset {
                digest: "ffff".into(),
            },
        );
        c.apply(command).unwrap();
    }
    let err = provider.browse(&req, &cx()).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);
}

#[test]
fn browse_scope_must_be_root() {
    let (_tree, _shared, provider) = fixture();
    let mut req = browse_req(10);
    req.scope = BrowseScope::Locator {
        locator: common::loc("whatever"),
    };
    assert_eq!(
        provider.browse(&req, &cx()).unwrap_err().kind(),
        ProviderErrorKind::InvalidRequest
    );
}

#[test]
fn search_is_typed_unsupported_not_empty() {
    let (_tree, _shared, provider) = fixture();
    let err = provider
        .search(
            &SearchQuery {
                text: "a".into(),
                scope: BrowseScope::Root,
                page_limit: 10,
                cursor: None,
            },
            &cx(),
        )
        .unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::UnsupportedCapability);
    assert!(!provider.capabilities(None).search.is_supported());
}

// ── ownership / safety ───────────────────────────────────────────

#[test]
fn foreign_provider_ref_is_rejected() {
    let (_tree, _shared, provider) = fixture();
    for err in [
        provider.stat(&aref(pid(9), "x"), &cx()).map(|_| ()),
        provider.pin(&aref(pid(9), "x"), &cx()).map(|_| ()),
    ] {
        assert_eq!(err.unwrap_err().kind(), ProviderErrorKind::InvalidRequest);
    }
}

#[test]
fn hostile_blob_identity_cannot_traverse() {
    // A project record whose digest contains traversal text must not
    // resolve to a path outside the blob dir.
    let tree = TempTree::new("project-evil");
    let snapshot = ProjectCatalogSnapshot {
        project_id: ProjectId::new(1),
        revision: ProjectRevision(0),
        assets: vec![ProjectAssetRecord {
            asset_id: synvelis_project::AssetId::new(1),
            digest: "../outside".to_owned(),
            sources: Vec::new(),
            imported_via: Vec::new(),
        }],
    };
    let provider = ProjectAssetsProvider::new(pid(1), snapshot, blobs(&tree));
    // Browse reports the entry but never reaches outside bytes.
    let page = provider.browse(&browse_req(10), &cx()).unwrap();
    assert!(!matches!(
        page.entries[0].availability,
        Availability::Available
    ));
    // stat stays typed; pin fails — there is no fetchable object.
    let err = provider
        .pin(
            &aref(pid(1), &synvelis_project::AssetId::new(1).to_tagged()),
            &cx(),
        )
        .unwrap_err();
    assert!(matches!(
        err.kind(),
        ProviderErrorKind::InvalidRequest | ProviderErrorKind::NotFound
    ));
}

#[test]
fn cancelled_context_fails() {
    let (_tree, _shared, provider) = fixture();
    let cx = cx();
    cx.cancel.cancel();
    assert_eq!(
        provider.browse(&browse_req(10), &cx).unwrap_err().kind(),
        ProviderErrorKind::Cancelled
    );
}

#[test]
fn capabilities_declare_search_gap_and_immutable_pin() {
    let (_tree, _shared, provider) = fixture();
    let caps = provider.capabilities(None);
    assert!(caps.browse.is_supported());
    assert!(caps.stat.is_supported());
    assert!(matches!(
        caps.search,
        Capability::Unsupported {
            reason: UnsupportedReason::RequiresIndex
        }
    ));
    assert_eq!(caps.pin, PinCapability::ImmutableObject);
    assert!(caps.fetch.is_supported());
    assert!(caps.checksum_hint.is_supported());
    assert_eq!(caps.access, AccessMode::ReadOnly);
}

#[test]
fn snapshot_catalog_supports_paging() {
    let tree = TempTree::new("project-snap");
    tree.write(&format!("blobs/{DIGEST_A}"), b"x");
    tree.write(&format!("blobs/{DIGEST_B}"), b"y");
    let snapshot = ProjectCatalogSnapshot {
        project_id: ProjectId::new(1),
        revision: ProjectRevision(2),
        assets: vec![
            ProjectAssetRecord {
                asset_id: synvelis_project::AssetId::new(1),
                digest: DIGEST_A.to_owned(),
                sources: Vec::new(),
                imported_via: Vec::new(),
            },
            ProjectAssetRecord {
                asset_id: synvelis_project::AssetId::new(2),
                digest: DIGEST_B.to_owned(),
                sources: Vec::new(),
                imported_via: Vec::new(),
            },
        ],
    };
    let provider = ProjectAssetsProvider::new(pid(1), snapshot, blobs(&tree));
    let first = provider.browse(&browse_req(1), &cx()).unwrap();
    assert_eq!(first.entries.len(), 1);
    // A live continuation cursor means the page is Partial, not Complete.
    assert_eq!(first.coverage, Coverage::Partial);
    let mut req = browse_req(1);
    req.cursor = first.next_cursor;
    let second = provider.browse(&req, &cx()).unwrap();
    assert_eq!(second.entries.len(), 1);
    assert!(second.next_cursor.is_none());
    assert_ne!(first.entries[0].locator, second.entries[0].locator);
    // The snapshot catalog is consistent — a frozen revision keeps
    // cursors valid, and the terminal page reports full coverage.
    assert_eq!(second.coverage, Coverage::Complete);
}

#[test]
fn cursor_dies_when_project_identity_changes() {
    // A cursor minted under one project must not replay against a
    // reloaded catalog, even at the same revision counter.
    let tree = TempTree::new("project-identity");
    tree.write(&format!("blobs/{DIGEST_A}"), b"x");
    tree.write(&format!("blobs/{DIGEST_B}"), b"y");
    let snapshot_at = |project_id| ProjectCatalogSnapshot {
        project_id,
        revision: ProjectRevision(2),
        assets: vec![
            ProjectAssetRecord {
                asset_id: synvelis_project::AssetId::new(1),
                digest: DIGEST_A.to_owned(),
                sources: Vec::new(),
                imported_via: Vec::new(),
            },
            ProjectAssetRecord {
                asset_id: synvelis_project::AssetId::new(2),
                digest: DIGEST_B.to_owned(),
                sources: Vec::new(),
                imported_via: Vec::new(),
            },
        ],
    };
    let provider_a =
        ProjectAssetsProvider::new(pid(1), snapshot_at(ProjectId::new(1)), blobs(&tree));
    let provider_b =
        ProjectAssetsProvider::new(pid(1), snapshot_at(ProjectId::new(2)), blobs(&tree));
    let cursor = provider_a
        .browse(&browse_req(1), &cx())
        .unwrap()
        .next_cursor
        .unwrap();
    let mut req = browse_req(1);
    req.cursor = Some(cursor);
    assert_eq!(
        provider_b.browse(&req, &cx()).unwrap_err().kind(),
        ProviderErrorKind::CursorInvalid
    );
}

#[test]
fn principal_change_invalidates_cursor() {
    let (_tree, _shared, provider) = fixture();
    let cursor = provider
        .browse(&browse_req(1), &cx())
        .unwrap()
        .next_cursor
        .unwrap();
    let mut req = browse_req(1);
    req.cursor = Some(cursor);
    let err = provider.browse(&req, &cx_named("other")).unwrap_err();
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);
}

// ── blob-store containment (V2) ──────────────────────────────────

#[test]
fn symlinked_blob_cannot_escape_the_store() {
    let tree = TempTree::new("project-links");
    tree.write("outside", b"secret");
    let link = tree.join(format!("blobs/{DIGEST_A}"));
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    if !common::try_symlink(&tree.join("outside"), &link) {
        return; // platform denies symlink creation — honestly skipped
    }
    let store = LocalBlobDir::new(tree.join("blobs"));
    let identity = ObjectIdentity::new(DIGEST_A).unwrap();
    for err in [
        store.open(&identity).map(|_| ()),
        store.stat(&identity).map(|_| ()),
    ] {
        assert_eq!(err.unwrap_err().kind(), ProviderErrorKind::PermissionDenied);
    }
}

#[test]
fn device_and_nonregular_blob_names_are_rejected() {
    let tree = TempTree::new("project-dev");
    // A directory entry named like a digest: joined path exists but is
    // not a regular file — typed rejection, never an opened handle.
    tree.mkdir(&format!("blobs/{DIGEST_A}"));
    let store = LocalBlobDir::new(tree.join("blobs"));
    let dir_id = ObjectIdentity::new(DIGEST_A).unwrap();
    assert_eq!(
        store.stat(&dir_id).unwrap_err().kind(),
        ProviderErrorKind::InvalidRequest
    );
    // DOS device basenames open real devices on Windows regardless of
    // the directory prefix — refused before any join resolves.
    for name in ["NUL", "con", "COM1", "lpt9"] {
        let device = ObjectIdentity::new(name).unwrap();
        assert_eq!(
            store.stat(&device).unwrap_err().kind(),
            ProviderErrorKind::InvalidRequest,
            "{name}"
        );
    }
}

// ── descending sort keeps unknowns last (V16) ────────────────────

#[test]
fn descending_size_sort_keeps_unknown_last() {
    let (tree, shared, provider) = fixture();
    // A missing blob has an unknown byte_len — it must sort last in
    // *both* directions, not float to the top under Desc.
    fs::remove_file(tree.join(format!("blobs/{DIGEST_B}"))).unwrap();
    let locator_b = asset_locator(&shared, DIGEST_B);
    let mut req = browse_req(10);
    req.sort = SortSpec {
        field: SortField::Size,
        direction: SortDirection::Desc,
    };
    let page = provider.browse(&req, &cx()).unwrap();
    let last = page.entries.last().unwrap();
    assert_eq!(last.locator.as_str(), locator_b);
    assert!(last.facts.byte_len.value().is_none());
}

// ── completion terminal state survives take (V23) ────────────────

#[test]
fn completion_stays_finished_after_take() {
    let (_tree, shared, provider) = fixture();
    let locator = asset_locator(&shared, DIGEST_A);
    let pin = provider.pin(&aref(pid(1), &locator), &cx()).unwrap();
    let mut read = provider.open_pinned(&pin, &cx()).unwrap();
    let mut sink = Vec::new();
    read.reader.read_to_end(&mut sink).unwrap();
    assert!(read.completion.is_finished());
    assert!(read.completion.take().is_some());
    // Consuming the outcome must not reset the terminal state.
    assert!(read.completion.is_finished());
}
