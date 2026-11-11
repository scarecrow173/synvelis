//! ALIB-02 residual slice 2 (synvelis#86 DoD-1): the *real* v5 snapshot
//! format end-to-end — bytes serialized by the actual v5 build (commit
//! `a401e5c`, the last `SCHEMA_VERSION = 5` checkout; generator source
//! and provenance in `evidence/issue-86-v5-migration/`).
//!
//! Unlike `alib02_digest.rs`, which derives a v5-shaped envelope from
//! current project JSON, the fixture here carries exactly what the v5
//! writer emitted: legacy `{"scope": ...}` control-owner tags, no
//! `routing`/`monitoring`/`plugin_instances`/`smf_documents` keys,
//! CRC-fold digests computed over a real blob payload, and an
//! L1-era applied-import receipt.
//!
//! Contract under test: decode → ordered `STEPS` migration → verbatim
//! digest preservation → new-schema save → reload, with the v5 source
//! bytes immutable, the read-only `SnapshotResolver` projection
//! deterministic, sibling blob/plugin-state files untouched, and
//! degradation classified through the existing vocabulary
//! (`ContentStatus`/`SnapshotValidation`/`SnapshotCompleteness`/
//! `SnapshotError`), never silently promoted or merged.

use std::fs;
use std::path::PathBuf;

use synvelis_project::project::{AssetDigest, DigestKind};
use synvelis_project::*;

/// The exact snapshot bytes the v5 writer produced at commit a401e5c.
const V5_SNAPSHOT: &[u8] = include_bytes!("fixtures/alib02-v5-legacy.json");
/// The real blob payload the fixture's CRC-fold digest addresses —
/// the forward+reverse crc32 fold of these bytes is [`CRC_DIGEST`],
/// the same function `synvelis-player`/`synvelis-recording` still
/// implement.
const V5_BLOB: &[u8] = include_bytes!("fixtures/alib02-v5-legacy.blob");
/// The forward+reverse CRC32 fold of [`V5_BLOB`] measured by the
/// v5-era producer (`synvelis_recording::crc32`).
const CRC_DIGEST: &str = "5fea10b959fe82a9";
const PROJECT_ID: &str = "project-00000000000000000000000000000086";
/// The revision recorded inside the fixture envelope.
const V5_REVISION: u64 = 22;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn store_with_v5_head() -> (InMemoryStore, String) {
    let mut store = InMemoryStore::new();
    let head = store
        .save(PROJECT_ID, ProjectRevision(V5_REVISION), V5_SNAPSHOT)
        .unwrap();
    assert_eq!(head, format!("r{V5_REVISION}.json"));
    (store, head)
}

/// A scripted `BlobProbe` keyed on digest strings.
struct Probe(std::collections::BTreeMap<String, BlobRead>);

impl BlobProbe for Probe {
    fn probe(&self, digest: &str) -> BlobRead {
        self.0.get(digest).cloned().unwrap_or(BlobRead::Unknown)
    }
}

fn saved_locator() -> SnapshotLocator {
    SnapshotLocator::Saved {
        project_id: ProjectId::new(0x86),
        head_ref: None,
    }
}

// ── real decode → migrate → digest → save → reload ───────────────

