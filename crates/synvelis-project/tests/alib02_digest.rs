//! ALIB-02 residual slice (synvelis#86): the v10→v11 asset-digest
//! migration step, typed digest classification, and applied-import
//! referential integrity.
//!
//! The contract under test: persisted digest strings
//! (`Asset.digest`, `Source.content_digest`,
//! `AppliedImport.content_hash`) are preserved **verbatim** — legacy
//! CRC-fold and opaque forms are never rewritten or promoted to
//! SHA-256 without byte-level verification — while the integrity
//! floor (non-empty digests, receipts naming live entities) is
//! enforced at every load and commit.

use synvelis_project::project::{AssetDigest, DigestKind};
use synvelis_project::*;

/// A legacy forward+reverse CRC32-fold digest — 16 lowercase hex,
/// the form `synvelis-player`/`synvelis-recording` wrote before the
/// SHA-256 namespace existed.
const CRC_DIGEST: &str = "0123456789abcdef";

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

/// A v5-shaped envelope for `c`: current project JSON minus the
/// post-v5 state (`routing`, bootstrap/strip-owned control targets)
/// that the migration ladder re-derives — the same shaping
/// `routing_sf01.rs` uses for its v5 fixtures.
fn as_v5_envelope(c: &ProjectController) -> serde_json::Value {
    let mut env = serde_json::json!({
        "schema_version": 5,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    });
    env["project"].as_object_mut().unwrap().remove("routing");
    let targets = env["project"]["control"]["targets"]
        .as_object_mut()
        .unwrap();
    targets.remove(&ParameterTargetId::new(MASTER_FADER_RAW).to_tagged());
    let strip_owned: Vec<String> = targets
        .iter()
        .filter(|(_, t)| t["owner"]["owner"] == "Strip")
        .map(|(k, _)| k.clone())
        .collect();
    for key in &strip_owned {
        targets.remove(key);
    }
    if !strip_owned.is_empty() {
        let lanes = env["project"]["control"]["lanes"].as_object_mut().unwrap();
        lanes.retain(|_, l| {
            l["target"]
                .as_str()
                .is_none_or(|t| !strip_owned.iter().any(|k| k == t))
        });
        let assignments = env["project"]["control"]["assignments"]
            .as_object_mut()
            .unwrap();
        assignments.retain(|_, a| {
            [a["target"].as_str(), a["depth_source"].as_str()]
                .into_iter()
                .flatten()
                .all(|t| !strip_owned.iter().any(|k| k == t))
        });
    }
    env
}

/// Registers one asset+source pair carrying `digest` and returns the
/// issued IDs.
fn register_media(c: &mut ProjectController, digest: &str) -> (AssetId, SourceId) {
    c.apply(cmd(
        c,
        ProjectOp::RegisterAsset {
            digest: digest.into(),
        },
    ))
    .unwrap();
    let asset = *c.live().assets.keys().next_back().unwrap();
    c.apply(cmd(
        c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: digest.into(),
        },
    ))
    .unwrap();
    let source = *c.live().sources.keys().next_back().unwrap();
    (asset, source)
}

/// Injects an `applied_imports` receipt into a serialized envelope
/// and lifts `id_watermark` past the fixture IDs so the load-time
/// watermark oracle stays authoritative.
fn inject_receipt(env: &mut serde_json::Value, import_id: ImportId, receipt: &AppliedImport) {
    env["project"]["applied_imports"]
        .as_object_mut()
        .unwrap()
        .insert(
            import_id.to_tagged(),
            serde_json::to_value(receipt).unwrap(),
        );
    env["project"]["id_watermark"] = serde_json::json!(0x1000u64);
}

// ── typed digest classification ──────────────────────────────────

