//! Frontend bundle revision compatibility.
//!
//! The built frontend ships a manifest (`synvelis-assets.json`,
//! [`MANIFEST_NAME`]) recording the schema, frontend, host, and IPC
//! revisions it was built against plus a per-file digest table.
//! [`check_bundle`] validates that manifest against the running binary:
//! a mismatch means the bundle was built for a different host/IPC
//! contract and serving it would violate the wire schema, so startup
//! stops with an explainable [`AssetError`] instead.
//!
//! The frontend's manifest writer (`ui/scripts/write-manifest.mjs`)
//! derives `host` from the same `Cargo.lock` source [`HOST_REVISION`]
//! is baked from, so honest builds always agree.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use ruviform_ui::AssetSource;
use ruviform_ui::ipc::SCHEMA_VERSION;

use crate::error::{AssetError, AssetErrorKind};

/// Manifest filename inside the `rvf://app` namespace.
pub const MANIFEST_NAME: &str = "synvelis-assets.json";

/// Manifest schema version; bump when the manifest shape changes.
pub const APP_ASSET_SCHEMA: u32 = 1;

/// Ruviform git revision this binary was linked against, baked in by
/// `build.rs` from `Cargo.lock` (`"unknown"` when the lock is absent —
/// a checkout state that fails the manifest host check by design).
pub const HOST_REVISION: &str = env!("SYNVELIS_RUVIFORM_REV");

/// Per-file digest entry in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Byte length of the file.
    pub bytes: usize,
    /// Lowercase hex SHA-256 of the file contents.
    pub sha256: String,
}

/// The parsed `synvelis-assets.json` manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetManifest {
    /// Manifest schema version — must equal [`APP_ASSET_SCHEMA`].
    pub schema: u32,
    /// Frontend build revision (informational provenance string).
    pub frontend: String,
    /// Ruviform git revision the bundle was built against — must equal
    /// [`HOST_REVISION`].
    pub host: String,
    /// IPC schema version the bundle targets — must equal
    /// [`SCHEMA_VERSION`].
    pub ipc: u32,
    /// Digest table of every file in the bundle (keys include
    /// `index.html` and the manifest itself is excluded).
    pub files: std::collections::BTreeMap<String, ManifestEntry>,
}

/// Validates the bundle served by `assets` against this binary.
///
/// Checks, in order: manifest present → parses → manifest schema →
/// IPC schema → host revision → every manifest entry exists, lies
/// inside the `rvf://app` namespace, and matches its recorded byte
/// length + SHA-256. `index.html` must additionally be present.
///
/// # Errors
/// [`AssetError`] with a [`AssetErrorKind`] identifying which check
/// failed — the caller reports it to the user and stops instead of
/// serving a mismatched bundle.
pub fn check_bundle(assets: &dyn AssetSource) -> Result<AssetManifest, AssetError> {
    let manifest_asset = assets.get(MANIFEST_NAME).ok_or_else(|| {
        AssetError::new(
            AssetErrorKind::MissingManifest,
            format!(
                "{MANIFEST_NAME} not found in the asset bundle — build the frontend \
                 (`mise run build-ui`) before launching"
            ),
        )
    })?;
    let manifest: AssetManifest = serde_json::from_slice(&manifest_asset.bytes).map_err(|e| {
        AssetError::new(
            AssetErrorKind::MalformedManifest,
            format!("{MANIFEST_NAME} is not a valid manifest: {e}"),
        )
    })?;

    if manifest.schema != APP_ASSET_SCHEMA {
        return Err(AssetError::new(
            AssetErrorKind::SchemaMismatch,
            format!(
                "manifest schema {} != host schema {APP_ASSET_SCHEMA} — rebuild the frontend",
                manifest.schema
            ),
        ));
    }
    if manifest.ipc != SCHEMA_VERSION {
        return Err(AssetError::new(
            AssetErrorKind::IpcMismatch,
            format!(
                "manifest ipc {} != host ipc {SCHEMA_VERSION} — frontend targets a \
                 different wire schema; rebuild",
                manifest.ipc
            ),
        ));
    }
    if manifest.host != HOST_REVISION {
        return Err(AssetError::new(
            AssetErrorKind::RevisionMismatch,
            format!(
                "manifest host revision {} != binary host revision {HOST_REVISION} — \
                 bundle was built against a different ruviform; rebuild",
                manifest.host
            ),
        ));
    }

    let mut saw_index = false;
    for (path, entry) in &manifest.files {
        if path.contains("..") || path.starts_with('/') || path.starts_with('\\') {
            return Err(AssetError::new(
                AssetErrorKind::UnsafeEntry,
                format!("manifest entry escapes the app namespace: {path:?}"),
            ));
        }
        let asset = assets.get(path).ok_or_else(|| {
            AssetError::new(
                AssetErrorKind::MissingEntry,
                format!("manifest entry {path:?} is absent from the bundle"),
            )
        })?;
        if asset.bytes.len() != entry.bytes || sha256_hex(&asset.bytes) != entry.sha256 {
            return Err(AssetError::new(
                AssetErrorKind::HashMismatch,
                format!("manifest entry {path:?} does not match its recorded digest"),
            ));
        }
        saw_index |= path == "index.html";
    }
    if !saw_index {
        return Err(AssetError::new(
            AssetErrorKind::MissingEntry,
            "manifest does not list index.html — not a Synvelis shell bundle".to_string(),
        ));
    }
    Ok(manifest)
}

/// Hex SHA-256 of `bytes` — used by the manifest writer side too
/// (kept symmetric with `ui/scripts/write-manifest.mjs`).
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().fold(String::new(), |mut out, b| {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// Serializes `manifest` as it would appear in `synvelis-assets.json`
/// (test + tooling symmetry — the JS writer emits the same shape).
#[must_use]
pub fn manifest_to_json(manifest: &AssetManifest) -> Value {
    serde_json::to_value(manifest).unwrap_or(Value::Null)
}