#[test]
fn real_v5_snapshot_decodes_migrates_and_reloads() {
    let (mut store, v5_head) = store_with_v5_head();

    let mut c = controller();
    c.load(&store).unwrap();
    let p = c.live();

    // Identity + integrity floor: the v5 body landed as current-schema
    // state, one implicit revision step so a save can never overwrite
    // the source snapshot.
    assert_eq!(p.project_id.to_tagged(), PROJECT_ID);
    assert_eq!(c.live_revision(), ProjectRevision(V5_REVISION + 1));

    // Legacy entities preserved verbatim — IDs were never rewritten.
    assert_eq!(p.tracks.len(), 1);
    assert_eq!(p.lanes.len(), 2);
    assert_eq!(p.clips.len(), 2);
    assert_eq!(p.notes.len(), 1);
    assert_eq!(p.midi_sources.len(), 1);
    assert_eq!(p.midi_sources.values().next().unwrap().notes.len(), 2);
    assert_eq!(p.clock_epochs.len(), 1);
    assert_eq!(p.recording.sessions.len(), 1);
    assert_eq!(p.recording.passes.len(), 1);
    assert_eq!(p.recording.streams.len(), 1);
    assert_eq!(p.recording.segments.len(), 1);
    assert_eq!(p.recording.takes.len(), 1);

    // Digest conversion rule (measured): the 16-hex CRC fold classifies
    // `Crc32Fold` — preserved verbatim on every carrier, never promoted
    // to a verified content address.
    let asset = p.assets.values().next().unwrap();
    assert_eq!(asset.digest, CRC_DIGEST);
    let source = p.sources.values().next().unwrap();
    assert_eq!(source.content_digest, CRC_DIGEST);
    let receipt = p.applied_imports.values().next().unwrap();
    assert_eq!(receipt.content_hash, CRC_DIGEST);
    for raw in [&asset.digest, &source.content_digest, &receipt.content_hash] {
        let digest = AssetDigest::parse(raw);
        assert_eq!(digest.kind(), DigestKind::Crc32Fold);
        assert!(!digest.is_content_addressed());
    }
    assert_eq!(receipt.asset_id, asset.id);
    assert_eq!(receipt.source_id, source.id);

    // v5→v6 routing projection ran: a track-owned strip joins the
    // master strip, linked by `Main` connections only — nothing v5
    // never had (no sends, no processors, no extra buses) is
    // fabricated.
    assert_eq!(p.routing.strips.len(), 2); // master + one track strip
    assert!(p.routing.sends.is_empty());
    assert!(p.routing.processors.is_empty());
    assert!(p.routing.buses.contains_key(&p.routing.master));
    let master_strip = p.routing.buses[&p.routing.master].strip;
    let track_strip = p
        .routing
        .strips
        .values()
        .find(|s| s.id != master_strip)
        .unwrap();
    assert_eq!(
        track_strip.owner,
        StripOwner::Track(*p.tracks.keys().next().unwrap())
    );
    assert!(!p.routing.connections.is_empty());
    for conn in p.routing.connections.values() {
        assert_eq!(conn.kind, ConnectionKind::Main);
    }

    // v6→v7 control projection: the two scope-tagged owners became
    // typed `ControlOwner`s with derived time scope (the v7→v8
    // bootstrap fader plus v9→v10 per-strip params account for the
    // rest of the target set — asserted by owner, not by count).
    let track = *p.tracks.keys().next().unwrap();
    let by_owner = |owner| {
        p.control
            .targets
            .values()
            .find(|t| t.owner == owner)
            .unwrap()
    };
    let t_track = by_owner(ControlOwner::Track(track));
    assert_eq!(t_track.time_scope, AutomationTimeScope::ProjectMusical);
    assert_eq!(t_track.role, ParameterRole::Generic);
    let clip_owner = p
        .control
        .targets
        .values()
        .find_map(|t| match t.owner {
            ControlOwner::Clip(c) => Some(c),
            _ => None,
        })
        .unwrap();
    assert!(p.clips.contains_key(&clip_owner));
    let t_clip = by_owner(ControlOwner::Clip(clip_owner));
    assert_eq!(t_clip.time_scope, AutomationTimeScope::ClipLocal);
    assert!(
        p.control
            .targets
            .contains_key(&ParameterTargetId::new(MASTER_FADER_RAW))
    );
    // Modulation and automation payloads preserved verbatim.
    assert_eq!(p.control.lanes.len(), 1);
    assert_eq!(p.control.sources.len(), 1);
    assert_eq!(p.control.assignments.len(), 1);

    // Post-v5 stores default honestly — never fabricated content.
    assert!(p.smf_documents.is_empty());
    assert!(p.monitoring.tracks.is_empty());
    assert!(p.plugin_instances.instances.is_empty());

    // Save publishes a *new* snapshot; the v5 bytes stay byte-identical
    // and reloadable (read-only original preservation).
    let v5_before = store.read_snapshot(&v5_head).unwrap();
    let saved = c.save(&mut store).unwrap();
    assert_ne!(saved.head, v5_head);
    assert_eq!(store.read_snapshot(&v5_head).unwrap(), v5_before);
    assert_eq!(store.read_snapshot(&v5_head).unwrap(), V5_SNAPSHOT);

    let mut reloaded = controller();
    reloaded.load(&store).unwrap();
    assert_eq!(reloaded.live(), c.live());
    assert_eq!(reloaded.live_revision(), c.live_revision());
}

// ── deterministic read-only projection ────────────────────────────