#[test]
fn digest_kind_classifies_persisted_forms() {
    let sha = format!("sha256:{}", "ab".repeat(32));
    let strong = AssetDigest::parse(&sha);
    assert_eq!(strong.kind(), DigestKind::Sha256);
    assert!(strong.is_content_addressed());
    assert_eq!(strong.as_str(), sha);
    // Uppercase hex is the same strong form — the blob layer's
    // namespace mapping lowercases it, so classification must agree.
    let upper = format!("sha256:{}", "AB".repeat(32));
    assert_eq!(AssetDigest::parse(&upper).kind(), DigestKind::Sha256);

    let crc = AssetDigest::parse(CRC_DIGEST);
    assert_eq!(crc.kind(), DigestKind::Crc32Fold);
    assert!(!crc.is_content_addressed());

    // Near-misses and producer-specific forms stay opaque — never
    // promoted to a verified content address.
    let short_sha = format!("sha256:{}", "ab".repeat(31));
    let truncated_sha = &sha[..sha.len() - 1];
    for raw in [
        "",
        "d",
        "deadbeef",
        "0123456789abcde",   // 15 hex — too short for the CRC fold
        "0123456789abcdef0", // 17 hex
        "0123456789ABCDEF",  // uppercase is not the legacy fold form
        "sha256:",
        "sha256:short",
        short_sha.as_str(),
        truncated_sha,
        "SHA256:0000000000000000000000000000000000000000000000000000000000000000",
        "crc32:0123456789abcdef",
    ] {
        assert_eq!(
            AssetDigest::parse(raw).kind(),
            DigestKind::Opaque,
            "{raw:?} must classify Opaque"
        );
        assert!(!AssetDigest::parse(raw).is_content_addressed());
    }

    // Serde is the verbatim string — the persisted form never changes.
    assert_eq!(
        serde_json::to_string(&crc).unwrap(),
        format!("\"{CRC_DIGEST}\"")
    );
    assert_eq!(
        serde_json::from_str::<AssetDigest>(&format!("\"{CRC_DIGEST}\"")).unwrap(),
        crc
    );
}

// ── v5 → v11 migration ───────────────────────────────────────────

#[test]
fn v5_snapshot_with_crc_digests_migrates_losslessly() {
    let mut c = controller();
    let (asset, source) = register_media(&mut c, CRC_DIGEST);
    let import_id = ImportId::new(0x00e1);
    let mut store = InMemoryStore::new();
    let saved_rev = c.live_revision();

    // A v5 body carrying an L1-era applied-import receipt whose
    // content_hash is the legacy CRC digest — exactly what
    // `synvelis-player`'s verified import wrote at schema v5.
    let mut env = as_v5_envelope(&c);
    inject_receipt(
        &mut env,
        import_id,
        &AppliedImport {
            content_hash: CRC_DIGEST.into(),
            asset_id: asset,
            source_id: source,
        },
    );
    let v5_head = store
        .save("p", saved_rev, &serde_json::to_vec(&env).unwrap())
        .unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    let p = c2.live();
    // Lossless: every digest string and the receipt survive verbatim.
    assert_eq!(p.assets[&asset].digest, CRC_DIGEST);
    assert_eq!(p.sources[&source].content_digest, CRC_DIGEST);
    assert_eq!(p.applied_imports[&import_id].content_hash, CRC_DIGEST);
    assert_eq!(p.applied_imports[&import_id].asset_id, asset);
    assert_eq!(p.applied_imports[&import_id].source_id, source);
    // Typed: the legacy form is classified weak — never a verified
    // content address.
    assert_eq!(
        AssetDigest::parse(&p.assets[&asset].digest).kind(),
        DigestKind::Crc32Fold
    );
    // The migrated load is one implicit revision step so the next
    // save publishes a new snapshot and never overwrites the v5 body.
    assert_eq!(c2.live_revision(), ProjectRevision(saved_rev.0 + 1));

    // Deterministic: the same v5 body always projects identically.
    let mut c3 = controller();
    c3.load(&store).unwrap();
    assert_eq!(c2.live(), c3.live());

    // The v5 original stays immutable; the migrated project saves as
    // the current schema and reloads to the same state.
    let v5_bytes = store.read_snapshot(&v5_head).unwrap();
    let v5_env: serde_json::Value = serde_json::from_slice(&v5_bytes).unwrap();
    assert_eq!(v5_env["schema_version"], serde_json::json!(5));
    c2.save(&mut store).unwrap();
    let mut c4 = controller();
    c4.load(&store).unwrap();
    assert_eq!(c4.live(), c2.live());
}

#[test]
fn crc_digest_collision_never_becomes_content_addressed() {
    // Two assets sharing one legacy CRC digest — the "old CRC matches
    // but bytes differ" hazard. The weak digest must not merge the
    // assets nor promote either to a verified content address.
    let mut c = controller();
    register_media(&mut c, CRC_DIGEST);
    c.apply(cmd(
        &c,
        ProjectOp::RegisterAsset {
            digest: CRC_DIGEST.into(),
        },
    ))
    .unwrap();
    assert_eq!(c.live().assets.len(), 2);
    let ids: Vec<AssetId> = c.live().assets.keys().copied().collect();
    assert_ne!(ids[0], ids[1]);
    for asset in c.live().assets.values() {
        let digest = AssetDigest::parse(&asset.digest);
        assert_eq!(digest.kind(), DigestKind::Crc32Fold);
        assert!(!digest.is_content_addressed());
    }
}

