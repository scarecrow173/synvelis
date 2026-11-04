//! Headless `check_bundle` acceptance/rejection tests (SV-GUI-01).
//!
//! Every [`AssetErrorKind`] must be reachable so the startup path can
//! explain exactly why a bundle was rejected — a bundle that fails any
//! check is never served (the binary falls back to a generated error
//! page; see `src/bin/synvelis.rs`).

use serde_json::{Value, json};

use ruviform_ui::MapAssetSource;

use synvelis_app::error::AssetErrorKind;
use synvelis_app::revision::{
    APP_ASSET_SCHEMA, HOST_REVISION, MANIFEST_NAME, check_bundle, sha256_hex,
};

/// Builds a `MapAssetSource` whose manifest truthfully describes
/// `files` — the "honest build" shape `write-manifest.mjs` emits.
fn honest_bundle(files: &[(&str, &[u8])]) -> MapAssetSource {
    let mut source = MapAssetSource::new();
    let mut entries = serde_json::Map::new();
    for (path, bytes) in files {
        source = source.with_asset(path, None, bytes.to_vec());
        entries.insert(
            (*path).to_string(),
            json!({ "bytes": bytes.len(), "sha256": sha256_hex(bytes) }),
        );
    }
    let manifest = json!({
        "schema": APP_ASSET_SCHEMA,
        "frontend": "test-frontend",
        "host": HOST_REVISION,
        "ipc": ruviform_ui::ipc::SCHEMA_VERSION,
        "files": Value::Object(entries),
    });
    source.with_asset(MANIFEST_NAME, None, manifest.to_string().into_bytes())
}

fn kind_of(src: &MapAssetSource) -> AssetErrorKind {
    check_bundle(src)
        .expect_err("bundle must be rejected")
        .kind()
}

#[test]
fn valid_bundle_passes() {
    let src = honest_bundle(&[
        ("index.html", b"<!doctype html>"),
        ("app.js", b"console.log(1)"),
        ("app.css", b"body{}"),
    ]);
    let manifest = check_bundle(&src).expect("honest bundle accepted");
    assert_eq!(manifest.host, HOST_REVISION);
    assert_eq!(manifest.ipc, ruviform_ui::ipc::SCHEMA_VERSION);
}

#[test]
fn missing_manifest_is_explained() {
    let src = MapAssetSource::new().with_asset("index.html", None, b"x".to_vec());
    assert_eq!(kind_of(&src), AssetErrorKind::MissingManifest);
}

#[test]
fn malformed_manifest_is_explained() {
    let src = MapAssetSource::new()
        .with_asset(MANIFEST_NAME, None, b"not json{".to_vec())
        .with_asset("index.html", None, b"x".to_vec());
    assert_eq!(kind_of(&src), AssetErrorKind::MalformedManifest);
}

#[test]
fn schema_mismatch_is_explained() {
    let src = honest_bundle(&[("index.html", b"x")]).with_asset(
        MANIFEST_NAME,
        None,
        json!({
            "schema": APP_ASSET_SCHEMA + 99, "frontend": "x", "host": HOST_REVISION,
            "ipc": 1, "files": {},
        })
        .to_string()
        .into_bytes(),
    );
    assert_eq!(kind_of(&src), AssetErrorKind::SchemaMismatch);
}

#[test]
fn ipc_mismatch_is_explained() {
    let src = honest_bundle(&[("index.html", b"x")]).with_asset(
        MANIFEST_NAME,
        None,
        json!({
            "schema": APP_ASSET_SCHEMA, "frontend": "x", "host": HOST_REVISION,
            "ipc": 999, "files": {},
        })
        .to_string()
        .into_bytes(),
    );
    assert_eq!(kind_of(&src), AssetErrorKind::IpcMismatch);
}

#[test]
fn revision_mismatch_is_explained() {
    let src = honest_bundle(&[("index.html", b"x")]).with_asset(
        MANIFEST_NAME,
        None,
        json!({
            "schema": APP_ASSET_SCHEMA, "frontend": "x",
            "host": "deadbeef-deadbeef-deadbeef-deadbeef-deadbeef",
            "ipc": ruviform_ui::ipc::SCHEMA_VERSION, "files": {},
        })
        .to_string()
        .into_bytes(),
    );
    assert_eq!(kind_of(&src), AssetErrorKind::RevisionMismatch);
}

#[test]
fn missing_entry_is_explained() {
    let mut src = honest_bundle(&[("index.html", b"x")]);
    // Manifest claims an asset that is not in the bundle.
    let manifest = json!({
        "schema": APP_ASSET_SCHEMA, "frontend": "x", "host": HOST_REVISION,
        "ipc": 1,
        "files": {
            "index.html": { "bytes": 1, "sha256": sha256_hex(b"x") },
            "gone.js": { "bytes": 1, "sha256": sha256_hex(b"x") },
        },
    });
    src = src.with_asset(MANIFEST_NAME, None, manifest.to_string().into_bytes());
    assert_eq!(kind_of(&src), AssetErrorKind::MissingEntry);
}

#[test]
fn hash_mismatch_is_explained() {
    let mut src = honest_bundle(&[("index.html", b"real")]);
    let manifest = json!({
        "schema": APP_ASSET_SCHEMA, "frontend": "x", "host": HOST_REVISION,
        "ipc": 1,
        "files": {
            "index.html": { "bytes": 4, "sha256": sha256_hex(b"FAKE") },
        },
    });
    src = src.with_asset(MANIFEST_NAME, None, manifest.to_string().into_bytes());
    assert_eq!(kind_of(&src), AssetErrorKind::HashMismatch);
}

#[test]
fn unsafe_entry_is_explained() {
    let mut src = honest_bundle(&[("index.html", b"x")]);
    let manifest = json!({
        "schema": APP_ASSET_SCHEMA, "frontend": "x", "host": HOST_REVISION,
        "ipc": 1,
        "files": {
            "index.html": { "bytes": 1, "sha256": sha256_hex(b"x") },
            "../escape": { "bytes": 1, "sha256": sha256_hex(b"x") },
        },
    });
    src = src.with_asset(MANIFEST_NAME, None, manifest.to_string().into_bytes());
    assert_eq!(kind_of(&src), AssetErrorKind::UnsafeEntry);
}

#[test]
fn bundle_without_index_html_is_explained() {
    let src = honest_bundle(&[("app.js", b"x")]);
    assert_eq!(kind_of(&src), AssetErrorKind::MissingEntry);
}