#[test]
fn real_v5_read_only_projection_is_deterministic() {
    let (store, _) = store_with_v5_head();
    let resolver = SnapshotResolver::new();
    let cancel = SnapshotCancel::new();
    let blobs = Probe(
        [(CRC_DIGEST.to_owned(), BlobRead::Present(V5_BLOB.to_vec()))]
            .into_iter()
            .collect(),
    );

    let first = resolver
        .resolve_saved(&store, &saved_locator(), &cancel, Some(&blobs))
        .unwrap();
    let second = resolver
        .resolve_saved(&store, &saved_locator(), &cancel, Some(&blobs))
        .unwrap();

    // The projection records both versions honestly: source stayed v5,
    // the projection conforms to the current schema.
    assert_eq!(first.identity.schema_version, 5);
    assert_eq!(first.identity.projection_version, SCHEMA_VERSION);
    assert_eq!(first.origin, SnapshotOrigin::Saved);

    // Determinism: the same v5 bytes project to the identical project —
    // migration-issued routing/control IDs, semantic span/PCM-domain
    // payloads, watermark and validation verdict all compare equal.
    // (`capture_generation` is intentionally fresh per call and is the
    // only field allowed to differ.)
    assert_eq!(first.projection.project(), second.projection.project());
    assert_eq!(first.assets, second.assets);
    assert_eq!(first.validation, second.validation);
    assert_eq!(first.validation, SnapshotValidation::Valid);
    assert_eq!(first.completeness, SnapshotCompleteness::Complete);
    assert_eq!(first.identity.digest, second.identity.digest);

    // Spot-check the semantic observations: the projected strip and
    // connection IDs and the clip's source window are byte-stable.
    let p1 = first.projection.project();
    let p2 = second.projection.project();
    assert_eq!(
        p1.routing.strips.keys().collect::<Vec<_>>(),
        p2.routing.strips.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        p1.routing.connections.keys().collect::<Vec<_>>(),
        p2.routing.connections.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        p1.clips
            .values()
            .map(|c| c.playback.source_region)
            .collect::<Vec<_>>(),
        p2.clips
            .values()
            .map(|c| c.playback.source_region)
            .collect::<Vec<_>>()
    );
}

// ── original preservation: bundle bytes, blobs, plugin opaque ─────

struct BundleGuard(PathBuf);

impl Drop for BundleGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workdir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "synvelis-v5mig-{tag}-{}-{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ))
}

#[test]
fn migrated_save_preserves_original_and_sibling_blobs() {
    let dir = workdir("bundle");
    let _guard = BundleGuard(dir.clone());
    fs::create_dir_all(dir.join("snapshots")).unwrap();
    fs::write(dir.join("snapshots/r22.json"), V5_SNAPSHOT).unwrap();
    fs::write(dir.join("HEAD"), "r22.json").unwrap();

    // Sibling payload files a v5-era project dir legitimately carries:
    // the flat legacy blob the CRC digest addresses, a coexisting
    // sha256-namespaced blob, and opaque plugin-state bytes owned by a
    // different subsystem.
    let flat_blob = dir.join("blobs").join(CRC_DIGEST);
    let sha_blob = dir.join("blobs").join("sha256").join("ab".repeat(32));
    let plugin_blob = dir.join("plugin-state").join("blobs").join("deadbeef");
    fs::create_dir_all(sha_blob.parent().unwrap()).unwrap();
    fs::create_dir_all(plugin_blob.parent().unwrap()).unwrap();
    fs::write(&flat_blob, V5_BLOB).unwrap();
    fs::write(&sha_blob, b"sha256-namespaced blob").unwrap();
    fs::write(&plugin_blob, b"\x00\x01opaque-plugin-state\xff").unwrap();
    let flat_before = fs::read(&flat_blob).unwrap();
    let sha_before = fs::read(&sha_blob).unwrap();
    let plugin_before = fs::read(&plugin_blob).unwrap();

    // Load the v5 snapshot through the versioned reader and save the
    // migrated project back into the same bundle.
    let mut store = DirectoryBundleStore::create(&dir).unwrap();
    let mut c = controller();
    c.load(&store).unwrap();
    let saved = c.save(&mut store).unwrap();
    assert_ne!(saved.head, "r22.json");
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), saved.head);

    // The v5 original is untouched; every sibling payload survived
    // byte-for-byte — migration is a read-only projection plus a new
    // immutable snapshot, never a rewrite of existing content.
    assert_eq!(
        fs::read(dir.join("snapshots/r22.json")).unwrap(),
        V5_SNAPSHOT
    );
    assert_eq!(fs::read(&flat_blob).unwrap(), flat_before);
    assert_eq!(fs::read(&sha_blob).unwrap(), sha_before);
    assert_eq!(fs::read(&plugin_blob).unwrap(), plugin_before);

    // The new snapshot round-trips: HEAD load returns the migrated
    // project at the current schema.
    let mut reloaded = controller();
    reloaded.load(&store).unwrap();
    assert_eq!(reloaded.live(), c.live());
}

// ── degradation classification (existing vocabulary, wired) ───────

#[test]
fn missing_blob_classifies_degraded_not_rejected() {
    let (store, _) = store_with_v5_head();
    let resolver = SnapshotResolver::new();
    let cancel = SnapshotCancel::new();
    let blobs = Probe(
        [(CRC_DIGEST.to_owned(), BlobRead::Absent)]
            .into_iter()
            .collect(),
    );

    let snap = resolver
        .resolve_saved(&store, &saved_locator(), &cancel, Some(&blobs))
        .unwrap();
    // The snapshot still resolves — degradation is recorded, never a
    // silent `Present` claim and never an error for absent bytes.
    assert_eq!(snap.validation, SnapshotValidation::Degraded);
    assert_eq!(snap.completeness, SnapshotCompleteness::Partial);
    assert_eq!(snap.assets.entries.len(), 1);
    assert_eq!(snap.assets.entries[0].status, ContentStatus::Missing);
    assert_eq!(snap.assets.entries[0].digest, CRC_DIGEST);
}