// ── integrity floor: load and commit both enforce ────────────────

#[test]
fn empty_digest_is_rejected_at_commit_and_load() {
    let mut c = controller();
    // The writer-side oracle catches it inside apply — the op
    // validates, mutates a clone, then runs validate_references.
    let rev = c.live_revision();
    assert!(matches!(
        c.apply(cmd(
            &c,
            ProjectOp::RegisterAsset {
                digest: String::new()
            }
        )),
        Err(Error::Validation(_))
    ));
    assert_eq!(c.live_revision(), rev);
    assert!(c.live().assets.is_empty());

    // A crafted snapshot carrying an empty digest is equally foreign.
    let mut c2 = controller();
    register_media(&mut c2, CRC_DIGEST);
    let mut store = InMemoryStore::new();
    c2.save(&mut store).unwrap();
    let head = store.head().unwrap().unwrap();
    let mut env: serde_json::Value =
        serde_json::from_slice(&store.read_snapshot(&head).unwrap()).unwrap();
    let assets = env["project"]["assets"].as_object_mut().unwrap();
    let asset_key = assets.keys().next().unwrap().clone();
    assets
        .get_mut(asset_key.as_str())
        .unwrap()
        .as_object_mut()
        .unwrap()["digest"] = serde_json::json!("");
    store
        .save("p", ProjectRevision(9), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut c3 = controller();
    assert!(matches!(c3.load(&store), Err(Error::Validation(_))));
    assert_eq!(c3.live_revision(), ProjectRevision(0));
}

#[test]
fn applied_import_receipt_must_name_live_entities() {
    let mut c = controller();
    let (asset, source) = register_media(&mut c, CRC_DIGEST);
    let import_id = ImportId::new(0x00e2);
    let receipt = AppliedImport {
        content_hash: CRC_DIGEST.into(),
        asset_id: asset,
        source_id: source,
    };
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();

    // Baseline: a coherent receipt loads.
    let head = store.head().unwrap().unwrap();
    let mut env: serde_json::Value =
        serde_json::from_slice(&store.read_snapshot(&head).unwrap()).unwrap();
    inject_receipt(&mut env, import_id, &receipt);
    store
        .save("p", ProjectRevision(7), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut ok = controller();
    ok.load(&store).unwrap();
    assert!(ok.live().applied_imports.contains_key(&import_id));

    // A receipt naming a missing asset is foreign state — rejected at
    // load, live state untouched.
    let mut bad = env.clone();
    let ikey = import_id.to_tagged();
    bad["project"]["applied_imports"]
        .as_object_mut()
        .unwrap()
        .get_mut(ikey.as_str())
        .unwrap()
        .as_object_mut()
        .unwrap()["asset_id"] = serde_json::json!(AssetId::new(0xffff).to_tagged());
    store
        .save("p", ProjectRevision(8), &serde_json::to_vec(&bad).unwrap())
        .unwrap();
    let mut c2 = controller();
    assert!(matches!(c2.load(&store), Err(Error::NotFound(_))));
    assert_eq!(c2.live_revision(), ProjectRevision(0));

    // A receipt naming a missing source is equally foreign.
    let mut bad2 = env.clone();
    bad2["project"]["applied_imports"]
        .as_object_mut()
        .unwrap()
        .get_mut(ikey.as_str())
        .unwrap()
        .as_object_mut()
        .unwrap()["source_id"] = serde_json::json!(SourceId::new(0xffff).to_tagged());
    store
        .save("p", ProjectRevision(9), &serde_json::to_vec(&bad2).unwrap())
        .unwrap();
    assert!(matches!(c2.load(&store), Err(Error::NotFound(_))));

    // An empty content hash carries no identity — rejected.
    let mut bad3 = env.clone();
    bad3["project"]["applied_imports"]
        .as_object_mut()
        .unwrap()
        .get_mut(ikey.as_str())
        .unwrap()
        .as_object_mut()
        .unwrap()["content_hash"] = serde_json::json!("");
    store
        .save(
            "p",
            ProjectRevision(10),
            &serde_json::to_vec(&bad3).unwrap(),
        )
        .unwrap();
    assert!(matches!(c2.load(&store), Err(Error::Validation(_))));
}
