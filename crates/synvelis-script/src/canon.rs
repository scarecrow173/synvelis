//! Canonical JSON serialization and SHA-256 digests.
//!
//! Same convention as `synvelis-automation::canon`: object keys are sorted
//! recursively so equal values always produce equal bytes, and the digest is
//! the lowercase hex SHA-256 of the canonical UTF-8 form. Digests are the
//! drift/staleness anchor shared by the snapshot, model, binding set, and
//! Luau stub.

use std::fmt::Write as _;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::ScriptApiError;

/// Serializes `value` to canonical JSON (sorted object keys, compact).
///
/// # Errors
/// [`ScriptApiError::Json`] — the value could not be serialized.
pub fn canonical_json<T: Serialize>(value: &T) -> Result<String, ScriptApiError> {
    let value = serde_json::to_value(value).map_err(|e| ScriptApiError::Json {
        reason: e.to_string(),
    })?;
    Ok(canonical_value(&value))
}

/// Lowercase hex SHA-256 of the canonical JSON form of `value`.
///
/// # Errors
/// [`ScriptApiError::Json`] — the value could not be serialized.
pub fn digest_of<T: Serialize>(value: &T) -> Result<String, ScriptApiError> {
    Ok(sha256_hex(canonical_json(value)?.as_bytes()))
}

/// Lowercase hex SHA-256 of raw bytes.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn canonical_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_owned()),
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(canonical_value).collect();
            format!("[{}]", inner.join(","))
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .into_iter()
                .map(|k| {
                    let key = serde_json::to_string(k).unwrap_or_else(|_| "\"\"".to_owned());
                    format!("{key}:{}", canonical_value(&map[k]))
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}