#[test]
fn unprobed_blob_never_reports_present() {
    let (store, _) = store_with_v5_head();
    let resolver = SnapshotResolver::new();
    let cancel = SnapshotCancel::new();

    let snap = resolver
        .resolve_saved(&store, &saved_locator(), &cancel, None)
        .unwrap();
    // No probe ⇒ `Unknown`, still honest degradation — never a
    // fabricated `Present`.
    assert_eq!(snap.assets.entries[0].status, ContentStatus::Unknown);
    assert_eq!(snap.validation, SnapshotValidation::Degraded);
}

#[test]
fn unknown_schema_is_rejected_not_degraded() {
    // schema_version outside the ladder (4 = below the floor, 99 =
    // above current) is an explicit reject through both readers —
    // never a degraded read.
    for version in [4u32, 99] {
        let mut env: serde_json::Value = serde_json::from_slice(V5_SNAPSHOT).unwrap();
        env["schema_version"] = serde_json::json!(version);
        let bytes = serde_json::to_vec(&env).unwrap();
        let mut store = InMemoryStore::new();
        let head = store
            .save(PROJECT_ID, ProjectRevision(90 + u64::from(version)), &bytes)
            .unwrap();
        let mut c = controller();
        assert!(matches!(
            c.load(&store),
            Err(Error::UnsupportedSchema { found }) if found == version
        ));

        let resolver = SnapshotResolver::new();
        match resolver.resolve_saved(&store, &saved_locator(), &SnapshotCancel::new(), None) {
            Err(SnapshotError::UnsupportedSchema { found }) => assert_eq!(found, version),
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
        // The rejected snapshot stays untouched in the store.
        assert_eq!(store.read_snapshot(&head).unwrap(), bytes);
    }
}

#[test]
fn corrupt_or_foreign_state_is_rejected() {
    // Truncated bytes → parse-layer reject through the controller.
    let truncated = &V5_SNAPSHOT[..V5_SNAPSHOT.len() / 2];
    let mut store = InMemoryStore::new();
    store
        .save(PROJECT_ID, ProjectRevision(30), truncated)
        .unwrap();
    let mut c = controller();
    assert!(matches!(c.load(&store), Err(Error::Deserialize(_))));

    // A v5 body carrying post-v5 routing state is foreign content the
    // projection cannot own — explicit validation reject, source bytes
    // untouched. Build a valid *but non-default* `RoutingModel` by
    // migrating a clean copy of the same fixture.
    let (clean_store, _) = store_with_v5_head();
    let mut clean = controller();
    clean.load(&clean_store).unwrap();
    let foreign_routing = serde_json::to_value(&clean.live().routing).unwrap();

    let mut env: serde_json::Value = serde_json::from_slice(V5_SNAPSHOT).unwrap();
    env["project"]["routing"] = foreign_routing;
    let bytes = serde_json::to_vec(&env).unwrap();
    let mut store2 = InMemoryStore::new();
    store2
        .save(PROJECT_ID, ProjectRevision(31), &bytes)
        .unwrap();
    assert!(matches!(c.load(&store2), Err(Error::Validation(_))));
    assert_eq!(store2.read_snapshot("r31.json").unwrap(), bytes);
}

#[test]
fn crc_collision_assets_stay_separate_and_weak() {
    // Variant of the real v5 body with a second asset sharing the same
    // CRC-fold digest — the "old hash matches but bytes differ"
    // hazard. Both assets survive as distinct identities; neither is
    // promoted to a verified content address.
    let mut env: serde_json::Value = serde_json::from_slice(V5_SNAPSHOT).unwrap();
    let second = AssetId::new(0x6100).to_tagged();
    env["project"]["assets"].as_object_mut().unwrap().insert(
        second.clone(),
        serde_json::json!({"id": second, "digest": CRC_DIGEST}),
    );
    let mut store = InMemoryStore::new();
    store
        .save(
            PROJECT_ID,
            ProjectRevision(40),
            &serde_json::to_vec(&env).unwrap(),
        )
        .unwrap();

    let mut c = controller();
    c.load(&store).unwrap();
    assert_eq!(c.live().assets.len(), 2);
    for asset in c.live().assets.values() {
        assert_eq!(asset.digest, CRC_DIGEST);
        let digest = AssetDigest::parse(&asset.digest);
        assert_eq!(digest.kind(), DigestKind::Crc32Fold);
        assert!(!digest.is_content_addressed());
    }
}
